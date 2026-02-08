use dallaspds_core::{AccountStatus, AccountStore, CreateAccountInput, RefreshTokenRecord};
use dallaspds_storage_sqlite::SqliteAccountStore;
use tempfile::TempDir;

async fn setup() -> (SqliteAccountStore, TempDir) {
    let tempdir = TempDir::new().unwrap();
    let db_path = tempdir.path().join("test.db");
    let db_url = format!("sqlite://{}?mode=rwc", db_path.display());
    let store = SqliteAccountStore::connect(&db_url).await.unwrap();
    (store, tempdir)
}

fn test_input(did: &str, handle: &str) -> CreateAccountInput {
    CreateAccountInput {
        did: did.to_string(),
        handle: handle.to_string(),
        email: Some(format!("{handle}@test.com")),
        password_hash: "$argon2id$v=19$m=65536,t=3,p=4$fakesalt$fakehash".to_string(),
        signing_key: vec![1, 2, 3, 4],
    }
}

// ── Account CRUD ────────────────────────────────────────────────────────

#[tokio::test]
async fn create_and_get_by_did() {
    let (store, _dir) = setup().await;
    let input = test_input("did:plc:test1", "alice.test");
    let account = store.create_account(&input).await.unwrap();
    assert_eq!(account.did, "did:plc:test1");
    assert_eq!(account.handle.as_deref(), Some("alice.test"));

    let fetched = store.get_account_by_did("did:plc:test1").await.unwrap();
    assert!(fetched.is_some());
    assert_eq!(fetched.unwrap().did, "did:plc:test1");
}

#[tokio::test]
async fn get_by_handle() {
    let (store, _dir) = setup().await;
    store.create_account(&test_input("did:plc:h1", "bob.test")).await.unwrap();
    let account = store.get_account_by_handle("bob.test").await.unwrap();
    assert!(account.is_some());
    assert_eq!(account.unwrap().did, "did:plc:h1");
}

#[tokio::test]
async fn get_by_email() {
    let (store, _dir) = setup().await;
    store.create_account(&test_input("did:plc:e1", "carol.test")).await.unwrap();
    let account = store.get_account_by_email("carol.test@test.com").await.unwrap();
    assert!(account.is_some());
    assert_eq!(account.unwrap().did, "did:plc:e1");
}

#[tokio::test]
async fn get_nonexistent_returns_none() {
    let (store, _dir) = setup().await;
    assert!(store.get_account_by_did("did:plc:nope").await.unwrap().is_none());
    assert!(store.get_account_by_handle("nope.test").await.unwrap().is_none());
    assert!(store.get_account_by_email("nope@test.com").await.unwrap().is_none());
}

// ── Updates ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn update_handle() {
    let (store, _dir) = setup().await;
    store.create_account(&test_input("did:plc:uh1", "old.test")).await.unwrap();
    store.update_handle("did:plc:uh1", "new.test").await.unwrap();
    let account = store.get_account_by_did("did:plc:uh1").await.unwrap().unwrap();
    assert_eq!(account.handle.as_deref(), Some("new.test"));
}

#[tokio::test]
async fn update_password() {
    let (store, _dir) = setup().await;
    store.create_account(&test_input("did:plc:up1", "pass.test")).await.unwrap();
    store.update_password("did:plc:up1", "new-hash").await.unwrap();
    let account = store.get_account_by_did("did:plc:up1").await.unwrap().unwrap();
    assert_eq!(account.password_hash, "new-hash");
}

// ── Lifecycle ───────────────────────────────────────────────────────────

#[tokio::test]
async fn deactivate_and_activate() {
    let (store, _dir) = setup().await;
    store.create_account(&test_input("did:plc:da1", "active.test")).await.unwrap();

    store.deactivate_account("did:plc:da1").await.unwrap();
    let account = store.get_account_by_did("did:plc:da1").await.unwrap().unwrap();
    assert_eq!(account.status, AccountStatus::Deactivated);
    assert!(account.deactivated_at.is_some());

    store.activate_account("did:plc:da1").await.unwrap();
    let account = store.get_account_by_did("did:plc:da1").await.unwrap().unwrap();
    assert_eq!(account.status, AccountStatus::Active);
    assert!(account.deactivated_at.is_none());
}

#[tokio::test]
async fn delete_account_cascades() {
    let (store, _dir) = setup().await;
    store.create_account(&test_input("did:plc:del1", "delete.test")).await.unwrap();
    store.delete_account("did:plc:del1").await.unwrap();
    assert!(store.get_account_by_did("did:plc:del1").await.unwrap().is_none());
}

// ── Repo root ───────────────────────────────────────────────────────────

#[tokio::test]
async fn repo_root_initially_empty() {
    let (store, _dir) = setup().await;
    store.create_account(&test_input("did:plc:rr1", "repo.test")).await.unwrap();
    let root = store.get_repo_root("did:plc:rr1").await.unwrap().unwrap();
    assert!(root.cid.is_empty(), "initial repo root CID should be empty");
}

#[tokio::test]
async fn repo_root_update_and_get() {
    let (store, _dir) = setup().await;
    store.create_account(&test_input("did:plc:rr2", "root.test")).await.unwrap();
    let cid_bytes = vec![0x01, 0x71, 0x12, 0x20, 0xAA];
    store.update_repo_root("did:plc:rr2", &cid_bytes, "rev1").await.unwrap();

    let root = store.get_repo_root("did:plc:rr2").await.unwrap().unwrap();
    assert_eq!(root.cid, cid_bytes);
    assert_eq!(root.rev, "rev1");
}

#[tokio::test]
async fn repo_root_overwrite() {
    let (store, _dir) = setup().await;
    store.create_account(&test_input("did:plc:rr3", "over.test")).await.unwrap();
    store.update_repo_root("did:plc:rr3", &[1], "rev1").await.unwrap();
    store.update_repo_root("did:plc:rr3", &[2], "rev2").await.unwrap();

    let root = store.get_repo_root("did:plc:rr3").await.unwrap().unwrap();
    assert_eq!(root.cid, vec![2]);
    assert_eq!(root.rev, "rev2");
}

// ── Refresh tokens ──────────────────────────────────────────────────────

#[tokio::test]
async fn refresh_token_crud() {
    let (store, _dir) = setup().await;
    store.create_account(&test_input("did:plc:rt1", "token.test")).await.unwrap();

    let token = RefreshTokenRecord {
        id: "tok-1".to_string(),
        did: "did:plc:rt1".to_string(),
        expires_at: chrono::Utc::now() + chrono::Duration::days(90),
        next_id: None,
        app_password_name: None,
    };
    store.create_refresh_token(&token).await.unwrap();

    let fetched = store.get_refresh_token("tok-1").await.unwrap();
    assert!(fetched.is_some());
    assert_eq!(fetched.unwrap().did, "did:plc:rt1");

    store.delete_refresh_token("tok-1").await.unwrap();
    assert!(store.get_refresh_token("tok-1").await.unwrap().is_none());
}

#[tokio::test]
async fn refresh_token_delete_all_for_did() {
    let (store, _dir) = setup().await;
    store.create_account(&test_input("did:plc:rt2", "tokens.test")).await.unwrap();

    for i in 0..3 {
        let token = RefreshTokenRecord {
            id: format!("tok-{i}"),
            did: "did:plc:rt2".to_string(),
            expires_at: chrono::Utc::now() + chrono::Duration::days(90),
            next_id: None,
            app_password_name: None,
        };
        store.create_refresh_token(&token).await.unwrap();
    }

    let deleted = store.delete_refresh_tokens_for_did("did:plc:rt2").await.unwrap();
    assert_eq!(deleted, 3);
    assert!(store.get_refresh_token("tok-0").await.unwrap().is_none());
}

#[tokio::test]
async fn refresh_token_get_nonexistent() {
    let (store, _dir) = setup().await;
    assert!(store.get_refresh_token("does-not-exist").await.unwrap().is_none());
}

// ── Pagination ──────────────────────────────────────────────────────────

#[tokio::test]
async fn list_accounts_empty() {
    let (store, _dir) = setup().await;
    let accounts = store.list_accounts(None, 10).await.unwrap();
    assert!(accounts.is_empty());
}

#[tokio::test]
async fn list_accounts_populated() {
    let (store, _dir) = setup().await;
    store.create_account(&test_input("did:plc:la1", "a.test")).await.unwrap();
    store.create_account(&test_input("did:plc:la2", "b.test")).await.unwrap();
    store.create_account(&test_input("did:plc:la3", "c.test")).await.unwrap();

    let accounts = store.list_accounts(None, 10).await.unwrap();
    assert_eq!(accounts.len(), 3);
}

#[tokio::test]
async fn list_accounts_cursor_and_limit() {
    let (store, _dir) = setup().await;
    store.create_account(&test_input("did:plc:p1", "p1.test")).await.unwrap();
    store.create_account(&test_input("did:plc:p2", "p2.test")).await.unwrap();
    store.create_account(&test_input("did:plc:p3", "p3.test")).await.unwrap();

    // Limit to 2
    let page1 = store.list_accounts(None, 2).await.unwrap();
    assert_eq!(page1.len(), 2);

    // Use cursor from last DID
    let cursor = &page1.last().unwrap().did;
    let page2 = store.list_accounts(Some(cursor), 10).await.unwrap();
    assert_eq!(page2.len(), 1);
}
