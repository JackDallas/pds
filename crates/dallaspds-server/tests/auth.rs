use dallaspds_test_utils::*;

#[tokio::test]
async fn missing_auth_header_401() {
    let (router, _stores) = create_test_router_and_stores().await;
    let (status, body) = send_request(
        &router,
        "GET",
        "/xrpc/com.atproto.server.getSession",
        None,
        None,
    )
    .await;
    assert_xrpc_error(status, &body, 401, "AuthenticationRequired");
}

#[tokio::test]
async fn invalid_bearer_401() {
    let (router, _stores) = create_test_router_and_stores().await;
    let (status, body) = send_request(
        &router,
        "GET",
        "/xrpc/com.atproto.server.getSession",
        Some("totally-not-a-jwt"),
        None,
    )
    .await;
    assert_xrpc_error(status, &body, 401, "InvalidToken");
}

#[tokio::test]
async fn expired_token_401() {
    let (router, _stores) = create_test_router_and_stores().await;

    // Create an expired token manually
    use jsonwebtoken::{EncodingKey, Header, encode};
    let now = chrono::Utc::now().timestamp();
    let claims = serde_json::json!({
        "sub": "did:plc:expired",
        "iat": now - 7200,
        "exp": now - 3600,
    });
    let key = EncodingKey::from_secret(TEST_ACCESS_SECRET.as_bytes());
    let token = encode(&Header::default(), &claims, &key).unwrap();

    let (status, body) = send_request(
        &router,
        "GET",
        "/xrpc/com.atproto.server.getSession",
        Some(&token),
        None,
    )
    .await;
    assert_xrpc_error(status, &body, 401, "ExpiredToken");
}

#[tokio::test]
async fn valid_token_extracts_did() {
    let (router, _stores) = create_test_router_and_stores().await;
    let (did, jwt, _) = create_account_via_api(&router, "auth.test.pds.local").await;

    let (status, body) = send_request(
        &router,
        "GET",
        "/xrpc/com.atproto.server.getSession",
        Some(&jwt),
        None,
    )
    .await;
    assert_xrpc_ok(status, &body);
    assert_eq!(body["did"], did);
}
