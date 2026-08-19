//! gizza-ai/one-time-pad — encrypt/decrypt with a true one-time pad (mod-26
//! letters, mod-10 digits or bytewise XOR) and generate CSPRNG pad material
//! sized to the message. Chat schema single-sourced from descriptor(); handler
//! delegates to run_skill. Pure (getrandom CSPRNG) → all backends incl. the
//! chat SW.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_one_time_pad_core::run;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default = "default_cipher")]
    cipher: String,
    #[serde(default)]
    message: String,
    #[serde(default)]
    pad: String,
    #[serde(default = "default_encoding")]
    encoding: String,
    #[serde(default)]
    length: u32,
    #[serde(default)]
    group: u32,
}
fn default_mode() -> String { "encrypt".into() }
fn default_cipher() -> String { "letters".into() }
fn default_encoding() -> String { "hex".into() }

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::enumv("mode", ["encrypt", "decrypt", "generate-pad"]).default("encrypt").describe("What to do: encrypt a message with a pad, decrypt a ciphertext with the pad it was encrypted with, or generate-pad to produce pad material on its own. Default encrypt."))
        .param(Param::enumv("cipher", ["letters", "digits", "xor"]).default("letters").describe("Pad alphabet: 'letters' (default) adds the pad to A-Z with C = (P + K) mod 26, keeping case, spaces and punctuation and consuming pad only for letters; 'digits' does the same mod 10 over 0-9; 'xor' XORs the message's raw UTF-8 bytes. Default letters."))
        .param(Param::string("message").default("").describe("The plaintext to encrypt, or the ciphertext to decrypt (for cipher=xor a decrypt message is the encoded ciphertext). In generate-pad it is optional and only used to size the pad when length is 0."))
        .param(Param::string("pad").default("").describe("The pad — A-Z for letters, 0-9 for digits, hex or base64 bytes for xor; spaces are ignored. Leave empty when encrypting to generate a fresh pad sized to the message (returned alongside the ciphertext). Required to decrypt. A pad shorter than the message is an error: it is never repeated."))
        .param(Param::enumv("encoding", ["hex", "base64"]).default("hex").describe("How the xor pad and ciphertext are encoded. Ignored by the letters and digits ciphers. Default hex."))
        .param(Param::integer("length").default(0).min(0.0).max(16384.0).describe("generate-pad only: how many pad symbols (letters, digits or bytes) to generate. 0 (default) sizes the pad from message."))
        .param(Param::integer("group").default(0).min(0.0).max(20.0).describe("Split the returned ciphertext and pad into blocks of this many characters — 5 gives classic five-character traffic groups. 0 (default) keeps the original layout."))
}
fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct OneTimePad;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/one-time-pad",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Encrypt, decrypt and generate one-time pads (mod-26 letters, mod-10 digits or XOR)",
    skill(
        description = "One-time pad encryption, decryption and pad generation, all locally. `cipher=letters` (default) applies C = (P + K) mod 26 to A-Z while preserving case, spaces and punctuation (which consume no pad); `cipher=digits` is the mod-10 equivalent; `cipher=xor` XORs the message's UTF-8 bytes with the pad, both carried as hex or base64 per `encoding`. Encrypting with an empty `pad` generates a fresh cryptographically random pad sized to the message and returns it with the ciphertext; `mode=generate-pad` returns pad material alone (`length` symbols, or sized from `message` when length is 0). A pad shorter than the message is a hard error naming the shortfall — the pad is never repeated, which is what separates this from a Vigenère or repeating-key XOR. `group` splits the returned ciphertext and pad into fixed-size blocks. Returns JSON with the mode, cipher, ciphertext or plaintext, the pad, and any warnings.",
        parameters = schema_json()
    ),
)]
impl OneTimePad {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "one-time-pad", |a: Args| {
            run(
                &a.mode,
                &a.cipher,
                &a.message,
                &a.pad,
                &a.encoding,
                a.length as usize,
                a.group as usize,
            )
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

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "mode":     { "type": "string", "enum": ["encrypt", "decrypt", "generate-pad"], "default": "encrypt", "description": "What to do: encrypt a message with a pad, decrypt a ciphertext with the pad it was encrypted with, or generate-pad to produce pad material on its own. Default encrypt." },
                    "cipher":   { "type": "string", "enum": ["letters", "digits", "xor"], "default": "letters", "description": "Pad alphabet: 'letters' (default) adds the pad to A-Z with C = (P + K) mod 26, keeping case, spaces and punctuation and consuming pad only for letters; 'digits' does the same mod 10 over 0-9; 'xor' XORs the message's raw UTF-8 bytes. Default letters." },
                    "message":  { "type": "string", "default": "", "description": "The plaintext to encrypt, or the ciphertext to decrypt (for cipher=xor a decrypt message is the encoded ciphertext). In generate-pad it is optional and only used to size the pad when length is 0." },
                    "pad":      { "type": "string", "default": "", "description": "The pad — A-Z for letters, 0-9 for digits, hex or base64 bytes for xor; spaces are ignored. Leave empty when encrypting to generate a fresh pad sized to the message (returned alongside the ciphertext). Required to decrypt. A pad shorter than the message is an error: it is never repeated." },
                    "encoding": { "type": "string", "enum": ["hex", "base64"], "default": "hex", "description": "How the xor pad and ciphertext are encoded. Ignored by the letters and digits ciphers. Default hex." },
                    "length":   { "type": "integer", "default": 0, "minimum": 0, "maximum": 16384, "description": "generate-pad only: how many pad symbols (letters, digits or bytes) to generate. 0 (default) sizes the pad from message." },
                    "group":    { "type": "integer", "default": 0, "minimum": 0, "maximum": 20, "description": "Split the returned ciphertext and pad into blocks of this many characters — 5 gives classic five-character traffic groups. 0 (default) keeps the original layout." }
                },
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn descriptor_param_order_matches_the_page_form() {
        // web/src/lib.rs's run() and page/meta.toml's [[input]] blocks are
        // positional against this order — keep the three in step.
        let names: Vec<String> = descriptor().params.iter().map(|p| p.name.clone()).collect();
        assert_eq!(
            names,
            ["mode", "cipher", "message", "pad", "encoding", "length", "group"]
        );
    }
}
