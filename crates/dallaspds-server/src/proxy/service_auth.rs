use dallaspds_core::{PdsError, PdsResult};
use dallaspds_crypto::SigningKey;
use serde::{Deserialize, Serialize};

/// Claims for a service auth JWT used when proxying requests to the AppView.
///
/// Service auth JWTs are signed with the user's repo signing key (ES256 or ES256K)
/// and include the `lxm` claim to restrict which Lexicon method the token is valid for.
#[derive(Debug, Serialize, Deserialize)]
pub struct ServiceAuthClaims {
    /// Issuer: the DID of the user.
    pub iss: String,
    /// Audience: the DID of the service being called (e.g., the AppView DID).
    pub aud: String,
    /// Lexicon method: the XRPC method this token authorizes (e.g., "app.bsky.feed.getTimeline").
    pub lxm: String,
    /// Issued at timestamp.
    pub iat: i64,
    /// Expiration timestamp.
    pub exp: i64,
}

/// Create a service auth JWT signed with the user's repo signing key.
///
/// The JWT is short-lived (60 seconds) and scoped to a specific lexicon method.
pub fn create_service_auth_token(
    signing_key: &SigningKey,
    user_did: &str,
    audience_did: &str,
    lexicon_method: &str,
) -> PdsResult<String> {
    let now = chrono::Utc::now().timestamp();
    let claims = ServiceAuthClaims {
        iss: user_did.to_string(),
        aud: audience_did.to_string(),
        lxm: lexicon_method.to_string(),
        iat: now,
        exp: now + 60, // 60 seconds
    };

    // Encode header + claims as JSON, then sign with the repo key.
    // Service auth uses the repo's ECDSA key, not HS256.
    let header_json = serde_json::json!({
        "typ": "JWT",
        "alg": signing_key.algorithm(),
    });
    let header_b64 = base64url_encode(&serde_json::to_vec(&header_json)
        .map_err(|e| PdsError::Crypto(format!("JSON encode error: {e}")))?);
    let claims_b64 = base64url_encode(&serde_json::to_vec(&claims)
        .map_err(|e| PdsError::Crypto(format!("JSON encode error: {e}")))?);

    let signing_input = format!("{header_b64}.{claims_b64}");
    let signature = signing_key.sign(signing_input.as_bytes())?;
    let sig_b64 = base64url_encode(&signature);

    Ok(format!("{signing_input}.{sig_b64}"))
}

/// Base64url encode without padding (JWT standard).
fn base64url_encode(data: &[u8]) -> String {
    use base64url_no_pad::encode;
    encode(data)
}

/// Minimal base64url (no pad) encoder.
mod base64url_no_pad {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

    pub fn encode(data: &[u8]) -> String {
        let mut result = String::with_capacity((data.len() + 2) / 3 * 4);
        for chunk in data.chunks(3) {
            let b0 = chunk[0] as u32;
            let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
            let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
            let n = (b0 << 16) | (b1 << 8) | b2;

            result.push(ALPHABET[((n >> 18) & 0x3F) as usize] as char);
            result.push(ALPHABET[((n >> 12) & 0x3F) as usize] as char);
            if chunk.len() > 1 {
                result.push(ALPHABET[((n >> 6) & 0x3F) as usize] as char);
            }
            if chunk.len() > 2 {
                result.push(ALPHABET[(n & 0x3F) as usize] as char);
            }
        }
        result
    }
}
