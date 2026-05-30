use std::path::Path;

use rusqlite::{params, Connection, Row};

use crate::Task;

const DB_FILE: &str = "tasks.db";

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

fn migrate(conn: &Connection) -> Result<(), String> {
    let has_column = conn
        .prepare("PRAGMA table_info(tasks)")
        .map_err(|e| e.to_string())?
        .query_map([], |row| row.get::<_, String>(1))
        .map_err(|e| e.to_string())?
        .filter_map(|name| name.ok())
        .any(|name| name == "completed_at");

    if !has_column {
        conn.execute("ALTER TABLE tasks ADD COLUMN completed_at TEXT", [])
            .map_err(|e| e.to_string())?;
    }

    conn.execute(
        "UPDATE tasks SET completed_at = datetime('now') WHERE done = 1 AND completed_at IS NULL",
        [],
    )
    .map_err(|e| e.to_string())?;

    Ok(())
}

fn row_to_task(row: &Row<'_>) -> rusqlite::Result<Task> {
    Ok(Task {
        id: row.get(0)?,
        title: row.get(1)?,
        done: row.get::<_, i32>(2)? != 0,
        completed_at: row.get(3)?,
    })
}

pub fn list_tasks(conn: &Connection) -> Result<Vec<Task>, String> {
    let mut stmt = conn
        .prepare("SELECT id, title, done, completed_at FROM tasks")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], row_to_task)
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}

pub fn fetch_task(conn: &Connection, id: i64) -> Result<Task, String> {
    conn.query_row(
        "SELECT id, title, done, completed_at FROM tasks WHERE id = ?1",
        params![id],
        row_to_task,
    )
    .map_err(|e| e.to_string())
}

pub fn create_task(conn: &Connection, title: &str) -> Result<Task, String> {
    conn.execute("INSERT INTO tasks (title, done) VALUES (?1, 0)", params![title])
        .map_err(|e| e.to_string())?;
    let id = conn.last_insert_rowid();
    Ok(Task {
        id,
        title: title.to_string(),
        done: false,
        completed_at: None,
    })
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
        conn
    }

    #[test]
    fn set_done_sets_and_clears_completed_at() {
        let conn = test_conn();
        let task = create_task(&conn, "test").unwrap();
        assert!(task.completed_at.is_none());

        let done = set_done(&conn, task.id, true).unwrap();
        assert!(done.done);
        assert!(done.completed_at.is_some());

        let undone = set_done(&conn, task.id, false).unwrap();
        assert!(!undone.done);
        assert!(undone.completed_at.is_none());
    }

    #[test]
    fn list_tasks_has_no_guaranteed_order() {
        let conn = test_conn();
        create_task(&conn, "a").unwrap();
        create_task(&conn, "b").unwrap();
        let tasks = list_tasks(&conn).unwrap();
        assert_eq!(tasks.len(), 2);
    }
}
