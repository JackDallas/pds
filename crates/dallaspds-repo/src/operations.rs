use std::sync::Arc;

use atrium_api::types::string::{Did, Tid};
use atrium_repo::blockstore::AsyncBlockStoreRead;
use atrium_repo::{Cid, Repository};
use dallaspds_core::error::{PdsError, PdsResult};
use dallaspds_core::traits::RepoStore;
use dallaspds_crypto::{SigningKey, TidGenerator};
use futures::TryStreamExt;

use crate::blockstore_adapter::{RepoStoreAdapter, cid_from_bytes, cid_to_bytes};

/// Output returned when a record is created, updated, or put.
#[derive(Debug, Clone)]
pub struct RecordWriteOutput {
    pub uri: String,
    pub cid: Vec<u8>,
    /// New repo root CID bytes after this write, for updating repo_root table.
    pub new_root: Vec<u8>,
    /// New rev string after this write, for updating repo_root table.
    pub new_rev: String,
}

/// Output returned when reading a record.
#[derive(Debug, Clone)]
pub struct RecordOutput {
    pub uri: String,
    pub cid: Vec<u8>,
    pub value: serde_json::Value,
}

/// Create a new repository for a DID, returning `(root_cid_bytes, rev_string)`.
///
/// This creates an empty MST, an initial signed commit, and writes everything
/// to the blockstore via the adapter.
pub async fn create_repo<R: RepoStore>(
    store: Arc<R>,
    did: &str,
    signing_key: &SigningKey,
) -> PdsResult<(Vec<u8>, String)> {
    let mut adapter = RepoStoreAdapter::new(store, did.to_string());

    let atrium_did = Did::new(did.to_string())
        .map_err(|e| PdsError::InvalidRequest(format!("invalid DID: {e}")))?;

    // Create repo builder (empty MST + unsigned commit)
    let builder = Repository::create(&mut adapter, atrium_did)
        .await
        .map_err(|e| PdsError::Storage(format!("failed to create repo: {e}")))?;

    // Sign the initial commit
    let commit_bytes = builder.bytes();
    let sig = signing_key.sign(&commit_bytes)?;

    // Finalize the repo (writes signed commit block)
    let repo = builder
        .finalize(sig)
        .await
        .map_err(|e| PdsError::Storage(format!("failed to finalize repo: {e}")))?;

    let root_cid = repo.root();
    let rev = repo.commit().rev().to_string();
    let root_cid_bytes = cid_to_bytes(&root_cid);

    Ok((root_cid_bytes, rev))
}

/// Create a new record in a repository.
///
/// If `rkey` is `None`, a new TID-based record key is generated.
/// The record is serialized as DAG-CBOR and stored in the MST at
/// `{collection}/{rkey}`.
pub async fn create_record<R: RepoStore>(
    store: Arc<R>,
    did: &str,
    signing_key: &SigningKey,
    collection: &str,
    rkey: Option<&str>,
    record: &serde_json::Value,
    tid_gen: &TidGenerator,
    current_root: &[u8],
) -> PdsResult<RecordWriteOutput> {
    let mut adapter = RepoStoreAdapter::new(store, did.to_string());

    // Parse the current root CID
    let root_cid = cid_from_bytes(current_root)
        .map_err(|e| PdsError::Storage(format!("invalid root CID: {e}")))?;

    // Open the existing repo
    let mut repo = Repository::open(&mut adapter, root_cid)
        .await
        .map_err(|e| PdsError::Storage(format!("failed to open repo: {e}")))?;

    // Generate or validate rkey
    let rkey_str = match rkey {
        Some(k) => k.to_string(),
        None => tid_gen.next_tid(),
    };

    // Build the MST path: "collection/rkey"
    let mst_key = format!("{collection}/{rkey_str}");

    // Write the record block and add to MST
    let (mut commit_builder, record_cid) = repo
        .add_raw(&mst_key, record)
        .await
        .map_err(|e| PdsError::Storage(format!("failed to add record: {e}")))?;

    // Generate a new rev (TID) and set prev
    let rev_str = tid_gen.next_tid();
    let rev_tid = Tid::new(rev_str.clone())
        .map_err(|e| PdsError::InvalidRequest(format!("invalid TID: {e}")))?;
    commit_builder.rev(rev_tid);
    commit_builder.prev(root_cid);

    // Sign and finalize the commit
    let commit_bytes = commit_builder.bytes();
    let sig = signing_key.sign(&commit_bytes)?;
    let new_root_cid = commit_builder
        .finalize(sig)
        .await
        .map_err(|e| PdsError::Storage(format!("failed to finalize commit: {e}")))?;

    let uri = format!("at://{did}/{collection}/{rkey_str}");

    Ok(RecordWriteOutput {
        uri,
        cid: cid_to_bytes(&record_cid),
        new_root: cid_to_bytes(&new_root_cid),
        new_rev: rev_str,
    })
}

/// Get a single record by its AT-URI components.
///
/// Returns `None` if the record does not exist.
pub async fn get_record<R: RepoStore>(
    store: Arc<R>,
    did: &str,
    collection: &str,
    rkey: &str,
    current_root: &[u8],
) -> PdsResult<Option<RecordOutput>> {
    let mut adapter = RepoStoreAdapter::new(store, did.to_string());

    let root_cid = cid_from_bytes(current_root)
        .map_err(|e| PdsError::Storage(format!("invalid root CID: {e}")))?;

    // Open repo, look up the record CID, then drop repo to release adapter borrow
    let maybe_cid = {
        let mut repo = Repository::open(&mut adapter, root_cid)
            .await
            .map_err(|e| PdsError::Storage(format!("failed to open repo: {e}")))?;

        let mst_key = format!("{collection}/{rkey}");
        let mut tree = repo.tree();
        tree.get(&mst_key)
            .await
            .map_err(|e| PdsError::Storage(format!("failed to get record from MST: {e}")))?
    };
    // repo and tree are now dropped, adapter is available again

    match maybe_cid {
        Some(record_cid) => {
            // Read the record block
            let block_data = adapter
                .read_block(record_cid)
                .await
                .map_err(|e| PdsError::Storage(format!("failed to read record block: {e}")))?;

            // Deserialize from DAG-CBOR to a JSON value
            let value: serde_json::Value = serde_ipld_dagcbor::from_reader(&block_data[..])
                .map_err(|e| PdsError::Storage(format!("failed to decode record: {e}")))?;

            let uri = format!("at://{did}/{collection}/{rkey}");
            Ok(Some(RecordOutput {
                uri,
                cid: cid_to_bytes(&record_cid),
                value,
            }))
        }
        None => Ok(None),
    }
}

/// List records in a given collection.
///
/// Returns up to `limit` records, optionally starting after `cursor` (an rkey).
pub async fn list_records<R: RepoStore>(
    store: Arc<R>,
    did: &str,
    collection: &str,
    limit: usize,
    cursor: Option<&str>,
    current_root: &[u8],
) -> PdsResult<Vec<RecordOutput>> {
    let mut adapter = RepoStoreAdapter::new(store, did.to_string());

    let root_cid = cid_from_bytes(current_root)
        .map_err(|e| PdsError::Storage(format!("invalid root CID: {e}")))?;

    let prefix = format!("{collection}/");

    // Collect entries from MST in a scope so repo/tree are dropped before we read blocks
    let entries: Vec<(String, Cid)> = {
        let mut repo = Repository::open(&mut adapter, root_cid)
            .await
            .map_err(|e| PdsError::Storage(format!("failed to open repo: {e}")))?;

        let mut tree = repo.tree();
        let entries_stream = tree.entries_prefixed(&prefix);
        futures::pin_mut!(entries_stream);

        let mut collected = Vec::new();
        while let Some((key, cid)) = entries_stream
            .try_next()
            .await
            .map_err(|e| PdsError::Storage(format!("failed to iterate MST: {e}")))?
        {
            // Extract the rkey from the full MST key
            let rkey = key.strip_prefix(&prefix).unwrap_or(&key);

            // Apply cursor: skip entries until we pass the cursor rkey
            if let Some(cursor_rkey) = cursor {
                if rkey <= cursor_rkey {
                    continue;
                }
            }

            collected.push((key, cid));
            if collected.len() >= limit {
                break;
            }
        }
        collected
    };
    // repo and tree are now dropped, adapter is available again

    // Read each record block
    let mut results = Vec::with_capacity(entries.len());
    for (key, record_cid) in entries {
        let rkey = key.strip_prefix(&prefix).unwrap_or(&key);
        let block_data = adapter
            .read_block(record_cid)
            .await
            .map_err(|e| PdsError::Storage(format!("failed to read record block: {e}")))?;

        let value: serde_json::Value = serde_ipld_dagcbor::from_reader(&block_data[..])
            .map_err(|e| PdsError::Storage(format!("failed to decode record: {e}")))?;

        results.push(RecordOutput {
            uri: format!("at://{did}/{collection}/{rkey}"),
            cid: cid_to_bytes(&record_cid),
            value,
        });
    }

    Ok(results)
}

/// Delete a record from a repository.
///
/// Returns the new root CID bytes and rev string for updating the repo root.
pub async fn delete_record<R: RepoStore>(
    store: Arc<R>,
    did: &str,
    signing_key: &SigningKey,
    collection: &str,
    rkey: &str,
    tid_gen: &TidGenerator,
    current_root: &[u8],
) -> PdsResult<(Vec<u8>, String)> {
    let mut adapter = RepoStoreAdapter::new(store, did.to_string());

    let root_cid = cid_from_bytes(current_root)
        .map_err(|e| PdsError::Storage(format!("invalid root CID: {e}")))?;

    let mut repo = Repository::open(&mut adapter, root_cid)
        .await
        .map_err(|e| PdsError::Storage(format!("failed to open repo: {e}")))?;

    let mst_key = format!("{collection}/{rkey}");

    // Delete from MST
    let mut commit_builder = repo
        .delete_raw(&mst_key)
        .await
        .map_err(|e| PdsError::Storage(format!("failed to delete record: {e}")))?;

    // Generate a new rev and set prev
    let rev_str = tid_gen.next_tid();
    let rev_tid = Tid::new(rev_str.clone())
        .map_err(|e| PdsError::InvalidRequest(format!("invalid TID: {e}")))?;
    commit_builder.rev(rev_tid);
    commit_builder.prev(root_cid);

    // Sign and finalize
    let commit_bytes = commit_builder.bytes();
    let sig = signing_key.sign(&commit_bytes)?;
    let new_root_cid = commit_builder
        .finalize(sig)
        .await
        .map_err(|e| PdsError::Storage(format!("failed to finalize commit: {e}")))?;

    Ok((cid_to_bytes(&new_root_cid), rev_str))
}

/// Create or update a record at a specific rkey.
///
/// If the record already exists at this path, it is updated; otherwise it is created.
pub async fn put_record<R: RepoStore>(
    store: Arc<R>,
    did: &str,
    signing_key: &SigningKey,
    collection: &str,
    rkey: &str,
    record: &serde_json::Value,
    tid_gen: &TidGenerator,
    current_root: &[u8],
) -> PdsResult<RecordWriteOutput> {
    let mut adapter = RepoStoreAdapter::new(store, did.to_string());

    let root_cid = cid_from_bytes(current_root)
        .map_err(|e| PdsError::Storage(format!("invalid root CID: {e}")))?;

    let mut repo = Repository::open(&mut adapter, root_cid)
        .await
        .map_err(|e| PdsError::Storage(format!("failed to open repo: {e}")))?;

    let mst_key = format!("{collection}/{rkey}");

    // Check if the record already exists
    let existing = {
        let mut tree = repo.tree();
        tree.get(&mst_key)
            .await
            .map_err(|e| PdsError::Storage(format!("failed to check existing record: {e}")))?
    };

    let (mut commit_builder, record_cid) = if existing.is_some() {
        // Update existing record
        repo.update_raw(&mst_key, record)
            .await
            .map_err(|e| PdsError::Storage(format!("failed to update record: {e}")))?
    } else {
        // Add new record
        repo.add_raw(&mst_key, record)
            .await
            .map_err(|e| PdsError::Storage(format!("failed to add record: {e}")))?
    };

    // Generate a new rev and set prev
    let rev_str = tid_gen.next_tid();
    let rev_tid = Tid::new(rev_str.clone())
        .map_err(|e| PdsError::InvalidRequest(format!("invalid TID: {e}")))?;
    commit_builder.rev(rev_tid);
    commit_builder.prev(root_cid);

    // Sign and finalize
    let commit_bytes = commit_builder.bytes();
    let sig = signing_key.sign(&commit_bytes)?;
    let new_root_cid = commit_builder
        .finalize(sig)
        .await
        .map_err(|e| PdsError::Storage(format!("failed to finalize commit: {e}")))?;

    let uri = format!("at://{did}/{collection}/{rkey}");

    Ok(RecordWriteOutput {
        uri,
        cid: cid_to_bytes(&record_cid),
        new_root: cid_to_bytes(&new_root_cid),
        new_rev: rev_str,
    })
}
