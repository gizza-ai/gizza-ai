//! gizza-ai/fernet-token — create or read Fernet tokens (AES-128-CBC + HMAC-SHA256).
//! The chat schema is single-sourced from descriptor() (which also drives the CLI);
//! handle() delegates to run_skill. Pure → runs on all backends. Randomness (key gen
//! + IV) comes from getrandom; the wall clock comes from SystemTime (the wafer runtime
//! provides the clock import). The core takes timestamp/IV explicitly so it stays
//! deterministic and testable.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_fernet_token_core::{decrypt, encrypt, key_from_bytes};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default)]
    key: String,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default)]
    ttl: u64,
}
fn default_mode() -> String {
    "encrypt".to_string()
}

#[derive(Serialize, Debug)]
struct Resp {
    result: String,
    mode: String,
    /// The Fernet key used (echoed so a freshly generated key can be saved).
    key: String,
    /// Token creation time (ISO-8601 UTC), present on decrypt.
    #[serde(skip_serializing_if = "Option::is_none")]
    created_at: Option<String>,
}

fn now_secs() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn random_bytes<const N: usize>() -> Result<[u8; N], String> {
    let mut b = [0u8; N];
    getrandom::getrandom(&mut b).map_err(|e| format!("randomness unavailable: {e}"))?;
    Ok(b)
}

/// Format a Unix-seconds timestamp as ISO-8601 UTC (no external date dep).
fn iso8601_utc(secs: u64) -> String {
    let days = secs / 86_400;
    let tod = secs % 86_400;
    let (h, mi, s) = (tod / 3600, (tod % 3600) / 60, tod % 60);
    // Civil-from-days (Howard Hinnant's algorithm).
    let z = days as i64 + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

/// Pure-compute entry shared by the chat block. `now` + RNG are injected here so
/// the underlying core stays deterministic.
fn run(a: Args) -> Result<Resp, String> {
    let mode = a.mode.trim().to_ascii_lowercase();
    match mode.as_str() {
        "encrypt" | "" => {
            let key = if a.key.trim().is_empty() {
                key_from_bytes(&random_bytes::<32>()?)
            } else {
                a.key.trim().to_string()
            };
            let iv = random_bytes::<16>()?;
            let token = encrypt(&key, a.text.as_bytes(), now_secs(), &iv)?;
            Ok(Resp { result: token, mode: "encrypt".into(), key, created_at: None })
        }
        "decrypt" => {
            if a.key.trim().is_empty() {
                return Err("decrypt requires the Fernet key the token was created with".into());
            }
            let ttl = if a.ttl == 0 { None } else { Some(a.ttl) };
            let d = decrypt(a.key.trim(), a.text.trim(), now_secs(), ttl)?;
            let plaintext = String::from_utf8(d.plaintext)
                .map_err(|_| "decrypted bytes are not valid UTF-8 text".to_string())?;
            Ok(Resp {
                result: plaintext,
                mode: "decrypt".into(),
                key: a.key.trim().to_string(),
                created_at: Some(iso8601_utc(d.timestamp)),
            })
        }
        other => Err(format!("unknown mode '{other}' (use 'encrypt' or 'decrypt')")),
    }
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("On encrypt: the text to protect. On decrypt: the Fernet token to read."),
        )
        .param(Param::string("key").describe(
            "The Fernet key: url-safe base64 of 32 bytes. Required for decrypt. On encrypt, leave blank to generate a fresh key (returned in the response).",
        ))
        .param(
            Param::enumv("mode", ["encrypt", "decrypt"]).default("encrypt").describe(
                "encrypt (default) turns text into a Fernet token; decrypt reads a token back into text.",
            ),
        )
        .param(
            Param::integer("ttl").min(0.0).default(0).describe(
                "Decrypt only: max token age in seconds. 0 (default) disables the age check; a positive value rejects tokens older than this (or with a future timestamp).",
            ),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/fernet-token",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Create or read Fernet tokens (AES-128-CBC + HMAC-SHA256)",
    skill(
        description = "Create or read Fernet symmetric tokens (the Fernet spec: AES-128-CBC for confidentiality + HMAC-SHA256 for authentication, with an embedded creation timestamp). mode=encrypt (default) turns text into a url-safe-base64 Fernet token; leave 'key' blank to generate a fresh 32-byte key (returned so you can save it). mode=decrypt verifies the HMAC and decrypts a token back to text; pass the same 'key', and optionally 'ttl' (max age in seconds) to reject expired tokens. Tokens carry their creation time, returned as created_at on decrypt. A wrong key or tampered token fails cleanly. Runs locally — text and keys never leave the device.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "fernet-token", |a: Args| {
            run(a).map_err(SkillError::InvalidArgs)
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
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "On encrypt: the text to protect. On decrypt: the Fernet token to read." },
                    "key": { "type": "string", "description": "The Fernet key: url-safe base64 of 32 bytes. Required for decrypt. On encrypt, leave blank to generate a fresh key (returned in the response)." },
                    "mode": { "type": "string", "enum": ["encrypt", "decrypt"], "default": "encrypt", "description": "encrypt (default) turns text into a Fernet token; decrypt reads a token back into text." },
                    "ttl": { "type": "integer", "minimum": 0, "default": 0, "description": "Decrypt only: max token age in seconds. 0 (default) disables the age check; a positive value rejects tokens older than this (or with a future timestamp)." }
                },
                "required": ["text"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn encrypt_then_decrypt_roundtrip() {
        let enc = run(Args {
            text: "top secret".into(),
            key: String::new(),
            mode: "encrypt".into(),
            ttl: 0,
        })
        .unwrap();
        assert!(!enc.key.is_empty());
        let dec = run(Args {
            text: enc.result,
            key: enc.key,
            mode: "decrypt".into(),
            ttl: 0,
        })
        .unwrap();
        assert_eq!(dec.result, "top secret");
        assert!(dec.created_at.is_some());
    }

    #[test]
    fn decrypt_without_key_errors() {
        let err = run(Args {
            text: "gAAAA".into(),
            key: String::new(),
            mode: "decrypt".into(),
            ttl: 0,
        })
        .unwrap_err();
        assert!(err.contains("requires the Fernet key"), "got: {err}");
    }

    #[test]
    fn iso8601_known_value() {
        // 499162800 = 1985-10-26T08:20:00Z (the Fernet spec test vector's timestamp)
        assert_eq!(iso8601_utc(499_162_800), "1985-10-26T08:20:00Z");
        assert_eq!(iso8601_utc(0), "1970-01-01T00:00:00Z");
    }
}
