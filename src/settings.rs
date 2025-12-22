use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub sftp_config: SftpConfig,
    pub download_threshold: u8, // 0-100%
    pub local_download_path: String,
    pub schedule: ScheduleConfig,
    #[serde(default)]
    pub last_remote_path: String,
    #[serde(default)]
    pub auto_connect: bool,
    #[serde(default)]
    pub max_download_speed: u64, // KB/s, 0 = unlimited
    #[serde(default)]
    pub download_stats: Vec<DailyStat>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyStat {
    pub date: String, // YYYY-MM-DD
    pub bytes_downloaded: u64,
    pub seconds_active: u64,
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
            last_remote_path: ".".to_string(),
            auto_connect: false,
            max_download_speed: 0,
            download_stats: Vec::new(),
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

    pub fn get_today_stat(&mut self) -> &mut DailyStat {
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        if self.download_stats.is_empty() || self.download_stats.last().unwrap().date != today {
            self.download_stats.push(DailyStat {
                date: today,
                bytes_downloaded: 0,
                seconds_active: 0,
            });
        }
        self.download_stats.last_mut().unwrap()
    }

    pub fn add_daily_stat(&mut self, bytes: u64, seconds: u64) {
        let stat = self.get_today_stat();
        stat.bytes_downloaded += bytes;
        stat.seconds_active += seconds;
    }

    pub fn get_weekly_average(&self) -> u64 {
        self.get_average_speed(7)
    }

    pub fn get_monthly_average(&self) -> u64 {
        self.get_average_speed(30)
    }

    fn get_average_speed(&self, days: usize) -> u64 {
        if self.download_stats.is_empty() {
            return 0;
        }
        let start_idx = self.download_stats.len().saturating_sub(days);
        let stats = &self.download_stats[start_idx..];

        let total_bytes: u64 = stats.iter().map(|s| s.bytes_downloaded).sum();
        let total_seconds: u64 = stats.iter().map(|s| s.seconds_active).sum();

        if total_seconds == 0 {
            0
        } else {
            total_bytes / total_seconds // Bytes per second
        }
    }
}
