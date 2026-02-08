use async_trait::async_trait;
use sqlx::PgPool;

use dallaspds_core::{
    AccountStore, ActorAccount, CreateAccountInput, PdsResult, RefreshTokenRecord, RepoRoot,
};

pub struct PostgresAccountStore {
    pool: PgPool,
}

impl PostgresAccountStore {
    pub async fn connect(url: &str) -> PdsResult<Self> {
        todo!()
    }
}

#[async_trait]
impl AccountStore for PostgresAccountStore {
    async fn create_account(&self, input: &CreateAccountInput) -> PdsResult<ActorAccount> {
        todo!()
    }

    async fn get_account_by_did(&self, did: &str) -> PdsResult<Option<ActorAccount>> {
        todo!()
    }

    async fn get_account_by_handle(&self, handle: &str) -> PdsResult<Option<ActorAccount>> {
        todo!()
    }

    async fn get_account_by_email(&self, email: &str) -> PdsResult<Option<ActorAccount>> {
        todo!()
    }

    async fn update_handle(&self, did: &str, handle: &str) -> PdsResult<()> {
        todo!()
    }

    async fn update_password(&self, did: &str, password_hash: &str) -> PdsResult<()> {
        todo!()
    }

    async fn deactivate_account(&self, did: &str) -> PdsResult<()> {
        todo!()
    }

    async fn activate_account(&self, did: &str) -> PdsResult<()> {
        todo!()
    }

    async fn delete_account(&self, did: &str) -> PdsResult<()> {
        todo!()
    }

    async fn get_repo_root(&self, did: &str) -> PdsResult<Option<RepoRoot>> {
        todo!()
    }

    async fn update_repo_root(&self, did: &str, cid: &[u8], rev: &str) -> PdsResult<()> {
        todo!()
    }

    async fn create_refresh_token(&self, token: &RefreshTokenRecord) -> PdsResult<()> {
        todo!()
    }

    async fn get_refresh_token(&self, id: &str) -> PdsResult<Option<RefreshTokenRecord>> {
        todo!()
    }

    async fn delete_refresh_token(&self, id: &str) -> PdsResult<()> {
        todo!()
    }

    async fn delete_refresh_tokens_for_did(&self, did: &str) -> PdsResult<u64> {
        todo!()
    }

    async fn list_accounts(
        &self,
        cursor: Option<&str>,
        limit: usize,
    ) -> PdsResult<Vec<ActorAccount>> {
        todo!()
    }
}
