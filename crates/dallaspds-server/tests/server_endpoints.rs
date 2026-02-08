use dallaspds_test_utils::*;
use serde_json::json;

// ── describeServer ──────────────────────────────────────────────────────

#[tokio::test]
async fn describe_server_returns_available_domains() {
    let (router, _stores) = create_test_router_and_stores().await;
    let (status, body) = send_request(
        &router,
        "GET",
        "/xrpc/com.atproto.server.describeServer",
        None,
        None,
    )
    .await;
    assert_xrpc_ok(status, &body);
    assert!(body["availableUserDomains"].is_array());
    let domains = body["availableUserDomains"].as_array().unwrap();
    assert!(!domains.is_empty());
}

// ── createAccount ───────────────────────────────────────────────────────

#[tokio::test]
async fn create_account_success() {
    let (router, _stores) = create_test_router_and_stores().await;
    let (did, access_jwt, refresh_jwt) =
        create_account_via_api(&router, "alice.test.pds.local").await;
    assert!(did.starts_with("did:plc:"));
    assert!(!access_jwt.is_empty());
    assert!(!refresh_jwt.is_empty());
}

#[tokio::test]
async fn create_account_returns_tokens_and_did() {
    let (router, _stores) = create_test_router_and_stores().await;
    let (status, body) = send_request(
        &router,
        "POST",
        "/xrpc/com.atproto.server.createAccount",
        None,
        Some(json!({
            "handle": "bob.test.pds.local",
            "email": "bob@test.com",
            "password": TEST_PASSWORD,
        })),
    )
    .await;
    assert_xrpc_ok(status, &body);
    assert!(body["did"].as_str().is_some());
    assert!(body["accessJwt"].as_str().is_some());
    assert!(body["refreshJwt"].as_str().is_some());
    assert_eq!(body["handle"], "bob.test.pds.local");
}

#[tokio::test]
async fn create_account_invalid_handle_rejected() {
    let (router, _stores) = create_test_router_and_stores().await;
    let (status, body) = send_request(
        &router,
        "POST",
        "/xrpc/com.atproto.server.createAccount",
        None,
        Some(json!({
            "handle": "alice.wrong-domain.com",
            "password": TEST_PASSWORD,
        })),
    )
    .await;
    assert_xrpc_error(status, &body, 400, "InvalidHandle");
}

#[tokio::test]
async fn single_mode_second_account_rejected() {
    let (router, _stores) = create_test_router_and_stores().await;
    // Create first account
    create_account_via_api(&router, "first.test.pds.local").await;

    // Try second account — should fail in single-user mode
    let (status, body) = send_request(
        &router,
        "POST",
        "/xrpc/com.atproto.server.createAccount",
        None,
        Some(json!({
            "handle": "second.test.pds.local",
            "password": TEST_PASSWORD,
        })),
    )
    .await;
    assert_xrpc_error(status, &body, 400, "AccountLimitReached");
}

#[tokio::test]
async fn create_account_initializes_repo() {
    let (router, stores) = create_test_router_and_stores().await;
    let (did, _, _) = create_account_via_api(&router, "repo.test.pds.local").await;

    use dallaspds_core::AccountStore;
    let root = stores.account_store.get_repo_root(&did).await.unwrap().unwrap();
    assert!(!root.cid.is_empty(), "repo root CID should be initialized");
    assert!(!root.rev.is_empty(), "repo rev should be initialized");
}

// ── createSession ───────────────────────────────────────────────────────

#[tokio::test]
async fn create_session_by_handle() {
    let (router, _stores) = create_test_router_and_stores().await;
    create_account_via_api(&router, "login.test.pds.local").await;

    let (status, body) = send_request(
        &router,
        "POST",
        "/xrpc/com.atproto.server.createSession",
        None,
        Some(json!({
            "identifier": "login.test.pds.local",
            "password": TEST_PASSWORD,
        })),
    )
    .await;
    assert_xrpc_ok(status, &body);
    assert!(body["accessJwt"].as_str().is_some());
}

#[tokio::test]
async fn create_session_by_email() {
    let (router, _stores) = create_test_router_and_stores().await;
    create_account_via_api(&router, "email.test.pds.local").await;

    let (status, body) = send_request(
        &router,
        "POST",
        "/xrpc/com.atproto.server.createSession",
        None,
        Some(json!({
            "identifier": "email@test.com",
            "password": TEST_PASSWORD,
        })),
    )
    .await;
    assert_xrpc_ok(status, &body);
    assert!(body["accessJwt"].as_str().is_some());
}

#[tokio::test]
async fn create_session_wrong_password() {
    let (router, _stores) = create_test_router_and_stores().await;
    create_account_via_api(&router, "wrong.test.pds.local").await;

    let (status, body) = send_request(
        &router,
        "POST",
        "/xrpc/com.atproto.server.createSession",
        None,
        Some(json!({
            "identifier": "wrong.test.pds.local",
            "password": "bad-password",
        })),
    )
    .await;
    assert_xrpc_error(status, &body, 401, "InvalidPassword");
}

#[tokio::test]
async fn create_session_nonexistent_account() {
    let (router, _stores) = create_test_router_and_stores().await;
    let (status, body) = send_request(
        &router,
        "POST",
        "/xrpc/com.atproto.server.createSession",
        None,
        Some(json!({
            "identifier": "nobody.test.pds.local",
            "password": "password",
        })),
    )
    .await;
    assert_xrpc_error(status, &body, 400, "AccountNotFound");
}

// ── getSession ──────────────────────────────────────────────────────────

#[tokio::test]
async fn get_session_authenticated() {
    let (router, _stores) = create_test_router_and_stores().await;
    let (did, jwt, _) = create_account_via_api(&router, "sess.test.pds.local").await;

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

#[tokio::test]
async fn get_session_no_auth_fails() {
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

// ── refreshSession ──────────────────────────────────────────────────────

#[tokio::test]
async fn refresh_session_returns_new_tokens() {
    let (router, _stores) = create_test_router_and_stores().await;
    let (_, _, refresh_jwt) = create_account_via_api(&router, "refresh.test.pds.local").await;

    let (status, body) = send_request(
        &router,
        "POST",
        "/xrpc/com.atproto.server.refreshSession",
        Some(&refresh_jwt),
        None,
    )
    .await;
    assert_xrpc_ok(status, &body);
    assert!(body["accessJwt"].as_str().is_some());
    assert!(body["refreshJwt"].as_str().is_some());
}

#[tokio::test]
async fn refresh_session_invalid_token() {
    let (router, _stores) = create_test_router_and_stores().await;
    let (status, body) = send_request(
        &router,
        "POST",
        "/xrpc/com.atproto.server.refreshSession",
        Some("not-a-valid-jwt"),
        None,
    )
    .await;
    assert_xrpc_error(status, &body, 401, "InvalidToken");
}

// ── deleteSession ───────────────────────────────────────────────────────

#[tokio::test]
async fn delete_session_clears_tokens() {
    let (router, _stores) = create_test_router_and_stores().await;
    let (_, access_jwt, _) = create_account_via_api(&router, "delsess.test.pds.local").await;

    let (status, _) = send_request(
        &router,
        "POST",
        "/xrpc/com.atproto.server.deleteSession",
        Some(&access_jwt),
        None,
    )
    .await;
    assert_eq!(status, 200);
}

// ── Email Confirmation ──────────────────────────────────────────────────

#[tokio::test]
async fn confirm_email_with_valid_token() {
    use dallaspds_core::AccountStore;

    let (router, stores) = create_test_router_and_stores().await;
    let (did, access_jwt, _) = create_account_via_api(&router, "confirm.test.pds.local").await;

    // Store a confirm_email token directly via the store
    stores
        .account_store
        .create_email_token("confirm_email", &did, "test-token-123")
        .await
        .unwrap();

    // Call confirmEmail with valid token + auth
    let (status, body) = send_request(
        &router,
        "POST",
        "/xrpc/com.atproto.server.confirmEmail",
        Some(&access_jwt),
        Some(json!({
            "email": "confirm@test.com",
            "token": "test-token-123",
        })),
    )
    .await;
    assert_xrpc_ok(status, &body);

    // Verify emailConfirmed via getSession
    let (status, body) = send_request(
        &router,
        "GET",
        "/xrpc/com.atproto.server.getSession",
        Some(&access_jwt),
        None,
    )
    .await;
    assert_xrpc_ok(status, &body);
    assert_eq!(body["emailConfirmed"], true);
}

#[tokio::test]
async fn confirm_email_invalid_token_rejected() {
    use dallaspds_core::AccountStore;

    let (router, stores) = create_test_router_and_stores().await;
    let (did, access_jwt, _) = create_account_via_api(&router, "badtok.test.pds.local").await;

    // Store a token
    stores
        .account_store
        .create_email_token("confirm_email", &did, "real-token-abc")
        .await
        .unwrap();

    // Call confirmEmail with wrong token
    let (status, body) = send_request(
        &router,
        "POST",
        "/xrpc/com.atproto.server.confirmEmail",
        Some(&access_jwt),
        Some(json!({
            "email": "badtok@test.com",
            "token": "wrong-token-xyz",
        })),
    )
    .await;
    assert_xrpc_error(status, &body, 400, "InvalidToken");
}

// ── Password Reset ──────────────────────────────────────────────────────

#[tokio::test]
async fn reset_password_flow() {
    use dallaspds_core::AccountStore;

    let (router, stores) = create_test_router_and_stores().await;
    let (did, _, _) = create_account_via_api(&router, "reset.test.pds.local").await;

    // Store a reset_password token directly
    stores
        .account_store
        .create_email_token("reset_password", &did, "reset-token-456")
        .await
        .unwrap();

    // Call resetPassword with valid token and new password
    let new_password = "new-password-123";
    let (status, body) = send_request(
        &router,
        "POST",
        "/xrpc/com.atproto.server.resetPassword",
        None,
        Some(json!({
            "token": "reset-token-456",
            "password": new_password,
        })),
    )
    .await;
    assert_xrpc_ok(status, &body);

    // Verify can login with new password
    let (status, body) = send_request(
        &router,
        "POST",
        "/xrpc/com.atproto.server.createSession",
        None,
        Some(json!({
            "identifier": "reset.test.pds.local",
            "password": new_password,
        })),
    )
    .await;
    assert_xrpc_ok(status, &body);
    assert!(body["accessJwt"].as_str().is_some());

    // Verify old password no longer works
    let (status, body) = send_request(
        &router,
        "POST",
        "/xrpc/com.atproto.server.createSession",
        None,
        Some(json!({
            "identifier": "reset.test.pds.local",
            "password": TEST_PASSWORD,
        })),
    )
    .await;
    assert_xrpc_error(status, &body, 401, "InvalidPassword");
}

#[tokio::test]
async fn request_password_reset_unknown_email_200() {
    let (router, _stores) = create_test_router_and_stores().await;

    // Call requestPasswordReset with unknown email — should still return 200
    let (status, body) = send_request(
        &router,
        "POST",
        "/xrpc/com.atproto.server.requestPasswordReset",
        None,
        Some(json!({
            "email": "unknown@example.com",
        })),
    )
    .await;
    assert_xrpc_ok(status, &body);
}

// ── Email Update ────────────────────────────────────────────────────────

#[tokio::test]
async fn update_email_without_smtp() {
    let (router, _stores) = create_test_router_and_stores().await;
    let (_, access_jwt, _) = create_account_via_api(&router, "updeml.test.pds.local").await;

    // Call updateEmail — no token needed since no SMTP configured
    let (status, body) = send_request(
        &router,
        "POST",
        "/xrpc/com.atproto.server.updateEmail",
        Some(&access_jwt),
        Some(json!({
            "email": "new@test.com",
        })),
    )
    .await;
    assert_xrpc_ok(status, &body);

    // Verify email changed via getSession
    let (status, body) = send_request(
        &router,
        "GET",
        "/xrpc/com.atproto.server.getSession",
        Some(&access_jwt),
        None,
    )
    .await;
    assert_xrpc_ok(status, &body);
    assert_eq!(body["email"], "new@test.com");
}

#[tokio::test]
async fn request_email_update_returns_token_not_required() {
    let (router, _stores) = create_test_router_and_stores().await;
    let (_, access_jwt, _) = create_account_via_api(&router, "reqeml.test.pds.local").await;

    // Call requestEmailUpdate — since no SMTP, tokenRequired should be false
    let (status, body) = send_request(
        &router,
        "POST",
        "/xrpc/com.atproto.server.requestEmailUpdate",
        Some(&access_jwt),
        None,
    )
    .await;
    assert_xrpc_ok(status, &body);
    assert_eq!(body["tokenRequired"], false);
}
