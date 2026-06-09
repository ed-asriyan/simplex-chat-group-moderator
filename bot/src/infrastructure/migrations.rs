use std::sync::{Arc, Mutex};

use rusqlite::Connection;

use crate::domain::moderator::ports::Err;

use include_dir::{Dir, include_dir};

static MIGRATIONS_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/src/infrastructure/migrations");

/// Apply all pending migrations, bringing the database schema up to date.
///
/// This is idempotent: migrations already recorded in `PRAGMA user_version` are
/// skipped. Each migration runs inside its own transaction together with the
/// version bump, so a failure leaves the database at the last fully-applied
/// version.
pub async fn run(conn: Arc<Mutex<Connection>>) -> Result<(), Err> {
    tokio::task::spawn_blocking(move || -> Result<(), Err> {
        let mut guard = conn.lock().expect("migration connection poisoned");
        apply(&mut guard)
    })
    .await
    .map_err(|e| -> Err { e.to_string().into() })?
}

fn apply(conn: &mut Connection) -> Result<(), Err> {
    let current: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    let current = current.max(0) as usize;

    // Get all files, filter for .sql, and sort them strictly by name (e.g. 0001_..., 0002_...)
    let mut files: Vec<_> = MIGRATIONS_DIR
        .files()
        .filter(|f| f.path().extension().is_some_and(|e| e == "sql"))
        .collect();
    files.sort_by_key(|f| f.path());

    for (idx, file) in files.iter().enumerate() {
        let version = idx + 1;
        if version <= current {
            continue;
        }

        let sql = file
            .contents_utf8()
            .ok_or("migration file is not valid UTF-8")?;

        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        // PRAGMA does not accept bound parameters; version is a trusted usize.
        tx.execute_batch(&format!("PRAGMA user_version = {version}"))?;
        tx.commit()?;
    }

    Ok(())
}
