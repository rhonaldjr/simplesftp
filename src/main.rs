mod components;
mod download_manager;
mod mock_data;
mod scheduler;
mod settings;
mod sftp_client;
mod style;
mod tray;

use download_manager::{DownloadCommand, DownloadEvent};
use iced::widget::{
    button, checkbox, column, container, horizontal_rule, horizontal_space, mouse_area, pane_grid,
    radio, row, scrollable, stack, text, text_input, vertical_space,
};
use iced::{Element, Length, Task, Theme};
use mock_data::{FileType, QueueItem, RemoteFile, TransferStatus};
use scheduler::Scheduler;
use settings::AppConfig;
use sftp_client::SftpClient;
use tray::{TrayAction, TrayManager};

use chrono::Local;
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::sync::mpsc;

pub fn main() -> iced::Result {
    iced::application("Simple SFTP", SftpApp::update, SftpApp::view)
        .theme(|_| Theme::Dark)
        .subscription(SftpApp::subscription)
        .run_with(SftpApp::new)
}

impl SftpApp {
    fn new() -> (Self, Task<Message>) {
        let mut app = Self::default();
        println!(
            "DEBUG: SftpApp::new - Auto Connect: {}, Last Path: {}",
            app.config.auto_connect, app.config.last_remote_path
        );
        if app.config.auto_connect && !app.config.sftp_config.host.is_empty() {
            app.status_message = format!("Auto-connecting to {}...", app.config.sftp_config.host);
            println!("DEBUG: Triggering Auto-Connect Task");
            return (
                app,
                Task::done(Message::ConfigOptionSelected(ConfigOption::Connect)),
            );
        }
        (app, Task::none())
    }
}

struct SftpApp {
    config: AppConfig,
    state: AppState,
    is_config_menu_open: bool,
    panes: pane_grid::State<PaneState>,
    // State
    is_connected: bool,
    is_checking_connection: bool,
    settings_error: Option<String>,
    app_error: Option<String>,
    sftp_client: Option<Arc<Mutex<SftpClient>>>,
    // Selection & Navigation
    selected_file: Option<String>,
    last_click: Option<(String, Instant)>,
    // Mock Data
    queue_items: Vec<QueueItem>,
    remote_files: Vec<RemoteFile>,
    current_remote_path: String,
    // Context Menu / Hover
    hovered_file: Option<String>,
    is_scanning_queue: bool,
    // Download Manager
    download_tx: Option<mpsc::Sender<DownloadCommand>>,
    download_rx: Option<Arc<tokio::sync::Mutex<mpsc::Receiver<DownloadEvent>>>>,
    is_downloading: bool,
    selected_queue_item: Option<String>,
    // Tray Icon
    tray_manager: Option<TrayManager>,
    last_schedule_allowed: bool,
    status_message: String,
}

#[derive(Debug, Clone)]
enum PaneState {
    Queue,
    Remote,
}

use std::fs::File;
use std::io::{BufReader, BufWriter};

fn save_queue(queue: &[QueueItem]) {
    if let Ok(file) = File::create("queue.json") {
        let writer = BufWriter::new(file);
        let _ = serde_json::to_writer(writer, queue);
    }
}

fn load_queue() -> Vec<QueueItem> {
    if let Ok(file) = File::open("queue.json") {
        let reader = BufReader::new(file);
        if let Ok(items) = serde_json::from_reader(reader) {
            return items;
        }
    }
    Vec::new()
}

impl Default for SftpApp {
    fn default() -> Self {
        let (mut panes, first_pane) = pane_grid::State::new(PaneState::Queue);
        let (_, split) = panes
            .split(pane_grid::Axis::Vertical, first_pane, PaneState::Remote)
            .expect("Split failed");

        panes.resize(split, 0.4); // 40% Queue, 60% Remote

        Self {
            config: AppConfig::load(),
            state: AppState::MainView,
            is_config_menu_open: false,
            panes,
            is_connected: false,
            is_checking_connection: false,
            settings_error: None,
            app_error: None,
            sftp_client: None,
            selected_file: None,
            last_click: None,
            queue_items: load_queue(),
            remote_files: Vec::new(),
            current_remote_path: ".".into(), // Start at home/current directory
            hovered_file: None,
            is_scanning_queue: false,
            download_tx: None,
            download_rx: None,
            is_downloading: false,
            selected_queue_item: None,
            tray_manager: None,
            last_schedule_allowed: true,
            status_message: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum AppState {
    MainView,
    SettingsView,
    ScheduleView,
}

#[derive(Debug, Clone)]
enum Message {
    ToggleConfigMenu,
    ConfigOptionSelected(ConfigOption),
    // Settings Form
    HostChanged(String),
    PortChanged(String),
    UsernameChanged(String),
    PasswordChanged(String),
    SaveSettings,
    CancelSettings,
    ConnectionResult(Result<Arc<Mutex<SftpClient>>, String>),
    RemoteFilesLoaded(String, Result<(String, Vec<RemoteFile>), String>),
    // Remote Navigation
    RemoteFileClicked(RemoteFile),
    GoToParent,
    // Local Navigation
    SelectDownloadPath,
    DownloadPathSelected(Option<std::path::PathBuf>),
    RemoteFileDoubleClicked(String),
    // Hover & Actions
    HoverFile(String),
    UnhoverFile,
    QueueFile(RemoteFile),
    DownloadFile(RemoteFile),
    // Scan result (auto_start)
    ScanResult(Result<Vec<RemoteFile>, String>, bool),
    // Queue Persistence & Resume
    ResumeQueue,
    QueueVerificationResult(Vec<(String, bool, u64)>),
    // Remote
    RefreshRemote,
    // Pane
    PaneResized(pane_grid::ResizeEvent),
    // Downloads
    StartDownloads,
    PollDownloadEvents,
    PauseDownload(String),
    ResumeDownload(String),
    CancelDownload(String),
    DownloadProgress {
        remote_file: String,
        bytes_downloaded: u64,
    },
    DownloadCompleted(String),
    DownloadFailed {
        remote_file: String,
        error: String,
    },
    DownloadStarted(String),
    QueueItemClicked(String),
    // Tray
    TrayEvent,
    HideToTray,
    ShowWindow,
    // Schedule
    ScheduleModeChanged(settings::ScheduleMode),
    ScheduleStartTimeChanged(u8, u8),
    Tick(Instant), // Periodic check
    ScheduleEndTimeChanged(u8, u8),
    ScheduleDayToggled(u8), // 0=Mon, 6=Sun
    SaveSchedule,
    CancelSchedule,
    // Toolbar
    NoOp,
    // Window Events
    Event(iced::Event),
}

#[derive(Debug, Clone)]
enum ConfigOption {
    Settings,
    Connect,
    Schedule,
    Minimize,
    Disconnect,
    Exit,
}

impl SftpApp {
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::ToggleConfigMenu => {
                self.is_config_menu_open = !self.is_config_menu_open;
            }
            Message::ConfigOptionSelected(option) => {
                self.is_config_menu_open = false;
                match option {
                    ConfigOption::Settings => {
                        self.settings_error = None;
                        self.state = AppState::SettingsView;
                    }
                    ConfigOption::Connect => {
                        println!("DEBUG: ConfigOption::Connect selected");
                        if !self.config.sftp_config.host.is_empty() {
                            self.is_checking_connection = true;
                            self.status_message =
                                format!("Connecting to {}...", self.config.sftp_config.host);
                            let config = self.config.sftp_config.clone();

                            return Task::future(async move {
                                let res = tokio::task::spawn_blocking(move || {
                                    SftpClient::connect(&config)
                                })
                                .await
                                .unwrap_or_else(|e| Err(e.to_string()));

                                Message::ConnectionResult(res.map(|c| Arc::new(Mutex::new(c))))
                            });
                        }
                    }
                    ConfigOption::Schedule => {
                        self.state = AppState::ScheduleView;
                    }
                    ConfigOption::Minimize => {
                        return self.update(Message::HideToTray);
                    }
                    ConfigOption::Disconnect => {
                        self.is_connected = false;
                        self.sftp_client = None;
                        self.remote_files.clear();
                    }
                    ConfigOption::Exit => {
                        self.config.last_remote_path = self.current_remote_path.clone();
                        self.config.auto_connect = self.is_connected;
                        let _ = self.config.save();
                        save_queue(&self.queue_items);
                        return iced::exit();
                    }
                }
            }
            Message::PaneResized(event) => {
                self.panes.resize(event.split, event.ratio);
            }
            Message::SaveSettings => {
                self.is_checking_connection = true;
                self.settings_error = None;
                let config = self.config.sftp_config.clone();

                return Task::future(async move {
                    let res = tokio::task::spawn_blocking(move || SftpClient::connect(&config))
                        .await
                        .unwrap_or_else(|e| Err(e.to_string()));

                    Message::ConnectionResult(res.map(|c| Arc::new(Mutex::new(c))))
                });
            }
            Message::ConnectionResult(result) => {
                self.is_checking_connection = false;
                match result {
                    Ok(client) => {
                        let _ = self.config.save();
                        self.is_connected = true;
                        self.sftp_client = Some(client.clone());
                        self.app_error = None; // clear error
                        self.state = AppState::MainView;
                        self.status_message = "Connected. Restoring session...".into();

                        println!(
                            "DEBUG: ConnectionResult - Last Path: '{}'",
                            self.config.last_remote_path
                        );
                        // Restore Last Path
                        let path = if !self.config.last_remote_path.is_empty() {
                            self.config.last_remote_path.clone()
                        } else {
                            ".".to_string()
                        };
                        println!("DEBUG: ConnectionResult - Using Path: '{}'", path);
                        self.current_remote_path = path.clone();

                        // Trigger file listing
                        // client is already Arc<Mutex<SftpClient>>, so clone is cheap
                        let list_client = client.clone();

                        let listing_task = Task::future(async move {
                            let path_clone = path.clone();
                            let res = tokio::task::spawn_blocking(move || {
                                let c = list_client.lock().unwrap();
                                c.list_dir(std::path::Path::new(&path_clone))
                            })
                            .await
                            .unwrap_or_else(|e| Err(e.to_string()));

                            Message::RemoteFilesLoaded(path, res)
                        });

                        // Trigger Queue Resume Check
                        let resume_task = Task::done(Message::ResumeQueue);

                        return Task::batch(vec![listing_task, resume_task]);
                    }
                    Err(e) => {
                        self.settings_error = Some(e);
                    }
                }
            }
            Message::RemoteFilesLoaded(req_path, result) => match result {
                Ok((resolved_path, files)) => {
                    self.remote_files = files;
                    self.current_remote_path = resolved_path;
                    self.selected_file = None;
                    self.app_error = None;
                }
                Err(e) => {
                    self.app_error = Some(format!("Error loading {}: {}", req_path, e));
                }
            },
            Message::RemoteFileClicked(file) => {
                self.selected_file = Some(file.name.clone());

                let now = Instant::now();
                let mut navigate = false;

                if let Some((last_name, last_time)) = &self.last_click {
                    if *last_name == file.name && now.duration_since(*last_time).as_millis() < 500 {
                        navigate = true;
                    }
                }
                self.last_click = Some((file.name.clone(), now));

                if navigate && file.file_type == FileType::Folder {
                    if file.name == ".." {
                        return self.update(Message::GoToParent);
                    }

                    // Enter folder
                    if let Some(client) = &self.sftp_client {
                        let client = client.clone();
                        let name = file.name;
                        // Calculate target path, but don't set it yet
                        let new_path = if self.current_remote_path.ends_with('/') {
                            format!("{}{}", self.current_remote_path, name)
                        } else {
                            format!("{}/{}", self.current_remote_path, name)
                        };

                        self.last_click = None; // Reset click tracking

                        return Task::future(async move {
                            let path_clone = new_path.clone();
                            let res = tokio::task::spawn_blocking(move || {
                                let c = client.lock().unwrap();
                                c.list_dir(std::path::Path::new(&path_clone))
                            })
                            .await
                            .unwrap_or_else(|e| Err(e.to_string()));
                            Message::RemoteFilesLoaded(new_path, res)
                        });
                    }
                }
            }
            Message::ResumeQueue => {
                if let Some(client) = self.sftp_client.clone() {
                    let items_to_check: Vec<(String, String)> = self
                        .queue_items
                        .iter()
                        .filter(|i| {
                            i.status == TransferStatus::Pending
                                || i.status == TransferStatus::Downloading
                                || i.status == TransferStatus::Paused
                        })
                        .map(|i| (i.remote_file.clone(), i.filename.clone()))
                        .collect();

                    if items_to_check.is_empty() {
                        return Task::none();
                    }

                    return Task::future(async move {
                        let res = tokio::task::spawn_blocking(move || {
                            let c = client.lock().unwrap();
                            let mut results = Vec::new();
                            for (path, _name) in items_to_check {
                                // Check if file exists and get size
                                match c.get_file_size(&path) {
                                    Ok(size) => results.push((path, true, size)),
                                    Err(_) => results.push((path, false, 0)),
                                }
                            }
                            results
                        })
                        .await
                        .unwrap_or_default();

                        Message::QueueVerificationResult(res)
                    });
                }
            }
            Message::QueueVerificationResult(results) => {
                let mut changed = false;
                for (path, exists, size) in results {
                    if let Some(item) = self.queue_items.iter_mut().find(|i| i.remote_file == path)
                    {
                        if !exists {
                            item.status = TransferStatus::Failed("Remote file missing".into());
                            changed = true;
                        } else {
                            if item.size_bytes == 0 {
                                item.size_bytes = size;
                                changed = true;
                            }
                            // Reset 'Downloading' to 'Pending' so manager picks it up (Auto-Resume)
                            if item.status == TransferStatus::Downloading {
                                item.status = TransferStatus::Pending;
                                changed = true;
                            }
                        }
                    }
                }

                if changed {
                    save_queue(&self.queue_items);
                }

                let pending_count = self
                    .queue_items
                    .iter()
                    .filter(|i| i.status == TransferStatus::Pending)
                    .count();
                if pending_count > 0 {
                    self.status_message = format!("Resuming {} downloads...", pending_count);
                } else {
                    self.status_message = "Connected.".to_string();
                }

                // Try to start manager if we have pending items
                return self.start_manager();
            }
            Message::HoverFile(filename) => {
                self.hovered_file = Some(filename);
            }
            Message::UnhoverFile => {
                self.hovered_file = None;
            }
            Message::QueueFile(file) => {
                // Check if it's a file or folder
                if file.file_type == FileType::File {
                    self.is_scanning_queue = true;
                    let file_clone = file.clone();
                    return Task::future(async move {
                        Message::ScanResult(Ok(vec![file_clone]), false)
                    });
                }

                // Queue only (don't auto-start)
                self.is_scanning_queue = true;

                let client = self.sftp_client.clone();
                let path = file.path.clone();
                let file_clone = file.clone(); // Clone file for the `Ok(vec![file_clone])` case

                return Task::future(async move {
                    let res = tokio::task::spawn_blocking(move || {
                        if let Some(client) = client {
                            let c = client.lock().unwrap();
                            c.recursive_scan(std::path::Path::new(&path))
                        } else {
                            // If client is not available, we can't scan, but we can still queue the single file
                            Ok(vec![file_clone])
                        }
                    })
                    .await
                    .unwrap_or_else(|e| Err(e.to_string()));

                    Message::ScanResult(res, false) // auto_start = false
                });
            }
            Message::DownloadFile(file) => {
                // Check if it's a file or folder
                if file.file_type == FileType::File {
                    self.is_scanning_queue = true;
                    let file_clone = file.clone();
                    return Task::future(
                        async move { Message::ScanResult(Ok(vec![file_clone]), true) },
                    );
                }

                // Recursively scan path
                self.is_scanning_queue = true;

                let client = self.sftp_client.clone();
                let path = file.path.clone();
                let file_clone = file.clone();

                return Task::future(async move {
                    let res = tokio::task::spawn_blocking(move || {
                        if let Some(client) = client {
                            let c = client.lock().unwrap();
                            c.recursive_scan(std::path::Path::new(&path))
                        } else {
                            Ok(vec![file_clone])
                        }
                    })
                    .await
                    .unwrap_or_else(|e| Err(e.to_string()));

                    Message::ScanResult(res, true) // auto_start = true
                });
            }
            Message::RefreshRemote => {
                if let Some(client) = &self.sftp_client {
                    let client = client.clone();
                    // Reload current path
                    let path = self.current_remote_path.clone();

                    return Task::future(async move {
                        let path_clone = path.clone();
                        let res = tokio::task::spawn_blocking(move || {
                            let c = client.lock().unwrap();
                            c.list_dir(std::path::Path::new(&path_clone))
                        })
                        .await
                        .unwrap_or_else(|e| Err(e.to_string()));
                        Message::RemoteFilesLoaded(path, res)
                    });
                }
            }
            Message::ScanResult(result, auto_start) => {
                self.is_scanning_queue = false;
                println!("DEBUG: ScanResult received. Auto-start: {}", auto_start);
                match result {
                    Ok(files) => {
                        println!("DEBUG: Found {} files.", files.len());
                        for file in files {
                            if !self.queue_items.iter().any(|i| i.remote_file == file.path) {
                                let item = QueueItem {
                                    local_location: self.config.local_download_path.clone(),
                                    filename: file.name,
                                    remote_file: file.path,
                                    size_bytes: file.size_bytes,
                                    bytes_downloaded: 0,
                                    priority: 10,
                                    status: TransferStatus::Pending,
                                };
                                self.queue_items.push(item.clone());
                                println!("DEBUG: Added item to queue: {}", item.filename);

                                // If downloading is active, send the item to the manager immediately
                                if self.is_downloading {
                                    if let Some(tx) = &self.download_tx {
                                        // Always add to manager if it's running. It will handle queueing/starting.
                                        match tx.try_send(DownloadCommand::AddItem(item)) {
                                            Ok(_) => println!("DEBUG: Sent AddItem to manager"),
                                            Err(e) => {
                                                println!("DEBUG: Failed to send AddItem: {}", e)
                                            }
                                        }
                                    }
                                }
                            } else {
                                println!("DEBUG: Item already in queue: {}", file.name);
                            }
                        }

                        // auto-start logic
                        if auto_start
                            && !self.is_downloading
                            && self
                                .queue_items
                                .iter()
                                .any(|i| i.status == TransferStatus::Pending)
                        {
                            println!("DEBUG: Auto-starting manager...");
                            return self.start_manager();
                        }
                    }
                    Err(e) => {
                        println!("DEBUG: Scan failed: {}", e);
                        self.app_error = Some(format!("Scan failed: {}", e));
                    }
                }
            }
            Message::GoToParent => {
                if let Some(client) = &self.sftp_client {
                    let client = client.clone();
                    // Calculate parent path
                    let parent = std::path::Path::new(&self.current_remote_path)
                        .parent()
                        .unwrap_or(std::path::Path::new("/"))
                        .to_string_lossy()
                        .to_string();

                    let parent = if parent.is_empty() {
                        "/".to_string()
                    } else {
                        parent
                    };

                    return Task::future(async move {
                        let path_clone = parent.clone();
                        let res = tokio::task::spawn_blocking(move || {
                            let c = client.lock().unwrap();
                            c.list_dir(std::path::Path::new(&path_clone))
                        })
                        .await
                        .unwrap_or_else(|e| Err(e.to_string()));
                        Message::RemoteFilesLoaded(parent, res)
                    });
                }
            }
            Message::SelectDownloadPath => {
                return Task::future(async {
                    let path = tokio::task::spawn_blocking(|| rfd::FileDialog::new().pick_folder())
                        .await
                        .unwrap_or(None);
                    Message::DownloadPathSelected(path)
                });
            }
            Message::DownloadPathSelected(path) => {
                if let Some(p) = path {
                    self.config.local_download_path = p.to_string_lossy().to_string();
                    let _ = self.config.save();
                }
            }

            Message::CancelSettings => self.state = AppState::MainView,
            Message::HostChanged(val) => self.config.sftp_config.host = val,
            Message::PortChanged(val) => {
                if let Ok(p) = val.parse::<u16>() {
                    self.config.sftp_config.port = p;
                }
            }
            Message::UsernameChanged(val) => self.config.sftp_config.username = val,
            Message::PasswordChanged(val) => self.config.sftp_config.password = Some(val),

            // Download Controls
            Message::StartDownloads => {
                return self.start_manager();
            }
            Message::PollDownloadEvents => {
                if let Some(rx) = &self.download_rx {
                    let rx = rx.clone();
                    return Task::future(async move {
                        let mut guard = rx.lock().await;
                        match guard.recv().await {
                            Some(DownloadEvent::Progress {
                                remote_file,
                                bytes_downloaded,
                            }) => Message::DownloadProgress {
                                remote_file,
                                bytes_downloaded,
                            },
                            Some(DownloadEvent::Completed { remote_file }) => {
                                Message::DownloadCompleted(remote_file)
                            }
                            Some(DownloadEvent::Failed { remote_file, error }) => {
                                Message::DownloadFailed { remote_file, error }
                            }
                            Some(DownloadEvent::Started { remote_file }) => {
                                Message::DownloadStarted(remote_file)
                            }
                            Some(DownloadEvent::Paused { remote_file: _ }) => {
                                Message::PollDownloadEvents // Continue polling
                            }
                            None => Message::NoOp,
                        }
                    });
                }
            }
            Message::PauseDownload(path) => {
                if let Some(tx) = &self.download_tx {
                    let _ = tx.try_send(DownloadCommand::Pause(path.clone()));
                }
                if let Some(item) = self.queue_items.iter_mut().find(|i| i.remote_file == path) {
                    item.status = TransferStatus::Paused;
                    save_queue(&self.queue_items);
                }
            }
            Message::ResumeDownload(path) => {
                if let Some(tx) = &self.download_tx {
                    let _ = tx.try_send(DownloadCommand::Resume(path.clone()));
                }
                if let Some(item) = self.queue_items.iter_mut().find(|i| i.remote_file == path) {
                    item.status = TransferStatus::Downloading;
                    save_queue(&self.queue_items);
                }
            }
            Message::CancelDownload(path) => {
                if let Some(tx) = &self.download_tx {
                    let _ = tx.try_send(DownloadCommand::Cancel(path.clone()));
                }
                self.queue_items.retain(|i| i.remote_file != path);
                save_queue(&self.queue_items);
            }
            Message::DownloadProgress {
                remote_file,
                bytes_downloaded,
            } => {
                if let Some(item) = self
                    .queue_items
                    .iter_mut()
                    .find(|i| i.remote_file == remote_file)
                {
                    item.bytes_downloaded = bytes_downloaded;
                    item.status = TransferStatus::Downloading;
                }
                // Continue polling for more events
                return self.update(Message::PollDownloadEvents);
            }
            Message::DownloadCompleted(remote_file) => {
                if let Some(item) = self
                    .queue_items
                    .iter_mut()
                    .find(|i| i.remote_file == remote_file)
                {
                    item.status = TransferStatus::Completed;
                    item.bytes_downloaded = item.size_bytes;
                }
                save_queue(&self.queue_items);
                // Continue polling for more events
                return self.update(Message::PollDownloadEvents);
            }
            Message::DownloadFailed { remote_file, error } => {
                if let Some(item) = self
                    .queue_items
                    .iter_mut()
                    .find(|i| i.remote_file == remote_file)
                {
                    item.status = TransferStatus::Failed(error);
                }
                save_queue(&self.queue_items);
                // Continue polling for more events
                return self.update(Message::PollDownloadEvents);
            }
            Message::DownloadStarted(remote_file) => {
                if let Some(item) = self
                    .queue_items
                    .iter_mut()
                    .find(|i| i.remote_file == remote_file)
                {
                    item.status = TransferStatus::Downloading;
                }
                save_queue(&self.queue_items);
                // Continue polling for more events
                return self.update(Message::PollDownloadEvents);
            }
            Message::QueueItemClicked(path) => {
                self.selected_queue_item = Some(path);
            }

            // Tray Icon Events
            Message::TrayEvent => {
                if let Some(tray) = &self.tray_manager {
                    tray.update(); // Pump GTK events
                    if let Some(action) = tray.poll_events() {
                        match action {
                            TrayAction::Show => {
                                return self.update(Message::ShowWindow);
                            }
                            TrayAction::Exit => {
                                self.config.last_remote_path = self.current_remote_path.clone();
                                self.config.auto_connect = self.is_connected;
                                let _ = self.config.save();
                                save_queue(&self.queue_items);
                                return iced::exit();
                            }
                        }
                    }
                }
            }
            Message::HideToTray => {
                // Create tray icon if it doesn't exist
                if self.tray_manager.is_none() {
                    match TrayManager::new() {
                        Ok(tray) => {
                            tray.update(); // Initial pump
                            self.tray_manager = Some(tray);
                        }
                        Err(e) => {
                            self.app_error = Some(format!("Failed to create tray icon: {}", e));
                            return Task::none();
                        }
                    }
                }
                // Hide window
                return iced::window::get_latest().and_then(iced::window::close);
            }
            Message::ShowWindow => {
                // Remove tray icon
                self.tray_manager = None;
                // Window will be shown automatically when tray is removed
                // or we can create a new window if needed
                return Task::none();
            }

            // Schedule Config
            Message::ScheduleModeChanged(mode) => {
                self.config.schedule.mode = mode;
            }
            Message::ScheduleStartTimeChanged(hour, minute) => {
                self.config.schedule.start_time.hour = hour;
                self.config.schedule.start_time.minute = minute;
            }
            Message::ScheduleEndTimeChanged(hour, minute) => {
                self.config.schedule.end_time.hour = hour;
                self.config.schedule.end_time.minute = minute;
            }
            Message::ScheduleDayToggled(day_idx) => match day_idx {
                0 => self.config.schedule.days.mon = !self.config.schedule.days.mon,
                1 => self.config.schedule.days.tue = !self.config.schedule.days.tue,
                2 => self.config.schedule.days.wed = !self.config.schedule.days.wed,
                3 => self.config.schedule.days.thu = !self.config.schedule.days.thu,
                4 => self.config.schedule.days.fri = !self.config.schedule.days.fri,
                5 => self.config.schedule.days.sat = !self.config.schedule.days.sat,
                6 => self.config.schedule.days.sun = !self.config.schedule.days.sun,
                _ => {}
            },
            Message::Tick(_) => {
                let now = Local::now();
                let allowed = Scheduler::is_allowed(&self.config.schedule, now);

                if allowed != self.last_schedule_allowed {
                    self.last_schedule_allowed = allowed;
                    if let Some(tx) = &self.download_tx {
                        if self.is_downloading {
                            if allowed {
                                let _ = tx.try_send(DownloadCommand::ResumeAll);
                            } else {
                                let _ = tx.try_send(DownloadCommand::PauseAll);
                            }
                        }
                    }
                }

                // Auto-start check
                if allowed && !self.is_downloading {
                    // Check if we have pending items
                    if self
                        .queue_items
                        .iter()
                        .any(|i| i.status == TransferStatus::Pending)
                    {
                        return self.start_manager();
                    }
                }
            }
            Message::SaveSchedule => {
                let _ = self.config.save();
                self.state = AppState::MainView;
            }
            Message::CancelSchedule => {
                // reload from disk to revert changes or just switch view?
                // For now just switch, but changes in memory obey immediate mode.
                // Ideally we should have a temp config or reload.
                self.config = AppConfig::load(); // Revert
                self.state = AppState::MainView;
            }

            Message::Event(event) => {
                if let iced::Event::Window(iced::window::Event::CloseRequested) = event {
                    println!("DEBUG: Window Close Requested. Saving config...");
                    self.config.last_remote_path = self.current_remote_path.clone();
                    self.config.auto_connect = self.is_connected;
                    match self.config.save() {
                        Ok(_) => println!(
                            "DEBUG: Config saved successfully. Path: {}",
                            self.config.last_remote_path
                        ),
                        Err(e) => println!("DEBUG: Failed to save config: {}", e),
                    }
                    save_queue(&self.queue_items);
                    return iced::exit();
                }
            }
            _ => {}
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        match self.state {
            AppState::SettingsView => return self.view_settings(),
            AppState::ScheduleView => return self.view_schedule(),
            _ => {}
        }

        let main_view = self.view_main();

        // This logic was for overlay, but SettingsView is now full screen or we can keep it overlay.
        // The original code used a stack/overlay approach for Settings.
        // Let's migrate to a clearer state matching, consistent with my proposed changes.
        // Actually, looking at lines 647-651, settings was overlay.
        // User requested "dialog".
        // Let's stick to full view switching for Schedule as per my plan implementation,
        // unless I want to overlay it. Let's overlay it like settings if settings was overlay.
        // Line 647 says: if self.state == AppState::SettingsView { stack![main_view, self.view_settings()] }
        // So I should follow that pattern.

        let root = match self.state {
            AppState::SettingsView => stack![main_view, self.view_settings()].into(),
            AppState::ScheduleView => stack![main_view, self.view_schedule()].into(),
            _ => main_view,
        };

        root
    }

    fn view_main(&self) -> Element<'_, Message> {
        // Menu Bar
        let config_btn = button("Config").on_press(Message::ToggleConfigMenu);
        let menu_bar = row![config_btn, button("Help").on_press(Message::NoOp)]
            .padding(5)
            .spacing(10);

        // Status Indicator
        let status_color = if self.is_connected {
            iced::Color::from_rgb(0.0, 0.8, 0.0) // Green
        } else {
            iced::Color::from_rgb(0.8, 0.0, 0.0) // Red
        };

        // Toolbar / Breadcrumbs
        let breadcrumb_bar =
            container(
                row![
                    text("Current Folder").size(14),
                    text(&self.current_remote_path)
                        .size(14)
                        .color(iced::Color::from_rgb(0.2, 0.4, 1.0)),
                    horizontal_space(),
                    container(container(horizontal_space()).width(10).height(10).style(
                        move |_| container::Style {
                            background: Some(status_color.into()),
                            border: iced::Border {
                                radius: 5.0.into(),
                                ..Default::default()
                            },
                            ..Default::default()
                        }
                    ))
                    .padding(5)
                ]
                .align_y(iced::Alignment::Center)
                .spacing(10),
            )
            .padding(5)
            .style(style::header_style);

        // Panes
        let pane_grid = pane_grid::PaneGrid::new(&self.panes, |_id, _pane_state, _max_size| {
            let content = match _pane_state {
                PaneState::Queue => self.view_queue(),
                PaneState::Remote => self.view_remote(),
            };
            pane_grid::Content::new(content).style(style::pane_style)
        })
        .on_resize(10, Message::PaneResized);

        // Status Bar
        let total_queued = self.queue_items.len();
        let total_bytes: u64 = self
            .queue_items
            .iter()
            .map(|i| i.size_bytes - i.bytes_downloaded)
            .sum();
        let total_size_str = self.format_bytes(&total_bytes.to_string());

        let scanning_text = if self.is_scanning_queue {
            " | Scanning..."
        } else {
            ""
        };

        let schedule_text = if self.config.schedule.mode != settings::ScheduleMode::None {
            if self.last_schedule_allowed {
                " | Schedule: Running"
            } else {
                " | Schedule: Paused ⏸"
            }
        } else {
            ""
        };

        let status_text = format!(
            "{}Total Queued: {} ({}){}{}",
            if self.status_message.is_empty() {
                String::new()
            } else {
                format!("{} | ", self.status_message)
            },
            total_queued,
            total_size_str,
            scanning_text,
            schedule_text
        );

        let status_bar = container(text(status_text).size(12))
            .padding(5)
            .style(style::header_style);

        let base_content = column![
            container(menu_bar).style(style::header_style),
            breadcrumb_bar,
            container(pane_grid)
                .height(Length::Fill)
                .width(Length::Fill),
            status_bar
        ];

        let mut base_content: Element<Message> = base_content.into();

        if self.is_config_menu_open {
            let menu_options = column![
                button("Settings")
                    .on_press(Message::ConfigOptionSelected(ConfigOption::Settings))
                    .width(Length::Fill),
                button("Connect")
                    .on_press(Message::ConfigOptionSelected(ConfigOption::Connect))
                    .width(Length::Fill),
                button("Schedule")
                    .on_press(Message::ConfigOptionSelected(ConfigOption::Schedule))
                    .width(Length::Fill),
                button("Minimize")
                    .on_press(Message::ConfigOptionSelected(ConfigOption::Minimize))
                    .width(Length::Fill),
                button("Disconnect")
                    .on_press(Message::ConfigOptionSelected(ConfigOption::Disconnect))
                    .width(Length::Fill),
                button("Exit")
                    .on_press(Message::ConfigOptionSelected(ConfigOption::Exit))
                    .width(Length::Fill),
            ]
            .width(150)
            .padding(5)
            .spacing(5);

            let menu_overlay = container(container(menu_options).style(style::header_style))
                .padding(iced::Padding {
                    top: 45.0,
                    left: 5.0,
                    bottom: 0.0,
                    right: 0.0,
                });

            // stack![base_content, menu_overlay].into()
            base_content = stack![base_content, menu_overlay].into();
        }

        base_content
    }

    fn view_queue(&self) -> Element<'_, Message> {
        let path_row = row![
            text(format!("Download to: {}", self.config.local_download_path)).size(14),
            horizontal_space(),
            button("Change")
                .on_press(Message::SelectDownloadPath)
                .padding(3)
                .style(button::secondary)
        ]
        .padding(5)
        .align_y(iced::Alignment::Center);

        // Determine button actions based on selected queue item
        let selected = self.selected_queue_item.clone();
        let selected_status = selected.as_ref().and_then(|path| {
            self.queue_items
                .iter()
                .find(|i| &i.remote_file == path)
                .map(|i| i.status.clone())
        });

        let start_btn = if self.is_downloading {
            button(text("Downloading...").size(12)).style(button::secondary)
        } else {
            button(text("Start").size(12))
                .on_press(Message::StartDownloads)
                .style(button::primary)
        };

        let pause_resume_btn = match &selected_status {
            Some(TransferStatus::Downloading) => button(text("Pause").size(12))
                .on_press(Message::PauseDownload(selected.clone().unwrap())),
            Some(TransferStatus::Paused) => button(text("Resume").size(12))
                .on_press(Message::ResumeDownload(selected.clone().unwrap())),
            _ => button(text("Pause").size(12)),
        };

        let remove_btn = if selected.is_some() {
            button(text("Remove").size(12))
                .on_press(Message::CancelDownload(selected.clone().unwrap()))
        } else {
            button(text("Remove").size(12))
        };

        let toolbar = row![
            text("Queue").size(18),
            horizontal_space(),
            start_btn,
            pause_resume_btn,
            remove_btn,
        ]
        .spacing(5)
        .padding(5);

        let headers = components::table_header(vec![
            "Local Location",
            "File name",
            "Remote file",
            "Downloaded",
            "Remaining",
            "Priority",
            "Progress",
        ]);

        let items = column(
            self.queue_items
                .iter()
                .map(|item| {
                    let is_selected = self.selected_queue_item.as_ref() == Some(&item.remote_file);
                    let remote_file = item.remote_file.clone();

                    let row_content = row![
                        container(text(&item.filename).size(12)).width(Length::FillPortion(2)),
                        container(
                            text(self.format_bytes(&item.bytes_downloaded.to_string())).size(12)
                        )
                        .width(Length::FillPortion(1)),
                        container(
                            text(self.format_bytes(
                                &(item.size_bytes - item.bytes_downloaded).to_string()
                            ))
                            .size(12)
                        )
                        .width(Length::FillPortion(1)),
                        container(text(item.priority.to_string()).size(12))
                            .width(Length::FillPortion(1)),
                        container(text(item.status.to_string()).size(12))
                            .width(Length::FillPortion(1)),
                    ]
                    .spacing(5);

                    let btn = button(container(row_content).padding(3))
                        .on_press(Message::QueueItemClicked(remote_file))
                        .width(Length::Fill)
                        .style(move |_theme, _status| {
                            if is_selected {
                                button::Style {
                                    background: Some(iced::Color::from_rgb(0.2, 0.4, 0.7).into()),
                                    text_color: iced::Color::WHITE,
                                    ..Default::default()
                                }
                            } else {
                                button::Style {
                                    text_color: iced::Color::WHITE,
                                    ..button::text(_theme, _status)
                                }
                            }
                        });

                    btn.into()
                })
                .collect::<Vec<_>>(),
        )
        .spacing(2);

        column![path_row, toolbar, headers, scrollable(items)].into()
    }

    fn view_remote(&self) -> Element<'_, Message> {
        let toolbar = row![
            text(format!(
                "Remote: {}, Folder: {}",
                self.config.sftp_config.host, self.current_remote_path
            ))
            .size(16),
            horizontal_space(),
            button("Up")
                .on_press(Message::GoToParent)
                .style(button::secondary)
        ]
        .padding(5)
        .align_y(iced::Alignment::Center);

        let headers = container(
            row![
                container(text("Name").size(14).font(iced::Font {
                    weight: iced::font::Weight::Bold,
                    ..Default::default()
                }))
                .width(Length::FillPortion(2)),
                container(text("Size").size(14).font(iced::Font {
                    weight: iced::font::Weight::Bold,
                    ..Default::default()
                }))
                .width(Length::FillPortion(1)),
                container(text("Type").size(14).font(iced::Font {
                    weight: iced::font::Weight::Bold,
                    ..Default::default()
                }))
                .width(Length::FillPortion(1)),
                container(text("Modified").size(14).font(iced::Font {
                    weight: iced::font::Weight::Bold,
                    ..Default::default()
                }))
                .width(Length::FillPortion(1)),
            ]
            .spacing(5),
        )
        .padding(5)
        .style(style::header_style);

        let items = column(
            self.remote_files
                .iter()
                .map(|file| {
                    let is_folder = file.file_type == FileType::Folder;
                    let icon = if is_folder { "📁" } else { "📄" };
                    let name_text = format!("{} {}", icon, file.name);

                    // Name is just text now, whole row is clickable
                    let name_widget: Element<Message> = text(name_text).size(14).into();

                    let type_str = if is_folder { "Folder" } else { "File" };

                    let is_selected = self.selected_file.as_ref() == Some(&file.name);
                    let is_hovered = self.hovered_file.as_ref() == Some(&file.name);

                    let row_content = row![
                        container(name_widget).width(Length::FillPortion(2)),
                        container(text(&file.size).size(14)).width(Length::FillPortion(1)),
                        container(text(type_str).size(14)).width(Length::FillPortion(1)),
                        container(text(&file.modified).size(14)).width(Length::FillPortion(1)),
                    ]
                    .spacing(5);

                    let main_btn = button(container(row_content).padding(5))
                        .on_press(Message::RemoteFileClicked(file.clone()))
                        .width(Length::Fill)
                        .style(move |_thread, _status| {
                            if is_selected {
                                button::Style {
                                    background: Some(iced::Color::from_rgb(0.2, 0.4, 0.7).into()),
                                    text_color: iced::Color::WHITE,
                                    ..Default::default()
                                }
                            } else {
                                button::Style {
                                    text_color: iced::Color::WHITE,
                                    ..button::text(_thread, _status)
                                }
                            }
                        });

                    let actions = if is_hovered {
                        row![
                            button(text("Queue").size(12))
                                .on_press(Message::QueueFile(file.clone()))
                                .style(button::secondary)
                                .padding(5),
                            button(text("Download").size(12))
                                .on_press(Message::DownloadFile(file.clone()))
                                .style(button::primary)
                                .padding(5),
                        ]
                        .spacing(5)
                        .padding(2)
                    } else {
                        row![].padding(2)
                    };

                    let container_row = row![main_btn, actions].align_y(iced::Alignment::Center);

                    mouse_area(container_row)
                        .on_enter(Message::HoverFile(file.name.clone()))
                        .on_exit(Message::UnhoverFile)
                        .into()
                })
                .collect::<Vec<_>>(),
        )
        .spacing(2);

        let mut content = column![toolbar];
        if let Some(err) = &self.app_error {
            content = content.push(
                container(
                    text(format!("Error: {}", err))
                        .size(14)
                        .color(iced::Color::from_rgb(1.0, 0.5, 0.5)),
                )
                .padding(5)
                .style(|_| container::Style {
                    background: Some(iced::Color::from_rgb(0.2, 0.0, 0.0).into()),
                    ..Default::default()
                }),
            );
        }
        content.push(headers).push(scrollable(items)).into()
    }

    fn view_settings(&self) -> Element<'_, Message> {
        let title = text("Settings").size(24);

        let content = if self.is_checking_connection {
            column![
                title,
                vertical_space().height(20),
                text("Checking connection...").size(18),
            ]
        } else {
            let host_input = text_input("Host", &self.config.sftp_config.host)
                .on_input(Message::HostChanged)
                .padding(10);

            let port_input = text_input("Port", &self.config.sftp_config.port.to_string())
                .on_input(Message::PortChanged)
                .padding(10)
                .width(80);

            let host_row = row![host_input, port_input].spacing(10);

            let user_input = text_input("Username", &self.config.sftp_config.username)
                .on_input(Message::UsernameChanged)
                .padding(10);

            let password_val = self.config.sftp_config.password.clone().unwrap_or_default();
            let pass_input = text_input("Password", &password_val)
                .on_input(Message::PasswordChanged)
                .secure(true)
                .padding(10);

            let controls = row![
                button("Save").on_press(Message::SaveSettings),
                button("Cancel").on_press(Message::CancelSettings),
            ]
            .spacing(20);

            let mut col = column![
                title,
                text("SFTP Connection Details"),
                host_row,
                user_input,
                pass_input,
            ];

            if let Some(err) = &self.settings_error {
                col = col.push(
                    text(format!("Error: {}", err)).color(iced::Color::from_rgb(1.0, 0.0, 0.0)),
                );
            }

            col.push(vertical_space().height(20)).push(controls)
        };

        container(
            container(content.spacing(20).max_width(400))
                .padding(20)
                .style(style::header_style),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .style(|_t: &Theme| container::Style {
            background: Some(iced::Color::from_rgba(0.0, 0.0, 0.0, 0.5).into()),
            ..Default::default()
        })
        .into()
    }

    fn view_schedule(&self) -> Element<'_, Message> {
        let title = text("Download Schedule").size(24);

        let mode_section = column![
            text("Schedule Mode:").size(16),
            radio(
                "None",
                settings::ScheduleMode::None,
                Some(self.config.schedule.mode),
                Message::ScheduleModeChanged
            ),
            radio(
                "Daily",
                settings::ScheduleMode::Daily,
                Some(self.config.schedule.mode),
                Message::ScheduleModeChanged
            ),
            radio(
                "Weekly",
                settings::ScheduleMode::Weekly,
                Some(self.config.schedule.mode),
                Message::ScheduleModeChanged
            ),
        ]
        .spacing(10);

        let mut content = column![title, mode_section].spacing(20).padding(20);

        if self.config.schedule.mode != settings::ScheduleMode::None {
            // Time Pickers
            let format_time = |h: u8, m: u8| -> String {
                let period = if h >= 12 { "PM" } else { "AM" };
                let h12 = if h == 0 || h == 12 { 12 } else { h % 12 };
                format!("{:02}:{:02} {}", h12, m, period)
            };

            let start_time_row = row![
                text("Start Time:").width(100),
                text(format_time(
                    self.config.schedule.start_time.hour,
                    self.config.schedule.start_time.minute
                ))
                .size(16),
                button("+H")
                    .on_press(Message::ScheduleStartTimeChanged(
                        (self.config.schedule.start_time.hour + 1) % 24,
                        self.config.schedule.start_time.minute
                    ))
                    .style(button::secondary),
                button("-H")
                    .on_press(Message::ScheduleStartTimeChanged(
                        (self.config.schedule.start_time.hour + 23) % 24,
                        self.config.schedule.start_time.minute
                    ))
                    .style(button::secondary),
                button("+M")
                    .on_press(Message::ScheduleStartTimeChanged(
                        self.config.schedule.start_time.hour,
                        (self.config.schedule.start_time.minute + 5) % 60
                    ))
                    .style(button::secondary),
                button("-M")
                    .on_press(Message::ScheduleStartTimeChanged(
                        self.config.schedule.start_time.hour,
                        (self.config.schedule.start_time.minute + 55) % 60
                    ))
                    .style(button::secondary),
            ]
            .spacing(10)
            .align_y(iced::Alignment::Center);

            let start_val = self.config.schedule.start_time.hour as u16 * 60
                + self.config.schedule.start_time.minute as u16;
            let end_val = self.config.schedule.end_time.hour as u16 * 60
                + self.config.schedule.end_time.minute as u16;
            let is_next_day = end_val < start_val;

            let end_time_row = row![
                text("End Time:").width(100),
                text(format_time(
                    self.config.schedule.end_time.hour,
                    self.config.schedule.end_time.minute
                ))
                .size(16),
                button("+H")
                    .on_press(Message::ScheduleEndTimeChanged(
                        (self.config.schedule.end_time.hour + 1) % 24,
                        self.config.schedule.end_time.minute
                    ))
                    .style(button::secondary),
                button("-H")
                    .on_press(Message::ScheduleEndTimeChanged(
                        (self.config.schedule.end_time.hour + 23) % 24,
                        self.config.schedule.end_time.minute
                    ))
                    .style(button::secondary),
                button("+M")
                    .on_press(Message::ScheduleEndTimeChanged(
                        self.config.schedule.end_time.hour,
                        (self.config.schedule.end_time.minute + 5) % 60
                    ))
                    .style(button::secondary),
                button("-M")
                    .on_press(Message::ScheduleEndTimeChanged(
                        self.config.schedule.end_time.hour,
                        (self.config.schedule.end_time.minute + 55) % 60
                    ))
                    .style(button::secondary),
                if is_next_day {
                    text("(Next Day)")
                        .size(12)
                        .color(iced::Color::from_rgb(0.6, 0.6, 0.6))
                } else {
                    text("")
                },
            ]
            .spacing(10)
            .align_y(iced::Alignment::Center);

            content = content.push(column![start_time_row, end_time_row].spacing(10));
        }

        if self.config.schedule.mode == settings::ScheduleMode::Weekly {
            let days = &self.config.schedule.days;
            let days_row = row![
                checkbox("Mon", days.mon).on_toggle(|_| Message::ScheduleDayToggled(0)),
                checkbox("Tue", days.tue).on_toggle(|_| Message::ScheduleDayToggled(1)),
                checkbox("Wed", days.wed).on_toggle(|_| Message::ScheduleDayToggled(2)),
                checkbox("Thu", days.thu).on_toggle(|_| Message::ScheduleDayToggled(3)),
                checkbox("Fri", days.fri).on_toggle(|_| Message::ScheduleDayToggled(4)),
                checkbox("Sat", days.sat).on_toggle(|_| Message::ScheduleDayToggled(5)),
                checkbox("Sun", days.sun).on_toggle(|_| Message::ScheduleDayToggled(6)),
            ]
            .spacing(15);

            content = content.push(text("Active Days:")).push(days_row);
        }

        let buttons = row![
            button("Save").on_press(Message::SaveSchedule),
            button("Cancel")
                .on_press(Message::CancelSchedule)
                .style(button::secondary),
        ]
        .spacing(10);

        content = content.push(horizontal_rule(1)).push(buttons);

        container(
            container(content.spacing(20).max_width(600))
                .padding(20)
                .style(style::header_style),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .center_x(Length::Fill)
        .center_y(Length::Fill)
        .style(|_t: &Theme| container::Style {
            background: Some(iced::Color::from_rgba(0.0, 0.0, 0.0, 0.5).into()),
            ..Default::default()
        })
        .into()
    }

    fn format_bytes(&self, size_str: &str) -> String {
        let size = size_str
            .trim()
            .replace(" B", "")
            .parse::<u64>()
            .unwrap_or(0);
        const KB: u64 = 1024;
        const MB: u64 = KB * 1024;
        const GB: u64 = MB * 1024;

        if size >= GB {
            format!("{:.2} GB", size as f64 / GB as f64)
        } else if size >= MB {
            format!("{:.2} MB", size as f64 / MB as f64)
        } else if size >= KB {
            format!("{:.2} KB", size as f64 / KB as f64)
        } else {
            format!("{} B", size)
        }
    }

    fn start_manager(&mut self) -> Task<Message> {
        if self.download_tx.is_none() {
            let (tx, rx) =
                download_manager::create_download_manager(self.config.sftp_config.clone());
            self.download_tx = Some(tx.clone());
            self.download_rx = Some(Arc::new(tokio::sync::Mutex::new(rx)));
            self.is_downloading = true;

            // Send all pending items to the download manager
            for item in &self.queue_items {
                if item.status == TransferStatus::Pending {
                    let _ = tx.try_send(DownloadCommand::AddItem(item.clone()));
                }
            }
            // Removed: If schedule is NOT allowed, we used to pause info.
            // But now we allow manual override, so if start_manager is called (manually or auto),
            // we assume we WANT to download.
            // Tick will handle pausing if schedule changes state.

            let _ = tx.try_send(DownloadCommand::StartAll);

            // Start polling for events
            return self.update(Message::PollDownloadEvents);
        }
        Task::none()
    }

    fn subscription(&self) -> iced::Subscription<Message> {
        let tray_sub = if self.tray_manager.is_some() {
            iced::time::every(std::time::Duration::from_millis(50)).map(|_| {
                // Pump GTK events to keep tray icon alive
                Message::TrayEvent
            })
        } else {
            iced::Subscription::none()
        };

        // Tick every 60 seconds for scheduler
        let tick_sub = iced::time::every(std::time::Duration::from_secs(60)).map(Message::Tick);

        // Listen for window events (CloseRequested)
        let event_sub = iced::event::listen().map(Message::Event);

        iced::Subscription::batch(vec![tray_sub, tick_sub, event_sub])
    }
}
