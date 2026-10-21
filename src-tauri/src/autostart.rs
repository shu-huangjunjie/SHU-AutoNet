const APP_NAME: &str = "SHU_AutoNet";

#[cfg(windows)]
use winreg::enums::{HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE};
#[cfg(windows)]
use winreg::RegKey;

pub struct AutoStartManager;

impl AutoStartManager {
    /// Returns current executable path wrapped with quotes and optional flags
    pub fn get_launch_command(launch_minimized: bool) -> Result<String, String> {
        let exe = std::env::current_exe()
            .map_err(|e| format!("Unable to determine current executable path: {}", e))?;
        let exe_str = exe.to_string_lossy();

        if launch_minimized {
            Ok(format!("\"{}\" --minimized", exe_str))
        } else {
            Ok(format!("\"{}\"", exe_str))
        }
    }

    #[cfg(windows)]
    pub fn is_enabled() -> bool {
        let hkcu = RegKey::predef(HKEY_CURRENT_USER);
        if let Ok(run_key) = hkcu.open_subkey_with_flags(
            r"Software\Microsoft\Windows\CurrentVersion\Run",
            KEY_QUERY_VALUE,
        ) {
            run_key.get_value::<String, _>(APP_NAME).is_ok()
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
            let _ = run_key.delete_value(APP_NAME);
        }

        Ok(())
    }

    #[cfg(not(windows))]
    pub fn apply(_enabled: bool, _launch_minimized: bool) -> Result<(), String> {
        Ok(())
    }
}
