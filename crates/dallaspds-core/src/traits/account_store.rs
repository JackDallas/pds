use async_trait::async_trait;

use crate::error::PdsResult;
use crate::types::{ActorAccount, CreateAccountInput, RefreshTokenRecord, RepoRoot};

#[async_trait]
pub trait AccountStore: Send + Sync + 'static {
    async fn create_account(&self, input: &CreateAccountInput) -> PdsResult<ActorAccount>;
    async fn get_account_by_did(&self, did: &str) -> PdsResult<Option<ActorAccount>>;
    async fn get_account_by_handle(&self, handle: &str) -> PdsResult<Option<ActorAccount>>;
    async fn get_account_by_email(&self, email: &str) -> PdsResult<Option<ActorAccount>>;
    async fn update_handle(&self, did: &str, handle: &str) -> PdsResult<()>;
    async fn update_password(&self, did: &str, password_hash: &str) -> PdsResult<()>;
    async fn deactivate_account(&self, did: &str) -> PdsResult<()>;
    async fn activate_account(&self, did: &str) -> PdsResult<()>;
    async fn delete_account(&self, did: &str) -> PdsResult<()>;
    async fn get_repo_root(&self, did: &str) -> PdsResult<Option<RepoRoot>>;
    async fn update_repo_root(&self, did: &str, cid: &[u8], rev: &str) -> PdsResult<()>;
    async fn create_refresh_token(&self, token: &RefreshTokenRecord) -> PdsResult<()>;
    async fn get_refresh_token(&self, id: &str) -> PdsResult<Option<RefreshTokenRecord>>;
    async fn delete_refresh_token(&self, id: &str) -> PdsResult<()>;
    async fn delete_refresh_tokens_for_did(&self, did: &str) -> PdsResult<u64>;
    async fn list_accounts(
        &self,
        cursor: Option<&str>,
        limit: usize,
    ) -> PdsResult<Vec<ActorAccount>>;
}
