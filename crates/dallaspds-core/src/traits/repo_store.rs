use async_trait::async_trait;

use crate::error::PdsResult;

#[async_trait]
pub trait RepoStore: Send + Sync + 'static {
    async fn get_block(&self, did: &str, cid: &[u8]) -> PdsResult<Option<Vec<u8>>>;
    async fn put_block(&self, did: &str, cid: &[u8], block: &[u8]) -> PdsResult<()>;
    async fn has_block(&self, did: &str, cid: &[u8]) -> PdsResult<bool>;
    async fn get_all_blocks(&self, did: &str) -> PdsResult<Vec<(Vec<u8>, Vec<u8>)>>;
    async fn delete_blocks_for_did(&self, did: &str) -> PdsResult<u64>;
}
