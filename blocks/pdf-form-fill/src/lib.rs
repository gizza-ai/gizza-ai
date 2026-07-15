//! gizza-ai/pdf-form-fill — fill AcroForm fields in a fillable PDF (URL/ref) and
//! return the filled PDF.
//!
//! Pipeline: resolve the PDF → parse `fields` (JSON object name->value) →
//! `core::fill` (lopdf) → base64 PDF envelope. The for_llm reports filled +
//! unknown field names.
//!
//! Pure Rust → runs on ALL backends. Surfaces: chat + CLI. No page (Document
//! input + PDF bytes output — F3 no-page file-input pattern, like pdf-rotate).
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
    /// JSON object of field name -> value.
    fields: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Document).param(
        Param::string("fields")
            .required()
            .describe("A JSON object mapping form field names to values, e.g. {\"name\":\"Ada\",\"agree\":\"Yes\"}. Text fields take strings; checkbox/radio (button) fields take the on-state name."),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

/// Parse the `fields` JSON object into name->string pairs (coercing scalar values).
fn parse_fields(s: &str) -> Result<Vec<(String, String)>, String> {
    let v: serde_json::Value = serde_json::from_str(s).map_err(|e| format!("`fields` must be a JSON object: {e}"))?;
    let obj = v.as_object().ok_or("`fields` must be a JSON object")?;
    let mut out = Vec::new();
    for (k, val) in obj {
        let s = match val {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Bool(b) => b.to_string(),
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::Null => String::new(),
            other => other.to_string(),
        };
        out.push((k.clone(), s));
    }
    if out.is_empty() {
        return Err("`fields` is empty — provide at least one field".into());
    }
    Ok(out)
}

#[cfg(target_arch = "wasm32")]
struct PdfFormFill;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/pdf-form-fill",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Fill AcroForm fields in a PDF",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Fill the interactive AcroForm fields of a fillable PDF and return the filled PDF. `fields` is a JSON object mapping field names to values (text fields take strings; checkbox/radio fields take the on-state name). Sets NeedAppearances so viewers render the values. Provide the PDF as either url (HTTP/HTTPS) or ref. The response lists which fields were filled and which names were not found.",
        parameters = schema_json()
    ),
)]
impl PdfFormFill {
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

    let args: Args = serde_json::from_slice(&body).invalid_args("pdf-form-fill")?;
    let fields = parse_fields(&args.fields).map_err(SkillError::InvalidArgs)?;
    let (bytes, _mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Document, MAX_BYTES)?;

    let res = gizza_ai_pdf_form_fill_core::fill(&bytes, &fields).map_err(SkillError::InvalidArgs)?;

    let filename = replace_extension(&in_filename, "filled.pdf");
    let unknown_note = if res.unknown.is_empty() {
        String::new()
    } else {
        let sample: Vec<&str> = res.available.iter().take(40).map(|s| s.as_str()).collect();
        format!("; not found: {} (available fields: {})", res.unknown.join(", "), sample.join(", "))
    };
    let for_llm = format!(
        "filled {} of {} field(s) in {in_filename} (filled: {}{unknown_note}) -> {filename}",
        res.filled.len(),
        res.total_fields,
        res.filled.join(", ")
    );
    let data_url = format!("data:application/pdf;base64,{}", B64.encode(&res.pdf));
    let env = Envelope {
        for_llm,
        for_ui: ForUi { data_url, mime: "application/pdf".to_string(), filename },
    };
    serde_json::to_vec(&env).map_err(|e| SkillError::Serialize(format!("serialize envelope: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_fields_coerces_scalars() {
        let f = parse_fields(r#"{"a":"x","b":true,"c":3}"#).unwrap();
        assert!(f.contains(&("a".to_string(), "x".to_string())));
        assert!(f.contains(&("b".to_string(), "true".to_string())));
        assert!(f.contains(&("c".to_string(), "3".to_string())));
        assert!(parse_fields("[]").is_err());
        assert!(parse_fields("{}").is_err());
    }

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":    { "type": "string", "description": "Document URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":    { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "fields": { "type": "string", "description": "A JSON object mapping form field names to values, e.g. {\"name\":\"Ada\",\"agree\":\"Yes\"}. Text fields take strings; checkbox/radio (button) fields take the on-state name." }
                },
                "additionalProperties": false,
                "required": ["fields"],
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
