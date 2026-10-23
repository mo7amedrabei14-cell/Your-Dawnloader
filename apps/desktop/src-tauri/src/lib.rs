use std::{
    collections::HashMap,
    env,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use dawn_api_contract::{SyncedDownloadStatus, SyncedMediaKind, UpsertDownloadRecordRequest};
use dawn_detection::classify;
use dawn_downloader::{DownloadError, DownloadEvent, DownloadRequest, download_direct};
use dawn_local_storage::{LocalDownloadRecord, LocalStore, NewLocalDownloadRecord};
use dawn_security::validate_http_url;
use keyring::Entry;
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::{Semaphore, mpsc, OnceCell};
use tokio_util::sync::CancellationToken;
use url::Url;
use uuid::Uuid;

const MAX_DOWNLOAD_BYTES: u64 = 10 * 1024 * 1024 * 1024;
const MAX_CONCURRENT_DOWNLOADS: usize = 3;
const KEYRING_SERVICE: &str = "com.yourdawnloader.app";
const KEYRING_USERNAME: &str = "approved-device-code";
const DEFAULT_API_BASE: &str = "http://127.0.0.1:8080";
const API_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone)]
struct PendingDeviceLink {
    device_code: String,
    expires_at: Instant,
    interval_seconds: u64,
}

#[derive(Clone)]
struct AppState {
    database_path: PathBuf,
    local_store: Arc<OnceCell<LocalStore>>,
    tasks: Arc<Mutex<HashMap<String, CancellationToken>>>,
    pending_device_links: Arc<Mutex<HashMap<String, PendingDeviceLink>>>,
    download_slots: Arc<Semaphore>,
    api_base_url: String,
    api_client: Client,
}

impl AppState {
    fn new(database_path: PathBuf) -> Result<Self, String> {
        let raw_api_base = env::var("YOUR_DAWNLOADER_API_BASE_URL")
            .ok()
            .or_else(|| option_env!("YOUR_DAWNLOADER_API_BASE_URL").map(str::to_owned))
            .unwrap_or_else(|| DEFAULT_API_BASE.to_owned());
        let parsed_api_base = Url::parse(raw_api_base.trim())
            .map_err(|_| "The account API URL is invalid.".to_owned())?;
        let is_local_http = parsed_api_base.scheme() == "http"
            && matches!(parsed_api_base.host_str(), Some("localhost" | "127.0.0.1" | "::1"));
        if parsed_api_base.username() != ""
            || parsed_api_base.password().is_some()
            || parsed_api_base.host_str().is_none()
            || !(parsed_api_base.scheme() == "https" || is_local_http) {
            return Err("The account API must use HTTPS (HTTP is permitted only for local development).".to_owned());
        }
        let api_base_url = parsed_api_base.as_str().trim_end_matches('/').to_owned();
        let api_client = Client::builder()
            .timeout(API_REQUEST_TIMEOUT)
            .redirect(reqwest::redirect::Policy::none())
            .user_agent("YourDawnloader-Desktop/0.1")
            .build()
            .map_err(|_| "Could not initialize the account network client.".to_owned())?;

        Ok(Self {
            database_path,
            local_store: Arc::new(OnceCell::new()),
            tasks: Arc::new(Mutex::new(HashMap::new())),
            pending_device_links: Arc::new(Mutex::new(HashMap::new())),
            download_slots: Arc::new(Semaphore::new(MAX_CONCURRENT_DOWNLOADS)),
            api_base_url,
            api_client,
        })
    }

    async fn store(&self) -> Result<LocalStore, String> {
        self.local_store
            .get_or_try_init(|| async { LocalStore::open(&self.database_path).await })
            .await
            .cloned()
            .map_err(|error| error.to_string())
    }

    fn emit(&self, app: &AppHandle, progress: DownloadProgress) {
        let _ = app.emit("download-progress", progress);
    }

    fn remove_task(&self, task_id: &str) {
        if let Ok(mut tasks) = self.tasks.lock() {
            tasks.remove(task_id);
        }
    }

    fn token_entry() -> Result<Entry, String> {
        Entry::new(KEYRING_SERVICE, KEYRING_USERNAME)
            .map_err(|_| "The operating-system credential store is unavailable.".to_owned())
    }

    fn device_token(&self) -> Result<Option<String>, String> {
        match Self::token_entry()?.get_password() {
            Ok(token) if !token.is_empty() => Ok(Some(token)),
            Ok(_) => Ok(None),
            Err(keyring::Error::NoEntry) => Ok(None),
            Err(_) => Err("Could not read the device credential from the operating-system credential store.".to_owned()),
        }
    }

    fn save_device_token(token: &str) -> Result<(), String> {
        Self::token_entry()?.set_password(token)
            .map_err(|_| "Could not save the device credential in the operating-system credential store.".to_owned())
    }

    fn clear_device_token() -> Result<(), String> {
        match Self::token_entry()?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err("Could not remove the device credential from the operating-system credential store.".to_owned()),
        }
    }

    async fn sync_completed_download(
        &self,
        record_id: Uuid,
        source_domain: &str,
        display_name: &str,
        media_kind: &str,
        container: Option<&str>,
        completed_at_unix_ms: i64,
    ) -> Result<(), String> {
        // Unlinked devices remain local-only. Once linked, persist metadata before attempting the network.
        if self.device_token()?.is_none() { return Ok(()) }
        self.store().await?
            .enqueue_sync_outbox(dawn_local_storage::NewSyncOutboxRecord {
                id: record_id,
                source_domain: source_domain.to_owned(),
                display_name: display_name.to_owned(),
                media_kind: media_kind.to_owned(),
                container: container.map(str::to_owned),
                completed_at_unix_ms,
            })
            .await
            .map_err(|_| "Download saved locally, but its sync record could not be queued for retry.".to_owned())?;
        self.sync_pending_downloads().await
    }

    async fn sync_pending_downloads(&self) -> Result<(), String> {
        let Some(token) = self.device_token()? else { return Ok(()) };
        let store = self.store().await?;
        let queued = store.list_sync_outbox(100).await
            .map_err(|_| "Could not read the local history-sync queue.".to_owned())?;
        if queued.is_empty() { return Ok(()) }

        let me_response = self.api_client
            .get(format!("{}/api/v1/device/me", self.api_base_url))
            .bearer_auth(&token)
            .send().await
            .map_err(|_| "Download saved locally; cloud history is queued and will retry when online.".to_owned())?;
        if matches!(me_response.status(), StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN | StatusCode::GONE) {
            let _ = Self::clear_device_token();
            let _ = store.clear_sync_outbox().await;
            return Err("Download saved locally. The account connection expired or was revoked; reconnect to sync future history.".to_owned());
        }
        if !me_response.status().is_success() {
            return Err("Download saved locally; cloud history is queued and will retry after the account service recovers.".to_owned());
        }
        let account: DeviceMeResponse = me_response.json().await
            .map_err(|_| "Download saved locally; cloud history remains queued until a valid account response is available.".to_owned())?;
        if !account.user.sync_download_history {
            store.clear_sync_outbox().await
                .map_err(|_| "History sync is off; could not clear the local retry queue.".to_owned())?;
            return Ok(());
        }

        for queued_record in queued {
            let id = match Uuid::parse_str(&queued_record.id) {
                Ok(id) => id,
                Err(_) => {
                    let _ = store.remove_sync_outbox_record(&queued_record.id).await;
                    continue;
                }
            };
            let media_kind = match queued_record.media_kind.as_str() {
                "audio" => SyncedMediaKind::Audio,
                "video" => SyncedMediaKind::Video,
                "image" => SyncedMediaKind::Image,
                "document" => SyncedMediaKind::Document,
                _ => SyncedMediaKind::Unknown,
            };
            let request = UpsertDownloadRecordRequest {
                client_record_id: id,
                source_domain: queued_record.source_domain.clone(),
                provider_item_key: None,
                display_name: Some(queued_record.display_name.clone()),
                media_kind,
                container: queued_record.container.clone(),
                quality_label: None,
                status: SyncedDownloadStatus::Completed,
                completed_at_unix_ms: Some(queued_record.completed_at_unix_ms),
            };
            let response = match self.api_client
                .post(format!("{}/api/v1/device/downloads", self.api_base_url))
                .bearer_auth(&token)
                .json(&request)
                .send().await {
                    Ok(response) => response,
                    Err(_) => {
                        let _ = store.increment_sync_outbox_attempt(&queued_record.id).await;
                        return Err("Download saved locally; one or more history records remain queued for retry.".to_owned());
                    }
                };
            if matches!(response.status(), StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN | StatusCode::GONE) {
                let _ = Self::clear_device_token();
                let _ = store.clear_sync_outbox().await;
                return Err("Download saved locally. Device authorization was revoked; reconnect before syncing future history.".to_owned());
            }
            if !response.status().is_success() {
                let _ = store.increment_sync_outbox_attempt(&queued_record.id).await;
                return Err("Download saved locally; one or more history records remain queued for retry.".to_owned());
            }
            store.remove_sync_outbox_record(&queued_record.id).await
                .map_err(|_| "The history record synced, but its local retry marker could not be removed.".to_owned())?;
        }
        Ok(())
    }
}

#[derive(Debug, Serialize)]
struct InspectionResult {
    host: String,
    source_kind: String,
    media_kind: String,
    note: String,
}

#[derive(Debug, Clone, Serialize)]
struct DownloadProgress {
    task_id: String,
    status: String,
    file_name: Option<String>,
    downloaded_bytes: u64,
    total_bytes: Option<u64>,
    message: Option<String>,
}

#[derive(Debug, Serialize)]
struct StartDownloadResponse {
    task_id: String,
}

#[derive(Debug, Serialize)]
struct StartAccountLinkResponse {
    link_id: String,
    user_code: String,
    verification_url: String,
    expires_in_seconds: u64,
    interval_seconds: u64,
}

#[derive(Debug, Deserialize)]
struct DeviceLinkStartWire {
    device_code: String,
    user_code: String,
    verification_url: String,
    expires_in_seconds: u64,
    interval_seconds: u64,
}

#[derive(Debug, Deserialize)]
struct DeviceLinkPollWire {
    status: String,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    sync_download_history: Option<bool>,
    #[serde(default)]
    device_label: Option<String>,
}

#[derive(Debug, Serialize)]
struct DeviceLinkPollResponse {
    status: String,
    display_name: Option<String>,
    email: Option<String>,
    sync_download_history: Option<bool>,
    device_label: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DeviceMeResponse {
    user: DeviceMeUser,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
struct DeviceMeUser {
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    email: Option<String>,
    sync_download_history: bool,
}

#[derive(Debug, Serialize, Clone)]
struct LinkedAccountResponse {
    connected: bool,
    display_name: Option<String>,
    email: Option<String>,
    sync_download_history: bool,
}

fn validate_verification_url(raw: &str, expected_user_code: &str) -> Result<String, String> {
    let url = Url::parse(raw).map_err(|_| "The account service returned an invalid verification URL.".to_owned())?;
    let local_http = url.scheme() == "http"
        && matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "::1"));
    if url.username() != ""
        || url.password().is_some()
        || url.host_str().is_none()
        || !(url.scheme() == "https" || local_http)
        || url.path() != "/account/connect"
        || !url.query_pairs().any(|(name, value)| name == "user_code" && value == expected_user_code) {
        return Err("The account service returned an unsafe verification URL.".to_owned());
    }
    Ok(url.to_string())
}

#[tauri::command]
async fn start_account_link(state: State<'_, AppState>) -> Result<StartAccountLinkResponse, String> {
    if state.device_token()?.is_some() {
        return Err("This device is already connected. Disconnect it before linking another account.".to_owned());
    }
    let response = state.api_client
        .post(format!("{}/api/v1/device-auth/start", state.api_base_url))
        .json(&serde_json::json!({
            "device_label": "Your Dawnloader desktop",
            "platform": env::consts::OS
        }))
        .send().await
        .map_err(|_| "Could not reach the account service to start device linking.".to_owned())?;
    if !response.status().is_success() {
        return Err("The account service refused to start device linking.".to_owned());
    }
    let wire: DeviceLinkStartWire = response.json().await
        .map_err(|_| "The account service returned an invalid device-link response.".to_owned())?;
    if wire.device_code.len() < 32 || wire.device_code.len() > 128
        || !wire.device_code.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
        return Err("The account service returned an invalid device credential.".to_owned());
    }
    let verification_url = validate_verification_url(&wire.verification_url, &wire.user_code)?;
    let link_id = Uuid::new_v4().to_string();
    let pending = PendingDeviceLink {
        device_code: wire.device_code,
        expires_at: Instant::now() + Duration::from_secs(wire.expires_in_seconds.min(600)),
        interval_seconds: wire.interval_seconds.clamp(2, 15),
    };
    state.pending_device_links.lock()
        .map_err(|_| "Device-link manager is unavailable.".to_owned())?
        .insert(link_id.clone(), pending);
    Ok(StartAccountLinkResponse {
        link_id,
        user_code: wire.user_code,
        verification_url,
        expires_in_seconds: wire.expires_in_seconds.min(600),
        interval_seconds: wire.interval_seconds.clamp(2, 15),
    })
}

#[tauri::command]
async fn poll_account_link(
    state: State<'_, AppState>,
    link_id: String,
) -> Result<DeviceLinkPollResponse, String> {
    let pending = state.pending_device_links.lock()
        .map_err(|_| "Device-link manager is unavailable.".to_owned())?
        .get(&link_id).cloned()
        .ok_or_else(|| "This device-link session no longer exists. Start a new one.".to_owned())?;
    if Instant::now() >= pending.expires_at {
        if let Ok(mut links) = state.pending_device_links.lock() { links.remove(&link_id); }
        return Ok(DeviceLinkPollResponse {
            status: "expired".to_owned(), display_name: None, email: None,
            sync_download_history: None, device_label: None,
        });
    }

    let response = state.api_client
        .post(format!("{}/api/v1/device-auth/poll", state.api_base_url))
        .json(&serde_json::json!({"device_code": pending.device_code}))
        .send().await
        .map_err(|_| "Could not check device approval. Check the network and retry.".to_owned())?;
    if response.status() == StatusCode::ACCEPTED {
        return Ok(DeviceLinkPollResponse {
            status: "pending".to_owned(), display_name: None, email: None,
            sync_download_history: None, device_label: None,
        });
    }
    if matches!(response.status(), StatusCode::GONE | StatusCode::NOT_FOUND) {
        if let Ok(mut links) = state.pending_device_links.lock() { links.remove(&link_id); }
        return Ok(DeviceLinkPollResponse {
            status: "expired".to_owned(), display_name: None, email: None,
            sync_download_history: None, device_label: None,
        });
    }
    if !response.status().is_success() {
        return Err("The account service could not check device approval.".to_owned());
    }
    let wire: DeviceLinkPollWire = response.json().await
        .map_err(|_| "The account service returned an invalid approval response.".to_owned())?;
    if wire.status != "authorized" {
        return Err("The account service returned an unknown device-link status.".to_owned());
    }

    // Drop any pending entries from a previous account before associating this local queue
    // with the newly approved device. The active bearer credential never leaves Rust.
    let store = state.store().await?;
    store.clear_sync_outbox().await
        .map_err(|_| "Could not reset the pending history queue for this account.".to_owned())?;
    AppState::save_device_token(&pending.device_code)?;

    // If backfill fails, preserve the pending in-memory link so a later poll can retry it.
    // Never report the pairing complete while silently skipping opted-in local history.
    if wire.sync_download_history.unwrap_or(false) {
        store.queue_all_history_for_sync().await
            .map_err(|_| "The device is connected, but existing local history could not be queued for sync. Retrying…".to_owned())?;
        let _ = state.sync_pending_downloads().await;
    }
    if let Ok(mut links) = state.pending_device_links.lock() { links.remove(&link_id); }
    Ok(DeviceLinkPollResponse {
        status: "authorized".to_owned(),
        display_name: wire.display_name,
        email: wire.email,
        sync_download_history: wire.sync_download_history,
        device_label: wire.device_label,
    })
}

#[tauri::command]
fn cancel_account_link(state: State<'_, AppState>, link_id: String) -> Result<bool, String> {
    Ok(state.pending_device_links.lock()
        .map_err(|_| "Device-link manager is unavailable.".to_owned())?
        .remove(&link_id).is_some())
}

async fn fetch_linked_account(state: &AppState) -> Result<Option<LinkedAccountResponse>, String> {
    let Some(token) = state.device_token()? else {
        // Never carry one account's pending history into a later device pairing.
        let _ = state.store().await?.clear_sync_outbox().await;
        return Ok(None);
    };
    let response = state.api_client
        .get(format!("{}/api/v1/device/me", state.api_base_url))
        .bearer_auth(&token)
        .send().await
        .map_err(|_| "Could not verify the connected account. Check the network and try again.".to_owned())?;
    if matches!(response.status(), StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN | StatusCode::GONE) {
        let _ = AppState::clear_device_token();
        let _ = state.store().await?.clear_sync_outbox().await;
        return Ok(None);
    }
    if !response.status().is_success() {
        return Err("The account service could not verify this device.".to_owned());
    }
    let payload: DeviceMeResponse = response.json().await
        .map_err(|_| "The account service returned an unreadable account response.".to_owned())?;
    Ok(Some(LinkedAccountResponse {
        connected: true,
        display_name: payload.user.display_name,
        email: payload.user.email,
        sync_download_history: payload.user.sync_download_history,
    }))
}

#[tauri::command]
async fn get_account_status(state: State<'_, AppState>) -> Result<Option<LinkedAccountResponse>, String> {
    fetch_linked_account(state.inner()).await
}

#[tauri::command]
async fn set_account_sync(
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<LinkedAccountResponse, String> {
    let token = state.device_token()?
        .ok_or_else(|| "Connect this device to an account first.".to_owned())?;
    let response = state.api_client
        .put(format!("{}/api/v1/device/me/preferences", state.api_base_url))
        .bearer_auth(&token)
        .json(&serde_json::json!({"sync_download_history": enabled}))
        .send().await
        .map_err(|_| "Could not update account preferences. Check the network and try again.".to_owned())?;
    if matches!(response.status(), StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN | StatusCode::GONE) {
        let _ = AppState::clear_device_token();
        return Err("The device connection has expired or been revoked. Connect it again.".to_owned());
    }
    if !response.status().is_success() {
        return Err("The account service rejected the preference change.".to_owned());
    }
    let store = state.store().await?;
    if enabled {
        // Explicit opt-in also includes older local records, not only new downloads.
        store.queue_all_history_for_sync().await
            .map_err(|_| "History sync is enabled, but existing local records could not be queued.".to_owned())?;
        // Keep queued rows for retry if temporarily offline; the one-minute worker will retry.
        let _ = state.sync_pending_downloads().await;
    } else {
        store.clear_sync_outbox().await
            .map_err(|_| "Cloud history sync is off, but the local retry queue could not be cleared.".to_owned())?;
    }
    fetch_linked_account(state.inner()).await?
        .ok_or_else(|| "Device was disconnected while updating preferences.".to_owned())
}

#[tauri::command]
async fn disconnect_account(state: State<'_, AppState>) -> Result<bool, String> {
    let Some(token) = state.device_token()? else {
        state.store().await?.clear_sync_outbox().await
            .map_err(|_| "The local history-sync queue could not be cleared.".to_owned())?;
        return Ok(false);
    };
    let response = state.api_client
        .post(format!("{}/api/v1/device/disconnect", state.api_base_url))
        .bearer_auth(&token)
        .send().await
        .map_err(|_| "Could not disconnect this device from the account service. Try again while online.".to_owned())?;
    if !(response.status().is_success()
        || matches!(response.status(), StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN | StatusCode::GONE | StatusCode::NOT_FOUND)) {
        return Err("The account service could not disconnect this device.".to_owned());
    }
    AppState::clear_device_token()?;
    state.store().await?.clear_sync_outbox().await
        .map_err(|_| "Device disconnected, but the local retry queue could not be cleared.".to_owned())?;
    Ok(true)
}

#[tauri::command]
fn inspect_url(url: String) -> Result<InspectionResult, String> {
    let parsed = validate_http_url(url.trim()).map_err(|error| error.to_string())?;
    let result = classify(&parsed, None);
    let (source_kind, note) = match result.source_kind {
        dawn_domain::SourceKind::DirectFile => (
            "direct_file".to_owned(),
            "A direct-file candidate was detected. The server response must still be checked.".to_owned(),
        ),
        dawn_domain::SourceKind::HlsManifest => (
            "hls_manifest".to_owned(),
            "An HLS manifest was detected. Support depends on format, authorization, and protection.".to_owned(),
        ),
        dawn_domain::SourceKind::DashManifest => (
            "dash_manifest".to_owned(),
            "A DASH manifest was detected. Support depends on format, authorization, and protection.".to_owned(),
        ),
        _ => (
            "unknown".to_owned(),
            "This URL is not identifiable as a direct media file from its path alone.".to_owned(),
        ),
    };
    Ok(InspectionResult {
        host: parsed.host_str().unwrap_or("unknown").to_owned(),
        source_kind,
        media_kind: format!("{:?}", result.media_kind).to_ascii_lowercase(),
        note,
    })
}

#[tauri::command]
async fn start_download(
    app: AppHandle,
    state: State<'_, AppState>,
    url: String,
) -> Result<StartDownloadResponse, String> {
    let parsed_url = validate_http_url(url.trim()).map_err(|error| error.to_string())?;
    let source_domain = parsed_url.host_str().unwrap_or("unknown").to_owned();
    let url_string = parsed_url.to_string();
    let task_id = Uuid::new_v4().to_string();
    let cancellation = CancellationToken::new();

    {
        let mut tasks = state.tasks.lock().map_err(|_| "Download manager is unavailable")?;
        tasks.insert(task_id.clone(), cancellation.clone());
    }

    let destination_dir = match app.path().download_dir() {
        Ok(path) => path,
        Err(_) => app.path().app_data_dir().map_err(|error| error.to_string())?,
    };
    let worker_state = state.inner().clone();
    worker_state.emit(&app, DownloadProgress {
        task_id: task_id.clone(),
        status: "queued".to_owned(),
        file_name: None,
        downloaded_bytes: 0,
        total_bytes: None,
        message: Some("Waiting for a download slot.".to_owned()),
    });

    let task_id_for_worker = task_id.clone();
    tauri::async_runtime::spawn(async move {
        let task_id = task_id_for_worker;
        let result: Result<dawn_downloader::DownloadOutcome, DownloadError> = async {
            let _permit = tokio::select! {
                _ = cancellation.cancelled() => return Err(DownloadError::Cancelled),
                permit = worker_state.download_slots.clone().acquire_owned() => {
                    permit.map_err(|_| DownloadError::Cancelled)?
                }
            };

            let (sender, mut receiver) = mpsc::unbounded_channel();
            let progress_app = app.clone();
            let progress_task_id = task_id.clone();
            let progress_worker = tauri::async_runtime::spawn(async move {
                while let Some(event) = receiver.recv().await {
                    let progress = match event {
                        DownloadEvent::Started { file_name, total_bytes } => Some(DownloadProgress {
                            task_id: progress_task_id.clone(),
                            status: "downloading".to_owned(),
                            file_name: Some(file_name),
                            downloaded_bytes: 0,
                            total_bytes,
                            message: None,
                        }),
                        DownloadEvent::Progress { downloaded_bytes, total_bytes } => Some(DownloadProgress {
                            task_id: progress_task_id.clone(),
                            status: "downloading".to_owned(),
                            file_name: None,
                            downloaded_bytes,
                            total_bytes,
                            message: None,
                        }),
                        DownloadEvent::Completed { .. } => None,
                    };
                    if let Some(progress) = progress {
                        let _ = progress_app.emit("download-progress", progress);
                    }
                }
            });

            let download_result = download_direct(
                DownloadRequest {
                    url: url_string,
                    destination_dir,
                    max_bytes: MAX_DOWNLOAD_BYTES,
                },
                cancellation.clone(),
                Some(sender.clone()),
            ).await;
            drop(sender);
            let _ = progress_worker.await;
            download_result
        }.await;

        match result {
            Ok(outcome) => {
                let file_name = outcome.path.file_name()
                    .and_then(|value| value.to_str())
                    .unwrap_or("downloaded-media")
                    .to_owned();
                let format = outcome.path.extension()
                    .and_then(|value| value.to_str())
                    .map(|value| value.to_ascii_lowercase());
                let media_kind = media_kind_for_path(&outcome.path);
                let record_id = Uuid::parse_str(&task_id).unwrap_or_else(|_| Uuid::new_v4());
                let format_for_sync = format.clone();
                let record = NewLocalDownloadRecord {
                    id: record_id,
                    title: file_name.clone(),
                    source_domain: source_domain.clone(),
                    media_kind: media_kind.to_owned(),
                    format,
                    file_name: file_name.clone(),
                    local_path: outcome.path,
                    size_bytes: outcome.downloaded_bytes,
                };

                let persistence_error = match worker_state.store().await {
                    Ok(store) => store.record_completed(record).await.err().map(|error| error.to_string()),
                    Err(error) => Some(error),
                };
                let mut completion_message = persistence_error.as_ref()
                    .map(|_| "File downloaded, but local history could not be saved.".to_owned());
                if completion_message.is_none() {
                    let completed_at = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis()
                        .min(i64::MAX as u128) as i64;
                    if let Err(sync_error) = worker_state.sync_completed_download(
                        record_id,
                        &source_domain,
                        &file_name,
                        media_kind,
                        format_for_sync.as_deref(),
                        completed_at,
                    ).await {
                        completion_message = Some(sync_error);
                    }
                }

                worker_state.emit(&app, DownloadProgress {
                    task_id: task_id.clone(),
                    status: "completed".to_owned(),
                    file_name: Some(file_name),
                    downloaded_bytes: outcome.downloaded_bytes,
                    total_bytes: Some(outcome.downloaded_bytes),
                    message: completion_message,
                });
            }
            Err(DownloadError::Cancelled) => {
                worker_state.emit(&app, DownloadProgress {
                    task_id: task_id.clone(),
                    status: "cancelled".to_owned(),
                    file_name: None,
                    downloaded_bytes: 0,
                    total_bytes: None,
                    message: Some("Download cancelled.".to_owned()),
                });
            }
            Err(error) => {
                worker_state.emit(&app, DownloadProgress {
                    task_id: task_id.clone(),
                    status: "failed".to_owned(),
                    file_name: None,
                    downloaded_bytes: 0,
                    total_bytes: None,
                    message: Some(error.to_string()),
                });
            }
        }
        worker_state.remove_task(&task_id);
    });

    Ok(StartDownloadResponse { task_id })
}

#[tauri::command]
fn cancel_download(state: State<'_, AppState>, task_id: String) -> Result<bool, String> {
    let token = state.tasks.lock()
        .map_err(|_| "Download manager is unavailable")?
        .get(&task_id)
        .cloned();
    if let Some(token) = token {
        token.cancel();
        Ok(true)
    } else {
        Ok(false)
    }
}

#[tauri::command]
async fn list_downloads(state: State<'_, AppState>) -> Result<Vec<LocalDownloadRecord>, String> {
    state.store().await?.list_downloads(100).await.map_err(|error| error.to_string())
}

#[tauri::command]
async fn delete_download_record(state: State<'_, AppState>, id: String) -> Result<bool, String> {
    let id = Uuid::parse_str(&id).map_err(|_| "Invalid download record ID".to_owned())?;
    state.store().await?.delete_record(id).await.map_err(|error| error.to_string())
}

#[tauri::command]
async fn clear_download_history(state: State<'_, AppState>) -> Result<u64, String> {
    state.store().await?.clear_history().await.map_err(|error| error.to_string())
}

fn media_kind_for_path(path: &PathBuf) -> &'static str {
    match path.extension().and_then(|value| value.to_str()).unwrap_or("").to_ascii_lowercase().as_str() {
        "mp3" | "m4a" | "aac" | "wav" | "ogg" | "opus" | "flac" => "audio",
        "mp4" | "m4v" | "mov" | "webm" | "mkv" | "avi" => "video",
        "png" | "jpg" | "jpeg" | "webp" | "gif" | "avif" => "image",
        "pdf" | "txt" | "zip" => "document",
        _ => "unknown",
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&app_data_dir)?;
            let state = AppState::new(app_data_dir.join("download-history.sqlite3"))
                .map_err(std::io::Error::other)?;
            app.manage(state.clone());
            tauri::async_runtime::spawn(async move {
                let mut ticker = tokio::time::interval(Duration::from_secs(60));
                loop {
                    ticker.tick().await;
                    let _ = state.sync_pending_downloads().await;
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            inspect_url,
            start_download,
            start_account_link,
            poll_account_link,
            cancel_account_link,
            get_account_status,
            set_account_sync,
            disconnect_account,
            cancel_download,
            list_downloads,
            delete_download_record,
            clear_download_history
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Your Dawnloader desktop application");
}
