//! ssh-keygen core — generate an OpenSSH key pair (Ed25519 or RSA) locally.
//!
//! Unlike the PKCS#8/SPKI generators (ed25519-key-pair-generator,
//! generate-rsa-key-pair), this emits the keys in the NATIVE OpenSSH wire
//! formats produced by `ssh-keygen(1)`:
//!   * private key — the `-----BEGIN OPENSSH PRIVATE KEY-----` blob (id_ed25519 / id_rsa)
//!   * public key  — the single-line authorized_keys form (`ssh-ed25519 AAAA... comment`)
//! plus the SHA256 fingerprint OpenSSH prints on load.
//!
//! Pure Rust (getrandom CSPRNG) → runs on all backends. No standalone page:
//! key generation is non-deterministic, so it doesn't fit the page's
//! recompute-on-input model. Surfaces: chat + CLI.

use serde::Serialize;
use ssh_key::private::{Ed25519Keypair, KeypairData, PrivateKey, RsaKeypair};
use ssh_key::rand_core::OsRng;
use ssh_key::{HashAlg, LineEnding};

/// Minimum accepted RSA modulus size, matching `ssh-keygen` (1024-bit keys are
/// long-deprecated and rejected by modern OpenSSH).
pub const MIN_RSA_BITS: usize = 2048;

/// A freshly generated OpenSSH key pair.
#[derive(Debug, Serialize)]
pub struct SshKeyPair {
    /// Private key in OpenSSH format (`-----BEGIN OPENSSH PRIVATE KEY-----`).
    pub private_key: String,
    /// Public key as a single authorized_keys line (`ssh-ed25519 AAAA... comment`).
    pub public_key: String,
    /// SHA256 fingerprint of the public key (`SHA256:...`), as OpenSSH prints it.
    pub fingerprint: String,
    /// OpenSSH algorithm name: `ssh-ed25519` or `ssh-rsa`.
    pub algorithm: String,
    /// RSA modulus size in bits; omitted for Ed25519 (fixed 256-bit curve).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bits: Option<usize>,
}

/// Generate an OpenSSH key pair.
///
/// * `key_type` — `"ed25519"` (default, recommended) or `"rsa"`.
/// * `bits` — RSA modulus size (2048 / 3072 / 4096); ignored for Ed25519.
/// * `comment` — trailing comment on the public-key line (e.g. `user@host`);
///   empty for none.
pub fn generate(key_type: &str, bits: usize, comment: &str) -> Result<SshKeyPair, String> {
    let mut rng = OsRng;
    let comment = comment.trim();

    let (keydata, out_bits) = match key_type.trim().to_ascii_lowercase().as_str() {
        "ed25519" => (KeypairData::from(Ed25519Keypair::random(&mut rng)), None),
        "rsa" => {
            if bits < MIN_RSA_BITS {
                return Err(format!(
                    "RSA key size {bits} is too small; choose at least {MIN_RSA_BITS} (2048, 3072, or 4096)"
                ));
            }
            let kp = RsaKeypair::random(&mut rng, bits)
                .map_err(|e| format!("RSA key generation failed: {e}"))?;
            (KeypairData::from(kp), Some(bits))
        }
        other => {
            return Err(format!(
                "invalid key type '{other}'; expected 'ed25519' or 'rsa'"
            ))
        }
    };

    let key = PrivateKey::new(keydata, comment)
        .map_err(|e| format!("key construction failed: {e}"))?;

    let private_key = key
        .to_openssh(LineEnding::LF)
        .map_err(|e| format!("private key encoding failed: {e}"))?
        .to_string();
    let public_key = key
        .public_key()
        .to_openssh()
        .map_err(|e| format!("public key encoding failed: {e}"))?;
    let fingerprint = key.fingerprint(HashAlg::Sha256).to_string();
    let algorithm = key.algorithm().as_str().to_string();

    Ok(SshKeyPair {
        private_key,
        public_key,
        fingerprint,
        algorithm,
        bits: out_bits,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ssh_key::PublicKey;

    #[test]
    fn ed25519_happy_path() {
        let kp = generate("ed25519", 0, "alice@laptop").unwrap();
        // OpenSSH private-key envelope.
        assert!(kp.private_key.starts_with("-----BEGIN OPENSSH PRIVATE KEY-----"));
        assert!(kp.private_key.trim_end().ends_with("-----END OPENSSH PRIVATE KEY-----"));
        // authorized_keys line: "ssh-ed25519 <base64> alice@laptop".
        assert!(kp.public_key.starts_with("ssh-ed25519 "));
        assert!(kp.public_key.trim_end().ends_with(" alice@laptop"));
        assert_eq!(kp.algorithm, "ssh-ed25519");
        assert!(kp.bits.is_none());
        assert!(kp.fingerprint.starts_with("SHA256:"));

        // The emitted public key must actually parse back as OpenSSH, and its
        // fingerprint must match what we reported.
        let pk = PublicKey::from_openssh(&kp.public_key).expect("public key re-parses");
        assert_eq!(pk.fingerprint(HashAlg::Sha256).to_string(), kp.fingerprint);
        // And the private key round-trips, deriving the same public key.
        let sk = PrivateKey::from_openssh(&kp.private_key).expect("private key re-parses");
        assert_eq!(sk.public_key().to_openssh().unwrap(), kp.public_key);
    }

    #[test]
    fn rsa_2048_has_bits_and_prefix() {
        let kp = generate("rsa", 2048, "").unwrap();
        assert_eq!(kp.algorithm, "ssh-rsa");
        assert_eq!(kp.bits, Some(2048));
        assert!(kp.public_key.starts_with("ssh-rsa "));
        // No comment → no trailing space-delimited comment field.
        assert_eq!(kp.public_key.split(' ').count(), 2);
        assert!(kp.private_key.contains("OPENSSH PRIVATE KEY"));
    }

    #[test]
    fn unknown_type_errors() {
        let err = generate("dsa", 0, "").unwrap_err();
        assert!(err.contains("invalid key type"), "got: {err}");
    }

    #[test]
    fn rsa_too_small_errors() {
        let err = generate("rsa", 1024, "").unwrap_err();
        assert!(err.contains("too small"), "got: {err}");
    }

    #[test]
    fn key_type_is_case_insensitive() {
        let kp = generate("Ed25519", 0, "").unwrap();
        assert_eq!(kp.algorithm, "ssh-ed25519");
    }
}
