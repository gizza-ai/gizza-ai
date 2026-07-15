//! gizza-ai/protect-pdf — add password encryption to a PDF (URL/ref) and return
//! the protected PDF.
//!
//! Pipeline: resolve the PDF → `core::run` (lopdf AES-256 standard security
//! handler) → base64 PDF envelope.
//!
//! Pure Rust → runs on ALL backends. Surfaces: chat + CLI. No page (Document
//! input + PDF bytes output — F3 no-page file-input pattern, like pdf-form-fill).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    replace_extension, Envelope, ForUi, Input, Param, SkillError, SkillResultExt, SourceFields,
    ToolDescriptor,
};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_BYTES: usize = 32 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    password: String,
    #[serde(default)]
    owner_password: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Document)
        .param(
            Param::string("password")
                .required()
                .describe("The password that will be required to open the PDF."),
        )
        .param(Param::string("owner_password").describe(
            "Optional separate owner password (controls permissions). Defaults to the same value as password.",
        ))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ProtectPdf;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/protect-pdf",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Password-protect a PDF",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Add password encryption to a PDF using the standard security handler (AES-256). The returned PDF requires the password to open in any viewer. Provide the PDF as either url (HTTP/HTTPS) or ref, a required password, and optionally a separate owner_password. Runs locally — the PDF and password never leave the device.",
        parameters = schema_json()
    ),
)]
impl ProtectPdf {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    use gizza_ai_block_utils::AssetKind;

    let args: Args = serde_json::from_slice(&body).invalid_args("protect-pdf")?;
    if args.password.is_empty() {
        return Err(SkillError::InvalidArgs(
            "a password is required to protect the PDF".into(),
        ));
    }
    let (bytes, _mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Document, MAX_BYTES)?;

    let out_pdf = gizza_ai_protect_pdf_core::run(&bytes, &args.password, &args.owner_password)
        .map_err(SkillError::InvalidArgs)?;

    let filename = replace_extension(&in_filename, "protected.pdf");
    let for_llm = format!(
        "password-protected {in_filename} with AES-256 encryption -> {filename} (the password is required to open it)"
    );
    let data_url = format!("data:application/pdf;base64,{}", B64.encode(&out_pdf));
    let env = Envelope {
        for_llm,
        for_ui: ForUi {
            data_url,
            mime: "application/pdf".to_string(),
            filename,
        },
    };
    serde_json::to_vec(&env).map_err(|e| SkillError::Serialize(format!("serialize envelope: {e}")))
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
                    "url":            { "type": "string", "description": "Document URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":            { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "password":       { "type": "string", "description": "The password that will be required to open the PDF." },
                    "owner_password": { "type": "string", "description": "Optional separate owner password (controls permissions). Defaults to the same value as password." }
                },
                "additionalProperties": false,
                "required": ["password"],
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
