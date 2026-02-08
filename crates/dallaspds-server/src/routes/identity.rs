use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::auth::AuthenticatedUser;
use crate::error::XrpcError;
use crate::state::AppState;
use dallaspds_core::traits::*;
use dallaspds_core::PdsError;

// ---------------------------------------------------------------------------
// 1. resolveHandle
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct ResolveHandleQuery {
    pub handle: String,
}

pub async fn resolve_handle<A, R, B>(
    State(state): State<AppState<A, R, B>>,
    Query(params): Query<ResolveHandleQuery>,
) -> Result<Json<Value>, XrpcError>
where
    A: AccountStore,
    R: RepoStore,
    B: BlobStore,
{
    // Look up the handle in our account store first (for locally-hosted handles).
    let account = state
        .account_store
        .get_account_by_handle(&params.handle)
        .await?;

    if let Some(acct) = account {
        return Ok(Json(json!({ "did": acct.did })));
    }

    // Fallback to external resolution (DNS TXT / HTTPS).
    match dallaspds_identity::resolve_handle(&params.handle).await {
        Ok(Some(did)) => Ok(Json(json!({ "did": did }))),
        _ => Err(XrpcError::new(
            StatusCode::NOT_FOUND,
            "HandleNotFound",
            format!("handle not found: {}", params.handle),
        )),
    }
}

// ---------------------------------------------------------------------------
// 2. updateHandle
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct UpdateHandleRequest {
    pub handle: String,
}

pub async fn update_handle<A, R, B>(
    State(state): State<AppState<A, R, B>>,
    user: AuthenticatedUser,
    Json(body): Json<UpdateHandleRequest>,
) -> Result<StatusCode, XrpcError>
where
    A: AccountStore,
    R: RepoStore,
    B: BlobStore,
{
    // Validate handle format — must end with one of the available user domains,
    // or we'd need to verify DNS/HTTPS for external handles.
    let handle_valid = state
        .config
        .available_user_domains
        .iter()
        .any(|domain| body.handle.ends_with(domain));

    if !handle_valid {
        return Err(XrpcError::new(
            StatusCode::BAD_REQUEST,
            "InvalidHandle",
            format!(
                "Handle must end with one of: {}",
                state.config.available_user_domains.join(", ")
            ),
        ));
    }

    // Check handle isn't already taken by someone else.
    if let Some(existing) = state.account_store.get_account_by_handle(&body.handle).await? {
        if existing.did != user.did {
            return Err(PdsError::HandleAlreadyTaken.into());
        }
    }

    // Update the handle.
    state
        .account_store
        .update_handle(&user.did, &body.handle)
        .await?;

    // If we have a sequencer, emit an identity event.
    if let Some(ref sequencer) = state.sequencer {
        use crate::firehose::events::{FirehoseEvent, IdentityEvent};
        let seq = sequencer.next_seq();
        let event = FirehoseEvent::Identity(IdentityEvent {
            seq,
            did: user.did.clone(),
            time: chrono::Utc::now().to_rfc3339(),
            handle: Some(body.handle.clone()),
        });
        crate::firehose::emit::emit_and_persist(&state, event).await;
    }

    Ok(StatusCode::OK)
}
