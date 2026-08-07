//! gizza-ai/x25519-ecdh core — X25519 (Curve25519) Diffie-Hellman: derive the
//! shared secret from your private key and a peer's public key, optionally
//! expanding it into a usable symmetric key with HKDF or SHA-256.
//!
//! Design notes:
//!
//! * Either key field may be left empty. An empty private key means "I don't
//!   have one yet" → a fresh pair is generated from the OS CSPRNG. An empty
//!   peer public key means "show me the whole exchange" → a demo peer pair is
//!   generated and both of its halves are printed, so a first-time user can see
//!   an end-to-end agreement without opening two browser tabs.
//! * Keys are accepted in every encoding real tooling pastes: raw 32 bytes as
//!   hex (with or without `0x`), standard base64, URL-safe base64, or an RFC
//!   8410 PKCS#8 / SubjectPublicKeyInfo PEM block.
//! * The raw X25519 output is NOT a uniformly random key. `kdf` runs it through
//!   HKDF (RFC 5869) or a plain SHA-256; with `kdf = none` the report says so
//!   in its own output rather than burying the caveat in documentation.
//! * A peer public key that is a low-order point drives the shared secret to
//!   all zeros — a value any attacker can predict. That is rejected as an error
//!   (dalek's contributory check), never returned as a "result".

use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD, URL_SAFE, URL_SAFE_NO_PAD};
use base64::Engine;
use hkdf::Hkdf;
use sha2::{Digest, Sha256, Sha512};
use x25519_dalek::{PublicKey, StaticSecret};

/// RFC 8410 X25519 algorithm identifier `1.3.101.110`, the 3 content bytes of
/// the DER OBJECT IDENTIFIER.
const OID_X25519: [u8; 3] = [0x2b, 0x65, 0x6e];
/// RFC 8410 Ed25519 `1.3.101.112` — recognised only so a pasted signing key
/// gets a precise error instead of "could not decode".
const OID_ED25519: [u8; 3] = [0x2b, 0x65, 0x70];

/// Largest derived-key length we accept, = 255 * 32 (the HKDF-SHA256 ceiling,
/// the smaller of the two supported hashes).
pub const MAX_KDF_LENGTH: usize = 8160;

/// Which half of a key pair a field expects — drives both the PEM label check
/// and the wording of parse errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeyRole {
    Private,
    Public,
}

impl KeyRole {
    fn pem_label(self) -> &'static str {
        match self {
            KeyRole::Private => "PRIVATE KEY",
            KeyRole::Public => "PUBLIC KEY",
        }
    }
}

/// Post-processing applied to the raw 32-byte agreement output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kdf {
    /// Report the raw X25519 output unchanged (RFC 7748 semantics).
    None,
    /// HKDF-SHA256 extract-and-expand (RFC 5869).
    HkdfSha256,
    /// HKDF-SHA512 extract-and-expand (RFC 5869).
    HkdfSha512,
    /// A single SHA-256 over the raw secret — the simplest "make it look like a
    /// key" step, and what several handshake write-ups use.
    Sha256,
}

impl Kdf {
    /// Parse a `kdf` value (case-insensitive; empty means `none`).
    pub fn parse(s: &str) -> Result<Kdf, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "none" => Ok(Kdf::None),
            "hkdf-sha256" => Ok(Kdf::HkdfSha256),
            "hkdf-sha512" => Ok(Kdf::HkdfSha512),
            "sha256" => Ok(Kdf::Sha256),
            other => Err(format!(
                "unknown kdf {other:?} (expected none, hkdf-sha256, hkdf-sha512, or sha256)"
            )),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Kdf::None => "none",
            Kdf::HkdfSha256 => "hkdf-sha256",
            Kdf::HkdfSha512 => "hkdf-sha512",
            Kdf::Sha256 => "sha256",
        }
    }
}

/// Output encoding for every raw byte string in the report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    Hex,
    Base64,
    Base64Url,
}

impl Encoding {
    /// Parse an `encoding` value (case-insensitive; empty means `hex`).
    pub fn parse(s: &str) -> Result<Encoding, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "hex" => Ok(Encoding::Hex),
            "base64" => Ok(Encoding::Base64),
            "base64url" => Ok(Encoding::Base64Url),
            other => Err(format!(
                "unknown encoding {other:?} (expected hex, base64, or base64url)"
            )),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Encoding::Hex => "hex",
            Encoding::Base64 => "base64",
            Encoding::Base64Url => "base64url",
        }
    }

    fn encode(self, bytes: &[u8]) -> String {
        match self {
            Encoding::Hex => hex::encode(bytes),
            Encoding::Base64 => STANDARD.encode(bytes),
            Encoding::Base64Url => URL_SAFE_NO_PAD.encode(bytes),
        }
    }
}

/// Everything the report needs. Grouped in a struct so the block, the CLI and
/// the browser export all pass the same shape.
#[derive(Debug, Clone)]
pub struct Request<'a> {
    /// Your X25519 private key, any accepted encoding. Empty → generate one.
    pub private_key: &'a str,
    /// The peer's X25519 public key, any accepted encoding. Empty → generate a
    /// demo peer pair.
    pub peer_public_key: &'a str,
    pub kdf: Kdf,
    /// HKDF salt, read as UTF-8 bytes. Empty → no salt (RFC 5869 zero salt).
    pub kdf_salt: &'a str,
    /// HKDF info / context label, read as UTF-8 bytes.
    pub kdf_info: &'a str,
    /// Derived key length in bytes (HKDF only).
    pub kdf_length: usize,
    pub encoding: Encoding,
    /// Also print PKCS#8 / SPKI PEM blocks for the keys.
    pub include_pem: bool,
}

/// The structured result behind the text report — kept public so callers that
/// want fields rather than prose (the chat block's JSON response) can use them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Agreement {
    pub private_key: [u8; 32],
    pub public_key: [u8; 32],
    pub peer_public_key: [u8; 32],
    /// Present only when the peer pair was generated here.
    pub peer_private_key: Option<[u8; 32]>,
    pub shared_secret: [u8; 32],
    /// Present when `kdf` is not `none`.
    pub derived_key: Option<Vec<u8>>,
    pub generated_private_key: bool,
}

/// Fill a 32-byte buffer from the platform CSPRNG.
fn random_32(what: &str) -> Result<[u8; 32], String> {
    let mut buf = [0u8; 32];
    getrandom::getrandom(&mut buf)
        .map_err(|e| format!("could not generate a {what}: no system randomness available ({e})"))?;
    Ok(buf)
}

/// Decode the base64 body of a PEM block with the given label.
fn pem_body(text: &str, role: KeyRole, field: &str) -> Result<Vec<u8>, String> {
    let want = role.pem_label();
    let other = match role {
        KeyRole::Private => "PUBLIC KEY",
        KeyRole::Public => "PRIVATE KEY",
    };
    if !text.contains(&format!("-----BEGIN {want}-----")) {
        if text.contains(&format!("-----BEGIN {other}-----")) {
            return Err(format!(
                "{field}: got a `-----BEGIN {other}-----` block, but this field needs a \
                 `-----BEGIN {want}-----` block"
            ));
        }
        return Err(format!(
            "{field}: unsupported PEM block — expected `-----BEGIN {want}-----`"
        ));
    }
    let body: String = text
        .lines()
        .skip_while(|l| !l.trim().starts_with("-----BEGIN"))
        .skip(1)
        .take_while(|l| !l.trim().starts_with("-----END"))
        .flat_map(|l| l.chars())
        .filter(|c| !c.is_ascii_whitespace())
        .collect();
    STANDARD
        .decode(body.as_bytes())
        .map_err(|e| format!("{field}: the PEM body is not valid base64 ({e})"))
}

/// Pull the 32-byte key out of an RFC 8410 DER structure: the PKCS#8 v1
/// `OneAsymmetricKey` (private) or `SubjectPublicKeyInfo` (public). Both are
/// fixed short-form byte templates for Curve25519, so the prefixes are exact.
fn key_from_der(der: &[u8], role: KeyRole, field: &str) -> Result<[u8; 32], String> {
    let (prefix, total): (Vec<u8>, usize) = match role {
        KeyRole::Private => (
            [
                &[0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03][..],
                &OID_X25519[..],
                &[0x04, 0x22, 0x04, 0x20][..],
            ]
            .concat(),
            48,
        ),
        KeyRole::Public => (
            [
                &[0x30, 0x2a, 0x30, 0x05, 0x06, 0x03][..],
                &OID_X25519[..],
                &[0x03, 0x21, 0x00][..],
            ]
            .concat(),
            44,
        ),
    };
    let has_ed = der.windows(OID_ED25519.len()).any(|w| w == OID_ED25519);
    let has_x = der.windows(OID_X25519.len()).any(|w| w == OID_X25519);
    if has_ed && !has_x {
        return Err(format!(
            "{field}: this is an Ed25519 key (OID 1.3.101.112). Ed25519 signs; X25519 \
             (OID 1.3.101.110) does key agreement — they are not interchangeable"
        ));
    }
    if der.len() != total || !der.starts_with(&prefix) {
        return Err(format!(
            "{field}: unsupported DER layout ({} bytes) — expected the {} RFC 8410 \
             X25519 structure OpenSSL writes ({total} bytes)",
            der.len(),
            match role {
                KeyRole::Private => "PKCS#8",
                KeyRole::Public => "SubjectPublicKeyInfo",
            }
        ));
    }
    let mut out = [0u8; 32];
    out.copy_from_slice(&der[total - 32..]);
    Ok(out)
}

/// Parse a key field: PEM, hex (optionally `0x`-prefixed), standard base64, or
/// URL-safe base64 — always yielding the raw 32 bytes.
fn parse_key(input: &str, role: KeyRole, field: &str) -> Result<[u8; 32], String> {
    let text = input.trim();
    if text.contains("-----BEGIN") {
        let der = pem_body(text, role, field)?;
        return key_from_der(&der, role, field);
    }

    let compact: String = text.chars().filter(|c| !c.is_ascii_whitespace()).collect();
    let compact = compact
        .strip_prefix("0x")
        .or_else(|| compact.strip_prefix("0X"))
        .unwrap_or(&compact)
        .to_string();

    let all_hex = !compact.is_empty() && compact.chars().all(|c| c.is_ascii_hexdigit());
    if all_hex && compact.len() == 64 {
        let bytes = hex::decode(&compact).map_err(|e| format!("{field}: bad hex ({e})"))?;
        let mut out = [0u8; 32];
        out.copy_from_slice(&bytes);
        return Ok(out);
    }

    // Every base64 flavour real tooling emits: padded/unpadded, standard/URL-safe.
    let attempts = [
        STANDARD.decode(compact.as_bytes()),
        STANDARD_NO_PAD.decode(compact.as_bytes()),
        URL_SAFE.decode(compact.as_bytes()),
        URL_SAFE_NO_PAD.decode(compact.as_bytes()),
    ];
    let mut decoded_len: Option<usize> = None;
    for bytes in attempts.into_iter().flatten() {
        if bytes.len() == 32 {
            let mut out = [0u8; 32];
            out.copy_from_slice(&bytes);
            return Ok(out);
        }
        decoded_len.get_or_insert(bytes.len());
    }

    // A hex-looking string is almost certainly meant as hex, so say so in hex
    // terms even though the base64 alphabet would also have accepted it.
    if all_hex {
        return Err(format!(
            "{field}: {} hex characters, but a 32-byte X25519 key is 64 hex characters",
            compact.len()
        ));
    }
    if let Some(n) = decoded_len {
        return Err(format!(
            "{field}: decoded to {n} bytes, but an X25519 key is exactly 32 bytes"
        ));
    }
    Err(format!(
        "{field}: could not decode {} character{} — expected 64 hex characters, \
         44-character base64 (standard or URL-safe), or a PEM block",
        compact.len(),
        if compact.len() == 1 { "" } else { "s" }
    ))
}

/// Wrap DER in RFC 7468 PEM armor with 64-character base64 lines.
fn pem_wrap(label: &str, der: &[u8]) -> String {
    let b64 = STANDARD.encode(der);
    let mut s = String::with_capacity(b64.len() + 64);
    s.push_str("-----BEGIN ");
    s.push_str(label);
    s.push_str("-----\n");
    for chunk in b64.as_bytes().chunks(64) {
        s.push_str(std::str::from_utf8(chunk).expect("base64 is ASCII"));
        s.push('\n');
    }
    s.push_str("-----END ");
    s.push_str(label);
    s.push_str("-----\n");
    s
}

/// RFC 8410 §7 PKCS#8 v1 DER for a 32-byte X25519 private key.
fn pkcs8_der(raw: &[u8; 32]) -> Vec<u8> {
    let mut v = Vec::with_capacity(48);
    v.extend_from_slice(&[0x30, 0x2e, 0x02, 0x01, 0x00, 0x30, 0x05, 0x06, 0x03]);
    v.extend_from_slice(&OID_X25519);
    v.extend_from_slice(&[0x04, 0x22, 0x04, 0x20]);
    v.extend_from_slice(raw);
    v
}

/// RFC 8410 §4 SubjectPublicKeyInfo DER for a 32-byte X25519 public key.
fn spki_der(raw: &[u8; 32]) -> Vec<u8> {
    let mut v = Vec::with_capacity(44);
    v.extend_from_slice(&[0x30, 0x2a, 0x30, 0x05, 0x06, 0x03]);
    v.extend_from_slice(&OID_X25519);
    v.extend_from_slice(&[0x03, 0x21, 0x00]);
    v.extend_from_slice(raw);
    v
}

/// Run the agreement (and any KDF) without formatting a report.
pub fn agree(req: &Request) -> Result<Agreement, String> {
    let (private_raw, generated_private_key) = if req.private_key.trim().is_empty() {
        (random_32("private key")?, true)
    } else {
        (
            parse_key(req.private_key, KeyRole::Private, "your private key")?,
            false,
        )
    };
    let secret = StaticSecret::from(private_raw);
    let public_raw = PublicKey::from(&secret).to_bytes();

    let (peer_public_raw, peer_private_key) = if req.peer_public_key.trim().is_empty() {
        let peer_priv = random_32("peer key pair")?;
        let peer_pub = PublicKey::from(&StaticSecret::from(peer_priv)).to_bytes();
        (peer_pub, Some(peer_priv))
    } else {
        (
            parse_key(req.peer_public_key, KeyRole::Public, "peer public key")?,
            None,
        )
    };

    let shared = secret.diffie_hellman(&PublicKey::from(peer_public_raw));
    if !shared.was_contributory() {
        return Err(
            "peer public key is a low-order point: the X25519 shared secret comes out all \
             zeros, a value anyone can predict. Reject this key and ask the peer for a real one."
                .to_string(),
        );
    }
    let shared_secret = shared.to_bytes();

    let derived_key = match req.kdf {
        Kdf::None => None,
        Kdf::Sha256 => Some(Sha256::digest(shared_secret).to_vec()),
        Kdf::HkdfSha256 | Kdf::HkdfSha512 => {
            if req.kdf_length == 0 || req.kdf_length > MAX_KDF_LENGTH {
                return Err(format!(
                    "kdf_length must be between 1 and {MAX_KDF_LENGTH} bytes; got {}",
                    req.kdf_length
                ));
            }
            let salt = (!req.kdf_salt.is_empty()).then(|| req.kdf_salt.as_bytes());
            let info = req.kdf_info.as_bytes();
            let mut okm = vec![0u8; req.kdf_length];
            let expanded = if req.kdf == Kdf::HkdfSha256 {
                Hkdf::<Sha256>::new(salt, &shared_secret).expand(info, &mut okm)
            } else {
                Hkdf::<Sha512>::new(salt, &shared_secret).expand(info, &mut okm)
            };
            expanded.map_err(|_| {
                format!(
                    "kdf_length {} is too long for {}",
                    req.kdf_length,
                    req.kdf.label()
                )
            })?;
            Some(okm)
        }
    };

    Ok(Agreement {
        private_key: private_raw,
        public_key: public_raw,
        peer_public_key: peer_public_raw,
        peer_private_key,
        shared_secret,
        derived_key,
        generated_private_key,
    })
}

/// Run the agreement and format the human-readable report every surface shows.
pub fn derive_report(req: &Request) -> Result<String, String> {
    let a = agree(req)?;
    let enc = req.encoding;
    let mut out = String::new();

    out.push_str(&format!(
        "X25519 ECDH · shared secret derived ({})\n\n",
        enc.label()
    ));
    let row = |label: &str, bytes: &[u8]| format!("{:<19}{}\n", label, enc.encode(bytes));
    out.push_str(&row("Your private key", &a.private_key));
    out.push_str(&row("Your public key", &a.public_key));
    out.push_str(&row("Peer public key", &a.peer_public_key));
    if let Some(pk) = a.peer_private_key {
        out.push_str(&row("Peer private key", &pk));
    }
    out.push('\n');
    out.push_str(&row("Shared secret", &a.shared_secret));
    if let Some(dk) = &a.derived_key {
        out.push_str(&row("Derived key", dk));
    }

    let mut notes: Vec<String> = Vec::new();
    match (req.kdf, &a.derived_key) {
        (Kdf::None, _) => notes.push(
            "The raw shared secret is not a uniformly random key. Set kdf = hkdf-sha256 to \
             expand it into one you can use directly."
                .to_string(),
        ),
        (kdf, Some(dk)) => {
            let mut detail = format!("Derived with {} · {} bytes", kdf.label(), dk.len());
            if kdf != Kdf::Sha256 {
                detail.push_str(&format!(
                    " · salt: {} · info: {}",
                    describe_kdf_input(req.kdf_salt),
                    describe_kdf_input(req.kdf_info)
                ));
            }
            notes.push(detail);
        }
        _ => {}
    }
    if a.generated_private_key {
        notes.push(
            "Your key pair was generated here with the system CSPRNG. Copy the private key \
             now — nothing is stored, so it cannot be recovered later."
                .to_string(),
        );
    }
    if a.peer_private_key.is_some() {
        notes.push(
            "The peer key pair was generated here so you can see both sides of one exchange. \
             Paste your peer's real public key to do the agreement for real."
                .to_string(),
        );
    }
    if !notes.is_empty() {
        out.push('\n');
        for n in &notes {
            out.push_str(n);
            out.push('\n');
        }
    }

    if req.include_pem {
        out.push_str("\nYour private key (PKCS#8 PEM)\n");
        out.push_str(&pem_wrap("PRIVATE KEY", &pkcs8_der(&a.private_key)));
        out.push_str("Your public key (SPKI PEM)\n");
        out.push_str(&pem_wrap("PUBLIC KEY", &spki_der(&a.public_key)));
        out.push_str("Peer public key (SPKI PEM)\n");
        out.push_str(&pem_wrap("PUBLIC KEY", &spki_der(&a.peer_public_key)));
    }

    Ok(out.trim_end().to_string())
}

/// String-in / string-out wrapper every surface funnels through: the chat
/// block, the CLI and the browser export all hand over the same eight raw
/// fields, so the enum parsing (and its error wording) happens in exactly one
/// place. Delegates to [`derive_report`].
#[allow(clippy::too_many_arguments)]
pub fn run(
    private_key: &str,
    peer_public_key: &str,
    kdf: &str,
    kdf_salt: &str,
    kdf_info: &str,
    kdf_length: usize,
    encoding: &str,
    include_pem: bool,
) -> Result<String, String> {
    derive_report(&Request {
        private_key,
        peer_public_key,
        kdf: Kdf::parse(kdf)?,
        kdf_salt,
        kdf_info,
        kdf_length,
        encoding: Encoding::parse(encoding)?,
        include_pem,
    })
}

/// Render a salt/info value for the detail line: quoted, or `(none)` if empty.
fn describe_kdf_input(s: &str) -> String {
    if s.is_empty() {
        "(none)".to_string()
    } else {
        format!("{s:?}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // RFC 7748 §6.1 Diffie-Hellman test vector.
    const ALICE_PRIV: &str = "77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a";
    const ALICE_PUB: &str = "8520f0098930a754748b7ddcb43ef75a0dbf3a0d26381af4eba4a98eaa9b4e6a";
    const BOB_PRIV: &str = "5dab087e624a8a4b79e17f8b83800ee66f3bb1292618b6fd1c2f8b27ff88e0eb";
    const BOB_PUB: &str = "de9edb7d7b7dc1b4d35b61c2ece435373f8343c85b78674dadfc7e146f882b4f";
    const SHARED: &str = "4a5d9d5ba4ce2de1728e3bf480350f25e07e21c947d19e3376f09b3c1e161742";

    fn req<'a>(private_key: &'a str, peer_public_key: &'a str) -> Request<'a> {
        Request {
            private_key,
            peer_public_key,
            kdf: Kdf::None,
            kdf_salt: "",
            kdf_info: "",
            kdf_length: 32,
            encoding: Encoding::Hex,
            include_pem: false,
        }
    }

    /// Happy path: the RFC 7748 §6.1 vector, end to end and in both directions.
    #[test]
    fn rfc7748_vector_agrees_both_ways() {
        let a = agree(&req(ALICE_PRIV, BOB_PUB)).unwrap();
        assert_eq!(hex::encode(a.public_key), ALICE_PUB);
        assert_eq!(hex::encode(a.shared_secret), SHARED);
        assert!(!a.generated_private_key);
        assert!(a.peer_private_key.is_none());

        let b = agree(&req(BOB_PRIV, ALICE_PUB)).unwrap();
        assert_eq!(hex::encode(b.public_key), BOB_PUB);
        assert_eq!(hex::encode(b.shared_secret), SHARED, "both sides agree");
    }

    #[test]
    fn report_layout_is_exact() {
        let out = derive_report(&req(ALICE_PRIV, BOB_PUB)).unwrap();
        assert_eq!(
            out,
            "X25519 ECDH · shared secret derived (hex)\n\
             \n\
             Your private key   77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a\n\
             Your public key    8520f0098930a754748b7ddcb43ef75a0dbf3a0d26381af4eba4a98eaa9b4e6a\n\
             Peer public key    de9edb7d7b7dc1b4d35b61c2ece435373f8343c85b78674dadfc7e146f882b4f\n\
             \n\
             Shared secret      4a5d9d5ba4ce2de1728e3bf480350f25e07e21c947d19e3376f09b3c1e161742\n\
             \n\
             The raw shared secret is not a uniformly random key. Set kdf = hkdf-sha256 to \
             expand it into one you can use directly."
        );
    }

    /// RFC 7748 §5.2 single-iteration vectors: scalar × u-coordinate. Feeding
    /// the u-coordinate in as the peer public key is exactly that computation.
    #[test]
    fn rfc7748_section_5_2_scalar_mult_vectors() {
        for (scalar, u, want) in [
            (
                "a546e36bf0527c9d3b16154b82465edd62144c0ac1fc5a18506a2244ba449ac4",
                "e6db6867583030db3594c1a424b15f7c726624ec26b3353b10a903a6d0ab1c4c",
                "c3da55379de9c6908e94ea4df28d084f32eccf03491c71f754b4075577a28552",
            ),
            (
                "4b66e9d4d1b4673c5ad22691957d6af5c11b6421e0ea01d42ca4169e7918ba0d",
                "e5210f12786811d3f4b7959d0538ae2c31dbe7106fc03c3efc4cd549c715a493",
                "95cbde9476e8907d7aade45cb4b873f88b595a68799fa152e6f8f7647aac7957",
            ),
        ] {
            let a = agree(&req(scalar, u)).unwrap();
            assert_eq!(hex::encode(a.shared_secret), want);
        }
    }

    #[test]
    fn accepts_base64_pem_and_hex_forms_of_the_same_key() {
        let want = agree(&req(ALICE_PRIV, BOB_PUB)).unwrap();
        let priv_raw: [u8; 32] = hex::decode(ALICE_PRIV).unwrap().try_into().unwrap();
        let pub_raw: [u8; 32] = hex::decode(BOB_PUB).unwrap().try_into().unwrap();

        for (p, q) in [
            (STANDARD.encode(priv_raw), STANDARD.encode(pub_raw)),
            (
                URL_SAFE_NO_PAD.encode(priv_raw),
                URL_SAFE_NO_PAD.encode(pub_raw),
            ),
            (format!("0x{ALICE_PRIV}"), format!("0X{BOB_PUB}")),
            (
                pem_wrap("PRIVATE KEY", &pkcs8_der(&priv_raw)),
                pem_wrap("PUBLIC KEY", &spki_der(&pub_raw)),
            ),
        ] {
            let got = agree(&req(&p, &q)).unwrap();
            assert_eq!(got.shared_secret, want.shared_secret, "form {p:?} agrees");
        }
    }

    /// Error path: a key that is the wrong size must say so in bytes.
    #[test]
    fn rejects_short_key_with_a_specific_message() {
        let err = agree(&req("deadbeef", BOB_PUB)).unwrap_err();
        assert_eq!(
            err,
            "your private key: 8 hex characters, but a 32-byte X25519 key is 64 hex characters"
        );
        let err = agree(&req(ALICE_PRIV, "AAAAAAAAAAAAAAAAAAAAAA==")).unwrap_err();
        assert_eq!(
            err,
            "peer public key: decoded to 16 bytes, but an X25519 key is exactly 32 bytes"
        );
    }

    /// Error path: the low-order all-zero result is refused, not reported.
    #[test]
    fn rejects_low_order_peer_public_key() {
        let all_zero = "0".repeat(64);
        let err = agree(&req(ALICE_PRIV, &all_zero)).unwrap_err();
        assert!(err.starts_with("peer public key is a low-order point"), "{err}");
        // Order-8 point from the classic low-order set — also all-zero output.
        let order8 = "e0eb7a7c3b41b8ae1656e3faf19fc46ada098deb9c32b1fd866205165f49b800";
        assert!(agree(&req(ALICE_PRIV, order8)).is_err());
    }

    #[test]
    fn rejects_ed25519_pem_by_naming_the_oid() {
        // Same RFC 8410 template, Ed25519 OID — what a pasted signing key looks like.
        let mut der = pkcs8_der(&[7u8; 32]);
        der[11] = 0x70;
        let pem = pem_wrap("PRIVATE KEY", &der);
        let err = agree(&req(&pem, BOB_PUB)).unwrap_err();
        assert!(err.contains("Ed25519 key (OID 1.3.101.112)"), "{err}");
    }

    #[test]
    fn rejects_swapped_pem_labels() {
        let pub_raw: [u8; 32] = hex::decode(BOB_PUB).unwrap().try_into().unwrap();
        let pem = pem_wrap("PRIVATE KEY", &pkcs8_der(&pub_raw));
        let err = agree(&req(ALICE_PRIV, &pem)).unwrap_err();
        assert_eq!(
            err,
            "peer public key: got a `-----BEGIN PRIVATE KEY-----` block, but this field needs \
             a `-----BEGIN PUBLIC KEY-----` block"
        );
    }

    /// HKDF-SHA256 over the RFC vector, cross-checked against the hkdf crate
    /// driven directly — the tool must not add hidden inputs of its own.
    #[test]
    fn hkdf_matches_a_direct_rfc5869_run() {
        let mut r = req(ALICE_PRIV, BOB_PUB);
        r.kdf = Kdf::HkdfSha256;
        r.kdf_salt = "gizza salt";
        r.kdf_info = "chat v1";
        r.kdf_length = 44;
        let got = agree(&r).unwrap().derived_key.unwrap();

        let ikm = hex::decode(SHARED).unwrap();
        let mut want = vec![0u8; 44];
        Hkdf::<Sha256>::new(Some(b"gizza salt"), &ikm)
            .expand(b"chat v1", &mut want)
            .unwrap();
        assert_eq!(got, want);
        assert_eq!(got.len(), 44);
    }

    #[test]
    fn sha256_kdf_hashes_the_raw_secret() {
        let mut r = req(ALICE_PRIV, BOB_PUB);
        r.kdf = Kdf::Sha256;
        let got = agree(&r).unwrap().derived_key.unwrap();
        assert_eq!(got, Sha256::digest(hex::decode(SHARED).unwrap()).to_vec());
    }

    #[test]
    fn hkdf_sha512_differs_from_sha256_at_the_same_length() {
        let mut r = req(ALICE_PRIV, BOB_PUB);
        r.kdf = Kdf::HkdfSha256;
        let a = agree(&r).unwrap().derived_key.unwrap();
        r.kdf = Kdf::HkdfSha512;
        let b = agree(&r).unwrap().derived_key.unwrap();
        assert_eq!(a.len(), 32);
        assert_eq!(b.len(), 32);
        assert_ne!(a, b);
    }

    #[test]
    fn rejects_out_of_range_kdf_length() {
        let mut r = req(ALICE_PRIV, BOB_PUB);
        r.kdf = Kdf::HkdfSha256;
        r.kdf_length = MAX_KDF_LENGTH + 1;
        assert_eq!(
            agree(&r).unwrap_err(),
            "kdf_length must be between 1 and 8160 bytes; got 8161"
        );
        r.kdf_length = MAX_KDF_LENGTH;
        assert_eq!(agree(&r).unwrap().derived_key.unwrap().len(), MAX_KDF_LENGTH);
    }

    #[test]
    fn empty_fields_generate_a_working_demo_exchange() {
        let a = agree(&req("", "")).unwrap();
        assert!(a.generated_private_key);
        let peer_priv = a.peer_private_key.expect("demo peer pair");
        // The peer must be able to reach the same secret from its own side.
        let back = StaticSecret::from(peer_priv).diffie_hellman(&PublicKey::from(a.public_key));
        assert_eq!(back.to_bytes(), a.shared_secret);
        // Two runs never repeat.
        assert_ne!(agree(&req("", "")).unwrap().private_key, a.private_key);
    }

    #[test]
    fn generated_private_key_still_uses_a_supplied_peer_key() {
        let a = agree(&req("", BOB_PUB)).unwrap();
        assert!(a.generated_private_key);
        assert!(a.peer_private_key.is_none());
        assert_eq!(hex::encode(a.peer_public_key), BOB_PUB);
    }

    #[test]
    fn include_pem_round_trips_through_our_own_parser() {
        let mut r = req(ALICE_PRIV, BOB_PUB);
        r.include_pem = true;
        let out = derive_report(&r).unwrap();
        assert!(out.contains("-----BEGIN PRIVATE KEY-----"));
        assert_eq!(out.matches("-----BEGIN PUBLIC KEY-----").count(), 2);
        // Pull the private PEM back out and re-parse it.
        let start = out.find("-----BEGIN PRIVATE KEY-----").unwrap();
        let end = out.find("-----END PRIVATE KEY-----").unwrap() + "-----END PRIVATE KEY-----".len();
        let parsed = parse_key(&out[start..end], KeyRole::Private, "f").unwrap();
        assert_eq!(hex::encode(parsed), ALICE_PRIV);
    }

    /// The string-in/string-out wrapper must agree with the typed path and
    /// surface the enum parse errors verbatim.
    #[test]
    fn run_wrapper_matches_the_typed_request() {
        let want = derive_report(&req(ALICE_PRIV, BOB_PUB)).unwrap();
        let got = run(ALICE_PRIV, BOB_PUB, "none", "", "", 32, "hex", false).unwrap();
        assert_eq!(got, want);

        let hkdf = run(ALICE_PRIV, BOB_PUB, "hkdf-sha256", "s", "i", 16, "base64", false).unwrap();
        assert!(hkdf.contains("Derived key"));
        assert!(hkdf.starts_with("X25519 ECDH · shared secret derived (base64)"));

        assert!(run(ALICE_PRIV, BOB_PUB, "hkdf-md5", "", "", 32, "hex", false)
            .unwrap_err()
            .contains("unknown kdf"));
        assert!(run(ALICE_PRIV, BOB_PUB, "none", "", "", 32, "base32", false)
            .unwrap_err()
            .contains("unknown encoding"));
    }

    #[test]
    fn encodings_render_the_same_bytes_three_ways() {
        let raw = hex::decode(SHARED).unwrap();
        assert_eq!(Encoding::Hex.encode(&raw), SHARED);
        assert_eq!(Encoding::Base64.encode(&raw), STANDARD.encode(&raw));
        assert_eq!(Encoding::Base64Url.encode(&raw), URL_SAFE_NO_PAD.encode(&raw));
        assert!(!Encoding::Base64Url.encode(&raw).contains('+'));
        assert_eq!(Encoding::parse("BASE64").unwrap(), Encoding::Base64);
        assert!(Encoding::parse("base32").is_err());
        assert_eq!(Kdf::parse("").unwrap(), Kdf::None);
        assert!(Kdf::parse("hkdf-md5").is_err());
    }
}
