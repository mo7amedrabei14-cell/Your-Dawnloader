//! Versioned wire contracts. Owner IDs are deliberately absent from client write payloads.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const API_VERSION: &str = "v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncedMediaKind { Audio, Video, Image, Document, Unknown }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncedDownloadStatus { Completed, Failed, Cancelled }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpsertDownloadRecordRequest {
    pub client_record_id: Uuid,
    pub source_domain: String,
    pub provider_item_key: Option<String>,
    pub display_name: Option<String>,
    pub media_kind: SyncedMediaKind,
    pub container: Option<String>,
    pub quality_label: Option<String>,
    pub status: SyncedDownloadStatus,
    pub completed_at_unix_ms: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DownloadRecordResponse {
    pub id: Uuid,
    pub source_domain: String,
    pub provider_item_key: Option<String>,
    pub display_name: Option<String>,
    pub media_kind: SyncedMediaKind,
    pub container: Option<String>,
    pub quality_label: Option<String>,
    pub status: SyncedDownloadStatus,
    pub completed_at_unix_ms: Option<i64>,
    pub created_at_unix_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DownloadHistoryPage { pub items: Vec<DownloadRecordResponse>, pub next_cursor: Option<String> }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn contract_has_no_owner_id_or_device_path_input() {
        let request = UpsertDownloadRecordRequest {
            client_record_id: Uuid::nil(),
            source_domain: "media.example.test".to_owned(),
            provider_item_key: None,
            display_name: Some("Sample".to_owned()),
            media_kind: SyncedMediaKind::Audio,
            container: Some("mp3".to_owned()),
            quality_label: Some("320 kbps".to_owned()),
            status: SyncedDownloadStatus::Completed,
            completed_at_unix_ms: Some(1000),
        };
        let json = serde_json::to_value(request).unwrap();
        assert!(json.get("owner_user_id").is_none());
        assert!(json.get("local_path").is_none());
    }
}
