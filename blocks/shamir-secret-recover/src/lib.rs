//! gizza-ai/shamir-secret-recover — chat skill block on the shared tool abstraction.
//! Reconstructs a byte-wise GF(256) Shamir secret from threshold shares.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    shares: String,
    #[serde(default)]
    share_format: String,
    #[serde(default)]
    share_encoding: String,
    #[serde(default)]
    field_poly: String,
    #[serde(default)]
    threshold: i64,
    #[serde(default = "default_true")]
    verify: bool,
    #[serde(default)]
    secret_encoding: String,
    #[serde(default)]
    output: String,
}

fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("shares")
                .required()
                .describe("Shamir shares to combine, one per line. Blank lines and # comments are ignored. Accepts index-prefixed shares such as 1-68b5..., leading-index sss: base64/base64url shares, and trailing-index hex/base64 shares. Maximum 255 shares."),
        )
        .param(
            Param::enumv("share_format", ["auto", "index-prefix", "leading-index", "trailing-index"])
                .default("auto")
                .describe("Share layout: auto (detect common forms), index-prefix (decimal x plus separator plus payload, e.g. 1-deadbeef), leading-index (decoded bytes start with x), or trailing-index (decoded bytes end with x). Default auto."),
        )
        .param(
            Param::enumv("share_encoding", ["auto", "hex", "base64"])
                .default("auto")
                .describe("How each share payload is encoded: auto (prefer hex when ambiguous), hex, or base64/base64url with optional padding. Default auto."),
        )
        .param(
            Param::enumv("field_poly", ["auto", "0x11b", "0x11d"])
                .default("auto")
                .describe("GF(256) reduction polynomial: auto, 0x11b (AES/Vault-style), or 0x11d (secrets.js-lineage). Auto uses redundant shares to confirm when possible. Default auto."),
        )
        .param(
            Param::integer("threshold")
                .default(0)
                .min(0.0)
                .max(255.0)
                .describe("Threshold K. Use 0 (default) to combine every share supplied, or set 2-255 when you have extra shares and want redundancy checks."),
        )
        .param(
            Param::boolean("verify")
                .default(true)
                .describe("When more shares than the threshold are supplied, cross-check alternate subsets and report or reject corrupted/foreign shares. Default true."),
        )
        .param(
            Param::enumv("secret_encoding", ["auto", "text", "hex", "base64", "binary"])
                .default("auto")
                .describe("Recovered secret rendering: auto (print text if safely printable UTF-8, otherwise hex), text, hex, base64, or binary bits. Default auto."),
        )
        .param(
            Param::enumv("output", ["secret", "report", "json"])
                .default("secret")
                .describe("Output shape: secret (just the recovered value), report (secret plus parsing and verification details), or json (structured result). Default secret."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/shamir-secret-recover",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Recover a Shamir secret from threshold shares.",
    skill(
        description = "Combine Shamir secret-sharing shares locally in the browser. Supports index-prefixed hex shares, leading-index sss: base64/base64url shares, trailing-index raw-byte shares, GF(256) polynomials 0x11b and 0x11d, optional threshold K, redundant-share verification, and text/hex/base64/binary output.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "shamir-secret-recover", |a: Args| {
            gizza_ai_shamir_secret_recover_core::run(
                &a.shares,
                &a.share_format,
                &a.share_encoding,
                &a.field_poly,
                a.threshold,
                a.verify,
                &a.secret_encoding,
                &a.output,
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
                    "shares":          { "type": "string", "description": "Shamir shares to combine, one per line. Blank lines and # comments are ignored. Accepts index-prefixed shares such as 1-68b5..., leading-index sss: base64/base64url shares, and trailing-index hex/base64 shares. Maximum 255 shares." },
                    "share_format":    { "type": "string", "enum": ["auto", "index-prefix", "leading-index", "trailing-index"], "default": "auto", "description": "Share layout: auto (detect common forms), index-prefix (decimal x plus separator plus payload, e.g. 1-deadbeef), leading-index (decoded bytes start with x), or trailing-index (decoded bytes end with x). Default auto." },
                    "share_encoding":  { "type": "string", "enum": ["auto", "hex", "base64"], "default": "auto", "description": "How each share payload is encoded: auto (prefer hex when ambiguous), hex, or base64/base64url with optional padding. Default auto." },
                    "field_poly":      { "type": "string", "enum": ["auto", "0x11b", "0x11d"], "default": "auto", "description": "GF(256) reduction polynomial: auto, 0x11b (AES/Vault-style), or 0x11d (secrets.js-lineage). Auto uses redundant shares to confirm when possible. Default auto." },
                    "threshold":       { "type": "integer", "default": 0, "minimum": 0, "maximum": 255, "description": "Threshold K. Use 0 (default) to combine every share supplied, or set 2-255 when you have extra shares and want redundancy checks." },
                    "verify":          { "type": "boolean", "default": true, "description": "When more shares than the threshold are supplied, cross-check alternate subsets and report or reject corrupted/foreign shares. Default true." },
                    "secret_encoding": { "type": "string", "enum": ["auto", "text", "hex", "base64", "binary"], "default": "auto", "description": "Recovered secret rendering: auto (print text if safely printable UTF-8, otherwise hex), text, hex, base64, or binary bits. Default auto." },
                    "output":          { "type": "string", "enum": ["secret", "report", "json"], "default": "secret", "description": "Output shape: secret (just the recovered value), report (secret plus parsing and verification details), or json (structured result). Default secret." }
                },
                "required": ["shares"],
                "additionalProperties": false
            }"#,
        ).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
