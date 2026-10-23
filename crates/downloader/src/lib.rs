//! Bounded direct HTTP(S) downloader. It does not resolve protected provider streams.

use std::{io, path::{Path, PathBuf}, time::Duration};
use dawn_security::validate_http_url;
use futures_util::StreamExt;
use reqwest::{Client, Response, redirect::Policy};
use thiserror::Error;
use tokio::{fs::{self, OpenOptions}, io::AsyncWriteExt, sync::mpsc};
use tokio_util::sync::CancellationToken;
use url::Url;
use uuid::Uuid;

const MAX_REDIRECTS: usize = 5;
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_FILENAME_CHARS: usize = 180;

#[derive(Debug, Clone)]
pub struct DownloadRequest { pub url: String, pub destination_dir: PathBuf, pub max_bytes: u64 }

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DownloadEvent {
    Started { file_name: String, total_bytes: Option<u64> },
    Progress { downloaded_bytes: u64, total_bytes: Option<u64> },
    Completed { path: PathBuf, downloaded_bytes: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadOutcome { pub path: PathBuf, pub downloaded_bytes: u64 }

#[derive(Debug, Error)]
pub enum DownloadError {
    #[error("URL rejected by security policy: {0}")] UrlPolicy(#[from] dawn_security::UrlPolicyError),
    #[error("download request failed: {0}")] Request(#[from] reqwest::Error),
    #[error("filesystem operation failed: {0}")] Io(#[from] io::Error),
    #[error("server returned unsuccessful status {0}")] HttpStatus(reqwest::StatusCode),
    #[error("source returned a web page rather than a direct file")] HtmlPage,
    #[error("download exceeds the configured byte limit")] TooLarge,
    #[error("download was cancelled")] Cancelled,
    #[error("the response was empty")] EmptyResponse,
    #[error("configured maximum download size must be greater than zero")] InvalidLimit,
    #[error("could not choose a unique output filename")] NameExhausted,
}

struct TempCleanup(PathBuf);
impl Drop for TempCleanup {
    fn drop(&mut self) { let _ = std::fs::remove_file(&self.0); }
}

pub async fn download_direct(
    request: DownloadRequest,
    cancellation: CancellationToken,
    events: Option<mpsc::UnboundedSender<DownloadEvent>>,
) -> Result<DownloadOutcome, DownloadError> {
    if request.max_bytes == 0 { return Err(DownloadError::InvalidLimit); }
    let parsed_url = validate_http_url(&request.url)?;
    let client = Client::builder()
        .timeout(DEFAULT_TIMEOUT)
        .redirect(Policy::custom(|attempt| {
            if attempt.previous().len() >= MAX_REDIRECTS { return attempt.error("redirect limit exceeded"); }
            if validate_http_url(attempt.url().as_str()).is_err() { return attempt.error("redirect rejected by URL policy"); }
            attempt.follow()
        }))
        .build()?;

    let response = tokio::select! {
        _ = cancellation.cancelled() => return Err(DownloadError::Cancelled),
        response = client.get(parsed_url).send() => response?,
    };
    ensure_success(&response)?;
    reject_html(&response)?;
    let total_bytes = response.content_length();
    if total_bytes.is_some_and(|size| size > request.max_bytes) { return Err(DownloadError::TooLarge); }

    fs::create_dir_all(&request.destination_dir).await?;
    let file_name = suggested_filename(response.url());
    let temporary_path = request.destination_dir.join(format!(".your-dawnloader-{}.part", Uuid::new_v4()));
    let _cleanup = TempCleanup(temporary_path.clone());
    let mut output = OpenOptions::new().write(true).create_new(true).open(&temporary_path).await?;
    emit(&events, DownloadEvent::Started { file_name: file_name.clone(), total_bytes });

    let mut stream = response.bytes_stream();
    let mut downloaded_bytes = 0_u64;
    loop {
        let next_chunk = tokio::select! {
            _ = cancellation.cancelled() => return Err(DownloadError::Cancelled),
            chunk = stream.next() => chunk,
        };
        let Some(chunk_result) = next_chunk else { break };
        let chunk = chunk_result?;
        downloaded_bytes = downloaded_bytes.checked_add(chunk.len() as u64).ok_or(DownloadError::TooLarge)?;
        if downloaded_bytes > request.max_bytes { return Err(DownloadError::TooLarge); }
        output.write_all(&chunk).await?;
        emit(&events, DownloadEvent::Progress { downloaded_bytes, total_bytes });
    }
    if downloaded_bytes == 0 { return Err(DownloadError::EmptyResponse); }
    output.flush().await?;
    drop(output);

    let path = publish_without_overwrite(&temporary_path, &request.destination_dir, &file_name).await?;
    emit(&events, DownloadEvent::Completed { path: path.clone(), downloaded_bytes });
    Ok(DownloadOutcome { path, downloaded_bytes })
}

fn ensure_success(response: &Response) -> Result<(), DownloadError> {
    if !response.status().is_success() { return Err(DownloadError::HttpStatus(response.status())); }
    Ok(())
}

fn reject_html(response: &Response) -> Result<(), DownloadError> {
    let mime = response.headers().get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok()).unwrap_or("")
        .split(';').next().unwrap_or("").trim().to_ascii_lowercase();
    if mime == "text/html" || mime == "application/xhtml+xml" { return Err(DownloadError::HtmlPage); }
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
    if name.is_empty() || name == "." || name == ".." { "download.bin".to_owned() } else { name }
}

async fn publish_without_overwrite(temporary_path: &Path, destination_dir: &Path, file_name: &str) -> Result<PathBuf, DownloadError> {
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
    if let Some(sender) = events { let _ = sender.send(event); }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn filename_is_sanitized() {
        assert_eq!(sanitize_filename("../../evil.mp4"), "_.._evil.mp4");
        assert_eq!(sanitize_filename("C:\\Users\\name.mp3"), "C__Users__name.mp3");
        assert_eq!(sanitize_filename("..."), "download.bin");
        assert!(sanitize_filename(&"a".repeat(300)).chars().count() <= MAX_FILENAME_CHARS);
    }
    #[tokio::test]
    async fn zero_limit_is_rejected_before_network() {
        let error = download_direct(
            DownloadRequest { url: "https://example.test/file.mp4".to_owned(), destination_dir: std::env::temp_dir(), max_bytes: 0 },
            CancellationToken::new(), None,
        ).await.unwrap_err();
        assert!(matches!(error, DownloadError::InvalidLimit));
    }
}
