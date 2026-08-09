//! gizza-ai/pkcs12-inspect — chat skill block on the shared tool abstraction.
//! Lists the bag structure of a PKCS#12 (.p12/.pfx) container — cert bags, key
//! bags, friendly names, local key IDs, and the PBE/MAC parameters — without the
//! password. Nothing is decrypted and no key material is ever emitted. The chat
//! schema is single-sourced from descriptor() (which also drives the CLI);
//! handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default = "default_encoding")]
    encoding: String,
    #[serde(default = "default_format")]
    format: String,
}

fn default_encoding() -> String {
    "auto".to_string()
}
fn default_format() -> String {
    "text".to_string()
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .multiline()
                .describe(
                    "The PKCS#12 container's raw bytes, encoded as base64 or hex — e.g. the \
output of `base64 -w0 keystore.p12`. Hex may use spaces or colons as separators. Do NOT paste a \
password: the structure is read without decrypting anything, and up to 4 MiB of decoded data is \
accepted (real .p12/.pfx files are a few KB).",
                ),
        )
        .param(
            Param::enumv("encoding", ["auto", "base64", "hex"])
                .default("auto")
                .describe(
                    "How 'data' is encoded. auto (default) treats input made only of hex digits \
and separators as hex and everything else as base64; force it with base64 or hex when a container \
happens to be base64 text that also looks like hex.",
                ),
        )
        .param(
            Param::enumv("format", ["text", "json"])
                .default("text")
                .describe(
                    "text (default) renders a readable report of the MAC parameters, each \
SafeContents entry and every bag; json returns the same data as a structured object with \
version, mac, safe_contents[].bags[] and a summary of bag counts.",
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
    name = "gizza-ai/pkcs12-inspect",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "List the bags inside a PKCS#12 (.p12/.pfx) file without its password",
    skill(
        description = "Inspect the structure of a PKCS#12 (.p12/.pfx) keystore WITHOUT its \
password. Pass the container's bytes as base64 or hex in 'data'. Reports the PFX version, the \
integrity-MAC parameters (digest algorithm, iteration count, MAC and salt lengths — the MAC \
itself cannot be verified without the password), and every AuthenticatedSafe SafeContents entry: \
for password-protected ones the PBE/PBES2 parameters (KDF, PRF, cipher, iterations, salt length, \
payload size), and for plaintext ones every SafeBag with its type (certBag, keyBag, \
pkcs8ShroudedKeyBag, crlBag, secretBag, safeContentsBag), friendlyName, localKeyID (the value \
that pairs a key with its certificate) and other bag attributes. Certificates in plaintext cert \
bags are decoded fully: subject, issuer, serial, validity window, self-signed and CA flags, \
public-key algorithm and size, signature algorithm and SHA-256 fingerprint. Nothing is decrypted \
and private-key material is never emitted — a shrouded key bag is reported by algorithm only. Use \
format=json for structured output. Everything runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "pkcs12-inspect", |a: Args| {
            gizza_ai_pkcs12_inspect_core::run(&a.data, &a.encoding, &a.format)
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
    /// schema exactly, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "data": {
                        "type": "string",
                        "description": "The PKCS#12 container's raw bytes, encoded as base64 or hex — e.g. the output of `base64 -w0 keystore.p12`. Hex may use spaces or colons as separators. Do NOT paste a password: the structure is read without decrypting anything, and up to 4 MiB of decoded data is accepted (real .p12/.pfx files are a few KB)."
                    },
                    "encoding": {
                        "type": "string",
                        "enum": ["auto", "base64", "hex"],
                        "default": "auto",
                        "description": "How 'data' is encoded. auto (default) treats input made only of hex digits and separators as hex and everything else as base64; force it with base64 or hex when a container happens to be base64 text that also looks like hex."
                    },
                    "format": {
                        "type": "string",
                        "enum": ["text", "json"],
                        "default": "text",
                        "description": "text (default) renders a readable report of the MAC parameters, each SafeContents entry and every bag; json returns the same data as a structured object with version, mac, safe_contents[].bags[] and a summary of bag counts."
                    }
                },
                "required": ["data"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
