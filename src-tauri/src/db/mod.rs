use std::path::Path;

use rusqlite::{params, Connection};

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
    Ok(conn)
}

pub fn list_tasks(conn: &Connection) -> Result<Vec<Task>, String> {
    let mut stmt = conn
        .prepare("SELECT id, title, done FROM tasks ORDER BY id ASC")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |row| {
            Ok(Task {
                id: row.get(0)?,
                title: row.get(1)?,
                done: row.get::<_, i32>(2)? != 0,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<Result<Vec<_>, _>>()
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
    })
}

pub fn set_done(conn: &Connection, id: i64, done: bool) -> Result<(), String> {
    let done_int: i32 = if done { 1 } else { 0 };
    let n = conn
        .execute(
            "UPDATE tasks SET done = ?1 WHERE id = ?2",
            params![done_int, id],
        )
        .map_err(|e| e.to_string())?;
    if n == 0 {
        return Err("task not found".to_string());
    }
    Ok(())
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
