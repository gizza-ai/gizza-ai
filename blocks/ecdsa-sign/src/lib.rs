//! gizza-ai/ecdsa-sign — sign a message with an ECDSA private key (NIST P-256 or
//! P-384), returning the signature in DER or raw (r||s) form. Chat schema
//! single-sourced from descriptor(); handler delegates to run_skill. Pure → all
//! backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_ecdsa_sign_core::{sign, Curve, SigFormat};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    message: String,
    private_key: String,
    #[serde(default = "default_curve")]
    curve: String,
    #[serde(default = "default_format")]
    format: String,
}

fn default_curve() -> String {
    "p256".to_string()
}
fn default_format() -> String {
    "der".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("message")
                .required()
                .describe("The message to sign."),
        )
        .param(Param::string("private_key").required().describe(
            "Your EC private key, PEM-encoded PKCS#8 ('-----BEGIN PRIVATE KEY-----'). Must match the chosen curve.",
        ))
        .param(
            Param::enumv("curve", ["p256", "p384"]).default("p256").describe(
                "Elliptic curve of the key: p256 (NIST P-256/secp256r1, SHA-256) or p384 (NIST P-384/secp384r1, SHA-384).",
            ),
        )
        .param(
            Param::enumv("format", ["der", "raw"]).default("der").describe(
                "Signature encoding: der (ASN.1 DER, as OpenSSL emits) or raw (fixed-length r||s, IEEE-P1363/JOSE).",
            ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct EcdsaSign;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/ecdsa-sign",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Sign a message with an ECDSA private key (P-256/P-384)",
    skill(
        description = "Sign a message with an ECDSA private key on NIST curve P-256 or P-384. The curve fixes the digest (P-256→SHA-256, P-384→SHA-384) and signing uses deterministic RFC-6979 nonces, so the signature is reproducible. The private key is PEM-encoded PKCS#8. Output is base64 (and hex), in DER (ASN.1, OpenSSL-style) or raw fixed-length r||s (IEEE-P1363/JOSE) form. Runs locally — the private key and message never leave the device.",
        parameters = schema_json()
    ),
)]
impl EcdsaSign {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "ecdsa-sign", |a: Args| {
            let curve = Curve::parse(&a.curve).map_err(SkillError::InvalidArgs)?;
            let format = SigFormat::parse(&a.format).map_err(SkillError::InvalidArgs)?;
            sign(&a.message, &a.private_key, curve, format).map_err(SkillError::InvalidArgs)
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
                    "message": { "type": "string", "description": "The message to sign." },
                    "private_key": { "type": "string", "description": "Your EC private key, PEM-encoded PKCS#8 ('-----BEGIN PRIVATE KEY-----'). Must match the chosen curve." },
                    "curve": { "type": "string", "enum": ["p256", "p384"], "default": "p256", "description": "Elliptic curve of the key: p256 (NIST P-256/secp256r1, SHA-256) or p384 (NIST P-384/secp384r1, SHA-384)." },
                    "format": { "type": "string", "enum": ["der", "raw"], "default": "der", "description": "Signature encoding: der (ASN.1 DER, as OpenSSL emits) or raw (fixed-length r||s, IEEE-P1363/JOSE)." }
                },
                "required": ["message", "private_key"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
