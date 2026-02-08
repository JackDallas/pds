use dallaspds_test_utils::*;
use serde_json::json;

#[tokio::test]
async fn resolve_handle_local() {
    let (router, _stores) = create_test_router_and_stores().await;
    let (did, _, _) = create_account_via_api(&router, "resolve.test.pds.local").await;

    let (status, body) = send_request(
        &router,
        "GET",
        "/xrpc/com.atproto.identity.resolveHandle?handle=resolve.test.pds.local",
        None,
        None,
    )
    .await;
    assert_xrpc_ok(status, &body);
    assert_eq!(body["did"], did);
}

#[tokio::test]
async fn resolve_unknown_404() {
    let (router, _stores) = create_test_router_and_stores().await;
    let (status, body) = send_request(
        &router,
        "GET",
        "/xrpc/com.atproto.identity.resolveHandle?handle=unknown.test.pds.local",
        None,
        None,
    )
    .await;
    assert_xrpc_error(status, &body, 404, "HandleNotFound");
}

#[tokio::test]
async fn update_handle_success() {
    let (router, stores) = create_test_router_and_stores().await;
    let (did, jwt, _) = create_account_via_api(&router, "oldh.test.pds.local").await;

    let (status, _) = send_request(
        &router,
        "POST",
        "/xrpc/com.atproto.identity.updateHandle",
        Some(&jwt),
        Some(json!({
            "handle": "newh.test.pds.local",
        })),
    )
    .await;
    assert_eq!(status, 200);

    // Verify update took effect
    use dallaspds_core::AccountStore;
    let account = stores.account_store.get_account_by_did(&did).await.unwrap().unwrap();
    assert_eq!(account.handle.as_deref(), Some("newh.test.pds.local"));
}

#[tokio::test]
async fn well_known_atproto_did() {
    let (router, _stores) = create_test_router_and_stores().await;
    let (did, _, _) = create_account_via_api(&router, "wk.test.pds.local").await;

    let req = axum::http::Request::builder()
        .method("GET")
        .uri("/.well-known/atproto-did")
        .body(axum::body::Body::empty())
        .unwrap();

    use http_body_util::BodyExt;
    use tower::ServiceExt;
    let resp = router.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), 200);
    let bytes = resp.into_body().collect().await.unwrap().to_bytes();
    let text = String::from_utf8(bytes.to_vec()).unwrap();
    assert_eq!(text, did);
}
