//! gizza-ai/pem-inspect — chat skill block on the shared tool abstraction.
//! Parses and pretty-prints every PEM block in the input — X.509 certificates,
//! CSRs, and public/private keys — entirely offline (pure Rust/wasm). Nothing is
//! uploaded; private keys are described by TYPE/ALGORITHM only, never by their
//! secret scalars. The chat schema is single-sourced from descriptor() (which
//! also drives the CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    now: i64,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .multiline()
                .describe(
                    "One or more PEM blocks to decode, pasted verbatim including the \
'-----BEGIN …-----' / '-----END …-----' lines. Supported block types: CERTIFICATE (X.509), \
CERTIFICATE REQUEST (PKCS#10 CSR), PUBLIC KEY (SPKI), RSA PUBLIC KEY (PKCS#1), PRIVATE KEY \
(PKCS#8), RSA PRIVATE KEY (PKCS#1), EC PRIVATE KEY (SEC1) and ENCRYPTED PRIVATE KEY. Multiple \
blocks (e.g. a full certificate chain) may be pasted at once — each is decoded independently. \
Everything runs locally; private-key material is reported by type/algorithm/size only and secret \
scalars are never emitted.",
                ),
        )
        .param(
            Param::integer("now")
                .min(0.0)
                .describe(
                    "Reference time as seconds since the Unix epoch, used to compute each \
certificate's expiry status and days-until-expiry. When omitted (0), the current clock time is \
used.",
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
    name = "gizza-ai/pem-inspect",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Decode PEM certificates, CSRs, and keys offline",
    skill(
        description = "Decode and inspect PEM-encoded X.509 certificates, PKCS#10 certificate \
requests (CSRs), and public/private keys entirely offline. Paste one or more '-----BEGIN …-----' \
blocks as 'input' (a full certificate chain works — each block is decoded independently). For a \
certificate it reports version, serial, subject, issuer, self-signed flag, validity window, \
expiry status and days-until-expiry, Subject Alternative Names, CA/basic-constraints, key usage \
and extended key usage, the public-key algorithm and size/curve, the signature algorithm, and \
SHA-256/SHA-1 fingerprints. CSRs report subject, requested SANs, public key and signature \
algorithm. Keys report format/algorithm/size only — private-key secret material is never emitted. \
Optionally set 'now' (Unix seconds) to evaluate certificate expiry at a specific instant; \
otherwise the current time is used. Returns one JSON object per PEM block.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "pem-inspect", |a: Args| {
            let now = if a.now == 0 {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_secs() as i64)
                    .unwrap_or(0)
            } else {
                a.now
            };
            let blocks = gizza_ai_pem_inspect_core::inspect(&a.input, now)
                .map_err(SkillError::InvalidArgs)?;
            serde_json::to_value(blocks).map_err(|e| SkillError::Serialize(e.to_string()))
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
                    "input": {
                        "type": "string",
                        "description": "One or more PEM blocks to decode, pasted verbatim including the '-----BEGIN …-----' / '-----END …-----' lines. Supported block types: CERTIFICATE (X.509), CERTIFICATE REQUEST (PKCS#10 CSR), PUBLIC KEY (SPKI), RSA PUBLIC KEY (PKCS#1), PRIVATE KEY (PKCS#8), RSA PRIVATE KEY (PKCS#1), EC PRIVATE KEY (SEC1) and ENCRYPTED PRIVATE KEY. Multiple blocks (e.g. a full certificate chain) may be pasted at once — each is decoded independently. Everything runs locally; private-key material is reported by type/algorithm/size only and secret scalars are never emitted."
                    },
                    "now": {
                        "type": "integer",
                        "minimum": 0,
                        "description": "Reference time as seconds since the Unix epoch, used to compute each certificate's expiry status and days-until-expiry. When omitted (0), the current clock time is used."
                    }
                },
                "required": ["input"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
