use async_trait::async_trait;
use aws_sdk_s3::primitives::ByteStream;
use bytes::Bytes;

use dallaspds_core::{BlobStore, PdsError, PdsResult};

#[derive(Clone)]
pub struct S3BlobStore {
    client: aws_sdk_s3::Client,
    bucket: String,
}

impl S3BlobStore {
    /// Create a new S3BlobStore.
    ///
    /// - `bucket`: S3 bucket name
    /// - `region`: AWS region (e.g. "us-east-1")
    /// - `endpoint`: Optional custom endpoint for S3-compatible services (MinIO, R2, etc.)
    pub async fn new(bucket: &str, region: &str, endpoint: Option<&str>) -> PdsResult<Self> {
        let mut config_loader = aws_config::defaults(aws_config::BehaviorVersion::latest())
            .region(aws_config::Region::new(region.to_owned()));

        if let Some(endpoint) = endpoint {
            config_loader = config_loader.endpoint_url(endpoint);
        }

        let sdk_config = config_loader.load().await;

        let s3_config = aws_sdk_s3::config::Builder::from(&sdk_config)
            .force_path_style(true)
            .build();

        let client = aws_sdk_s3::Client::from_conf(s3_config);

        Ok(Self {
            client,
            bucket: bucket.to_owned(),
        })
    }

    /// Convert a DID string into a storage-safe prefix by replacing ':' with '_'.
    fn safe_did(did: &str) -> String {
        did.replace(':', "_")
    }

    /// Return the S3 object key for a blob: {safe_did}/{cid}
    fn object_key(did: &str, cid: &str) -> String {
        format!("{}/{}", Self::safe_did(did), cid)
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
        let key = Self::object_key(did, cid);

        self.client
            .put_object()
            .bucket(&self.bucket)
            .key(&key)
            .content_type(mime_type)
            .body(ByteStream::from(data))
            .send()
            .await
            .map_err(|e| PdsError::Storage(format!("S3 put_object failed: {e}")))?;

        Ok(())
    }

    async fn get_blob(&self, did: &str, cid: &str) -> PdsResult<Option<(Bytes, String)>> {
        let key = Self::object_key(did, cid);

        let result = self
            .client
            .get_object()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await;

        match result {
            Ok(output) => {
                let content_type = output
                    .content_type()
                    .unwrap_or("application/octet-stream")
                    .to_owned();

                let body = output
                    .body
                    .collect()
                    .await
                    .map_err(|e| PdsError::Storage(format!("S3 read body failed: {e}")))?;

                Ok(Some((body.into_bytes(), content_type)))
            }
            Err(sdk_err) => {
                if is_not_found(&sdk_err) {
                    Ok(None)
                } else {
                    Err(PdsError::Storage(format!(
                        "S3 get_object failed: {sdk_err}"
                    )))
                }
            }
        }
    }

    async fn has_blob(&self, did: &str, cid: &str) -> PdsResult<bool> {
        let key = Self::object_key(did, cid);

        let result = self
            .client
            .head_object()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await;

        match result {
            Ok(_) => Ok(true),
            Err(sdk_err) => {
                if is_not_found(&sdk_err) {
                    Ok(false)
                } else {
                    Err(PdsError::Storage(format!(
                        "S3 head_object failed: {sdk_err}"
                    )))
                }
            }
        }
    }

    async fn delete_blob(&self, did: &str, cid: &str) -> PdsResult<()> {
        let key = Self::object_key(did, cid);

        self.client
            .delete_object()
            .bucket(&self.bucket)
            .key(&key)
            .send()
            .await
            .map_err(|e| PdsError::Storage(format!("S3 delete_object failed: {e}")))?;

        Ok(())
    }

    async fn list_blobs(
        &self,
        did: &str,
        cursor: Option<&str>,
        limit: usize,
    ) -> PdsResult<Vec<String>> {
        let prefix = format!("{}/", Self::safe_did(did));

        let mut request = self
            .client
            .list_objects_v2()
            .bucket(&self.bucket)
            .prefix(&prefix)
            .max_keys(limit as i32);

        if let Some(cursor) = cursor {
            // start_after is exclusive — objects with key > start_after are returned
            let start_after_key = Self::object_key(did, cursor);
            request = request.start_after(start_after_key);
        }

        let output = request
            .send()
            .await
            .map_err(|e| PdsError::Storage(format!("S3 list_objects_v2 failed: {e}")))?;

        let cids: Vec<String> = output
            .contents()
            .iter()
            .filter_map(|obj| {
                obj.key().and_then(|key| {
                    // Key is "{safe_did}/{cid}" — extract the CID part
                    key.strip_prefix(&prefix).map(|cid| cid.to_owned())
                })
            })
            .collect();

        Ok(cids)
    }
}

/// Check if an S3 SDK error is a "not found" (NoSuchKey / NotFound).
fn is_not_found<E: std::fmt::Debug>(err: &aws_sdk_s3::error::SdkError<E>) -> bool {
    match err {
        aws_sdk_s3::error::SdkError::ServiceError(service_err) => {
            let raw = service_err.raw();
            matches!(raw.status().as_u16(), 404)
        }
        _ => false,
    }
}
