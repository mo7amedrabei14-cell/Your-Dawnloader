//! Per-device download history. Local paths are intentionally excluded from serialized
//! records and from the account-sync contract.

use std::{
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::Serialize;
use sqlx::{
    Row, SqlitePool,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions, SqliteSynchronous},
};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum LocalStoreError {
    #[error("database connection failed: {0}")]
    Database(#[from] sqlx::Error),
    #[error("filesystem setup failed: {0}")]
    Filesystem(#[from] std::io::Error),
    #[error("file size is outside the supported range")]
    InvalidSize,
}

#[derive(Debug, Clone)]
pub struct NewLocalDownloadRecord {
    pub id: Uuid,
    pub title: String,
    pub source_domain: String,
    pub media_kind: String,
    pub format: Option<String>,
    pub file_name: String,
    pub local_path: PathBuf,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct LocalDownloadRecord {
    pub id: String,
    pub title: String,
    pub source_domain: String,
    pub media_kind: String,
    pub format: Option<String>,
    pub file_name: String,
    pub size_bytes: u64,
    pub completed_at_unix_ms: i64,
    pub file_exists: bool,
    #[serde(skip_serializing)]
    pub local_path: String,
}

#[derive(Debug, Clone)]
pub struct LocalStore {
    pool: SqlitePool,
}

impl LocalStore {
    pub async fn open(path: &Path) -> Result<Self, LocalStoreError> {
        let in_memory = path == Path::new(":memory:");
        if !in_memory {
            if let Some(parent) = path.parent() {
                if !parent.as_os_str().is_empty() {
                    std::fs::create_dir_all(parent)?;
                }
            }
        }
        let mut options = SqliteConnectOptions::new()
            .create_if_missing(true)
            .foreign_keys(true)
            .busy_timeout(Duration::from_secs(5));
        if in_memory {
            options = options.filename(":memory:");
        } else {
            options = options
                .filename(path)
                .journal_mode(SqliteJournalMode::Wal)
                .synchronous(SqliteSynchronous::Normal);
        }

        let pool = SqlitePoolOptions::new()
            .max_connections(if in_memory { 1 } else { 4 })
            .connect_with(options)
            .await?;

        sqlx::query(
            "CREATE TABLE IF NOT EXISTS download_history (
                id TEXT PRIMARY KEY NOT NULL,
                title TEXT NOT NULL CHECK(length(title) BETWEEN 1 AND 300),
                source_domain TEXT NOT NULL CHECK(length(source_domain) BETWEEN 1 AND 253),
                media_kind TEXT NOT NULL CHECK(media_kind IN ('audio','video','image','document','unknown')),
                format TEXT CHECK(format IS NULL OR length(format) <= 32),
                file_name TEXT NOT NULL CHECK(length(file_name) BETWEEN 1 AND 255),
                local_path TEXT NOT NULL,
                size_bytes INTEGER NOT NULL CHECK(size_bytes >= 0),
                completed_at_unix_ms INTEGER NOT NULL
            )",
        )
        .execute(&pool)
        .await?;
        sqlx::query("CREATE INDEX IF NOT EXISTS download_history_completed_idx ON download_history(completed_at_unix_ms DESC)")
            .execute(&pool)
            .await?;

        Ok(Self { pool })
    }

    pub async fn record_completed(&self, record: NewLocalDownloadRecord) -> Result<(), LocalStoreError> {
        let size_bytes = i64::try_from(record.size_bytes).map_err(|_| LocalStoreError::InvalidSize)?;
        let title = bounded(&record.title, 300);
        let source_domain = bounded(&record.source_domain, 253);
        let file_name = bounded(&record.file_name, 255);
        let media_kind = match record.media_kind.as_str() {
            "audio" | "video" | "image" | "document" => record.media_kind.as_str(),
            _ => "unknown",
        };
        let format = record.format.as_deref().map(|value| bounded(value, 32));
        let local_path = record.local_path.to_string_lossy().to_string();
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default().as_millis();
        let completed_at_unix_ms = now.min(i64::MAX as u128) as i64;

        sqlx::query(
            "INSERT INTO download_history
             (id, title, source_domain, media_kind, format, file_name, local_path, size_bytes, completed_at_unix_ms)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT(id) DO UPDATE SET
               title=excluded.title, source_domain=excluded.source_domain, media_kind=excluded.media_kind,
               format=excluded.format, file_name=excluded.file_name, local_path=excluded.local_path,
               size_bytes=excluded.size_bytes, completed_at_unix_ms=excluded.completed_at_unix_ms",
        )
        .bind(record.id.to_string())
        .bind(title)
        .bind(source_domain)
        .bind(media_kind)
        .bind(format)
        .bind(file_name)
        .bind(local_path)
        .bind(size_bytes)
        .bind(completed_at_unix_ms)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn list_downloads(&self, limit: u32) -> Result<Vec<LocalDownloadRecord>, LocalStoreError> {
        let limit = i64::from(limit.clamp(1, 500));
        let rows = sqlx::query(
            "SELECT id, title, source_domain, media_kind, format, file_name, local_path, size_bytes, completed_at_unix_ms
             FROM download_history ORDER BY completed_at_unix_ms DESC LIMIT ?",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;

        rows.into_iter()
            .map(|row| {
                let local_path: String = row.try_get("local_path")?;
                let path = PathBuf::from(&local_path);
                let raw_size: i64 = row.try_get("size_bytes")?;
                Ok(LocalDownloadRecord {
                    id: row.try_get("id")?,
                    title: row.try_get("title")?,
                    source_domain: row.try_get("source_domain")?,
                    media_kind: row.try_get("media_kind")?,
                    format: row.try_get("format")?,
                    file_name: row.try_get("file_name")?,
                    size_bytes: raw_size.max(0) as u64,
                    completed_at_unix_ms: row.try_get("completed_at_unix_ms")?,
                    file_exists: path.is_file(),
                    local_path,
                })
            })
            .collect::<Result<Vec<_>, sqlx::Error>>()
            .map_err(LocalStoreError::Database)
    }

    /// Removes only the history record; the media file on disk is never deleted here.
    pub async fn delete_record(&self, id: Uuid) -> Result<bool, LocalStoreError> {
        let result = sqlx::query("DELETE FROM download_history WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected() > 0)
    }

    /// Clears the history database only. Downloaded files remain untouched.
    pub async fn clear_history(&self) -> Result<u64, LocalStoreError> {
        let result = sqlx::query("DELETE FROM download_history").execute(&self.pool).await?;
        Ok(result.rows_affected())
    }
}

fn bounded(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn local_history_does_not_serialize_local_paths() {
        let store = LocalStore::open(Path::new(":memory:")).await.unwrap();
        store.record_completed(NewLocalDownloadRecord {
            id: Uuid::nil(),
            title: "Sample track".to_owned(),
            source_domain: "media.example.test".to_owned(),
            media_kind: "audio".to_owned(),
            format: Some("mp3".to_owned()),
            file_name: "sample.mp3".to_owned(),
            local_path: PathBuf::from("/private/device/sample.mp3"),
            size_bytes: 1024,
        }).await.unwrap();

        let items = store.list_downloads(50).await.unwrap();
        assert_eq!(items.len(), 1);
        assert!(!items[0].file_exists);
        let json = serde_json::to_value(&items[0]).unwrap();
        assert!(json.get("local_path").is_none());
        assert_eq!(json["title"], "Sample track");
    }

    #[tokio::test]
    async fn deleting_history_never_deletes_the_media_file() {
        let store = LocalStore::open(Path::new(":memory:")).await.unwrap();
        let record_id = Uuid::new_v4();
        store.record_completed(NewLocalDownloadRecord {
            id: record_id,
            title: "Test".to_owned(),
            source_domain: "example.test".to_owned(),
            media_kind: "unknown".to_owned(),
            format: None,
            file_name: "item.bin".to_owned(),
            local_path: PathBuf::from("/not/used/item.bin"),
            size_bytes: 1,
        }).await.unwrap();
        assert!(store.delete_record(record_id).await.unwrap());
        assert!(store.list_downloads(10).await.unwrap().is_empty());
    }
}
