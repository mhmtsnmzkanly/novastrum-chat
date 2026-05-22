#![allow(dead_code)]

use rand_core::{OsRng, RngCore};
use sha2::{Digest, Sha256};

const SESSION_TOKEN_BYTES: usize = 32;

pub type SessionTokenResult<T> = Result<T, SessionTokenError>;

#[derive(Debug, thiserror::Error)]
pub enum SessionTokenError {
    #[error("session token cannot be empty")]
    Empty,
}

pub fn generate_session_token() -> SessionTokenResult<String> {
    let mut bytes = [0_u8; SESSION_TOKEN_BYTES];
    OsRng.fill_bytes(&mut bytes);

    Ok(hex::encode(bytes))
}

pub fn hash_session_token(token: &str) -> SessionTokenResult<String> {
    if token.is_empty() {
        return Err(SessionTokenError::Empty);
    }

    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    Ok(hex::encode(hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_hex_session_token() {
        let token = generate_session_token().unwrap();

        assert_eq!(token.len(), SESSION_TOKEN_BYTES * 2);
        assert!(token.chars().all(|character| character.is_ascii_hexdigit()));
    }

    #[test]
    fn hashes_session_token_without_returning_raw_token() {
        let token = generate_session_token().unwrap();
        let token_hash = hash_session_token(&token).unwrap();

        assert_eq!(token_hash.len(), 64);
        assert_ne!(token_hash, token);
        assert_eq!(token_hash, hash_session_token(&token).unwrap());
    }
}
