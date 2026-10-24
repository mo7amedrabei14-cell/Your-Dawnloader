//! Server-side PostgreSQL access for identity, sessions and user-scoped synced metadata.
//! Account identity is always derived from verified OIDC claims, never request JSON.

use std::time::Duration;

use dawn_api_contract::{DownloadHistoryPage, DownloadRecordResponse, UpsertDownloadRecordRequest};
use sqlx::{postgres::PgPoolOptions, PgPool, Postgres, Row, Transaction};
use thiserror::Error;
use uuid::Uuid;

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("database operation failed: {0}")]
    Database(#[from] sqlx::Error),
    #[error("database migration failed: {0}")]
    Migration(String),
    #[error("database URL is empty")]
    MissingUrl,
}

#[derive(Clone)]
pub struct Storage { pool: PgPool }

#[derive(Debug, Clone)]
pub struct LoginState {
    pub nonce: String,
    pub pkce_verifier: String,
    pub return_to: Option<String>,
}

#[derive(Debug, Clone)]
pub struct DeviceIdentity {
    pub identity: SessionIdentity,
    pub device_id: Uuid,
    pub device_label: Option<String>,
    pub platform: String,
}

#[derive(Debug, Clone)]
pub enum DeviceLinkStatus {
    Pending,
    Authorized(Box<DeviceIdentity>),
    Expired,
    Revoked,
}

#[derive(Debug, Clone)]
pub struct SessionIdentity {
    pub user_id: Uuid,
    pub issuer: String,
    pub subject: String,
    pub email: Option<String>,
    pub email_verified: bool,
    pub display_name: Option<String>,
    pub locale: String,
    pub sync_download_history: bool,
    pub role: String,
    pub account_status: String,
}

impl Storage {
    pub async fn connect(database_url: &str) -> Result<Self, StorageError> {
        if database_url.trim().is_empty() { return Err(StorageError::MissingUrl); }
        let pool = PgPoolOptions::new().max_connections(10)
            .acquire_timeout(Duration::from_secs(5))
            .connect(database_url).await?;
        Ok(Self { pool })
    }

    pub async fn migrate(&self) -> Result<(), StorageError> {
        MIGRATOR.run(&self.pool).await.map_err(|error| StorageError::Migration(error.to_string()))
    }

    pub async fn begin_user_transaction(&self, verified_user_id: Uuid) -> Result<Transaction<'_, Postgres>, StorageError> {
        let mut transaction = self.pool.begin().await?;
        sqlx::query("SELECT set_config('app.user_id', $1, true)")
            .bind(verified_user_id.to_string())
            .execute(&mut *transaction).await?;
        Ok(transaction)
    }

    pub async fn ping(&self) -> Result<(), StorageError> {
        sqlx::query("SELECT 1").execute(&self.pool).await?;
        Ok(())
    }

    pub async fn create_login_state(&self, state_hash: &str, nonce: &str, pkce_verifier: &str, return_to: &str) -> Result<(), StorageError> {
        sqlx::query("INSERT INTO oidc_login_states(state_hash, nonce, pkce_verifier, return_to) VALUES ($1,$2,$3,$4)")
            .bind(state_hash).bind(nonce).bind(pkce_verifier).bind(return_to)
            .execute(&self.pool).await?;
        Ok(())
    }

    /// Atomic, single-use consumption avoids OAuth callback replay.
    pub async fn consume_login_state(&self, state_hash: &str) -> Result<Option<LoginState>, StorageError> {
        sqlx::query("DELETE FROM oidc_login_states WHERE expires_at <= now()").execute(&self.pool).await?;
        let row = sqlx::query(
            "DELETE FROM oidc_login_states WHERE state_hash=$1 AND expires_at > now()
             RETURNING nonce, pkce_verifier, return_to"
        )
        .bind(state_hash).fetch_optional(&self.pool).await?;
        if let Some(row) = row {
            Ok(Some(LoginState {
                nonce: row.try_get::<String, _>("nonce")?,
                pkce_verifier: row.try_get::<String, _>("pkce_verifier")?,
                return_to: row.try_get::<Option<String>, _>("return_to")?,
            }))
        } else {
            Ok(None)
        }
    }

    pub async fn upsert_oidc_user(
        &self,
        issuer: &str,
        subject: &str,
        email: Option<&str>,
        email_verified: bool,
        display_name: Option<&str>,
        owner_email: &str,
        owner_display_name: &str,
    ) -> Result<Uuid, StorageError> {
        let is_owner = email_verified && email
            .is_some_and(|candidate| candidate.trim().eq_ignore_ascii_case(owner_email.trim()));
        let account_display_name = if is_owner { Some(owner_display_name) } else { display_name };
        let mut tx = self.pool.begin().await?;
        let proposed_id = Uuid::new_v4();
        let row = sqlx::query(
            "INSERT INTO app_users(id, identity_issuer, identity_subject)
             VALUES ($1,$2,$3)
             ON CONFLICT(identity_issuer, identity_subject)
             DO UPDATE SET identity_subject=EXCLUDED.identity_subject
             RETURNING id"
        )
        .bind(proposed_id).bind(issuer).bind(subject)
        .fetch_one(&mut *tx).await?;
        let user_id: Uuid = row.try_get("id")?;

        // The RLS context is transaction-local and comes from the verified identity mapping.
        sqlx::query("SELECT set_config('app.user_id', $1, true)")
            .bind(user_id.to_string()).execute(&mut *tx).await?;

        sqlx::query(
            "INSERT INTO user_profiles(owner_user_id, display_name, email, email_verified)
             VALUES ($1,$2,CASE WHEN $4 THEN $3 ELSE NULL END,$4)
             ON CONFLICT(owner_user_id) DO UPDATE SET
               display_name=COALESCE(EXCLUDED.display_name, user_profiles.display_name),
               email=CASE WHEN EXCLUDED.email_verified THEN EXCLUDED.email ELSE user_profiles.email END,
               email_verified=(user_profiles.email_verified OR EXCLUDED.email_verified),
               updated_at=now()"
        )
        .bind(user_id).bind(account_display_name).bind(email).bind(email_verified)
        .execute(&mut *tx).await?;

        if is_owner {
            sqlx::query("UPDATE app_users SET role='owner', account_status='active' WHERE id=$1")
                .bind(user_id).execute(&mut *tx).await?;
        }

        sqlx::query("INSERT INTO account_preferences(owner_user_id) VALUES ($1) ON CONFLICT(owner_user_id) DO NOTHING")
            .bind(user_id).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(user_id)
    }

    pub async fn create_session(
        &self,
        owner_user_id: Uuid,
        session_token_hash: &str,
        csrf_token_hash: &str,
        expires_at_unix: i64,
    ) -> Result<(), StorageError> {
        let mut tx = self.begin_user_transaction(owner_user_id).await?;
        sqlx::query(
            "INSERT INTO account_sessions(id, owner_user_id, session_token_hash, csrf_token_hash, expires_at)
             VALUES ($1,$2,$3,$4,to_timestamp($5))"
        )
        .bind(Uuid::new_v4()).bind(owner_user_id).bind(session_token_hash).bind(csrf_token_hash)
        .bind(expires_at_unix).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn session_identity(&self, session_token_hash: &str) -> Result<Option<SessionIdentity>, StorageError> {
        // First resolve only the owner from an active opaque session, then open an
        // owner-scoped transaction for RLS-protected profile/preferences tables.
        let owner = sqlx::query(
            "SELECT owner_user_id FROM account_sessions
             WHERE session_token_hash=$1 AND revoked_at IS NULL AND expires_at > now()"
        )
        .bind(session_token_hash).fetch_optional(&self.pool).await?;
        let Some(owner) = owner else { return Ok(None) };
        let owner_user_id: Uuid = owner.try_get("owner_user_id")?;
        let mut tx = self.begin_user_transaction(owner_user_id).await?;
        let row = sqlx::query(
            "SELECT u.id AS user_id, u.identity_issuer AS issuer, u.identity_subject AS subject,
                    p.email, p.email_verified, p.display_name, p.locale,
                    COALESCE(pref.sync_download_history, FALSE) AS sync_download_history,
                    u.role, u.account_status
             FROM account_sessions s
             JOIN app_users u ON u.id=s.owner_user_id
             LEFT JOIN user_profiles p ON p.owner_user_id=u.id
             LEFT JOIN account_preferences pref ON pref.owner_user_id=u.id
             WHERE s.session_token_hash=$1 AND s.owner_user_id=$2
               AND s.revoked_at IS NULL AND s.expires_at > now()"
        )
        .bind(session_token_hash).bind(owner_user_id).fetch_optional(&mut *tx).await?;
        let identity = if let Some(row) = row {
            Some(SessionIdentity {
                user_id: row.try_get("user_id")?,
                issuer: row.try_get("issuer")?,
                subject: row.try_get("subject")?,
                email: row.try_get("email")?,
                email_verified: row.try_get("email_verified")?,
                display_name: row.try_get("display_name")?,
                locale: row.try_get("locale")?,
                sync_download_history: row.try_get("sync_download_history")?,
                role: row.try_get("role")?,
                account_status: row.try_get("account_status")?,
            })
        } else {
            None
        };
        tx.commit().await?;
        Ok(identity)
    }

    pub async fn validate_session_csrf(&self, session_token_hash: &str, csrf_token_hash: &str) -> Result<bool, StorageError> {
        Ok(sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM account_sessions
             WHERE session_token_hash=$1 AND csrf_token_hash=$2
               AND revoked_at IS NULL AND expires_at > now())"
        )
        .bind(session_token_hash).bind(csrf_token_hash).fetch_one(&self.pool).await?)
    }

    pub async fn revoke_session(&self, session_token_hash: &str) -> Result<(), StorageError> {
        sqlx::query("UPDATE account_sessions SET revoked_at=now() WHERE session_token_hash=$1 AND revoked_at IS NULL")
            .bind(session_token_hash).execute(&self.pool).await?;
        Ok(())
    }

    pub async fn set_download_sync(&self, user_id: Uuid, enabled: bool) -> Result<(), StorageError> {
        let mut tx = self.begin_user_transaction(user_id).await?;
        sqlx::query(
            "INSERT INTO account_preferences(owner_user_id, sync_download_history) VALUES ($1,$2)
             ON CONFLICT(owner_user_id) DO UPDATE SET sync_download_history=EXCLUDED.sync_download_history, updated_at=now()"
        )
        .bind(user_id).bind(enabled).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn list_download_records(&self, user_id: Uuid, limit: u32) -> Result<DownloadHistoryPage, StorageError> {
        let mut tx = self.begin_user_transaction(user_id).await?;
        let rows = sqlx::query(
            "SELECT id, source_domain, provider_item_key, display_name, media_kind, container, quality_label, status,
                    (EXTRACT(EPOCH FROM completed_at) * 1000)::BIGINT AS completed_at_unix_ms,
                    (EXTRACT(EPOCH FROM created_at) * 1000)::BIGINT AS created_at_unix_ms
             FROM download_records ORDER BY created_at DESC, id DESC LIMIT $1"
        )
        .bind(i64::from(limit.clamp(1, 100))).fetch_all(&mut *tx).await?;
        let mut items = Vec::with_capacity(rows.len());
        for row in rows {
            let media_kind = match row.try_get::<String, _>("media_kind")?.as_str() {
                "audio" => dawn_api_contract::SyncedMediaKind::Audio,
                "video" => dawn_api_contract::SyncedMediaKind::Video,
                "image" => dawn_api_contract::SyncedMediaKind::Image,
                "document" => dawn_api_contract::SyncedMediaKind::Document,
                _ => dawn_api_contract::SyncedMediaKind::Unknown,
            };
            let status = match row.try_get::<String, _>("status")?.as_str() {
                "failed" => dawn_api_contract::SyncedDownloadStatus::Failed,
                "cancelled" => dawn_api_contract::SyncedDownloadStatus::Cancelled,
                _ => dawn_api_contract::SyncedDownloadStatus::Completed,
            };
            items.push(DownloadRecordResponse {
                id: row.try_get("id")?,
                source_domain: row.try_get("source_domain")?,
                provider_item_key: row.try_get("provider_item_key")?,
                display_name: row.try_get("display_name")?,
                media_kind,
                container: row.try_get("container")?,
                quality_label: row.try_get("quality_label")?,
                status,
                completed_at_unix_ms: row.try_get("completed_at_unix_ms")?,
                created_at_unix_ms: row.try_get("created_at_unix_ms")?,
            });
        }
        tx.commit().await?;
        Ok(DownloadHistoryPage { items, next_cursor: None })
    }

    pub async fn upsert_download_record(
        &self, user_id: Uuid, request: &UpsertDownloadRecordRequest,
    ) -> Result<DownloadRecordResponse, StorageError> {
        let mut tx = self.begin_user_transaction(user_id).await?;
        let media_kind = match request.media_kind {
            dawn_api_contract::SyncedMediaKind::Audio => "audio",
            dawn_api_contract::SyncedMediaKind::Video => "video",
            dawn_api_contract::SyncedMediaKind::Image => "image",
            dawn_api_contract::SyncedMediaKind::Document => "document",
            dawn_api_contract::SyncedMediaKind::Unknown => "unknown",
        };
        let status = match request.status {
            dawn_api_contract::SyncedDownloadStatus::Completed => "completed",
            dawn_api_contract::SyncedDownloadStatus::Failed => "failed",
            dawn_api_contract::SyncedDownloadStatus::Cancelled => "cancelled",
        };
        let row = sqlx::query(
            "INSERT INTO download_records(
               id, owner_user_id, client_record_id, source_domain, provider_item_key, display_name,
               media_kind, container, quality_label, status, completed_at
             )
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,
                     CASE WHEN $11::BIGINT IS NULL THEN NULL ELSE to_timestamp($11::DOUBLE PRECISION / 1000.0) END)
             ON CONFLICT(owner_user_id, client_record_id) DO UPDATE SET
               source_domain=EXCLUDED.source_domain, provider_item_key=EXCLUDED.provider_item_key,
               display_name=EXCLUDED.display_name, media_kind=EXCLUDED.media_kind,
               container=EXCLUDED.container, quality_label=EXCLUDED.quality_label,
               status=EXCLUDED.status, completed_at=EXCLUDED.completed_at, updated_at=now()
             RETURNING id, source_domain, provider_item_key, display_name, media_kind, container, quality_label, status,
                       (EXTRACT(EPOCH FROM completed_at) * 1000)::BIGINT AS completed_at_unix_ms,
                       (EXTRACT(EPOCH FROM created_at) * 1000)::BIGINT AS created_at_unix_ms"
        )
        .bind(Uuid::new_v4()).bind(user_id).bind(request.client_record_id)
        .bind(&request.source_domain).bind(&request.provider_item_key).bind(&request.display_name)
        .bind(media_kind).bind(&request.container).bind(&request.quality_label).bind(status)
        .bind(request.completed_at_unix_ms).fetch_one(&mut *tx).await?;
        let kind = match row.try_get::<String, _>("media_kind")?.as_str() {
            "audio" => dawn_api_contract::SyncedMediaKind::Audio,
            "video" => dawn_api_contract::SyncedMediaKind::Video,
            "image" => dawn_api_contract::SyncedMediaKind::Image,
            "document" => dawn_api_contract::SyncedMediaKind::Document,
            _ => dawn_api_contract::SyncedMediaKind::Unknown,
        };
        let record_status = match row.try_get::<String, _>("status")?.as_str() {
            "failed" => dawn_api_contract::SyncedDownloadStatus::Failed,
            "cancelled" => dawn_api_contract::SyncedDownloadStatus::Cancelled,
            _ => dawn_api_contract::SyncedDownloadStatus::Completed,
        };
        let record = DownloadRecordResponse {
            id: row.try_get("id")?,
            source_domain: row.try_get("source_domain")?,
            provider_item_key: row.try_get("provider_item_key")?,
            display_name: row.try_get("display_name")?,
            media_kind: kind,
            container: row.try_get("container")?,
            quality_label: row.try_get("quality_label")?,
            status: record_status,
            completed_at_unix_ms: row.try_get("completed_at_unix_ms")?,
            created_at_unix_ms: row.try_get("created_at_unix_ms")?,
        };
        tx.commit().await?;
        Ok(record)
    }

    pub async fn delete_download_record(&self, user_id: Uuid, record_id: Uuid) -> Result<bool, StorageError> {
        let mut tx = self.begin_user_transaction(user_id).await?;
        let result = sqlx::query("DELETE FROM download_records WHERE owner_user_id=$1 AND id=$2")
            .bind(user_id).bind(record_id).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn clear_download_records(&self, user_id: Uuid) -> Result<u64, StorageError> {
        let mut tx = self.begin_user_transaction(user_id).await?;
        let result = sqlx::query("DELETE FROM download_records WHERE owner_user_id=$1")
            .bind(user_id).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(result.rows_affected())
    }

    /// Produces an account data export without session/CSRF token hashes or media bytes.
    pub async fn export_account_data(&self, user_id: Uuid) -> Result<serde_json::Value, StorageError> {
        let mut tx = self.begin_user_transaction(user_id).await?;

        let account = sqlx::query(
            "SELECT id, identity_issuer, identity_subject,
                    to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at
             FROM app_users WHERE id=$1"
        )
        .bind(user_id).fetch_one(&mut *tx).await?;

        let profile = sqlx::query(
            "SELECT display_name, email, email_verified, locale,
                    to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at
             FROM user_profiles WHERE owner_user_id=$1"
        )
        .bind(user_id).fetch_optional(&mut *tx).await?;

        let preferences = sqlx::query(
            "SELECT sync_download_history, theme,
                    to_char(updated_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS updated_at
             FROM account_preferences WHERE owner_user_id=$1"
        )
        .bind(user_id).fetch_optional(&mut *tx).await?;

        let device_rows = sqlx::query(
            "SELECT id, device_label, platform,
                    to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at,
                    to_char(last_seen_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS last_seen_at,
                    (revoked_at IS NOT NULL) AS revoked
             FROM user_devices WHERE owner_user_id=$1 ORDER BY created_at ASC"
        )
        .bind(user_id).fetch_all(&mut *tx).await?;

        let download_rows = sqlx::query(
            "SELECT id, source_domain, provider_item_key, display_name, media_kind, container, quality_label, status,
                    (EXTRACT(EPOCH FROM completed_at) * 1000)::BIGINT AS completed_at_unix_ms,
                    (EXTRACT(EPOCH FROM created_at) * 1000)::BIGINT AS created_at_unix_ms
             FROM download_records WHERE owner_user_id=$1 ORDER BY created_at ASC, id ASC"
        )
        .bind(user_id).fetch_all(&mut *tx).await?;

        let devices = device_rows.into_iter().map(|row| {
            Ok(serde_json::json!({
                "id": row.try_get::<Uuid, _>("id")?,
                "label": row.try_get::<Option<String>, _>("device_label")?,
                "platform": row.try_get::<String, _>("platform")?,
                "created_at": row.try_get::<Option<String>, _>("created_at")?,
                "last_seen_at": row.try_get::<Option<String>, _>("last_seen_at")?,
                "revoked": row.try_get::<bool, _>("revoked")?
            }))
        }).collect::<Result<Vec<_>, sqlx::Error>>()?;

        let downloads = download_rows.into_iter().map(|row| {
            Ok(serde_json::json!({
                "id": row.try_get::<Uuid, _>("id")?,
                "source_domain": row.try_get::<String, _>("source_domain")?,
                "provider_item_key": row.try_get::<Option<String>, _>("provider_item_key")?,
                "display_name": row.try_get::<Option<String>, _>("display_name")?,
                "media_kind": row.try_get::<String, _>("media_kind")?,
                "container": row.try_get::<Option<String>, _>("container")?,
                "quality_label": row.try_get::<Option<String>, _>("quality_label")?,
                "status": row.try_get::<String, _>("status")?,
                "completed_at_unix_ms": row.try_get::<Option<i64>, _>("completed_at_unix_ms")?,
                "created_at_unix_ms": row.try_get::<i64, _>("created_at_unix_ms")?
            }))
        }).collect::<Result<Vec<_>, sqlx::Error>>()?;

        let account_json = serde_json::json!({
            "id": account.try_get::<Uuid, _>("id")?,
            "identity_issuer": account.try_get::<String, _>("identity_issuer")?,
            "identity_subject": account.try_get::<String, _>("identity_subject")?,
            "created_at": account.try_get::<Option<String>, _>("created_at")?
        });
        let profile_json = if let Some(row) = profile {
            serde_json::json!({
                "display_name": row.try_get::<Option<String>, _>("display_name")?,
                "email": row.try_get::<Option<String>, _>("email")?,
                "email_verified": row.try_get::<bool, _>("email_verified")?,
                "locale": row.try_get::<String, _>("locale")?,
                "created_at": row.try_get::<Option<String>, _>("created_at")?
            })
        } else { serde_json::Value::Null };
        let preferences_json = if let Some(row) = preferences {
            serde_json::json!({
                "sync_download_history": row.try_get::<bool, _>("sync_download_history")?,
                "theme": row.try_get::<String, _>("theme")?,
                "updated_at": row.try_get::<Option<String>, _>("updated_at")?
            })
        } else { serde_json::Value::Null };

        tx.commit().await?;
        Ok(serde_json::json!({
            "schema_version": 1,
            "exported_at": sqlx::query_scalar::<_, String>("SELECT to_char(now() AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"')")
                .fetch_one(&self.pool).await?,
            "account": account_json,
            "profile": profile_json,
            "preferences": preferences_json,
            "devices": devices,
            "download_history": downloads,
            "notice": "This export contains account metadata, not media files, local device paths, or active session credentials."
        }))
    }

    pub async fn create_device_link_request(
        &self,
        device_code_hash: &str,
        user_code_hash: &str,
        device_label: &str,
        platform: &str,
        expires_at_unix: i64,
    ) -> Result<(), StorageError> {
        sqlx::query("DELETE FROM device_link_requests WHERE approved_at IS NULL AND expires_at <= now()")
            .execute(&self.pool).await?;
        sqlx::query(
            "INSERT INTO device_link_requests(
                id, device_code_hash, user_code_hash, device_label, platform, expires_at
             ) VALUES ($1,$2,$3,$4,$5,to_timestamp($6))"
        )
        .bind(Uuid::new_v4()).bind(device_code_hash).bind(user_code_hash)
        .bind(device_label).bind(platform).bind(expires_at_unix)
        .execute(&self.pool).await?;
        Ok(())
    }

    /// Atomically associates the pending device code with the already authenticated website account.
    pub async fn approve_device_link(&self, owner_user_id: Uuid, user_code_hash: &str) -> Result<Option<Uuid>, StorageError> {
        let mut tx = self.begin_user_transaction(owner_user_id).await?;
        let request = sqlx::query(
            "SELECT id, device_label, platform
             FROM device_link_requests
             WHERE user_code_hash=$1 AND owner_user_id IS NULL AND device_id IS NULL
               AND approved_at IS NULL AND revoked_at IS NULL AND expires_at > now()
             FOR UPDATE"
        )
        .bind(user_code_hash).fetch_optional(&mut *tx).await?;
        let Some(request) = request else { return Ok(None) };
        let request_id: Uuid = request.try_get("id")?;
        let device_label: String = request.try_get("device_label")?;
        let platform: String = request.try_get("platform")?;
        let device_id = Uuid::new_v4();

        sqlx::query("INSERT INTO user_devices(id, owner_user_id, device_label, platform) VALUES ($1,$2,$3,$4)")
            .bind(device_id).bind(owner_user_id).bind(device_label).bind(platform)
            .execute(&mut *tx).await?;
        let updated = sqlx::query(
            "UPDATE device_link_requests SET owner_user_id=$2, device_id=$3, approved_at=now()
             WHERE id=$1 AND owner_user_id IS NULL AND approved_at IS NULL"
        )
        .bind(request_id).bind(owner_user_id).bind(device_id).execute(&mut *tx).await?;
        if updated.rows_affected() != 1 {
            return Err(StorageError::Migration("device authorization changed while being approved".to_owned()));
        }
        tx.commit().await?;
        Ok(Some(device_id))
    }

    pub async fn device_link_status(&self, device_code_hash: &str) -> Result<Option<DeviceLinkStatus>, StorageError> {
        let row = sqlx::query(
            "SELECT owner_user_id, approved_at IS NOT NULL AS approved,
                    revoked_at IS NOT NULL AS revoked, expires_at > now() AS unexpired
             FROM device_link_requests WHERE device_code_hash=$1"
        )
        .bind(device_code_hash).fetch_optional(&self.pool).await?;
        let Some(row) = row else { return Ok(None) };
        let approved: bool = row.try_get("approved")?;
        let revoked: bool = row.try_get("revoked")?;
        let unexpired: bool = row.try_get("unexpired")?;
        let owner_user_id: Option<Uuid> = row.try_get("owner_user_id")?;
        if revoked {
            return Ok(Some(DeviceLinkStatus::Revoked));
        }
        if approved && owner_user_id.is_some() {
            return Ok(match self.device_identity(device_code_hash).await? {
                Some(identity) => Some(DeviceLinkStatus::Authorized(Box::new(identity))),
                None => Some(DeviceLinkStatus::Revoked),
            });
        }
        if !unexpired {
            return Ok(Some(DeviceLinkStatus::Expired));
        }
        Ok(Some(DeviceLinkStatus::Pending))
    }

    /// A device code becomes a bearer credential only after approval. Only its hash is stored.
    pub async fn device_identity(&self, device_code_hash: &str) -> Result<Option<DeviceIdentity>, StorageError> {
        let owner = sqlx::query(
            "SELECT owner_user_id, device_id FROM device_link_requests
             WHERE device_code_hash=$1 AND owner_user_id IS NOT NULL
               AND device_id IS NOT NULL AND approved_at IS NOT NULL AND revoked_at IS NULL"
        )
        .bind(device_code_hash).fetch_optional(&self.pool).await?;
        let Some(owner) = owner else { return Ok(None) };
        let owner_user_id: Uuid = owner.try_get("owner_user_id")?;
        let device_id: Uuid = owner.try_get("device_id")?;
        let mut tx = self.begin_user_transaction(owner_user_id).await?;
        let row = sqlx::query(
            "SELECT u.id AS user_id, u.identity_issuer AS issuer, u.identity_subject AS subject,
                    p.email, p.email_verified, p.display_name, p.locale,
                    COALESCE(pref.sync_download_history, FALSE) AS sync_download_history,
                    u.role, u.account_status,
                    d.id AS device_id, d.device_label, d.platform
             FROM device_link_requests r
             JOIN user_devices d ON d.id=r.device_id AND d.owner_user_id=r.owner_user_id
             JOIN app_users u ON u.id=r.owner_user_id
             LEFT JOIN user_profiles p ON p.owner_user_id=u.id
             LEFT JOIN account_preferences pref ON pref.owner_user_id=u.id
             WHERE r.device_code_hash=$1 AND r.owner_user_id=$2 AND r.device_id=$3
               AND r.approved_at IS NOT NULL AND r.revoked_at IS NULL AND d.revoked_at IS NULL
               AND u.account_status='active'"
        )
        .bind(device_code_hash).bind(owner_user_id).bind(device_id)
        .fetch_optional(&mut *tx).await?;
        let Some(row) = row else {
            tx.commit().await?;
            return Ok(None);
        };
        let identity = SessionIdentity {
            user_id: row.try_get("user_id")?,
            issuer: row.try_get("issuer")?,
            subject: row.try_get("subject")?,
            email: row.try_get("email")?,
            email_verified: row.try_get("email_verified")?,
            display_name: row.try_get("display_name")?,
            locale: row.try_get("locale")?,
            sync_download_history: row.try_get("sync_download_history")?,
            role: row.try_get("role")?,
            account_status: row.try_get("account_status")?,
        };
        let device = DeviceIdentity {
            identity,
            device_id: row.try_get("device_id")?,
            device_label: row.try_get("device_label")?,
            platform: row.try_get("platform")?,
        };
        sqlx::query("UPDATE user_devices SET last_seen_at=now() WHERE id=$1 AND owner_user_id=$2")
            .bind(device_id).bind(owner_user_id).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(Some(device))
    }

    pub async fn revoke_device(&self, owner_user_id: Uuid, device_id: Uuid) -> Result<bool, StorageError> {
        let mut tx = self.begin_user_transaction(owner_user_id).await?;
        let result = sqlx::query(
            "UPDATE user_devices SET revoked_at=COALESCE(revoked_at, now())
             WHERE owner_user_id=$1 AND id=$2 AND revoked_at IS NULL"
        )
        .bind(owner_user_id).bind(device_id).execute(&mut *tx).await?;
        sqlx::query(
            "UPDATE device_link_requests SET revoked_at=COALESCE(revoked_at, now())
             WHERE owner_user_id=$1 AND device_id=$2 AND revoked_at IS NULL"
        )
        .bind(owner_user_id).bind(device_id).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn list_devices(&self, user_id: Uuid) -> Result<Vec<serde_json::Value>, StorageError> {
        let mut tx = self.begin_user_transaction(user_id).await?;
        let rows = sqlx::query(
            "SELECT id, device_label, platform,
                    to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at,
                    to_char(last_seen_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS last_seen_at,
                    revoked_at IS NOT NULL AS revoked
             FROM user_devices WHERE owner_user_id=$1
             ORDER BY (revoked_at IS NOT NULL) ASC, last_seen_at DESC NULLS LAST, created_at DESC"
        )
        .bind(user_id).fetch_all(&mut *tx).await?;
        let mut devices = Vec::with_capacity(rows.len());
        for row in rows {
            devices.push(serde_json::json!({
                "id": row.try_get::<Uuid, _>("id")?,
                "label": row.try_get::<Option<String>, _>("device_label")?,
                "platform": row.try_get::<String, _>("platform")?,
                "created_at": row.try_get::<Option<String>, _>("created_at")?,
                "last_seen_at": row.try_get::<Option<String>, _>("last_seen_at")?,
                "revoked": row.try_get::<bool, _>("revoked")?
            }));
        }
        tx.commit().await?;
        Ok(devices)
    }

    /// Owner-only listing. The actor ID must come from the already verified browser session.
    pub async fn list_admin_users(&self, actor_user_id: Uuid, limit: u32) -> Result<Vec<serde_json::Value>, StorageError> {
        let mut tx = self.begin_user_transaction(actor_user_id).await?;
        let rows = sqlx::query(
            "SELECT u.id, u.role, u.account_status,
                    p.email, p.email_verified, p.display_name, p.locale,
                    to_char(u.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at
             FROM app_users u
             LEFT JOIN user_profiles p ON p.owner_user_id=u.id
             ORDER BY u.created_at DESC, u.id ASC
             LIMIT $1"
        )
        .bind(i64::from(limit.clamp(1, 100)))
        .fetch_all(&mut *tx).await?;
        let mut users = Vec::with_capacity(rows.len());
        for row in rows {
            users.push(serde_json::json!({
                "id": row.try_get::<Uuid, _>("id")?,
                "role": row.try_get::<String, _>("role")?,
                "account_status": row.try_get::<String, _>("account_status")?,
                "email": row.try_get::<Option<String>, _>("email")?,
                "email_verified": row.try_get::<bool, _>("email_verified")?,
                "display_name": row.try_get::<Option<String>, _>("display_name")?,
                "locale": row.try_get::<Option<String>, _>("locale")?,
                "created_at": row.try_get::<Option<String>, _>("created_at")?
            }));
        }
        tx.commit().await?;
        Ok(users)
    }

    /// Updates a non-owner account only. The authenticated actor must be checked as owner by the API.
    pub async fn update_admin_user(
        &self,
        actor_user_id: Uuid,
        target_user_id: Uuid,
        role: Option<&str>,
        account_status: Option<&str>,
        display_name: Option<&str>,
        locale: Option<&str>,
    ) -> Result<Option<serde_json::Value>, StorageError> {
        let mut tx = self.begin_user_transaction(actor_user_id).await?;
        let target = sqlx::query("SELECT role FROM app_users WHERE id=$1 FOR UPDATE")
            .bind(target_user_id).fetch_optional(&mut *tx).await?;
        let Some(target) = target else {
            tx.rollback().await?;
            return Ok(None);
        };
        let current_role: String = target.try_get("role")?;
        if current_role == "owner" || target_user_id == actor_user_id {
            tx.rollback().await?;
            return Ok(None);
        }

        if let Some(role) = role {
            sqlx::query("UPDATE app_users SET role=$2 WHERE id=$1")
                .bind(target_user_id).bind(role).execute(&mut *tx).await?;
        }
        if let Some(account_status) = account_status {
            sqlx::query("UPDATE app_users SET account_status=$2 WHERE id=$1")
                .bind(target_user_id).bind(account_status).execute(&mut *tx).await?;
            if account_status == "suspended" {
                sqlx::query("UPDATE account_sessions SET revoked_at=COALESCE(revoked_at, now()) WHERE owner_user_id=$1 AND revoked_at IS NULL")
                    .bind(target_user_id).execute(&mut *tx).await?;
                sqlx::query("UPDATE user_devices SET revoked_at=COALESCE(revoked_at, now()) WHERE owner_user_id=$1 AND revoked_at IS NULL")
                    .bind(target_user_id).execute(&mut *tx).await?;
                sqlx::query("UPDATE device_link_requests SET revoked_at=COALESCE(revoked_at, now()) WHERE owner_user_id=$1 AND revoked_at IS NULL")
                    .bind(target_user_id).execute(&mut *tx).await?;
            }
        }
        if display_name.is_some() || locale.is_some() {
            // app.user_id is still the authenticated owner; the role-aware profile policy allows
            // owners to edit non-owner profiles while ordinary sessions remain row-scoped.
            sqlx::query(
                "UPDATE user_profiles SET
                    display_name=COALESCE($2, display_name),
                    locale=COALESCE($3, locale),
                    updated_at=now()
                 WHERE owner_user_id=$1"
            )
            .bind(target_user_id).bind(display_name).bind(locale)
            .execute(&mut *tx).await?;
        }

        let row = sqlx::query(
            "SELECT u.id, u.role, u.account_status,
                    p.email, p.email_verified, p.display_name, p.locale,
                    to_char(u.created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS\"Z\"') AS created_at
             FROM app_users u
             LEFT JOIN user_profiles p ON p.owner_user_id=u.id
             WHERE u.id=$1"
        )
        .bind(target_user_id).fetch_one(&mut *tx).await?;
        let user = serde_json::json!({
            "id": row.try_get::<Uuid, _>("id")?,
            "role": row.try_get::<String, _>("role")?,
            "account_status": row.try_get::<String, _>("account_status")?,
            "email": row.try_get::<Option<String>, _>("email")?,
            "email_verified": row.try_get::<bool, _>("email_verified")?,
            "display_name": row.try_get::<Option<String>, _>("display_name")?,
            "locale": row.try_get::<Option<String>, _>("locale")?,
            "created_at": row.try_get::<Option<String>, _>("created_at")?
        });
        tx.commit().await?;
        Ok(Some(user))
    }

    /// Permanently deletes the account and dependent cloud metadata via declared foreign-key cascades.
    /// The caller must pass the user ID resolved from the active verified session.
    pub async fn delete_account(&self, user_id: Uuid) -> Result<bool, StorageError> {
        let mut tx = self.begin_user_transaction(user_id).await?;
        let result = sqlx::query("DELETE FROM app_users WHERE id=$1")
            .bind(user_id).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(result.rows_affected() == 1)
    }
}
