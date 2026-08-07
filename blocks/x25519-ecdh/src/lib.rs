//! gizza-ai/x25519-ecdh — chat skill block on the shared tool abstraction.
//! Runs an X25519 (Curve25519) Diffie-Hellman agreement between a private key
//! and a peer's public key, then optionally expands the raw shared secret into
//! a usable symmetric key with HKDF or SHA-256. The chat schema is
//! single-sourced from descriptor() (which also drives the CLI); handle()
//! delegates to block_utils::run_skill. Pure compute, no host calls — runs
//! entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn default_kdf() -> String {
    "hkdf-sha256".to_string()
}
fn default_encoding() -> String {
    "hex".to_string()
}
fn default_kdf_length() -> i64 {
    32
}

#[derive(Deserialize)]
struct Args {
    /// Your X25519 private key in any accepted encoding. Empty → generated.
    #[serde(default)]
    private_key: String,
    /// The peer's X25519 public key. Empty → a demo peer pair is generated.
    #[serde(default)]
    peer_public_key: String,
    /// "none" | "hkdf-sha256" | "hkdf-sha512" | "sha256".
    #[serde(default = "default_kdf")]
    kdf: String,
    #[serde(default)]
    kdf_salt: String,
    #[serde(default)]
    kdf_info: String,
    #[serde(default = "default_kdf_length")]
    kdf_length: i64,
    /// "hex" | "base64" | "base64url".
    #[serde(default = "default_encoding")]
    encoding: String,
    #[serde(default)]
    include_pem: bool,
}

/// Run the agreement from the parsed args. The core wrapper owns enum parsing
/// and every range check, so this is the one place the surfaces converge.
fn run_args(a: &Args) -> Result<String, String> {
    gizza_ai_x25519_ecdh_core::run(
        &a.private_key,
        &a.peer_public_key,
        &a.kdf,
        &a.kdf_salt,
        &a.kdf_info,
        a.kdf_length.max(0) as usize,
        &a.encoding,
        a.include_pem,
    )
}

/// Single source for the chat schema (and CLI + page controls). Every param
/// carries a `.describe()` an LLM/CLI user can act on. `kdf`/`encoding` are
/// `enumv` (→ `<select>`), `include_pem` is a boolean (→ checkbox), and
/// `kdf_length` is a bounded integer. Both key fields are optional: leaving
/// them empty is a documented mode, not a missing argument.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("private_key")
                .default("")
                .describe("Your X25519 private key: 64 hex characters (with or without a 0x prefix), 32 raw bytes as standard or URL-safe base64, or an RFC 8410 PKCS#8 `-----BEGIN PRIVATE KEY-----` PEM block. Leave empty to generate a fresh key pair from the system CSPRNG."),
        )
        .param(
            Param::string("peer_public_key")
                .default("")
                .describe("The other party's X25519 public key, in the same encodings as private_key (hex, base64, base64url, or an RFC 8410 SubjectPublicKeyInfo `-----BEGIN PUBLIC KEY-----` PEM block). Leave empty to generate a demo peer key pair and see both sides of one exchange."),
        )
        .param(
            Param::enumv("kdf", ["none", "hkdf-sha256", "hkdf-sha512", "sha256"])
                .default("hkdf-sha256")
                .describe("How to turn the raw 32-byte agreement output into a usable key: hkdf-sha256/hkdf-sha512 = RFC 5869 extract-and-expand with kdf_salt and kdf_info (recommended), sha256 = a single SHA-256 of the raw secret, none = report the raw RFC 7748 output unchanged. Default hkdf-sha256."),
        )
        .param(
            Param::string("kdf_salt")
                .describe("HKDF salt, read as UTF-8 bytes. Optional and public — a random per-session value strengthens the extract step. Empty means the RFC 5869 all-zero salt. Ignored by kdf = none and kdf = sha256."),
        )
        .param(
            Param::string("kdf_info")
                .describe("HKDF info / context label, read as UTF-8 bytes. Bind the key to its purpose (e.g. 'app v1 chat key') so the same agreement yields different keys for different uses; both sides must use the identical value. Ignored by kdf = none and kdf = sha256."),
        )
        .param(
            Param::integer("kdf_length")
                .min(1.0)
                .max(8160.0)
                .default(32)
                .describe("Length of the derived key in bytes, 1-8160 (the HKDF 255 x hash-length ceiling). 32 suits AES-256 or ChaCha20; 44 covers a key plus a 12-byte nonce. HKDF only. Default 32."),
        )
        .param(
            Param::enumv("encoding", ["hex", "base64", "base64url"])
                .default("hex")
                .describe("Encoding for every key and secret in the output: hex = lowercase hex, base64 = standard padded base64, base64url = URL-safe base64 without padding. Default hex."),
        )
        .param(
            Param::boolean("include_pem")
                .default(false)
                .describe("Also print RFC 8410 PEM blocks: a PKCS#8 private key and SubjectPublicKeyInfo public keys, the forms OpenSSL and most libraries import. Default off."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/x25519-ecdh",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "X25519 ECDH shared secret and HKDF key derivation",
    skill(
        description = "Perform an X25519 (Curve25519) Elliptic-Curve Diffie-Hellman key agreement: give a private key and the peer's public key and get the 32-byte shared secret, plus the public key derived from that private key so you can check you handed the peer the right half. Keys are accepted as hex (0x optional), standard or URL-safe base64, or RFC 8410 PKCS#8 / SubjectPublicKeyInfo PEM, and can be emitted as PEM with include_pem. Because the raw agreement output is not a uniformly random key, kdf expands it with HKDF-SHA256/HKDF-SHA512 (RFC 5869, with kdf_salt, kdf_info and kdf_length) or hashes it with SHA-256; kdf = none reports the raw RFC 7748 value and says so. Leave private_key empty to generate a fresh key pair, or peer_public_key empty to generate a demo peer and see both sides of one exchange. A low-order peer public key, which forces an all-zero predictable secret, is rejected with an explanation. To encrypt with the derived key, pass it to a symmetric cipher tool. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "x25519-ecdh", |a: Args| {
            run_args(&a).map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // RFC 7748 §6.1 Diffie-Hellman test vector.
    const ALICE_PRIV: &str = "77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a";
    const ALICE_PUB: &str = "8520f0098930a754748b7ddcb43ef75a0dbf3a0d26381af4eba4a98eaa9b4e6a";
    const BOB_PUB: &str = "de9edb7d7b7dc1b4d35b61c2ece435373f8343c85b78674dadfc7e146f882b4f";
    const SHARED: &str = "4a5d9d5ba4ce2de1728e3bf480350f25e07e21c947d19e3376f09b3c1e161742";

    /// Migration/consistency guard: the descriptor-derived chat schema must
    /// match this authored blob, so the LLM (and the page form, which reads the
    /// synced manifest) sees a stable shape. Regenerate this literal whenever
    /// the descriptor changes on purpose.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "private_key": { "type": "string", "default": "", "description": "Your X25519 private key: 64 hex characters (with or without a 0x prefix), 32 raw bytes as standard or URL-safe base64, or an RFC 8410 PKCS#8 `-----BEGIN PRIVATE KEY-----` PEM block. Leave empty to generate a fresh key pair from the system CSPRNG." },
                    "peer_public_key": { "type": "string", "default": "", "description": "The other party's X25519 public key, in the same encodings as private_key (hex, base64, base64url, or an RFC 8410 SubjectPublicKeyInfo `-----BEGIN PUBLIC KEY-----` PEM block). Leave empty to generate a demo peer key pair and see both sides of one exchange." },
                    "kdf": { "type": "string", "enum": ["none", "hkdf-sha256", "hkdf-sha512", "sha256"], "default": "hkdf-sha256", "description": "How to turn the raw 32-byte agreement output into a usable key: hkdf-sha256/hkdf-sha512 = RFC 5869 extract-and-expand with kdf_salt and kdf_info (recommended), sha256 = a single SHA-256 of the raw secret, none = report the raw RFC 7748 output unchanged. Default hkdf-sha256." },
                    "kdf_salt": { "type": "string", "description": "HKDF salt, read as UTF-8 bytes. Optional and public — a random per-session value strengthens the extract step. Empty means the RFC 5869 all-zero salt. Ignored by kdf = none and kdf = sha256." },
                    "kdf_info": { "type": "string", "description": "HKDF info / context label, read as UTF-8 bytes. Bind the key to its purpose (e.g. 'app v1 chat key') so the same agreement yields different keys for different uses; both sides must use the identical value. Ignored by kdf = none and kdf = sha256." },
                    "kdf_length": { "type": "integer", "minimum": 1, "maximum": 8160, "default": 32, "description": "Length of the derived key in bytes, 1-8160 (the HKDF 255 x hash-length ceiling). 32 suits AES-256 or ChaCha20; 44 covers a key plus a 12-byte nonce. HKDF only. Default 32." },
                    "encoding": { "type": "string", "enum": ["hex", "base64", "base64url"], "default": "hex", "description": "Encoding for every key and secret in the output: hex = lowercase hex, base64 = standard padded base64, base64url = URL-safe base64 without padding. Default hex." },
                    "include_pem": { "type": "boolean", "default": false, "description": "Also print RFC 8410 PEM blocks: a PKCS#8 private key and SubjectPublicKeyInfo public keys, the forms OpenSSL and most libraries import. Default off." }
                },
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    /// Both key fields are optional by design, so an empty object must parse.
    #[test]
    fn args_default_every_param() {
        let a: Args = serde_json::from_str("{}").unwrap();
        assert_eq!(a.private_key, "");
        assert_eq!(a.peer_public_key, "");
        assert_eq!(a.kdf, "hkdf-sha256");
        assert_eq!(a.kdf_salt, "");
        assert_eq!(a.kdf_info, "");
        assert_eq!(a.kdf_length, 32);
        assert_eq!(a.encoding, "hex");
        assert!(!a.include_pem);
        // No arguments at all still produces a complete demo exchange.
        let out = run_args(&a).unwrap();
        assert!(out.contains("Peer private key"), "{out}");
        assert!(out.contains("Derived key"), "{out}");
    }

    #[test]
    fn rejects_unknown_enum_values() {
        let a: Args = serde_json::from_str(r#"{"kdf":"hkdf-md5"}"#).unwrap();
        assert!(run_args(&a).unwrap_err().contains("unknown kdf"));
        let a: Args = serde_json::from_str(r#"{"encoding":"base32"}"#).unwrap();
        assert!(run_args(&a).unwrap_err().contains("unknown encoding"));
    }

    #[test]
    fn rejects_out_of_range_kdf_length() {
        let a: Args = serde_json::from_str(r#"{"kdf_length":8161}"#).unwrap();
        assert_eq!(
            run_args(&a).unwrap_err(),
            "kdf_length must be between 1 and 8160 bytes; got 8161"
        );
        // A negative length clamps to 0 and hits the same guard, not a panic.
        let a: Args = serde_json::from_str(r#"{"kdf_length":-1}"#).unwrap();
        assert!(run_args(&a).unwrap_err().starts_with("kdf_length must be"));
    }

    #[test]
    fn end_to_end_happy_path_through_the_args_shape() {
        let a: Args = serde_json::from_str(&format!(
            r#"{{"private_key":"{ALICE_PRIV}","peer_public_key":"{BOB_PUB}","kdf":"none"}}"#
        ))
        .unwrap();
        let out = run_args(&a).unwrap();
        assert!(out.contains(&format!("Your public key    {ALICE_PUB}")), "{out}");
        assert!(out.contains(&format!("Shared secret      {SHARED}")), "{out}");
        assert!(
            out.contains("The raw shared secret is not a uniformly random key."),
            "kdf = none warns in the output itself: {out}"
        );
    }

    #[test]
    fn hkdf_and_pem_options_reach_the_core() {
        let a: Args = serde_json::from_str(&format!(
            r#"{{"private_key":"{ALICE_PRIV}","peer_public_key":"{BOB_PUB}",
                 "kdf":"hkdf-sha256","kdf_salt":"salt","kdf_info":"app v1",
                 "kdf_length":44,"encoding":"base64url","include_pem":true}}"#
        ))
        .unwrap();
        let out = run_args(&a).unwrap();
        assert!(out.starts_with("X25519 ECDH · shared secret derived (base64url)"));
        assert!(out.contains("hkdf-sha256 · 44 bytes"), "{out}");
        assert!(out.contains("-----BEGIN PRIVATE KEY-----"), "{out}");
        assert_eq!(out.matches("-----BEGIN PUBLIC KEY-----").count(), 2);
    }
}
