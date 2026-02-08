use dallaspds_test_utils::*;
use serde_json::json;

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
