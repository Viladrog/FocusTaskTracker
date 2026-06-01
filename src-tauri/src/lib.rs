mod db;
mod panel_layout;
mod purge_boundary;
mod settings;
mod title_validation;

use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Mutex;
use std::thread;
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use tauri::{
    image::Image,
    menu::{Menu, MenuEvent, MenuId, MenuItem},
    tray::TrayIconBuilder,
    Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewWindow, WindowEvent,
};
use rusqlite::Connection;
use tauri_plugin_global_shortcut::{
    Builder as ShortcutBuilder, GlobalShortcutExt, ShortcutState,
};

/// Set when the user chose "Exit" — process ends after the webview window is destroyed.
static APP_EXITING: AtomicBool = AtomicBool::new(false);
/// Prevents nested `Resized` handling when we call `set_position` / `set_size`.
static PANEL_RESIZE_BUSY: AtomicBool = AtomicBool::new(false);
/// Debounce generation for deferred `settings.json` writes during drag-resize.
static PANEL_WIDTH_SAVE_GEN: AtomicU64 = AtomicU64::new(0);
/// Latest observed (clamped) panel width from resize events.
static PANEL_WIDTH_LATEST: AtomicU32 = AtomicU32::new(DEFAULT_PANEL_WIDTH);
/// Ensures we run at most one saver thread at a time.
static PANEL_WIDTH_SAVER_RUNNING: AtomicBool = AtomicBool::new(false);
/// Throttle high-frequency debug logs.
static PANEL_DEBUG_LAST_LOG_MS: AtomicU64 = AtomicU64::new(0);
/// Counts resize events for correlation.
static PANEL_DEBUG_RESIZE_SEQ: AtomicU64 = AtomicU64::new(0);

use settings::{AppSettings, DEFAULT_PANEL_WIDTH};

const PURGE_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: i64,
    pub title: String,
    pub done: bool,
    pub completed_at: Option<String>,
    pub position: f64,
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

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn panel_log_path(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    Ok(app_data_dir(app)?.join("panel-debug.log"))
}

fn panel_log_line(app: &tauri::AppHandle, line: &str) {
    let Ok(path) = panel_log_path(app) else {
        return;
    };
    let stamp = now_ms();
    let raw = format!("[{stamp}] {line}\n");
    let _ = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .and_then(|mut f| std::io::Write::write_all(&mut f, raw.as_bytes()));
}

fn panel_log_throttled(app: &tauri::AppHandle, line: &str) {
    let ms = now_ms();
    let last = PANEL_DEBUG_LAST_LOG_MS.load(Ordering::Relaxed);
    if ms.saturating_sub(last) < 120 {
        return;
    }
    PANEL_DEBUG_LAST_LOG_MS.store(ms, Ordering::Relaxed);
    panel_log_line(app, line);
}

fn settings_load(app: &tauri::AppHandle) -> AppSettings {
    let Ok(dir) = app_data_dir(app) else {
        return AppSettings {
            panel_width: DEFAULT_PANEL_WIDTH,
        };
    };
    settings::settings_load_from_dir(&dir)
}

fn settings_save(app: &tauri::AppHandle, settings: &AppSettings) -> Result<(), String> {
    let dir = app_data_dir(app)?;
    settings::settings_save_to_dir(&dir, settings)
}

fn purge_completed_tasks(db: &Mutex<Connection>) -> Result<usize, String> {
    let boundary = purge_boundary::today_local_midnight_boundary_utc();
    let conn = db.lock().map_err(|e| e.to_string())?;
    db::purge_completed_before(&conn, &boundary)
}

fn spawn_purge_scheduler(app: tauri::AppHandle) {
    thread::spawn(move || {
        loop {
            thread::sleep(PURGE_INTERVAL);
            if APP_EXITING.load(Ordering::SeqCst) {
                break;
            }
            let db = app.state::<Mutex<Connection>>();
            if let Ok(n) = purge_completed_tasks(&db) {
                if n > 0 {
                    let _ = app.emit("tasks-purged", ());
                }
            }
        }
    });
}

#[tauri::command]
fn tasks_load(db: tauri::State<'_, Mutex<Connection>>) -> Result<Vec<Task>, String> {
    let conn = db.lock().map_err(|e| e.to_string())?;
    db::list_tasks(&conn)
}

#[tauri::command]
fn task_create(
    db: tauri::State<'_, Mutex<Connection>>,
    title: String,
) -> Result<Task, String> {
    let title = title_validation::normalize_task_title(&title)?;
    let conn = db.lock().map_err(|e| e.to_string())?;
    db::create_task(&conn, title)
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
fn reposition_panel(app: tauri::AppHandle) -> Result<(), String> {
    let win = app
        .get_webview_window("main")
        .ok_or_else(|| "window main not found".to_string())?;
    anchor_panel_right(&win)
}

/// Keeps the panel glued to the right edge without changing its size.
fn anchor_panel_right(win: &WebviewWindow) -> Result<(), String> {
    let monitor = pick_monitor(win)?;
    let wa = monitor.work_area();
    let size = win.outer_size().map_err(|e| e.to_string())?;
    let pos = win.outer_position().map_err(|e| e.to_string())?;
    let x = panel_layout::panel_anchor_x(wa.position.x, wa.size.width, size.width);
    let y = wa.position.y;

    if pos.x == x && pos.y == y {
        return Ok(());
    }

    win
        .set_position(PhysicalPosition::new(x, y))
        .map_err(|e| e.to_string())?;
    Ok(())
}

fn place_panel_window(win: &WebviewWindow, app: &tauri::AppHandle) -> Result<(), String> {
    let monitor = pick_monitor(win)?;
    let wa = monitor.work_area();
    let width = settings_load(app).panel_width;
    let x = panel_layout::panel_place_x(wa.position.x, wa.size.width, width);
    let y = wa.position.y;
    let height = wa.size.height.max(200);

    win
        .set_position(PhysicalPosition::new(x, y))
        .map_err(|e| e.to_string())?;
    win
        .set_size(PhysicalSize::new(width, height))
        .map_err(|e| e.to_string())?;
    win.set_always_on_top(true).map_err(|e| e.to_string())?;
    win.set_skip_taskbar(true).map_err(|e| e.to_string())?;
    Ok(())
}

fn defer_save_panel_width(app: tauri::AppHandle, width: u32) {
    PANEL_WIDTH_LATEST.store(width, Ordering::Relaxed);
    let generation = PANEL_WIDTH_SAVE_GEN.fetch_add(1, Ordering::SeqCst) + 1;

    // Start a single saver thread that waits until the generation stabilizes.
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

            // Stable: persist latest width.
            let latest = PANEL_WIDTH_LATEST.load(Ordering::Relaxed) as u32;
            panel_log_line(
                &app_for_thread,
                &format!("save_width stable gen={gen_after} width={latest}"),
            );
            let settings = AppSettings { panel_width: latest };
            let _ = settings_save(&app_for_thread, &settings);
            break;
        }
        PANEL_WIDTH_SAVER_RUNNING.store(false, Ordering::SeqCst);
    });

    // Make it visible in logs that we scheduled a save.
    panel_log_throttled(&app, &format!("save_width scheduled gen={generation} width={width}"));
}

fn on_panel_resized(win: &WebviewWindow, app: &tauri::AppHandle) {
    let seq = PANEL_DEBUG_RESIZE_SEQ.fetch_add(1, Ordering::Relaxed) + 1;

    if PANEL_RESIZE_BUSY.swap(true, Ordering::SeqCst) {
        panel_log_throttled(app, &format!("resize seq={seq} skip busy=true"));
        return;
    }

    let result = (|| {
        let Ok(size) = win.outer_size() else {
            panel_log_line(app, &format!("resize seq={seq} outer_size=ERR"));
            return;
        };
        let pos = win.outer_position().ok();
        // IMPORTANT: Do NOT call `set_size` while handling `Resized`.
        // On some systems this can create a feedback loop (size drift) and hang the UI.
        // We only persist the (clamped) width and keep the panel anchored to the right edge.
        let width = size.width;
        panel_log_throttled(
            app,
            &format!(
                "resize seq={seq} size=({}x{}) pos={} width={}",
                size.width,
                size.height,
                pos.map(|p| format!("({}, {})", p.x, p.y)).unwrap_or_else(|| "ERR".to_string()),
                width
            ),
        );

        defer_save_panel_width(app.clone(), width);
        let _ = anchor_panel_right(win);
    })();

    PANEL_RESIZE_BUSY.store(false, Ordering::SeqCst);
    let _ = result;
}

/// Prefer monitor under cursor (multi-monitor); fallback primary.
fn pick_monitor(win: &WebviewWindow) -> Result<tauri::Monitor, String> {
    let pos = win.cursor_position().map_err(|e| e.to_string())?;
    if let Ok(Some(m)) = win.monitor_from_point(pos.x, pos.y) {
        return Ok(m);
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
    let shortcut_plugin = ShortcutBuilder::new()
        .with_shortcut("ctrl+shift+space")
        .expect("invalid shortcut string")
        .with_handler(|app, _shortcut, event| {
            if event.state == ShortcutState::Pressed {
                toggle_window(app);
            }
        })
        .build();

    tauri::Builder::default()
        .plugin(shortcut_plugin)
        .setup(|app| {
            let handle = app.handle().clone();
            let data_dir = app_data_dir(&handle)?;
            let conn = db::open(&data_dir)?;
            let boundary = purge_boundary::today_local_midnight_boundary_utc();
            let _ = db::purge_completed_before(&conn, &boundary);
            handle.manage(Mutex::new(conn));
            spawn_purge_scheduler(handle.clone());

            let win = app
                .get_webview_window("main")
                .expect("main webview window");
            place_panel_window(&win, &handle).expect("place panel");

            let app_for_destroy = handle.clone();
            let win_for_close = win.clone();
            let app_for_resize = handle.clone();
            let _ = win.on_window_event(move |ev| {
                match ev {
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
                }
            });

            let icon = Image::from_bytes(include_bytes!("../icons/32x32.png"))
                .expect("tray icon decode");

            let tray_show = MenuItem::with_id(&handle, "tray_show", "Показать", true, None::<&str>)?;
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
            tasks_load,
            task_create,
            task_set_done,
            task_delete,
            task_move_active,
            reposition_panel,
            panel_toggle
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
