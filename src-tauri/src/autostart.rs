const APP_NAME: &str = "SHU_AutoNet";
use std::path::Path;

#[cfg(windows)]
use winreg::enums::{HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE};
#[cfg(windows)]
use winreg::RegKey;

pub struct AutoStartManager;

impl AutoStartManager {
    fn command_for_exe(exe: &Path, launch_minimized: bool) -> String {
        let suffix = if launch_minimized { " --minimized" } else { "" };
        format!("\"{}\"{}", exe.to_string_lossy(), suffix)
    }

    fn command_matches_exe(command: &str, exe: &Path) -> bool {
        [false, true].into_iter().any(|minimized| {
            command
                .trim()
                .eq_ignore_ascii_case(&Self::command_for_exe(exe, minimized))
        })
    }
    /// Returns current executable path wrapped with quotes and optional flags
    pub fn get_launch_command(launch_minimized: bool) -> Result<String, String> {
        let exe = std::env::current_exe()
            .map_err(|e| format!("Unable to determine current executable path: {}", e))?;
        Ok(Self::command_for_exe(&exe, launch_minimized))
    }

    #[cfg(windows)]
    pub fn is_enabled() -> bool {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        if let Ok(run_key) = hkcu.open_subkey_with_flags(
            r"Software\Microsoft\Windows\CurrentVersion\Run",
            KEY_QUERY_VALUE,
        ) {
            run_key
                .get_value::<String, _>(APP_NAME)
                .ok()
                .zip(std::env::current_exe().ok())
                .is_some_and(|(command, exe)| Self::command_matches_exe(&command, &exe))
        } else {
            false
        }
    }

    #[cfg(not(windows))]
    pub fn is_enabled() -> bool {
        false
    }

    #[cfg(windows)]
    pub fn apply(enabled: bool, launch_minimized: bool) -> Result<(), String> {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        let run_key = hkcu
            .open_subkey_with_flags(
                r"Software\Microsoft\Windows\CurrentVersion\Run",
                KEY_SET_VALUE | KEY_QUERY_VALUE,
            )
            .map_err(|e| format!("Failed to open registry Run key: {}", e))?;

        if enabled {
            let cmd = Self::get_launch_command(launch_minimized)?;
            run_key
                .set_value(APP_NAME, &cmd)
                .map_err(|e| format!("Failed to set registry auto-start: {}", e))?;
        } else {
            // A portable copy may have a separate startup preference. Do not
            // remove it when the installed copy has never enabled autostart.
            if Self::is_enabled() {
                run_key
                    .delete_value(APP_NAME)
                    .map_err(|e| format!("Failed to disable auto-start: {}", e))?;
            }
        }

        Ok(())
    }

    #[cfg(not(windows))]
    pub fn apply(_enabled: bool, _launch_minimized: bool) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn startup_commands_quote_paths_and_keep_deployments_separate() {
        let installed = Path::new(r"C:\Users\User\AppData\Local\SHU AutoNet\shu-autonet.exe");
        let portable = Path::new(r"E:\Portable Apps\SHU_AutoNet.exe");
        let command = AutoStartManager::command_for_exe(installed, true);
        assert!(command.starts_with('"') && command.ends_with("\" --minimized"));
        assert!(AutoStartManager::command_matches_exe(&command, installed));
        assert!(!AutoStartManager::command_matches_exe(&command, portable));
        assert!(AutoStartManager::command_matches_exe(
            &command.to_uppercase(),
            installed
        ));
    }
}
