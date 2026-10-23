use serde::{Deserialize, Serialize};
use url::Url;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaKind { Audio, Video, Image, Document, Unknown }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind { DirectFile, HlsManifest, DashManifest, ProviderPage, BrowserCandidate, Unknown }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DownloadAuthorization { Unknown, UserConfirmed, ProviderAuthorized, NotAvailable, Protected }

/// A discovery record is not proof that a provider grants download rights.
/// Do not log source_url: its query can contain temporary signed credentials.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MediaCandidate {
    pub id: Uuid,
    pub source_url: Url,
    pub source_kind: SourceKind,
    pub media_kind: MediaKind,
    pub title: Option<String>,
    pub source_domain: String,
    pub content_type: Option<String>,
    pub size_bytes: Option<u64>,
    pub duration_seconds: Option<u64>,
    pub authorization: DownloadAuthorization,
    pub representations: Vec<MediaRepresentation>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MediaRepresentation {
    pub id: Uuid,
    pub label: String,
    pub container: Option<String>,
    pub codec: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub audio_bitrate_kbps: Option<u32>,
    pub size_bytes: Option<u64>,
    pub source_url: Option<Url>,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn candidate_does_not_claim_download_rights_by_default() {
        let candidate = MediaCandidate {
            id: Uuid::nil(),
            source_url: Url::parse("https://media.example.test/song.mp3").unwrap(),
            source_kind: SourceKind::DirectFile,
            media_kind: MediaKind::Audio,
            title: None,
            source_domain: "media.example.test".to_owned(),
            content_type: Some("audio/mpeg".to_owned()),
            size_bytes: None,
            duration_seconds: None,
            authorization: DownloadAuthorization::Unknown,
            representations: vec![],
        };
        assert_eq!(candidate.authorization, DownloadAuthorization::Unknown);
    }
}
