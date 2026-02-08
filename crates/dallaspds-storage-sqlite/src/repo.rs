use async_trait::async_trait;
use sqlx::{Row, SqlitePool};

use dallaspds_core::{PdsError, PdsResult, RepoStore};

#[derive(Clone)]
pub struct SqliteRepoStore {
    pool: SqlitePool,
}

impl SqliteRepoStore {
    pub async fn connect(url: &str) -> PdsResult<Self> {
        let pool = SqlitePool::connect(url)
            .await
            .map_err(|e| PdsError::Storage(e.to_string()))?;

        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .map_err(|e| PdsError::Storage(e.to_string()))?;

        Ok(Self { pool })
    }
}

#[async_trait]
impl RepoStore for SqliteRepoStore {
    async fn get_block(&self, did: &str, cid: &[u8]) -> PdsResult<Option<Vec<u8>>> {
        let row = sqlx::query("SELECT block FROM repo_block WHERE did = ? AND cid = ?")
            .bind(did)
            .bind(cid)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| PdsError::Storage(e.to_string()))?;

        match row {
            Some(ref r) => {
                let block: Vec<u8> = r
                    .try_get("block")
                    .map_err(|e| PdsError::Storage(e.to_string()))?;
                Ok(Some(block))
            }
            None => Ok(None),
        }
    }

    async fn put_block(&self, did: &str, cid: &[u8], block: &[u8]) -> PdsResult<()> {
        sqlx::query("INSERT OR IGNORE INTO repo_block (did, cid, block) VALUES (?, ?, ?)")
            .bind(did)
            .bind(cid)
            .bind(block)
            .execute(&self.pool)
            .await
            .map_err(|e| PdsError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn has_block(&self, did: &str, cid: &[u8]) -> PdsResult<bool> {
        let row = sqlx::query("SELECT 1 FROM repo_block WHERE did = ? AND cid = ?")
            .bind(did)
            .bind(cid)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| PdsError::Storage(e.to_string()))?;

        Ok(row.is_some())
    }

    async fn get_all_blocks(&self, did: &str) -> PdsResult<Vec<(Vec<u8>, Vec<u8>)>> {
        let rows = sqlx::query("SELECT cid, block FROM repo_block WHERE did = ?")
            .bind(did)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| PdsError::Storage(e.to_string()))?;

        let mut blocks = Vec::with_capacity(rows.len());
        for row in &rows {
            let cid: Vec<u8> = row
                .try_get("cid")
                .map_err(|e| PdsError::Storage(e.to_string()))?;
            let block: Vec<u8> = row
                .try_get("block")
                .map_err(|e| PdsError::Storage(e.to_string()))?;
            blocks.push((cid, block));
        }
        Ok(blocks)
    }

    async fn delete_blocks_for_did(&self, did: &str) -> PdsResult<u64> {
        let result = sqlx::query("DELETE FROM repo_block WHERE did = ?")
            .bind(did)
            .execute(&self.pool)
            .await
            .map_err(|e| PdsError::Storage(e.to_string()))?;
        Ok(result.rows_affected())
    }
}
