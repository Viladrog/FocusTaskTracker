use std::path::Path;

use rusqlite::{params, Connection, Row};

use crate::Task;

const DB_FILE: &str = "tasks.db";
const POSITION_EPS: f64 = 1e-9;

pub fn open(data_dir: &Path) -> Result<Connection, String> {
    let path = data_dir.join(DB_FILE);
    let conn = Connection::open(path).map_err(|e| e.to_string())?;
    conn.execute_batch(
        "PRAGMA journal_mode=WAL;
         CREATE TABLE IF NOT EXISTS tasks (
             id    INTEGER PRIMARY KEY AUTOINCREMENT,
             title TEXT NOT NULL,
             done  INTEGER NOT NULL CHECK (done IN (0, 1))
         );",
    )
    .map_err(|e| e.to_string())?;
    migrate(&conn)?;
    Ok(conn)
}

fn column_exists(conn: &Connection, name: &str) -> Result<bool, String> {
    let names = conn
        .prepare("PRAGMA table_info(tasks)")
        .map_err(|e| e.to_string())?
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| e.to_string())?
        .filter_map(|n| n.ok())
        .collect::<Vec<_>>();
    Ok(names.iter().any(|n| n == name))
}

fn migrate(conn: &Connection) -> Result<(), String> {
    if !column_exists(conn, "completed_at")? {
        conn.execute("ALTER TABLE tasks ADD COLUMN completed_at TEXT", [])
            .map_err(|e| e.to_string())?;
    }

    conn.execute(
        "UPDATE tasks SET completed_at = datetime('now') WHERE done = 1 AND completed_at IS NULL",
        [],
    )
    .map_err(|e| e.to_string())?;

    if !column_exists(conn, "position")? {
        conn.execute(
            "ALTER TABLE tasks ADD COLUMN position REAL NOT NULL DEFAULT 0",
            [],
        )
        .map_err(|e| e.to_string())?;
        conn.execute(
            "UPDATE tasks SET position = CAST(id AS REAL)",
            [],
        )
        .map_err(|e| e.to_string())?;
    }

    Ok(())
}

fn row_to_task(row: &Row<'_>) -> rusqlite::Result<Task> {
    Ok(Task {
        id: row.get(0)?,
        title: row.get(1)?,
        done: row.get::<_, i32>(2)? != 0,
        completed_at: row.get(3)?,
        position: row.get(4)?,
    })
}

pub fn list_tasks(conn: &Connection) -> Result<Vec<Task>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id, title, done, completed_at, position
             FROM tasks
             ORDER BY done ASC,
                      CASE WHEN done = 0 THEN position ELSE 0 END DESC,
                      CASE WHEN done = 1 THEN completed_at ELSE '' END DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], row_to_task)
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

pub fn fetch_task(conn: &Connection, id: i64) -> Result<Task, String> {
    conn.query_row(
        "SELECT id, title, done, completed_at, position FROM tasks WHERE id = ?1",
        params![id],
        row_to_task,
    )
    .map_err(|e| e.to_string())
}

fn list_active_ids_by_position(conn: &Connection) -> Result<Vec<i64>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT id FROM tasks WHERE done = 0 ORDER BY position DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

pub fn rebalance_active_positions(conn: &Connection) -> Result<(), String> {
    let ids = list_active_ids_by_position(conn)?;
    let tx = conn
        .unchecked_transaction()
        .map_err(|e| e.to_string())?;
    let n = ids.len() as f64;
    for (i, id) in ids.iter().enumerate() {
        let pos = n - i as f64;
        tx.execute(
            "UPDATE tasks SET position = ?1 WHERE id = ?2",
            params![pos, id],
        )
        .map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    Ok(())
}

fn assign_position_between(before: Option<f64>, after: Option<f64>) -> f64 {
    match (before, after) {
        (None, None) => 1.0,
        (None, Some(a)) => a + 1.0,
        (Some(b), None) => b - 1.0,
        (Some(b), Some(a)) => (b + a) / 2.0,
    }
}

fn needs_rebalance(before: f64, after: f64, new_pos: f64) -> bool {
    (after - before).abs() < POSITION_EPS || new_pos == before || new_pos == after
}

pub struct MoveActiveResult {
    pub task: Task,
    /// True when `rebalance_active_positions` ran (all active `position` values changed).
    pub rebalanced: bool,
}

/// Moves an active task to `new_index` among active tasks sorted by `position DESC`.
pub fn move_active_to_index(
    conn: &Connection,
    id: i64,
    new_index: usize,
) -> Result<MoveActiveResult, String> {
    let task = fetch_task(conn, id)?;
    if task.done {
        return Err("task is not active".to_string());
    }

    let mut rebalanced = false;

    for _ in 0..3 {
        let actives: Vec<Task> = conn
            .prepare(
                "SELECT id, title, done, completed_at, position
                 FROM tasks WHERE done = 0 ORDER BY position DESC",
            )
            .map_err(|e| e.to_string())?
            .query_map([], row_to_task)
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;

        let without: Vec<&Task> = actives.iter().filter(|t| t.id != id).collect();
        let idx = new_index.min(without.len());

        let before_pos = if idx > 0 {
            Some(without[idx - 1].position)
        } else {
            None
        };
        let after_pos = if idx < without.len() {
            Some(without[idx].position)
        } else {
            None
        };

        let new_pos = assign_position_between(before_pos, after_pos);

        if let (Some(b), Some(a)) = (before_pos, after_pos) {
            if needs_rebalance(b, a, new_pos) {
                rebalance_active_positions(conn)?;
                rebalanced = true;
                continue;
            }
        }

        conn.execute(
            "UPDATE tasks SET position = ?1 WHERE id = ?2",
            params![new_pos, id],
        )
        .map_err(|e| e.to_string())?;

        return Ok(MoveActiveResult {
            task: fetch_task(conn, id)?,
            rebalanced,
        });
    }

    Err("failed to assign position after rebalance".to_string())
}

pub fn create_task(conn: &Connection, title: &str) -> Result<Task, String> {
    conn.execute("INSERT INTO tasks (title, done, position) VALUES (?1, 0, 0)", params![title])
        .map_err(|e| e.to_string())?;
    let id = conn.last_insert_rowid();
    conn.execute(
        "UPDATE tasks SET position = CAST(?1 AS REAL) WHERE id = ?1",
        params![id],
    )
    .map_err(|e| e.to_string())?;
    fetch_task(conn, id)
}

pub fn set_done(conn: &Connection, id: i64, done: bool) -> Result<Task, String> {
    let n = if done {
        conn.execute(
            "UPDATE tasks SET done = 1, completed_at = datetime('now') WHERE id = ?1",
            params![id],
        )
    } else {
        conn.execute(
            "UPDATE tasks SET done = 0, completed_at = NULL WHERE id = ?1",
            params![id],
        )
    }
    .map_err(|e| e.to_string())?;

    if n == 0 {
        return Err("task not found".to_string());
    }

    fetch_task(conn, id)
}

pub fn delete_task(conn: &Connection, id: i64) -> Result<(), String> {
    let n = conn
        .execute("DELETE FROM tasks WHERE id = ?1", params![id])
        .map_err(|e| e.to_string())?;
    if n == 0 {
        return Err("task not found".to_string());
    }
    Ok(())
}

/// Removes completed tasks with `completed_at` strictly before `boundary`.
/// `boundary` must be `"YYYY-MM-DD HH:MM:SS"` (UTC), matching SQLite `datetime('now')`.
pub fn purge_completed_before(conn: &Connection, boundary: &str) -> Result<usize, String> {
    conn.execute(
        "DELETE FROM tasks
         WHERE done = 1
           AND completed_at IS NOT NULL
           AND completed_at < ?1",
        params![boundary],
    )
    .map_err(|e| e.to_string())?;
    Ok(conn.changes() as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE tasks (
                 id    INTEGER PRIMARY KEY AUTOINCREMENT,
                 title TEXT NOT NULL,
                 done  INTEGER NOT NULL CHECK (done IN (0, 1)),
                 completed_at TEXT
             );",
        )
        .unwrap();
        migrate(&conn).unwrap();
        conn
    }

    #[test]
    fn set_done_sets_and_clears_completed_at() {
        let conn = test_conn();
        let task = create_task(&conn, "test").unwrap();
        assert!(task.completed_at.is_none());
        assert_eq!(task.position, task.id as f64);

        let done = set_done(&conn, task.id, true).unwrap();
        assert!(done.done);
        assert!(done.completed_at.is_some());
        assert_eq!(done.position, task.position);

        let undone = set_done(&conn, task.id, false).unwrap();
        assert!(!undone.done);
        assert!(undone.completed_at.is_none());
        assert_eq!(undone.position, task.position);
    }

    #[test]
    fn create_assigns_position_equal_to_id() {
        let conn = test_conn();
        let a = create_task(&conn, "a").unwrap();
        let b = create_task(&conn, "b").unwrap();
        assert_eq!(a.position, a.id as f64);
        assert_eq!(b.position, b.id as f64);
        let list = list_tasks(&conn).unwrap();
        assert_eq!(list[0].id, b.id);
        assert_eq!(list[1].id, a.id);
    }

    #[test]
    fn list_orders_active_before_done() {
        let conn = test_conn();
        let active = create_task(&conn, "active").unwrap();
        let mut done = create_task(&conn, "done").unwrap();
        done = set_done(&conn, done.id, true).unwrap();
        let list = list_tasks(&conn).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].id, active.id);
        assert_eq!(list[1].id, done.id);
    }

    #[test]
    fn move_active_changes_order() {
        let conn = test_conn();
        let a = create_task(&conn, "a").unwrap();
        let b = create_task(&conn, "b").unwrap();
        move_active_to_index(&conn, a.id, 0)
            .unwrap()
            .task;
        let list = list_tasks(&conn).unwrap();
        assert_eq!(list[0].id, a.id);
        assert_eq!(list[1].id, b.id);
    }

    fn insert_task(conn: &Connection, title: &str, done: bool, completed_at: Option<&str>) {
        conn.execute(
            "INSERT INTO tasks (title, done, completed_at, position) VALUES (?1, ?2, ?3, 0)",
            params![title, if done { 1 } else { 0 }, completed_at],
        )
        .unwrap();
        let id = conn.last_insert_rowid();
        conn.execute(
            "UPDATE tasks SET position = CAST(?1 AS REAL) WHERE id = ?1",
            params![id],
        )
        .unwrap();
    }

    #[test]
    fn purge_removes_old_completed() {
        let conn = test_conn();
        insert_task(&conn, "old", true, Some("2026-05-29 10:00:00"));
        let deleted = purge_completed_before(&conn, "2026-05-30 00:00:00").unwrap();
        assert_eq!(deleted, 1);
        assert_eq!(list_tasks(&conn).unwrap().len(), 0);
    }

    #[test]
    fn purge_keeps_today_completed() {
        let conn = test_conn();
        insert_task(&conn, "today", true, Some("2026-05-30 08:00:00"));
        let deleted = purge_completed_before(&conn, "2026-05-30 00:00:00").unwrap();
        assert_eq!(deleted, 0);
        assert_eq!(list_tasks(&conn).unwrap().len(), 1);
    }

    #[test]
    fn purge_keeps_active() {
        let conn = test_conn();
        insert_task(&conn, "active", false, None);
        let deleted = purge_completed_before(&conn, "2026-05-30 00:00:00").unwrap();
        assert_eq!(deleted, 0);
        assert_eq!(list_tasks(&conn).unwrap().len(), 1);
    }

    #[test]
    fn purge_skips_null_completed_at() {
        let conn = test_conn();
        insert_task(&conn, "no date", true, None);
        let deleted = purge_completed_before(&conn, "2026-05-30 00:00:00").unwrap();
        assert_eq!(deleted, 0);
        assert_eq!(list_tasks(&conn).unwrap().len(), 1);
    }
}
