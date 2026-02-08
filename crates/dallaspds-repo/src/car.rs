use std::sync::Arc;

use atrium_repo::blockstore::{AsyncBlockStoreRead, AsyncBlockStoreWrite, CarStore, SHA2_256};
use atrium_repo::{Cid, Repository};
use dallaspds_core::error::{PdsError, PdsResult};
use dallaspds_core::traits::RepoStore;

use crate::blockstore_adapter::{RepoStoreAdapter, cid_from_bytes};

/// Export the full repository as a CAR file (v1).
///
/// The CAR file contains the commit root as the single root CID,
/// followed by all blocks in the repository (commit, MST nodes, record blocks).
pub async fn export_full_car<R: RepoStore>(
    store: Arc<R>,
    did: &str,
    current_root: &[u8],
) -> PdsResult<Vec<u8>> {
    let mut adapter = RepoStoreAdapter::new(store, did.to_string());

    let root_cid = cid_from_bytes(current_root)
        .map_err(|e| PdsError::Storage(format!("invalid root CID: {e}")))?;

    // Open the repository to get the list of all CIDs to export
    let cids = {
        let mut repo = Repository::open(&mut adapter, root_cid)
            .await
            .map_err(|e| PdsError::Storage(format!("failed to open repo: {e}")))?;

        // export() returns an iterator of all CIDs in the repo (commit + MST + records)
        repo.export()
            .await
            .map_err(|e| PdsError::Storage(format!("failed to export repo CIDs: {e}")))?
            .collect::<Vec<_>>()
    };
    // repo is dropped, adapter is available again

    // Create a CAR file in memory with the root CID
    let mut car_buf = Vec::new();
    let mut car_store =
        CarStore::create_with_roots(std::io::Cursor::new(&mut car_buf), [root_cid])
            .await
            .map_err(|e| PdsError::Storage(format!("failed to create CAR: {e}")))?;

    // Write each block into the CAR
    for cid in cids {
        let block = adapter
            .read_block(cid)
            .await
            .map_err(|e| PdsError::Storage(format!("failed to read block {cid}: {e}")))?;

        car_store
            .write_block(cid.codec(), SHA2_256, &block)
            .await
            .map_err(|e| PdsError::Storage(format!("failed to write block to CAR: {e}")))?;
    }

    // The car_store borrows car_buf via Cursor; drop it to release the borrow
    drop(car_store);

    Ok(car_buf)
}

/// Generate a diff CAR containing only blocks changed since a given revision.
///
/// This compares the current repo state with a previous commit CID and returns
/// a CAR file containing only the new/changed blocks.
///
/// If `since_root` is `None`, this behaves identically to `export_full_car`.
pub async fn generate_diff_car<R: RepoStore>(
    store: Arc<R>,
    did: &str,
    current_root: &[u8],
    since_root: Option<&[u8]>,
) -> PdsResult<Vec<u8>> {
    // If no previous root, just do a full export
    let since_cid = match since_root {
        Some(bytes) => cid_from_bytes(bytes)
            .map_err(|e| PdsError::Storage(format!("invalid since CID: {e}")))?,
        None => return export_full_car(store, did, current_root).await,
    };

    let current_cid = cid_from_bytes(current_root)
        .map_err(|e| PdsError::Storage(format!("invalid current root CID: {e}")))?;

    let mut adapter = RepoStoreAdapter::new(store, did.to_string());

    // Get current repo CIDs
    let current_cids = {
        let mut repo = Repository::open(&mut adapter, current_cid)
            .await
            .map_err(|e| PdsError::Storage(format!("failed to open current repo: {e}")))?;

        repo.export()
            .await
            .map_err(|e| PdsError::Storage(format!("failed to export current CIDs: {e}")))?
            .collect::<std::collections::HashSet<_>>()
    };

    // Get previous repo CIDs
    let previous_cids = {
        let mut repo = Repository::open(&mut adapter, since_cid)
            .await
            .map_err(|e| PdsError::Storage(format!("failed to open previous repo: {e}")))?;

        repo.export()
            .await
            .map_err(|e| PdsError::Storage(format!("failed to export previous CIDs: {e}")))?
            .collect::<std::collections::HashSet<_>>()
    };

    // Diff: blocks in current but not in previous
    let diff_cids: Vec<Cid> = current_cids
        .difference(&previous_cids)
        .copied()
        .collect();

    // Create a CAR with just the diff blocks
    let mut car_buf = Vec::new();
    let mut car_store =
        CarStore::create_with_roots(std::io::Cursor::new(&mut car_buf), [current_cid])
            .await
            .map_err(|e| PdsError::Storage(format!("failed to create diff CAR: {e}")))?;

    for cid in diff_cids {
        let block = adapter
            .read_block(cid)
            .await
            .map_err(|e| PdsError::Storage(format!("failed to read block {cid}: {e}")))?;

        car_store
            .write_block(cid.codec(), SHA2_256, &block)
            .await
            .map_err(|e| PdsError::Storage(format!("failed to write block to CAR: {e}")))?;
    }

    drop(car_store);

    Ok(car_buf)
}
