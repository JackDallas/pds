use async_trait::async_trait;
use chrono::{NaiveDateTime, TimeZone, Utc};
use sqlx::{Row, SqlitePool};

use dallaspds_core::{
    AccountStatus, AccountStore, ActorAccount, CreateAccountInput, InviteCode, InviteCodeUse,
    PdsError, PdsResult, RefreshTokenRecord, RepoRoot,
};

#[derive(Clone)]
pub struct SqliteAccountStore {
    pool: SqlitePool,
}

/// Parse a SQLite datetime text string into a chrono DateTime<Utc>.
///
/// SQLite stores datetimes as TEXT in the format produced by
/// `strftime('%Y-%m-%dT%H:%M:%fZ', 'now')`, which yields strings like
/// `2025-01-01T00:00:00.000Z`.
fn parse_datetime(s: &str) -> Result<chrono::DateTime<Utc>, PdsError> {
    // Try RFC 3339 first (handles the trailing Z)
    if let Ok(dt) = chrono::DateTime::parse_from_rfc3339(s) {
        return Ok(dt.with_timezone(&Utc));
    }
    // Fallback: parse as NaiveDateTime with milliseconds
    if let Ok(naive) = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.fZ") {
        return Ok(Utc.from_utc_datetime(&naive));
    }
    // Fallback: parse without fractional seconds
    if let Ok(naive) = NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%SZ") {
        return Ok(Utc.from_utc_datetime(&naive));
    }
    Err(PdsError::Storage(format!(
        "failed to parse datetime: {s}"
    )))
}

/// Parse an optional SQLite datetime text string.
fn parse_datetime_opt(s: Option<&str>) -> Result<Option<chrono::DateTime<Utc>>, PdsError> {
    match s {
        Some(s) => Ok(Some(parse_datetime(s)?)),
        None => Ok(None),
    }
}

/// Compute the account status from the deactivated_at and takedown_ref fields.
fn compute_status(deactivated_at: &Option<String>, takedown_ref: &Option<String>) -> AccountStatus {
    if takedown_ref.is_some() {
        AccountStatus::Takendown
    } else if deactivated_at.is_some() {
        AccountStatus::Deactivated
    } else {
        AccountStatus::Active
    }
}

/// Map a sqlx Row (from a joined actor + account query) to an ActorAccount.
fn row_to_actor_account(row: &sqlx::sqlite::SqliteRow) -> Result<ActorAccount, PdsError> {
    let did: String = row
        .try_get("did")
        .map_err(|e| PdsError::Storage(e.to_string()))?;
    let handle: Option<String> = row
        .try_get("handle")
        .map_err(|e| PdsError::Storage(e.to_string()))?;
    let email: Option<String> = row
        .try_get("email")
        .map_err(|e| PdsError::Storage(e.to_string()))?;
    let email_confirmed_at: Option<String> = row
        .try_get("email_confirmed_at")
        .map_err(|e| PdsError::Storage(e.to_string()))?;
    let password_hash: String = row
        .try_get("password_hash")
        .map_err(|e| PdsError::Storage(e.to_string()))?;
    let signing_key: Vec<u8> = row
        .try_get("signing_key")
        .map_err(|e| PdsError::Storage(e.to_string()))?;
    let created_at: String = row
        .try_get("created_at")
        .map_err(|e| PdsError::Storage(e.to_string()))?;
    let deactivated_at: Option<String> = row
        .try_get("deactivated_at")
        .map_err(|e| PdsError::Storage(e.to_string()))?;
    let takedown_ref: Option<String> = row
        .try_get("takedown_ref")
        .map_err(|e| PdsError::Storage(e.to_string()))?;
    let delete_after: Option<String> = row
        .try_get("delete_after")
        .map_err(|e| PdsError::Storage(e.to_string()))?;

    let status = compute_status(&deactivated_at, &takedown_ref);

    Ok(ActorAccount {
        did,
        handle,
        email,
        email_confirmed_at: parse_datetime_opt(email_confirmed_at.as_deref())?,
        password_hash,
        signing_key,
        created_at: parse_datetime(&created_at)?,
        status,
        deactivated_at: parse_datetime_opt(deactivated_at.as_deref())?,
        takedown_ref,
        delete_after: parse_datetime_opt(delete_after.as_deref())?,
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

impl SqliteAccountStore {
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
impl AccountStore for SqliteAccountStore {
    async fn create_account(&self, input: &CreateAccountInput) -> PdsResult<ActorAccount> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| PdsError::Storage(e.to_string()))?;

        // Insert into actor table
        sqlx::query("INSERT INTO actor (did, handle) VALUES (?, ?)")
            .bind(&input.did)
            .bind(&input.handle)
            .execute(&mut *tx)
            .await
            .map_err(|e| PdsError::Storage(e.to_string()))?;

        // Insert into account table
        sqlx::query(
            "INSERT INTO account (did, email, password_hash, signing_key) VALUES (?, ?, ?, ?)",
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
        sqlx::query("INSERT INTO repo_root (did, cid, rev) VALUES (?, ?, ?)")
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
        self.get_account_where("a.did = ?", did).await
    }

    async fn get_account_by_handle(&self, handle: &str) -> PdsResult<Option<ActorAccount>> {
        self.get_account_where("a.handle = ?", handle).await
    }

    async fn get_account_by_email(&self, email: &str) -> PdsResult<Option<ActorAccount>> {
        self.get_account_where("ac.email = ?", email).await
    }

    async fn update_handle(&self, did: &str, handle: &str) -> PdsResult<()> {
        sqlx::query("UPDATE actor SET handle = ? WHERE did = ?")
            .bind(handle)
            .bind(did)
            .execute(&self.pool)
            .await
            .map_err(|e| PdsError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn update_password(&self, did: &str, password_hash: &str) -> PdsResult<()> {
        sqlx::query("UPDATE account SET password_hash = ? WHERE did = ?")
            .bind(password_hash)
            .bind(did)
            .execute(&self.pool)
            .await
            .map_err(|e| PdsError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn deactivate_account(&self, did: &str) -> PdsResult<()> {
        sqlx::query(
            "UPDATE actor SET deactivated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE did = ?",
        )
        .bind(did)
        .execute(&self.pool)
        .await
        .map_err(|e| PdsError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn activate_account(&self, did: &str) -> PdsResult<()> {
        sqlx::query("UPDATE actor SET deactivated_at = NULL WHERE did = ?")
            .bind(did)
            .execute(&self.pool)
            .await
            .map_err(|e| PdsError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn delete_account(&self, did: &str) -> PdsResult<()> {
        sqlx::query("DELETE FROM actor WHERE did = ?")
            .bind(did)
            .execute(&self.pool)
            .await
            .map_err(|e| PdsError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn get_repo_root(&self, did: &str) -> PdsResult<Option<RepoRoot>> {
        let row = sqlx::query("SELECT did, cid, rev, indexed_at FROM repo_root WHERE did = ?")
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
                let indexed_at: String = r
                    .try_get("indexed_at")
                    .map_err(|e| PdsError::Storage(e.to_string()))?;

                Ok(Some(RepoRoot {
                    did,
                    cid,
                    rev,
                    indexed_at: parse_datetime(&indexed_at)?,
                }))
            }
            None => Ok(None),
        }
    }

    async fn update_repo_root(&self, did: &str, cid: &[u8], rev: &str) -> PdsResult<()> {
        sqlx::query(
            "INSERT OR REPLACE INTO repo_root (did, cid, rev, indexed_at) VALUES (?, ?, ?, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))",
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
            "INSERT INTO refresh_token (id, did, expires_at, next_id, app_password_name) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(&token.id)
        .bind(&token.did)
        .bind(token.expires_at.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string())
        .bind(&token.next_id)
        .bind(&token.app_password_name)
        .execute(&self.pool)
        .await
        .map_err(|e| PdsError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn get_refresh_token(&self, id: &str) -> PdsResult<Option<RefreshTokenRecord>> {
        let row = sqlx::query(
            "SELECT id, did, expires_at, next_id, app_password_name FROM refresh_token WHERE id = ?",
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
                let expires_at: String = r
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
                    expires_at: parse_datetime(&expires_at)?,
                    next_id,
                    app_password_name,
                }))
            }
            None => Ok(None),
        }
    }

    async fn delete_refresh_token(&self, id: &str) -> PdsResult<()> {
        sqlx::query("DELETE FROM refresh_token WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await
            .map_err(|e| PdsError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn delete_refresh_tokens_for_did(&self, did: &str) -> PdsResult<u64> {
        let result = sqlx::query("DELETE FROM refresh_token WHERE did = ?")
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
            let sql = format!("{ACCOUNT_SELECT} WHERE a.did > ? ORDER BY a.did ASC LIMIT ?");
            sqlx::query(&sql)
                .bind(cursor)
                .bind(limit as i64)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| PdsError::Storage(e.to_string()))?
        } else {
            let sql = format!("{ACCOUNT_SELECT} ORDER BY a.did ASC LIMIT ?");
            sqlx::query(&sql)
                .bind(limit as i64)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| PdsError::Storage(e.to_string()))?
        };

        rows.iter().map(row_to_actor_account).collect()
    }

    // Invite code management (stubs for Phase 2 compatibility)
    async fn create_invite_code(&self, code: &str, available_uses: i32, for_account: &str, created_by: &str) -> PdsResult<InviteCode> {
        sqlx::query("INSERT INTO invite_code (code, available_uses, for_account, created_by) VALUES (?, ?, ?, ?)")
            .bind(code)
            .bind(available_uses)
            .bind(for_account)
            .bind(created_by)
            .execute(&self.pool)
            .await
            .map_err(|e| PdsError::Storage(e.to_string()))?;

        self.get_invite_code(code)
            .await?
            .ok_or_else(|| PdsError::Storage("failed to retrieve invite code after creation".to_string()))
    }

    async fn get_invite_code(&self, code: &str) -> PdsResult<Option<InviteCode>> {
        let row = sqlx::query("SELECT code, available_uses, disabled, for_account, created_by, created_at FROM invite_code WHERE code = ?")
            .bind(code)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| PdsError::Storage(e.to_string()))?;

        let row = match row {
            Some(r) => r,
            None => return Ok(None),
        };

        let code_val: String = row.try_get("code").map_err(|e| PdsError::Storage(e.to_string()))?;
        let available_uses: i32 = row.try_get("available_uses").map_err(|e| PdsError::Storage(e.to_string()))?;
        let disabled_int: i32 = row.try_get("disabled").map_err(|e| PdsError::Storage(e.to_string()))?;
        let for_account: String = row.try_get("for_account").map_err(|e| PdsError::Storage(e.to_string()))?;
        let created_by: String = row.try_get("created_by").map_err(|e| PdsError::Storage(e.to_string()))?;
        let created_at: String = row.try_get("created_at").map_err(|e| PdsError::Storage(e.to_string()))?;

        let use_rows = sqlx::query("SELECT code, used_by, used_at FROM invite_code_use WHERE code = ?")
            .bind(&code_val)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| PdsError::Storage(e.to_string()))?;

        let mut uses = Vec::new();
        for ur in &use_rows {
            let u_code: String = ur.try_get("code").map_err(|e| PdsError::Storage(e.to_string()))?;
            let used_by: String = ur.try_get("used_by").map_err(|e| PdsError::Storage(e.to_string()))?;
            let used_at: String = ur.try_get("used_at").map_err(|e| PdsError::Storage(e.to_string()))?;
            uses.push(InviteCodeUse {
                code: u_code,
                used_by,
                used_at: parse_datetime(&used_at)?,
            });
        }

        Ok(Some(InviteCode {
            code: code_val,
            available_uses,
            disabled: disabled_int != 0,
            for_account,
            created_by,
            created_at: parse_datetime(&created_at)?,
            uses,
        }))
    }

    async fn use_invite_code(&self, code: &str, used_by: &str) -> PdsResult<()> {
        sqlx::query("INSERT INTO invite_code_use (code, used_by) VALUES (?, ?)")
            .bind(code)
            .bind(used_by)
            .execute(&self.pool)
            .await
            .map_err(|e| PdsError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn list_invite_codes(&self, cursor: Option<&str>, limit: usize) -> PdsResult<Vec<InviteCode>> {
        let codes: Vec<String> = if let Some(cursor) = cursor {
            let rows = sqlx::query("SELECT code FROM invite_code WHERE code < ? ORDER BY created_at DESC LIMIT ?")
                .bind(cursor)
                .bind(limit as i64)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| PdsError::Storage(e.to_string()))?;
            rows.iter()
                .map(|r| r.try_get("code").map_err(|e| PdsError::Storage(e.to_string())))
                .collect::<PdsResult<Vec<String>>>()?
        } else {
            let rows = sqlx::query("SELECT code FROM invite_code ORDER BY created_at DESC LIMIT ?")
                .bind(limit as i64)
                .fetch_all(&self.pool)
                .await
                .map_err(|e| PdsError::Storage(e.to_string()))?;
            rows.iter()
                .map(|r| r.try_get("code").map_err(|e| PdsError::Storage(e.to_string())))
                .collect::<PdsResult<Vec<String>>>()?
        };

        let mut result = Vec::new();
        for code in &codes {
            if let Some(invite) = self.get_invite_code(code).await? {
                result.push(invite);
            }
        }
        Ok(result)
    }

    async fn list_invite_codes_for_account(&self, did: &str) -> PdsResult<Vec<InviteCode>> {
        let rows = sqlx::query("SELECT code FROM invite_code WHERE for_account = ? OR created_by = ?")
            .bind(did)
            .bind(did)
            .fetch_all(&self.pool)
            .await
            .map_err(|e| PdsError::Storage(e.to_string()))?;

        let codes: Vec<String> = rows.iter()
            .map(|r| r.try_get("code").map_err(|e| PdsError::Storage(e.to_string())))
            .collect::<PdsResult<Vec<String>>>()?;

        let mut result = Vec::new();
        for code in &codes {
            if let Some(invite) = self.get_invite_code(code).await? {
                result.push(invite);
            }
        }
        Ok(result)
    }

    async fn disable_invite_code(&self, code: &str) -> PdsResult<()> {
        sqlx::query("UPDATE invite_code SET disabled = 1 WHERE code = ?")
            .bind(code)
            .execute(&self.pool)
            .await
            .map_err(|e| PdsError::Storage(e.to_string()))?;
        Ok(())
    }

    // Account search and moderation (stubs for Phase 2 compatibility)
    async fn search_accounts(&self, query: Option<&str>, cursor: Option<&str>, limit: usize) -> PdsResult<Vec<ActorAccount>> {
        let rows = match (query, cursor) {
            (Some(q), Some(cursor)) => {
                let pattern = format!("%{}%", q);
                let sql = format!("{ACCOUNT_SELECT} WHERE (a.handle LIKE ? OR ac.email LIKE ?) AND a.did > ? ORDER BY a.did ASC LIMIT ?");
                sqlx::query(&sql)
                    .bind(&pattern)
                    .bind(&pattern)
                    .bind(cursor)
                    .bind(limit as i64)
                    .fetch_all(&self.pool)
                    .await
                    .map_err(|e| PdsError::Storage(e.to_string()))?
            }
            (Some(q), None) => {
                let pattern = format!("%{}%", q);
                let sql = format!("{ACCOUNT_SELECT} WHERE (a.handle LIKE ? OR ac.email LIKE ?) ORDER BY a.did ASC LIMIT ?");
                sqlx::query(&sql)
                    .bind(&pattern)
                    .bind(&pattern)
                    .bind(limit as i64)
                    .fetch_all(&self.pool)
                    .await
                    .map_err(|e| PdsError::Storage(e.to_string()))?
            }
            (None, Some(cursor)) => {
                let sql = format!("{ACCOUNT_SELECT} WHERE a.did > ? ORDER BY a.did ASC LIMIT ?");
                sqlx::query(&sql)
                    .bind(cursor)
                    .bind(limit as i64)
                    .fetch_all(&self.pool)
                    .await
                    .map_err(|e| PdsError::Storage(e.to_string()))?
            }
            (None, None) => {
                let sql = format!("{ACCOUNT_SELECT} ORDER BY a.did ASC LIMIT ?");
                sqlx::query(&sql)
                    .bind(limit as i64)
                    .fetch_all(&self.pool)
                    .await
                    .map_err(|e| PdsError::Storage(e.to_string()))?
            }
        };

        rows.iter().map(row_to_actor_account).collect()
    }

    async fn set_takedown(&self, did: &str, takedown_ref: Option<&str>) -> PdsResult<()> {
        sqlx::query("UPDATE actor SET takedown_ref = ? WHERE did = ?")
            .bind(takedown_ref)
            .bind(did)
            .execute(&self.pool)
            .await
            .map_err(|e| PdsError::Storage(e.to_string()))?;
        Ok(())
    }

    // Email token management (Phase 2)
    async fn create_email_token(&self, purpose: &str, did: &str, token: &str) -> PdsResult<()> {
        sqlx::query("INSERT OR REPLACE INTO email_token (purpose, did, token) VALUES (?, ?, ?)")
            .bind(purpose)
            .bind(did)
            .bind(token)
            .execute(&self.pool)
            .await
            .map_err(|e| PdsError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn get_email_token(&self, purpose: &str, did: &str) -> PdsResult<Option<(String, chrono::DateTime<chrono::Utc>)>> {
        let row = sqlx::query("SELECT token, requested_at FROM email_token WHERE purpose = ? AND did = ?")
            .bind(purpose)
            .bind(did)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| PdsError::Storage(e.to_string()))?;

        match row {
            Some(ref r) => {
                let token: String = r
                    .try_get("token")
                    .map_err(|e| PdsError::Storage(e.to_string()))?;
                let requested_at: String = r
                    .try_get("requested_at")
                    .map_err(|e| PdsError::Storage(e.to_string()))?;
                
                Ok(Some((token, parse_datetime(&requested_at)?)))
            }
            None => Ok(None),
        }
    }

    async fn get_email_token_by_token(&self, purpose: &str, token: &str) -> PdsResult<Option<(String, chrono::DateTime<chrono::Utc>)>> {
        let row = sqlx::query("SELECT did, requested_at FROM email_token WHERE purpose = ? AND token = ?")
            .bind(purpose)
            .bind(token)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| PdsError::Storage(e.to_string()))?;

        match row {
            Some(ref r) => {
                let did: String = r
                    .try_get("did")
                    .map_err(|e| PdsError::Storage(e.to_string()))?;
                let requested_at: String = r
                    .try_get("requested_at")
                    .map_err(|e| PdsError::Storage(e.to_string()))?;
                
                Ok(Some((did, parse_datetime(&requested_at)?)))
            }
            None => Ok(None),
        }
    }

    async fn delete_email_token(&self, purpose: &str, did: &str) -> PdsResult<()> {
        sqlx::query("DELETE FROM email_token WHERE purpose = ? AND did = ?")
            .bind(purpose)
            .bind(did)
            .execute(&self.pool)
            .await
            .map_err(|e| PdsError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn confirm_email(&self, did: &str) -> PdsResult<()> {
        sqlx::query("UPDATE account SET email_confirmed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now') WHERE did = ?")
            .bind(did)
            .execute(&self.pool)
            .await
            .map_err(|e| PdsError::Storage(e.to_string()))?;
        Ok(())
    }

    async fn update_email(&self, did: &str, email: &str) -> PdsResult<()> {
        sqlx::query("UPDATE account SET email = ? WHERE did = ?")
            .bind(email)
            .bind(did)
            .execute(&self.pool)
            .await
            .map_err(|e| PdsError::Storage(e.to_string()))?;
        Ok(())
    }
}
