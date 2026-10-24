//! Password and TOTP primitives only; callers rate-limit and persist accepted TOTP steps atomically.
use crate::AuthKey;
use argon2::{
    Argon2, Params,
    password_hash::{PasswordHasher, PasswordVerifier, phc::PasswordHash},
};
use chacha20poly1305::{
    XChaCha20Poly1305,
    aead::{Aead, KeyInit, Payload},
};
use totp_rs::{Algorithm, Builder, Secret, Totp};

type Result<T> = std::result::Result<T, &'static str>;

pub fn validate_password(password: &str) -> Result<()> {
    if password.len() > 512 || !(12..=128).contains(&password.chars().count()) {
        return Err("Password must contain 12–128 characters and at most 512 UTF-8 bytes");
    }
    Ok(())
}

/// Argon2id v19: 19 MiB, two iterations, one lane, independently generated 128-bit salt.
/// Run hashing/verification on a bounded blocking pool, never directly on Tokio workers.
pub fn hash_password(password: &str) -> Result<String> {
    validate_password(password)?;
    let mut salt = [0; 16];
    getrandom::fill(&mut salt).map_err(|_| "Password salt generation failed")?;
    Argon2::default()
        .hash_password_with_salt(password.as_bytes(), &salt)
        .map(|hash: PasswordHash| hash.to_string())
        .map_err(|_| "Password hashing failed")
}

pub fn verify_password(password: &str, encoded: &str) -> Result<bool> {
    validate_password(password)?;
    if encoded.len() > 256 {
        return Err("Stored password hash is too long");
    }
    let hash = PasswordHash::new(encoded).map_err(|_| "Invalid stored password hash")?;
    let params =
        Params::try_from(&hash.params).map_err(|_| "Invalid stored password parameters")?;
    // Only our fixed policy is accepted, before any expensive allocation or computation.
    if hash.algorithm.as_str() != "argon2id"
        || hash.version != Some(19)
        || params != Params::DEFAULT
        || hash.salt.as_ref().is_none_or(|salt| salt.len() != 16)
        || hash.hash.as_ref().is_none_or(|output| output.len() != 32)
    {
        return Err("Unsupported stored password hash policy");
    }
    match Argon2::default().verify_password(password.as_bytes(), &hash) {
        Ok(()) => Ok(true),
        Err(argon2::password_hash::Error::PasswordInvalid) => Ok(false),
        Err(_) => Err("Password verification failed"),
    }
}

pub fn new_totp_secret() -> Result<[u8; 20]> {
    let mut secret = [0; 20];
    getrandom::fill(&mut secret).map_err(|_| "TOTP secret generation failed")?;
    Ok(secret)
}

fn totp(secret: &[u8]) -> Result<Totp> {
    let bytes: [u8; 20] = secret
        .try_into()
        .map_err(|_| "Invalid TOTP secret length")?;
    Builder::new()
        .with_algorithm(Algorithm::SHA1)
        .with_secret(Secret::new_stack(bytes))
        .with_digits(6)
        .with_step_duration(30)
        .with_skew(1)
        .build()
        .map_err(|_| "Invalid TOTP configuration")
}

pub fn totp_uri(secret: &[u8], account: &str) -> Result<String> {
    if account.is_empty() || account.len() > 254 || account.chars().any(char::is_control) {
        return Err("Invalid TOTP account label");
    }
    let bytes: [u8; 20] = secret
        .try_into()
        .map_err(|_| "Invalid TOTP secret length")?;
    Builder::new()
        .with_secret(Secret::new_stack(bytes))
        .with_digits(6)
        .with_step_duration(30)
        .with_skew(1)
        .with_issuer(Some("Rapidmaker"))
        .with_account_name(account)
        .build()
        .map_err(|_| "Invalid TOTP configuration")?
        .to_url()
        .map_err(|_| "Invalid TOTP provisioning URI")
}

/// Return the matched 30-second counter; storage must require it to exceed the previous one.
pub fn verify_totp(secret: &[u8], code: &str, unix_seconds: u64) -> Result<Option<u64>> {
    if code.len() != 6 || !code.bytes().all(|byte| byte.is_ascii_digit()) {
        return Ok(None);
    }
    // Bound time before totp-rs multiplies a skewed counter by its step duration.
    if unix_seconds > u64::MAX - 60 {
        return Err("TOTP timestamp is out of range");
    }
    Ok(totp(secret)?.check(code, unix_seconds))
}

fn account_aad(account_id: &str) -> Result<&[u8]> {
    if account_id.is_empty() || account_id.len() > 128 {
        return Err("Invalid TOTP account identifier");
    }
    Ok(account_id.as_bytes())
}

/// Version byte + random 192-bit nonce + encrypted 160-bit secret and authentication tag.
pub fn seal_totp(key: &AuthKey, account_id: &str, secret: &[u8]) -> Result<Vec<u8>> {
    let aad = account_aad(account_id)?;
    if secret.len() != 20 {
        return Err("Invalid TOTP secret length");
    }
    let cipher = XChaCha20Poly1305::new(&key.mac(&[b"totp-secret-encryption-v1"]).into());
    let mut nonce = [0; 24];
    getrandom::fill(&mut nonce).map_err(|_| "TOTP nonce generation failed")?;
    let ciphertext = cipher
        .encrypt(&nonce.into(), Payload { msg: secret, aad })
        .map_err(|_| "TOTP secret encryption failed")?;
    let mut sealed = Vec::with_capacity(61);
    sealed.push(1);
    sealed.extend_from_slice(&nonce);
    sealed.extend_from_slice(&ciphertext);
    Ok(sealed)
}

pub fn open_totp(key: &AuthKey, account_id: &str, sealed: &[u8]) -> Result<Vec<u8>> {
    let aad = account_aad(account_id)?;
    if sealed.len() != 61 || sealed[0] != 1 {
        return Err("Invalid encrypted TOTP secret");
    }
    let cipher = XChaCha20Poly1305::new(&key.mac(&[b"totp-secret-encryption-v1"]).into());
    let mut nonce = [0; 24];
    nonce.copy_from_slice(&sealed[1..25]);
    cipher
        .decrypt(
            &nonce.into(),
            Payload {
                msg: &sealed[25..],
                aad,
            },
        )
        .map_err(|_| "TOTP secret authentication failed")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_hashes_are_salted_and_reject_wrong_password_and_expensive_parameters() {
        let password = "a valid password";
        let first = hash_password(password).unwrap();
        assert_ne!(first, hash_password(password).unwrap());
        assert!(verify_password(password, &first).unwrap());
        assert!(!verify_password("a wrong password", &first).unwrap());
        assert!(verify_password(password, &first.replace("m=19456", "m=4294967295")).is_err());
        assert!(validate_password("short").is_err());
        assert!(validate_password(&"🙂".repeat(128)).is_ok());
        assert!(validate_password(&"🙂".repeat(129)).is_err());
    }

    #[test]
    fn totp_matches_rfc6238_and_returns_counter_for_atomic_replay_protection() {
        // RFC6238 SHA1 vector at t=59: 94287082, reduced to six digits.
        let secret = b"12345678901234567890";
        assert_eq!(verify_totp(secret, "287082", 59).unwrap(), Some(1));
        assert_eq!(verify_totp(secret, "287082", 89).unwrap(), Some(1));
        assert_eq!(verify_totp(secret, "287082", 90).unwrap(), None);
        assert_eq!(verify_totp(secret, "287082", 29).unwrap(), Some(1));
        assert_eq!(verify_totp(secret, "28708 ", 59).unwrap(), None);
        assert!(verify_totp(secret, "287082", u64::MAX).is_err());
        let uri = totp_uri(secret, "test@example.com").unwrap();
        assert!(uri.starts_with("otpauth://totp/"));
        assert!(uri.contains("issuer=Rapidmaker"));
        let provisioned = Totp::from_url(&uri).unwrap();
        assert_eq!(provisioned.digits(), 6);
        assert_eq!(provisioned.step(), 30);
        assert_eq!(provisioned.check("287082", 59), Some(1));
    }

    #[test]
    fn encrypted_totp_secrets_bind_account_key_and_ciphertext() {
        let key = AuthKey::from_hex(&"ab".repeat(32)).unwrap();
        let secret = new_totp_secret().unwrap();
        let mut sealed = seal_totp(&key, "account-a", &secret).unwrap();
        assert_eq!(open_totp(&key, "account-a", &sealed).unwrap(), secret);
        assert_ne!(sealed, seal_totp(&key, "account-a", &secret).unwrap());
        assert!(open_totp(&key, "account-b", &sealed).is_err());
        let other_key = AuthKey::from_hex(&"cd".repeat(32)).unwrap();
        assert!(open_totp(&other_key, "account-a", &sealed).is_err());
        sealed[30] ^= 1;
        assert!(open_totp(&key, "account-a", &sealed).is_err());
        assert!(open_totp(&key, "account-a", &sealed[..10]).is_err());
    }
}
