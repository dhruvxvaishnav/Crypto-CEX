use argon2::password_hash::rand_core::OsRng;
use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::{Algorithm, Argon2, Params, Version};
use thiserror::Error;

const ARGON2_MEMORY_KIB: u32 = 64 * 1024;
const ARGON2_ITERATIONS: u32 = 3;
const ARGON2_PARALLELISM: u32 = 4;

/// Password hashing error.
#[derive(Debug, Error)]
pub enum PasswordError {
    /// Argon2 params could not be constructed.
    #[error("invalid argon2 params")]
    Params,
    /// Hashing failed.
    #[error("password hash failed")]
    Hash,
    /// Stored password hash is malformed.
    #[error("password hash parse failed")]
    Parse,
}

/// Hashes a password with argon2id per PRD FR-AUTH-01.
///
/// # Errors
///
/// Returns [`PasswordError`] if params or hashing fail.
pub fn hash_password(password: &str) -> Result<String, PasswordError> {
    let salt = SaltString::generate(&mut OsRng);
    argon2()?
        .hash_password(password.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|_| PasswordError::Hash)
}

/// Verifies a password against a stored argon2 hash.
///
/// # Errors
///
/// Returns [`PasswordError::Parse`] when the stored hash is malformed.
pub fn verify_password(password: &str, stored_hash: &str) -> Result<bool, PasswordError> {
    let parsed_hash = PasswordHash::new(stored_hash).map_err(|_| PasswordError::Parse)?;
    Ok(argon2()?
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok())
}

fn argon2() -> Result<Argon2<'static>, PasswordError> {
    let params = Params::new(
        ARGON2_MEMORY_KIB,
        ARGON2_ITERATIONS,
        ARGON2_PARALLELISM,
        None,
    )
    .map_err(|_| PasswordError::Params)?;
    Ok(Argon2::new(Algorithm::Argon2id, Version::V0x13, params))
}
