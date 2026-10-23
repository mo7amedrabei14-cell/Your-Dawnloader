//! PostgreSQL access. Callers must establish a server-verified principal before using
//! begin_user_transaction, and must not share transactions across requests.

use sqlx::{PgPool, Postgres, Transaction, postgres::PgPoolOptions};
use thiserror::Error;
use uuid::Uuid;

static MIGRATOR: sqlx::migrate::Migrator = sqlx::migrate!("./migrations");

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("database operation failed: {0}")] Database(#[from] sqlx::Error),
    #[error("database migration failed: {0}")] Migration(String),
    #[error("database URL is empty")] MissingUrl,
}

#[derive(Clone)]
pub struct Storage { pool: PgPool }

impl Storage {
    pub async fn connect(database_url: &str) -> Result<Self, StorageError> {
        if database_url.trim().is_empty() { return Err(StorageError::MissingUrl); }
        let pool = PgPoolOptions::new().max_connections(10)
            .acquire_timeout(std::time::Duration::from_secs(5))
            .connect(database_url).await?;
        Ok(Self { pool })
    }

    pub async fn migrate(&self) -> Result<(), StorageError> {
        MIGRATOR.run(&self.pool).await.map_err(|error| StorageError::Migration(error.to_string()))
    }

    /// verified_user_id must be resolved from a verified OIDC principal, never request JSON.
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
}
