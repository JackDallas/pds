use async_trait::async_trait;

use crate::error::PdsResult;
use crate::types::{ActorAccount, CreateAccountInput, InviteCode, RefreshTokenRecord, RepoRoot};

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

    // Invite code management
    async fn create_invite_code(
        &self,
        code: &str,
        available_uses: i32,
        for_account: &str,
        created_by: &str,
    ) -> PdsResult<InviteCode>;
    async fn get_invite_code(&self, code: &str) -> PdsResult<Option<InviteCode>>;
    async fn use_invite_code(&self, code: &str, used_by: &str) -> PdsResult<()>;
    async fn list_invite_codes(
        &self,
        cursor: Option<&str>,
        limit: usize,
    ) -> PdsResult<Vec<InviteCode>>;
    async fn list_invite_codes_for_account(&self, did: &str) -> PdsResult<Vec<InviteCode>>;
    async fn disable_invite_code(&self, code: &str) -> PdsResult<()>;

    // Account search and moderation
    async fn search_accounts(
        &self,
        query: Option<&str>,
        cursor: Option<&str>,
        limit: usize,
    ) -> PdsResult<Vec<ActorAccount>>;
    async fn set_takedown(&self, did: &str, takedown_ref: Option<&str>) -> PdsResult<()>;

    // Email token management
    async fn create_email_token(&self, purpose: &str, did: &str, token: &str) -> PdsResult<()>;
    async fn get_email_token(&self, purpose: &str, did: &str) -> PdsResult<Option<(String, chrono::DateTime<chrono::Utc>)>>;
    async fn get_email_token_by_token(&self, purpose: &str, token: &str) -> PdsResult<Option<(String, chrono::DateTime<chrono::Utc>)>>;
    async fn delete_email_token(&self, purpose: &str, did: &str) -> PdsResult<()>;
    async fn confirm_email(&self, did: &str) -> PdsResult<()>;
    async fn update_email(&self, did: &str, email: &str) -> PdsResult<()>;
}
