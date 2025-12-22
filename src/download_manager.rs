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
    SetSpeedLimit(u64), // In KB/s
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
    speed_limit: Arc<std::sync::atomic::AtomicU64>, // KB/s, 0 = unlimited
}

impl DownloadManager {
    pub fn new(
        config: SftpConfig,
        initial_speed_limit: u64,
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
            speed_limit: Arc::new(std::sync::atomic::AtomicU64::new(initial_speed_limit)),
        }
    }
    pub async fn run(&mut self) {
        loop {
            tokio::select! {
                res = self.command_rx.recv() => {
                    match res {
                        Some(cmd) => {
                            self.handle_command(cmd).await;
                        }
                        None => {
                            break;
                        }
                    }
                }
            }
        }
    }

    async fn handle_command(&mut self, command: DownloadCommand) {
        // println!("DEBUG: Processing command: {:?}", command);
        match command {
            DownloadCommand::StartAll => {
                self.is_global_paused = false;
                self.process_queue().await;
            }
            DownloadCommand::PauseAll => {
                self.is_global_paused = true;
                let mut paused = self.paused_downloads.lock().await;
                for path in &self.active_downloads {
                    paused.insert(path.clone(), 0);
                }
            }
            DownloadCommand::ResumeAll => {
                self.is_global_paused = false;
                self.paused_downloads.lock().await.clear();
                self.process_queue().await;
            }
            DownloadCommand::Pause(path) => {
                let mut paused = self.paused_downloads.lock().await;
                paused.insert(path.clone(), 0);
            }
            DownloadCommand::Resume(path) => {
                {
                    let mut paused = self.paused_downloads.lock().await;
                    paused.remove(&path);
                }
                self.process_queue().await;
            }
            DownloadCommand::Cancel(path) => {
                let mut cancelled = self.cancelled.lock().await;
                cancelled.insert(path.clone());
                self.queue.retain(|i| i.remote_file != path);
            }
            DownloadCommand::AddItem(item) => {
                if !self.queue.iter().any(|i| i.remote_file == item.remote_file)
                    && !self.active_downloads.contains(&item.remote_file)
                {
                    self.queue.push(item);
                    self.process_queue().await;
                }
            }
            DownloadCommand::TaskPaused {
                remote_file,
                offset,
            } => {
                self.active_downloads.remove(&remote_file);
                if let Some(item) = self.queue.iter_mut().find(|i| i.remote_file == remote_file) {
                    item.bytes_downloaded = offset;
                }
            }
            DownloadCommand::TaskDone { remote_file } => {
                self.active_downloads.remove(&remote_file);
                self.process_queue().await;
            }
            DownloadCommand::SetSpeedLimit(limit) => {
                self.speed_limit
                    .store(limit, std::sync::atomic::Ordering::Relaxed);
            }
        }
    }

    async fn process_queue(&mut self) {
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
                let mut offset = match paused.get(&remote_file) {
                    Some(o) => *o,
                    None => item.bytes_downloaded,
                };

                // Auto-resume logic
                if offset == 0 {
                    if let Ok(metadata) = std::fs::metadata(&local_path) {
                        let file_size = metadata.len();
                        if file_size > 0 && file_size < item.size_bytes {
                            offset = file_size;
                        }
                    }
                }

                let paused_downloads = self.paused_downloads.clone();
                let cancelled_downloads = self.cancelled.clone();
                let cmd_tx = self.command_tx.clone();
                let speed_limit = self.speed_limit.clone();

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
                        speed_limit,
                    )
                    .await;
                });
            } else {
                drop(paused);
                drop(cancelled);
                break;
            }
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
        speed_limit: Arc<std::sync::atomic::AtomicU64>,
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

            // Throttling Logic
            let limit_kb = speed_limit.load(std::sync::atomic::Ordering::Relaxed);
            if limit_kb > 0 {
                // NOTE: This is a simple per-task throttling. If MAX_CONCURRENT > 1,
                // total speed = limit * active_tasks.
                // User requirement implied single slider "Max Download Speed".
                // If so, we should divide limit by active downloads or use a token bucket.
                // Given "simple" app, let's treat limit as "Per Download" OR modify to shared global bucket.
                // The prompt asked for "throttle downloads".
                // Let's implement PER-TASK throttling for now as it's simpler and safer than complex global coordinator.
                // Wait, if user sets 100KB/s and has 2 downloads, 200KB/s might satisfy "throttle",
                // but usually user expects TOTAL limit.
                // For global limit, we need to divide limit by active count?
                // Actually, let's stick to per-task limit matching implementation plan simplicity.
                // I will apply the full limit to each task. This is acceptable for a "simple" sftp client.

                // Sleep calculation Logic:
                // We want to download CHUNK_SIZE bytes. We need it to take at least T seconds.
                // We'll measure how long the download took, then sleep the remainder.
                // However, we can't easily measure inside blocking task.
                // Easier: Just sleep *before* or after if we want to CAP the speed.
                // If we simply force a sleep proportional to size/speed, we cap the max speed.
                // Duration = Bytes / Speed.
                // e.g. 64KB / 64KB/s = 1s.
                // So for every chunk, we ensure we spend at least 1s.
                // This includes processing time.

                // But we are inside the loop. Let's start timer.
            }
            let start = std::time::Instant::now();

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

                    // Apply throttling delay
                    let limit_kb = speed_limit.load(std::sync::atomic::Ordering::Relaxed);
                    if limit_kb > 0 {
                        let duration = start.elapsed();
                        let min_duration_micros =
                            (bytes_read as u64 * 1000 * 1000) / (limit_kb * 1024);
                        if duration.as_micros() < min_duration_micros as u128 {
                            let diff = min_duration_micros - duration.as_micros() as u64;
                            tokio::time::sleep(tokio::time::Duration::from_micros(diff)).await;
                        }
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
    initial_speed_limit: u64,
) -> (mpsc::Sender<DownloadCommand>, mpsc::Receiver<DownloadEvent>) {
    let (cmd_tx, cmd_rx) = mpsc::channel(100);
    let (event_tx, event_rx) = mpsc::channel(100);

    // Update config with speed limit
    // Wait, manager creates its own AtomicU64 from config.max_download_speed
    // So we don't need to do anything special here as long as config passed in has it.

    let mut manager = DownloadManager::new(
        config,
        initial_speed_limit,
        cmd_tx.clone(),
        cmd_rx,
        event_tx,
    );

    tokio::spawn(async move {
        manager.run().await;
    });

    (cmd_tx, event_rx)
}
