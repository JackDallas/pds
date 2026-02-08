use dallaspds_test_utils::*;
use serde_json::json;

#[tokio::test]
async fn get_repo_returns_car_bytes() {
    let (router, _stores) = create_test_router_and_stores().await;
    let (did, _, _) = create_account_via_api(&router, "car.test.pds.local").await;

    let req = axum::http::Request::builder()
        .method("GET")
        .uri(format!("/xrpc/com.atproto.sync.getRepo?did={did}"))
        .body(axum::body::Body::empty())
        .unwrap();

    use http_body_util::BodyExt;
    use tower::ServiceExt;
    let resp = router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 200);
    let ct = resp.headers().get("content-type").unwrap().to_str().unwrap();
    assert_eq!(ct, "application/vnd.ipld.car");

    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    assert!(!bytes.is_empty(), "CAR file should not be empty");
}

#[tokio::test]
async fn get_repo_nonexistent_did_fails() {
    let (router, _stores) = create_test_router_and_stores().await;
    let (status, body) = send_request(
        &router,
        "GET",
        "/xrpc/com.atproto.sync.getRepo?did=did:plc:nonexistent",
        None,
        None,
    )
    .await;
    assert_xrpc_error(status, &body, 400, "RepoNotFound");
}

#[tokio::test]
async fn get_latest_commit_after_create_account() {
    let (router, _stores) = create_test_router_and_stores().await;
    let (did, _, _) = create_account_via_api(&router, "commit.test.pds.local").await;

    let (status, body) = send_request(
        &router,
        "GET",
        &format!("/xrpc/com.atproto.sync.getLatestCommit?did={did}"),
        None,
        None,
    )
    .await;
    assert_xrpc_ok(status, &body);
    assert!(body["cid"].as_str().is_some());
    assert!(body["rev"].as_str().is_some());
}

#[tokio::test]
async fn get_latest_commit_changes_after_write() {
    let (router, _stores) = create_test_router_and_stores().await;
    let (did, jwt, _) = create_account_via_api(&router, "rev.test.pds.local").await;

    // Get initial commit
    let (_, body1) = send_request(
        &router,
        "GET",
        &format!("/xrpc/com.atproto.sync.getLatestCommit?did={did}"),
        None,
        None,
    )
    .await;
    let cid1 = body1["cid"].as_str().unwrap().to_string();

    // Write a record
    send_request(
        &router,
        "POST",
        "/xrpc/com.atproto.repo.createRecord",
        Some(&jwt),
        Some(json!({
            "repo": did,
            "collection": "app.bsky.feed.post",
            "record": { "$type": "app.bsky.feed.post", "text": "bump", "createdAt": "2025-01-01T00:00:00Z" }
        })),
    )
    .await;

    // Get updated commit
    let (_, body2) = send_request(
        &router,
        "GET",
        &format!("/xrpc/com.atproto.sync.getLatestCommit?did={did}"),
        None,
        None,
    )
    .await;
    let cid2 = body2["cid"].as_str().unwrap();
    assert_ne!(cid1, cid2, "CID should change after write");
}

#[tokio::test]
async fn list_blobs_after_upload() {
    let (router, _stores) = create_test_router_and_stores().await;
    let (did, jwt, _) = create_account_via_api(&router, "lsblob.test.pds.local").await;

    // Upload a blob
    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/xrpc/com.atproto.repo.uploadBlob")
        .header("authorization", format!("Bearer {jwt}"))
        .header("content-type", "text/plain")
        .body(axum::body::Body::from(b"blob data".to_vec()))
        .unwrap();

    use http_body_util::BodyExt;
    use tower::ServiceExt;
    let resp = router.clone().oneshot(req).await.unwrap();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let upload_body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let cid = upload_body["blob"]["ref"]["$link"].as_str().unwrap();

    // List blobs
    let (status, body) = send_request(
        &router,
        "GET",
        &format!("/xrpc/com.atproto.sync.listBlobs?did={did}"),
        None,
        None,
    )
    .await;
    assert_xrpc_ok(status, &body);
    let cids = body["cids"].as_array().unwrap();
    assert!(cids.iter().any(|c| c.as_str() == Some(cid)));
}

#[tokio::test]
async fn list_repos_includes_account() {
    let (router, _stores) = create_test_router_and_stores().await;
    let (did, _, _) = create_account_via_api(&router, "repos.test.pds.local").await;

    let (status, body) = send_request(
        &router,
        "GET",
        "/xrpc/com.atproto.sync.listRepos",
        None,
        None,
    )
    .await;
    assert_xrpc_ok(status, &body);
    let repos = body["repos"].as_array().unwrap();
    assert_eq!(repos.len(), 1);
    assert_eq!(repos[0]["did"], did);
    assert_eq!(repos[0]["active"], true);
}

#[tokio::test]
async fn get_blob_after_upload() {
    let (router, _stores) = create_test_router_and_stores().await;
    let (did, jwt, _) = create_account_via_api(&router, "getblob.test.pds.local").await;

    // Upload a blob
    let blob_data = b"get blob test data";
    let req = axum::http::Request::builder()
        .method("POST")
        .uri("/xrpc/com.atproto.repo.uploadBlob")
        .header("authorization", format!("Bearer {jwt}"))
        .header("content-type", "application/octet-stream")
        .body(axum::body::Body::from(blob_data.to_vec()))
        .unwrap();

    use http_body_util::BodyExt;
    use tower::ServiceExt;
    let resp = router.clone().oneshot(req).await.unwrap();
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let upload_body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    let cid = upload_body["blob"]["ref"]["$link"].as_str().unwrap();

    // Get blob
    let req = axum::http::Request::builder()
        .method("GET")
        .uri(format!("/xrpc/com.atproto.sync.getBlob?did={did}&cid={cid}"))
        .body(axum::body::Body::empty())
        .unwrap();

    let resp = router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 200);
    let body_bytes = resp.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(&body_bytes[..], blob_data);
}
