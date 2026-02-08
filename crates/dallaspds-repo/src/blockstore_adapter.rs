use std::sync::Arc;

use atrium_repo::blockstore::{AsyncBlockStoreRead, AsyncBlockStoreWrite, SHA2_256};
use atrium_repo::{Cid, Multihash};
use dallaspds_core::traits::RepoStore;
use sha2::{Digest, Sha256};

/// Adapter that bridges our [`RepoStore`] trait to atrium-repo's blockstore traits.
///
/// Scopes all operations to a specific DID, converting between our byte-based
/// CID representation and atrium-repo's `Cid` type.
pub struct RepoStoreAdapter<R: RepoStore> {
    store: Arc<R>,
    did: String,
}

impl<R: RepoStore> RepoStoreAdapter<R> {
    pub fn new(store: Arc<R>, did: String) -> Self {
        Self { store, did }
    }

    /// Returns a reference to the underlying store.
    pub fn store(&self) -> &R {
        &self.store
    }

    /// Returns the DID this adapter is scoped to.
    pub fn did(&self) -> &str {
        &self.did
    }
}

/// Convert an `ipld_core::cid::Cid` to its byte representation for storage.
pub fn cid_to_bytes(cid: &Cid) -> Vec<u8> {
    cid.to_bytes()
}

/// Convert stored CID bytes back to an `ipld_core::cid::Cid`.
pub fn cid_from_bytes(bytes: &[u8]) -> Result<Cid, String> {
    Cid::read_bytes(std::io::Cursor::new(bytes))
        .map_err(|e| format!("invalid CID bytes: {e}"))
}

/// Compute a CID from codec, multihash code, and content bytes.
///
/// Only SHA2-256 is supported (multihash code 0x12).
fn compute_cid(codec: u64, hash_code: u64, contents: &[u8]) -> Result<Cid, atrium_repo::blockstore::Error> {
    if hash_code != SHA2_256 {
        return Err(atrium_repo::blockstore::Error::UnsupportedHash(hash_code));
    }
    let digest = Sha256::digest(contents);
    let mh = Multihash::wrap(hash_code, digest.as_slice())
        .map_err(|e| atrium_repo::blockstore::Error::Other(Box::new(e)))?;
    Ok(Cid::new_v1(codec, mh))
}

impl<R: RepoStore> AsyncBlockStoreRead for RepoStoreAdapter<R> {
    async fn read_block_into(
        &mut self,
        cid: Cid,
        contents: &mut Vec<u8>,
    ) -> Result<(), atrium_repo::blockstore::Error> {
        let cid_bytes = cid_to_bytes(&cid);
        let data = self
            .store
            .get_block(&self.did, &cid_bytes)
            .await
            .map_err(|e| atrium_repo::blockstore::Error::Other(Box::new(e)))?;

        match data {
            Some(block) => {
                contents.extend_from_slice(&block);
                Ok(())
            }
            None => Err(atrium_repo::blockstore::Error::CidNotFound),
        }
    }
}

impl<R: RepoStore> AsyncBlockStoreWrite for RepoStoreAdapter<R> {
    async fn write_block(
        &mut self,
        codec: u64,
        hash: u64,
        contents: &[u8],
    ) -> Result<Cid, atrium_repo::blockstore::Error> {
        let cid = compute_cid(codec, hash, contents)?;
        let cid_bytes = cid_to_bytes(&cid);

        self.store
            .put_block(&self.did, &cid_bytes, contents)
            .await
            .map_err(|e| atrium_repo::blockstore::Error::Other(Box::new(e)))?;

        Ok(cid)
    }
}
