use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub sftp_config: SftpConfig,
    pub download_threshold: u8, // 0-100%
    pub local_download_path: String,
    pub schedule: ScheduleConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        let local_download_path = directories::UserDirs::new()
            .and_then(|dirs| dirs.download_dir().map(|p| p.to_string_lossy().to_string()))
            .unwrap_or_else(|| ".".to_string());

        Self {
            sftp_config: SftpConfig::default(),
            download_threshold: 0,
            local_download_path,
            schedule: ScheduleConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SftpConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    pub password: Option<String>,
    pub private_key_path: Option<String>,
}

impl Default for SftpConfig {
    fn default() -> Self {
        Self {
            host: String::from("localhost"),
            port: 22,
            username: String::new(),
            password: None,
            private_key_path: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ScheduleMode {
    None,
    Daily,
    Weekly,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TimeOfDay {
    pub hour: u8,
    pub minute: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeekDays {
    pub mon: bool,
    pub tue: bool,
    pub wed: bool,
    pub thu: bool,
    pub fri: bool,
    pub sat: bool,
    pub sun: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScheduleConfig {
    pub mode: ScheduleMode,
    pub start_time: TimeOfDay,
    pub end_time: TimeOfDay,
    pub days: WeekDays,
}

impl Default for ScheduleConfig {
    fn default() -> Self {
        Self {
            mode: ScheduleMode::None,
            start_time: TimeOfDay { hour: 0, minute: 0 },
            end_time: TimeOfDay { hour: 6, minute: 0 },
            days: WeekDays {
                mon: false,
                tue: false,
                wed: false,
                thu: false,
                fri: false,
                sat: false,
                sun: false,
            },
        }
    }
}

impl AppConfig {
    pub fn load() -> Self {
        if let Ok(content) = std::fs::read_to_string("config.json") {
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    pub fn save(&self) -> std::io::Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write("config.json", content)
    }
}
