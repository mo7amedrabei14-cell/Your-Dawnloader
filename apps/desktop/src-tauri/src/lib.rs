use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use dawn_detection::classify;
use dawn_downloader::{DownloadError, DownloadEvent, DownloadRequest, download_direct};
use dawn_local_storage::{LocalDownloadRecord, LocalStore, NewLocalDownloadRecord};
use dawn_security::validate_http_url;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tokio::sync::{Semaphore, mpsc, OnceCell};
use tokio_util::sync::CancellationToken;
use url::Url;
use uuid::Uuid;

const MAX_DOWNLOAD_BYTES: u64 = 10 * 1024 * 1024 * 1024;
const MAX_CONCURRENT_DOWNLOADS: usize = 3;

#[derive(Clone)]
struct AppState {
    database_path: PathBuf,
    local_store: Arc<OnceCell<LocalStore>>,
    tasks: Arc<Mutex<HashMap<String, CancellationToken>>>,
    download_slots: Arc<Semaphore>,
}

impl AppState {
    fn new(database_path: PathBuf) -> Self {
        Self {
            database_path,
            local_store: Arc::new(OnceCell::new()),
            tasks: Arc::new(Mutex::new(HashMap::new())),
            download_slots: Arc::new(Semaphore::new(MAX_CONCURRENT_DOWNLOADS)),
        }
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
                let record = NewLocalDownloadRecord {
                    id: Uuid::parse_str(&task_id).unwrap_or_else(|_| Uuid::new_v4()),
                    title: file_name.clone(),
                    source_domain,
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

                worker_state.emit(&app, DownloadProgress {
                    task_id: task_id.clone(),
                    status: "completed".to_owned(),
                    file_name: Some(file_name),
                    downloaded_bytes: outcome.downloaded_bytes,
                    total_bytes: Some(outcome.downloaded_bytes),
                    message: persistence_error.map(|_| "File downloaded, but local history could not be saved.".to_owned()),
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
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&app_data_dir)?;
            app.manage(AppState::new(app_data_dir.join("download-history.sqlite3")));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            inspect_url,
            start_download,
            cancel_download,
            list_downloads,
            delete_download_record,
            clear_download_history
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Your Dawnloader desktop application");
}
