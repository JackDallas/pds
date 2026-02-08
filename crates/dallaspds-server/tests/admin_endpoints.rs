use dallaspds_test_utils::*;
use serde_json::json;
use dallaspds_core::config::PdsMode;

#[tokio::test]
async fn deactivate_account() {
    let (router, stores) = create_test_router_and_stores().await;
    let (did, jwt, _) = create_account_via_api(&router, "deact.test.pds.local").await;

    let (status, _) = send_request(
        &router,
        "POST",
        "/xrpc/com.atproto.server.deactivateAccount",
        Some(&jwt),
        None,
    )
    .await;
    assert_eq!(status, 200);

    use dallaspds_core::AccountStore;
    let account = stores.account_store.get_account_by_did(&did).await.unwrap().unwrap();
    assert_eq!(account.status, dallaspds_core::AccountStatus::Deactivated);
}

#[tokio::test]
async fn activate_after_deactivation() {
    let (router, stores) = create_test_router_and_stores().await;
    let (did, jwt, _) = create_account_via_api(&router, "react.test.pds.local").await;

    // Deactivate
    send_request(
        &router,
        "POST",
        "/xrpc/com.atproto.server.deactivateAccount",
        Some(&jwt),
        None,
    )
    .await;

    // Activate
    let (status, _) = send_request(
        &router,
        "POST",
        "/xrpc/com.atproto.server.activateAccount",
        Some(&jwt),
        None,
    )
    .await;
    assert_eq!(status, 200);

    use dallaspds_core::AccountStore;
    let account = stores.account_store.get_account_by_did(&did).await.unwrap().unwrap();
    assert_eq!(account.status, dallaspds_core::AccountStatus::Active);
}

#[tokio::test]
async fn delete_with_correct_password() {
    let (router, stores) = create_test_router_and_stores().await;
    let (did, jwt, _) = create_account_via_api(&router, "delete.test.pds.local").await;

    let (status, _) = send_request(
        &router,
        "POST",
        "/xrpc/com.atproto.server.deleteAccount",
        Some(&jwt),
        Some(json!({
            "did": did,
            "password": TEST_PASSWORD,
        })),
    )
    .await;
    assert_eq!(status, 200);

    use dallaspds_core::AccountStore;
    let account = stores.account_store.get_account_by_did(&did).await.unwrap();
    assert!(account.is_none(), "account should be deleted");
}

#[tokio::test]
async fn delete_wrong_password_fails() {
    let (router, _stores) = create_test_router_and_stores().await;
    let (did, jwt, _) = create_account_via_api(&router, "delfail.test.pds.local").await;

    let (status, body) = send_request(
        &router,
        "POST",
        "/xrpc/com.atproto.server.deleteAccount",
        Some(&jwt),
        Some(json!({
            "did": did,
            "password": "wrong-password",
        })),
    )
    .await;
    assert_xrpc_error(status, &body, 401, "InvalidPassword");
}

// ── Phase 1: Multi-user Admin Tests ────────────────────────────────────

#[tokio::test]
async fn admin_can_create_and_list_invite_codes() {
    let stores = create_test_stores().await;
    
    // Create a router in Multi mode with admin
    let mut config = create_test_config();
    config.mode = PdsMode::Multi;
    let temp_router = create_test_router_with_config(&stores, config.clone());
    
    // Create account to be admin
    let (admin_did, admin_jwt, _) = create_account_via_api(&temp_router, "admin.test.pds.local").await;
    
    // Now create a router with this account as admin
    config.admin_dids = vec![admin_did.clone()];
    let router = create_test_router_with_config(&stores, config);
    
    // Admin creates an invite code
    let (status, body) = send_request(
        &router,
        "POST",
        "/xrpc/com.atproto.server.createInviteCode",
        Some(&admin_jwt),
        Some(json!({
            "useCount": 2,
        })),
    )
    .await;
    assert_xrpc_ok(status, &body);
    let code = body["code"].as_str().unwrap();
    assert!(code.contains('-'), "invite code should have format xxxxx-xxxxx");
    
    // List invite codes
    let (status, body) = send_request(
        &router,
        "GET",
        "/xrpc/com.atproto.server.getAccountInviteCodes",
        Some(&admin_jwt),
        None,
    )
    .await;
    assert_xrpc_ok(status, &body);
    let codes = body["codes"].as_array().unwrap();
    assert_eq!(codes.len(), 1);
    assert_eq!(codes[0]["code"], code);
    assert_eq!(codes[0]["available"], 2);
}

#[tokio::test]
async fn non_admin_gets_403_on_admin_endpoints() {
    let stores = create_test_stores().await;
    
    // Create account without admin privileges in multi mode
    let mut config = create_test_config();
    config.mode = PdsMode::Multi;
    let router = create_test_router_with_config(&stores, config);
    let (did, jwt, _) = create_account_via_api(&router, "user.test.pds.local").await;
    
    // Try to access admin endpoint
    let (status, body) = send_request(
        &router,
        "GET",
        &format!("/xrpc/com.atproto.admin.getAccountInfo?did={}", did),
        Some(&jwt),
        None,
    )
    .await;
    assert_xrpc_error(status, &body, 403, "Forbidden");
}

#[tokio::test]
async fn check_admin_status_endpoint() {
    let stores = create_test_stores().await;
    
    // Create two accounts in multi mode
    let mut config = create_test_config();
    config.mode = PdsMode::Multi;
    let temp_router = create_test_router_with_config(&stores, config.clone());
    
    let (admin_did, admin_jwt, _) = create_account_via_api(&temp_router, "admin.test.pds.local").await;
    let (_, user_jwt, _) = create_account_via_api(&temp_router, "user.test.pds.local").await;
    
    // Build router with admin_did configured
    config.admin_dids = vec![admin_did.clone()];
    let router = create_test_router_with_config(&stores, config);
    
    // Admin checks status
    let (status, body) = send_request(
        &router,
        "GET",
        "/xrpc/com.dallaspds.admin.checkAdminStatus",
        Some(&admin_jwt),
        None,
    )
    .await;
    assert_xrpc_ok(status, &body);
    assert_eq!(body["isAdmin"], true);
    
    // Non-admin checks status
    let (status, body) = send_request(
        &router,
        "GET",
        "/xrpc/com.dallaspds.admin.checkAdminStatus",
        Some(&user_jwt),
        None,
    )
    .await;
    assert_xrpc_ok(status, &body);
    assert_eq!(body["isAdmin"], false);
}

#[tokio::test]
async fn admin_can_search_accounts() {
    let stores = create_test_stores().await;
    
    // Create multiple accounts in multi mode
    let mut config = create_test_config();
    config.mode = PdsMode::Multi;
    let temp_router = create_test_router_with_config(&stores, config.clone());
    
    let (admin_did, admin_jwt, _) = create_account_via_api(&temp_router, "admin.test.pds.local").await;
    create_account_via_api(&temp_router, "alice.test.pds.local").await;
    create_account_via_api(&temp_router, "bob.test.pds.local").await;
    create_account_via_api(&temp_router, "charlie.test.pds.local").await;
    
    // Build router with admin
    config.admin_dids = vec![admin_did.clone()];
    let router = create_test_router_with_config(&stores, config);
    
    // Admin searches for accounts with "ali" in handle
    let (status, body) = send_request(
        &router,
        "GET",
        "/xrpc/com.dallaspds.admin.listAccounts?query=ali",
        Some(&admin_jwt),
        None,
    )
    .await;
    assert_xrpc_ok(status, &body);
    let accounts = body["accounts"].as_array().unwrap();
    assert!(accounts.len() >= 1, "should find at least alice");
    let found_alice = accounts.iter().any(|a| a["handle"].as_str().unwrap().contains("alice"));
    assert!(found_alice, "should find alice account");
}

#[tokio::test]
async fn admin_can_list_all_accounts() {
    let stores = create_test_stores().await;
    
    // Create accounts in multi mode
    let mut config = create_test_config();
    config.mode = PdsMode::Multi;
    let temp_router = create_test_router_with_config(&stores, config.clone());
    
    let (admin_did, admin_jwt, _) = create_account_via_api(&temp_router, "admin.test.pds.local").await;
    create_account_via_api(&temp_router, "user1.test.pds.local").await;
    create_account_via_api(&temp_router, "user2.test.pds.local").await;
    
    // Build router with admin
    config.admin_dids = vec![admin_did.clone()];
    let router = create_test_router_with_config(&stores, config);
    
    // List all accounts (no query)
    let (status, body) = send_request(
        &router,
        "GET",
        "/xrpc/com.dallaspds.admin.listAccounts",
        Some(&admin_jwt),
        None,
    )
    .await;
    assert_xrpc_ok(status, &body);
    let accounts = body["accounts"].as_array().unwrap();
    assert_eq!(accounts.len(), 3, "should have 3 accounts");
}

#[tokio::test]
async fn admin_takedown_and_restore() {
    let stores = create_test_stores().await;
    
    // Create accounts in multi mode
    let mut config = create_test_config();
    config.mode = PdsMode::Multi;
    let temp_router = create_test_router_with_config(&stores, config.clone());
    
    let (admin_did, admin_jwt, _) = create_account_via_api(&temp_router, "admin.test.pds.local").await;
    let (target_did, _, _) = create_account_via_api(&temp_router, "target.test.pds.local").await;
    
    // Build router with admin
    config.admin_dids = vec![admin_did.clone()];
    let router = create_test_router_with_config(&stores, config);
    
    // Admin takes down the account
    let (status, body) = send_request(
        &router,
        "POST",
        "/xrpc/com.atproto.admin.updateSubjectStatus",
        Some(&admin_jwt),
        Some(json!({
            "subject": {
                "did": target_did,
            },
            "takedown": {
                "applied": true,
                "ref": "violation-12345",
            },
        })),
    )
    .await;
    assert_xrpc_ok(status, &body);
    assert_eq!(body["takedown"]["applied"], true);
    assert_eq!(body["takedown"]["ref"], "violation-12345");
    
    // Verify status via getSubjectStatus
    let (status, body) = send_request(
        &router,
        "GET",
        &format!("/xrpc/com.atproto.admin.getSubjectStatus?did={}", target_did),
        Some(&admin_jwt),
        None,
    )
    .await;
    assert_xrpc_ok(status, &body);
    assert_eq!(body["takedown"]["applied"], true);
    assert_eq!(body["takedown"]["ref"], "violation-12345");
    
    // Restore (remove takedown)
    let (status, body) = send_request(
        &router,
        "POST",
        "/xrpc/com.atproto.admin.updateSubjectStatus",
        Some(&admin_jwt),
        Some(json!({
            "subject": {
                "did": target_did,
            },
            "takedown": {
                "applied": false,
            },
        })),
    )
    .await;
    assert_xrpc_ok(status, &body);
    assert_eq!(body["takedown"]["applied"], false);
    
    // Verify restored
    let (status, body) = send_request(
        &router,
        "GET",
        &format!("/xrpc/com.atproto.admin.getSubjectStatus?did={}", target_did),
        Some(&admin_jwt),
        None,
    )
    .await;
    assert_xrpc_ok(status, &body);
    assert_eq!(body["takedown"]["applied"], false);
}

#[tokio::test]
async fn admin_can_get_account_info() {
    let stores = create_test_stores().await;
    
    // Create accounts in multi mode
    let mut config = create_test_config();
    config.mode = PdsMode::Multi;
    let temp_router = create_test_router_with_config(&stores, config.clone());
    
    let (admin_did, admin_jwt, _) = create_account_via_api(&temp_router, "admin.test.pds.local").await;
    let (target_did, _, _) = create_account_via_api(&temp_router, "target.test.pds.local").await;
    
    // Build router with admin
    config.admin_dids = vec![admin_did.clone()];
    let router = create_test_router_with_config(&stores, config);
    
    // Get account info
    let (status, body) = send_request(
        &router,
        "GET",
        &format!("/xrpc/com.atproto.admin.getAccountInfo?did={}", target_did),
        Some(&admin_jwt),
        None,
    )
    .await;
    assert_xrpc_ok(status, &body);
    assert_eq!(body["did"], target_did);
    assert_eq!(body["handle"], "target.test.pds.local");
    assert_eq!(body["status"], "active");
    assert!(body["createdAt"].is_string());
}

#[tokio::test]
async fn invite_code_required_rejects_without_code() {
    let stores = create_test_stores().await;
    
    // Build router with invite_required = true and multi mode
    let mut config = create_test_config();
    config.mode = PdsMode::Multi;
    config.invite_required = true;
    let router = create_test_router_with_config(&stores, config);
    
    // Try to create account without invite code
    let (status, body) = send_request(
        &router,
        "POST",
        "/xrpc/com.atproto.server.createAccount",
        None,
        Some(json!({
            "handle": "user.test.pds.local",
            "email": "user@test.com",
            "password": TEST_PASSWORD,
        })),
    )
    .await;
    assert_xrpc_error(status, &body, 400, "InvalidInviteCode");
}

#[tokio::test]
async fn invite_code_allows_signup() {
    let stores = create_test_stores().await;
    
    // Create admin and invite code in multi mode
    let mut config = create_test_config();
    config.mode = PdsMode::Multi;
    let temp_router = create_test_router_with_config(&stores, config.clone());
    let (admin_did, admin_jwt, _) = create_account_via_api(&temp_router, "admin.test.pds.local").await;
    
    config.admin_dids = vec![admin_did.clone()];
    config.invite_required = true;
    let router = create_test_router_with_config(&stores, config);
    
    // Admin creates invite code
    let (status, body) = send_request(
        &router,
        "POST",
        "/xrpc/com.atproto.server.createInviteCode",
        Some(&admin_jwt),
        Some(json!({
            "useCount": 1,
        })),
    )
    .await;
    assert_xrpc_ok(status, &body);
    let code = body["code"].as_str().unwrap().to_string();
    
    // Use the invite code to create account
    let (status, body) = send_request(
        &router,
        "POST",
        "/xrpc/com.atproto.server.createAccount",
        None,
        Some(json!({
            "handle": "invited.test.pds.local",
            "email": "invited@test.com",
            "password": TEST_PASSWORD,
            "inviteCode": code,
        })),
    )
    .await;
    assert_xrpc_ok(status, &body);
    assert!(body["did"].is_string());
    assert_eq!(body["handle"], "invited.test.pds.local");
}

#[tokio::test]
async fn invite_code_depletes_after_use() {
    let stores = create_test_stores().await;
    
    // Create admin in multi mode
    let mut config = create_test_config();
    config.mode = PdsMode::Multi;
    let temp_router = create_test_router_with_config(&stores, config.clone());
    let (admin_did, admin_jwt, _) = create_account_via_api(&temp_router, "admin.test.pds.local").await;
    
    config.admin_dids = vec![admin_did.clone()];
    config.invite_required = true;
    let router = create_test_router_with_config(&stores, config);
    
    // Create invite code with useCount=1
    let (status, body) = send_request(
        &router,
        "POST",
        "/xrpc/com.atproto.server.createInviteCode",
        Some(&admin_jwt),
        Some(json!({
            "useCount": 1,
        })),
    )
    .await;
    assert_xrpc_ok(status, &body);
    let code = body["code"].as_str().unwrap().to_string();
    
    // Use it once
    let (status, _) = send_request(
        &router,
        "POST",
        "/xrpc/com.atproto.server.createAccount",
        None,
        Some(json!({
            "handle": "user1.test.pds.local",
            "email": "user1@test.com",
            "password": TEST_PASSWORD,
            "inviteCode": code.clone(),
        })),
    )
    .await;
    assert_eq!(status, 200);
    
    // Try to use it again - should fail
    let (status, body) = send_request(
        &router,
        "POST",
        "/xrpc/com.atproto.server.createAccount",
        None,
        Some(json!({
            "handle": "user2.test.pds.local",
            "email": "user2@test.com",
            "password": TEST_PASSWORD,
            "inviteCode": code,
        })),
    )
    .await;
    assert_xrpc_error(status, &body, 400, "InvalidInviteCode");
}

#[tokio::test]
async fn admin_can_create_multiple_invite_codes() {
    let stores = create_test_stores().await;
    
    // Create admin in multi mode
    let mut config = create_test_config();
    config.mode = PdsMode::Multi;
    let temp_router = create_test_router_with_config(&stores, config.clone());
    let (admin_did, admin_jwt, _) = create_account_via_api(&temp_router, "admin.test.pds.local").await;
    
    config.admin_dids = vec![admin_did.clone()];
    let router = create_test_router_with_config(&stores, config);
    
    // Create multiple codes at once
    let (status, body) = send_request(
        &router,
        "POST",
        "/xrpc/com.atproto.server.createInviteCodes",
        Some(&admin_jwt),
        Some(json!({
            "codeCount": 3,
            "useCount": 5,
        })),
    )
    .await;
    assert_xrpc_ok(status, &body);
    let codes_array = body["codes"].as_array().unwrap();
    assert_eq!(codes_array.len(), 1);
    let codes = codes_array[0]["codes"].as_array().unwrap();
    assert_eq!(codes.len(), 3);
    
    // Verify each code is valid format
    for code in codes {
        let code_str = code.as_str().unwrap();
        assert!(code_str.contains('-'), "each code should have xxxxx-xxxxx format");
    }
}

#[tokio::test]
async fn non_admin_cannot_create_invite_codes() {
    let stores = create_test_stores().await;
    
    // Create non-admin user in multi mode
    let mut config = create_test_config();
    config.mode = PdsMode::Multi;
    let router = create_test_router_with_config(&stores, config);
    let (_, jwt, _) = create_account_via_api(&router, "user.test.pds.local").await;
    
    // Try to create invite code
    let (status, body) = send_request(
        &router,
        "POST",
        "/xrpc/com.atproto.server.createInviteCode",
        Some(&jwt),
        Some(json!({
            "useCount": 1,
        })),
    )
    .await;
    assert_xrpc_error(status, &body, 403, "Forbidden");
}

#[tokio::test]
async fn non_admin_cannot_search_accounts() {
    let stores = create_test_stores().await;
    
    // Create non-admin user in multi mode
    let mut config = create_test_config();
    config.mode = PdsMode::Multi;
    let router = create_test_router_with_config(&stores, config);
    let (_, jwt, _) = create_account_via_api(&router, "user.test.pds.local").await;
    
    // Try to search accounts
    let (status, body) = send_request(
        &router,
        "GET",
        "/xrpc/com.dallaspds.admin.listAccounts",
        Some(&jwt),
        None,
    )
    .await;
    assert_xrpc_error(status, &body, 403, "Forbidden");
}
