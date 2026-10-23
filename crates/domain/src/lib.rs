//! Shared, UI-independent domain types and invariants.
pub mod media;
pub mod task;
pub use media::{DownloadAuthorization, MediaCandidate, MediaKind, MediaRepresentation, SourceKind};
pub use task::{DownloadTask, DownloadTaskStatus, InvalidTransition};
