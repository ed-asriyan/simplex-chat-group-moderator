//! A fresh in-memory database with every migration applied, for adapter tests.

use std::sync::{Arc, Mutex};

use rusqlite::Connection;

use super::migrations;

pub(crate) async fn test_db() -> Arc<Mutex<Connection>> {
    let conn = Arc::new(Mutex::new(
        Connection::open_in_memory().expect("in-memory database"),
    ));
    migrations::run(conn.clone())
        .await
        .expect("migrations apply");
    conn
}
