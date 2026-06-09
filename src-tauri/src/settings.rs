use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const DEFAULT_PANEL_WIDTH: u32 = 360;
pub const DEFAULT_HOTKEY: &str = "ctrl+shift+space";
pub const MAX_RETENTION_DAYS: u32 = 365;
pub const MIN_PURGE_INTERVAL_HOURS: u32 = 1;
pub const MAX_PURGE_INTERVAL_HOURS: u32 = 168;
pub const SETTINGS_FILE: &str = "settings.json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppSettings {
    #[serde(default = "default_panel_width")]
    pub panel_width: u32,
    #[serde(default = "default_hotkey")]
    pub hotkey: String,
    #[serde(default)]
    pub autostart: bool,
    #[serde(default = "default_true")]
    pub show_created_at: bool,
    #[serde(default = "default_retention_days")]
    pub completed_retention_days: u32,
    #[serde(default = "default_true")]
    pub confirm_task_delete: bool,
    #[serde(default = "default_purge_interval_hours")]
    pub purge_interval_hours: u32,
    #[serde(default = "default_true")]
    pub show_completed_tasks: bool,
}

#[derive(Debug, Clone, Deserialize, Default, PartialEq)]
pub struct SettingsPatch {
    pub hotkey: Option<String>,
    pub autostart: Option<bool>,
    pub show_created_at: Option<bool>,
    pub completed_retention_days: Option<u32>,
    pub confirm_task_delete: Option<bool>,
    pub purge_interval_hours: Option<u32>,
    pub show_completed_tasks: Option<bool>,
}

fn default_panel_width() -> u32 {
    DEFAULT_PANEL_WIDTH
}

fn default_hotkey() -> String {
    DEFAULT_HOTKEY.to_string()
}

fn default_true() -> bool {
    true
}

fn default_retention_days() -> u32 {
    1
}

fn default_purge_interval_hours() -> u32 {
    6
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            panel_width: DEFAULT_PANEL_WIDTH,
            hotkey: default_hotkey(),
            autostart: false,
            show_created_at: true,
            completed_retention_days: 1,
            confirm_task_delete: true,
            purge_interval_hours: 6,
            show_completed_tasks: true,
        }
    }
}

pub fn validate(settings: &AppSettings) -> Result<(), String> {
    if settings.completed_retention_days > MAX_RETENTION_DAYS {
        return Err(format!(
            "completed_retention_days must be <= {MAX_RETENTION_DAYS}"
        ));
    }
    if settings.purge_interval_hours < MIN_PURGE_INTERVAL_HOURS
        || settings.purge_interval_hours > MAX_PURGE_INTERVAL_HOURS
    {
        return Err(format!(
            "purge_interval_hours must be between {MIN_PURGE_INTERVAL_HOURS} and {MAX_PURGE_INTERVAL_HOURS}"
        ));
    }
    if settings.hotkey.trim().is_empty() {
        return Err("hotkey must not be empty".to_string());
    }
    Ok(())
}

pub fn apply_patch(mut settings: AppSettings, patch: SettingsPatch) -> Result<AppSettings, String> {
    if let Some(hotkey) = patch.hotkey {
        settings.hotkey = hotkey;
    }
    if let Some(autostart) = patch.autostart {
        settings.autostart = autostart;
    }
    if let Some(show_created_at) = patch.show_created_at {
        settings.show_created_at = show_created_at;
    }
    if let Some(completed_retention_days) = patch.completed_retention_days {
        settings.completed_retention_days = completed_retention_days;
    }
    if let Some(confirm_task_delete) = patch.confirm_task_delete {
        settings.confirm_task_delete = confirm_task_delete;
    }
    if let Some(purge_interval_hours) = patch.purge_interval_hours {
        settings.purge_interval_hours = purge_interval_hours;
    }
    if let Some(show_completed_tasks) = patch.show_completed_tasks {
        settings.show_completed_tasks = show_completed_tasks;
    }
    validate(&settings)?;
    Ok(settings)
}

pub fn settings_file_path(data_dir: &Path) -> PathBuf {
    data_dir.join(SETTINGS_FILE)
}

pub fn settings_load_from_dir(data_dir: &Path) -> AppSettings {
    let path = settings_file_path(data_dir);
    if !path.exists() {
        return AppSettings::default();
    }
    let Ok(raw) = std::fs::read_to_string(&path) else {
        return AppSettings::default();
    };
    serde_json::from_str(&raw).unwrap_or_default()
}

pub fn settings_save_to_dir(data_dir: &Path, settings: &AppSettings) -> Result<(), String> {
    validate(settings)?;
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
        assert_eq!(s, AppSettings::default());
    }

    #[test]
    fn settings_roundtrip_all_fields() {
        let dir = tempfile::tempdir().unwrap();
        let written = AppSettings {
            panel_width: 420,
            hotkey: "ctrl+alt+k".to_string(),
            autostart: true,
            show_created_at: false,
            completed_retention_days: 3,
            confirm_task_delete: false,
            purge_interval_hours: 12,
            show_completed_tasks: false,
        };
        settings_save_to_dir(dir.path(), &written).unwrap();
        let loaded = settings_load_from_dir(dir.path());
        assert_eq!(loaded, written);
    }

    #[test]
    fn settings_partial_json_uses_defaults() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(SETTINGS_FILE),
            r#"{"panel_width": 400}"#,
        )
        .unwrap();
        let loaded = settings_load_from_dir(dir.path());
        assert_eq!(loaded.panel_width, 400);
        assert_eq!(loaded.hotkey, DEFAULT_HOTKEY);
        assert!(loaded.show_created_at);
        assert_eq!(loaded.completed_retention_days, 1);
        assert_eq!(loaded.purge_interval_hours, 6);
    }

    #[test]
    fn validate_retention_days() {
        let mut s = AppSettings::default();
        s.completed_retention_days = 0;
        assert!(validate(&s).is_ok());
        s.completed_retention_days = 365;
        assert!(validate(&s).is_ok());
        s.completed_retention_days = 366;
        assert!(validate(&s).is_err());
    }

    #[test]
    fn validate_purge_interval_hours() {
        let mut s = AppSettings::default();
        s.purge_interval_hours = 1;
        assert!(validate(&s).is_ok());
        s.purge_interval_hours = 168;
        assert!(validate(&s).is_ok());
        s.purge_interval_hours = 0;
        assert!(validate(&s).is_err());
        s.purge_interval_hours = 169;
        assert!(validate(&s).is_err());
    }

    #[test]
    fn apply_patch_merges_fields() {
        let base = AppSettings::default();
        let merged = apply_patch(
            base,
            SettingsPatch {
                show_created_at: Some(false),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(!merged.show_created_at);
        assert_eq!(merged.hotkey, DEFAULT_HOTKEY);
    }
}
