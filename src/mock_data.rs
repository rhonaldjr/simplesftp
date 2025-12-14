#[derive(Debug, Clone)]
pub struct QueueItem {
    pub local_location: String,
    pub filename: String,
    pub remote_file: String,
    pub downloaded: String,
    pub remaining: String,
    pub priority: u8,
    pub progress: String, // "In Progress", "Pending"
}

#[derive(Debug, Clone, PartialEq)]
pub enum FileType {
    File,
    Folder,
}

#[derive(Debug, Clone)]
pub struct RemoteFile {
    pub name: String,
    pub size: String,
    pub file_type: FileType,
    pub modified: String,
}

pub fn generate_mock_queue() -> Vec<QueueItem> {
    vec![
        QueueItem {
            local_location: ".../Animations".into(),
            filename: "Lost.in.Space.2018.S02E01.720p.WEB...".into(),
            remote_file: "Lost.in.Space.2018.S02E01.720p.NF...".into(),
            downloaded: "20MB".into(),
            remaining: "80MB".into(),
            priority: 10,
            progress: "In Progress".into(),
        },
        QueueItem {
            local_location: ".../Movies".into(),
            filename: "Matrix.1999.1080p.mkv".into(),
            remote_file: "Matrix.1999.1080p.mkv".into(),
            downloaded: "0MB".into(),
            remaining: "2.5GB".into(),
            priority: 5,
            progress: "Pending".into(),
        },
    ]
}

#[allow(dead_code)]
pub fn generate_mock_remote_files() -> Vec<RemoteFile> {
    vec![
        RemoteFile {
            name: "..".into(),
            size: "".into(),
            file_type: FileType::Folder,
            modified: "".into(),
        },
        RemoteFile {
            name: "files".into(),
            size: "".into(),
            file_type: FileType::Folder,
            modified: "2023-01-01 12:00:00".into(),
        },
        RemoteFile {
            name: "Lost.in.Space.2018.S02E01.720p.NF.WEBRip...".into(),
            size: "80MB".into(),
            file_type: FileType::File,
            modified: "2023-10-27 09:30:00".into(),
        },
        RemoteFile {
            name: "config.txt".into(),
            size: "2KB".into(),
            file_type: FileType::File,
            modified: "2023-11-05 15:45:00".into(),
        },
    ]
}
