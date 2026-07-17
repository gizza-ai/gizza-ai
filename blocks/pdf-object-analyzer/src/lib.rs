//! gizza-ai/pdf-object-analyzer — map a PDF's object tree and surface the
//! structural indicators used in malicious-document triage: automatic actions
//! that fire on open (`/OpenAction`, `/AA`), embedded JavaScript (`/JS`,
//! `/JavaScript`), `/Launch` actions that can start an external program,
//! embedded files, and outbound links (`/URI`, `/SubmitForm`, `/GoToR`).
//!
//! Pipeline: parse `{pdf_base64, detail}` → decode the PDF bytes → delegate to
//! the pure `core::analyze` (lopdf) → return the structured analysis as a flat
//! JSON response the LLM reads directly.
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI). Like pdf-signature-inspector / pdf-extract-text, the
//! success shape is a flat JSON object (not the `{ "result": … }` wrapper
//! `run_skill` produces), so the handler stays thin.
//!
//! Static structural analysis only: it never executes, sandboxes, or renders the
//! PDF, and it does NOT make a malware verdict.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::Engine;
#[cfg(test)]
use gizza_ai_block_utils::{Input, Param, ToolDescriptor};
use gizza_ai_block_utils::SkillError;
use gizza_ai_pdf_object_analyzer_core::analyze;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 16 * 1024 * 1024; // 16 MiB

#[derive(Deserialize, Debug)]
struct Args {
    /// Base64-encoded PDF bytes. A `data:application/pdf;base64,...` prefix is
    /// accepted for copy/paste convenience.
    pdf_base64: String,
    /// `full` (default) includes extracted JavaScript source and the object-id
    /// lists per indicator; `summary` returns only the structural overview,
    /// indicator keys/counts, risk level, and target lists.
    #[serde(default)]
    detail: Option<String>,
}

/// Single-source param descriptor → chat schema (and CLI). The PDF is supplied
/// as base64 so this analyzer stays pure/no-network and can run in every wafer
/// runtime; `detail` is a fixed-choice report mode.
#[cfg(test)]
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("pdf_base64")
                .required()
                .describe(
                    "Base64-encoded PDF bytes. A data:application/pdf;base64,... prefix is accepted.",
                ),
        )
        .param(
        Param::enumv("detail", ["full", "summary"])
            .default("full")
            .describe(
                "Report depth. 'full' (default) includes extracted JavaScript \
                 source snippets and the object-id list behind each indicator; \
                 'summary' returns only the structural overview, indicator \
                 keys/counts, risk level, and launch/URL/embedded-file targets.",
            ),
        )
}

#[cfg(test)]
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct PdfObjectAnalyzer;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/pdf-object-analyzer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Map a PDF's object tree and flag JavaScript, embedded files, and auto-run triggers",
    skill(
        description = "Statically map a PDF's object tree and flag the structural indicators used in malicious-document triage: automatic actions that fire on open (/OpenAction, /AA), embedded JavaScript (/JS, /JavaScript), /Launch actions that start an external program, embedded files (/EmbeddedFile, /Filespec), object streams (/ObjStm) and other obfuscation, and outbound links (/URI, /SubmitForm, /GoToR). Reports the PDF version, object/stream/page counts, encryption, every indicator with its category, count, and the object ids it appeared in, extracted JavaScript snippets, embedded-file and /Launch targets, referenced URLs, and a coarse risk level (none/low/medium/high) with reasons. detail=summary omits the JavaScript source and object-id lists. Provide pdf_base64 containing the PDF bytes (a data:application/pdf;base64,... prefix is accepted). This is static structural analysis: it does NOT execute, sandbox, or render the PDF, and it is NOT a malware verdict.",
        parameters = r#"{
                "type": "object",
                "properties": {
                    "pdf_base64": { "type": "string", "description": "Base64-encoded PDF bytes. A data:application/pdf;base64,... prefix is accepted." },
                    "detail": {
                        "type": "string",
                        "enum": ["full", "summary"],
                        "default": "full",
                        "description": "Report depth. 'full' (default) includes extracted JavaScript source snippets and the object-id list behind each indicator; 'summary' returns only the structural overview, indicator keys/counts, risk level, and launch/URL/embedded-file targets."
                    }
                },
                "additionalProperties": false,
                "required": ["pdf_base64"]
            }"#
    )
)]
impl PdfObjectAnalyzer {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).map_err(|e| {
        SkillError::InvalidArgs(format!("invalid arguments for pdf-object-analyzer: {e}"))
    })?;

    let summary = match args.detail.as_deref() {
        None | Some("full") => false,
        Some("summary") => true,
        Some(other) => {
            return Err(SkillError::InvalidArgs(format!(
                "detail must be 'full' or 'summary', got '{other}'"
            )))
        }
    };

    let encoded = args
        .pdf_base64
        .strip_prefix("data:application/pdf;base64,")
        .unwrap_or(&args.pdf_base64);
    let input_bytes = base64::engine::general_purpose::STANDARD
        .decode(encoded.trim())
        .map_err(|e| SkillError::InvalidArgs(format!("pdf_base64 is not valid base64: {e}")))?;
    if input_bytes.len() > MAX_INPUT_BYTES {
        return Err(SkillError::InvalidArgs(format!(
            "PDF is too large: {} bytes (limit {MAX_INPUT_BYTES})",
            input_bytes.len()
        )));
    }

    let mut analysis = analyze(&input_bytes).map_err(SkillError::InvalidArgs)?;
    if summary {
        analysis = analysis.summarized();
    }

    serde_json::to_vec(&analysis)
        .map_err(|e| SkillError::Serialize(format!("serialize pdf-object-analyzer response: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Migration safety: the descriptor-derived chat schema must match the
    /// authored schema (drift guard). `pdf_base64` is required; `detail` is an
    /// enum with a `full` default.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "pdf_base64": { "type": "string", "description": "Base64-encoded PDF bytes. A data:application/pdf;base64,... prefix is accepted." },
                    "detail": {
                        "type": "string",
                        "enum": ["full", "summary"],
                        "default": "full",
                        "description": "Report depth. 'full' (default) includes extracted JavaScript source snippets and the object-id list behind each indicator; 'summary' returns only the structural overview, indicator keys/counts, risk level, and launch/URL/embedded-file targets."
                    }
                },
                "additionalProperties": false,
                "required": ["pdf_base64"]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn args_parse_base64_default_detail() {
        let a: Args = serde_json::from_str(r#"{"pdf_base64":"JVBERi0xLjQ="}"#).unwrap();
        assert_eq!(a.pdf_base64, "JVBERi0xLjQ=");
        assert_eq!(a.detail, None);
    }

    #[test]
    fn args_parse_data_url_summary() {
        let a: Args = serde_json::from_str(
            r#"{"pdf_base64":"data:application/pdf;base64,JVBERi0xLjQ=","detail":"summary"}"#,
        )
        .unwrap();
        assert!(a.pdf_base64.starts_with("data:application/pdf;base64,"));
        assert_eq!(a.detail.as_deref(), Some("summary"));
    }

    #[test]
    fn args_require_base64() {
        let err = serde_json::from_str::<Args>(r#"{"detail":"summary"}"#).unwrap_err();
        assert!(err.to_string().contains("missing field `pdf_base64`"));
    }
}
