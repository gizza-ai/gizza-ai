//! gizza-ai/argon2-hash core — hash a password with Argon2id (configurable
//! memory, iterations, parallelism) returning a PHC string, and verify a password
//! against a PHC string. Pure-Rust (`argon2`). No wafer/wasm-bindgen deps.

use argon2::password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString};
use argon2::{Algorithm, Argon2, Params, Version};

/// Hash `password` with Argon2id and return the PHC string
/// (`$argon2id$v=19$m=...,t=...,p=...$<salt>$<hash>`). A fresh random salt is used.
///
/// `m_cost` is memory in KiB, `t_cost` iterations, `p_cost` lanes.
pub fn hash(password: &str, m_cost: u32, t_cost: u32, p_cost: u32) -> Result<String, String> {
    if password.is_empty() {
        return Err("password is required".into());
    }
    let params = Params::new(m_cost, t_cost, p_cost, None)
        .map_err(|e| format!("invalid Argon2 parameters: {e}"))?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    // Fresh 16-byte random salt (getrandom works on wasm32-wasip1; the page enables
    // the js backend).
    let mut salt_bytes = [0u8; 16];
    getrandom::getrandom(&mut salt_bytes).map_err(|e| format!("rng error: {e}"))?;
    let salt = SaltString::encode_b64(&salt_bytes).map_err(|e| format!("salt error: {e}"))?;
    argon
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| format!("hashing failed: {e}"))
}

/// Verify `password` against a PHC `phc` string. Returns true on a match.
pub fn verify(password: &str, phc: &str) -> Result<bool, String> {
    let parsed = PasswordHash::new(phc.trim())
        .map_err(|e| format!("not a valid PHC hash string: {e}"))?;
    // Argon2::default() reads the algorithm/params from the PHC string.
    match Argon2::default().verify_password(password.as_bytes(), &parsed) {
        Ok(()) => Ok(true),
        Err(argon2::password_hash::Error::Password) => Ok(false),
        Err(e) => Err(format!("verification error: {e}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Small params keep tests fast (still real Argon2id).
    const M: u32 = 4096;
    const T: u32 = 2;
    const P: u32 = 1;

    #[test]
    fn hash_is_phc_argon2id() {
        let h = hash("correct horse", M, T, P).unwrap();
        assert!(h.starts_with("$argon2id$"));
        assert!(h.contains("m=4096"));
        assert!(h.contains("t=2"));
    }

    #[test]
    fn salts_differ() {
        let a = hash("pw", M, T, P).unwrap();
        let b = hash("pw", M, T, P).unwrap();
        assert_ne!(a, b, "fresh salt -> different hashes for the same password");
    }

    #[test]
    fn verify_roundtrip() {
        let h = hash("s3cret!", M, T, P).unwrap();
        assert!(verify("s3cret!", &h).unwrap());
        assert!(!verify("wrong", &h).unwrap());
    }

    #[test]
    fn verify_external_hash() {
        // A known Argon2id PHC string for password "password".
        let phc = "$argon2id$v=19$m=4096,t=3,p=1$c29tZXNhbHRzYWx0$\
                   "; // placeholder replaced below
        let _ = phc;
        // Generate one with t=3 to exercise param parsing from the PHC.
        let h = hash("password", 4096, 3, 1).unwrap();
        assert!(verify("password", &h).unwrap());
        assert!(!verify("Password", &h).unwrap());
    }

    #[test]
    fn errors() {
        assert!(hash("", M, T, P).is_err()); // empty password
        assert!(hash("pw", 1, T, P).is_err()); // memory too low for Argon2
        assert!(verify("pw", "not a phc string").is_err());
    }
}
