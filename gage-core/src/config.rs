use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// Returns the gage home directory.
///
/// If `GAGE_HOME` is set, uses that value. Otherwise uses `$HOME/.gage`.
pub fn gage_home() -> PathBuf {
    if let Ok(home) = env::var("GAGE_HOME") {
        PathBuf::from(home)
    } else {
        let home = env::var("HOME").expect("HOME environment variable not set");
        PathBuf::from(home).join(".gage")
    }
}

/// Returns the default settings file path.
pub fn default_settings_path() -> PathBuf {
    gage_home().join("settings.json")
}

/// Returns the plugin marketplace directory: `~/.gage/.plugin-marketplace`.
///
/// This is the long-lived location where `gage init` stages the Claude
/// Code plugin marketplace registered with `claude plugin marketplace add`.
pub fn plugin_marketplace_dir() -> PathBuf {
    gage_home().join(".plugin-marketplace")
}

/// Returns a display-friendly settings path.
///
/// Uses `~` in place of `$HOME` when `GAGE_HOME` is not set.
/// When `GAGE_HOME` is set, returns the literal resolved path.
pub fn display_settings_path() -> String {
    if env::var("GAGE_HOME").is_ok() {
        default_settings_path().to_string_lossy().into_owned()
    } else {
        "~/.gage/settings.json".to_string()
    }
}

/// Contents of `~/.gage/settings.json`.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Settings {
    #[serde(default)]
    pub scanners: ScannerSettings,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ScannerSettings {
    /// When non-empty, only these scanners are enabled. Otherwise all
    /// scanners not in `disable` are enabled.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub enable: Vec<String>,

    /// Scanners that are explicitly disabled. Takes precedence over
    /// `enable`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub disable: Vec<String>,
}

impl Settings {
    /// Load settings from the default path. Returns defaults if the
    /// file does not exist.
    pub fn load() -> io::Result<Self> {
        Self::load_from(&default_settings_path())
    }

    pub fn load_from(path: &Path) -> io::Result<Self> {
        match fs::read_to_string(path) {
            Ok(s) => {
                let settings: Settings = serde_json::from_str(&s).map_err(io::Error::other)?;
                Ok(settings)
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Settings::default()),
            Err(e) => Err(e),
        }
    }

    pub fn save(&self) -> io::Result<()> {
        self.save_to(&default_settings_path())
    }

    pub fn save_to(&self, path: &Path) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self).map_err(io::Error::other)?;
        fs::write(path, format!("{json}\n"))
    }

    /// True if the named scanner is enabled per these settings.
    ///
    /// A scanner is enabled when (a) `enable` is empty or contains the
    /// name, and (b) `disable` does not contain the name.
    pub fn is_scanner_enabled(&self, name: &str) -> bool {
        if self.scanners.disable.iter().any(|n| n == name) {
            return false;
        }
        if self.scanners.enable.is_empty() {
            return true;
        }
        self.scanners.enable.iter().any(|n| n == name)
    }
}
