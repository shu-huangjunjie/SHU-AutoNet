use serde::{Deserialize, Deserializer, Serialize};
use std::fs;
use std::path::PathBuf;

fn deserialize_string_or_number<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringOrNum {
        String(String),
        Number(i64),
    }

    match StringOrNum::deserialize(deserializer)? {
        StringOrNum::String(s) => Ok(s),
        StringOrNum::Number(n) => Ok(n.to_string()),
    }
}

fn deserialize_usize_or_string<'de, D>(deserializer: D) -> Result<usize, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum UsizeOrString {
        Number(usize),
        String(String),
    }

    match UsizeOrString::deserialize(deserializer)? {
        UsizeOrString::Number(n) => Ok(n),
        UsizeOrString::String(s) => s.trim().parse::<usize>().map_err(serde::de::Error::custom),
    }
}

fn default_server() -> String {
    "10.10.9.9".to_string()
}
fn default_port() -> String {
    "8080".to_string()
}
fn default_test_url() -> String {
    "http://123.123.123.123/".to_string()
}
fn default_start_token() -> String {
    "index.jsp?".to_string()
}
fn default_offset() -> usize {
    10
}
fn default_end_token() -> String {
    "'</script>".to_string()
}
fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub user_id: String,

    #[serde(default)]
    pub password: String,

    #[serde(default = "default_server")]
    pub auth_server: String,

    #[serde(default = "default_port", deserialize_with = "deserialize_string_or_number")]
    pub auth_port: String,

    #[serde(default = "default_test_url")]
    pub test_url: String,

    #[serde(default = "default_true")]
    pub minimize_to_tray: bool,

    #[serde(default = "default_true")]
    pub close_to_tray: bool,

    #[serde(default)]
    pub start_with_windows: bool,

    #[serde(default)]
    pub launch_minimized: bool,

    #[serde(default = "default_start_token")]
    pub parse_start_token: String,

    #[serde(default = "default_offset", deserialize_with = "deserialize_usize_or_string")]
    pub parse_start_offset: usize,

    #[serde(default = "default_end_token")]
    pub parse_end_token: String,

    #[serde(default = "default_theme")]
    pub theme: String,
}

fn default_theme() -> String {
    "dark".to_string()
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            user_id: String::new(),
            password: String::new(),
            auth_server: default_server(),
            auth_port: default_port(),
            test_url: default_test_url(),
            minimize_to_tray: true,
            close_to_tray: true,
            start_with_windows: false,
            launch_minimized: false,
            parse_start_token: default_start_token(),
            parse_start_offset: default_offset(),
            parse_end_token: default_end_token(),
            theme: default_theme(),
        }
    }
}

pub struct ConfigManager;

impl ConfigManager {
    /// Returns the absolute path of `config.json` relative to the current executable directory.
    /// This ensures 100% portability across different drives, folders, and machines.
    pub fn get_config_path() -> PathBuf {
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
        exe_dir.join("config.json")
    }

    /// Loads the configuration file. Falls back to default if file is missing or corrupted.
    pub fn load() -> AppConfig {
        let path = Self::get_config_path();
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(config) = serde_json::from_str::<AppConfig>(&content) {
                    return config;
                }
            }
        }
        AppConfig::default()
    }

    /// Saves the configuration file with pretty-printed JSON.
    pub fn save(config: &AppConfig) -> Result<(), String> {
        let path = Self::get_config_path();
        let content = serde_json::to_string_pretty(config)
            .map_err(|e| format!("Serialization error: {}", e))?;
        fs::write(&path, content).map_err(|e| format!("Write failed: {}", e))?;
        Ok(())
    }
}
