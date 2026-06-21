//! gizza-ai/bcrypt-hash core — hash a password with bcrypt at a chosen cost
//! returning a modular-crypt `$2b$` string, and verify a password against an
//! existing bcrypt hash. Pure-Rust (`bcrypt`). No wafer/wasm-bindgen deps.

/// bcrypt only considers the first 72 bytes of a password. Longer inputs are
/// silently truncated by the algorithm; we surface a clear error instead.
const BCRYPT_MAX_BYTES: usize = 72;

/// Hash `password` with bcrypt at the given `cost` (work factor, 4..=31) and
/// return a modular-crypt-format string (`$2b$<cost>$<22-char-salt><31-char-hash>`).
/// A fresh random salt is generated.
pub fn hash(password: &str, cost: u32) -> Result<String, String> {
    if password.is_empty() {
        return Err("password is required".into());
    }
    if password.len() > BCRYPT_MAX_BYTES {
        return Err(format!(
            "password is {} bytes; bcrypt only hashes the first {BCRYPT_MAX_BYTES} bytes — \
             shorten it or pre-hash long inputs",
            password.len()
        ));
    }
    if !(4..=31).contains(&cost) {
        return Err(format!("cost must be between 4 and 31 (got {cost})"));
    }
    bcrypt::hash(password, cost).map_err(|e| format!("hashing failed: {e}"))
}

/// Verify `password` against an existing bcrypt `hash` string. Returns true on
/// a match. Accepts the `$2a$`, `$2b$`, `$2x$`, `$2y$` variants.
pub fn verify(password: &str, hash: &str) -> Result<bool, String> {
    bcrypt::verify(password, hash.trim())
        .map_err(|e| format!("not a valid bcrypt hash (or verify error): {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Low cost keeps tests fast (still real bcrypt).
    const COST: u32 = 4;

    #[test]
    fn hash_is_bcrypt_mcf() {
        let h = hash("correct horse", COST).unwrap();
        assert!(h.starts_with("$2b$04$"), "got {h}");
        // $2b$ + 2-digit cost + $ + 22 salt + 31 hash = 60 chars
        assert_eq!(h.len(), 60, "bcrypt MCF strings are 60 chars");
    }

    #[test]
    fn salts_differ() {
        let a = hash("pw", COST).unwrap();
        let b = hash("pw", COST).unwrap();
        assert_ne!(a, b, "fresh salt -> different hashes for the same password");
    }

    #[test]
    fn verify_roundtrip() {
        let h = hash("s3cret!", COST).unwrap();
        assert!(verify("s3cret!", &h).unwrap());
        assert!(!verify("wrong", &h).unwrap());
    }

    #[test]
    fn verify_external_hash() {
        // A real bcrypt hash of "password" (cost 10, $2b$ variant) produced by an
        // independent implementation — exercises parsing a hash we didn't make.
        let known = "$2b$10$odaUG9d1NgVaLONfFhi3hOg4b7Xje3Mw.OF/nyPS0EBKUUbkpDDIW";
        assert!(verify("password", known).unwrap());
        assert!(!verify("Password", known).unwrap());
    }

    #[test]
    fn verify_2y_variant() {
        // Same digest, PHP-style $2y$ variant tag — bcrypt treats 2a/2b/2x/2y alike.
        let phpish = "$2y$10$odaUG9d1NgVaLONfFhi3hOg4b7Xje3Mw.OF/nyPS0EBKUUbkpDDIW";
        assert!(verify("password", phpish).unwrap());
    }

    #[test]
    fn errors() {
        assert!(hash("", COST).is_err()); // empty password
        assert!(hash("pw", 3).is_err()); // cost too low
        assert!(hash("pw", 32).is_err()); // cost too high
        assert!(hash(&"x".repeat(73), COST).is_err()); // > 72 bytes
        assert!(verify("pw", "not a bcrypt string").is_err());
    }
}
