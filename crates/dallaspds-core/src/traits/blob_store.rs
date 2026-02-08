use async_trait::async_trait;
use bytes::Bytes;

use crate::error::PdsResult;

#[async_trait]
pub trait BlobStore: Send + Sync + 'static {
    async fn put_blob(
        &self,
        did: &str,
        cid: &str,
        data: Bytes,
        mime_type: &str,
    ) -> PdsResult<()>;
    async fn get_blob(&self, did: &str, cid: &str) -> PdsResult<Option<(Bytes, String)>>;
    async fn has_blob(&self, did: &str, cid: &str) -> PdsResult<bool>;
    async fn delete_blob(&self, did: &str, cid: &str) -> PdsResult<()>;
    async fn list_blobs(
        &self,
        did: &str,
        cursor: Option<&str>,
        limit: usize,
    ) -> PdsResult<Vec<String>>;
}
