use crate::mock_data::{FileType, RemoteFile};
use crate::settings::SftpConfig;

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

                    let size = if stat.is_dir() {
                        "".to_string()
                    } else {
                        format!("{} B", stat.size.unwrap_or(0))
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

                    let size = if stat.is_dir() {
                        "".to_string()
                    } else {
                        format!("{}", stat.size.unwrap_or(0))
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
}
