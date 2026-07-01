//! gizza-ai/xor-cipher — repeating-key XOR chat skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_xor_cipher_core::{xor_cipher, DataFormat, OutFormat};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_input")]
    input: String,
    key: String,
    #[serde(default = "default_key_format")]
    key_format: String,
    #[serde(default = "default_output")]
    output: String,
}

fn default_input() -> String {
    "text".to_string()
}
fn default_key_format() -> String {
    "text".to_string()
}
fn default_output() -> String {
    "hex".to_string()
}

#[derive(Serialize)]
struct Resp {
    output: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe(
            "The data to XOR. Read as plain UTF-8 text (input=text, default), or decoded from hex/base64 when input=hex/base64. To decrypt, pass the ciphertext here with the matching input encoding.",
        ))
        .param(
            Param::enumv("input", ["text", "hex", "base64"]).default("text").describe(
                "How to read `data`: text = UTF-8 (default); hex or base64 = decode to bytes first.",
            ),
        )
        .param(Param::string("key").required().describe(
            "The XOR key (must be non-empty). It repeats over the data (repeating-key XOR). Read as a UTF-8 passphrase when key_format=text (default), or decoded from hex/base64.",
        ))
        .param(
            Param::enumv("key_format", ["text", "hex", "base64"]).default("text").describe(
                "How to read `key`: text = UTF-8 passphrase (default); hex or base64 = decode to bytes.",
            ),
        )
        .param(
            Param::enumv("output", ["hex", "base64", "utf8"]).default("hex").describe(
                "Encoding for the XORed result: hex (default), base64, or utf8 (decode bytes as UTF-8 text — fails if the result isn't valid UTF-8). Use utf8 to recover plaintext when decrypting.",
            ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct XorCipher;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/xor-cipher",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "XOR text/hex/base64 data against a repeating key (output hex, Base64 or UTF-8)",
    skill(
        description = "Repeating-key XOR cipher: XOR `data` against a repeating `key`, byte by byte. XOR is symmetric — the same operation encrypts and decrypts (feed the ciphertext back with the same key to recover the plaintext). input = how to read data (text, hex or base64; default text). key_format = how to read the key (text, hex or base64; default text). output = how to encode the result (hex default, base64, or utf8 to get plaintext back). WARNING: repeating-key XOR is NOT secure — use it for CTFs, obfuscation, interop and learning only. For real encryption use aes-cipher or text-encrypt.",
        parameters = schema_json()
    ),
)]
impl XorCipher {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "xor-cipher", |a: Args| {
            let input = DataFormat::parse(&a.input).map_err(SkillError::InvalidArgs)?;
            let key_format = DataFormat::parse(&a.key_format).map_err(SkillError::InvalidArgs)?;
            let output = OutFormat::parse(&a.output).map_err(SkillError::InvalidArgs)?;
            let output = xor_cipher(&a.data, input, &a.key, key_format, output)
                .map_err(SkillError::InvalidArgs)?;
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
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "data": { "type": "string", "description": "The data to XOR. Read as plain UTF-8 text (input=text, default), or decoded from hex/base64 when input=hex/base64. To decrypt, pass the ciphertext here with the matching input encoding." },
                    "input": { "type": "string", "enum": ["text", "hex", "base64"], "default": "text", "description": "How to read `data`: text = UTF-8 (default); hex or base64 = decode to bytes first." },
                    "key": { "type": "string", "description": "The XOR key (must be non-empty). It repeats over the data (repeating-key XOR). Read as a UTF-8 passphrase when key_format=text (default), or decoded from hex/base64." },
                    "key_format": { "type": "string", "enum": ["text", "hex", "base64"], "default": "text", "description": "How to read `key`: text = UTF-8 passphrase (default); hex or base64 = decode to bytes." },
                    "output": { "type": "string", "enum": ["hex", "base64", "utf8"], "default": "hex", "description": "Encoding for the XORed result: hex (default), base64, or utf8 (decode bytes as UTF-8 text — fails if the result isn't valid UTF-8). Use utf8 to recover plaintext when decrypting." }
                },
                "required": ["data", "key"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
