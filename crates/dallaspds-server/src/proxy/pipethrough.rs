
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::{StatusCode, header};
use axum::response::Response;

use crate::auth::AuthenticatedUser;
use crate::error::XrpcError;
use crate::state::AppState;
use dallaspds_core::traits::*;

use super::service_auth::create_service_auth_token;

/// Proxy an XRPC request through to a configured AppView service.
///
/// This handler is used as a fallback for any XRPC method that the PDS doesn't
/// implement locally. It attaches a service auth JWT and forwards the request.
pub async fn pipethrough<A, R, B>(
    State(state): State<AppState<A, R, B>>,
    user: Option<AuthenticatedUser>,
    request: Request,
) -> Result<Response, XrpcError>
where
    A: AccountStore,
    R: RepoStore,
    B: BlobStore,
{
    let appview_url = match &state.config.appview_url {
        Some(url) => url.clone(),
        None => {
            return Err(XrpcError::new(
                StatusCode::NOT_IMPLEMENTED,
                "MethodNotImplemented",
                "No AppView configured for proxying",
            ));
        }
    };

    let appview_did = state.config.appview_did.as_deref().unwrap_or("");

    // Extract the XRPC method from the path.
    let path = request.uri().path().to_string();
    let method_name = path
        .strip_prefix("/xrpc/")
        .unwrap_or(&path);

    // Build upstream URL preserving query string.
    let query = request.uri().query().map(|q| format!("?{q}")).unwrap_or_default();
    let upstream_url = format!(
        "{}/xrpc/{}{}",
        appview_url.trim_end_matches('/'),
        method_name,
        query,
    );

    let http_method = request.method().clone();
    let client = reqwest::Client::new();
    let mut builder = client.request(http_method.clone(), &upstream_url);

    // Copy relevant headers.
    for (name, value) in request.headers() {
        if name == header::HOST || name == header::AUTHORIZATION {
            continue;
        }
        if let Ok(v) = value.to_str() {
            builder = builder.header(name.as_str(), v);
        }
    }

    // Add service auth if we have an authenticated user.
    if let Some(ref user) = user {
        let account = state
            .account_store
            .get_account_by_did(&user.did)
            .await
            .map_err(|e| XrpcError::new(StatusCode::INTERNAL_SERVER_ERROR, "InternalServerError", e.to_string()))?;

        if let Some(account) = account {
            let signing_key = dallaspds_crypto::SigningKey::from_bytes("p256", &account.signing_key)
                .map_err(|e| XrpcError::new(StatusCode::INTERNAL_SERVER_ERROR, "InternalServerError", e.to_string()))?;

            match create_service_auth_token(&signing_key, &user.did, appview_did, method_name) {
                Ok(token) => {
                    builder = builder.header("authorization", format!("Bearer {token}"));
                }
                Err(e) => {
                    tracing::warn!("Failed to create service auth token: {e}");
                }
            }
        }
    }

    // Forward request body for POST/PUT methods.
    let body_bytes = axum::body::to_bytes(request.into_body(), 10 * 1024 * 1024)
        .await
        .map_err(|e| XrpcError::new(StatusCode::BAD_REQUEST, "InvalidRequest", e.to_string()))?;
    if !body_bytes.is_empty() {
        builder = builder.body(body_bytes.to_vec());
    }

    // Send upstream request.
    let upstream_resp = builder
        .send()
        .await
        .map_err(|e| XrpcError::new(StatusCode::BAD_GATEWAY, "UpstreamFailure", e.to_string()))?;

    // Convert upstream response back to axum response.
    let status = StatusCode::from_u16(upstream_resp.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let mut response_builder = Response::builder().status(status);

    for (name, value) in upstream_resp.headers() {
        if name == header::TRANSFER_ENCODING {
            continue;
        }
        response_builder = response_builder.header(name, value);
    }

    let resp_body = upstream_resp
        .bytes()
        .await
        .map_err(|e| XrpcError::new(StatusCode::BAD_GATEWAY, "UpstreamFailure", e.to_string()))?;

    response_builder
        .body(Body::from(resp_body))
        .map_err(|e| XrpcError::new(StatusCode::INTERNAL_SERVER_ERROR, "InternalServerError", e.to_string()))
}

/// Fallback handler for the router. Extracts optional auth from the request
/// headers and delegates to `pipethrough`.
pub async fn pipethrough_fallback<A, R, B>(
    State(state): State<AppState<A, R, B>>,
    request: Request,
) -> Result<Response, XrpcError>
where
    A: AccountStore,
    R: RepoStore,
    B: BlobStore,
{
    // Try to extract auth from the request headers.
    let user = extract_optional_auth(&state, &request);
    pipethrough(State(state), user, request).await
}

/// Try to extract an authenticated user from the request's Authorization header.
/// Returns `None` if no header is present or if the token is invalid.
fn extract_optional_auth<A, R, B>(
    state: &AppState<A, R, B>,
    request: &Request,
) -> Option<AuthenticatedUser>
where
    A: AccountStore,
    R: RepoStore,
    B: BlobStore,
{
    let auth_header = request
        .headers()
        .get("authorization")
        .and_then(|v| v.to_str().ok())?;
    let token = auth_header.strip_prefix("Bearer ")?;
    let claims = dallaspds_crypto::jwt::validate_access_token(token, &state.config.jwt.access_secret).ok()?;
    Some(AuthenticatedUser { did: claims.sub })
}
