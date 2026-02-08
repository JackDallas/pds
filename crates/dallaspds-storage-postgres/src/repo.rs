use async_trait::async_trait;
use sqlx::PgPool;

use dallaspds_core::{PdsResult, RepoStore};

pub struct PostgresRepoStore {
    pool: PgPool,
}

impl PostgresRepoStore {
    pub async fn connect(url: &str) -> PdsResult<Self> {
        todo!()
    }
}

#[async_trait]
impl RepoStore for PostgresRepoStore {
    async fn get_block(&self, did: &str, cid: &[u8]) -> PdsResult<Option<Vec<u8>>> {
        todo!()
    }

    async fn put_block(&self, did: &str, cid: &[u8], block: &[u8]) -> PdsResult<()> {
        todo!()
    }

    async fn has_block(&self, did: &str, cid: &[u8]) -> PdsResult<bool> {
        todo!()
    }

    async fn get_all_blocks(&self, did: &str) -> PdsResult<Vec<(Vec<u8>, Vec<u8>)>> {
        todo!()
    }

    async fn delete_blocks_for_did(&self, did: &str) -> PdsResult<u64> {
        todo!()
    }
}
