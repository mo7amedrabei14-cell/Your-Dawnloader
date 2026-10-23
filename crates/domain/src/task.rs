use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DownloadTaskStatus { Queued, Probing, Downloading, Paused, Completed, Failed, Cancelled }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DownloadTask {
    pub id: Uuid,
    pub status: DownloadTaskStatus,
    pub bytes_downloaded: u64,
    pub total_bytes: Option<u64>,
    pub last_error_code: Option<String>,
}

impl DownloadTask {
    pub fn new() -> Self {
        Self { id: Uuid::new_v4(), status: DownloadTaskStatus::Queued, bytes_downloaded: 0, total_bytes: None, last_error_code: None }
    }

    pub fn transition(&mut self, next: DownloadTaskStatus) -> Result<(), InvalidTransition> {
        use DownloadTaskStatus as S;
        let allowed = matches!(
            (self.status, next),
            (S::Queued, S::Probing | S::Downloading | S::Cancelled | S::Failed)
                | (S::Probing, S::Queued | S::Downloading | S::Cancelled | S::Failed)
                | (S::Downloading, S::Paused | S::Completed | S::Cancelled | S::Failed)
                | (S::Paused, S::Downloading | S::Cancelled | S::Failed)
                | (S::Failed, S::Queued | S::Cancelled)
        );
        if !allowed { return Err(InvalidTransition { from: self.status, to: next }); }
        self.status = next;
        Ok(())
    }
}
impl Default for DownloadTask { fn default() -> Self { Self::new() } }

#[derive(Debug, Error, PartialEq, Eq)]
#[error("invalid download task transition: {from:?} -> {to:?}")]
pub struct InvalidTransition { pub from: DownloadTaskStatus, pub to: DownloadTaskStatus }

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn valid_download_lifecycle_is_supported() {
        let mut task = DownloadTask::new();
        task.transition(DownloadTaskStatus::Probing).unwrap();
        task.transition(DownloadTaskStatus::Downloading).unwrap();
        task.transition(DownloadTaskStatus::Paused).unwrap();
        task.transition(DownloadTaskStatus::Downloading).unwrap();
        task.transition(DownloadTaskStatus::Completed).unwrap();
        assert_eq!(task.status, DownloadTaskStatus::Completed);
    }
    #[test]
    fn completed_task_is_terminal() {
        let mut task = DownloadTask::new();
        task.transition(DownloadTaskStatus::Downloading).unwrap();
        task.transition(DownloadTaskStatus::Completed).unwrap();
        assert!(task.transition(DownloadTaskStatus::Downloading).is_err());
    }
}
