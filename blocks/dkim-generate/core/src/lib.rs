//! dkim-generate core — generate a DKIM signing key pair and the DNS TXT record
//! that publishes its public half. No wafer/wasm-bindgen deps.
//!
//! Two key types are supported:
//! * **RSA** (RFC 6376 `k=rsa`) at 1024/2048/4096 bits — the interoperable default.
//!   The private key is emitted as PKCS#8 *and* PKCS#1 PEM (OpenDKIM wants PKCS#1);
//!   the `p=` tag is the base64 of the DER SubjectPublicKeyInfo, as RFC 6376 requires.
//! * **Ed25519** (RFC 8463 `k=ed25519`) — the private key is the base64 of the raw
//!   32-byte seed (what OpenDKIM/rspamd store) plus a PKCS#8 PEM; `p=` is the base64
//!   of the raw 32-byte public key.
//!
//! An existing key can be pasted instead of generating a fresh one, so the record
//! can be rebuilt for a key that is already installed on a mail server.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine as _;
// `spki`'s Decode/EncodePublicKey traits are re-exported by both `rsa` and
// `ed25519-dalek` at the same version, so one import of each covers both key
// types. `PublicKeyParts` is what exposes the RSA modulus (for the bit length).
use ed25519_dalek::pkcs8::DecodePublicKey as EdDecodePublicKey;
use ed25519_dalek::{SigningKey, VerifyingKey};
use rsa::pkcs1::{DecodeRsaPrivateKey, DecodeRsaPublicKey, EncodeRsaPrivateKey};
use rsa::pkcs8::{DecodePrivateKey, EncodePrivateKey, EncodePublicKey, LineEnding};
use rsa::traits::PublicKeyParts;
use rsa::{RsaPrivateKey, RsaPublicKey};

/// A single DNS TXT string may hold at most 255 characters (RFC 1035 §3.3.14).
pub const DNS_STRING_LIMIT: usize = 255;

/// The TTL written into the BIND-style zone line and the text report.
pub const TTL: u32 = 3600;

/// A throwaway 1024-bit RSA key used by the unit tests and the page spec so the
/// whole report is deterministic. It signs nothing real — never use it.
pub const TEST_RSA_PKCS8_PEM: &str = "-----BEGIN PRIVATE KEY-----\n\
MIICdwIBADANBgkqhkiG9w0BAQEFAASCAmEwggJdAgEAAoGBAMlGx+vqjYM3RDkF\n\
Mi2JoV27om45PoIYfDwFTM2YfBjPFNDG2Vke+4TacCQWLi9Os9KBTG28RpbP6RBc\n\
EV3TCxlKQXyQ+WN6azWO0zdTTJpg5q81DstihBkn0Fko3myvfKu2yKBooQJfz3kU\n\
F4ScC0VUJKCgjY3YC1w2kQwjaV5xAgMBAAECgYEApP8dDEwuzY4UoxmbVLEqUwhp\n\
0ymiEEu6LAm8OB30PPIlAjDAI8q+LN5UZ4C3Q5ik2L+dw6c/xomRaRPQOLW0tgYY\n\
jHUiG2hncyGcnmBesT31FU5tM1TRfgi3L+243ItlNUqSa5NnAfVGvIqa2aH/a0IH\n\
7p24FzHUXAYe8vHR67ECQQDtj1XVmXpv7pnFB5VvfvbDHY3gZrjWBL8dS7AyYFMG\n\
bg7DmagUdD2I8tWcUVID3t3zwELzJIxDULT0LgN0LXnLAkEA2OZw2Yj2u8D2F19U\n\
0vLk/YiwyA4h5uC8l4CFVa0mRvHnpgwn0Y+SSjFkMzeHj8NHolNmK6JawV7jSadc\n\
PdbxMwJAGRZfASyxImr3MDiJznmBA7/2QLF4aZmcuTJKDDfh7Lbotj1e47IW4YSL\n\
jHOGDsSVee1e5KCCc4VtZrAJS1aWGwJAC3HNTo0dh7acxYrwLNlIH+CQg13LiCUf\n\
jlbnsieat2+YK3jEuRf8PhHxGYq6imlhZD1GTNMaOs3I7F/6TAEFZQJBAKh4FEEN\n\
9gOXNXgzGjq3PNHhVRE08eHzh79cwR+vDHmodNriiHPbnG14V/6eejhgLDJpexwr\n\
0E/EOfm3bP6UF0A=\n\
-----END PRIVATE KEY-----\n";

/// The `p=` tag value of [`TEST_RSA_PKCS8_PEM`] (base64 DER SubjectPublicKeyInfo).
pub const TEST_RSA_P_TAG: &str = "MIGfMA0GCSqGSIb3DQEBAQUAA4GNADCBiQKBgQDJRsfr6o2DN0Q5BTItiaFdu6JuOT6CGHw8BUzNmHwYzxTQxtlZHvuE2nAkFi4vTrPSgUxtvEaWz+kQXBFd0wsZSkF8kPljems1jtM3U0yaYOavNQ7LYoQZJ9BZKN5sr3yrtsigaKECX895FBeEnAtFVCSgoI2N2AtcNpEMI2lecQIDAQAB";

/// A throwaway Ed25519 seed (base64 of 32 raw bytes) for tests. Never use it.
pub const TEST_ED25519_SEED: &str = "8XreH6+LuIrTCrt0Gj1p8/SfKgzVHbT6SfyESLYd6Es=";

/// The `p=` tag value of [`TEST_ED25519_SEED`] (base64 of the raw public key).
pub const TEST_ED25519_P_TAG: &str = "mi4oZe5oURig5G66mm5QpOArHluNiVwjh2Q1i5QvUK8=";

/// What the caller asked to be generated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KeyChoice {
    Rsa(usize),
    Ed25519,
}

impl KeyChoice {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().replace('_', "-").as_str() {
            "rsa-1024" | "rsa1024" | "1024" => Ok(KeyChoice::Rsa(1024)),
            "rsa-2048" | "rsa2048" | "2048" | "" => Ok(KeyChoice::Rsa(2048)),
            "rsa-4096" | "rsa4096" | "4096" => Ok(KeyChoice::Rsa(4096)),
            "ed25519" | "ed-25519" => Ok(KeyChoice::Ed25519),
            other => Err(format!(
                "unknown key_type '{other}' (expected rsa-1024, rsa-2048, rsa-4096 or ed25519)"
            )),
        }
    }
}

/// The key material the record is built from. `Public` variants come from a
/// pasted public key: enough to publish DNS, but there is no private half.
enum Material {
    RsaPrivate(Box<RsaPrivateKey>),
    RsaPublic(Box<RsaPublicKey>),
    EdPrivate(Box<SigningKey>),
    EdPublic(Box<VerifyingKey>),
}

/// Where the key came from, for the report's `Key source:` line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Origin {
    Generated,
    SuppliedPrivate,
    SuppliedPublic,
}

impl Material {
    /// Human label, e.g. `RSA 2048-bit` / `Ed25519`.
    fn label(&self) -> String {
        match self {
            Material::RsaPrivate(k) => format!("RSA {}-bit", k.n().bits()),
            Material::RsaPublic(k) => format!("RSA {}-bit", k.n().bits()),
            Material::EdPrivate(_) | Material::EdPublic(_) => "Ed25519".to_string(),
        }
    }

    /// The DKIM `k=` tag value.
    fn k_tag(&self) -> &'static str {
        match self {
            Material::RsaPrivate(_) | Material::RsaPublic(_) => "rsa",
            Material::EdPrivate(_) | Material::EdPublic(_) => "ed25519",
        }
    }

    fn bits(&self) -> Option<usize> {
        match self {
            Material::RsaPrivate(k) => Some(k.n().bits()),
            Material::RsaPublic(k) => Some(k.n().bits()),
            _ => None,
        }
    }

    /// The `p=` tag: base64 DER SubjectPublicKeyInfo for RSA, base64 of the raw
    /// 32-byte key for Ed25519 (RFC 8463 §3).
    fn p_tag(&self) -> Result<String, String> {
        match self {
            Material::RsaPrivate(k) => spki_b64(&RsaPublicKey::from(k.as_ref())),
            Material::RsaPublic(k) => spki_b64(k),
            Material::EdPrivate(k) => Ok(B64.encode(k.verifying_key().to_bytes())),
            Material::EdPublic(k) => Ok(B64.encode(k.to_bytes())),
        }
    }

    /// Public key in SPKI PEM, the portable "here is my public half" form.
    fn public_pem(&self) -> Result<String, String> {
        let pem = match self {
            Material::RsaPrivate(k) => RsaPublicKey::from(k.as_ref())
                .to_public_key_pem(LineEnding::LF)
                .map_err(|e| format!("failed to encode public key: {e}"))?,
            Material::RsaPublic(k) => k
                .to_public_key_pem(LineEnding::LF)
                .map_err(|e| format!("failed to encode public key: {e}"))?,
            Material::EdPrivate(k) => k
                .verifying_key()
                .to_public_key_pem(LineEnding::LF)
                .map_err(|e| format!("failed to encode public key: {e}"))?,
            Material::EdPublic(k) => k
                .to_public_key_pem(LineEnding::LF)
                .map_err(|e| format!("failed to encode public key: {e}"))?,
        };
        Ok(pem.trim_end().to_string())
    }

    /// PKCS#8 private key PEM, or `None` when only a public key was supplied.
    fn private_pkcs8_pem(&self) -> Result<Option<String>, String> {
        Ok(match self {
            Material::RsaPrivate(k) => Some(
                k.to_pkcs8_pem(LineEnding::LF)
                    .map_err(|e| format!("failed to encode private key: {e}"))?
                    .trim_end()
                    .to_string(),
            ),
            Material::EdPrivate(k) => Some(
                k.to_pkcs8_pem(LineEnding::LF)
                    .map_err(|e| format!("failed to encode private key: {e}"))?
                    .trim_end()
                    .to_string(),
            ),
            _ => None,
        })
    }

    /// PKCS#1 (`BEGIN RSA PRIVATE KEY`) PEM — what OpenDKIM and several ESPs read.
    fn private_pkcs1_pem(&self) -> Result<Option<String>, String> {
        Ok(match self {
            Material::RsaPrivate(k) => Some(
                k.to_pkcs1_pem(LineEnding::LF)
                    .map_err(|e| format!("failed to encode private key: {e}"))?
                    .trim_end()
                    .to_string(),
            ),
            _ => None,
        })
    }

    /// Base64 of the raw 32-byte Ed25519 seed (RFC 8463 private-key storage form).
    fn private_seed_b64(&self) -> Option<String> {
        match self {
            Material::EdPrivate(k) => Some(B64.encode(k.to_bytes())),
            _ => None,
        }
    }
}

fn spki_b64(key: &RsaPublicKey) -> Result<String, String> {
    let der = key
        .to_public_key_der()
        .map_err(|e| format!("failed to encode public key: {e}"))?;
    Ok(B64.encode(der.as_bytes()))
}

/// Normalize + validate the domain the selector will live under.
pub fn normalize_domain(input: &str) -> Result<String, String> {
    let mut d = input.trim().trim_matches('"').trim().to_ascii_lowercase();
    for prefix in ["https://", "http://"] {
        if let Some(rest) = d.strip_prefix(prefix) {
            d = rest.to_string();
        }
    }
    // A pasted address or URL: keep the host part only.
    if let Some((_, rest)) = d.rsplit_once('@') {
        d = rest.to_string();
    }
    if let Some((head, _)) = d.split_once('/') {
        d = head.to_string();
    }
    if let Some((head, _)) = d.split_once(':') {
        d = head.to_string();
    }
    // A pasted full record host (selector._domainkey.example.com) → example.com.
    if let Some((_, rest)) = d.split_once("._domainkey.") {
        d = rest.to_string();
    }
    let d = d.trim_end_matches('.').to_string();

    if d.is_empty() {
        return Err(
            "domain is required — enter the domain you send mail from, e.g. example.com"
                .to_string(),
        );
    }
    if !d.is_ascii() {
        return Err(format!(
            "domain '{d}' is not ASCII — convert an internationalized domain to its punycode \
             (xn--) form first"
        ));
    }
    if d.len() > 253 {
        return Err(format!(
            "domain is {} characters; the DNS limit is 253",
            d.len()
        ));
    }
    if !d.contains('.') {
        return Err(format!(
            "'{d}' is not a full domain name — include the top-level domain, e.g. example.com"
        ));
    }
    for label in d.split('.') {
        check_label(label, "domain")?;
    }
    Ok(d)
}

/// Normalize + validate the DKIM selector (the label left of `._domainkey`).
pub fn normalize_selector(input: &str) -> Result<String, String> {
    let mut s = input.trim().trim_matches('"').trim().to_ascii_lowercase();
    // A pasted record host: mail._domainkey.example.com → mail.
    if let Some((head, _)) = s.split_once("._domainkey") {
        s = head.to_string();
    }
    let s = s.trim_matches('.').to_string();

    if s.is_empty() {
        return Err(
            "selector is required — pick a short label such as mail, s1, or 2026a".to_string(),
        );
    }
    if !s.is_ascii() {
        return Err(format!(
            "selector '{s}' must be ASCII letters, digits, hyphens or dots"
        ));
    }
    if s.len() > 63 {
        return Err(format!(
            "selector is {} characters; keep it to 63 or fewer so the DNS label stays valid",
            s.len()
        ));
    }
    for label in s.split('.') {
        check_label(label, "selector")?;
    }
    Ok(s)
}

fn check_label(label: &str, what: &str) -> Result<(), String> {
    if label.is_empty() {
        return Err(format!(
            "{what} has an empty label — check for a doubled or trailing dot"
        ));
    }
    if label.len() > 63 {
        return Err(format!(
            "{what} label '{label}' is longer than the 63-character DNS limit"
        ));
    }
    if label.starts_with('-') || label.ends_with('-') {
        return Err(format!(
            "{what} label '{label}' may not start or end with a hyphen"
        ));
    }
    if let Some(bad) = label
        .chars()
        .find(|c| !c.is_ascii_alphanumeric() && *c != '-')
    {
        return Err(format!(
            "{what} label '{label}' contains '{bad}' — only letters, digits and hyphens are allowed"
        ));
    }
    Ok(())
}

/// The `t=` flag tag: `none`, `y` (testing), `s` (no subdomain), or `y:s`.
fn flags_tag(flags: &str) -> Result<Option<&'static str>, String> {
    match flags.trim().to_ascii_lowercase().as_str() {
        "" | "none" => Ok(None),
        "y" => Ok(Some("y")),
        "s" => Ok(Some("s")),
        "y:s" | "s:y" => Ok(Some("y:s")),
        other => Err(format!(
            "unknown flags value '{other}' (expected none, y, s or y:s)"
        )),
    }
}

/// Parse a pasted key: PKCS#8 / PKCS#1 / SPKI PEM, a bare base64 Ed25519 seed,
/// or a `p=` tag value copied out of an existing record (public key only).
fn parse_supplied_key(input: &str) -> Result<(Material, Origin), String> {
    let text = input.trim();
    if text.contains("BEGIN OPENSSH PRIVATE KEY") {
        return Err(
            "OpenSSH-format keys are not supported — export the key as a PKCS#8 or \
                    PKCS#1 PEM (openssl pkey -in key -out key.pem) and paste that"
                .to_string(),
        );
    }
    if text.contains("BEGIN ENCRYPTED PRIVATE KEY") {
        return Err(
            "this private key is passphrase-encrypted — decrypt it first \
                    (openssl pkcs8 -in key.pem -out plain.pem) and paste the plain key"
                .to_string(),
        );
    }
    if text.contains("BEGIN RSA PRIVATE KEY") {
        let key = RsaPrivateKey::from_pkcs1_pem(text)
            .map_err(|e| format!("could not read the PKCS#1 RSA private key: {e}"))?;
        return Ok((Material::RsaPrivate(Box::new(key)), Origin::SuppliedPrivate));
    }
    if text.contains("BEGIN PRIVATE KEY") {
        if let Ok(key) = RsaPrivateKey::from_pkcs8_pem(text) {
            return Ok((Material::RsaPrivate(Box::new(key)), Origin::SuppliedPrivate));
        }
        let key = SigningKey::from_pkcs8_pem(text)
            .map_err(|e| format!("could not read the PKCS#8 private key as RSA or Ed25519: {e}"))?;
        return Ok((Material::EdPrivate(Box::new(key)), Origin::SuppliedPrivate));
    }
    if text.contains("BEGIN RSA PUBLIC KEY") {
        let key = RsaPublicKey::from_pkcs1_pem(text)
            .map_err(|e| format!("could not read the PKCS#1 RSA public key: {e}"))?;
        return Ok((Material::RsaPublic(Box::new(key)), Origin::SuppliedPublic));
    }
    if text.contains("BEGIN PUBLIC KEY") {
        if let Ok(key) = RsaPublicKey::from_public_key_pem(text) {
            return Ok((Material::RsaPublic(Box::new(key)), Origin::SuppliedPublic));
        }
        let key = VerifyingKey::from_public_key_pem(text)
            .map_err(|e| format!("could not read the public key as RSA or Ed25519: {e}"))?;
        return Ok((Material::EdPublic(Box::new(key)), Origin::SuppliedPublic));
    }
    if text.contains("BEGIN CERTIFICATE") {
        return Err(
            "that is a certificate, not a key — paste the private key PEM, or the \
                    public key PEM if you only want the DNS record"
                .to_string(),
        );
    }

    // No PEM armor: a bare base64 blob. 32 bytes = an Ed25519 seed or public key
    // (RFC 8463 stores both that way); anything else is a paste mistake.
    let compact: String = text.chars().filter(|c| !c.is_whitespace()).collect();
    // A leading `p=` is the DKIM public-key tag copied out of a record, so those
    // bytes are the *public* half and there is no private key to report. Without
    // the tag, 32 bare bytes stay an Ed25519 seed — the private form OpenDKIM and
    // rspamd store.
    let (compact, tagged_public) = match compact
        .strip_prefix("p=")
        .or_else(|| compact.strip_prefix("P="))
    {
        Some(rest) => (rest, true),
        None => (compact.as_str(), false),
    };
    let bytes = B64.decode(compact).map_err(|_| {
        "could not read the key you pasted — expected a PEM block (-----BEGIN PRIVATE KEY-----) \
         or a base64 Ed25519 key"
            .to_string()
    })?;
    match bytes.len() {
        32 if tagged_public => {
            let raw: [u8; 32] = bytes.try_into().expect("length checked");
            let key = VerifyingKey::from_bytes(&raw).map_err(|_| {
                "the p= value you pasted is 32 bytes but is not a valid Ed25519 public key"
                    .to_string()
            })?;
            Ok((Material::EdPublic(Box::new(key)), Origin::SuppliedPublic))
        }
        32 => {
            let seed: [u8; 32] = bytes.try_into().expect("length checked");
            Ok((
                Material::EdPrivate(Box::new(SigningKey::from_bytes(&seed))),
                Origin::SuppliedPrivate,
            ))
        }
        n if n > 32 => {
            // Most likely the base64 DER SubjectPublicKeyInfo out of a p= tag.
            let key = RsaPublicKey::from_public_key_der(&bytes).map_err(|_| {
                format!(
                    "the base64 you pasted decodes to {n} bytes, which is neither a 32-byte \
                     Ed25519 key nor a DER RSA public key"
                )
            })?;
            Ok((Material::RsaPublic(Box::new(key)), Origin::SuppliedPublic))
        }
        n => Err(format!(
            "the base64 you pasted decodes to {n} bytes; an Ed25519 key is exactly 32"
        )),
    }
}

fn generate(choice: KeyChoice) -> Result<Material, String> {
    let mut rng = rand::rngs::OsRng;
    match choice {
        KeyChoice::Rsa(bits) => {
            let key = RsaPrivateKey::new(&mut rng, bits)
                .map_err(|e| format!("RSA key generation failed: {e}"))?;
            Ok(Material::RsaPrivate(Box::new(key)))
        }
        KeyChoice::Ed25519 => Ok(Material::EdPrivate(Box::new(SigningKey::generate(
            &mut rng,
        )))),
    }
}

/// Split a TXT value into the 255-character strings a zone file needs.
pub fn chunk_txt(value: &str) -> Vec<String> {
    value
        .as_bytes()
        .chunks(DNS_STRING_LIMIT)
        .map(|c| String::from_utf8_lossy(c).into_owned())
        .collect()
}

/// Everything a caller needs about one generated selector.
pub struct DkimRecord {
    pub domain: String,
    pub selector: String,
    pub host: String,
    pub value: String,
    pub chunks: Vec<String>,
    pub bind: String,
    pub key_label: String,
    pub k_tag: &'static str,
    pub bits: Option<usize>,
    pub p_tag: String,
    pub public_pem: String,
    pub private_pkcs8_pem: Option<String>,
    pub private_pkcs1_pem: Option<String>,
    pub private_seed_b64: Option<String>,
    pub notes: Vec<String>,
    origin: Origin,
}

/// Build the record (generating a key unless one was supplied).
pub fn build(
    domain: &str,
    selector: &str,
    key_type: &str,
    include_hash: bool,
    flags: &str,
    private_key: &str,
) -> Result<DkimRecord, String> {
    let domain = normalize_domain(domain)?;
    let selector = normalize_selector(selector)?;
    let choice = KeyChoice::parse(key_type)?;
    let t_tag = flags_tag(flags)?;

    let (material, origin) = if private_key.trim().is_empty() {
        (generate(choice)?, Origin::Generated)
    } else {
        parse_supplied_key(private_key)?
    };

    let p_tag = material.p_tag()?;
    let mut value = String::from("v=DKIM1;");
    if include_hash {
        value.push_str(" h=sha256;");
    }
    value.push_str(&format!(" k={};", material.k_tag()));
    if let Some(t) = t_tag {
        value.push_str(&format!(" t={t};"));
    }
    value.push_str(&format!(" p={p_tag}"));

    let host = format!("{selector}._domainkey.{domain}");
    let chunks = chunk_txt(&value);
    let bind = if chunks.len() == 1 {
        format!("{host}. {TTL} IN TXT \"{}\"", chunks[0])
    } else {
        let body = chunks
            .iter()
            .map(|c| format!("  \"{c}\""))
            .collect::<Vec<_>>()
            .join("\n");
        format!("{host}. {TTL} IN TXT (\n{body} )")
    };

    let mut notes = Vec::new();
    match material.bits() {
        Some(b) if b < 2048 => notes.push(format!(
            "{b}-bit RSA is below what large mailbox providers now expect. Use 2048-bit unless a \
             legacy signer forces the smaller key."
        )),
        Some(b) if b > 2048 => notes.push(format!(
            "{b}-bit RSA produces a {} character record. Some DNS panels and older resolvers \
             refuse records this long — 2048-bit is the interoperable choice.",
            value.len()
        )),
        _ => {}
    }
    if material.k_tag() == "ed25519" {
        notes.push(
            "Ed25519 (RFC 8463) keys are short, but not every receiver verifies them yet. Publish \
             an RSA selector alongside this one and sign with both."
                .to_string(),
        );
    }
    if origin != Origin::Generated {
        notes.push(
            "The key came from what you pasted, so the Key type choice was ignored.".to_string(),
        );
    }
    if origin == Origin::SuppliedPublic {
        notes.push(
            "Only a public key was supplied — the private half is not shown because it cannot be \
             recovered from a public key."
                .to_string(),
        );
    }
    if t_tag == Some("y") || t_tag == Some("y:s") {
        notes.push(
            "t=y marks the selector as being in test mode, so receivers must not treat a \
             signature failure as a policy failure. Remove it once signing works."
                .to_string(),
        );
    }
    if chunks.len() > 1 {
        notes.push(format!(
            "The value is {} characters, so a zone file needs {} quoted strings of at most {} \
             characters. Most hosted DNS panels accept the single-line value and split it for you.",
            value.len(),
            chunks.len(),
            DNS_STRING_LIMIT
        ));
    }

    Ok(DkimRecord {
        domain,
        selector,
        host,
        value,
        chunks,
        bind,
        key_label: material.label(),
        k_tag: material.k_tag(),
        bits: material.bits(),
        p_tag,
        public_pem: material.public_pem()?,
        private_pkcs8_pem: material.private_pkcs8_pem()?,
        private_pkcs1_pem: material.private_pkcs1_pem()?,
        private_seed_b64: material.private_seed_b64(),
        notes,
        origin,
    })
}

impl DkimRecord {
    fn source_line(&self) -> &'static str {
        match self.origin {
            Origin::Generated => "generated locally just now (never uploaded)",
            Origin::SuppliedPrivate => "the private key you supplied",
            Origin::SuppliedPublic => "the public key you supplied",
        }
    }

    /// The default human-readable report.
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("DKIM key for {}\n", self.domain));
        out.push_str(&format!("  Selector:   {}\n", self.selector));
        out.push_str(&format!("  Key type:   {}\n", self.key_label));
        out.push_str(&format!("  Key source: {}\n", self.source_line()));

        out.push_str("\nDNS record to publish\n");
        out.push_str(&format!("  Host / Name: {}\n", self.host));
        out.push_str("  Type:        TXT\n");
        out.push_str(&format!(
            "  TTL:         {TTL} (or your provider's default)\n"
        ));
        out.push_str(&format!("  Value:       {}\n", self.value));

        out.push_str("\nZone-file form\n");
        for line in self.bind.lines() {
            out.push_str(&format!("  {line}\n"));
        }

        if let Some(pem) = &self.private_pkcs8_pem {
            out.push_str(
                "\nPrivate key, PKCS#8 PEM — keep secret, install on the signing server\n",
            );
            out.push_str(pem);
            out.push('\n');
        }
        if let Some(pem) = &self.private_pkcs1_pem {
            out.push_str("\nPrivate key, PKCS#1 PEM — the form OpenDKIM and several ESPs expect\n");
            out.push_str(pem);
            out.push('\n');
        }
        if let Some(seed) = &self.private_seed_b64 {
            out.push_str("\nPrivate key, base64 seed — the form OpenDKIM and rspamd store\n");
            out.push_str(seed);
            out.push('\n');
        }

        out.push_str("\nPublic key, SPKI PEM\n");
        out.push_str(&self.public_pem);
        out.push('\n');

        if !self.notes.is_empty() {
            out.push_str("\nNotes\n");
            for note in &self.notes {
                out.push_str(&format!("  - {note}\n"));
            }
        }
        out.trim_end().to_string()
    }

    /// Machine-readable output.
    pub fn to_json(&self) -> String {
        let value = serde_json::json!({
            "domain": self.domain,
            "selector": self.selector,
            "key_type": self.k_tag,
            "key_bits": self.bits,
            "dns_record": {
                "name": self.host,
                "type": "TXT",
                "ttl": TTL,
                "value": self.value,
                "value_length": self.value.len(),
                "chunks": self.chunks,
                "zone_file": self.bind,
            },
            "public_key": {
                "p_tag": self.p_tag,
                "pem": self.public_pem,
            },
            "private_key": {
                "pkcs8_pem": self.private_pkcs8_pem,
                "pkcs1_pem": self.private_pkcs1_pem,
                "base64_seed": self.private_seed_b64,
            },
            "notes": self.notes,
        });
        serde_json::to_string_pretty(&value).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
    }
}

/// Entry point shared by the chat block, the CLI and the page.
pub fn run(
    domain: &str,
    selector: &str,
    key_type: &str,
    format: &str,
    include_hash: bool,
    flags: &str,
    private_key: &str,
) -> Result<String, String> {
    let fmt = format.trim().to_ascii_lowercase();
    let fmt = if fmt.is_empty() {
        "text".to_string()
    } else {
        fmt
    };
    if !matches!(
        fmt.as_str(),
        "text" | "json" | "dns_value" | "zone_file" | "private_key" | "public_key"
    ) {
        return Err(format!(
            "unknown format '{fmt}' (expected text, json, dns_value, zone_file, private_key or \
             public_key)"
        ));
    }

    let record = build(domain, selector, key_type, include_hash, flags, private_key)?;
    Ok(match fmt.as_str() {
        "json" => record.to_json(),
        "dns_value" => record.value.clone(),
        "zone_file" => record.bind.clone(),
        "public_key" => record.public_pem.clone(),
        "private_key" => match (
            &record.private_pkcs1_pem,
            &record.private_pkcs8_pem,
            &record.private_seed_b64,
        ) {
            (Some(pkcs1), _, _) => pkcs1.clone(),
            (None, Some(pkcs8), _) => pkcs8.clone(),
            (None, None, Some(seed)) => seed.clone(),
            _ => {
                return Err(
                    "no private key to show — you supplied a public key, and a private key \
                     cannot be recovered from it"
                        .to_string(),
                )
            }
        },
        _ => record.to_text(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(private_key: &str) -> DkimRecord {
        build("example.com", "mail", "rsa-2048", true, "none", private_key).unwrap()
    }

    #[test]
    fn supplied_rsa_key_builds_the_expected_record() {
        let r = rec(TEST_RSA_PKCS8_PEM);
        assert_eq!(r.host, "mail._domainkey.example.com");
        assert_eq!(r.k_tag, "rsa");
        assert_eq!(r.bits, Some(1024));
        assert_eq!(r.p_tag, TEST_RSA_P_TAG);
        assert_eq!(
            r.value,
            format!("v=DKIM1; h=sha256; k=rsa; p={TEST_RSA_P_TAG}")
        );
        // 1024-bit RSA: 216-char p= tag, so the whole value still fits one string.
        assert_eq!(r.chunks.len(), 1);
        assert_eq!(
            r.bind,
            format!("mail._domainkey.example.com. 3600 IN TXT \"{}\"", r.value)
        );
        assert!(r
            .private_pkcs1_pem
            .unwrap()
            .starts_with("-----BEGIN RSA PRIVATE KEY-----"));
    }

    #[test]
    fn pkcs1_paste_matches_pkcs8_paste() {
        let pkcs1 = rec(TEST_RSA_PKCS8_PEM).private_pkcs1_pem.unwrap();
        assert_eq!(rec(&pkcs1).p_tag, TEST_RSA_P_TAG);
    }

    #[test]
    fn ed25519_seed_paste_uses_raw_public_key() {
        let r = build(
            "example.com",
            "s1",
            "rsa-2048",
            true,
            "none",
            TEST_ED25519_SEED,
        )
        .unwrap();
        assert_eq!(r.k_tag, "ed25519");
        assert_eq!(r.p_tag, TEST_ED25519_P_TAG);
        assert_eq!(
            r.value,
            format!("v=DKIM1; h=sha256; k=ed25519; p={TEST_ED25519_P_TAG}")
        );
        assert_eq!(r.private_seed_b64.as_deref(), Some(TEST_ED25519_SEED));
        assert!(r
            .private_pkcs8_pem
            .unwrap()
            .starts_with("-----BEGIN PRIVATE KEY-----"));
        assert!(r.notes.iter().any(|n| n.contains("Ed25519")));
    }

    #[test]
    fn ed25519_p_tag_paste_is_a_public_key_not_a_seed() {
        let r = build(
            "example.test",
            "s2",
            "ed25519",
            true,
            "s",
            &format!("p={TEST_ED25519_P_TAG}"),
        )
        .unwrap();
        assert_eq!(r.k_tag, "ed25519");
        // The pasted tag is republished as-is, not re-derived from a seed.
        assert_eq!(r.p_tag, TEST_ED25519_P_TAG);
        assert_eq!(
            r.value,
            format!("v=DKIM1; h=sha256; k=ed25519; t=s; p={TEST_ED25519_P_TAG}")
        );
        assert!(r.private_seed_b64.is_none());
        assert!(r.private_pkcs8_pem.is_none());
        assert!(r.private_pkcs1_pem.is_none());
        assert!(r.notes.iter().any(|n| n.contains("Only a public key")));

        // The same bytes without the tag stay an Ed25519 seed (the private form),
        // whose public half is a different value entirely.
        let seed = build("example.test", "s2", "ed25519", true, "s", TEST_ED25519_P_TAG).unwrap();
        assert_ne!(seed.p_tag, TEST_ED25519_P_TAG);
        assert_eq!(seed.private_seed_b64.as_deref(), Some(TEST_ED25519_P_TAG));
    }

    #[test]
    fn rsa_p_tag_paste_publishes_the_same_record() {
        let r = build(
            "example.com",
            "mail",
            "rsa-2048",
            true,
            "none",
            &format!("p={TEST_RSA_P_TAG}"),
        )
        .unwrap();
        assert_eq!(r.p_tag, TEST_RSA_P_TAG);
        assert_eq!(r.bits, Some(1024));
        assert!(r.private_pkcs8_pem.is_none());
    }

    #[test]
    fn public_key_paste_publishes_without_a_private_half() {
        let pub_pem = rec(TEST_RSA_PKCS8_PEM).public_pem;
        let r = rec(&pub_pem);
        assert_eq!(r.p_tag, TEST_RSA_P_TAG);
        assert!(r.private_pkcs8_pem.is_none());
        assert!(r.notes.iter().any(|n| n.contains("Only a public key")));
        let err = run(
            "example.com",
            "mail",
            "rsa-2048",
            "private_key",
            true,
            "none",
            &pub_pem,
        )
        .unwrap_err();
        assert!(err.contains("cannot be recovered"), "{err}");
    }

    #[test]
    fn p_tag_base64_round_trips_to_a_public_key() {
        let r = rec(TEST_RSA_PKCS8_PEM);
        let der = B64.decode(&r.p_tag).unwrap();
        let key = RsaPublicKey::from_public_key_der(&der).unwrap();
        assert_eq!(key.n().bits(), 1024);
    }

    #[test]
    fn tags_follow_the_options() {
        let plain = build("example.com", "mail", "", false, "none", TEST_RSA_PKCS8_PEM).unwrap();
        assert_eq!(plain.value, format!("v=DKIM1; k=rsa; p={TEST_RSA_P_TAG}"));
        let flagged = build("example.com", "mail", "", true, "y:s", TEST_RSA_PKCS8_PEM).unwrap();
        assert_eq!(
            flagged.value,
            format!("v=DKIM1; h=sha256; k=rsa; t=y:s; p={TEST_RSA_P_TAG}")
        );
        assert!(flagged.notes.iter().any(|n| n.contains("test mode")));
    }

    #[test]
    fn long_values_split_into_255_character_strings() {
        let value = "x".repeat(600);
        let chunks = chunk_txt(&value);
        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].len(), 255);
        assert_eq!(chunks[2].len(), 90);
    }

    #[test]
    fn generates_a_fresh_ed25519_key_pair() {
        let a = build("example.com", "mail", "ed25519", true, "none", "").unwrap();
        let b = build("example.com", "mail", "ed25519", true, "none", "").unwrap();
        assert_ne!(a.p_tag, b.p_tag, "each run generates a new key");
        assert_eq!(B64.decode(&a.p_tag).unwrap().len(), 32);
        assert_eq!(a.chunks.len(), 1);
    }

    #[test]
    fn generates_a_fresh_rsa_1024_key_pair() {
        // 1024 keeps the test fast while exercising the real RSA generator.
        let r = build("example.com", "mail", "rsa-1024", true, "none", "").unwrap();
        assert_eq!(r.bits, Some(1024));
        assert!(r
            .notes
            .iter()
            .any(|n| n.contains("below what large mailbox providers")));
        let der = B64.decode(&r.p_tag).unwrap();
        assert!(RsaPublicKey::from_public_key_der(&der).is_ok());
    }

    #[test]
    fn domain_and_selector_are_normalized() {
        assert_eq!(
            normalize_domain(" https://Example.COM/path ").unwrap(),
            "example.com"
        );
        assert_eq!(
            normalize_domain("user@mail.example.com").unwrap(),
            "mail.example.com"
        );
        assert_eq!(
            normalize_domain("mail._domainkey.example.com.").unwrap(),
            "example.com"
        );
        assert_eq!(normalize_selector(" S1 ").unwrap(), "s1");
        assert_eq!(
            normalize_selector("mail._domainkey.example.com").unwrap(),
            "mail"
        );
    }

    #[test]
    fn rejects_bad_input() {
        assert!(normalize_domain("")
            .unwrap_err()
            .contains("domain is required"));
        assert!(normalize_domain("localhost")
            .unwrap_err()
            .contains("full domain name"));
        assert!(normalize_domain("exa mple.com")
            .unwrap_err()
            .contains("only letters"));
        assert!(normalize_domain("bücher.de")
            .unwrap_err()
            .contains("punycode"));
        assert!(normalize_selector("")
            .unwrap_err()
            .contains("selector is required"));
        assert!(normalize_selector("-bad-").unwrap_err().contains("hyphen"));
        assert!(KeyChoice::parse("rsa-3000").is_err());
        assert!(flags_tag("maybe").is_err());
        assert!(run(
            "example.com",
            "mail",
            "rsa-2048",
            "yaml",
            true,
            "none",
            TEST_RSA_PKCS8_PEM
        )
        .unwrap_err()
        .contains("unknown format"));
    }

    #[test]
    fn rejects_unusable_pasted_keys() {
        // `.err().unwrap()` rather than `.unwrap_err()`: the latter needs
        // `DkimRecord: Debug`, and a struct holding private key PEMs should not
        // print itself into a panic message or a stray `{:?}`.
        let err = build("example.com", "mail", "", true, "none", "not a key")
            .err()
            .unwrap();
        assert!(err.contains("could not read the key"), "{err}");
        let err = build(
            "example.com",
            "mail",
            "",
            true,
            "none",
            "-----BEGIN ENCRYPTED PRIVATE KEY-----\nAAAA\n-----END ENCRYPTED PRIVATE KEY-----",
        )
        .err()
        .unwrap();
        assert!(err.contains("passphrase-encrypted"), "{err}");
    }

    #[test]
    fn formats_return_their_own_slice_of_the_record() {
        let args = (
            "example.com",
            "mail",
            "rsa-2048",
            true,
            "none",
            TEST_RSA_PKCS8_PEM,
        );
        let r = build(args.0, args.1, args.2, args.3, args.4, args.5).unwrap();
        assert_eq!(
            run(args.0, args.1, args.2, "dns_value", args.3, args.4, args.5).unwrap(),
            r.value
        );
        assert_eq!(
            run(args.0, args.1, args.2, "zone_file", args.3, args.4, args.5).unwrap(),
            r.bind
        );
        assert_eq!(
            run(args.0, args.1, args.2, "public_key", args.3, args.4, args.5).unwrap(),
            r.public_pem
        );
        assert_eq!(
            run(
                args.0,
                args.1,
                args.2,
                "private_key",
                args.3,
                args.4,
                args.5
            )
            .unwrap(),
            r.private_pkcs1_pem.unwrap()
        );
        let json: serde_json::Value = serde_json::from_str(
            &run(args.0, args.1, args.2, "json", args.3, args.4, args.5).unwrap(),
        )
        .unwrap();
        assert_eq!(json["dns_record"]["name"], "mail._domainkey.example.com");
        assert_eq!(json["public_key"]["p_tag"], TEST_RSA_P_TAG);
        assert_eq!(json["key_bits"], 1024);
    }

    #[test]
    fn text_report_is_stable_for_a_supplied_key() {
        let text = run(
            "example.com",
            "mail",
            "rsa-2048",
            "text",
            true,
            "none",
            TEST_RSA_PKCS8_PEM,
        )
        .unwrap();
        assert!(text.starts_with("DKIM key for example.com\n  Selector:   mail\n"));
        assert!(text.contains("  Host / Name: mail._domainkey.example.com\n"));
        assert!(text.contains(&format!(
            "  Value:       v=DKIM1; h=sha256; k=rsa; p={TEST_RSA_P_TAG}\n"
        )));
        assert!(text.contains("-----BEGIN RSA PRIVATE KEY-----"));
        assert!(text.contains("-----BEGIN PUBLIC KEY-----"));
        assert!(text.ends_with("was ignored."));
    }
}
