use dallaspds_core::{PdsError, PdsResult};
use jsonwebtoken::{DecodingKey, EncodingKey, Header, Validation, decode, encode};
use serde::{Deserialize, Serialize};

/// Claims for an access token (short-lived).
#[derive(Debug, Serialize, Deserialize)]
pub struct AccessTokenClaims {
    pub sub: String,
    pub iat: i64,
    pub exp: i64,
}

/// Claims for a refresh token (long-lived).
#[derive(Debug, Serialize, Deserialize)]
pub struct RefreshTokenClaims {
    pub sub: String,
    pub jti: String,
    pub iat: i64,
    pub exp: i64,
}

/// Create an access token with a 2-hour expiry.
///
/// Uses HS256 symmetric signing with the provided secret.
pub fn create_access_token(did: &str, secret: &str) -> PdsResult<String> {
    let now = chrono::Utc::now().timestamp();
    let claims = AccessTokenClaims {
        sub: did.to_string(),
        iat: now,
        exp: now + 2 * 60 * 60, // 2 hours
    };
    let key = EncodingKey::from_secret(secret.as_bytes());
    encode(&Header::default(), &claims, &key).map_err(|e| PdsError::Auth(e.to_string()))
}

/// Create a refresh token with a 90-day expiry.
///
/// Uses HS256 symmetric signing with the provided secret.
pub fn create_refresh_token(did: &str, jti: &str, secret: &str) -> PdsResult<String> {
    let now = chrono::Utc::now().timestamp();
    let claims = RefreshTokenClaims {
        sub: did.to_string(),
        jti: jti.to_string(),
        iat: now,
        exp: now + 90 * 24 * 60 * 60, // 90 days
    };
    let key = EncodingKey::from_secret(secret.as_bytes());
    encode(&Header::default(), &claims, &key).map_err(|e| PdsError::Auth(e.to_string()))
}

/// Validate an access token and return its claims.
pub fn validate_access_token(token: &str, secret: &str) -> PdsResult<AccessTokenClaims> {
    let key = DecodingKey::from_secret(secret.as_bytes());
    let validation = Validation::default();
    let token_data = decode::<AccessTokenClaims>(token, &key, &validation)
        .map_err(|e| PdsError::Auth(e.to_string()))?;
    Ok(token_data.claims)
}

/// Validate a refresh token and return its claims.
pub fn validate_refresh_token(token: &str, secret: &str) -> PdsResult<RefreshTokenClaims> {
    let key = DecodingKey::from_secret(secret.as_bytes());
    let validation = Validation::default();
    let token_data = decode::<RefreshTokenClaims>(token, &key, &validation)
        .map_err(|e| PdsError::Auth(e.to_string()))?;
    Ok(token_data.claims)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: &str = "test-secret-key-for-jwt-tests";
    const OTHER_SECRET: &str = "different-secret-key-for-jwt";
    const DID: &str = "did:plc:testuser123";

    #[test]
    fn access_token_roundtrip() {
        let token = create_access_token(DID, SECRET).unwrap();
        let claims = validate_access_token(&token, SECRET).unwrap();
        assert_eq!(claims.sub, DID);
    }

    #[test]
    fn access_token_wrong_secret_fails() {
        let token = create_access_token(DID, SECRET).unwrap();
        let result = validate_access_token(&token, OTHER_SECRET);
        assert!(result.is_err());
    }

    #[test]
    fn refresh_token_roundtrip() {
        let token = create_refresh_token(DID, "jti-123", SECRET).unwrap();
        let claims = validate_refresh_token(&token, SECRET).unwrap();
        assert_eq!(claims.sub, DID);
        assert_eq!(claims.jti, "jti-123");
    }

    #[test]
    fn refresh_token_wrong_secret_fails() {
        let token = create_refresh_token(DID, "jti-123", SECRET).unwrap();
        let result = validate_refresh_token(&token, OTHER_SECRET);
        assert!(result.is_err());
    }

    #[test]
    fn access_token_has_2hr_expiry() {
        let token = create_access_token(DID, SECRET).unwrap();
        let claims = validate_access_token(&token, SECRET).unwrap();
        let duration = claims.exp - claims.iat;
        assert_eq!(duration, 2 * 60 * 60, "access token should expire in 2 hours");
    }

    #[test]
    fn refresh_token_has_90day_expiry() {
        let token = create_refresh_token(DID, "jti-456", SECRET).unwrap();
        let claims = validate_refresh_token(&token, SECRET).unwrap();
        let duration = claims.exp - claims.iat;
        assert_eq!(duration, 90 * 24 * 60 * 60, "refresh token should expire in 90 days");
    }

    #[test]
    fn expired_token_validation_fails() {
        // Manually construct a token with exp in the past
        let now = chrono::Utc::now().timestamp();
        let claims = AccessTokenClaims {
            sub: DID.to_string(),
            iat: now - 7200,
            exp: now - 3600, // expired 1 hour ago
        };
        let key = EncodingKey::from_secret(SECRET.as_bytes());
        let token = encode(&Header::default(), &claims, &key).unwrap();

        let result = validate_access_token(&token, SECRET);
        assert!(result.is_err(), "expired token should fail validation");
    }
}
