use dallaspds_core::RepoStore;
use dallaspds_storage_sqlite::SqliteRepoStore;
use tempfile::TempDir;

async fn setup() -> (SqliteRepoStore, TempDir) {
    let tempdir = TempDir::new().unwrap();
    let db_path = tempdir.path().join("test.db");
    let db_url = format!("sqlite://{}?mode=rwc", db_path.display());
    let store = SqliteRepoStore::connect(&db_url).await.unwrap();
    (store, tempdir)
}

#[tokio::test]
async fn put_and_get_block() {
    let (store, _dir) = setup().await;
    let cid = vec![0x01, 0x71, 0x12, 0x20, 0xAA];
    let block = b"block data here".to_vec();

    store.put_block("did:plc:test", &cid, &block).await.unwrap();
    let result = store.get_block("did:plc:test", &cid).await.unwrap();
    assert_eq!(result, Some(block));
}

#[tokio::test]
async fn get_nonexistent() {
    let (store, _dir) = setup().await;
    let result = store.get_block("did:plc:test", &[0xFF]).await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn has_block() {
    let (store, _dir) = setup().await;
    let cid = vec![1, 2, 3];
    assert!(!store.has_block("did:plc:test", &cid).await.unwrap());

    store.put_block("did:plc:test", &cid, b"data").await.unwrap();
    assert!(store.has_block("did:plc:test", &cid).await.unwrap());
}

#[tokio::test]
async fn put_idempotent() {
    let (store, _dir) = setup().await;
    let cid = vec![1, 2, 3];
    store.put_block("did:plc:test", &cid, b"data").await.unwrap();
    // INSERT OR IGNORE should not error on duplicate
    store.put_block("did:plc:test", &cid, b"data").await.unwrap();
    let result = store.get_block("did:plc:test", &cid).await.unwrap();
    assert_eq!(result, Some(b"data".to_vec()));
}

#[tokio::test]
async fn get_all_blocks() {
    let (store, _dir) = setup().await;
    store.put_block("did:plc:test", &[1], b"block1").await.unwrap();
    store.put_block("did:plc:test", &[2], b"block2").await.unwrap();
    store.put_block("did:plc:test", &[3], b"block3").await.unwrap();

    let blocks = store.get_all_blocks("did:plc:test").await.unwrap();
    assert_eq!(blocks.len(), 3);
}

#[tokio::test]
async fn scoped_to_did() {
    let (store, _dir) = setup().await;
    let cid = vec![1, 2, 3];
    store.put_block("did:plc:a", &cid, b"block-a").await.unwrap();
    store.put_block("did:plc:b", &cid, b"block-b").await.unwrap();

    let result_a = store.get_block("did:plc:a", &cid).await.unwrap();
    assert_eq!(result_a, Some(b"block-a".to_vec()));

    let result_b = store.get_block("did:plc:b", &cid).await.unwrap();
    assert_eq!(result_b, Some(b"block-b".to_vec()));
}

#[tokio::test]
async fn delete_blocks_for_did() {
    let (store, _dir) = setup().await;
    store.put_block("did:plc:del", &[1], b"a").await.unwrap();
    store.put_block("did:plc:del", &[2], b"b").await.unwrap();
    store.put_block("did:plc:keep", &[1], b"c").await.unwrap();

    let deleted = store.delete_blocks_for_did("did:plc:del").await.unwrap();
    assert_eq!(deleted, 2);

    assert!(store.get_block("did:plc:del", &[1]).await.unwrap().is_none());
    assert!(store.get_block("did:plc:keep", &[1]).await.unwrap().is_some());
}
