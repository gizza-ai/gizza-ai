//! gizza-ai/classical-cipher-tool — chat skill block on the shared tool abstraction.
//! Encrypts / decrypts with the classic Caesar, Vigenère, Atbash, and Rail-fence
//! ciphers (plus Caesar brute-force). The chat schema is single-sourced from
//! descriptor() (which also drives the CLI); handle() delegates to
//! block_utils::run_skill. No host calls — runs entirely in the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default)]
    cipher: String,
    #[serde(default)]
    operation: String,
    /// Cipher-specific key: Caesar shift (int), Vigenère keyword, or rail count.
    #[serde(default)]
    key: String,
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The message to encrypt or decrypt. Letters are transformed; digits, spaces, and punctuation pass through unchanged (rail-fence transposes the whole string)."),
        )
        .param(
            Param::enumv("cipher", ["caesar", "vigenere", "atbash", "rail-fence"])
                .default("caesar")
                .describe("Which classical cipher to use. 'caesar' shifts each letter by a fixed amount; 'vigenere' shifts by a repeating keyword; 'atbash' mirrors the alphabet (A<->Z) and is keyless; 'rail-fence' is a zig-zag transposition over N rails."),
        )
        .param(
            Param::enumv("operation", ["encrypt", "decrypt", "brute-force"])
                .default("encrypt")
                .describe("Direction: 'encrypt' (default), 'decrypt', or 'brute-force'. brute-force is only valid for caesar and returns all 26 candidate shifts as 'shift NN: <text>' lines."),
        )
        .param(
            Param::string("key")
                .describe("Cipher key. caesar: the integer shift (negative allowed, reduced mod 26; blank -> 3). vigenere: the keyword (letters only; at least one letter required). atbash: ignored. rail-fence: the number of rails, 2-64 (blank -> 3). Ignored for caesar brute-force."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ClassicalCipher;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/classical-cipher-tool",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Classical cipher (Caesar / Vigenère / Atbash / rail-fence) skill",
    skill(
        description = "Encrypt or decrypt text with classic pre-computer ciphers. cipher='caesar' (default) shifts each letter by a fixed key (e.g. shift 3: 'Hello' -> 'Khoor'); cipher='vigenere' shifts by a repeating keyword (key='LEMON'); cipher='atbash' mirrors the alphabet A<->Z and needs no key; cipher='rail-fence' is a zig-zag transposition over `key` rails (2-64, default 3). operation='encrypt' (default) or 'decrypt'. operation='brute-force' works only for caesar and returns all 26 shifts so you can spot the readable plaintext. Letters are transformed with case preserved; digits/spaces/punctuation pass through (rail-fence transposes the whole string). Runs locally — these are educational/puzzle ciphers, NOT secure encryption.",
        parameters = schema_json()
    ),
)]
impl ClassicalCipher {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "classical-cipher-tool", |a: Args| {
            gizza_ai_classical_cipher_tool_core::run(&a.cipher, &a.operation, &a.key, &a.text)
                .map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional and reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "The message to encrypt or decrypt. Letters are transformed; digits, spaces, and punctuation pass through unchanged (rail-fence transposes the whole string)." },
                    "cipher": { "type": "string", "enum": ["caesar", "vigenere", "atbash", "rail-fence"], "default": "caesar", "description": "Which classical cipher to use. 'caesar' shifts each letter by a fixed amount; 'vigenere' shifts by a repeating keyword; 'atbash' mirrors the alphabet (A<->Z) and is keyless; 'rail-fence' is a zig-zag transposition over N rails." },
                    "operation": { "type": "string", "enum": ["encrypt", "decrypt", "brute-force"], "default": "encrypt", "description": "Direction: 'encrypt' (default), 'decrypt', or 'brute-force'. brute-force is only valid for caesar and returns all 26 candidate shifts as 'shift NN: <text>' lines." },
                    "key": { "type": "string", "description": "Cipher key. caesar: the integer shift (negative allowed, reduced mod 26; blank -> 3). vigenere: the keyword (letters only; at least one letter required). atbash: ignored. rail-fence: the number of rails, 2-64 (blank -> 3). Ignored for caesar brute-force." }
                },
                "required": ["text"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
