use serde::{Deserialize, Deserializer, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

const INSTALL_MARKER: &str = "installed.flag";
const APP_IDENTIFIER: &str = "com.shu.autonet";

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

    #[serde(
        default = "default_port",
        deserialize_with = "deserialize_string_or_number"
    )]
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

    #[serde(
        default = "default_offset",
        deserialize_with = "deserialize_usize_or_string"
    )]
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
    fn config_path_for(exe_dir: &Path, user_config_dir: Option<&Path>) -> PathBuf {
        if exe_dir.join(INSTALL_MARKER).is_file() {
            if let Some(root) = user_config_dir {
                return root.join(APP_IDENTIFIER).join("config.json");
            }
        }
        exe_dir.join("config.json")
    }

    pub fn is_portable() -> bool {
        std::env::current_exe()
            .ok()
            .and_then(|path| path.parent().map(|dir| !dir.join(INSTALL_MARKER).is_file()))
            .unwrap_or(true)
    }

    /// Installed copies use per-user application data. Unmarked portable copies
    /// retain their config beside the EXE, including existing v1.0.0/1.0.1 folders.
    pub fn get_config_path() -> PathBuf {
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
        Self::config_path_for(&exe_dir, dirs::config_dir().as_deref())
    }

    /// Loads the configuration file. Falls back to default if file is missing or corrupted.
    pub fn load() -> AppConfig {
        let path = Self::get_config_path();
        Self::load_from_path(&path)
    }

    fn load_from_path(path: &Path) -> AppConfig {
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
        Self::save_to_path(config, &path)
    }

    fn save_to_path(config: &AppConfig, path: &Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Cannot create config directory: {}", e))?;
        }
        let content = serde_json::to_string_pretty(config)
            .map_err(|e| format!("Serialization error: {}", e))?;
        fs::write(&path, content).map_err(|e| format!("Write failed: {}", e))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_dir() -> PathBuf {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "shu-autonet-config-{}-{}",
            std::process::id(),
            unique
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    fn cleanup(root: PathBuf) {
        let resolved = root.canonicalize().unwrap();
        assert!(resolved.starts_with(std::env::temp_dir().canonicalize().unwrap()));
        assert!(root
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("shu-autonet-config-"));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn portable_and_legacy_copies_keep_their_existing_config() {
        let root = test_dir();
        let exe_dir = root.join("portable");
        fs::create_dir(&exe_dir).unwrap();
        let path = ConfigManager::config_path_for(&exe_dir, Some(&root.join("user-data")));
        assert_eq!(path, exe_dir.join("config.json"));
        let cfg = AppConfig {
            user_id: "test-student".into(),
            ..AppConfig::default()
        };
        ConfigManager::save_to_path(&cfg, &path).unwrap();
        assert_eq!(ConfigManager::load_from_path(&path).user_id, "test-student");
        cleanup(root);
    }

    #[test]
    fn installed_config_is_writable_in_user_data_and_preserved_on_reinstall() {
        let root = test_dir();
        let exe_dir = root.join("installed");
        let user_dir = root.join("user-data");
        fs::create_dir(&exe_dir).unwrap();
        fs::write(exe_dir.join(INSTALL_MARKER), "installed").unwrap();
        let path = ConfigManager::config_path_for(&exe_dir, Some(&user_dir));
        assert_eq!(path, user_dir.join(APP_IDENTIFIER).join("config.json"));
        let cfg = AppConfig {
            theme: "light".into(),
            ..AppConfig::default()
        };
        ConfigManager::save_to_path(&cfg, &path).unwrap();
        fs::write(exe_dir.join(INSTALL_MARKER), "updated installer").unwrap();
        assert_eq!(ConfigManager::load_from_path(&path).theme, "light");
        assert!(!exe_dir.join("config.json").exists());
        cleanup(root);
    }
}
