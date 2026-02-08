use std::sync::Arc;
use tempfile::TempDir;

use dallaspds_blob_fs::FsBlobStore;
use dallaspds_core::EventStore;
use dallaspds_storage_sqlite::{SqliteAccountStore, SqliteEventStore, SqliteRepoStore};

pub struct TestStores {
    pub account_store: SqliteAccountStore,
    pub repo_store: SqliteRepoStore,
    pub event_store: SqliteEventStore,
    pub blob_store: FsBlobStore,
    /// Hold the TempDir to keep it alive for the test's duration.
    pub _tempdir: TempDir,
}

/// Create a fresh set of test stores backed by a tempdir.
///
/// All SQLite stores share the same file-backed database. The blob store
/// writes to a `blobs/` subdirectory inside the same tempdir.
pub async fn create_test_stores() -> TestStores {
    let tempdir = TempDir::new().expect("failed to create tempdir");
    let db_path = tempdir.path().join("test.db");
    let db_url = format!("sqlite://{}?mode=rwc", db_path.display());

    let account_store = SqliteAccountStore::connect(&db_url)
        .await
        .expect("failed to connect account store");
    let repo_store = SqliteRepoStore::connect(&db_url)
        .await
        .expect("failed to connect repo store");
    let event_store = SqliteEventStore::connect(&db_url)
        .await
        .expect("failed to connect event store");

    // Create the firehose_event table (event store doesn't run migrations)
    let pool = sqlx::SqlitePool::connect(&db_url)
        .await
        .expect("pool connect");
    sqlx::query(
        "CREATE TABLE IF NOT EXISTS firehose_event (
            seq INTEGER PRIMARY KEY AUTOINCREMENT,
            event_type TEXT NOT NULL,
            did TEXT NOT NULL,
            payload BLOB NOT NULL,
            created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
        )",
    )
    .execute(&pool)
    .await
    .expect("create firehose_event table");

    let blobs_path = tempdir.path().join("blobs");
    let blob_store =
        FsBlobStore::new(blobs_path.to_str().unwrap()).expect("failed to create blob store");

    TestStores {
        account_store,
        repo_store,
        event_store,
        blob_store,
        _tempdir: tempdir,
    }
}

impl TestStores {
    pub fn event_store_arc(&self) -> Arc<dyn EventStore> {
        Arc::new(self.event_store.clone()) as Arc<dyn EventStore>
    }
}
