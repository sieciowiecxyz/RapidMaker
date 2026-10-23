//! Secret generation and domain-separated verifiers; no HTTP or persistence.
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

type HmacSha256 = Hmac<Sha256>;

pub struct AuthKey([u8; 32]);

impl AuthKey {
    pub fn from_hex(value: &str) -> Result<Self, &'static str> {
        let bytes = decode_token(value).ok_or("AUTH_CODE_KEY must be 32 random bytes in hex")?;
        Ok(Self(bytes))
    }

    pub fn verifier(&self, id: &str, email: &str, browser_hash: &[u8], code: &str) -> [u8; 32] {
        self.mac(&[
            b"login-code-v1",
            id.as_bytes(),
            email.as_bytes(),
            browser_hash,
            code.as_bytes(),
        ])
    }

    pub fn csrf(&self, session_token: &str) -> String {
        hex(&self.mac(&[b"csrf-v1", session_token.as_bytes()]))
    }

    pub fn verify_csrf(&self, token: &str, csrf: &str) -> bool {
        decode_token(csrf).is_some_and(|supplied| {
            bool::from(self.mac(&[b"csrf-v1", token.as_bytes()]).ct_eq(&supplied))
        })
    }

    fn mac(&self, fields: &[&[u8]]) -> [u8; 32] {
        // HMAC accepts every key length, including our fixed 32-byte key.
        let mut mac = HmacSha256::new_from_slice(&self.0).expect("valid HMAC key length");
        for field in fields {
            mac.update(&(field.len() as u64).to_be_bytes());
            mac.update(field);
        }
        mac.finalize().into_bytes().into()
    }
}

pub fn hash_token(token: &str) -> [u8; 32] {
    Sha256::digest(token.as_bytes()).into()
}

pub fn valid_token(token: &str) -> bool {
    decode_token(token).is_some()
}

pub fn constant_eq(a: &[u8], b: &[u8]) -> bool {
    bool::from(a.ct_eq(b))
}

pub fn random_token() -> Result<String, getrandom::Error> {
    let mut bytes = [0; 32];
    getrandom::fill(&mut bytes)?;
    Ok(hex(&bytes))
}

pub fn random_code() -> Result<String, getrandom::Error> {
    // Rejection sampling avoids modulo bias for the 100,000,000 possible codes.
    const RANGE: u32 = 100_000_000;
    const CEILING: u32 = u32::MAX - u32::MAX % RANGE;
    loop {
        let mut bytes = [0; 4];
        getrandom::fill(&mut bytes)?;
        let value = u32::from_le_bytes(bytes);
        if value < CEILING {
            return Ok(format!("{:08}", value % RANGE));
        }
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn decode_token(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let mut result = [0; 32];
    for (index, byte) in result.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).ok()?;
    }
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifier_binds_every_field_and_csrf_domain() {
        let key = AuthKey::from_hex(&"ab".repeat(32)).unwrap();
        let verifier = key.verifier("id", "a@b.pl", &[1; 32], "00000001");
        for altered in [
            key.verifier("other", "a@b.pl", &[1; 32], "00000001"),
            key.verifier("id", "b@b.pl", &[1; 32], "00000001"),
            key.verifier("id", "a@b.pl", &[2; 32], "00000001"),
            key.verifier("id", "a@b.pl", &[1; 32], "00000002"),
        ] {
            assert!(!constant_eq(&verifier, &altered));
        }
        let token = random_token().unwrap();
        assert!(key.verify_csrf(&token, &key.csrf(&token)));
        assert!(!key.verify_csrf("another", &key.csrf(&token)));
        assert!(!key.verify_csrf(&token, "not hex"));
        assert!(AuthKey::from_hex("short").is_err());
    }

    #[test]
    fn generated_secrets_have_expected_format() {
        let a = random_token().unwrap();
        let b = random_token().unwrap();
        assert!(valid_token(&a));
        assert_ne!(a, b);
        for _ in 0..32 {
            let code = random_code().unwrap();
            assert_eq!(code.len(), 8);
            assert!(code.bytes().all(|b| b.is_ascii_digit()));
        }
    }
}
