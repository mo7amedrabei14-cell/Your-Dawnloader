use std::{env, net::SocketAddr};
use axum::{extract::State, http::StatusCode, routing::get, Json, Router};
use dawn_storage::Storage;
use serde_json::{json, Value};
use tracing::info;

async fn live() -> (StatusCode, Json<Value>) {
    (StatusCode::OK, Json(json!({"status": "live", "service": "dawn-api"})))
}

async fn ready(State(storage): State<Storage>) -> (StatusCode, Json<Value>) {
    match storage.ping().await {
        Ok(()) => (StatusCode::OK, Json(json!({"status": "ready"}))),
        Err(_) => (StatusCode::SERVICE_UNAVAILABLE, Json(json!({"status": "not_ready"}))),
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(env::var("RUST_LOG").unwrap_or_else(|_| "info,dawn_api=debug".to_owned()))
        .init();

    let database_url = env::var("DATABASE_URL")
        .map_err(|_| anyhow::anyhow!("DATABASE_URL must be configured; do not commit it"))?;
    let storage = Storage::connect(&database_url).await?;
    storage.migrate().await?;

    let address: SocketAddr = env::var("API_BIND_ADDRESS")
        .unwrap_or_else(|_| "127.0.0.1:8080".to_owned())
        .parse()?;
    let app = Router::new()
        .route("/health/live", get(live))
        .route("/health/ready", get(ready))
        .with_state(storage);

    let listener = tokio::net::TcpListener::bind(address).await?;
    info!(%address, "Your Dawnloader API health endpoints listening");
    axum::serve(listener, app).await?;
    Ok(())
}
