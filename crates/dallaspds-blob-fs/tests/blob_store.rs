use bytes::Bytes;
use dallaspds_blob_fs::FsBlobStore;
use dallaspds_core::BlobStore;
use tempfile::TempDir;

fn setup() -> (FsBlobStore, TempDir) {
    let tempdir = TempDir::new().unwrap();
    let blobs_path = tempdir.path().join("blobs");
    let store = FsBlobStore::new(blobs_path.to_str().unwrap()).unwrap();
    (store, tempdir)
}

#[tokio::test]
async fn put_and_get() {
    let (store, _dir) = setup();
    let data = Bytes::from_static(b"hello blob");
    store.put_blob("did:plc:test", "cid1", data.clone(), "text/plain").await.unwrap();

    let result = store.get_blob("did:plc:test", "cid1").await.unwrap();
    assert!(result.is_some());
    let (got_data, got_mime) = result.unwrap();
    assert_eq!(got_data, data);
    assert_eq!(got_mime, "text/plain");
}

#[tokio::test]
async fn stores_mime_type() {
    let (store, _dir) = setup();
    store.put_blob("did:plc:test", "cid-img", Bytes::from_static(b"png data"), "image/png").await.unwrap();

    let (_, mime) = store.get_blob("did:plc:test", "cid-img").await.unwrap().unwrap();
    assert_eq!(mime, "image/png");
}

#[tokio::test]
async fn get_nonexistent() {
    let (store, _dir) = setup();
    let result = store.get_blob("did:plc:test", "no-such-cid").await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn has_blob() {
    let (store, _dir) = setup();
    assert!(!store.has_blob("did:plc:test", "cid-x").await.unwrap());

    store.put_blob("did:plc:test", "cid-x", Bytes::from_static(b"data"), "application/octet-stream").await.unwrap();
    assert!(store.has_blob("did:plc:test", "cid-x").await.unwrap());
}

#[tokio::test]
async fn delete_blob() {
    let (store, _dir) = setup();
    store.put_blob("did:plc:test", "cid-del", Bytes::from_static(b"data"), "text/plain").await.unwrap();
    assert!(store.has_blob("did:plc:test", "cid-del").await.unwrap());

    store.delete_blob("did:plc:test", "cid-del").await.unwrap();
    assert!(!store.has_blob("did:plc:test", "cid-del").await.unwrap());
    assert!(store.get_blob("did:plc:test", "cid-del").await.unwrap().is_none());
}

#[tokio::test]
async fn list_blobs_sorted() {
    let (store, _dir) = setup();
    store.put_blob("did:plc:test", "ccc", Bytes::from_static(b"c"), "text/plain").await.unwrap();
    store.put_blob("did:plc:test", "aaa", Bytes::from_static(b"a"), "text/plain").await.unwrap();
    store.put_blob("did:plc:test", "bbb", Bytes::from_static(b"b"), "text/plain").await.unwrap();

    let cids = store.list_blobs("did:plc:test", None, 100).await.unwrap();
    assert_eq!(cids, vec!["aaa", "bbb", "ccc"]);
}

#[tokio::test]
async fn list_blobs_cursor() {
    let (store, _dir) = setup();
    store.put_blob("did:plc:test", "aaa", Bytes::from_static(b"a"), "text/plain").await.unwrap();
    store.put_blob("did:plc:test", "bbb", Bytes::from_static(b"b"), "text/plain").await.unwrap();
    store.put_blob("did:plc:test", "ccc", Bytes::from_static(b"c"), "text/plain").await.unwrap();

    let cids = store.list_blobs("did:plc:test", Some("aaa"), 100).await.unwrap();
    assert_eq!(cids, vec!["bbb", "ccc"]);
}

#[tokio::test]
async fn list_blobs_limit() {
    let (store, _dir) = setup();
    store.put_blob("did:plc:test", "a", Bytes::from_static(b"a"), "text/plain").await.unwrap();
    store.put_blob("did:plc:test", "b", Bytes::from_static(b"b"), "text/plain").await.unwrap();
    store.put_blob("did:plc:test", "c", Bytes::from_static(b"c"), "text/plain").await.unwrap();

    let cids = store.list_blobs("did:plc:test", None, 2).await.unwrap();
    assert_eq!(cids.len(), 2);
}

#[tokio::test]
async fn scoped_to_did() {
    let (store, _dir) = setup();
    store.put_blob("did:plc:a", "cid1", Bytes::from_static(b"data-a"), "text/plain").await.unwrap();
    store.put_blob("did:plc:b", "cid1", Bytes::from_static(b"data-b"), "text/plain").await.unwrap();

    let (data_a, _) = store.get_blob("did:plc:a", "cid1").await.unwrap().unwrap();
    assert_eq!(data_a, Bytes::from_static(b"data-a"));

    let (data_b, _) = store.get_blob("did:plc:b", "cid1").await.unwrap().unwrap();
    assert_eq!(data_b, Bytes::from_static(b"data-b"));
}

#[tokio::test]
async fn did_directory_escaping() {
    let (store, _dir) = setup();
    // DID with colons should be escaped to underscores in paths
    store.put_blob("did:plc:abc123", "cid1", Bytes::from_static(b"data"), "text/plain").await.unwrap();
    let result = store.get_blob("did:plc:abc123", "cid1").await.unwrap();
    assert!(result.is_some());
}
