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

    pub async fn create_login_state(&self, state_hash: &str, nonce: &str, pkce_verifier: &str) -> Result<(), StorageError> {
        sqlx::query("INSERT INTO oidc_login_states(state_hash, nonce, pkce_verifier) VALUES ($1,$2,$3)")
            .bind(state_hash).bind(nonce).bind(pkce_verifier)
            .execute(&self.pool).await?;
        Ok(())
    }

    /// Atomic, single-use consumption avoids OAuth callback replay.
    pub async fn consume_login_state(&self, state_hash: &str) -> Result<Option<LoginState>, StorageError> {
        sqlx::query("DELETE FROM oidc_login_states WHERE expires_at <= now()").execute(&self.pool).await?;
        let row = sqlx::query(
            "DELETE FROM oidc_login_states WHERE state_hash=$1 AND expires_at > now()
             RETURNING nonce, pkce_verifier"
        )
        .bind(state_hash).fetch_optional(&self.pool).await?;
        if let Some(row) = row {
            Ok(Some(LoginState {
                nonce: row.try_get::<String, _>("nonce")?,
                pkce_verifier: row.try_get::<String, _>("pkce_verifier")?,
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
    ) -> Result<Uuid, StorageError> {
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
        .bind(user_id).bind(display_name).bind(email).bind(email_verified)
        .execute(&mut *tx).await?;

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
                    COALESCE(pref.sync_download_history, FALSE) AS sync_download_history
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
}
