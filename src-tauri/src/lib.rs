mod db;
mod panel_layout;
mod purge_boundary;
mod settings;
mod title_validation;

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tauri::{
    image::Image,
    menu::{Menu, MenuEvent, MenuId, MenuItem},
    tray::TrayIconBuilder,
    window::Color,
    Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder, WindowEvent,
};
use tauri_plugin_global_shortcut::{Builder as ShortcutBuilder, GlobalShortcutExt, ShortcutState};

/// Set when the user chose "Exit" — process ends after the webview window is destroyed.
static APP_EXITING: AtomicBool = AtomicBool::new(false);
/// Set after `db` is registered via `.manage()` in setup.
static BACKEND_READY: AtomicBool = AtomicBool::new(false);
/// Debounce generation for deferred `settings.json` writes during drag-resize.
static PANEL_WIDTH_SAVE_GEN: AtomicU64 = AtomicU64::new(0);
/// Latest observed panel width from resize events.
static PANEL_WIDTH_LATEST: AtomicU32 = AtomicU32::new(DEFAULT_PANEL_WIDTH);
/// Ensures we run at most one saver thread at a time.
static PANEL_WIDTH_SAVER_RUNNING: AtomicBool = AtomicBool::new(false);

use settings::{AppSettings, DEFAULT_PANEL_WIDTH, SettingsPatch};

use tauri_plugin_global_shortcut::Shortcut;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskList {
    Urgent,
    Daily,
    Weekly,
    Backlog,
}

impl TaskList {
    pub fn as_str(self) -> &'static str {
        match self {
            TaskList::Urgent => "urgent",
            TaskList::Daily => "daily",
            TaskList::Weekly => "weekly",
            TaskList::Backlog => "backlog",
        }
    }

    pub fn parse(s: &str) -> Result<Self, String> {
        match s {
            "urgent" => Ok(TaskList::Urgent),
            "daily" => Ok(TaskList::Daily),
            "weekly" => Ok(TaskList::Weekly),
            "backlog" => Ok(TaskList::Backlog),
            _ => Err(format!("invalid task list: {s}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: i64,
    pub title: String,
    pub done: bool,
    pub completed_at: Option<String>,
    pub position: f64,
    pub created_at: Option<String>,
    pub list: TaskList,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskMoveResult {
    pub task: Task,
    pub rebalanced: bool,
}

fn app_data_dir(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

fn load_app_settings(app: &tauri::AppHandle) -> AppSettings {
    let Ok(dir) = app_data_dir(app) else {
        return AppSettings::default();
    };
    settings::settings_load_from_dir(&dir)
}

fn settings_save(app: &tauri::AppHandle, settings: &AppSettings) -> Result<(), String> {
    let dir = app_data_dir(app)?;
    settings::settings_save_to_dir(&dir, settings)
}

fn run_task_maintenance(app: &tauri::AppHandle, db: &Mutex<Connection>) -> Result<usize, String> {
    let settings = load_app_settings(app);
    let cutoff = purge_boundary::retention_cutoff_date(settings.completed_retention_days);
    let today = purge_boundary::local_today();
    let week_start = purge_boundary::local_week_start();
    let conn = db.lock().map_err(|e| e.to_string())?;
    let purged = db::purge_completed_by_created_date(&conn, &cutoff)?;
    let daily = db::reset_daily_tasks(&conn, &today)?;
    let weekly = db::reset_weekly_tasks(&conn, &week_start)?;
    Ok(purged + daily + weekly)
}

fn spawn_task_maintenance_scheduler(app: tauri::AppHandle) {
    thread::spawn(move || loop {
        if APP_EXITING.load(Ordering::SeqCst) {
            break;
        }
        let hours = load_app_settings(&app)
            .task_update_interval_hours
            .max(settings::MIN_TASK_UPDATE_INTERVAL_HOURS);
        thread::sleep(Duration::from_secs(u64::from(hours) * 3600));
        if APP_EXITING.load(Ordering::SeqCst) {
            break;
        }
        let db = app.state::<Mutex<Connection>>();
        if let Ok(n) = run_task_maintenance(&app, &db) {
            if n > 0 {
                let _ = app.emit("tasks-purged", ());
            }
        }
    });
}

fn register_toggle_shortcut(app: &tauri::AppHandle, hotkey: &str) -> Result<(), String> {
    app.global_shortcut()
        .unregister_all()
        .map_err(|e| e.to_string())?;
    let shortcut: Shortcut = hotkey
        .parse()
        .map_err(|_| format!("invalid hotkey: {hotkey}"))?;
    app.global_shortcut()
        .on_shortcut(shortcut, |app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                toggle_window(app);
            }
        })
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(desktop)]
fn sync_autostart(app: &tauri::AppHandle, enabled: bool) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    if enabled {
        app.autolaunch().enable().map_err(|e| e.to_string())?;
    } else {
        app.autolaunch().disable().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[cfg(not(desktop))]
fn sync_autostart(_app: &tauri::AppHandle, _enabled: bool) -> Result<(), String> {
    Ok(())
}

#[cfg(desktop)]
fn sync_autostart_from_settings(
    app: &tauri::AppHandle,
    settings: &AppSettings,
) -> Result<(), String> {
    use tauri_plugin_autostart::ManagerExt;
    let current = app.autolaunch().is_enabled().map_err(|e| e.to_string())?;
    if current != settings.autostart {
        sync_autostart(app, settings.autostart)?;
    }
    Ok(())
}

#[cfg(not(desktop))]
fn sync_autostart_from_settings(
    _app: &tauri::AppHandle,
    _settings: &AppSettings,
) -> Result<(), String> {
    Ok(())
}

fn apply_settings_side_effects(
    app: &tauri::AppHandle,
    settings: &AppSettings,
    db: &Mutex<Connection>,
    patch: &SettingsPatch,
) -> Result<(), String> {
    if patch.hotkey.is_some() {
        register_toggle_shortcut(app, &settings.hotkey)?;
    }
    if patch.autostart.is_some() {
        sync_autostart(app, settings.autostart)?;
    }
    if patch.completed_retention_days.is_some() {
        let n = run_task_maintenance(app, db)?;
        if n > 0 {
            let _ = app.emit("tasks-purged", ());
        }
    }
    Ok(())
}

#[tauri::command]
fn backend_ready() -> bool {
    BACKEND_READY.load(Ordering::SeqCst)
}

#[tauri::command]
fn tasks_load(
    db: tauri::State<'_, Mutex<Connection>>,
    list: String,
) -> Result<Vec<Task>, String> {
    let list = TaskList::parse(&list)?;
    let conn = db.lock().map_err(|e| e.to_string())?;
    db::list_tasks(&conn, list)
}

#[tauri::command]
fn task_create(
    db: tauri::State<'_, Mutex<Connection>>,
    title: String,
    list: String,
) -> Result<Task, String> {
    let title = title_validation::normalize_task_title(&title)?;
    let list = TaskList::parse(&list)?;
    let conn = db.lock().map_err(|e| e.to_string())?;
    db::create_task(&conn, title, list)
}

#[tauri::command]
fn task_update_title(
    db: tauri::State<'_, Mutex<Connection>>,
    id: i64,
    title: String,
) -> Result<Task, String> {
    let title = title_validation::normalize_task_title(&title)?;
    let conn = db.lock().map_err(|e| e.to_string())?;
    db::update_task_title(&conn, id, title)
}

#[tauri::command]
fn task_set_done(
    db: tauri::State<'_, Mutex<Connection>>,
    id: i64,
    done: bool,
) -> Result<Task, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    db::set_done(&conn, id, done)
}

#[tauri::command]
fn task_delete(db: tauri::State<'_, Mutex<Connection>>, id: i64) -> Result<(), String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    db::delete_task(&conn, id)
}

#[tauri::command]
fn task_move_active(
    db: tauri::State<'_, Mutex<Connection>>,
    id: i64,
    new_index: usize,
) -> Result<TaskMoveResult, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    let result = db::move_active_to_index(&conn, id, new_index)?;
    Ok(TaskMoveResult {
        task: result.task,
        rebalanced: result.rebalanced,
    })
}

#[tauri::command]
fn task_move_list(
    db: tauri::State<'_, Mutex<Connection>>,
    id: i64,
    list: String,
) -> Result<Task, String> {
    let target = TaskList::parse(&list)?;
    let conn = db.lock().map_err(|e| e.to_string())?;
    db::move_task_to_list(&conn, id, target)
}

const PANEL_APP_BG: Color = Color(20, 22, 28, 255);

fn apply_panel_window_chrome(win: &WebviewWindow) {
    let _ = win.set_background_color(Some(PANEL_APP_BG));
}

/// Keeps the panel glued to the right edge without changing its size.
fn anchor_panel_right(win: &WebviewWindow) -> Result<(), String> {
    let monitor = pick_monitor(win)?;
    let wa = monitor.work_area();
    let size = win.outer_size().map_err(|e| e.to_string())?;
    let pos = win.outer_position().map_err(|e| e.to_string())?;
    let target_right = wa.position.x + wa.size.width as i32;
    let current_right = pos.x + size.width as i32;
    let x = panel_layout::panel_anchor_x(wa.position.x, wa.size.width, size.width);
    let y = wa.position.y;

    if current_right == target_right && pos.y == y {
        return Ok(());
    }

    win.set_position(PhysicalPosition::new(x, y))
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn place_panel_window(win: &WebviewWindow, app: &tauri::AppHandle) -> Result<(), String> {
    let monitor = pick_monitor(win)?;
    let wa = monitor.work_area();
    let width = load_app_settings(app).panel_width;
    let x = panel_layout::panel_place_x(wa.position.x, wa.size.width, width);
    let y = wa.position.y;
    let height = wa.size.height.max(200);

    win.set_position(PhysicalPosition::new(x, y))
        .map_err(|e| e.to_string())?;
    win.set_size(PhysicalSize::new(width, height))
        .map_err(|e| e.to_string())?;
    win.set_always_on_top(true).map_err(|e| e.to_string())?;
    win.set_skip_taskbar(true).map_err(|e| e.to_string())?;
    Ok(())
}

fn defer_save_panel_width(app: tauri::AppHandle, width: u32) {
    PANEL_WIDTH_LATEST.store(width, Ordering::Relaxed);
    PANEL_WIDTH_SAVE_GEN.fetch_add(1, Ordering::SeqCst);

    if PANEL_WIDTH_SAVER_RUNNING.swap(true, Ordering::SeqCst) {
        return;
    }

    let app_for_thread = app.clone();
    thread::spawn(move || {
        loop {
            let gen_before = PANEL_WIDTH_SAVE_GEN.load(Ordering::SeqCst);
            thread::sleep(Duration::from_millis(350));
            let gen_after = PANEL_WIDTH_SAVE_GEN.load(Ordering::SeqCst);
            if gen_before != gen_after {
                continue;
            }

            let latest = PANEL_WIDTH_LATEST.load(Ordering::Relaxed) as u32;
            let mut settings = load_app_settings(&app_for_thread);
            settings.panel_width = latest;
            let _ = settings_save(&app_for_thread, &settings);
            break;
        }
        PANEL_WIDTH_SAVER_RUNNING.store(false, Ordering::SeqCst);
    });
}

fn on_panel_resized(win: &WebviewWindow, app: &tauri::AppHandle) {
    let Ok(size) = win.outer_size() else {
        return;
    };
    // Do NOT call `set_size` here — on Windows it can loop Resized events and hang the UI.
    defer_save_panel_width(app.clone(), size.width);
    apply_panel_window_chrome(win);
    let _ = anchor_panel_right(win);
}

/// Prefer monitor containing the window; fallback cursor, then primary.
fn pick_monitor(win: &WebviewWindow) -> Result<tauri::Monitor, String> {
    let pos = win.outer_position().map_err(|e| e.to_string())?;
    let size = win.outer_size().map_err(|e| e.to_string())?;
    let center_x = pos.x + size.width as i32 / 2;
    let center_y = pos.y + size.height as i32 / 2;
    if let Ok(Some(m)) = win.monitor_from_point(center_x as f64, center_y as f64) {
        return Ok(m);
    }

    if let Ok(cursor) = win.cursor_position() {
        if let Ok(Some(m)) = win.monitor_from_point(cursor.x, cursor.y) {
            return Ok(m);
        }
    }

    win.primary_monitor()
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "no primary monitor".to_string())
}

fn toggle_window(app: &tauri::AppHandle) {
    let Some(win) = app.get_webview_window("main") else {
        return;
    };
    let visible = win.is_visible().unwrap_or(true);
    if visible {
        let _ = win.hide();
    } else {
        let _ = place_panel_window(&win, app);
        let _ = win.show();
        let _ = win.set_focus();
    }
}

#[tauri::command]
fn panel_toggle(app: tauri::AppHandle) {
    toggle_window(&app);
}

#[tauri::command]
fn settings_load(app: tauri::AppHandle) -> Result<AppSettings, String> {
    Ok(load_app_settings(&app))
}

#[tauri::command]
fn settings_set(
    app: tauri::AppHandle,
    patch: SettingsPatch,
    db: tauri::State<'_, Mutex<Connection>>,
) -> Result<AppSettings, String> {
    let current = load_app_settings(&app);
    let updated = settings::apply_patch(current, patch.clone())?;
    settings_save(&app, &updated)?;
    apply_settings_side_effects(&app, &updated, &db, &patch)?;
    let _ = app.emit("settings-changed", &updated);
    Ok(updated)
}

fn attach_settings_close_handler(win: &WebviewWindow) {
    let win_for_close = win.clone();
    let _ = win.on_window_event(move |ev| {
        if let WindowEvent::CloseRequested { api, .. } = ev {
            api.prevent_close();
            let _ = win_for_close.hide();
        }
    });
}

fn create_settings_window(app: &tauri::AppHandle) -> Result<WebviewWindow, String> {
    let win = WebviewWindowBuilder::new(app, "settings", WebviewUrl::default())
        .title("Настройки")
        .inner_size(400.0, 520.0)
        .resizable(false)
        .decorations(true)
        .center()
        .visible(false)
        .build()
        .map_err(|e| e.to_string())?;
    attach_settings_close_handler(&win);
    Ok(win)
}

fn ensure_settings_window(app: &tauri::AppHandle) -> Result<WebviewWindow, String> {
    if let Some(win) = app.get_webview_window("settings") {
        return Ok(win);
    }
    create_settings_window(app)
}

#[tauri::command]
fn settings_open(app: tauri::AppHandle) -> Result<(), String> {
    let win = ensure_settings_window(&app)?;
    win.center().map_err(|e| e.to_string())?;
    win.show().map_err(|e| e.to_string())?;
    win.set_focus().map_err(|e| e.to_string())?;
    Ok(())
}

fn show_window(app: &tauri::AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = place_panel_window(&win, app);
        let _ = win.show();
        let _ = win.set_focus();
    }
}

fn hide_window(app: &tauri::AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.hide();
    }
}

fn quit_application(app: &tauri::AppHandle) {
    APP_EXITING.store(true, Ordering::SeqCst);

    let _ = app.global_shortcut().unregister_all();

    if let Some(win) = app.get_webview_window("main") {
        // destroy() (not close()) — close is intercepted and only hides the panel
        if win.destroy().is_err() {
            app.exit(0);
        }
    } else {
        app.exit(0);
    }
}

fn handle_tray_menu(app: &tauri::AppHandle, event: MenuEvent) {
    if MenuId::new("tray_show") == event.id() {
        show_window(app);
    } else if MenuId::new("tray_hide") == event.id() {
        hide_window(app);
    } else if MenuId::new("tray_quit") == event.id() {
        quit_application(app);
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let shortcut_plugin = ShortcutBuilder::new().build();

    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .plugin(shortcut_plugin)
        .setup(|app| {
            let handle = app.handle().clone();
            let data_dir = app_data_dir(&handle)?;
            let conn = db::open(&data_dir)?;
            handle.manage(Mutex::new(conn));
            BACKEND_READY.store(true, Ordering::SeqCst);
            let _ = handle.emit("app-ready", ());

            let settings = load_app_settings(&handle);
            register_toggle_shortcut(&handle, &settings.hotkey)?;
            sync_autostart_from_settings(&handle, &settings)?;

            let db = handle.state::<Mutex<Connection>>();
            let _ = run_task_maintenance(&handle, &db);
            spawn_task_maintenance_scheduler(handle.clone());

            let win = app.get_webview_window("main").expect("main webview window");
            place_panel_window(&win, &handle).expect("place panel");
            apply_panel_window_chrome(&win);

            if let Some(settings_win) = app.get_webview_window("settings") {
                attach_settings_close_handler(&settings_win);
            }

            let app_for_destroy = handle.clone();
            let win_for_close = win.clone();
            let app_for_resize = handle.clone();
            let _ = win.on_window_event(move |ev| match ev {
                WindowEvent::CloseRequested { api, .. } => {
                    api.prevent_close();
                    let _ = win_for_close.hide();
                }
                WindowEvent::Resized(_) => {
                    on_panel_resized(&win_for_close, &app_for_resize);
                }
                WindowEvent::Destroyed => {
                    if APP_EXITING.load(Ordering::SeqCst) {
                        app_for_destroy.exit(0);
                    }
                }
                _ => {}
            });

            let icon =
                Image::from_bytes(include_bytes!("../icons/32x32.png")).expect("tray icon decode");

            let tray_show =
                MenuItem::with_id(&handle, "tray_show", "Показать", true, None::<&str>)?;
            let tray_hide = MenuItem::with_id(&handle, "tray_hide", "Скрыть", true, None::<&str>)?;
            let tray_quit = MenuItem::with_id(&handle, "tray_quit", "Выход", true, None::<&str>)?;
            let menu = Menu::with_items(&handle, &[&tray_show, &tray_hide, &tray_quit])?;

            TrayIconBuilder::new()
                .icon(icon)
                .tooltip("Focus Task Tracker")
                .menu(&menu)
                .show_menu_on_left_click(true)
                .on_menu_event(move |app, e| handle_tray_menu(app, e))
                .build(&handle)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            backend_ready,
            tasks_load,
            task_create,
            task_update_title,
            task_set_done,
            task_delete,
            task_move_active,
            task_move_list,
            panel_toggle,
            settings_load,
            settings_set,
            settings_open
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
