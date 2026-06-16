use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const DEFAULT_PANEL_WIDTH: u32 = 360;
pub const DEFAULT_HOTKEY: &str = "ctrl+shift+space";
pub const MAX_RETENTION_DAYS: u32 = 365;
pub const MIN_TASK_UPDATE_INTERVAL_HOURS: u32 = 1;
pub const MAX_TASK_UPDATE_INTERVAL_HOURS: u32 = 168;
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
    #[serde(default = "default_true")]
    pub show_completed_at: bool,
    #[serde(default = "default_retention_days")]
    pub completed_retention_days: u32,
    #[serde(default = "default_true")]
    pub confirm_task_delete: bool,
    #[serde(
        rename = "task_update_interval_hours",
        alias = "purge_interval_hours",
        default = "default_task_update_interval_hours"
    )]
    pub task_update_interval_hours: u32,
    #[serde(default = "default_true")]
    pub show_completed_tasks: bool,
    #[serde(default = "default_true")]
    pub use_daily: bool,
    #[serde(default = "default_true")]
    pub use_weekly: bool,
    #[serde(default = "default_true")]
    pub use_backlog: bool,
}

#[derive(Debug, Clone, Deserialize, Default, PartialEq)]
pub struct SettingsPatch {
    pub hotkey: Option<String>,
    pub autostart: Option<bool>,
    pub show_created_at: Option<bool>,
    pub show_completed_at: Option<bool>,
    pub completed_retention_days: Option<u32>,
    pub confirm_task_delete: Option<bool>,
    #[serde(alias = "purge_interval_hours")]
    pub task_update_interval_hours: Option<u32>,
    pub show_completed_tasks: Option<bool>,
    pub use_daily: Option<bool>,
    pub use_weekly: Option<bool>,
    pub use_backlog: Option<bool>,
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
    3
}

fn default_task_update_interval_hours() -> u32 {
    6
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            panel_width: DEFAULT_PANEL_WIDTH,
            hotkey: default_hotkey(),
            autostart: false,
            show_created_at: true,
            show_completed_at: true,
            completed_retention_days: 3,
            confirm_task_delete: true,
            task_update_interval_hours: 6,
            show_completed_tasks: true,
            use_daily: true,
            use_weekly: true,
            use_backlog: true,
        }
    }
}

pub fn validate(settings: &AppSettings) -> Result<(), String> {
    if settings.completed_retention_days > MAX_RETENTION_DAYS {
        return Err(format!(
            "completed_retention_days must be <= {MAX_RETENTION_DAYS}"
        ));
    }
    if settings.task_update_interval_hours < MIN_TASK_UPDATE_INTERVAL_HOURS
        || settings.task_update_interval_hours > MAX_TASK_UPDATE_INTERVAL_HOURS
    {
        return Err(format!(
            "task_update_interval_hours must be between {MIN_TASK_UPDATE_INTERVAL_HOURS} and {MAX_TASK_UPDATE_INTERVAL_HOURS}"
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
    if let Some(show_completed_at) = patch.show_completed_at {
        settings.show_completed_at = show_completed_at;
    }
    if let Some(completed_retention_days) = patch.completed_retention_days {
        settings.completed_retention_days = completed_retention_days;
    }
    if let Some(confirm_task_delete) = patch.confirm_task_delete {
        settings.confirm_task_delete = confirm_task_delete;
    }
    if let Some(task_update_interval_hours) = patch.task_update_interval_hours {
        settings.task_update_interval_hours = task_update_interval_hours;
    }
    if let Some(show_completed_tasks) = patch.show_completed_tasks {
        settings.show_completed_tasks = show_completed_tasks;
    }
    if let Some(use_daily) = patch.use_daily {
        settings.use_daily = use_daily;
    }
    if let Some(use_weekly) = patch.use_weekly {
        settings.use_weekly = use_weekly;
    }
    if let Some(use_backlog) = patch.use_backlog {
        settings.use_backlog = use_backlog;
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
            show_completed_at: false,
            completed_retention_days: 3,
            confirm_task_delete: false,
            task_update_interval_hours: 12,
            show_completed_tasks: false,
            use_daily: false,
            use_weekly: false,
            use_backlog: false,
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
        assert!(loaded.show_completed_at);
        assert_eq!(loaded.completed_retention_days, 3);
        assert_eq!(loaded.task_update_interval_hours, 6);
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
    fn validate_task_update_interval_hours() {
        let mut s = AppSettings::default();
        s.task_update_interval_hours = 1;
        assert!(validate(&s).is_ok());
        s.task_update_interval_hours = 168;
        assert!(validate(&s).is_ok());
        s.task_update_interval_hours = 0;
        assert!(validate(&s).is_err());
        s.task_update_interval_hours = 169;
        assert!(validate(&s).is_err());
    }

    #[test]
    fn settings_reads_legacy_purge_interval_alias() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(SETTINGS_FILE),
            r#"{"purge_interval_hours": 12}"#,
        )
        .unwrap();
        let loaded = settings_load_from_dir(dir.path());
        assert_eq!(loaded.task_update_interval_hours, 12);
    }

    #[test]
    fn apply_patch_merges_fields() {
        let base = AppSettings::default();
        let merged = apply_patch(
            base,
            SettingsPatch {
                show_created_at: Some(false),
                show_completed_at: Some(false),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(!merged.show_created_at);
        assert!(!merged.show_completed_at);
        assert_eq!(merged.hotkey, DEFAULT_HOTKEY);
    }

    #[test]
    fn use_backlog_defaults_true() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(SETTINGS_FILE),
            r#"{"panel_width": 400}"#,
        )
        .unwrap();
        let loaded = settings_load_from_dir(dir.path());
        assert!(loaded.use_backlog);
    }

    #[test]
    fn use_daily_defaults_true() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(SETTINGS_FILE),
            r#"{"panel_width": 400}"#,
        )
        .unwrap();
        let loaded = settings_load_from_dir(dir.path());
        assert!(loaded.use_daily);
    }

    #[test]
    fn use_weekly_defaults_true() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join(SETTINGS_FILE),
            r#"{"panel_width": 400}"#,
        )
        .unwrap();
        let loaded = settings_load_from_dir(dir.path());
        assert!(loaded.use_weekly);
    }

    #[test]
    fn apply_patch_use_daily() {
        let merged = apply_patch(
            AppSettings::default(),
            SettingsPatch {
                use_daily: Some(false),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(!merged.use_daily);
    }

    #[test]
    fn apply_patch_use_weekly() {
        let merged = apply_patch(
            AppSettings::default(),
            SettingsPatch {
                use_weekly: Some(false),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(!merged.use_weekly);
    }

    #[test]
    fn apply_patch_use_backlog() {
        let merged = apply_patch(
            AppSettings::default(),
            SettingsPatch {
                use_backlog: Some(false),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(!merged.use_backlog);
    }
}
