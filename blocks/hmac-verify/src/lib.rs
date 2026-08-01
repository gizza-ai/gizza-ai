//! gizza-ai/hmac-verify — constant-time HMAC tag verification.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    message: String,
    key: String,
    expected: String,
    #[serde(default = "default_algorithm")]
    algorithm: String,
    #[serde(default = "default_text")]
    message_encoding: String,
    #[serde(default = "default_text")]
    key_encoding: String,
    #[serde(default = "default_auto")]
    expected_encoding: String,
}

fn default_algorithm() -> String {
    "sha256".into()
}
fn default_text() -> String {
    "text".into()
}
fn default_auto() -> String {
    "auto".into()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("message").required().describe("Message or payload bytes to authenticate. Interpreted according to message_encoding (text by default)."))
        .param(Param::string("key").required().describe("Secret HMAC key. Interpreted according to key_encoding (text by default); use hex/base64 for binary keys."))
        .param(Param::string("expected").required().describe("Expected HMAC tag/signature to verify. Hex or base64; expected_encoding=auto accepts either and also tolerates prefixes like sha256= or 0x."))
        .param(Param::enumv("algorithm", ["md5", "sha1", "sha224", "sha256", "sha384", "sha512", "sha3-256", "sha3-512"]).default("sha256").describe("Underlying digest algorithm for HMAC. Default sha256; choose the same algorithm the sender used."))
        .param(Param::enumv("message_encoding", ["text", "hex", "base64"]).default("text").describe("How to decode the message before HMAC: text (UTF-8 bytes), hex, or base64."))
        .param(Param::enumv("key_encoding", ["text", "hex", "base64"]).default("text").describe("How to decode the secret key before HMAC: text (UTF-8 bytes), hex, or base64."))
        .param(Param::enumv("expected_encoding", ["auto", "hex", "base64"]).default("auto").describe("How to decode the expected tag. auto tries hex first then base64, and tolerates common webhook prefixes."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/hmac-verify",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Verify an HMAC tag in constant time",
    skill(
        description = "Verify that an HMAC tag/signature matches a message and secret key using a constant-time comparison. Inputs: message, key, expected tag; algorithm=md5|sha1|sha224|sha256|sha384|sha512|sha3-256|sha3-512 (default sha256); message_encoding and key_encoding=text|hex|base64 (default text); expected_encoding=auto|hex|base64 (default auto). Returns MATCH/MISMATCH plus the normalized expected and computed tags.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": ... }. For a media
        // tool, use resolve_source + dispatch_ffmpeg + build_media_envelope
        // instead (see blocks/image-resize/src/lib.rs).
        match run_skill(&body, "hmac-verify", |a: Args| {
            gizza_ai_hmac_verify_core::verify_report(
                &a.message,
                &a.key,
                &a.expected,
                &a.algorithm,
                &a.message_encoding,
                &a.key_encoding,
                &a.expected_encoding,
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
    fn schema_json_contains_expected_params() {
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = schema.get("properties").unwrap();
        assert!(props.get("message").is_some());
        assert!(props.get("key").is_some());
        assert!(props.get("expected").is_some());
        assert_eq!(props["algorithm"]["default"], "sha256");
        assert_eq!(props["expected_encoding"]["default"], "auto");
    }
}
