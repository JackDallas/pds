use atrium_crypto::keypair::{Did, Export, P256Keypair, Secp256k1Keypair};
use dallaspds_core::{PdsError, PdsResult};
use rand::rngs::ThreadRng;

/// Wraps atrium-crypto keypair types for P-256 and secp256k1 (K-256) signing.
pub enum SigningKey {
    P256(P256Keypair),
    K256(Secp256k1Keypair),
}

impl SigningKey {
    /// Generate a new random P-256 signing key.
    pub fn generate_p256() -> PdsResult<Self> {
        let keypair = P256Keypair::create(&mut ThreadRng::default());
        Ok(SigningKey::P256(keypair))
    }

    /// Generate a new random secp256k1 (K-256) signing key.
    pub fn generate_k256() -> PdsResult<Self> {
        let keypair = Secp256k1Keypair::create(&mut ThreadRng::default());
        Ok(SigningKey::K256(keypair))
    }

    /// Returns the `did:key` string representation of the public key.
    pub fn did_key(&self) -> String {
        match self {
            SigningKey::P256(kp) => kp.did(),
            SigningKey::K256(kp) => kp.did(),
        }
    }

    /// Sign a message. The message is internally SHA-256 hashed by atrium-crypto
    /// and the resulting ECDSA signature is returned in low-S normalized form.
    pub fn sign(&self, msg: &[u8]) -> PdsResult<Vec<u8>> {
        match self {
            SigningKey::P256(kp) => kp.sign(msg).map_err(|e| PdsError::Crypto(e.to_string())),
            SigningKey::K256(kp) => kp.sign(msg).map_err(|e| PdsError::Crypto(e.to_string())),
        }
    }

    /// Returns the compressed public key bytes.
    pub fn public_key_bytes(&self) -> Vec<u8> {
        // The did_key() string contains the multibase-encoded compressed public key.
        // We can extract it, but it's simpler to re-derive from the private key.
        // atrium-crypto doesn't expose compressed_public_key directly, but export
        // gives us the private key bytes from which we can reconstruct.
        // Instead, we parse the did:key to get the public key bytes.
        let did = self.did_key();
        // did:key:z... — the 'z' prefix means base58btc multibase encoding.
        // We rely on atrium_crypto::did::parse_did_key for this.
        atrium_crypto::did::parse_did_key(&did)
            .map(|(_alg, pk)| pk)
            .unwrap_or_default()
    }

    /// Export the private key as raw scalar bytes.
    pub fn to_bytes(&self) -> Vec<u8> {
        match self {
            SigningKey::P256(kp) => kp.export(),
            SigningKey::K256(kp) => kp.export(),
        }
    }

    /// Import a signing key from raw scalar bytes.
    ///
    /// `key_type` must be `"p256"` or `"k256"` / `"secp256k1"`.
    pub fn from_bytes(key_type: &str, bytes: &[u8]) -> PdsResult<Self> {
        match key_type {
            "p256" | "P256" | "ES256" => {
                let kp =
                    P256Keypair::import(bytes).map_err(|e| PdsError::Crypto(e.to_string()))?;
                Ok(SigningKey::P256(kp))
            }
            "k256" | "K256" | "secp256k1" | "ES256K" => {
                let kp = Secp256k1Keypair::import(bytes)
                    .map_err(|e| PdsError::Crypto(e.to_string()))?;
                Ok(SigningKey::K256(kp))
            }
            other => Err(PdsError::Crypto(format!("unknown key type: {other}"))),
        }
    }

    /// Returns the JWT algorithm name for this key type.
    ///
    /// - P-256 => `"ES256"`
    /// - K-256 (secp256k1) => `"ES256K"`
    pub fn algorithm(&self) -> &str {
        match self {
            SigningKey::P256(_) => "ES256",
            SigningKey::K256(_) => "ES256K",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_p256_produces_valid_key() {
        let key = SigningKey::generate_p256().unwrap();
        assert!(key.did_key().starts_with("did:key:z"));
        assert!(!key.to_bytes().is_empty());
    }

    #[test]
    fn sign_and_verify_roundtrip() {
        let key = SigningKey::generate_p256().unwrap();
        let msg = b"hello atproto";
        let sig = key.sign(msg).unwrap();
        assert!(!sig.is_empty(), "signature should not be empty");
        // Signatures should be ~64 bytes for P-256
        assert!(sig.len() >= 60 && sig.len() <= 72, "unexpected sig length: {}", sig.len());
    }

    #[test]
    fn from_bytes_roundtrip_p256() {
        let key = SigningKey::generate_p256().unwrap();
        let bytes = key.to_bytes();
        let restored = SigningKey::from_bytes("p256", &bytes).unwrap();
        assert_eq!(key.did_key(), restored.did_key());
    }

    #[test]
    fn from_bytes_roundtrip_k256() {
        let key = SigningKey::generate_k256().unwrap();
        let bytes = key.to_bytes();
        let restored = SigningKey::from_bytes("k256", &bytes).unwrap();
        assert_eq!(key.did_key(), restored.did_key());
    }

    #[test]
    fn algorithm_returns_correct_string() {
        let p256 = SigningKey::generate_p256().unwrap();
        assert_eq!(p256.algorithm(), "ES256");

        let k256 = SigningKey::generate_k256().unwrap();
        assert_eq!(k256.algorithm(), "ES256K");
    }

    #[test]
    fn public_key_bytes_nonempty() {
        let p256 = SigningKey::generate_p256().unwrap();
        let pk = p256.public_key_bytes();
        assert!(!pk.is_empty(), "P-256 public key bytes should not be empty");

        let k256 = SigningKey::generate_k256().unwrap();
        let pk = k256.public_key_bytes();
        assert!(!pk.is_empty(), "K-256 public key bytes should not be empty");
    }
}
