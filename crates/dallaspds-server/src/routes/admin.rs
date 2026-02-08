use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;

use crate::auth::{AdminAuth, AuthenticatedUser};
use crate::error::XrpcError;
use crate::state::AppState;
use dallaspds_core::traits::*;
use dallaspds_core::PdsError;

// ---------------------------------------------------------------------------
// 1. deleteAccount
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct DeleteAccountRequest {
    pub did: String,
    pub password: String,
    #[serde(default)]
    pub token: Option<String>,
}

pub async fn delete_account<A, R, B>(
    State(state): State<AppState<A, R, B>>,
    user: AuthenticatedUser,
    Json(body): Json<DeleteAccountRequest>,
) -> Result<StatusCode, XrpcError>
where
    A: AccountStore,
    R: RepoStore,
    B: BlobStore,
{
    // Verify the DID matches the authenticated user.
    if body.did != user.did {
        return Err(XrpcError::new(
            StatusCode::FORBIDDEN,
            "AuthorizationError",
            "Token did not match account DID",
        ));
    }

    // Verify password.
    let account = state
        .account_store
        .get_account_by_did(&user.did)
        .await?
        .ok_or(PdsError::AccountNotFound)?;

    let valid = dallaspds_crypto::verify_password(&body.password, &account.password_hash)
        .map_err(|e| XrpcError::new(StatusCode::INTERNAL_SERVER_ERROR, "InternalServerError", e.to_string()))?;
    if !valid {
        return Err(PdsError::InvalidPassword.into());
    }

    // Delete associated data.
    state.repo_store.delete_blocks_for_did(&user.did).await?;
    state.account_store.delete_refresh_tokens_for_did(&user.did).await?;
    state.account_store.delete_account(&user.did).await?;

    // Emit account event.
    if let Some(ref sequencer) = state.sequencer {
        use crate::firehose::events::{AccountEvent, FirehoseEvent};
        let seq = sequencer.next_seq();
        let event = FirehoseEvent::Account(AccountEvent {
            seq,
            did: user.did.clone(),
            time: chrono::Utc::now().to_rfc3339(),
            active: false,
            status: Some("deleted".to_string()),
        });
        crate::firehose::emit::emit_and_persist(&state, event).await;
    }

    Ok(StatusCode::OK)
}

// ---------------------------------------------------------------------------
// 2. deactivateAccount
// ---------------------------------------------------------------------------

pub async fn deactivate_account<A, R, B>(
    State(state): State<AppState<A, R, B>>,
    user: AuthenticatedUser,
) -> Result<StatusCode, XrpcError>
where
    A: AccountStore,
    R: RepoStore,
    B: BlobStore,
{
    state
        .account_store
        .deactivate_account(&user.did)
        .await?;

    // Emit account event.
    if let Some(ref sequencer) = state.sequencer {
        use crate::firehose::events::{AccountEvent, FirehoseEvent};
        let seq = sequencer.next_seq();
        let event = FirehoseEvent::Account(AccountEvent {
            seq,
            did: user.did.clone(),
            time: chrono::Utc::now().to_rfc3339(),
            active: false,
            status: Some("deactivated".to_string()),
        });
        crate::firehose::emit::emit_and_persist(&state, event).await;
    }

    Ok(StatusCode::OK)
}

// ---------------------------------------------------------------------------
// 3. activateAccount
// ---------------------------------------------------------------------------

pub async fn activate_account<A, R, B>(
    State(state): State<AppState<A, R, B>>,
    user: AuthenticatedUser,
) -> Result<StatusCode, XrpcError>
where
    A: AccountStore,
    R: RepoStore,
    B: BlobStore,
{
    state
        .account_store
        .activate_account(&user.did)
        .await?;

    // Emit account event.
    if let Some(ref sequencer) = state.sequencer {
        use crate::firehose::events::{AccountEvent, FirehoseEvent};
        let seq = sequencer.next_seq();
        let event = FirehoseEvent::Account(AccountEvent {
            seq,
            did: user.did.clone(),
            time: chrono::Utc::now().to_rfc3339(),
            active: true,
            status: None,
        });
        crate::firehose::emit::emit_and_persist(&state, event).await;
    }

    Ok(StatusCode::OK)
}

// ---------------------------------------------------------------------------
// 4. get_account_info
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct GetAccountInfoQuery {
    pub did: String,
}

pub async fn get_account_info<A, R, B>(
    State(state): State<AppState<A, R, B>>,
    _admin: AdminAuth,
    Query(params): Query<GetAccountInfoQuery>,
) -> Result<Json<serde_json::Value>, XrpcError>
where
    A: AccountStore,
    R: RepoStore,
    B: BlobStore,
{
    let account = state
        .account_store
        .get_account_by_did(&params.did)
        .await?
        .ok_or(PdsError::AccountNotFound)?;

    Ok(Json(serde_json::json!({
        "did": account.did,
        "handle": account.handle,
        "email": account.email,
        "emailConfirmedAt": account.email_confirmed_at.map(|dt| dt.to_rfc3339()),
        "createdAt": account.created_at.to_rfc3339(),
        "status": match account.status {
            dallaspds_core::types::AccountStatus::Active => "active",
            dallaspds_core::types::AccountStatus::Deactivated => "deactivated",
            dallaspds_core::types::AccountStatus::Takendown => "takendown",
            dallaspds_core::types::AccountStatus::Suspended => "suspended",
            dallaspds_core::types::AccountStatus::Deleted => "deleted",
        },
        "deactivatedAt": account.deactivated_at.map(|dt| dt.to_rfc3339()),
        "takedownRef": account.takedown_ref,
        "deleteAfter": account.delete_after.map(|dt| dt.to_rfc3339()),
    })))
}

// ---------------------------------------------------------------------------
// 5. get_subject_status
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct GetSubjectStatusQuery {
    pub did: String,
}

pub async fn get_subject_status<A, R, B>(
    State(state): State<AppState<A, R, B>>,
    _admin: AdminAuth,
    Query(params): Query<GetSubjectStatusQuery>,
) -> Result<Json<serde_json::Value>, XrpcError>
where
    A: AccountStore,
    R: RepoStore,
    B: BlobStore,
{
    let account = state
        .account_store
        .get_account_by_did(&params.did)
        .await?
        .ok_or(PdsError::AccountNotFound)?;

    let takedown = if account.takedown_ref.is_some() {
        serde_json::json!({
            "applied": true,
            "ref": account.takedown_ref,
        })
    } else {
        serde_json::json!({
            "applied": false,
        })
    };

    Ok(Json(serde_json::json!({
        "subject": {
            "$type": "com.atproto.admin.defs#repoRef",
            "did": account.did,
        },
        "takedown": takedown,
        "deactivated": matches!(account.status, dallaspds_core::types::AccountStatus::Deactivated),
    })))
}

// ---------------------------------------------------------------------------
// 6. update_subject_status
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct UpdateSubjectStatusRequest {
    pub subject: SubjectRef,
    #[serde(default)]
    pub takedown: Option<TakedownStatus>,
}

#[derive(Debug, Deserialize)]
pub struct SubjectRef {
    pub did: String,
}

#[derive(Debug, Deserialize)]
pub struct TakedownStatus {
    #[serde(default)]
    pub applied: bool,
    #[serde(default)]
    pub r#ref: Option<String>,
}

pub async fn update_subject_status<A, R, B>(
    State(state): State<AppState<A, R, B>>,
    _admin: AdminAuth,
    Json(body): Json<UpdateSubjectStatusRequest>,
) -> Result<Json<serde_json::Value>, XrpcError>
where
    A: AccountStore,
    R: RepoStore,
    B: BlobStore,
{
    let did = &body.subject.did;

    // Check if account exists
    state
        .account_store
        .get_account_by_did(did)
        .await?
        .ok_or(PdsError::AccountNotFound)?;

    // Update takedown status if provided
    if let Some(takedown) = body.takedown {
        if takedown.applied {
            state
                .account_store
                .set_takedown(did, takedown.r#ref.as_deref())
                .await?;
        } else {
            state
                .account_store
                .set_takedown(did, None)
                .await?;
        }
    }

    // Fetch updated account
    let account = state
        .account_store
        .get_account_by_did(did)
        .await?
        .ok_or(PdsError::AccountNotFound)?;

    let takedown = if account.takedown_ref.is_some() {
        serde_json::json!({
            "applied": true,
            "ref": account.takedown_ref,
        })
    } else {
        serde_json::json!({
            "applied": false,
        })
    };

    Ok(Json(serde_json::json!({
        "subject": {
            "$type": "com.atproto.admin.defs#repoRef",
            "did": account.did,
        },
        "takedown": takedown,
    })))
}

// ---------------------------------------------------------------------------
// 7. create_invite_code_endpoint
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateInviteCodeRequest {
    pub use_count: i32,
    #[serde(default)]
    pub for_account: Option<String>,
}

pub async fn create_invite_code_endpoint<A, R, B>(
    State(state): State<AppState<A, R, B>>,
    admin: AdminAuth,
    Json(body): Json<CreateInviteCodeRequest>,
) -> Result<Json<serde_json::Value>, XrpcError>
where
    A: AccountStore,
    R: RepoStore,
    B: BlobStore,
{
    // Generate random code: {5chars}-{5chars}
    let code = generate_invite_code();

    let for_account = body.for_account.as_deref().unwrap_or("");
    
    state
        .account_store
        .create_invite_code(&code, body.use_count, for_account, &admin.did)
        .await?;

    Ok(Json(serde_json::json!({
        "code": code,
    })))
}

// ---------------------------------------------------------------------------
// 8. create_invite_codes_endpoint
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateInviteCodesRequest {
    pub code_count: i32,
    pub use_count: i32,
}

pub async fn create_invite_codes_endpoint<A, R, B>(
    State(state): State<AppState<A, R, B>>,
    admin: AdminAuth,
    Json(body): Json<CreateInviteCodesRequest>,
) -> Result<Json<serde_json::Value>, XrpcError>
where
    A: AccountStore,
    R: RepoStore,
    B: BlobStore,
{
    let mut codes = Vec::new();

    for _ in 0..body.code_count {
        let code = generate_invite_code();
        state
            .account_store
            .create_invite_code(&code, body.use_count, "", &admin.did)
            .await?;
        codes.push(code);
    }

    Ok(Json(serde_json::json!({
        "codes": [{
            "account": "",
            "codes": codes,
        }],
    })))
}

// ---------------------------------------------------------------------------
// 9. get_account_invite_codes
// ---------------------------------------------------------------------------

pub async fn get_account_invite_codes<A, R, B>(
    State(state): State<AppState<A, R, B>>,
    user: AuthenticatedUser,
) -> Result<Json<serde_json::Value>, XrpcError>
where
    A: AccountStore,
    R: RepoStore,
    B: BlobStore,
{
    let invite_codes = state
        .account_store
        .list_invite_codes_for_account(&user.did)
        .await?;

    let codes: Vec<serde_json::Value> = invite_codes
        .into_iter()
        .map(|ic| {
            serde_json::json!({
                "code": ic.code,
                "available": ic.available_uses - ic.uses.len() as i32,
                "disabled": ic.disabled,
                "forAccount": ic.for_account,
                "createdBy": ic.created_by,
                "createdAt": ic.created_at.to_rfc3339(),
                "uses": ic.uses.iter().map(|u| serde_json::json!({
                    "usedBy": u.used_by,
                    "usedAt": u.used_at.to_rfc3339(),
                })).collect::<Vec<_>>(),
            })
        })
        .collect();

    Ok(Json(serde_json::json!({
        "codes": codes,
    })))
}

// ---------------------------------------------------------------------------
// 10. list_accounts_admin
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct ListAccountsQuery {
    #[serde(default)]
    pub query: Option<String>,
    #[serde(default)]
    pub cursor: Option<String>,
    #[serde(default)]
    pub limit: Option<usize>,
}

pub async fn list_accounts_admin<A, R, B>(
    State(state): State<AppState<A, R, B>>,
    _admin: AdminAuth,
    Query(params): Query<ListAccountsQuery>,
) -> Result<Json<serde_json::Value>, XrpcError>
where
    A: AccountStore,
    R: RepoStore,
    B: BlobStore,
{
    let limit = params.limit.unwrap_or(50).min(100);
    
    let accounts = state
        .account_store
        .search_accounts(
            params.query.as_deref(),
            params.cursor.as_deref(),
            limit,
        )
        .await?;

    let cursor = if accounts.len() >= limit {
        accounts.last().map(|a| a.did.clone())
    } else {
        None
    };

    let accounts_json: Vec<serde_json::Value> = accounts
        .into_iter()
        .map(|a| {
            serde_json::json!({
                "did": a.did,
                "handle": a.handle,
                "email": a.email,
                "createdAt": a.created_at.to_rfc3339(),
                "status": match a.status {
                    dallaspds_core::types::AccountStatus::Active => "active",
                    dallaspds_core::types::AccountStatus::Deactivated => "deactivated",
                    dallaspds_core::types::AccountStatus::Takendown => "takendown",
                    dallaspds_core::types::AccountStatus::Suspended => "suspended",
                    dallaspds_core::types::AccountStatus::Deleted => "deleted",
                },
            })
        })
        .collect();

    Ok(Json(serde_json::json!({
        "accounts": accounts_json,
        "cursor": cursor,
    })))
}

// ---------------------------------------------------------------------------
// 11. check_admin_status
// ---------------------------------------------------------------------------

pub async fn check_admin_status<A, R, B>(
    State(state): State<AppState<A, R, B>>,
    user: AuthenticatedUser,
) -> Result<Json<serde_json::Value>, XrpcError>
where
    A: AccountStore,
    R: RepoStore,
    B: BlobStore,
{
    let is_admin = state.config.admin_dids.contains(&user.did);

    Ok(Json(serde_json::json!({
        "isAdmin": is_admin,
    })))
}

// ---------------------------------------------------------------------------
// Helper: Generate invite code
// ---------------------------------------------------------------------------

fn generate_invite_code() -> String {
    use rand::Rng;
    
    let alphabet = b"abcdefghijklmnopqrstuvwxyz234567";
    let mut rng = rand::thread_rng();
    
    let part1: String = (0..5)
        .map(|_| {
            let idx = rng.gen_range(0..alphabet.len());
            alphabet[idx] as char
        })
        .collect();
    
    let part2: String = (0..5)
        .map(|_| {
            let idx = rng.gen_range(0..alphabet.len());
            alphabet[idx] as char
        })
        .collect();
    
    format!("{}-{}", part1, part2)
}

// ---------------------------------------------------------------------------
// 12. list_invite_codes_admin
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct ListInviteCodesQuery {
    #[serde(default)]
    pub cursor: Option<String>,
    #[serde(default)]
    pub limit: Option<usize>,
}

pub async fn list_invite_codes_admin<A, R, B>(
    State(state): State<AppState<A, R, B>>,
    _admin: AdminAuth,
    Query(params): Query<ListInviteCodesQuery>,
) -> Result<Json<serde_json::Value>, XrpcError>
where
    A: AccountStore,
    R: RepoStore,
    B: BlobStore,
{
    let limit = params.limit.unwrap_or(50).min(100);

    let invite_codes = state
        .account_store
        .list_invite_codes(params.cursor.as_deref(), limit)
        .await?;

    let cursor = if invite_codes.len() >= limit {
        invite_codes.last().map(|ic| ic.code.clone())
    } else {
        None
    };

    let codes: Vec<serde_json::Value> = invite_codes
        .into_iter()
        .map(|ic| {
            serde_json::json!({
                "code": ic.code,
                "availableUses": ic.available_uses,
                "disabled": ic.disabled,
                "forAccount": ic.for_account,
                "createdBy": ic.created_by,
                "createdAt": ic.created_at.to_rfc3339(),
                "uses": ic.uses.iter().map(|u| serde_json::json!({
                    "usedBy": u.used_by,
                    "usedAt": u.used_at.to_rfc3339(),
                })).collect::<Vec<_>>(),
            })
        })
        .collect();

    Ok(Json(serde_json::json!({
        "codes": codes,
        "cursor": cursor,
    })))
}

// ---------------------------------------------------------------------------
// 13. get_config
// ---------------------------------------------------------------------------

pub async fn get_config<A, R, B>(
    State(state): State<AppState<A, R, B>>,
    _admin: AdminAuth,
) -> Result<Json<serde_json::Value>, XrpcError>
where
    A: AccountStore,
    R: RepoStore,
    B: BlobStore,
{
    let config = &state.config;

    let mode = match config.mode {
        dallaspds_core::config::PdsMode::Single => "single",
        dallaspds_core::config::PdsMode::Multi => "multi",
    };

    Ok(Json(serde_json::json!({
        "hostname": config.hostname,
        "publicUrl": config.public_url,
        "mode": mode,
        "availableUserDomains": config.available_user_domains,
        "inviteRequired": config.invite_required,
        "appviewUrl": config.appview_url,
        "appviewDid": config.appview_did,
        "relayUrl": config.relay_url,
        "adminDids": config.admin_dids,
    })))
}
