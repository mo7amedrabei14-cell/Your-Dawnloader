//! Bounded direct HTTP(S) downloader with validator-checked resume.
//! It does not resolve protected provider streams. Resume metadata contains hashes,
//! validators and a safe filename; it never stores the original (possibly signed) URL.

use std::{
    collections::HashSet,
    io,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::Duration,
};

use dawn_security::validate_http_url;
use futures_util::StreamExt;
use reqwest::{
    header::{CONTENT_RANGE, ETAG, IF_RANGE, LAST_MODIFIED, RANGE},
    Client, Response, StatusCode,
    redirect::Policy,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use tokio::{
    fs::{self, File, OpenOptions},
    io::AsyncWriteExt,
    sync::mpsc,
};
use tokio_util::sync::CancellationToken;
use url::Url;
use uuid::Uuid;

const MAX_REDIRECTS: usize = 5;
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_FILENAME_CHARS: usize = 180;
const RESUME_VERSION: u8 = 1;

static ACTIVE_DOWNLOADS: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();

#[derive(Debug, Clone)]
pub struct DownloadRequest {
    pub url: String,
    pub destination_dir: PathBuf,
    pub max_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DownloadEvent {
    Started { file_name: String, total_bytes: Option<u64> },
    Progress { downloaded_bytes: u64, total_bytes: Option<u64> },
    Completed { path: PathBuf, downloaded_bytes: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadOutcome {
    pub path: PathBuf,
    pub downloaded_bytes: u64,
}

#[derive(Debug, Error)]
pub enum DownloadError {
    #[error("URL rejected by security policy: {0}")]
    UrlPolicy(#[from] dawn_security::UrlPolicyError),
    #[error("download request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("filesystem operation failed: {0}")]
    Io(#[from] io::Error),
    #[error("server returned unsuccessful status {0}")]
    HttpStatus(StatusCode),
    #[error("source returned a web page rather than a direct file")]
    HtmlPage,
    #[error("download exceeds the configured byte limit")]
    TooLarge,
    #[error("download was cancelled")]
    Cancelled,
    #[error("the response was empty")]
    EmptyResponse,
    #[error("configured maximum download size must be greater than zero")]
    InvalidLimit,
    #[error("could not choose a unique output filename")]
    NameExhausted,
    #[error("a download from this URL is already active in the destination folder")]
    AlreadyRunning,
    #[error("the server returned an invalid partial response; it was not appended")]
    InvalidPartialResponse,
    #[error("the response ended before its declared byte length")]
    TruncatedResponse,
    #[error("download manager synchronization is unavailable")]
    ManagerUnavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ResumeMetadata {
    version: u8,
    source_fingerprint: String,
    response_fingerprint: String,
    file_name: String,
    validator_kind: String,
    validator: String,
    total_bytes: Option<u64>,
}

struct ResumeFiles {
    partial: PathBuf,
    metadata: PathBuf,
}

struct ResumeState {
    metadata: ResumeMetadata,
    offset: u64,
}

struct ActiveDownloadGuard {
    key: String,
}

impl ActiveDownloadGuard {
    fn acquire(key: String) -> Result<Self, DownloadError> {
        let active = ACTIVE_DOWNLOADS.get_or_init(|| Mutex::new(HashSet::new()));
        let mut active = active.lock().map_err(|_| DownloadError::ManagerUnavailable)?;
        if !active.insert(key.clone()) {
            return Err(DownloadError::AlreadyRunning);
        }
        Ok(Self { key })
    }
}

impl Drop for ActiveDownloadGuard {
    fn drop(&mut self) {
        if let Some(active) = ACTIVE_DOWNLOADS.get() {
            if let Ok(mut active) = active.lock() {
                active.remove(&self.key);
            }
        }
    }
}

fn fingerprint(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    let mut output = String::with_capacity(64);
    for byte in digest {
        output.push_str(&format!("{byte:02x}"));
    }
    output
}

fn resume_files(destination_dir: &Path, source_fingerprint: &str) -> ResumeFiles {
    let short_hash = &source_fingerprint[..32];
    ResumeFiles {
        partial: destination_dir.join(format!(".your-dawnloader-{short_hash}.part")),
        metadata: destination_dir.join(format!(".your-dawnloader-{short_hash}.resume.json")),
    }
}

async fn remove_if_exists(path: &Path) -> Result<(), DownloadError> {
    match fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(DownloadError::Io(error)),
    }
}

async fn discard_resume_files(files: &ResumeFiles) -> Result<(), DownloadError> {
    remove_if_exists(&files.metadata).await?;
    remove_if_exists(&files.partial).await?;
    Ok(())
}

fn validator_from_headers(headers: &reqwest::header::HeaderMap) -> Option<(String, String)> {
    if let Some(value) = headers.get(ETAG).and_then(|value| value.to_str().ok()) {
        // Weak ETags are not suitable for byte-for-byte If-Range validation.
        if !value.trim().is_empty()
            && !value.starts_with("W/")
            && value.len() <= 512
            && !value.chars().any(char::is_control)
        {
            return Some(("etag".to_owned(), value.to_owned()));
        }
    }
    if let Some(value) = headers.get(LAST_MODIFIED).and_then(|value| value.to_str().ok()) {
        if !value.trim().is_empty()
            && value.len() <= 128
            && !value.chars().any(char::is_control)
        {
            return Some(("last-modified".to_owned(), value.to_owned()));
        }
    }
    None
}

fn matches_validator(headers: &reqwest::header::HeaderMap, metadata: &ResumeMetadata) -> bool {
    let actual = match metadata.validator_kind.as_str() {
        "etag" => headers.get(ETAG).and_then(|value| value.to_str().ok()),
        "last-modified" => headers.get(LAST_MODIFIED).and_then(|value| value.to_str().ok()),
        _ => None,
    };
    actual == Some(metadata.validator.as_str())
}

fn parse_content_range(value: &str) -> Option<(u64, u64, u64)> {
    let value = value.strip_prefix("bytes ")?;
    let (range, total) = value.split_once('/')?;
    if total == "*" {
        return None;
    }
    let total = total.parse::<u64>().ok()?;
    let (start, end) = range.split_once('-')?;
    let start = start.parse::<u64>().ok()?;
    let end = end.parse::<u64>().ok()?;
    if end < start || total == 0 || end >= total {
        return None;
    }
    Some((start, end, total))
}

fn valid_partial_response(
    response: &Response,
    saved: &ResumeMetadata,
    offset: u64,
    max_bytes: u64,
) -> Option<(u64, u64)> {
    if response.status() != StatusCode::PARTIAL_CONTENT
        || fingerprint(response.url().as_str()) != saved.response_fingerprint
        || !matches_validator(response.headers(), saved)
    {
        return None;
    }

    let header = response.headers().get(CONTENT_RANGE)?.to_str().ok()?;
    let (start, end, total) = parse_content_range(header)?;
    if start != offset || total > max_bytes || saved.total_bytes.is_some_and(|saved_total| saved_total != total) {
        return None;
    }

    let response_bytes = end.checked_sub(start)?.checked_add(1)?;
    if response.content_length().is_some_and(|length| length != response_bytes) {
        return None;
    }
    Some((total, response_bytes))
}

async fn load_resume_state(
    files: &ResumeFiles,
    expected_source_fingerprint: &str,
    max_bytes: u64,
) -> Result<Option<ResumeState>, DownloadError> {
    let partial_metadata = match fs::symlink_metadata(&files.partial).await {
        Ok(metadata) => Some(metadata),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(DownloadError::Io(error)),
    };
    let sidecar_metadata = match fs::symlink_metadata(&files.metadata).await {
        Ok(metadata) => Some(metadata),
        Err(error) if error.kind() == io::ErrorKind::NotFound => None,
        Err(error) => return Err(DownloadError::Io(error)),
    };

    let (Some(partial_metadata), Some(sidecar_metadata)) = (partial_metadata, sidecar_metadata) else {
        if partial_metadata.is_some() || sidecar_metadata.is_some() {
            discard_resume_files(files).await?;
        }
        return Ok(None);
    };

    if !partial_metadata.file_type().is_file()
        || partial_metadata.file_type().is_symlink()
        || !sidecar_metadata.file_type().is_file()
        || sidecar_metadata.file_type().is_symlink()
    {
        discard_resume_files(files).await?;
        return Ok(None);
    }

    let offset = partial_metadata.len();
    if offset == 0 || offset > max_bytes {
        discard_resume_files(files).await?;
        return Ok(None);
    }

    let bytes = match fs::read(&files.metadata).await {
        Ok(bytes) => bytes,
        Err(error) => {
            discard_resume_files(files).await?;
            if error.kind() == io::ErrorKind::NotFound {
                return Ok(None);
            }
            return Err(DownloadError::Io(error));
        }
    };
    let metadata: ResumeMetadata = match serde_json::from_slice(&bytes) {
        Ok(metadata) => metadata,
        Err(_) => {
            discard_resume_files(files).await?;
            return Ok(None);
        }
    };

    let safe_validator = !metadata.validator.trim().is_empty()
        && metadata.validator.len() <= 512
        && !metadata.validator.chars().any(char::is_control)
        && match metadata.validator_kind.as_str() {
            "etag" => !metadata.validator.starts_with("W/"),
            "last-modified" => metadata.validator.len() <= 128,
            _ => false,
        };
    let valid = metadata.version == RESUME_VERSION
        && metadata.source_fingerprint == expected_source_fingerprint
        && metadata.response_fingerprint.len() == 64
        && metadata.response_fingerprint.bytes().all(|byte| byte.is_ascii_hexdigit())
        && metadata.file_name == sanitize_filename(&metadata.file_name)
        && safe_validator
        && metadata.total_bytes.is_none_or(|total| total > offset && total <= max_bytes)
        && offset <= metadata.total_bytes.unwrap_or(max_bytes);

    if !valid {
        discard_resume_files(files).await?;
        return Ok(None);
    }

    Ok(Some(ResumeState { metadata, offset }))
}

async fn persist_resume_metadata(path: &Path, metadata: &ResumeMetadata) -> Result<(), DownloadError> {
    let bytes = serde_json::to_vec(metadata)
        .map_err(|error| DownloadError::Io(io::Error::new(io::ErrorKind::InvalidData, error)))?;
    let temporary = path.with_extension(format!("resume-{}.tmp", Uuid::new_v4()));
    let write_result = async {
        let mut file = OpenOptions::new().write(true).create_new(true).open(&temporary).await?;
        file.write_all(&bytes).await?;
        file.sync_all().await?;
        drop(file);
        fs::rename(&temporary, path).await?;
        Ok::<(), io::Error>(())
    }.await;
    if let Err(error) = write_result {
        let _ = fs::remove_file(&temporary).await;
        return Err(DownloadError::Io(error));
    }
    Ok(())
}

async fn send_get(
    client: &Client,
    url: &Url,
    resume: Option<&ResumeMetadata>,
    offset: u64,
    cancellation: &CancellationToken,
) -> Result<Response, DownloadError> {
    let mut request = client.get(url.clone());
    if let Some(metadata) = resume {
        request = request
            .header(RANGE, format!("bytes={offset}-"))
            .header(IF_RANGE, metadata.validator.as_str());
    }
    tokio::select! {
        _ = cancellation.cancelled() => Err(DownloadError::Cancelled),
        response = request.send() => Ok(response?),
    }
}

pub async fn download_direct(
    request: DownloadRequest,
    cancellation: CancellationToken,
    events: Option<mpsc::UnboundedSender<DownloadEvent>>,
) -> Result<DownloadOutcome, DownloadError> {
    if request.max_bytes == 0 {
        return Err(DownloadError::InvalidLimit);
    }

    let parsed_url = validate_http_url(request.url.trim())?;
    fs::create_dir_all(&request.destination_dir).await?;
    let destination_dir = fs::canonicalize(&request.destination_dir).await?;
    let normalized_url = parsed_url.as_str().to_owned();
    let source_fingerprint = fingerprint(&normalized_url);
    let files = resume_files(&destination_dir, &source_fingerprint);
    let active_key = fingerprint(&format!("{}:{}", destination_dir.display(), source_fingerprint));
    let _active = ActiveDownloadGuard::acquire(active_key)?;

    let client = Client::builder()
        .timeout(DEFAULT_TIMEOUT)
        .redirect(Policy::custom(|attempt| {
            if attempt.previous().len() >= MAX_REDIRECTS {
                return attempt.error("redirect limit exceeded");
            }
            if validate_http_url(attempt.url().as_str()).is_err() {
                return attempt.error("redirect rejected by URL policy");
            }
            attempt.follow()
        }))
        .build()?;

    let mut saved_state = load_resume_state(&files, &source_fingerprint, request.max_bytes).await?;
    let mut response: Response;
    let mut resumed_offset = 0_u64;
    let mut resume_metadata: Option<ResumeMetadata> = None;
    let mut total_bytes: Option<u64> = None;
    let mut expected_response_bytes: Option<u64> = None;
    let mut file_name: Option<String> = None;
    let mut is_resuming = false;

    // Retry the same URL with Range only when we have a real, validator-backed partial.
    // The first un-ranged response also ensures that a server which changes redirects
    // or validators cannot cause unrelated bytes to be appended.
    if let Some(state) = saved_state.take() {
        response = send_get(&client, &parsed_url, Some(&state.metadata), state.offset, &cancellation).await?;
        match response.status() {
            StatusCode::PARTIAL_CONTENT => {
                if let Some((total, response_bytes)) = valid_partial_response(
                    &response,
                    &state.metadata,
                    state.offset,
                    request.max_bytes,
                ) {
                    resumed_offset = state.offset;
                    total_bytes = Some(total);
                    expected_response_bytes = Some(response_bytes);
                    file_name = Some(state.metadata.file_name.clone());
                    resume_metadata = Some(state.metadata);
                    is_resuming = true;
                } else {
                    discard_resume_files(&files).await?;
                    response = send_get(&client, &parsed_url, None, 0, &cancellation).await?;
                }
            }
            StatusCode::OK => {
                // A server may ignore Range or the object may have changed. Never append a 200 body.
                discard_resume_files(&files).await?;
            }
            StatusCode::RANGE_NOT_SATISFIABLE => {
                discard_resume_files(&files).await?;
                response = send_get(&client, &parsed_url, None, 0, &cancellation).await?;
            }
            status if !status.is_success() => return Err(DownloadError::HttpStatus(status)),
            _ => {
                discard_resume_files(&files).await?;
                response = send_get(&client, &parsed_url, None, 0, &cancellation).await?;
            }
        }
    } else {
        response = send_get(&client, &parsed_url, None, 0, &cancellation).await?;
    }

    if !is_resuming {
        ensure_success(&response)?;
        if response.status() == StatusCode::PARTIAL_CONTENT {
            return Err(DownloadError::InvalidPartialResponse);
        }
        reject_html(&response)?;
        total_bytes = response.content_length();
        if total_bytes.is_some_and(|size| size > request.max_bytes) {
            discard_resume_files(&files).await?;
            return Err(DownloadError::TooLarge);
        }
        expected_response_bytes = total_bytes;
        file_name = Some(suggested_filename(response.url()));
        if let Some((validator_kind, validator)) = validator_from_headers(response.headers()) {
            resume_metadata = Some(ResumeMetadata {
                version: RESUME_VERSION,
                source_fingerprint: source_fingerprint.clone(),
                response_fingerprint: fingerprint(response.url().as_str()),
                file_name: file_name.clone().unwrap_or_else(|| "download.bin".to_owned()),
                validator_kind,
                validator,
                total_bytes,
            });
        }
    } else {
        ensure_success(&response)?;
        reject_html(&response)?;
    }

    let file_name = file_name.unwrap_or_else(|| suggested_filename(response.url()));
    let mut output: File = if is_resuming {
        // Re-check immediately before opening. The deterministic paths are internal filenames,
        // not values taken from the source URL or response.
        let existing = fs::symlink_metadata(&files.partial).await?;
        if !existing.file_type().is_file() || existing.file_type().is_symlink() || existing.len() != resumed_offset {
            discard_resume_files(&files).await?;
            return Err(DownloadError::InvalidPartialResponse);
        }
        OpenOptions::new().append(true).open(&files.partial).await?
    } else {
        OpenOptions::new().write(true).create_new(true).open(&files.partial).await?
    };

    if !is_resuming {
        if let Some(metadata) = resume_metadata.as_ref() {
            if let Err(error) = persist_resume_metadata(&files.metadata, metadata).await {
                drop(output);
                let _ = discard_resume_files(&files).await;
                return Err(error);
            }
        }
    }

    emit(&events, DownloadEvent::Started {
        file_name: file_name.clone(),
        total_bytes,
    });
    if resumed_offset > 0 {
        emit(&events, DownloadEvent::Progress {
            downloaded_bytes: resumed_offset,
            total_bytes,
        });
    }

    let mut stream = response.bytes_stream();
    let mut response_bytes = 0_u64;
    let mut failure: Option<DownloadError> = None;

    loop {
        let next_chunk = tokio::select! {
            _ = cancellation.cancelled() => {
                failure = Some(DownloadError::Cancelled);
                break;
            }
            chunk = stream.next() => chunk,
        };
        let Some(chunk_result) = next_chunk else { break };
        let chunk = match chunk_result {
            Ok(chunk) => chunk,
            Err(error) => {
                failure = Some(DownloadError::Request(error));
                break;
            }
        };

        let Some(next_response_bytes) = response_bytes.checked_add(chunk.len() as u64) else {
            failure = Some(DownloadError::TooLarge);
            break;
        };
        let Some(next_total) = resumed_offset.checked_add(next_response_bytes) else {
            failure = Some(DownloadError::TooLarge);
            break;
        };
        if next_total > request.max_bytes
            || expected_response_bytes.is_some_and(|expected| next_response_bytes > expected)
        {
            failure = Some(DownloadError::TooLarge);
            break;
        }
        if let Err(error) = output.write_all(&chunk).await {
            failure = Some(DownloadError::Io(error));
            break;
        }
        response_bytes = next_response_bytes;
        emit(&events, DownloadEvent::Progress {
            downloaded_bytes: resumed_offset + response_bytes,
            total_bytes,
        });
    }

    if failure.is_none() && response_bytes == 0 && resumed_offset == 0 {
        failure = Some(DownloadError::EmptyResponse);
    }
    if failure.is_none() && expected_response_bytes.is_some_and(|expected| response_bytes != expected) {
        failure = Some(DownloadError::TruncatedResponse);
    }
    let cumulative_bytes = resumed_offset.saturating_add(response_bytes);
    if failure.is_none() && total_bytes.is_some_and(|total| cumulative_bytes != total) {
        failure = Some(DownloadError::TruncatedResponse);
    }

    if let Some(error) = failure {
        let keep_partial = if resume_metadata.is_some() && !matches!(error, DownloadError::TooLarge) {
            output.flush().await.is_ok()
                && output.sync_all().await.is_ok()
                && fs::symlink_metadata(&files.partial).await
                    .map(|metadata| metadata.file_type().is_file()
                        && !metadata.file_type().is_symlink()
                        && metadata.len() > 0
                        && metadata.len() <= request.max_bytes)
                    .unwrap_or(false)
        } else {
            false
        };
        drop(output);
        if !keep_partial {
            let _ = discard_resume_files(&files).await;
        }
        return Err(error);
    }

    if let Err(error) = output.flush().await {
        drop(output);
        if resume_metadata.is_none() {
            let _ = discard_resume_files(&files).await;
        }
        return Err(DownloadError::Io(error));
    }
    if let Err(error) = output.sync_all().await {
        drop(output);
        if resume_metadata.is_none() {
            let _ = discard_resume_files(&files).await;
        }
        return Err(DownloadError::Io(error));
    }
    drop(output);

    let path = match publish_without_overwrite(&files.partial, &destination_dir, &file_name).await {
        Ok(path) => path,
        Err(error) => {
            let _ = discard_resume_files(&files).await;
            return Err(error);
        }
    };
    let _ = remove_if_exists(&files.metadata).await;
    let _ = remove_if_exists(&files.partial).await;
    emit(&events, DownloadEvent::Completed {
        path: path.clone(),
        downloaded_bytes: cumulative_bytes,
    });
    Ok(DownloadOutcome { path, downloaded_bytes: cumulative_bytes })
}

fn ensure_success(response: &Response) -> Result<(), DownloadError> {
    if !response.status().is_success() {
        return Err(DownloadError::HttpStatus(response.status()));
    }
    Ok(())
}

fn reject_html(response: &Response) -> Result<(), DownloadError> {
    let mime = response.headers().get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok()).unwrap_or("")
        .split(';').next().unwrap_or("").trim().to_ascii_lowercase();
    if mime == "text/html" || mime == "application/xhtml+xml" {
        return Err(DownloadError::HtmlPage);
    }
    Ok(())
}

fn suggested_filename(url: &Url) -> String {
    let raw = url.path_segments()
        .and_then(|mut segments| segments.rfind(|segment| !segment.is_empty()))
        .unwrap_or("download.bin");
    sanitize_filename(raw)
}

pub fn sanitize_filename(raw: &str) -> String {
    let mut name: String = raw.chars().take(MAX_FILENAME_CHARS)
        .map(|ch| if ch.is_control() || r#"/\:*?"<>|"#.contains(ch) { '_' } else { ch })
        .collect();
    name = name.trim().trim_matches('.').to_owned();
    if name.is_empty() || name == "." || name == ".." {
        "download.bin".to_owned()
    } else {
        name
    }
}

async fn publish_without_overwrite(
    temporary_path: &Path,
    destination_dir: &Path,
    file_name: &str,
) -> Result<PathBuf, DownloadError> {
    let raw = Path::new(file_name);
    let stem = raw.file_stem().and_then(|value| value.to_str()).unwrap_or("download");
    let extension = raw.extension().and_then(|value| value.to_str());
    for index in 0..=999_u16 {
        let candidate_name = if index == 0 {
            file_name.to_owned()
        } else if let Some(extension) = extension {
            format!("{stem} ({index}).{extension}")
        } else {
            format!("{stem} ({index})")
        };
        let candidate = destination_dir.join(candidate_name);
        match fs::hard_link(temporary_path, &candidate).await {
            Ok(()) => return Ok(candidate),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(DownloadError::Io(error)),
        }
    }
    Err(DownloadError::NameExhausted)
}

fn emit(events: &Option<mpsc::UnboundedSender<DownloadEvent>>, event: DownloadEvent) {
    if let Some(sender) = events {
        let _ = sender.send(event);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::{TcpListener, TcpStream},
    };

    fn temporary_directory(label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("your-dawnloader-{label}-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    async fn read_http_request(stream: &mut TcpStream) -> String {
        let mut bytes = Vec::new();
        let mut chunk = [0_u8; 1024];
        loop {
            let count = stream.read(&mut chunk).await.unwrap();
            if count == 0 {
                break;
            }
            bytes.extend_from_slice(&chunk[..count]);
            if bytes.windows(4).any(|window| window == b"\r\n\r\n") {
                break;
            }
        }
        String::from_utf8_lossy(&bytes).into_owned()
    }

    async fn start_resume_fixture(ignore_range: bool) -> (String, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let base_url = format!("http://{address}/sample.bin?token=do-not-store-this-secret");
        let handle = tokio::spawn(async move {
            let (mut first, _) = listener.accept().await.unwrap();
            let first_request = read_http_request(&mut first).await;
            assert!(first_request.starts_with("GET /sample.bin?token=do-not-store-this-secret HTTP/1.1"));
            assert!(!first_request.to_ascii_lowercase().contains("range:"));
            let prefix = b"hello";
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: 10\r\nETag: \"v1\"\r\nContent-Type: application/octet-stream\r\nConnection: close\r\n\r\n"
            );
            first.write_all(head.as_bytes()).await.unwrap();
            first.write_all(prefix).await.unwrap();
            first.flush().await.unwrap();
            drop(first);

            let (mut second, _) = listener.accept().await.unwrap();
            let second_request = read_http_request(&mut second).await;
            let range_value = second_request.lines()
                .find(|line| line.to_ascii_lowercase().starts_with("range:"))
                .and_then(|line| line.split_once(':').map(|(_, value)| value.trim().to_owned()));
            let if_range = second_request.lines()
                .find(|line| line.to_ascii_lowercase().starts_with("if-range:"))
                .and_then(|line| line.split_once(':').map(|(_, value)| value.trim().to_owned()));

            if ignore_range {
                let body = b"helloworld";
                let head = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nETag: \"v1\"\r\nContent-Type: application/octet-stream\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                second.write_all(head.as_bytes()).await.unwrap();
                second.write_all(body).await.unwrap();
            } else if let (Some(range), Some(if_range)) = (range_value, if_range) {
                let offset = range.strip_prefix("bytes=")
                    .and_then(|value| value.strip_suffix('-'))
                    .and_then(|value| value.parse::<usize>().ok())
                    .unwrap_or(0);
                if offset > 0 && if_range == "\"v1\"" && offset < 10 {
                    let body = &b"helloworld"[offset..];
                    let head = format!(
                        "HTTP/1.1 206 Partial Content\r\nContent-Length: {}\r\nContent-Range: bytes {}-9/10\r\nETag: \"v1\"\r\nContent-Type: application/octet-stream\r\nConnection: close\r\n\r\n",
                        body.len(), offset
                    );
                    second.write_all(head.as_bytes()).await.unwrap();
                    second.write_all(body).await.unwrap();
                } else {
                    let body = b"bad";
                    let head = format!(
                        "HTTP/1.1 500 Internal Server Error\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        body.len()
                    );
                    second.write_all(head.as_bytes()).await.unwrap();
                    second.write_all(body).await.unwrap();
                }
            } else {
                let body = b"missing-range";
                let head = format!(
                    "HTTP/1.1 500 Internal Server Error\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                second.write_all(head.as_bytes()).await.unwrap();
                second.write_all(body).await.unwrap();
            }
        });
        (base_url, handle)
    }

    #[test]
    fn filename_is_sanitized() {
        assert_eq!(sanitize_filename("../../evil.mp4"), "_.._evil.mp4");
        assert_eq!(sanitize_filename("C:\\Users\\name.mp3"), "C__Users_name.mp3");
        assert_eq!(sanitize_filename("..."), "download.bin");
        assert!(sanitize_filename(&"a".repeat(300)).chars().count() <= MAX_FILENAME_CHARS);
    }

    #[tokio::test]
    async fn zero_limit_is_rejected_before_network() {
        let error = download_direct(
            DownloadRequest {
                url: "https://example.test/file.mp4".to_owned(),
                destination_dir: std::env::temp_dir(),
                max_bytes: 0,
            },
            CancellationToken::new(),
            None,
        ).await.unwrap_err();
        assert!(matches!(error, DownloadError::InvalidLimit));
    }

    #[tokio::test]
    async fn truncation_resumes_only_with_matching_validator_and_range() {
        let directory = temporary_directory("resume");
        let (url, server) = start_resume_fixture(false).await;
        let request = || DownloadRequest {
            url: url.clone(),
            destination_dir: directory.clone(),
            max_bytes: 1024,
        };

        assert!(download_direct(request(), CancellationToken::new(), None).await.is_err());

        let source_fingerprint = fingerprint(&validate_http_url(&url).unwrap().to_string());
        let files = resume_files(&directory, &source_fingerprint);
        let sidecar = fs::read_to_string(&files.metadata).await.unwrap();
        assert!(!sidecar.contains("do-not-store-this-secret"));
        assert!(fs::metadata(&files.partial).await.unwrap().len() > 0);

        let result = download_direct(request(), CancellationToken::new(), None).await.unwrap();
        assert_eq!(result.downloaded_bytes, 10);
        assert_eq!(fs::read(&result.path).await.unwrap(), b"helloworld");
        server.await.unwrap();
        assert!(!files.partial.exists());
        assert!(!files.metadata.exists());
        let _ = fs::remove_dir_all(directory).await;
    }

    #[tokio::test]
    async fn server_that_ignores_range_restarts_from_zero_without_duplicate_bytes() {
        let directory = temporary_directory("range-ignored");
        let (url, server) = start_resume_fixture(true).await;
        let request = || DownloadRequest {
            url: url.clone(),
            destination_dir: directory.clone(),
            max_bytes: 1024,
        };

        assert!(download_direct(request(), CancellationToken::new(), None).await.is_err());
        let result = download_direct(request(), CancellationToken::new(), None).await.unwrap();
        assert_eq!(result.downloaded_bytes, 10);
        assert_eq!(fs::read(&result.path).await.unwrap(), b"helloworld");
        server.await.unwrap();
        let _ = fs::remove_dir_all(directory).await;
    }
}
