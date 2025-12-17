mod components;
mod download_manager;
mod mock_data;
mod settings;
mod sftp_client;
mod style;

use download_manager::{DownloadCommand, DownloadEvent};
use iced::widget::{
    button, column, container, horizontal_space, mouse_area, pane_grid, row, scrollable, stack,
    text, text_input, vertical_space,
};
use iced::{Element, Length, Task, Theme};
use mock_data::{FileType, QueueItem, RemoteFile, TransferStatus};
use settings::AppConfig;
use sftp_client::SftpClient;

use std::sync::{Arc, Mutex};
use std::time::Instant;
use tokio::sync::mpsc;

pub fn main() -> iced::Result {
    iced::application("Simple SFTP", SftpApp::update, SftpApp::view)
        .theme(|_| Theme::Dark)
        .run()
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
    // Context Menu
    context_menu: Option<RemoteFile>,
    is_scanning_queue: bool,
    // Download Manager
    download_tx: Option<mpsc::Sender<DownloadCommand>>,
    download_rx: Option<Arc<tokio::sync::Mutex<mpsc::Receiver<DownloadEvent>>>>,
    is_downloading: bool,
    selected_queue_item: Option<String>,
}

#[derive(Debug, Clone)]
enum PaneState {
    Queue,
    Remote,
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
            queue_items: Vec::new(),
            remote_files: Vec::new(),
            current_remote_path: ".".into(), // Start at home/current directory
            context_menu: None,
            is_scanning_queue: false,
            download_tx: None,
            download_rx: None,
            is_downloading: false,
            selected_queue_item: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum AppState {
    MainView,
    SettingsView,
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
    // Context Menu
    RemoteFileRightClicked(RemoteFile),
    ContextOptionSelected(ContextOption),
    ScanResult(Result<Vec<RemoteFile>, String>),
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
    // Toolbar
    NoOp,
}

#[derive(Debug, Clone)]
enum ConfigOption {
    Settings,
    Connect,
    Minimize,
    Disconnect,
    Exit,
}

#[derive(Debug, Clone, Copy)]
enum ContextOption {
    Download,
    Dismiss,
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
                        if !self.config.sftp_config.host.is_empty() {
                            self.is_checking_connection = true;
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
                    ConfigOption::Disconnect => {
                        self.is_connected = false;
                        self.sftp_client = None;
                        self.remote_files.clear();
                    }
                    ConfigOption::Exit => return iced::exit(),
                    _ => {}
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

                        // Trigger file listing
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
            Message::RemoteFileRightClicked(file) => {
                // Removed point
                self.context_menu = Some(file);
            }
            Message::ContextOptionSelected(option) => {
                if let Some(file) = self.context_menu.take() {
                    // Removed point from destructuring
                    match option {
                        ContextOption::Download => {
                            if let Some(client) = &self.sftp_client {
                                let client = client.clone();
                                self.is_scanning_queue = true;

                                return Task::future(async move {
                                    let path = std::path::Path::new(&file.path).to_path_buf();

                                    let res = tokio::task::spawn_blocking(move || {
                                        let c = client.lock().unwrap();
                                        if file.file_type == FileType::Folder {
                                            c.recursive_scan(&path)
                                        } else {
                                            Ok(vec![file])
                                        }
                                    })
                                    .await
                                    .unwrap_or_else(|e| Err(e.to_string()));

                                    Message::ScanResult(res)
                                });
                            }
                        }
                        ContextOption::Dismiss => {
                            // Do nothing, just close the menu
                        }
                    }
                }
            }
            Message::ScanResult(result) => {
                self.is_scanning_queue = false;
                match result {
                    Ok(files) => {
                        for file in files {
                            if !self.queue_items.iter().any(|i| i.remote_file == file.path) {
                                self.queue_items.push(QueueItem {
                                    local_location: self.config.local_download_path.clone(),
                                    filename: file.name,
                                    remote_file: file.path,
                                    size_bytes: file.size_bytes,
                                    bytes_downloaded: 0,
                                    priority: 10,
                                    status: TransferStatus::Pending,
                                });
                            }
                        }
                    }
                    Err(e) => {
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
                    let _ = tx.try_send(DownloadCommand::StartAll);

                    // Start polling for events
                    return self.update(Message::PollDownloadEvents);
                }
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
                }
            }
            Message::ResumeDownload(path) => {
                if let Some(tx) = &self.download_tx {
                    let _ = tx.try_send(DownloadCommand::Resume(path.clone()));
                }
                if let Some(item) = self.queue_items.iter_mut().find(|i| i.remote_file == path) {
                    item.status = TransferStatus::Downloading;
                }
            }
            Message::CancelDownload(path) => {
                if let Some(tx) = &self.download_tx {
                    let _ = tx.try_send(DownloadCommand::Cancel(path.clone()));
                }
                self.queue_items.retain(|i| i.remote_file != path);
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
                // Continue polling for more events
                return self.update(Message::PollDownloadEvents);
            }
            Message::QueueItemClicked(path) => {
                self.selected_queue_item = Some(path);
            }
            _ => {}
        }
        Task::none()
    }

    fn view(&self) -> Element<'_, Message> {
        let main_view = self.view_main();

        let root = if self.state == AppState::SettingsView {
            stack![main_view, self.view_settings()].into()
        } else {
            main_view
        };

        if let Some(file) = &self.context_menu {
            // Updated to directly use file
            let menu = container(
                column![
                    text(format!("Selected: {}", file.name)).size(14),
                    self.context_menu_view()
                ]
                .spacing(10)
                .padding(10),
            )
            .style(style::pane_style) // Use a styled container for the menu box
            .width(Length::Shrink)
            .height(Length::Shrink)
            .padding(20);

            // Backdrop to close menu
            let backdrop = button(text(" "))
                .on_press(Message::ContextOptionSelected(ContextOption::Dismiss)) // Changed to Dismiss
                .style(button::text) // Transparent-ish
                .width(Length::Fill)
                .height(Length::Fill);

            stack![
                root,
                backdrop,
                container(menu)
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .align_x(iced::Alignment::Center)
                    .align_y(iced::Alignment::Center)
            ]
            .into()
        } else {
            root
        }
    }

    fn context_menu_view(&self) -> Element<'_, Message> {
        container(
            button(text("Download").size(14))
                .on_press(Message::ContextOptionSelected(ContextOption::Download))
                .padding(5)
                .style(button::secondary),
        )
        .padding(5)
        .style(style::header_style)
        .into()
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
        let status_text = format!(
            "Total Queued: {} ({}){}",
            total_queued, total_size_str, scanning_text
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

        if self.is_config_menu_open {
            let menu_options = column![
                button("Settings")
                    .on_press(Message::ConfigOptionSelected(ConfigOption::Settings))
                    .width(Length::Fill),
                button("Connect")
                    .on_press(Message::ConfigOptionSelected(ConfigOption::Connect))
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

            stack![base_content, menu_overlay].into()
        } else {
            base_content.into()
        }
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

                    let row_content = row![
                        container(name_widget).width(Length::FillPortion(2)),
                        container(text(&file.size).size(14)).width(Length::FillPortion(1)),
                        container(text(type_str).size(14)).width(Length::FillPortion(1)),
                        container(text(&file.modified).size(14)).width(Length::FillPortion(1)),
                    ]
                    .spacing(5);

                    let btn = button(container(row_content).padding(5))
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

                    mouse_area(btn)
                        .on_right_press(Message::RemoteFileRightClicked(file.clone())) // Removed closure and point
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
}
