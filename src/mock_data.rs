#[derive(Debug, Clone, PartialEq)]
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

#[derive(Debug, Clone)]
pub struct QueueItem {
    pub local_location: String,
    pub filename: String,
    pub remote_file: String,
    pub size_bytes: u64,
    pub bytes_downloaded: u64,
    pub priority: u8,
    pub status: TransferStatus,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FileType {
    File,
    Folder,
}

#[derive(Debug, Clone)]
pub struct RemoteFile {
    pub name: String,
    pub path: String,
    pub size: String,
    pub size_bytes: u64,
    pub file_type: FileType,
    pub modified: String,
}

#[allow(dead_code)]
pub fn generate_mock_remote_files() -> Vec<RemoteFile> {
    vec![
        RemoteFile {
            name: "..".into(),
            path: "/mnt/movies".into(), // Mock path
            size: "".into(),
            size_bytes: 0,
            file_type: FileType::Folder,
            modified: "".into(),
        },
        RemoteFile {
            name: "files".into(),
            path: "/mnt/movies/files".into(),
            size: "".into(),
            size_bytes: 0,
            file_type: FileType::Folder,
            modified: "2023-01-01 12:00:00".into(),
        },
        RemoteFile {
            name: "Lost.in.Space.2018.S02E01.720p.NF.WEBRip...".into(),
            path: "/mnt/movies/Lost.in.Space.2018.S02E01.720p.NF.WEBRip...".into(),
            size: "80MB".into(),
            size_bytes: 83886080,
            file_type: FileType::File,
            modified: "2023-10-27 09:30:00".into(),
        },
        RemoteFile {
            name: "config.txt".into(),
            path: "/mnt/movies/config.txt".into(),
            size: "2KB".into(),
            size_bytes: 2048,
            file_type: FileType::File,
            modified: "2023-11-05 15:45:00".into(),
        },
    ]
}
