//! gizza-ai/pem-bundle-splitter — split a concatenated PEM bundle into labeled blocks.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn output_default() -> String {
    "report".to_string()
}
fn fingerprints_default() -> bool {
    true
}

#[derive(Deserialize)]
struct Args {
    pem: String,
    #[serde(default = "output_default")]
    output: String,
    #[serde(default = "fingerprints_default")]
    fingerprints: bool,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("pem").required().multiline().describe("Concatenated PEM bundle text containing one or more -----BEGIN ...----- / -----END ...----- blocks."))
        .param(Param::enumv("output", ["report", "json", "pem"]).default("report").describe("Output format: readable report, structured JSON, or cleaned individual PEM blocks."))
        .param(Param::boolean("fingerprints").default(true).describe("Include a SHA-256 fingerprint of each block's decoded DER bytes."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/pem-bundle-splitter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Split PEM bundles into labeled blocks",
    skill(
        description = "Split a multi-block PEM bundle into individual labeled blocks in input order. Reports the PEM label, friendly type, category counts, DER byte length, suggested filename, and optional SHA-256 fingerprint. Supports certificates, private/public keys, CSRs, params, PGP/OpenSSH blocks, and unknown PEM labels.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "pem-bundle-splitter", |a: Args| {
            let mode = gizza_ai_pem_bundle_splitter_core::parse_output(&a.output)
                .map_err(SkillError::InvalidArgs)?;
            gizza_ai_pem_bundle_splitter_core::run(&a.pem, mode, a.fingerprints)
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
                    "pem": { "type": "string", "description": "Concatenated PEM bundle text containing one or more -----BEGIN ...----- / -----END ...----- blocks." },
                    "output": { "type": "string", "enum": ["report", "json", "pem"], "default": "report", "description": "Output format: readable report, structured JSON, or cleaned individual PEM blocks." },
                    "fingerprints": { "type": "boolean", "default": true, "description": "Include a SHA-256 fingerprint of each block's decoded DER bytes." }
                },
                "required": ["pem"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
