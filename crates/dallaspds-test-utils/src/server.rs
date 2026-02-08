use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;

use dallaspds_blob_fs::FsBlobStore;
use dallaspds_core::config::{BlobsConfig, DatabaseConfig, JwtConfig, PdsConfig, PdsMode};
use dallaspds_server::{AppState, Sequencer, build_router};
use dallaspds_storage_sqlite::{SqliteAccountStore, SqliteRepoStore};

use crate::stores::{TestStores, create_test_stores};

pub const TEST_ACCESS_SECRET: &str = "test-access-secret-at-least-32-chars-long";
pub const TEST_REFRESH_SECRET: &str = "test-refresh-secret-at-least-32-chars-long";
pub const TEST_PASSWORD: &str = "hunter2-test-password";

pub fn create_test_config() -> PdsConfig {
    PdsConfig {
        hostname: "test.pds.local".to_string(),
        port: 0,
        public_url: "https://test.pds.local".to_string(),
        plc_url: "https://plc.directory".to_string(),
        available_user_domains: vec![".test.pds.local".to_string()],
        invite_required: false,
        jwt: JwtConfig {
            access_secret: TEST_ACCESS_SECRET.to_string(),
            refresh_secret: TEST_REFRESH_SECRET.to_string(),
        },
        database: DatabaseConfig {
            url: String::new(), // not used; stores are pre-connected
        },
        blobs: BlobsConfig {
            path: None,
            bucket: None,
            region: None,
        },
        mode: PdsMode::Single,
        appview_url: None,
        appview_did: None,
        relay_url: None,
        tls: None,
    }
}

pub fn create_test_app_state(
    stores: &TestStores,
) -> AppState<SqliteAccountStore, SqliteRepoStore, FsBlobStore> {
    let sequencer = Sequencer::new(1, 256);

    AppState {
        account_store: Arc::new(stores.account_store.clone()),
        repo_store: Arc::new(stores.repo_store.clone()),
        blob_store: Arc::new(stores.blob_store.clone()),
        config: Arc::new(create_test_config()),
        sequencer: Some(sequencer),
        relay_notifier: None,
        event_store: Some(stores.event_store_arc()),
    }
}

pub fn create_test_router(
    stores: &TestStores,
) -> Router {
    let state = create_test_app_state(stores);
    build_router(state)
}

pub async fn create_test_router_and_stores() -> (Router, TestStores) {
    let stores = create_test_stores().await;
    let router = create_test_router(&stores);
    (router, stores)
}

/// Create an account via the API and return (did, access_jwt, refresh_jwt).
pub async fn create_account_via_api(
    router: &Router,
    handle: &str,
) -> (String, String, String) {
    let body = serde_json::json!({
        "handle": handle,
        "email": format!("{}@test.com", handle.split('.').next().unwrap_or("user")),
        "password": TEST_PASSWORD,
    });

    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/xrpc/com.atproto.server.createAccount")
        .header("content-type", "application/json")
        .body(Body::from(serde_json::to_vec(&body).unwrap()))
        .unwrap();

    let resp = router.clone().oneshot(req).await.unwrap();
    let status = resp.status();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let json: Value = serde_json::from_slice(&bytes)
        .unwrap_or_else(|_| panic!("createAccount failed with status {status}: {}", String::from_utf8_lossy(&bytes)));

    assert_eq!(status, 200, "createAccount failed: {json}");

    let did = json["did"].as_str().unwrap().to_string();
    let access_jwt = json["accessJwt"].as_str().unwrap().to_string();
    let refresh_jwt = json["refreshJwt"].as_str().unwrap().to_string();

    (did, access_jwt, refresh_jwt)
}

/// Send a request through the router and return (status, body_json).
pub async fn send_request(
    router: &Router,
    method: &str,
    uri: &str,
    auth_token: Option<&str>,
    body: Option<Value>,
) -> (u16, Value) {
    let mut builder = axum::http::Request::builder()
        .method(method)
        .uri(uri);

    if let Some(token) = auth_token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }

    if body.is_some() {
        builder = builder.header("content-type", "application/json");
    }

    let req_body = match body {
        Some(b) => Body::from(serde_json::to_vec(&b).unwrap()),
        None => Body::empty(),
    };

    let req = builder.body(req_body).unwrap();
    let resp = router.clone().oneshot(req).await.unwrap();
    let status = resp.status().as_u16();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();

    let json = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::String(
            String::from_utf8_lossy(&bytes).to_string(),
        ))
    };

    (status, json)
}
