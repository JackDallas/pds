use crate::signing::SigningKey;
use dallaspds_core::{PdsError, PdsResult};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

/// Create a did:plc genesis operation.
///
/// Returns `(did, signed_genesis_op)` where:
/// - `did` is the `did:plc:<24-char-base32>` identifier
/// - `signed_genesis_op` is the JSON object with `"sig"` field included
///
/// # Process
///
/// 1. Build an unsigned genesis operation JSON
/// 2. Serialize the unsigned op to DAG-CBOR
/// 3. Sign the DAG-CBOR bytes with the signing key (atrium-crypto handles SHA-256 internally)
/// 4. Base64url-encode the signature (no padding)
/// 5. Add `"sig"` field to the operation
/// 6. Compute the DID: `did:plc:` + first 24 chars of base32-lower(sha256(dag-cbor(signed_op)))
pub fn create_did_plc_operation(
    signing_key: &SigningKey,
    rotation_keys: Vec<String>,
    handle: &str,
    pds_endpoint: &str,
) -> PdsResult<(String, serde_json::Value)> {
    // Step 1: Build unsigned genesis operation
    let unsigned_op = json!({
        "type": "plc_operation",
        "rotationKeys": rotation_keys,
        "verificationMethods": {
            "atproto": signing_key.did_key()
        },
        "alsoKnownAs": [format!("at://{handle}")],
        "services": {
            "atproto_pds": {
                "type": "AtprotoPersonalDataServer",
                "endpoint": pds_endpoint
            }
        },
        "prev": null
    });

    // Step 2: Serialize unsigned op to DAG-CBOR
    let unsigned_cbor = dag_cbor_encode(&unsigned_op)?;

    // Step 3: Sign the DAG-CBOR bytes
    // atrium-crypto's sign() internally hashes with SHA-256 then signs
    let signature = signing_key.sign(&unsigned_cbor)?;

    // Step 4: Base64url-encode the signature (no padding)
    let sig_b64 = base64url_encode(&signature);

    // Step 5: Build signed operation (add "sig" field)
    let mut signed_op = match unsigned_op {
        Value::Object(map) => map,
        _ => unreachable!(),
    };
    signed_op.insert("sig".to_string(), Value::String(sig_b64));
    let signed_op_value = Value::Object(signed_op.clone());

    // Step 6: Compute the DID
    // Hash the DAG-CBOR of the *signed* operation
    let signed_cbor = dag_cbor_encode(&signed_op_value)?;
    let hash = Sha256::digest(&signed_cbor);
    let hash_b32 = base32::encode(base32::Alphabet::Rfc4648Lower { padding: false }, &hash);
    let did = format!("did:plc:{}", &hash_b32[..24]);

    Ok((did, signed_op_value))
}

/// Encode a serde_json::Value to DAG-CBOR bytes.
///
/// DAG-CBOR requires deterministic key ordering (sorted) and specific CBOR
/// encoding rules. We convert JSON to ipld_core::ipld::Ipld first, then
/// serialize with serde_ipld_dagcbor.
fn dag_cbor_encode(value: &serde_json::Value) -> PdsResult<Vec<u8>> {
    let ipld = json_to_ipld(value);
    serde_ipld_dagcbor::to_vec(&ipld)
        .map_err(|e| PdsError::Crypto(format!("DAG-CBOR encoding failed: {e}")))
}

/// Convert a serde_json::Value to an ipld_core::ipld::Ipld value.
///
/// DAG-CBOR requires maps to have sorted keys. ipld_core::ipld::Ipld uses
/// BTreeMap which provides sorted ordering automatically.
fn json_to_ipld(value: &serde_json::Value) -> ipld_core::ipld::Ipld {
    match value {
        Value::Null => ipld_core::ipld::Ipld::Null,
        Value::Bool(b) => ipld_core::ipld::Ipld::Bool(*b),
        Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                ipld_core::ipld::Ipld::Integer(i as i128)
            } else if let Some(f) = n.as_f64() {
                ipld_core::ipld::Ipld::Float(f)
            } else {
                ipld_core::ipld::Ipld::Null
            }
        }
        Value::String(s) => ipld_core::ipld::Ipld::String(s.clone()),
        Value::Array(arr) => {
            ipld_core::ipld::Ipld::List(arr.iter().map(json_to_ipld).collect())
        }
        Value::Object(map) => {
            let btree: std::collections::BTreeMap<String, ipld_core::ipld::Ipld> =
                map.iter().map(|(k, v)| (k.clone(), json_to_ipld(v))).collect();
            ipld_core::ipld::Ipld::Map(btree)
        }
    }
}

/// Base64url encoding without padding (RFC 4648 section 5).
fn base64url_encode(data: &[u8]) -> String {
    use base64url_alphabet::encode;
    encode(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::signing::SigningKey;

    #[test]
    fn create_did_plc_produces_valid_did() {
        let key = SigningKey::generate_p256().unwrap();
        let rotation_keys = vec![key.did_key()];
        let (did, _op) = create_did_plc_operation(&key, rotation_keys, "alice.test", "https://pds.test").unwrap();
        assert!(did.starts_with("did:plc:"), "DID should start with did:plc:, got: {did}");
        // did:plc: prefix (8 chars) + 24-char hash
        assert_eq!(did.len(), 32, "did:plc should be 32 chars total, got: {}", did.len());
    }

    #[test]
    fn signed_op_has_sig_field() {
        let key = SigningKey::generate_p256().unwrap();
        let rotation_keys = vec![key.did_key()];
        let (_did, op) = create_did_plc_operation(&key, rotation_keys, "alice.test", "https://pds.test").unwrap();
        assert!(op.get("sig").is_some(), "signed op must have a 'sig' field");
        assert!(op["sig"].as_str().unwrap().len() > 10, "sig should be non-trivial");
    }

    #[test]
    fn op_has_required_fields() {
        let key = SigningKey::generate_p256().unwrap();
        let rotation_keys = vec![key.did_key()];
        let (_did, op) = create_did_plc_operation(&key, rotation_keys, "alice.test", "https://pds.test").unwrap();

        assert_eq!(op["type"], "plc_operation");
        assert!(op["rotationKeys"].is_array());
        assert!(op["verificationMethods"].is_object());
        assert!(op["alsoKnownAs"].is_array());
        assert!(op["services"].is_object());
        assert!(op["prev"].is_null());
    }

    #[test]
    fn deterministic_dag_cbor_encoding() {
        // DAG-CBOR must produce the same output for the same input
        let value = serde_json::json!({"b": 2, "a": 1});
        let enc1 = dag_cbor_encode(&value).unwrap();
        let enc2 = dag_cbor_encode(&value).unwrap();
        assert_eq!(enc1, enc2, "DAG-CBOR encoding should be deterministic");

        // Keys should be sorted: "a" before "b"
        let value_reordered = serde_json::json!({"a": 1, "b": 2});
        let enc3 = dag_cbor_encode(&value_reordered).unwrap();
        assert_eq!(enc1, enc3, "key order in JSON should not affect DAG-CBOR output");
    }
}

/// Minimal base64url encoder (no padding) to avoid an extra dependency.
mod base64url_alphabet {
    const TABLE: &[u8; 64] =
        b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";

    pub fn encode(data: &[u8]) -> String {
        let mut result = String::with_capacity((data.len() + 2) / 3 * 4);
        let chunks = data.chunks(3);
        for chunk in chunks {
            let b0 = chunk[0] as u32;
            let b1 = if chunk.len() > 1 { chunk[1] as u32 } else { 0 };
            let b2 = if chunk.len() > 2 { chunk[2] as u32 } else { 0 };
            let triple = (b0 << 16) | (b1 << 8) | b2;

            result.push(TABLE[((triple >> 18) & 0x3F) as usize] as char);
            result.push(TABLE[((triple >> 12) & 0x3F) as usize] as char);
            if chunk.len() > 1 {
                result.push(TABLE[((triple >> 6) & 0x3F) as usize] as char);
            }
            if chunk.len() > 2 {
                result.push(TABLE[(triple & 0x3F) as usize] as char);
            }
        }
        result
    }
}
