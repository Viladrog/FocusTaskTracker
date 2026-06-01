use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const DEFAULT_PANEL_WIDTH: u32 = 360;
pub const SETTINGS_FILE: &str = "settings.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    #[serde(default = "default_panel_width")]
    pub panel_width: u32,
}

fn default_panel_width() -> u32 {
    DEFAULT_PANEL_WIDTH
}

pub fn settings_file_path(data_dir: &Path) -> PathBuf {
    data_dir.join(SETTINGS_FILE)
}

pub fn settings_load_from_dir(data_dir: &Path) -> AppSettings {
    let path = settings_file_path(data_dir);
    if !path.exists() {
        return AppSettings {
            panel_width: DEFAULT_PANEL_WIDTH,
        };
    }
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return AppSettings {
            panel_width: DEFAULT_PANEL_WIDTH,
        };
    };
    serde_json::from_str(&raw).unwrap_or(AppSettings {
        panel_width: DEFAULT_PANEL_WIDTH,
    })
}

pub fn settings_save_to_dir(data_dir: &Path, settings: &AppSettings) -> Result<(), String> {
    std::fs::create_dir_all(data_dir).map_err(|e| e.to_string())?;
    let path = settings_file_path(data_dir);
    let raw = serde_json::to_string_pretty(settings).map_err(|e| e.to_string())?;
    std::fs::write(&path, raw).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_default_when_missing() {
        let dir = tempfile::tempdir().unwrap();
        let s = settings_load_from_dir(dir.path());
        assert_eq!(s.panel_width, DEFAULT_PANEL_WIDTH);
    }

    #[test]
    fn settings_roundtrip_panel_width() {
        let dir = tempfile::tempdir().unwrap();
        let written = AppSettings { panel_width: 420 };
        settings_save_to_dir(dir.path(), &written).unwrap();
        let loaded = settings_load_from_dir(dir.path());
        assert_eq!(loaded.panel_width, 420);
    }
}
