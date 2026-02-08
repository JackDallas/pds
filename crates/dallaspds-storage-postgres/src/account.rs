use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row};

use dallaspds_core::{
    AccountStatus, AccountStore, ActorAccount, CreateAccountInput, PdsError, PdsResult,
    RefreshTokenRecord, RepoRoot,
};

#[derive(Clone)]
pub struct PostgresAccountStore {
    pool: PgPool,
}

/// Compute the account status from the deactivated_at and takedown_ref fields.
fn compute_status(
    deactivated_at: &Option<DateTime<Utc>>,
    takedown_ref: &Option<String>,
) -> AccountStatus {
    if takedown_ref.is_some() {
        AccountStatus::Takendown
    } else if deactivated_at.is_some() {
        AccountStatus::Deactivated
    } else {
        AccountStatus::Active
    }
}

/// Map a sqlx Row (from a joined actor + account query) to an ActorAccount.
fn row_to_actor_account(row: &sqlx::postgres::PgRow) -> Result<ActorAccount, PdsError> {
    let did: String = row
        .try_get("did")
        .map_err(|e| PdsError::Storage(e.to_string()))?;
    let handle: Option<String> = row
        .try_get("handle")
        .map_err(|e| PdsError::Storage(e.to_string()))?;
    let email: Option<String> = row
        .try_get("email")
        .map_err(|e| PdsError::Storage(e.to_string()))?;
    let email_confirmed_at: Option<DateTime<Utc>> = row
        .try_get("email_confirmed_at")
        .map_err(|e| PdsError::Storage(e.to_string()))?;
    let password_hash: String = row
        .try_get("password_hash")
        .map_err(|e| PdsError::Storage(e.to_string()))?;
    let signing_key: Vec<u8> = row
        .try_get("signing_key")
        .map_err(|e| PdsError::Storage(e.to_string()))?;
    let created_at: DateTime<Utc> = row
        .try_get("created_at")
        .map_err(|e| PdsError::Storage(e.to_string()))?;
    let deactivated_at: Option<DateTime<Utc>> = row
        .try_get("deactivated_at")
        .map_err(|e| PdsError::Storage(e.to_string()))?;
    let takedown_ref: Option<String> = row
        .try_get("takedown_ref")
        .map_err(|e| PdsError::Storage(e.to_string()))?;
    let delete_after: Option<DateTime<Utc>> = row
        .try_get("delete_after")
        .map_err(|e| PdsError::Storage(e.to_string()))?;

    let status = compute_status(&deactivated_at, &takedown_ref);

    Ok(ActorAccount {
        did,
        handle,
        email,
        email_confirmed_at,
        password_hash,
        signing_key,
        created_at,
        status,
        deactivated_at,
        takedown_ref,
        delete_after,
    })
}

/// SQL fragment for the joined actor + account SELECT.
const ACCOUNT_SELECT: &str = r#"
    SELECT
        a.did,
        a.handle,
        a.created_at,
        a.takedown_ref,
        a.deactivated_at,
        a.delete_after,
        ac.email,
        ac.email_confirmed_at,
        ac.password_hash,
        ac.signing_key
    FROM actor a
    INNER JOIN account ac ON a.did = ac.did
"#;

impl PostgresAccountStore {
    pub async fn connect(url: &str) -> PdsResult<Self> {
        let pool = PgPool::connect(url)
            .await
            .map_err(|e| PdsError::Storage(e.to_string()))?;

        sqlx::migrate!("./migrations")
            .run(&pool)
            .await
            .map_err(|e| PdsError::Storage(e.to_string()))?;

        Ok(Self { pool })
    }

    /// Helper: fetch an ActorAccount with a WHERE clause appended to the base SELECT.
    async fn get_account_where(
        &self,
        where_clause: &str,
        bind_value: &str,
    ) -> PdsResult<Option<ActorAccount>> {
        let sql = format!("{ACCOUNT_SELECT} WHERE {where_clause}");
        let row = sqlx::query(&sql)
            .bind(bind_value)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| PdsError::Storage(e.to_string()))?;

        match row {
            Some(ref r) => Ok(Some(row_to_actor_account(r)?)),
            None => Ok(None),
        }
    }
}

#[async_trait]
impl AccountStore for PostgresAccountStore {
    async fn create_account(&self, input: &CreateAccountInput) -> PdsResult<ActorAccount> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| PdsError::Storage(e.to_string()))?;

        // Insert into actor table
        sqlx::query("INSERT INTO actor (did, handle) VALUES ($1, $2)")
            .bind(&input.did)
            .bind(&input.handle)
            .execute(&mut *tx)
            .await
            .map_err(|e| PdsError::Storage(e.to_string()))?;

        // Insert into account table
        sqlx::query(
            "INSERT INTO account (did, email, password_hash, signing_key) VALUES ($1, $2, $3, $4)",
        )
        .bind(&input.did)
        .bind(&input.email)
        .bind(&input.password_hash)
        .bind(&input.signing_key)
        .execute(&mut *tx)
        .await
        .map_err(|e| PdsError::Storage(e.to_string()))?;

        // Insert into repo_root with empty cid and rev
        let empty_cid: &[u8] = &[];
        sqlx::query("INSERT INTO repo_root (did, cid, rev) VALUES ($1, $2, $3)")
            .bind(&input.did)
            .bind(empty_cid)
            .bind("")
            .execute(&mut *tx)
            .await
            .map_err(|e| PdsError::Storage(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| PdsError::Storage(e.to_string()))?;

        // Query back the full ActorAccount
        self.get_account_by_did(&input.did)
            .await?
            .ok_or_else(|| {
                PdsError::Storage("failed to retrieve account after creation".to_string())
            })
    }

    async fn get_account_by_did(&self, did: &str) -> PdsResult<Option<ActorAccount>> {
        self.get_account_where("a.did = $1", did).await
    }

    async fn get_account_by_handle(&self, handle: &str) -> PdsResult<Option<ActorAccount>> {
        self.get_account_where("a.handle = $1", handle).await
    }

    async fn get_account_by_email(&self, email: &str) -> PdsResult<Option<ActorAccount>> {
        self.get_account_where("ac.email = $1", email).await
    }

    async fn update_handle(&self, did: &str, handle: &str) -> PdsResult<()> {
        sqlx::query("UPDATE actor SET handle = $1 WHERE did = $2")
            .bind(handle)
            .bind(did)
            .execute(&self.pool)
            .await
            .map_err(|e| PdsError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn update_password(&self, did: &str, password_hash: &str) -> PdsResult<()> {
        sqlx::query("UPDATE account SET password_hash = $1 WHERE did = $2")
            .bind(password_hash)
            .bind(did)
            .execute(&self.pool)
            .await
            .map_err(|e| PdsError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn deactivate_account(&self, did: &str) -> PdsResult<()> {
        sqlx::query("UPDATE actor SET deactivated_at = NOW() WHERE did = $1")
            .bind(did)
            .execute(&self.pool)
            .await
            .map_err(|e| PdsError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn activate_account(&self, did: &str) -> PdsResult<()> {
        sqlx::query("UPDATE actor SET deactivated_at = NULL WHERE did = $1")
            .bind(did)
            .execute(&self.pool)
            .await
            .map_err(|e| PdsError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn delete_account(&self, did: &str) -> PdsResult<()> {
        sqlx::query("DELETE FROM actor WHERE did = $1")
            .bind(did)
            .execute(&self.pool)
            .await
            .map_err(|e| PdsError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn get_repo_root(&self, did: &str) -> PdsResult<Option<RepoRoot>> {
        let row =
            sqlx::query("SELECT did, cid, rev, indexed_at FROM repo_root WHERE did = $1")
                .bind(did)
                .fetch_optional(&self.pool)
                .await
                .map_err(|e| PdsError::Storage(e.to_string()))?;

        match row {
            Some(ref r) => {
                let did: String = r
                    .try_get("did")
                    .map_err(|e| PdsError::Storage(e.to_string()))?;
                let cid: Vec<u8> = r
                    .try_get("cid")
                    .map_err(|e| PdsError::Storage(e.to_string()))?;
                let rev: String = r
                    .try_get("rev")
                    .map_err(|e| PdsError::Storage(e.to_string()))?;
                let indexed_at: DateTime<Utc> = r
                    .try_get("indexed_at")
                    .map_err(|e| PdsError::Storage(e.to_string()))?;

                Ok(Some(RepoRoot {
                    did,
                    cid,
                    rev,
                    indexed_at,
                }))
            }
            None => Ok(None),
        }
    }

    async fn update_repo_root(&self, did: &str, cid: &[u8], rev: &str) -> PdsResult<()> {
        sqlx::query(
            "INSERT INTO repo_root (did, cid, rev, indexed_at) VALUES ($1, $2, $3, NOW()) \
             ON CONFLICT (did) DO UPDATE SET cid = $2, rev = $3, indexed_at = NOW()",
        )
        .bind(did)
        .bind(cid)
        .bind(rev)
        .execute(&self.pool)
        .await
        .map_err(|e| PdsError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn create_refresh_token(&self, token: &RefreshTokenRecord) -> PdsResult<()> {
        sqlx::query(
            "INSERT INTO refresh_token (id, did, expires_at, next_id, app_password_name) VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(&token.id)
        .bind(&token.did)
        .bind(token.expires_at)
        .bind(&token.next_id)
        .bind(&token.app_password_name)
        .execute(&self.pool)
        .await
        .map_err(|e| PdsError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn get_refresh_token(&self, id: &str) -> PdsResult<Option<RefreshTokenRecord>> {
        let row = sqlx::query(
            "SELECT id, did, expires_at, next_id, app_password_name FROM refresh_token WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| PdsError::Storage(e.to_string()))?;

        match row {
            Some(ref r) => {
                let id: String = r
                    .try_get("id")
                    .map_err(|e| PdsError::Storage(e.to_string()))?;
                let did: String = r
                    .try_get("did")
                    .map_err(|e| PdsError::Storage(e.to_string()))?;
                let expires_at: DateTime<Utc> = r
                    .try_get("expires_at")
                    .map_err(|e| PdsError::Storage(e.to_string()))?;
                let next_id: Option<String> = r
                    .try_get("next_id")
                    .map_err(|e| PdsError::Storage(e.to_string()))?;
                let app_password_name: Option<String> = r
                    .try_get("app_password_name")
                    .map_err(|e| PdsError::Storage(e.to_string()))?;

                Ok(Some(RefreshTokenRecord {
                    id,
                    did,
                    expires_at,
                    next_id,
                    app_password_name,
                }))
            }
            None => Ok(None),
        }
    }

    async fn delete_refresh_token(&self, id: &str) -> PdsResult<()> {
        sqlx::query("DELETE FROM refresh_token WHERE id = $1")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| PdsError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn delete_refresh_tokens_for_did(&self, did: &str) -> PdsResult<u64> {
        let result = sqlx::query("DELETE FROM refresh_token WHERE did = $1")
            .bind(did)
            .execute(&self.pool)
            .await
            .map_err(|e| PdsError::Storage(e.to_string()))?;
        Ok(result.rows_affected())
    }

    async fn list_accounts(
        &self,
        cursor: Option<&str>,
        limit: usize,
    ) -> PdsResult<Vec<ActorAccount>> {
        let rows = if let Some(cursor) = cursor {
            let sql =
                format!("{ACCOUNT_SELECT} WHERE a.did > $1 ORDER BY a.did ASC LIMIT $2");
            sqlx::query(&sql)
                .bind(cursor)
                .bind(limit as i64)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| PdsError::Storage(e.to_string()))?
        } else {
            let sql = format!("{ACCOUNT_SELECT} ORDER BY a.did ASC LIMIT $1");
            sqlx::query(&sql)
                .bind(limit as i64)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| PdsError::Storage(e.to_string()))?
        };

        rows.iter().map(row_to_actor_account).collect()
    }
}
