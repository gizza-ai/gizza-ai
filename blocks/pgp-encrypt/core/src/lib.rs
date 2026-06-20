//! gizza-ai/pgp-encrypt core — encrypt a message to one or more OpenPGP public
//! keys, producing an ASCII-armored encrypted block. Pure-Rust (rPGP): no
//! wafer/wasm-bindgen deps.
//!
//! For each recipient we encrypt to its first encryption-capable subkey (the
//! standard layout of GPG/Sequoia/Proton keys), falling back to the primary key
//! when it is itself encryption-capable. The session key is AES-256 and the
//! message is written in SEIP v1 (the widely-interoperable form).

use std::io;

use pgp::composed::{Deserializable, Message, SignedPublicKey};
use pgp::crypto::hash::HashAlgorithm;
use pgp::crypto::public_key::PublicKeyAlgorithm;
use pgp::crypto::sym::SymmetricKeyAlgorithm;
use pgp::errors::Result as PgpResult;
use pgp::packet::{PublicKey, PublicSubkey};
use pgp::types::{
    EskType, Fingerprint, KeyId, KeyVersion, PkeskBytes, PublicKeyTrait, PublicParams,
    SignatureBytes,
};
use rand::rngs::OsRng;

/// A recipient's chosen encryption key — either an encryption subkey or, for
/// keys without one, the (encryption-capable) primary key. Wrapping both in one
/// enum lets us pass a homogeneous `&[&Recipient]` slice to `encrypt_to_keys`,
/// even when different recipients use different key positions.
#[derive(Debug)]
enum Recipient<'a> {
    Primary(&'a PublicKey),
    Sub(&'a PublicSubkey),
}

/// Delegate a `PublicKeyTrait` method to whichever inner key the recipient holds.
macro_rules! deleg {
    ($self:ident, $k:ident => $call:expr) => {
        match $self {
            Recipient::Primary($k) => $call,
            Recipient::Sub($k) => $call,
        }
    };
}

impl PublicKeyTrait for Recipient<'_> {
    fn version(&self) -> KeyVersion {
        deleg!(self, k => k.version())
    }
    fn fingerprint(&self) -> Fingerprint {
        deleg!(self, k => k.fingerprint())
    }
    fn key_id(&self) -> KeyId {
        deleg!(self, k => k.key_id())
    }
    fn algorithm(&self) -> PublicKeyAlgorithm {
        deleg!(self, k => k.algorithm())
    }
    fn created_at(&self) -> &chrono::DateTime<chrono::Utc> {
        deleg!(self, k => k.created_at())
    }
    fn expiration(&self) -> Option<u16> {
        deleg!(self, k => k.expiration())
    }
    fn verify_signature(
        &self,
        hash: HashAlgorithm,
        data: &[u8],
        sig: &SignatureBytes,
    ) -> PgpResult<()> {
        deleg!(self, k => k.verify_signature(hash, data, sig))
    }
    fn encrypt<R: rand::CryptoRng + rand::Rng>(
        &self,
        rng: R,
        plain: &[u8],
        typ: EskType,
    ) -> PgpResult<PkeskBytes> {
        deleg!(self, k => k.encrypt(rng, plain, typ))
    }
    fn serialize_for_hashing(&self, writer: &mut impl io::Write) -> PgpResult<()> {
        deleg!(self, k => k.serialize_for_hashing(writer))
    }
    fn public_params(&self) -> &PublicParams {
        deleg!(self, k => k.public_params())
    }
}

/// Split a blob that may contain several concatenated ASCII-armored public-key
/// blocks into the individual block strings.
fn split_armored_blocks(input: &str) -> Vec<String> {
    const BEGIN: &str = "-----BEGIN PGP PUBLIC KEY BLOCK-----";
    const END: &str = "-----END PGP PUBLIC KEY BLOCK-----";
    let mut blocks = Vec::new();
    let mut rest = input;
    while let Some(start) = rest.find(BEGIN) {
        let after = &rest[start..];
        if let Some(end_rel) = after.find(END) {
            let end = end_rel + END.len();
            blocks.push(after[..end].to_string());
            rest = &after[end..];
        } else {
            break;
        }
    }
    blocks
}

/// Pick the encryption key to use for a recipient: first encryption-capable
/// subkey, else the primary key if it can encrypt.
fn pick_encryption_key(key: &SignedPublicKey) -> Result<Recipient<'_>, String> {
    if let Some(sub) = key
        .public_subkeys
        .iter()
        .find(|s| s.key.is_encryption_key())
    {
        return Ok(Recipient::Sub(&sub.key));
    }
    if key.primary_key.is_encryption_key() {
        return Ok(Recipient::Primary(&key.primary_key));
    }
    Err(format!(
        "public key {} has no encryption-capable key (it looks sign-only)",
        hex::encode_upper(key.primary_key.fingerprint().as_bytes())
    ))
}

/// Encrypt `message` to every public key found in `keys_pem` (one or more
/// ASCII-armored public-key blocks). Returns the ASCII-armored encrypted message.
pub fn run(message: &str, keys_pem: &str) -> Result<String, String> {
    let blocks = split_armored_blocks(keys_pem);
    if blocks.is_empty() {
        return Err(
            "no PGP public key found — paste one or more ASCII-armored '-----BEGIN PGP PUBLIC KEY BLOCK-----' keys"
                .into(),
        );
    }

    let mut keys: Vec<SignedPublicKey> = Vec::with_capacity(blocks.len());
    for (i, b) in blocks.iter().enumerate() {
        let (k, _) = SignedPublicKey::from_string(b)
            .map_err(|e| format!("public key #{} is not a valid PGP public key: {e}", i + 1))?;
        k.verify()
            .map_err(|e| format!("public key #{} failed self-verification: {e}", i + 1))?;
        keys.push(k);
    }

    let recipients: Vec<Recipient> = keys
        .iter()
        .map(pick_encryption_key)
        .collect::<Result<_, _>>()?;
    let recipient_refs: Vec<&Recipient> = recipients.iter().collect();

    let lit = Message::new_literal_bytes("", message.as_bytes());
    let mut rng = OsRng;
    let encrypted = lit
        .encrypt_to_keys_seipdv1(&mut rng, SymmetricKeyAlgorithm::AES256, &recipient_refs)
        .map_err(|e| format!("encryption failed: {e}"))?;

    encrypted
        .to_armored_string(Default::default())
        .map_err(|e| format!("failed to ASCII-armor the result: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pgp::composed::{KeyType, SecretKeyParamsBuilder, SubkeyParamsBuilder};
    use pgp::types::SecretKeyTrait;
    use smallvec::smallvec;

    /// Generate a signed (RSA primary + RSA encryption subkey) secret key for tests.
    fn gen_secret() -> pgp::SignedSecretKey {
        let mut subkey = SubkeyParamsBuilder::default();
        subkey
            .key_type(KeyType::Rsa(2048))
            .can_encrypt(true)
            .passphrase(None);
        let subkey = subkey.build().unwrap();
        let mut params = SecretKeyParamsBuilder::default();
        params
            .key_type(KeyType::Rsa(2048))
            .can_sign(true)
            .can_certify(true)
            .primary_user_id("Test <test@example.com>".into())
            .preferred_symmetric_algorithms(smallvec![SymmetricKeyAlgorithm::AES256])
            .subkey(subkey);
        let params = params.build().unwrap();
        let sk = params.generate(OsRng).unwrap();
        sk.sign(OsRng, || String::new()).unwrap()
    }

    fn armored_pub(sk: &pgp::SignedSecretKey) -> String {
        let pk = sk.public_key();
        let signed = pk.sign(OsRng, &sk.primary_key, || String::new()).unwrap();
        signed.to_armored_string(Default::default()).unwrap()
    }

    #[test]
    fn round_trip_single_recipient() {
        let sk = gen_secret();
        let pubkey = armored_pub(&sk);

        let armored = run("hello pgp", &pubkey).unwrap();
        assert!(armored.contains("-----BEGIN PGP MESSAGE-----"));

        // Decrypt with the matching secret key to confirm correctness.
        let (msg, _) = Message::from_string(&armored).unwrap();
        let (mut decrypted, _) = msg.decrypt(|| String::new(), &[&sk]).unwrap();
        if let Message::Compressed(_) = decrypted {
            decrypted = decrypted.decompress().unwrap();
        }
        let content = decrypted.get_content().unwrap().unwrap();
        assert_eq!(content, b"hello pgp");
    }

    #[test]
    fn round_trip_two_recipients() {
        let sk1 = gen_secret();
        let sk2 = gen_secret();
        let two = format!("{}\n{}", armored_pub(&sk1), armored_pub(&sk2));

        let armored = run("multi recipient", &two).unwrap();
        // Either secret key can decrypt the same message.
        for sk in [&sk1, &sk2] {
            let (msg, _) = Message::from_string(&armored).unwrap();
            let (mut decrypted, _) = msg.decrypt(|| String::new(), &[sk]).unwrap();
            if let Message::Compressed(_) = decrypted {
                decrypted = decrypted.decompress().unwrap();
            }
            let content = decrypted.get_content().unwrap().unwrap();
            assert_eq!(content, b"multi recipient");
        }
    }

    #[test]
    fn errors_without_a_key() {
        assert!(run("hi", "").is_err());
        assert!(run("hi", "not a key").is_err());
    }
}
