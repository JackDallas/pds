use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde_json::{json, Value};

use crate::error::XrpcError;
use crate::state::AppState;
use dallaspds_core::traits::*;

// ---------------------------------------------------------------------------
// OAuth Authorization Server Metadata (RFC 8414)
// ---------------------------------------------------------------------------

/// Returns the OAuth Authorization Server metadata document.
///
/// This is served at `/.well-known/oauth-authorization-server`.
pub async fn authorization_server_metadata<A, R, B>(
    State(state): State<AppState<A, R, B>>,
) -> Result<Json<Value>, XrpcError>
where
    A: AccountStore,
    R: RepoStore,
    B: BlobStore,
{
    let issuer = &state.config.public_url;

    Ok(Json(json!({
        "issuer": issuer,
        "authorization_endpoint": format!("{issuer}/oauth/authorize"),
        "token_endpoint": format!("{issuer}/oauth/token"),
        "pushed_authorization_request_endpoint": format!("{issuer}/oauth/par"),
        "revocation_endpoint": format!("{issuer}/oauth/revoke"),
        "introspection_endpoint": format!("{issuer}/oauth/introspect"),
        "jwks_uri": format!("{issuer}/oauth/jwks"),
        "scopes_supported": ["atproto", "transition:generic", "transition:chat.bsky"],
        "response_types_supported": ["code"],
        "response_modes_supported": ["query"],
        "grant_types_supported": ["authorization_code", "refresh_token"],
        "subject_types_supported": ["public"],
        "token_endpoint_auth_methods_supported": ["none", "private_key_jwt"],
        "token_endpoint_auth_signing_alg_values_supported": ["ES256", "ES256K"],
        "dpop_signing_alg_values_supported": ["ES256", "ES256K"],
        "code_challenge_methods_supported": ["S256"],
        "require_pushed_authorization_requests": true,
        "require_request_uri_registration": true,
        "client_id_metadata_document_supported": true,
    })))
}

// ---------------------------------------------------------------------------
// OAuth Protected Resource Metadata (RFC 9728)
// ---------------------------------------------------------------------------

/// Returns the OAuth Protected Resource metadata document.
///
/// This is served at `/.well-known/oauth-protected-resource`.
pub async fn protected_resource_metadata<A, R, B>(
    State(state): State<AppState<A, R, B>>,
) -> Result<Json<Value>, XrpcError>
where
    A: AccountStore,
    R: RepoStore,
    B: BlobStore,
{
    let resource = &state.config.public_url;

    Ok(Json(json!({
        "resource": resource,
        "authorization_servers": [resource],
        "scopes_supported": ["atproto", "transition:generic", "transition:chat.bsky"],
        "bearer_methods_supported": ["header"],
        "resource_documentation": "https://atproto.com",
    })))
}

// ---------------------------------------------------------------------------
// Placeholder OAuth endpoints
//
// Full OAuth DPoP + PAR implementation is complex. These stubs return
// well-formed error responses so clients know the endpoints exist.
// ---------------------------------------------------------------------------

pub async fn oauth_par<A, R, B>(
    State(_state): State<AppState<A, R, B>>,
) -> Result<Json<Value>, XrpcError>
where
    A: AccountStore,
    R: RepoStore,
    B: BlobStore,
{
    Err(XrpcError::new(
        StatusCode::NOT_IMPLEMENTED,
        "NotImplemented",
        "OAuth PAR endpoint not yet implemented",
    ))
}

pub async fn oauth_authorize<A, R, B>(
    State(_state): State<AppState<A, R, B>>,
) -> Result<Json<Value>, XrpcError>
where
    A: AccountStore,
    R: RepoStore,
    B: BlobStore,
{
    Err(XrpcError::new(
        StatusCode::NOT_IMPLEMENTED,
        "NotImplemented",
        "OAuth authorization endpoint not yet implemented",
    ))
}

pub async fn oauth_token<A, R, B>(
    State(_state): State<AppState<A, R, B>>,
) -> Result<Json<Value>, XrpcError>
where
    A: AccountStore,
    R: RepoStore,
    B: BlobStore,
{
    Err(XrpcError::new(
        StatusCode::NOT_IMPLEMENTED,
        "NotImplemented",
        "OAuth token endpoint not yet implemented",
    ))
}

pub async fn oauth_revoke<A, R, B>(
    State(_state): State<AppState<A, R, B>>,
) -> Result<Json<Value>, XrpcError>
where
    A: AccountStore,
    R: RepoStore,
    B: BlobStore,
{
    Err(XrpcError::new(
        StatusCode::NOT_IMPLEMENTED,
        "NotImplemented",
        "OAuth revoke endpoint not yet implemented",
    ))
}

pub async fn oauth_jwks<A, R, B>(
    State(_state): State<AppState<A, R, B>>,
) -> Result<Json<Value>, XrpcError>
where
    A: AccountStore,
    R: RepoStore,
    B: BlobStore,
{
    // Return an empty JWK set for now.
    Ok(Json(json!({ "keys": [] })))
}
