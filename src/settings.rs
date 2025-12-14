use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub sftp_config: SftpConfig,
    pub download_threshold: u8, // 0-100%
    pub local_download_path: String,
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
