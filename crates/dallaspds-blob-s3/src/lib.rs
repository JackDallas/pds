use async_trait::async_trait;
use bytes::Bytes;

use dallaspds_core::{BlobStore, PdsResult};

pub struct S3BlobStore {
    bucket: String,
    region: String,
}

impl S3BlobStore {
    pub async fn new() -> PdsResult<Self> {
        todo!()
    }
}

#[async_trait]
impl BlobStore for S3BlobStore {
    async fn put_blob(
        &self,
        did: &str,
        cid: &str,
        data: Bytes,
        mime_type: &str,
    ) -> PdsResult<()> {
        todo!()
    }

    async fn get_blob(&self, did: &str, cid: &str) -> PdsResult<Option<(Bytes, String)>> {
        todo!()
    }

    async fn has_blob(&self, did: &str, cid: &str) -> PdsResult<bool> {
        todo!()
    }

    async fn delete_blob(&self, did: &str, cid: &str) -> PdsResult<()> {
        todo!()
    }

    async fn list_blobs(
        &self,
        did: &str,
        cursor: Option<&str>,
        limit: usize,
    ) -> PdsResult<Vec<String>> {
        todo!()
    }
}
