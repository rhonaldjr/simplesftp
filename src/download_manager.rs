use crate::mock_data::{QueueItem, TransferStatus};
use crate::settings::SftpConfig;
use crate::sftp_client::SftpClient;

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::{mpsc, Mutex};

const CHUNK_SIZE: usize = 65536; // 64KB chunks
const MAX_CONCURRENT: usize = 2;

#[derive(Debug, Clone)]
pub enum DownloadCommand {
    StartAll,
    PauseAll,
    ResumeAll,
    Pause(String), // remote_file path
    Resume(String),
    Cancel(String),
    AddItem(QueueItem),
    // Internal commands sent by download tasks
    TaskPaused { remote_file: String, offset: u64 },
    TaskDone { remote_file: String },
}

#[derive(Debug, Clone)]
pub enum DownloadEvent {
    Progress {
        remote_file: String,
        bytes_downloaded: u64,
    },
    Completed {
        remote_file: String,
    },
    Failed {
        remote_file: String,
        error: String,
    },
    Paused {
        remote_file: String,
    },
    Started {
        remote_file: String,
    },
}

pub struct DownloadManager {
    config: SftpConfig,
    command_tx: mpsc::Sender<DownloadCommand>, // Need this to pass to tasks
    command_rx: mpsc::Receiver<DownloadCommand>,
    event_tx: mpsc::Sender<DownloadEvent>,
    queue: Vec<QueueItem>,
    active_downloads: HashSet<String>,
    paused_downloads: Arc<Mutex<HashMap<String, u64>>>, // Shared for pause checking
    cancelled: Arc<Mutex<HashSet<String>>>,             // Shared for cancel checking
    is_global_paused: bool,
}

impl DownloadManager {
    pub fn new(
        config: SftpConfig,
        command_tx: mpsc::Sender<DownloadCommand>,
        command_rx: mpsc::Receiver<DownloadCommand>,
        event_tx: mpsc::Sender<DownloadEvent>,
    ) -> Self {
        Self {
            config,
            command_tx,
            command_rx,
            event_tx,
            queue: Vec::new(),
            active_downloads: HashSet::new(),
            paused_downloads: Arc::new(Mutex::new(HashMap::new())),
            cancelled: Arc::new(Mutex::new(HashSet::new())),
            is_global_paused: false,
        }
    }

    pub async fn run(mut self) {
        loop {
            // Process commands
            while let Ok(cmd) = self.command_rx.try_recv() {
                match cmd {
                    DownloadCommand::AddItem(item) => {
                        self.queue.push(item);
                    }
                    DownloadCommand::StartAll => {
                        // Will be processed below
                    }
                    DownloadCommand::PauseAll => {
                        self.is_global_paused = true;
                        // Pause all active downloads
                        let mut paused = self.paused_downloads.lock().await;

                        for path in &self.active_downloads {
                            paused.insert(path.clone(), 0);
                        }
                    }
                    DownloadCommand::ResumeAll => {
                        self.is_global_paused = false;
                        let mut paused = self.paused_downloads.lock().await;
                        paused.clear();
                    }
                    DownloadCommand::Pause(path) => {
                        let mut paused = self.paused_downloads.lock().await;
                        paused.insert(path.clone(), 0);
                    }
                    DownloadCommand::Resume(path) => {
                        let mut paused = self.paused_downloads.lock().await;
                        paused.remove(&path);
                    }
                    DownloadCommand::Cancel(path) => {
                        let mut cancelled = self.cancelled.lock().await;
                        cancelled.insert(path.clone());
                        // Also remove from queue immediately so it doesn't get picked up again
                        self.queue.retain(|i| i.remote_file != path);
                    }
                    DownloadCommand::TaskPaused {
                        remote_file,
                        offset,
                    } => {
                        self.active_downloads.remove(&remote_file);
                        // Update queue item with progress so we can resume correctly
                        if let Some(item) =
                            self.queue.iter_mut().find(|i| i.remote_file == remote_file)
                        {
                            item.bytes_downloaded = offset;
                        }
                    }
                    DownloadCommand::TaskDone { remote_file } => {
                        self.active_downloads.remove(&remote_file);
                    }
                }
            }

            // Start downloads if we have capacity AND NOT PAUSED GLOBALLY
            while self.active_downloads.len() < MAX_CONCURRENT && !self.is_global_paused {
                // Find next pending item that's not paused or cancelled
                let paused = self.paused_downloads.lock().await;
                let cancelled = self.cancelled.lock().await;

                let next_item = self.queue.iter().find(|item| {
                    item.status == TransferStatus::Pending
                        && !self.active_downloads.contains(&item.remote_file)
                        && !paused.contains_key(&item.remote_file)
                        && !cancelled.contains(&item.remote_file)
                });

                if let Some(item) = next_item {
                    let remote_file = item.remote_file.clone();
                    let local_path = format!("{}/{}", item.local_location, item.filename);
                    let config = self.config.clone();
                    let event_tx = self.event_tx.clone();

                    // Determine start offset: use stored item progress if available
                    let offset = match paused.get(&remote_file) {
                        Some(o) => *o,
                        None => item.bytes_downloaded,
                    };

                    let paused_downloads = self.paused_downloads.clone();
                    let cancelled_downloads = self.cancelled.clone();
                    let cmd_tx = self.command_tx.clone();

                    drop(paused);
                    drop(cancelled);

                    self.active_downloads.insert(remote_file.clone());

                    let _ = self
                        .event_tx
                        .send(DownloadEvent::Started {
                            remote_file: remote_file.clone(),
                        })
                        .await;

                    // Spawn download task with shared pause/cancel state
                    let remote_file_clone = remote_file.clone();
                    tokio::spawn(async move {
                        Self::download_file(
                            config,
                            remote_file_clone,
                            local_path,
                            offset,
                            event_tx,
                            cmd_tx,
                            paused_downloads,
                            cancelled_downloads,
                        )
                        .await;
                    });
                } else {
                    drop(paused);
                    drop(cancelled);
                    break;
                }
            }

            // Small sleep to avoid busy loop
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn download_file(
        config: SftpConfig,
        remote_file: String,
        local_path: String,
        start_offset: u64,
        event_tx: mpsc::Sender<DownloadEvent>,
        cmd_tx: mpsc::Sender<DownloadCommand>,
        paused_downloads: Arc<Mutex<HashMap<String, u64>>>,
        cancelled_downloads: Arc<Mutex<HashSet<String>>>,
    ) {
        // Connect to SFTP
        let client = match tokio::task::spawn_blocking({
            let config = config.clone();
            move || SftpClient::connect(&config)
        })
        .await
        {
            Ok(Ok(client)) => client,
            Ok(Err(e)) => {
                let _ = event_tx
                    .send(DownloadEvent::Failed {
                        remote_file: remote_file.clone(),
                        error: e,
                    })
                    .await;
                let _ = cmd_tx.send(DownloadCommand::TaskDone { remote_file }).await;
                return;
            }
            Err(e) => {
                let _ = event_tx
                    .send(DownloadEvent::Failed {
                        remote_file: remote_file.clone(),
                        error: e.to_string(),
                    })
                    .await;
                let _ = cmd_tx.send(DownloadCommand::TaskDone { remote_file }).await;
                return;
            }
        };

        let client = Arc::new(Mutex::new(client));
        let mut bytes_downloaded = start_offset;

        loop {
            // Check if paused
            {
                let paused = paused_downloads.lock().await;
                if paused.contains_key(&remote_file) {
                    // Store current progress and exit
                    drop(paused);
                    let mut paused = paused_downloads.lock().await;
                    paused.insert(remote_file.clone(), bytes_downloaded);
                    let _ = event_tx
                        .send(DownloadEvent::Paused {
                            remote_file: remote_file.clone(),
                        })
                        .await;
                    // Notify manager to clear active state and persist offset
                    let _ = cmd_tx
                        .send(DownloadCommand::TaskPaused {
                            remote_file,
                            offset: bytes_downloaded,
                        })
                        .await;
                    return;
                }
            }

            // Check if cancelled
            {
                let cancelled = cancelled_downloads.lock().await;
                if cancelled.contains(&remote_file) {
                    let _ = cmd_tx.send(DownloadCommand::TaskDone { remote_file }).await;
                    return;
                }
            }

            let client_clone = client.clone();
            let remote_path = remote_file.clone();
            let local = local_path.clone();
            let offset = bytes_downloaded;

            let result = tokio::task::spawn_blocking(move || {
                let c = client_clone.blocking_lock();
                c.download_chunk(
                    Path::new(&remote_path),
                    Path::new(&local),
                    offset,
                    CHUNK_SIZE,
                )
            })
            .await;

            match result {
                Ok(Ok(bytes_read)) => {
                    if bytes_read == 0 {
                        // Download complete
                        let _ = event_tx
                            .send(DownloadEvent::Completed {
                                remote_file: remote_file.clone(),
                            })
                            .await;
                        let _ = cmd_tx.send(DownloadCommand::TaskDone { remote_file }).await;
                        break;
                    }

                    bytes_downloaded += bytes_read as u64;

                    let _ = event_tx
                        .send(DownloadEvent::Progress {
                            remote_file: remote_file.clone(),
                            bytes_downloaded,
                        })
                        .await;
                }
                Ok(Err(e)) => {
                    let _ = event_tx
                        .send(DownloadEvent::Failed {
                            remote_file: remote_file.clone(),
                            error: e,
                        })
                        .await;
                    let _ = cmd_tx.send(DownloadCommand::TaskDone { remote_file }).await;
                    break;
                }
                Err(e) => {
                    let _ = event_tx
                        .send(DownloadEvent::Failed {
                            remote_file: remote_file.clone(),
                            error: e.to_string(),
                        })
                        .await;
                    let _ = cmd_tx.send(DownloadCommand::TaskDone { remote_file }).await;
                    break;
                }
            }
        }
    }
}

/// Creates a download manager and returns the command sender and event receiver
pub fn create_download_manager(
    config: SftpConfig,
) -> (mpsc::Sender<DownloadCommand>, mpsc::Receiver<DownloadEvent>) {
    let (cmd_tx, cmd_rx) = mpsc::channel(100);
    let (event_tx, event_rx) = mpsc::channel(100);

    let manager = DownloadManager::new(config, cmd_tx.clone(), cmd_rx, event_tx);

    tokio::spawn(async move {
        manager.run().await;
    });

    (cmd_tx, event_rx)
}
