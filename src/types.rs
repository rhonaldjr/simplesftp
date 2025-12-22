use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum TransferStatus {
    Pending,
    Downloading,
    Paused,
    Completed,
    Failed(String),
}

impl std::fmt::Display for TransferStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransferStatus::Pending => write!(f, "Pending"),
            TransferStatus::Downloading => write!(f, "Downloading"),
            TransferStatus::Paused => write!(f, "Paused"),
            TransferStatus::Completed => write!(f, "Completed"),
            TransferStatus::Failed(e) => write!(f, "Failed: {}", e),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueueItem {
    pub local_location: String,
    pub filename: String,
    pub remote_file: String,
    pub size_bytes: u64,
    pub bytes_downloaded: u64,
    pub priority: u8,
    pub status: TransferStatus,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FileType {
    File,
    Folder,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteFile {
    pub name: String,
    pub path: String,
    pub size: String,
    pub size_bytes: u64,
    pub file_type: FileType,
    pub modified: String,
}
