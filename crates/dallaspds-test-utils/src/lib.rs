pub mod assertions;
pub mod server;
pub mod stores;

pub use assertions::{assert_xrpc_error, assert_xrpc_ok};
pub use server::{
    TEST_ACCESS_SECRET, TEST_PASSWORD, TEST_REFRESH_SECRET,
    create_account_via_api, create_test_app_state, create_test_router,
    create_test_router_and_stores, send_request,
};
pub use stores::{TestStores, create_test_stores};

#[cfg(test)]
mod tests {
    use super::*;
    use dallaspds_core::AccountStore;

    #[tokio::test]
    async fn test_stores_are_usable() {
        let stores = create_test_stores().await;

        // Verify we can query an empty account store
        let result = stores.account_store.list_accounts(None, 10).await.unwrap();
        assert!(result.is_empty());
    }
}
