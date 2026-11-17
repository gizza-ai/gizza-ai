//! gizza-ai/xsalsa20-cipher — XSalsa20 chat skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_xsalsa20_cipher_core::{decrypt, encrypt, Encoding, KeyFormat};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    #[serde(default = "default_operation")]
    operation: String,
    data: String,
    key: String,
    nonce: String,
    #[serde(default = "default_key_format")]
    key_format: String,
    /// Kept as a raw JSON number so `1.0` (a common LLM serialisation) and
    /// negatives get clear errors instead of a serde type error.
    #[serde(default)]
    counter: Option<serde_json::Number>,
    #[serde(default = "default_format")]
    format: String,
}
fn default_operation() -> String { "encrypt".into() }
fn default_key_format() -> String { "encoded".into() }
fn default_format() -> String { "hex".into() }

/// Accepts any non-negative whole JSON number up to u64::MAX, including
/// float-encoded integers like `3.0`.
fn parse_counter(n: Option<&serde_json::Number>) -> Result<u64, String> {
    let Some(n) = n else { return Ok(0) };
    if let Some(u) = n.as_u64() {
        return Ok(u);
    }
    if n.as_i64().is_some() {
        return Err("counter must be >= 0".into());
    }
    match n.as_f64() {
        Some(f) if f < 0.0 => Err("counter must be >= 0".into()),
        // 2^64 is exactly representable; anything below it with no fraction fits u64
        Some(f) if f.fract() == 0.0 && f < 18_446_744_073_709_551_616.0 => Ok(f as u64),
        _ => Err("counter must be a whole number".into()),
    }
}

#[derive(Serialize)]
struct Resp { output: String }

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::enumv("operation", ["encrypt", "decrypt"]).default("encrypt").describe("Encrypt plaintext or decrypt ciphertext. XSalsa20 is symmetric; the same key, nonce, and counter reverse the operation."))
        .param(Param::string("data").required().describe("Plaintext for operation=encrypt; ciphertext encoded as format for operation=decrypt."))
        .param(Param::string("key").required().describe("XSalsa20 key: exactly 32 bytes. By default key_format=encoded, so it is read as hex or base64 according to format."))
        .param(Param::string("nonce").required().describe("XSalsa20 nonce: exactly 24 bytes. Never reuse the same key+nonce pair for different messages."))
        .param(Param::enumv("key_format", ["encoded", "text"]).default("encoded").describe("How to read key and nonce: encoded = hex/base64 according to format; text = raw UTF-8 bytes."))
        .param(Param::integer("counter").default(0).min(0.0).describe("Initial 64-byte block counter, default 0."))
        .param(Param::enumv("format", ["hex", "base64"]).default("hex").describe("Ciphertext/key/nonce encoding when key_format=encoded: hex or base64."))
}

fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct Xsalsa20Cipher;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/xsalsa20-cipher",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Encrypt or decrypt data with XSalsa20 using a 32-byte key and 24-byte nonce",
    skill(description = "Encrypt or decrypt UTF-8 data with XSalsa20, the extended-nonce Salsa20 stream cipher. operation=encrypt returns ciphertext encoded as hex/base64; operation=decrypt decodes the ciphertext and returns UTF-8 plaintext. The key must be exactly 32 bytes and the nonce exactly 24 bytes. Never reuse a key+nonce pair.", parameters = schema_json())
)]
impl Xsalsa20Cipher {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "xsalsa20-cipher", |a: Args| {
            let format = Encoding::parse(&a.format).map_err(SkillError::InvalidArgs)?;
            let key_format = KeyFormat::parse(&a.key_format).map_err(SkillError::InvalidArgs)?;
            let counter = parse_counter(a.counter.as_ref()).map_err(SkillError::InvalidArgs)?;
            let output = match a.operation.trim().to_ascii_lowercase().as_str() {
                "" | "encrypt" => encrypt(&a.data, &a.key, &a.nonce, key_format, counter, format),
                "decrypt" => decrypt(&a.data, &a.key, &a.nonce, key_format, counter, format),
                other => Err(format!("unknown operation '{other}' (use encrypt or decrypt)")),
            }.map_err(SkillError::InvalidArgs)?;
            Ok(Resp { output })
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(r#"{
          "type":"object","properties":{
            "operation":{"type":"string","enum":["encrypt","decrypt"],"default":"encrypt","description":"Encrypt plaintext or decrypt ciphertext. XSalsa20 is symmetric; the same key, nonce, and counter reverse the operation."},
            "data":{"type":"string","description":"Plaintext for operation=encrypt; ciphertext encoded as format for operation=decrypt."},
            "key":{"type":"string","description":"XSalsa20 key: exactly 32 bytes. By default key_format=encoded, so it is read as hex or base64 according to format."},
            "nonce":{"type":"string","description":"XSalsa20 nonce: exactly 24 bytes. Never reuse the same key+nonce pair for different messages."},
            "key_format":{"type":"string","enum":["encoded","text"],"default":"encoded","description":"How to read key and nonce: encoded = hex/base64 according to format; text = raw UTF-8 bytes."},
            "counter":{"type":"integer","default":0,"minimum":0,"description":"Initial 64-byte block counter, default 0."},
            "format":{"type":"string","enum":["hex","base64"],"default":"hex","description":"Ciphertext/key/nonce encoding when key_format=encoded: hex or base64."}
          },"required":["data","key","nonce"],"additionalProperties":false
        }"#).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn counter_accepts_whole_numbers_and_rejects_the_rest() {
        let num = |v: &str| serde_json::from_str::<serde_json::Number>(v).unwrap();
        assert_eq!(parse_counter(None), Ok(0));
        assert_eq!(parse_counter(Some(&num("7"))), Ok(7));
        assert_eq!(parse_counter(Some(&num("1.0"))), Ok(1));
        assert_eq!(parse_counter(Some(&num("18446744073709551615"))), Ok(u64::MAX));
        assert_eq!(parse_counter(Some(&num("-1"))), Err("counter must be >= 0".into()));
        assert_eq!(parse_counter(Some(&num("-1.5"))), Err("counter must be >= 0".into()));
        assert_eq!(parse_counter(Some(&num("1.5"))), Err("counter must be a whole number".into()));
        assert_eq!(parse_counter(Some(&num("1e30"))), Err("counter must be a whole number".into()));
    }

    #[test]
    fn args_accept_float_counter() {
        let a: Args = serde_json::from_str(r#"{"data":"x","key":"k","nonce":"n","counter":1.0}"#).unwrap();
        assert_eq!(parse_counter(a.counter.as_ref()), Ok(1));
    }
}
