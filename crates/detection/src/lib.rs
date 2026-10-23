//! Conservative media classification; identification is not authorization.

use dawn_domain::{MediaKind, SourceKind};
use url::Url;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DetectionResult { pub source_kind: SourceKind, pub media_kind: MediaKind }

pub fn classify(url: &Url, content_type: Option<&str>) -> DetectionResult {
    let mime = content_type.unwrap_or("").split(';').next().unwrap_or("").trim().to_ascii_lowercase();
    let path = url.path().to_ascii_lowercase();
    let extension = path.rsplit('/').next().unwrap_or("").rsplit('.').next().unwrap_or("");

    let source_kind = match (extension, mime.as_str()) {
        ("m3u8", _) | (_, "application/vnd.apple.mpegurl") | (_, "application/x-mpegurl") => SourceKind::HlsManifest,
        ("mpd", _) | (_, "application/dash+xml") => SourceKind::DashManifest,
        ("mp3" | "m4a" | "aac" | "wav" | "ogg" | "opus" | "flac" | "mp4" | "m4v" | "mov" | "webm" | "mkv" | "avi", _) => SourceKind::DirectFile,
        (_, m) if m.starts_with("audio/") || m.starts_with("video/") => SourceKind::DirectFile,
        _ => SourceKind::Unknown,
    };
    let media_kind = match extension {
        "mp3" | "m4a" | "aac" | "wav" | "ogg" | "opus" | "flac" => MediaKind::Audio,
        "mp4" | "m4v" | "mov" | "webm" | "mkv" | "avi" | "m3u8" | "mpd" => MediaKind::Video,
        "png" | "jpg" | "jpeg" | "webp" | "gif" | "avif" => MediaKind::Image,
        _ if mime.starts_with("audio/") => MediaKind::Audio,
        _ if mime.starts_with("video/") => MediaKind::Video,
        _ if mime.starts_with("image/") => MediaKind::Image,
        _ => MediaKind::Unknown,
    };
    DetectionResult { source_kind, media_kind }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn parse(value: &str) -> Url { Url::parse(value).unwrap() }
    #[test]
    fn identifies_direct_audio() {
        let result = classify(&parse("https://cdn.example.test/song.mp3"), None);
        assert_eq!(result.source_kind, SourceKind::DirectFile);
        assert_eq!(result.media_kind, MediaKind::Audio);
    }
    #[test]
    fn identifies_manifests_without_claiming_they_are_files() {
        assert_eq!(classify(&parse("https://cdn.example.test/live.m3u8"), None).source_kind, SourceKind::HlsManifest);
        assert_eq!(classify(&parse("https://cdn.example.test/video.mpd"), None).source_kind, SourceKind::DashManifest);
    }
    #[test]
    fn does_not_infer_from_a_page_id() {
        assert_eq!(classify(&parse("https://play.example.test/song/12345"), None).source_kind, SourceKind::Unknown);
    }
}
