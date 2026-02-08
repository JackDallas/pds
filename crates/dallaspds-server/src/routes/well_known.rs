use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::IntoResponse;

use crate::error::XrpcError;
use crate::state::AppState;
use dallaspds_core::config::PdsMode;
use dallaspds_core::traits::*;

/// GET /.well-known/atproto-did
///
/// Returns the DID of the account as plain text.
///
/// - In single-user mode: returns the DID of the sole account.
/// - In multi-user mode: resolves from Host header to find the account.
pub async fn atproto_did<A, R, B>(
    State(state): State<AppState<A, R, B>>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, XrpcError>
where
    A: AccountStore,
    R: RepoStore,
    B: BlobStore,
{
    match state.config.mode {
        PdsMode::Multi => {
            // Extract handle from Host header.
            let host = headers
                .get("host")
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");

            // Strip port if present.
            let hostname = host.split(':').next().unwrap_or(host);

            let account = state
                .account_store
                .get_account_by_handle(hostname)
                .await?
                .ok_or_else(|| {
                    XrpcError::new(
                        StatusCode::NOT_FOUND,
                        "AccountNotFound",
                        format!("No account found for host: {}", hostname),
                    )
                })?;

            Ok((
                StatusCode::OK,
                [("content-type", "text/plain")],
                account.did,
            ))
        }
        PdsMode::Single => {
            // In single-user mode, return the sole account's DID.
            let accounts = state.account_store.list_accounts(None, 1).await?;
            let account = accounts.into_iter().next().ok_or_else(|| {
                XrpcError::new(
                    StatusCode::NOT_FOUND,
                    "AccountNotFound",
                    "No account found on this server",
                )
            })?;

            Ok((
                StatusCode::OK,
                [("content-type", "text/plain")],
                account.did,
            ))
        }
    }
}
