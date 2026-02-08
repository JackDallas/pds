use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::StatusCode;
use axum::Extension;

use crate::error::XrpcError;

/// A newtype wrapper around the JWT access secret, added as an Axum Extension.
#[derive(Clone)]
pub struct JwtSecret(pub String);

/// A newtype wrapper around the JWT refresh secret, added as an Axum Extension.
#[derive(Clone)]
pub struct JwtRefreshSecret(pub String);

/// Represents an authenticated user extracted from a valid JWT bearer token.
#[derive(Debug, Clone)]
pub struct AuthenticatedUser {
    pub did: String,
}

/// An optional authentication extractor. Returns `None` when no Authorization
/// header is present, rather than returning an error.
#[derive(Debug, Clone)]
pub struct OptionalAuth(pub Option<AuthenticatedUser>);

impl<S> FromRequestParts<S> for AuthenticatedUser
where
    S: Send + Sync,
{
    type Rejection = XrpcError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Extension(jwt_secret) = Extension::<JwtSecret>::from_request_parts(parts, state)
            .await
            .map_err(|_| {
                XrpcError::new(
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "InternalError",
                    "JWT secret not configured",
                )
            })?;

        let auth_header = parts
            .headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| {
                XrpcError::new(
                    StatusCode::UNAUTHORIZED,
                    "AuthenticationRequired",
                    "Missing authorization header",
                )
            })?;

        let token = auth_header.strip_prefix("Bearer ").ok_or_else(|| {
            XrpcError::new(
                StatusCode::UNAUTHORIZED,
                "AuthenticationRequired",
                "Invalid authorization format",
            )
        })?;

        let claims = dallaspds_crypto::jwt::validate_access_token(token, &jwt_secret.0)
            .map_err(|e| {
                let err_msg = e.to_string();
                if err_msg.contains("ExpiredSignature") {
                    XrpcError::new(
                        StatusCode::UNAUTHORIZED,
                        "ExpiredToken",
                        "Token has expired",
                    )
                } else {
                    XrpcError::new(StatusCode::UNAUTHORIZED, "InvalidToken", "Invalid token")
                }
            })?;

        Ok(AuthenticatedUser { did: claims.sub })
    }
}

impl<S> FromRequestParts<S> for OptionalAuth
where
    S: Send + Sync,
{
    type Rejection = XrpcError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        // If no authorization header is present, return None (no auth).
        let has_auth = parts
            .headers
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .is_some();

        if !has_auth {
            return Ok(OptionalAuth(None));
        }

        // Header is present, so attempt full authentication.
        match AuthenticatedUser::from_request_parts(parts, state).await {
            Ok(user) => Ok(OptionalAuth(Some(user))),
            Err(e) => Err(e),
        }
    }
}
