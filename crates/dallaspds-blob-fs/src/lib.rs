use async_trait::async_trait;
use bytes::Bytes;
use std::path::PathBuf;

use dallaspds_core::{BlobStore, PdsError, PdsResult};

#[derive(Clone)]
pub struct FsBlobStore {
    base_path: PathBuf,
}

impl FsBlobStore {
    pub fn new(path: &str) -> PdsResult<Self> {
        let base_path = PathBuf::from(path);
        std::fs::create_dir_all(&base_path)
            .map_err(|e| PdsError::Storage(format!("failed to create blob directory: {e}")))?;
        Ok(Self { base_path })
    }

    /// Convert a DID string into a filesystem-safe directory name by replacing ':' with '_'.
    fn safe_did(did: &str) -> String {
        did.replace(':', "_")
    }

    /// Return the directory path for a given DID: {base_path}/{safe_did}/
    fn did_dir(&self, did: &str) -> PathBuf {
        self.base_path.join(Self::safe_did(did))
    }

    /// Return the blob file path: {base_path}/{safe_did}/{cid}
    fn blob_path(&self, did: &str, cid: &str) -> PathBuf {
        self.did_dir(did).join(cid)
    }

    /// Return the metadata file path: {base_path}/{safe_did}/{cid}.meta
    fn meta_path(&self, did: &str, cid: &str) -> PathBuf {
        self.did_dir(did).join(format!("{cid}.meta"))
    }
}

#[async_trait]
impl BlobStore for FsBlobStore {
    async fn put_blob(
        &self,
        did: &str,
        cid: &str,
        data: Bytes,
        mime_type: &str,
    ) -> PdsResult<()> {
        let dir = self.did_dir(did);
        tokio::fs::create_dir_all(&dir)
            .await
            .map_err(|e| PdsError::Storage(format!("failed to create DID directory: {e}")))?;

        let blob_path = self.blob_path(did, cid);
        tokio::fs::write(&blob_path, &data)
            .await
            .map_err(|e| PdsError::Storage(format!("failed to write blob: {e}")))?;

        let meta_path = self.meta_path(did, cid);
        tokio::fs::write(&meta_path, mime_type.as_bytes())
            .await
            .map_err(|e| PdsError::Storage(format!("failed to write blob metadata: {e}")))?;

        Ok(())
    }

    async fn get_blob(&self, did: &str, cid: &str) -> PdsResult<Option<(Bytes, String)>> {
        let blob_path = self.blob_path(did, cid);
        let meta_path = self.meta_path(did, cid);

        let data = match tokio::fs::read(&blob_path).await {
            Ok(data) => data,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => {
                return Err(PdsError::Storage(format!("failed to read blob: {e}")));
            }
        };

        let mime_type = match tokio::fs::read_to_string(&meta_path).await {
            Ok(mime) => mime,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // Data file exists but meta file is missing — treat as not found
                return Ok(None);
            }
            Err(e) => {
                return Err(PdsError::Storage(format!(
                    "failed to read blob metadata: {e}"
                )));
            }
        };

        Ok(Some((Bytes::from(data), mime_type)))
    }

    async fn has_blob(&self, did: &str, cid: &str) -> PdsResult<bool> {
        let blob_path = self.blob_path(did, cid);
        match tokio::fs::metadata(&blob_path).await {
            Ok(_) => Ok(true),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(e) => Err(PdsError::Storage(format!(
                "failed to check blob existence: {e}"
            ))),
        }
    }

    async fn delete_blob(&self, did: &str, cid: &str) -> PdsResult<()> {
        let blob_path = self.blob_path(did, cid);
        let meta_path = self.meta_path(did, cid);

        // Remove the blob data file; ignore NotFound errors
        match tokio::fs::remove_file(&blob_path).await {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(PdsError::Storage(format!("failed to delete blob: {e}")));
            }
        }

        // Remove the metadata file; ignore NotFound errors
        match tokio::fs::remove_file(&meta_path).await {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(PdsError::Storage(format!(
                    "failed to delete blob metadata: {e}"
                )));
            }
        }

        Ok(())
    }

    async fn list_blobs(
        &self,
        did: &str,
        cursor: Option<&str>,
        limit: usize,
    ) -> PdsResult<Vec<String>> {
        let dir = self.did_dir(did);

        let mut entries = match tokio::fs::read_dir(&dir).await {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                // DID directory doesn't exist — no blobs
                return Ok(Vec::new());
            }
            Err(e) => {
                return Err(PdsError::Storage(format!(
                    "failed to list blob directory: {e}"
                )));
            }
        };

        let mut cids = Vec::new();
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| PdsError::Storage(format!("failed to read directory entry: {e}")))?
        {
            let file_name = entry.file_name();
            let name = file_name.to_string_lossy();

            // Skip .meta files — we only list blob data files
            if name.ends_with(".meta") {
                continue;
            }

            cids.push(name.into_owned());
        }

        // Sort for deterministic ordering and cursor-based pagination
        cids.sort();

        // Apply cursor: skip entries up to and including the cursor value
        let cids = if let Some(cursor) = cursor {
            cids.into_iter()
                .filter(|cid| cid.as_str() > cursor)
                .collect()
        } else {
            cids
        };

        // Apply limit
        let cids: Vec<String> = cids.into_iter().take(limit).collect();

        Ok(cids)
    }
}
