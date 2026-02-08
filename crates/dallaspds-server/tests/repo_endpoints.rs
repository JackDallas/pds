use dallaspds_test_utils::*;
use serde_json::json;

// ── createRecord ────────────────────────────────────────────────────────

#[tokio::test]
async fn create_record_success() {
    let (router, _stores) = create_test_router_and_stores().await;
    let (did, jwt, _) = create_account_via_api(&router, "rec.test.pds.local").await;

    let (status, body) = send_request(
        &router,
        "POST",
        "/xrpc/com.atproto.repo.createRecord",
        Some(&jwt),
        Some(json!({
            "repo": did,
            "collection": "app.bsky.feed.post",
            "record": {
                "$type": "app.bsky.feed.post",
                "text": "Hello from tests!",
                "createdAt": "2025-01-01T00:00:00Z"
            }
        })),
    )
    .await;
    assert_xrpc_ok(status, &body);
    assert!(body["uri"].as_str().unwrap().starts_with("at://"));
    assert!(body["cid"].as_str().is_some());
}

#[tokio::test]
async fn create_record_unauthenticated_fails() {
    let (router, _stores) = create_test_router_and_stores().await;
    let (status, body) = send_request(
        &router,
        "POST",
        "/xrpc/com.atproto.repo.createRecord",
        None,
        Some(json!({
            "repo": "did:plc:fake",
            "collection": "app.bsky.feed.post",
            "record": { "text": "test" }
        })),
    )
    .await;
    assert_xrpc_error(status, &body, 401, "AuthenticationRequired");
}

#[tokio::test]
async fn create_record_wrong_did_forbidden() {
    let (router, _stores) = create_test_router_and_stores().await;
    let (_, jwt, _) = create_account_via_api(&router, "owner.test.pds.local").await;

    let (status, body) = send_request(
        &router,
        "POST",
        "/xrpc/com.atproto.repo.createRecord",
        Some(&jwt),
        Some(json!({
            "repo": "did:plc:someone-else",
            "collection": "app.bsky.feed.post",
            "record": { "text": "test" }
        })),
    )
    .await;
    assert_xrpc_error(status, &body, 403, "AuthorizationError");
}

// ── getRecord ───────────────────────────────────────────────────────────

#[tokio::test]
async fn get_record_after_create() {
    let (router, _stores) = create_test_router_and_stores().await;
    let (did, jwt, _) = create_account_via_api(&router, "get.test.pds.local").await;

    // Create a record
    let (_, create_body) = send_request(
        &router,
        "POST",
        "/xrpc/com.atproto.repo.createRecord",
        Some(&jwt),
        Some(json!({
            "repo": did,
            "collection": "app.bsky.feed.post",
            "record": {
                "$type": "app.bsky.feed.post",
                "text": "Hello!",
                "createdAt": "2025-01-01T00:00:00Z"
            }
        })),
    )
    .await;

    // Extract rkey from URI
    let uri = create_body["uri"].as_str().unwrap();
    let rkey = uri.rsplit('/').next().unwrap();

    // Get the record
    let (status, body) = send_request(
        &router,
        "GET",
        &format!("/xrpc/com.atproto.repo.getRecord?repo={did}&collection=app.bsky.feed.post&rkey={rkey}"),
        None,
        None,
    )
    .await;
    assert_xrpc_ok(status, &body);
    assert_eq!(body["value"]["text"], "Hello!");
}

#[tokio::test]
async fn get_record_nonexistent_404() {
    let (router, _stores) = create_test_router_and_stores().await;
    let (did, _, _) = create_account_via_api(&router, "notrec.test.pds.local").await;

    let (status, body) = send_request(
        &router,
        "GET",
        &format!("/xrpc/com.atproto.repo.getRecord?repo={did}&collection=app.bsky.feed.post&rkey=nonexistent"),
        None,
        None,
    )
    .await;
    assert_xrpc_error(status, &body, 400, "RecordNotFound");
}

#[tokio::test]
async fn get_record_no_auth_required() {
    let (router, _stores) = create_test_router_and_stores().await;
    let (did, jwt, _) = create_account_via_api(&router, "noauth.test.pds.local").await;

    // Create record
    let (_, create_body) = send_request(
        &router,
        "POST",
        "/xrpc/com.atproto.repo.createRecord",
        Some(&jwt),
        Some(json!({
            "repo": did,
            "collection": "app.bsky.feed.post",
            "record": { "$type": "app.bsky.feed.post", "text": "Public", "createdAt": "2025-01-01T00:00:00Z" }
        })),
    )
    .await;

    let uri = create_body["uri"].as_str().unwrap();
    let rkey = uri.rsplit('/').next().unwrap();

    // Get without auth — should succeed
    let (status, _) = send_request(
        &router,
        "GET",
        &format!("/xrpc/com.atproto.repo.getRecord?repo={did}&collection=app.bsky.feed.post&rkey={rkey}"),
        None,
        None,
    )
    .await;
    assert_eq!(status, 200);
}

// ── listRecords ─────────────────────────────────────────────────────────

#[tokio::test]
async fn list_records_empty_collection() {
    let (router, _stores) = create_test_router_and_stores().await;
    let (did, _, _) = create_account_via_api(&router, "empty.test.pds.local").await;

    let (status, body) = send_request(
        &router,
        "GET",
        &format!("/xrpc/com.atproto.repo.listRecords?repo={did}&collection=app.bsky.feed.post"),
        None,
        None,
    )
    .await;
    assert_xrpc_ok(status, &body);
    assert_eq!(body["records"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn list_records_returns_records() {
    let (router, _stores) = create_test_router_and_stores().await;
    let (did, jwt, _) = create_account_via_api(&router, "list.test.pds.local").await;

    // Create 3 records
    for i in 0..3 {
        send_request(
            &router,
            "POST",
            "/xrpc/com.atproto.repo.createRecord",
            Some(&jwt),
            Some(json!({
                "repo": did,
                "collection": "app.bsky.feed.post",
                "record": { "$type": "app.bsky.feed.post", "text": format!("Post {i}"), "createdAt": "2025-01-01T00:00:00Z" }
            })),
        )
        .await;
    }

    let (status, body) = send_request(
        &router,
        "GET",
        &format!("/xrpc/com.atproto.repo.listRecords?repo={did}&collection=app.bsky.feed.post"),
        None,
        None,
    )
    .await;
    assert_xrpc_ok(status, &body);
    assert_eq!(body["records"].as_array().unwrap().len(), 3);
}

#[tokio::test]
async fn list_records_limit() {
    let (router, _stores) = create_test_router_and_stores().await;
    let (did, jwt, _) = create_account_via_api(&router, "limit.test.pds.local").await;

    for i in 0..5 {
        send_request(
            &router,
            "POST",
            "/xrpc/com.atproto.repo.createRecord",
            Some(&jwt),
            Some(json!({
                "repo": did,
                "collection": "app.bsky.feed.post",
                "record": { "$type": "app.bsky.feed.post", "text": format!("Post {i}"), "createdAt": "2025-01-01T00:00:00Z" }
            })),
        )
        .await;
    }

    let (status, body) = send_request(
        &router,
        "GET",
        &format!("/xrpc/com.atproto.repo.listRecords?repo={did}&collection=app.bsky.feed.post&limit=2"),
        None,
        None,
    )
    .await;
    assert_xrpc_ok(status, &body);
    assert_eq!(body["records"].as_array().unwrap().len(), 2);
}

// ── putRecord ───────────────────────────────────────────────────────────

#[tokio::test]
async fn put_record_creates_new() {
    let (router, _stores) = create_test_router_and_stores().await;
    let (did, jwt, _) = create_account_via_api(&router, "put.test.pds.local").await;

    let (status, body) = send_request(
        &router,
        "POST",
        "/xrpc/com.atproto.repo.putRecord",
        Some(&jwt),
        Some(json!({
            "repo": did,
            "collection": "app.bsky.actor.profile",
            "rkey": "self",
            "record": { "$type": "app.bsky.actor.profile", "displayName": "Test User" }
        })),
    )
    .await;
    assert_xrpc_ok(status, &body);
    assert!(body["uri"].as_str().unwrap().contains("self"));
}

#[tokio::test]
async fn put_record_updates_existing() {
    let (router, _stores) = create_test_router_and_stores().await;
    let (did, jwt, _) = create_account_via_api(&router, "upd.test.pds.local").await;

    // Create
    send_request(
        &router,
        "POST",
        "/xrpc/com.atproto.repo.putRecord",
        Some(&jwt),
        Some(json!({
            "repo": did,
            "collection": "app.bsky.actor.profile",
            "rkey": "self",
            "record": { "$type": "app.bsky.actor.profile", "displayName": "V1" }
        })),
    )
    .await;

    // Update
    let (status, body) = send_request(
        &router,
        "POST",
        "/xrpc/com.atproto.repo.putRecord",
        Some(&jwt),
        Some(json!({
            "repo": did,
            "collection": "app.bsky.actor.profile",
            "rkey": "self",
            "record": { "$type": "app.bsky.actor.profile", "displayName": "V2" }
        })),
    )
    .await;
    assert_xrpc_ok(status, &body);

    // Verify update
    let (_, get_body) = send_request(
        &router,
        "GET",
        &format!("/xrpc/com.atproto.repo.getRecord?repo={did}&collection=app.bsky.actor.profile&rkey=self"),
        None,
        None,
    )
    .await;
    assert_eq!(get_body["value"]["displayName"], "V2");
}

#[tokio::test]
async fn put_record_wrong_did_forbidden() {
    let (router, _stores) = create_test_router_and_stores().await;
    let (_, jwt, _) = create_account_via_api(&router, "putfail.test.pds.local").await;

    let (status, body) = send_request(
        &router,
        "POST",
        "/xrpc/com.atproto.repo.putRecord",
        Some(&jwt),
        Some(json!({
            "repo": "did:plc:someone-else",
            "collection": "app.bsky.actor.profile",
            "rkey": "self",
            "record": { "displayName": "nope" }
        })),
    )
    .await;
    assert_xrpc_error(status, &body, 403, "AuthorizationError");
}

// ── deleteRecord ────────────────────────────────────────────────────────

#[tokio::test]
async fn delete_record_success() {
    let (router, _stores) = create_test_router_and_stores().await;
    let (did, jwt, _) = create_account_via_api(&router, "del.test.pds.local").await;

    // Create a record
    let (_, create_body) = send_request(
        &router,
        "POST",
        "/xrpc/com.atproto.repo.createRecord",
        Some(&jwt),
        Some(json!({
            "repo": did,
            "collection": "app.bsky.feed.post",
            "record": { "$type": "app.bsky.feed.post", "text": "bye", "createdAt": "2025-01-01T00:00:00Z" }
        })),
    )
    .await;

    let uri = create_body["uri"].as_str().unwrap();
    let rkey = uri.rsplit('/').next().unwrap();

    // Delete it
    let (status, _) = send_request(
        &router,
        "POST",
        "/xrpc/com.atproto.repo.deleteRecord",
        Some(&jwt),
        Some(json!({
            "repo": did,
            "collection": "app.bsky.feed.post",
            "rkey": rkey
        })),
    )
    .await;
    assert_eq!(status, 200);
}

#[tokio::test]
async fn delete_record_then_get_fails() {
    let (router, _stores) = create_test_router_and_stores().await;
    let (did, jwt, _) = create_account_via_api(&router, "delget.test.pds.local").await;

    let (_, create_body) = send_request(
        &router,
        "POST",
        "/xrpc/com.atproto.repo.createRecord",
        Some(&jwt),
        Some(json!({
            "repo": did,
            "collection": "app.bsky.feed.post",
            "record": { "$type": "app.bsky.feed.post", "text": "temp", "createdAt": "2025-01-01T00:00:00Z" }
        })),
    )
    .await;

    let uri = create_body["uri"].as_str().unwrap();
    let rkey = uri.rsplit('/').next().unwrap();

    // Delete
    send_request(
        &router,
        "POST",
        "/xrpc/com.atproto.repo.deleteRecord",
        Some(&jwt),
        Some(json!({
            "repo": did,
            "collection": "app.bsky.feed.post",
            "rkey": rkey
        })),
    )
    .await;

    // Get should fail
    let (status, body) = send_request(
        &router,
        "GET",
        &format!("/xrpc/com.atproto.repo.getRecord?repo={did}&collection=app.bsky.feed.post&rkey={rkey}"),
        None,
        None,
    )
    .await;
    assert_xrpc_error(status, &body, 400, "RecordNotFound");
}

#[tokio::test]
async fn delete_record_wrong_did_forbidden() {
    let (router, _stores) = create_test_router_and_stores().await;
    let (_, jwt, _) = create_account_via_api(&router, "delfail.test.pds.local").await;

    let (status, body) = send_request(
        &router,
        "POST",
        "/xrpc/com.atproto.repo.deleteRecord",
        Some(&jwt),
        Some(json!({
            "repo": "did:plc:someone-else",
            "collection": "app.bsky.feed.post",
            "rkey": "abc"
        })),
    )
    .await;
    assert_xrpc_error(status, &body, 403, "AuthorizationError");
}

// ── describeRepo ────────────────────────────────────────────────────────

#[tokio::test]
async fn describe_repo_by_did() {
    let (router, _stores) = create_test_router_and_stores().await;
    let (did, _, _) = create_account_via_api(&router, "desc.test.pds.local").await;

    let (status, body) = send_request(
        &router,
        "GET",
        &format!("/xrpc/com.atproto.repo.describeRepo?repo={did}"),
        None,
        None,
    )
    .await;
    assert_xrpc_ok(status, &body);
    assert_eq!(body["did"], did);
    assert_eq!(body["handle"], "desc.test.pds.local");
}

// ── uploadBlob ──────────────────────────────────────────────────────────

#[tokio::test]
async fn upload_blob_returns_blob_ref() {
    let (router, _stores) = create_test_router_and_stores().await;
    let (_, jwt, _) = create_account_via_api(&router, "blob.test.pds.local").await;

    // Upload a blob with raw body
    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/xrpc/com.atproto.repo.uploadBlob")
        .header("authorization", format!("Bearer {jwt}"))
        .header("content-type", "image/png")
        .body(axum::body::Body::from(b"fake png data".to_vec()))
        .unwrap();

    use http_body_util::BodyExt;
    use tower::ServiceExt;
    let resp = router.clone().oneshot(req).await.unwrap();
    let status = resp.status().as_u16();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();

    assert_eq!(status, 200);
    assert!(body["blob"]["ref"]["$link"].as_str().is_some());
    assert_eq!(body["blob"]["mimeType"], "image/png");
}
