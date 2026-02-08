use dallaspds_test_utils::{assert_xrpc_ok, create_test_router_and_stores, send_request};

#[tokio::test]
async fn health_returns_200_with_version() {
    let (router, _stores) = create_test_router_and_stores().await;
    let (status, body) = send_request(&router, "GET", "/xrpc/_health", None, None).await;
    assert_xrpc_ok(status, &body);
    assert_eq!(body["version"], "0.1.0");
}
