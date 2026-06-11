use std::path::Path;

use rusqlite::{params, Connection, Row};

use crate::{Task, TaskList};

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
        conn.execute("UPDATE tasks SET position = CAST(id AS REAL)", [])
            .map_err(|e| e.to_string())?;
    }

    if !column_exists(conn, "created_at")? {
        conn.execute("ALTER TABLE tasks ADD COLUMN created_at TEXT", [])
            .map_err(|e| e.to_string())?;
    }

    conn.execute(
        "UPDATE tasks SET created_at = datetime('now') WHERE created_at IS NULL",
        [],
    )
    .map_err(|e| e.to_string())?;

    if !column_exists(conn, "list")? {
        conn.execute(
            "ALTER TABLE tasks ADD COLUMN list TEXT NOT NULL DEFAULT 'urgent'",
            [],
        )
        .map_err(|e| e.to_string())?;
    }

    conn.execute(
        "UPDATE tasks SET list = 'urgent' WHERE list IS NULL OR list = ''",
        [],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

fn row_get_list(row: &Row<'_>, idx: usize) -> rusqlite::Result<TaskList> {
    let s: String = row.get(idx)?;
    TaskList::parse(&s).map_err(|_| rusqlite::Error::InvalidQuery)
}

fn row_to_task(row: &Row<'_>) -> rusqlite::Result<Task> {
    Ok(Task {
        id: row.get(0)?,
        title: row.get(1)?,
        done: row.get::<_, i32>(2)? != 0,
        completed_at: row.get(3)?,
        position: row.get(4)?,
        created_at: row.get(5)?,
        list: row_get_list(row, 6)?,
    })
}

pub fn list_tasks(conn: &Connection, list: TaskList) -> Result<Vec<Task>, String> {
    let list_str = list.as_str();
    let mut stmt = conn
        .prepare(
            "SELECT id, title, done, completed_at, position, created_at, list
             FROM tasks
             WHERE list = ?1
             ORDER BY done ASC,
                      CASE WHEN done = 0 THEN position ELSE 0 END DESC,
                      CASE WHEN done = 1 THEN completed_at ELSE '' END DESC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![list_str], row_to_task)
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

pub fn fetch_task(conn: &Connection, id: i64) -> Result<Task, String> {
    conn.query_row(
        "SELECT id, title, done, completed_at, position, created_at, list FROM tasks WHERE id = ?1",
        params![id],
        row_to_task,
    )
    .map_err(|e| e.to_string())
}

fn list_active_ids_by_position(conn: &Connection, list: TaskList) -> Result<Vec<i64>, String> {
    let list_str = list.as_str();
    let mut stmt = conn
        .prepare("SELECT id FROM tasks WHERE done = 0 AND list = ?1 ORDER BY position DESC")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![list_str], |row| row.get(0))
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

pub fn rebalance_active_positions(conn: &Connection, list: TaskList) -> Result<(), String> {
    let ids = list_active_ids_by_position(conn, list)?;
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
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

pub(crate) fn assign_position_between(before: Option<f64>, after: Option<f64>) -> f64 {
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

#[derive(Debug)]
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

    let list = task.list;

    for _ in 0..3 {
        let actives: Vec<Task> = conn
            .prepare(
                "SELECT id, title, done, completed_at, position, created_at, list
                 FROM tasks WHERE done = 0 AND list = ?1 ORDER BY position DESC",
            )
            .map_err(|e| e.to_string())?
            .query_map(params![list.as_str()], row_to_task)
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
                rebalance_active_positions(conn, list)?;
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

pub fn create_task(conn: &Connection, title: &str, list: TaskList) -> Result<Task, String> {
    conn.execute(
        "INSERT INTO tasks (title, done, position, created_at, list) VALUES (?1, 0, 0, datetime('now'), ?2)",
        params![title, list.as_str()],
    )
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

pub fn update_task_title(conn: &Connection, id: i64, title: &str) -> Result<Task, String> {
    let n = conn
        .execute(
            "UPDATE tasks SET title = ?1 WHERE id = ?2",
            params![title, id],
        )
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

/// Removes completed tasks whose calendar `created_at` is on or before `cutoff_date`.
/// `cutoff_date` must be `"YYYY-MM-DD"` (local retention cutoff).
pub fn purge_completed_by_created_date(
    conn: &Connection,
    cutoff_date: &str,
) -> Result<usize, String> {
    conn.execute(
        "DELETE FROM tasks
         WHERE list = 'urgent'
           AND done = 1
           AND created_at IS NOT NULL
           AND date(created_at) <= date(?1)",
        params![cutoff_date],
    )
    .map_err(|e| e.to_string())?;
    Ok(conn.changes() as usize)
}

/// Resets daily tasks completed before `today` (`YYYY-MM-DD`, local).
pub fn reset_daily_tasks(conn: &Connection, today: &str) -> Result<usize, String> {
    conn.execute(
        "UPDATE tasks
         SET done = 0, completed_at = NULL
         WHERE list = 'daily'
           AND done = 1
           AND completed_at IS NOT NULL
           AND date(completed_at) < date(?1)",
        params![today],
    )
    .map_err(|e| e.to_string())?;
    Ok(conn.changes() as usize)
}

/// Resets weekly tasks completed before `week_start` (`YYYY-MM-DD`, local Monday).
pub fn reset_weekly_tasks(conn: &Connection, week_start: &str) -> Result<usize, String> {
    conn.execute(
        "UPDATE tasks
         SET done = 0, completed_at = NULL
         WHERE list = 'weekly'
           AND done = 1
           AND completed_at IS NOT NULL
           AND date(completed_at) < date(?1)",
        params![week_start],
    )
    .map_err(|e| e.to_string())?;
    Ok(conn.changes() as usize)
}

#[cfg(test)]
mod tests {
    use super::*;

    const URGENT: TaskList = TaskList::Urgent;

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
        let task = create_task(&conn, "test", URGENT).unwrap();
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
        let a = create_task(&conn, "a", URGENT).unwrap();
        let b = create_task(&conn, "b", URGENT).unwrap();
        assert_eq!(a.position, a.id as f64);
        assert_eq!(b.position, b.id as f64);
        let list = list_tasks(&conn, URGENT).unwrap();
        assert_eq!(list[0].id, b.id);
        assert_eq!(list[1].id, a.id);
    }

    #[test]
    fn list_orders_active_before_done() {
        let conn = test_conn();
        let active = create_task(&conn, "active", URGENT).unwrap();
        let mut done = create_task(&conn, "done", URGENT).unwrap();
        done = set_done(&conn, done.id, true).unwrap();
        let list = list_tasks(&conn, URGENT).unwrap();
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].id, active.id);
        assert_eq!(list[1].id, done.id);
    }

    #[test]
    fn move_active_changes_order() {
        let conn = test_conn();
        let a = create_task(&conn, "a", URGENT).unwrap();
        let b = create_task(&conn, "b", URGENT).unwrap();
        move_active_to_index(&conn, a.id, 0).unwrap().task;
        let list = list_tasks(&conn, URGENT).unwrap();
        assert_eq!(list[0].id, a.id);
        assert_eq!(list[1].id, b.id);
    }

    fn insert_task(
        conn: &Connection,
        title: &str,
        done: bool,
        created_at: Option<&str>,
        completed_at: Option<&str>,
        list: TaskList,
    ) {
        conn.execute(
            "INSERT INTO tasks (title, done, position, created_at, completed_at, list) VALUES (?1, ?2, 0, ?3, ?4, ?5)",
            params![
                title,
                if done { 1 } else { 0 },
                created_at,
                completed_at,
                list.as_str()
            ],
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
    fn purge_removes_completed_on_or_before_cutoff() {
        let conn = test_conn();
        insert_task(
            &conn,
            "old",
            true,
            Some("2026-06-08 10:00:00"),
            None,
            URGENT,
        );
        let deleted = purge_completed_by_created_date(&conn, "2026-06-09").unwrap();
        assert_eq!(deleted, 1);
        assert_eq!(list_tasks(&conn, URGENT).unwrap().len(), 0);
    }

    #[test]
    fn purge_keeps_completed_after_cutoff() {
        let conn = test_conn();
        insert_task(
            &conn,
            "recent",
            true,
            Some("2026-06-10 08:00:00"),
            None,
            URGENT,
        );
        let deleted = purge_completed_by_created_date(&conn, "2026-06-09").unwrap();
        assert_eq!(deleted, 0);
        assert_eq!(list_tasks(&conn, URGENT).unwrap().len(), 1);
    }

    #[test]
    fn purge_keeps_completed_on_cutoff_date() {
        let conn = test_conn();
        insert_task(
            &conn,
            "border",
            true,
            Some("2026-06-09 23:59:00"),
            None,
            URGENT,
        );
        let deleted = purge_completed_by_created_date(&conn, "2026-06-09").unwrap();
        assert_eq!(deleted, 1);
        assert_eq!(list_tasks(&conn, URGENT).unwrap().len(), 0);
    }

    #[test]
    fn purge_keeps_active() {
        let conn = test_conn();
        insert_task(
            &conn,
            "active",
            false,
            Some("2026-06-01 00:00:00"),
            None,
            URGENT,
        );
        let deleted = purge_completed_by_created_date(&conn, "2026-06-09").unwrap();
        assert_eq!(deleted, 0);
        assert_eq!(list_tasks(&conn, URGENT).unwrap().len(), 1);
    }

    #[test]
    fn purge_skips_null_created_at() {
        let conn = test_conn();
        insert_task(&conn, "no date", true, None, None, URGENT);
        let deleted = purge_completed_by_created_date(&conn, "2026-06-09").unwrap();
        assert_eq!(deleted, 0);
        assert_eq!(list_tasks(&conn, URGENT).unwrap().len(), 1);
    }

    #[test]
    fn update_task_title_changes_title() {
        let conn = test_conn();
        let task = create_task(&conn, "old", URGENT).unwrap();
        let updated = update_task_title(&conn, task.id, "new").unwrap();
        assert_eq!(updated.title, "new");
        assert_eq!(updated.done, task.done);
        assert_eq!(updated.position, task.position);
        assert_eq!(updated.completed_at, task.completed_at);
        assert_eq!(updated.created_at, task.created_at);
    }

    #[test]
    fn update_task_title_not_found() {
        let conn = test_conn();
        assert_eq!(
            update_task_title(&conn, 999, "x").unwrap_err(),
            "task not found".to_string()
        );
    }

    #[test]
    fn update_task_title_on_done_task() {
        let conn = test_conn();
        let task = create_task(&conn, "done task", URGENT).unwrap();
        let done = set_done(&conn, task.id, true).unwrap();
        let updated = update_task_title(&conn, done.id, "renamed").unwrap();
        assert_eq!(updated.title, "renamed");
        assert!(updated.done);
        assert!(updated.completed_at.is_some());
    }

    #[test]
    fn delete_task_removes_row() {
        let conn = test_conn();
        let task = create_task(&conn, "gone", URGENT).unwrap();
        delete_task(&conn, task.id).unwrap();
        assert!(fetch_task(&conn, task.id).is_err());
    }

    #[test]
    fn delete_task_not_found() {
        let conn = test_conn();
        assert_eq!(delete_task(&conn, 999), Err("task not found".to_string()));
    }

    #[test]
    fn set_done_not_found() {
        let conn = test_conn();
        assert_eq!(
            set_done(&conn, 999, true).unwrap_err(),
            "task not found".to_string()
        );
    }

    #[test]
    fn fetch_task_not_found() {
        let conn = test_conn();
        assert!(fetch_task(&conn, 999).is_err());
    }

    #[test]
    fn open_applies_migrations() {
        let dir = tempfile::tempdir().unwrap();
        let conn = open(dir.path()).unwrap();
        assert!(column_exists(&conn, "completed_at").unwrap());
        assert!(column_exists(&conn, "position").unwrap());
        assert!(column_exists(&conn, "created_at").unwrap());
        assert!(column_exists(&conn, "list").unwrap());
        assert!(dir.path().join(DB_FILE).exists());
        drop(conn);
    }

    #[test]
    fn list_active_sorted_by_position_desc() {
        let conn = test_conn();
        let a = create_task(&conn, "a", URGENT).unwrap();
        let b = create_task(&conn, "b", URGENT).unwrap();
        conn.execute(
            "UPDATE tasks SET position = 1.0 WHERE id = ?1",
            params![a.id],
        )
        .unwrap();
        conn.execute(
            "UPDATE tasks SET position = 2.0 WHERE id = ?1",
            params![b.id],
        )
        .unwrap();
        let list = list_tasks(&conn, URGENT).unwrap();
        let active: Vec<_> = list.iter().filter(|t| !t.done).collect();
        assert_eq!(active[0].id, b.id);
        assert_eq!(active[1].id, a.id);
    }

    #[test]
    fn list_done_sorted_by_completed_at_desc() {
        let conn = test_conn();
        insert_task(
            &conn,
            "old",
            true,
            Some("2026-05-28 10:00:00"),
            Some("2026-05-28 10:00:00"),
            URGENT,
        );
        insert_task(
            &conn,
            "new",
            true,
            Some("2026-05-30 10:00:00"),
            Some("2026-05-30 10:00:00"),
            URGENT,
        );
        let list = list_tasks(&conn, URGENT).unwrap();
        let done: Vec<_> = list.iter().filter(|t| t.done).collect();
        assert_eq!(done.len(), 2);
        assert_eq!(done[0].title, "new");
        assert_eq!(done[1].title, "old");
    }

    #[test]
    fn move_active_rejects_done_task() {
        let conn = test_conn();
        let task = create_task(&conn, "x", URGENT).unwrap();
        set_done(&conn, task.id, true).unwrap();
        assert_eq!(
            move_active_to_index(&conn, task.id, 0).unwrap_err(),
            "task is not active".to_string()
        );
    }

    #[test]
    fn move_active_to_index_clamps_to_end() {
        let conn = test_conn();
        let a = create_task(&conn, "a", URGENT).unwrap();
        let _b = create_task(&conn, "b", URGENT).unwrap();
        let _c = create_task(&conn, "c", URGENT).unwrap();
        move_active_to_index(&conn, a.id, 100).unwrap();
        let list = list_tasks(&conn, URGENT).unwrap();
        let active: Vec<_> = list.iter().filter(|t| !t.done).collect();
        assert_eq!(active.last().unwrap().id, a.id);
    }

    #[test]
    fn rebalance_reassigns_sequential_positions() {
        let conn = test_conn();
        let a = create_task(&conn, "a", URGENT).unwrap();
        let b = create_task(&conn, "b", URGENT).unwrap();
        conn.execute(
            "UPDATE tasks SET position = 1.0 WHERE id = ?1",
            params![a.id],
        )
        .unwrap();
        conn.execute(
            "UPDATE tasks SET position = 2.0 WHERE id = ?1",
            params![b.id],
        )
        .unwrap();
        rebalance_active_positions(&conn, URGENT).unwrap();
        let a_after = fetch_task(&conn, a.id).unwrap();
        let b_after = fetch_task(&conn, b.id).unwrap();
        assert_eq!(b_after.position, 2.0);
        assert_eq!(a_after.position, 1.0);
    }

    #[test]
    fn assign_position_between_cases() {
        assert_eq!(assign_position_between(None, None), 1.0);
        assert_eq!(assign_position_between(None, Some(5.0)), 6.0);
        assert_eq!(assign_position_between(Some(3.0), None), 2.0);
        assert_eq!(assign_position_between(Some(3.0), Some(5.0)), 4.0);
    }

    #[test]
    fn migrate_adds_created_at_and_backfills() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE tasks (
                 id    INTEGER PRIMARY KEY AUTOINCREMENT,
                 title TEXT NOT NULL,
                 done  INTEGER NOT NULL CHECK (done IN (0, 1)),
                 completed_at TEXT,
                 position REAL NOT NULL DEFAULT 0
             );",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO tasks (title, done, position) VALUES ('legacy', 0, 1.0)",
            [],
        )
        .unwrap();
        migrate(&conn).unwrap();
        assert!(column_exists(&conn, "created_at").unwrap());
        let task = fetch_task(&conn, 1).unwrap();
        assert!(task.created_at.is_some());
    }

    #[test]
    fn create_task_sets_created_at() {
        let conn = test_conn();
        let task = create_task(&conn, "new", URGENT).unwrap();
        assert!(task.created_at.is_some());
    }

    #[test]
    fn set_done_does_not_change_created_at() {
        let conn = test_conn();
        let task = create_task(&conn, "x", URGENT).unwrap();
        let created = task.created_at.clone();
        let done = set_done(&conn, task.id, true).unwrap();
        assert_eq!(done.created_at, created);
        let undone = set_done(&conn, task.id, false).unwrap();
        assert_eq!(undone.created_at, created);
    }

    #[test]
    fn update_task_title_does_not_change_created_at() {
        let conn = test_conn();
        let task = create_task(&conn, "old", URGENT).unwrap();
        let created = task.created_at.clone();
        let updated = update_task_title(&conn, task.id, "new").unwrap();
        assert_eq!(updated.created_at, created);
    }

    #[test]
    fn list_tasks_isolated_by_list() {
        let conn = test_conn();
        create_task(&conn, "urgent", URGENT).unwrap();
        create_task(&conn, "daily", TaskList::Daily).unwrap();
        assert_eq!(list_tasks(&conn, URGENT).unwrap().len(), 1);
        assert_eq!(list_tasks(&conn, TaskList::Daily).unwrap().len(), 1);
        assert_eq!(list_tasks(&conn, TaskList::Weekly).unwrap().len(), 0);
    }

    #[test]
    fn move_active_scoped_to_list() {
        let conn = test_conn();
        let urgent_a = create_task(&conn, "ua", URGENT).unwrap();
        let urgent_b = create_task(&conn, "ub", URGENT).unwrap();
        let daily_a = create_task(&conn, "da", TaskList::Daily).unwrap();
        move_active_to_index(&conn, urgent_a.id, 0).unwrap();
        let urgent = list_tasks(&conn, URGENT).unwrap();
        assert_eq!(urgent[0].id, urgent_a.id);
        assert_eq!(urgent[1].id, urgent_b.id);
        let daily = list_tasks(&conn, TaskList::Daily).unwrap();
        assert_eq!(daily[0].id, daily_a.id);
    }

    #[test]
    fn purge_does_not_remove_daily_or_weekly() {
        let conn = test_conn();
        insert_task(
            &conn,
            "daily old",
            true,
            Some("2026-06-01 00:00:00"),
            Some("2026-06-01 00:00:00"),
            TaskList::Daily,
        );
        insert_task(
            &conn,
            "weekly old",
            true,
            Some("2026-06-01 00:00:00"),
            Some("2026-06-01 00:00:00"),
            TaskList::Weekly,
        );
        let deleted = purge_completed_by_created_date(&conn, "2026-06-09").unwrap();
        assert_eq!(deleted, 0);
        assert_eq!(list_tasks(&conn, TaskList::Daily).unwrap().len(), 1);
        assert_eq!(list_tasks(&conn, TaskList::Weekly).unwrap().len(), 1);
    }

    #[test]
    fn reset_daily_clears_done_before_today() {
        let conn = test_conn();
        insert_task(
            &conn,
            "yesterday",
            true,
            Some("2026-06-09 12:00:00"),
            Some("2026-06-09 12:00:00"),
            TaskList::Daily,
        );
        insert_task(
            &conn,
            "today",
            true,
            Some("2026-06-10 08:00:00"),
            Some("2026-06-10 08:00:00"),
            TaskList::Daily,
        );
        let n = reset_daily_tasks(&conn, "2026-06-10").unwrap();
        assert_eq!(n, 1);
        let list = list_tasks(&conn, TaskList::Daily).unwrap();
        let yesterday = list.iter().find(|t| t.title == "yesterday").unwrap();
        let today = list.iter().find(|t| t.title == "today").unwrap();
        assert!(!yesterday.done);
        assert!(today.done);
    }

    #[test]
    fn reset_weekly_clears_done_before_week_start() {
        let conn = test_conn();
        insert_task(
            &conn,
            "last week",
            true,
            Some("2026-06-06 12:00:00"),
            Some("2026-06-06 12:00:00"),
            TaskList::Weekly,
        );
        insert_task(
            &conn,
            "this week",
            true,
            Some("2026-06-10 08:00:00"),
            Some("2026-06-10 08:00:00"),
            TaskList::Weekly,
        );
        let n = reset_weekly_tasks(&conn, "2026-06-08").unwrap();
        assert_eq!(n, 1);
        let list = list_tasks(&conn, TaskList::Weekly).unwrap();
        let last = list.iter().find(|t| t.title == "last week").unwrap();
        let current = list.iter().find(|t| t.title == "this week").unwrap();
        assert!(!last.done);
        assert!(current.done);
    }
}
