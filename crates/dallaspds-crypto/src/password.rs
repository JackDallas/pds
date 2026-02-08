use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::SaltString;
use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier};
use dallaspds_core::{PdsError, PdsResult};

/// Hash a password using Argon2id with a random salt.
pub fn hash_password(password: &str) -> PdsResult<String> {
    let salt = SaltString::generate(&mut OsRng);
    let argon2 = Argon2::default(); // Argon2id by default
    let hash = argon2
        .hash_password(password.as_bytes(), &salt)
        .map_err(|e| PdsError::Crypto(format!("password hashing failed: {e}")))?;
    Ok(hash.to_string())
}

/// Verify a password against an Argon2id hash string.
///
/// Returns `Ok(true)` if the password matches, `Ok(false)` otherwise.
pub fn verify_password(password: &str, hash: &str) -> PdsResult<bool> {
    let parsed_hash = PasswordHash::new(hash)
        .map_err(|e| PdsError::Crypto(format!("invalid password hash: {e}")))?;
    let argon2 = Argon2::default();
    match argon2.verify_password(password.as_bytes(), &parsed_hash) {
        Ok(()) => Ok(true),
        Err(argon2::password_hash::Error::Password) => Ok(false),
        Err(e) => Err(PdsError::Crypto(format!("password verification failed: {e}"))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_verify_correct_password() {
        let hash = hash_password("correct-horse").unwrap();
        assert!(verify_password("correct-horse", &hash).unwrap());
    }

    #[test]
    fn hash_verify_wrong_password() {
        let hash = hash_password("correct-horse").unwrap();
        assert!(!verify_password("wrong-horse", &hash).unwrap());
    }

    #[test]
    fn hash_produces_argon2_format() {
        let hash = hash_password("test").unwrap();
        assert!(hash.starts_with("$argon2"), "hash should start with $argon2, got: {hash}");
    }

    #[test]
    fn different_hashes_for_same_password() {
        let hash1 = hash_password("same-password").unwrap();
        let hash2 = hash_password("same-password").unwrap();
        assert_ne!(hash1, hash2, "different salts should produce different hashes");
    }
}
