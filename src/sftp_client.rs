use crate::settings::SftpConfig;
use crate::types::{FileType, RemoteFile};

const KB: u64 = 1024;
const MB: u64 = KB * 1024;
const GB: u64 = MB * 1024;
const TB: u64 = GB * 1024;

fn format_size(size: u64) -> String {
    if size >= TB {
        format!("{:.2} TB", size as f64 / TB as f64)
    } else if size >= GB {
        format!("{:.2} GB", size as f64 / GB as f64)
    } else if size >= MB {
        format!("{:.2} MB", size as f64 / MB as f64)
    } else if size >= KB {
        format!("{:.2} KB", size as f64 / KB as f64)
    } else {
        format!("{} B", size)
    }
}

use ssh2::{Session, Sftp};
use std::fmt;
use std::net::TcpStream;
use std::path::Path;

pub struct SftpClient {
    _session: Session, // Keep session alive
    sftp: Sftp,
}

impl fmt::Debug for SftpClient {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SftpClient")
    }
}

impl SftpClient {
    pub fn connect(config: &SftpConfig) -> Result<Self, String> {
        let tcp = TcpStream::connect(format!("{}:{}", config.host, config.port))
            .map_err(|e| format!("Failed to connect to host: {}", e))?;

        let mut session = Session::new().map_err(|e| format!("Session error: {}", e))?;
        session.set_tcp_stream(tcp);
        session
            .handshake()
            .map_err(|e| format!("Handshake failed: {}", e))?;

        if let Some(password) = &config.password {
            session
                .userauth_password(&config.username, password)
                .map_err(|e| format!("Authentication failed: {}", e))?;
        } else {
            // TODO: Key auth support later
            return Err("Password required for now".into());
        }

        if !session.authenticated() {
            return Err("Authentication failed".into());
        }

        let sftp = session.sftp().map_err(|e| format!("SFTP error: {}", e))?;

        Ok(Self {
            _session: session,
            sftp,
        })
    }

    pub fn get_file_size(&self, path: &str) -> Result<u64, String> {
        let canonical_path = self
            .sftp
            .realpath(Path::new(path))
            .map_err(|e| format!("Canonicalization failed: {}", e))?;

        let stat = self
            .sftp
            .stat(&canonical_path)
            .map_err(|e| format!("Stat failed: {}", e))?;

        Ok(stat.size.unwrap_or(0))
    }

    pub fn list_dir(&self, path: &Path) -> Result<(String, Vec<RemoteFile>), String> {
        println!("DEBUG: Listing directory: {:?}", path);

        let canonical_path = self
            .sftp
            .realpath(path)
            .map_err(|e| format!("Canonicalization failed: {}", e))?;

        let path_str = canonical_path.to_str().unwrap_or(".").to_string();
        println!("DEBUG: Resolved to: {}", path_str);

        match self.sftp.readdir(&canonical_path) {
            Ok(files) => {
                let mut remote_files = Vec::new();
                for (path_buf, stat) in files {
                    let filename = path_buf.file_name().unwrap().to_str().unwrap().to_string();
                    if filename == "." {
                        continue;
                    }

                    let raw_size = stat.size.unwrap_or(0);
                    let size = if stat.is_dir() {
                        "".to_string()
                    } else {
                        format_size(raw_size)
                    };
                    let file_type = if stat.is_dir() {
                        FileType::Folder
                    } else {
                        FileType::File
                    };

                    let modified = if let Some(mtime) = stat.mtime {
                        if let Some(dt) = chrono::DateTime::from_timestamp(mtime as i64, 0) {
                            dt.format("%Y-%m-%d %H:%M:%S").to_string()
                        } else {
                            "".to_string()
                        }
                    } else {
                        "".to_string()
                    };

                    let full_path = canonical_path.join(&filename);
                    let full_path_str = full_path.to_string_lossy().to_string();

                    remote_files.push(RemoteFile {
                        name: filename,
                        path: full_path_str,
                        size,
                        size_bytes: raw_size,
                        file_type,
                        modified,
                    });
                }

                remote_files.sort_by(|a, b| {
                    if a.file_type == b.file_type {
                        a.name.cmp(&b.name)
                    } else {
                        if a.file_type == FileType::Folder {
                            std::cmp::Ordering::Less
                        } else {
                            std::cmp::Ordering::Greater
                        }
                    }
                });

                Ok((path_str, remote_files))
            }
            Err(e) => Err(format!("SFTP Error: {}", e)),
        }
    }

    pub fn recursive_scan(&self, path: &Path) -> Result<Vec<RemoteFile>, String> {
        let mut all_files = Vec::new();
        let canonical_path = self
            .sftp
            .realpath(path)
            .map_err(|e| format!("Canonicalization failed: {}", e))?;

        let mut stack = vec![canonical_path];

        while let Some(current_path) = stack.pop() {
            if let Ok(entries) = self.sftp.readdir(&current_path) {
                for (path, stat) in entries {
                    let filename = path
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    if filename == "." || filename == ".." {
                        continue;
                    }

                    let raw_size = stat.size.unwrap_or(0);
                    let size = if stat.is_dir() {
                        "".to_string()
                    } else {
                        format_size(raw_size)
                    };
                    let file_type = if stat.is_dir() {
                        FileType::Folder
                    } else {
                        FileType::File
                    };

                    let modified = if let Some(mtime) = stat.mtime {
                        if let Some(dt) = chrono::DateTime::from_timestamp(mtime as i64, 0) {
                            dt.format("%Y-%m-%d %H:%M:%S").to_string()
                        } else {
                            "".to_string()
                        }
                    } else {
                        "".to_string()
                    };

                    let remote_file = RemoteFile {
                        name: filename,
                        path: path.to_string_lossy().to_string(),
                        size,
                        size_bytes: raw_size,
                        file_type: file_type.clone(),
                        modified,
                    };

                    if file_type == FileType::Folder {
                        stack.push(path);
                    } else {
                        all_files.push(remote_file);
                    }
                }
            }
        }
        Ok(all_files)
    }

    pub fn download_chunk(
        &self,
        remote_path: &Path,
        local_path: &Path,
        offset: u64,
        chunk_size: usize,
    ) -> Result<usize, String> {
        use std::fs::{File, OpenOptions};
        use std::io::{Read, Seek, SeekFrom, Write};

        // Open remote file
        let mut remote_file = self
            .sftp
            .open(remote_path)
            .map_err(|e| format!("Failed to open remote file: {}", e))?;

        // Seek to offset
        remote_file
            .seek(SeekFrom::Start(offset))
            .map_err(|e| format!("Failed to seek in remote file: {}", e))?;

        // Read chunk
        let mut buffer = vec![0u8; chunk_size];
        let bytes_read = remote_file
            .read(&mut buffer)
            .map_err(|e| format!("Failed to read from remote file: {}", e))?;

        if bytes_read == 0 {
            return Ok(0); // EOF
        }

        // Open/create local file
        let mut local_file = if offset == 0 {
            File::create(local_path).map_err(|e| format!("Failed to create local file: {}", e))?
        } else {
            OpenOptions::new()
                .write(true)
                .append(true)
                .open(local_path)
                .map_err(|e| format!("Failed to open local file for append: {}", e))?
        };

        // Write chunk
        local_file
            .write_all(&buffer[..bytes_read])
            .map_err(|e| format!("Failed to write to local file: {}", e))?;

        Ok(bytes_read)
    }

    #[allow(dead_code)]
    pub fn remove(&self, path: &Path) -> Result<(), String> {
        // Try to remove as file first, then as directory
        // Alternatively check stat first
        let stat = self
            .sftp
            .stat(path)
            .map_err(|e| format!("Failed to stat path: {}", e))?;

        if stat.is_dir() {
            self.sftp
                .rmdir(path)
                .map_err(|e| format!("Failed to remove directory: {}", e))
        } else {
            self.sftp
                .unlink(path)
                .map_err(|e| format!("Failed to remove file: {}", e))
        }
    }
}
