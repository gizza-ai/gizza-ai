//! gizza-ai/pem-der-convert — chat skill block on the shared tool abstraction.
//! Generic PEM<->DER re-encoder (keys, certs, CSRs). The chat schema is
//! single-sourced from descriptor() (which also drives the CLI); handle()
//! delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_pem_der_convert_core::{parse_der_format, parse_direction, run};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    direction: String,
    #[serde(default)]
    der_format: String,
    #[serde(default)]
    label: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The object to convert: a PEM block (-----BEGIN ...-----) for pem-to-der, or DER bytes as hex/base64 for der-to-pem."),
        )
        .param(
            Param::enumv("direction", ["auto", "pem-to-der", "der-to-pem"])
                .default("auto")
                .describe("Which way to convert: auto (default) detects PEM vs DER from the input; pem-to-der parses PEM and outputs DER; der-to-pem wraps DER bytes back into a PEM block."),
        )
        .param(
            Param::enumv("der_format", ["hex", "base64"])
                .default("hex")
                .describe("How DER binary is represented as text — both for the DER output (pem-to-der) and the DER input (der-to-pem). hex (default) or base64."),
        )
        .param(
            Param::string("label")
                .default("")
                .describe("PEM label to use when direction is der-to-pem (e.g. CERTIFICATE, PRIVATE KEY, EC PRIVATE KEY, CERTIFICATE REQUEST). Ignored for pem-to-der (the label is read from the input). Defaults to CERTIFICATE if blank."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/pem-der-convert",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert keys/certs/CSRs between PEM and DER",
    skill(
        description = "Convert cryptographic objects (keys, certificates, CSRs, CRLs) between PEM and DER encodings in either direction. PEM is the base64 text format wrapped in -----BEGIN <label>----- armor; DER is the raw binary ASN.1 form, shown/accepted here as hex or base64. Direction defaults to auto (detects PEM vs DER from the input); set pem-to-der to parse a PEM block (or a multi-block chain) and output each block's DER bytes + detected label, or der-to-pem to wrap DER bytes — supplied via der_format as hex or base64 — into a PEM block with the given label. This is a generic re-encoder: it does not parse the inner ASN.1, so it works for any object type.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "pem-der-convert", |a: Args| {
            let dir = parse_direction(&a.direction).map_err(SkillError::InvalidArgs)?;
            let fmt = parse_der_format(&a.der_format).map_err(SkillError::InvalidArgs)?;
            run(&a.input, dir, fmt, &a.label).map_err(SkillError::InvalidArgs)
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
                    "input":      { "type": "string", "description": "The object to convert: a PEM block (-----BEGIN ...-----) for pem-to-der, or DER bytes as hex/base64 for der-to-pem." },
                    "direction":  { "type": "string", "enum": ["auto", "pem-to-der", "der-to-pem"], "default": "auto", "description": "Which way to convert: auto (default) detects PEM vs DER from the input; pem-to-der parses PEM and outputs DER; der-to-pem wraps DER bytes back into a PEM block." },
                    "der_format": { "type": "string", "enum": ["hex", "base64"], "default": "hex", "description": "How DER binary is represented as text — both for the DER output (pem-to-der) and the DER input (der-to-pem). hex (default) or base64." },
                    "label":      { "type": "string", "default": "", "description": "PEM label to use when direction is der-to-pem (e.g. CERTIFICATE, PRIVATE KEY, EC PRIVATE KEY, CERTIFICATE REQUEST). Ignored for pem-to-der (the label is read from the input). Defaults to CERTIFICATE if blank." }
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
