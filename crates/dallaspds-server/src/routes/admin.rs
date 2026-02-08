use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;

use crate::auth::AuthenticatedUser;
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
