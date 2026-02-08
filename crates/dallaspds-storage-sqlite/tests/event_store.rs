use dallaspds_core::EventStore;
use dallaspds_storage_sqlite::{SqliteAccountStore, SqliteEventStore};
use tempfile::TempDir;

async fn setup() -> (SqliteEventStore, TempDir) {
    let tempdir = TempDir::new().unwrap();
    let db_path = tempdir.path().join("test.db");
    let db_url = format!("sqlite://{}?mode=rwc", db_path.display());

    // Run account store migrations first to create the schema, since event
    // store needs the firehose_event table which is created by migrations.
    let _account_store = SqliteAccountStore::connect(&db_url).await.unwrap();

    // Now create the firehose_event table
    let pool = sqlx::SqlitePool::connect(&db_url).await.unwrap();
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
    .unwrap();

    let event_store = SqliteEventStore::connect(&db_url).await.unwrap();
    (event_store, tempdir)
}

#[tokio::test]
async fn append_returns_seq() {
    let (store, _dir) = setup().await;
    let seq = store.append_event("commit", "did:plc:test", b"payload1").await.unwrap();
    assert!(seq > 0, "first seq should be > 0");
}

#[tokio::test]
async fn sequential_seq() {
    let (store, _dir) = setup().await;
    let seq1 = store.append_event("commit", "did:plc:test", b"p1").await.unwrap();
    let seq2 = store.append_event("commit", "did:plc:test", b"p2").await.unwrap();
    let seq3 = store.append_event("identity", "did:plc:test", b"p3").await.unwrap();
    assert!(seq2 > seq1);
    assert!(seq3 > seq2);
}

#[tokio::test]
async fn get_events_after() {
    let (store, _dir) = setup().await;
    let seq1 = store.append_event("commit", "did:plc:a", b"p1").await.unwrap();
    let _seq2 = store.append_event("commit", "did:plc:b", b"p2").await.unwrap();
    let _seq3 = store.append_event("identity", "did:plc:c", b"p3").await.unwrap();

    let events = store.get_events_after(seq1, 100).await.unwrap();
    assert_eq!(events.len(), 2, "should get 2 events after seq1");
    assert_eq!(events[0].did, "did:plc:b");
    assert_eq!(events[1].did, "did:plc:c");
}

#[tokio::test]
async fn get_events_limit() {
    let (store, _dir) = setup().await;
    for i in 0..5 {
        store.append_event("commit", &format!("did:plc:{i}"), b"p").await.unwrap();
    }

    let events = store.get_events_after(0, 2).await.unwrap();
    assert_eq!(events.len(), 2);
}

#[tokio::test]
async fn get_events_empty() {
    let (store, _dir) = setup().await;
    let events = store.get_events_after(0, 100).await.unwrap();
    assert!(events.is_empty());
}

#[tokio::test]
async fn max_seq_initial() {
    let (store, _dir) = setup().await;
    let max = store.get_max_seq().await.unwrap();
    assert_eq!(max, 0);
}

#[tokio::test]
async fn max_seq_after_inserts() {
    let (store, _dir) = setup().await;
    let seq1 = store.append_event("commit", "did:plc:a", b"p1").await.unwrap();
    let seq2 = store.append_event("commit", "did:plc:b", b"p2").await.unwrap();
    let max = store.get_max_seq().await.unwrap();
    assert_eq!(max, seq2);
    assert!(max > seq1);
}
