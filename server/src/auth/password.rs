#![allow(dead_code)]

use argon2::{
    password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use rand_core::OsRng;

pub const MIN_PASSWORD_LEN: usize = 8;

pub type PasswordResult<T> = Result<T, PasswordError>;

#[derive(Debug, thiserror::Error)]
pub enum PasswordError {
    #[error("password must be at least {MIN_PASSWORD_LEN} characters")]
    TooShort,
    #[error("password hashing failed")]
    Hash,
    #[error("password hash is invalid")]
    InvalidHash,
}

pub fn validate_password(password: &str) -> PasswordResult<()> {
    if password.chars().count() < MIN_PASSWORD_LEN {
        return Err(PasswordError::TooShort);
    }

    Ok(())
}

pub fn hash_password(password: &str) -> PasswordResult<String> {
    validate_password(password)?;

    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|_| PasswordError::Hash)
}

pub fn verify_password(password: &str, password_hash: &str) -> PasswordResult<bool> {
    let parsed_hash = PasswordHash::new(password_hash).map_err(|_| PasswordError::InvalidHash)?;

    Ok(Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_short_passwords() {
        assert!(matches!(
            validate_password("short"),
            Err(PasswordError::TooShort)
        ));
    }

    #[test]
    fn hashes_and_verifies_passwords() {
        let hash = hash_password("correct horse battery staple").unwrap();

        assert_ne!(hash, "correct horse battery staple");
        assert!(verify_password("correct horse battery staple", &hash).unwrap());
        assert!(!verify_password("wrong password", &hash).unwrap());
    }
}
