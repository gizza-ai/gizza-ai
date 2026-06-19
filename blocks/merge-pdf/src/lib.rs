//! gizza-ai/merge-pdf — combine several PDFs into one, in order.
//!
//! Pipeline: parse `{ "inputs": [ {url|ref}, ... ] }` → load each PDF's bytes
//! via `block-utils` `resolve_source` (URL fetch through `wafer-run/network`,
//! or an uploaded attachment `ref`), all validated as the `application/*`
//! document MIME class → concatenate every page with the pure `merge-pdf-core`
//! → base64-encode → emit an envelope `{_for_llm, _for_ui}` with a
//! `data:application/pdf` URL the chat UI can offer for download.
//!
//! The chat schema is derived from `descriptor()` (single source — shared shape
//! across chat + CLI): `Input::None` (the inputs arrive as an array param, not
//! the scalar `url`⊕`ref` an `Input::Document` block takes) plus a required
//! `inputs` [`Param::source_list`] of at least two `{ url | ref }` sources. No
//! page surface — chat + CLI only.

// The #[wafer_block] macro emits the impl gated to wasm32. The supporting
// imports + the Args type are only used inside that impl, so they look "unused"
// when running native unit tests. The descriptor stays native-compilable so the
// drift-guard unit test below can exercise it.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    Envelope, ForUi, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use serde::Deserialize;
use wafer_sdk::*;

/// Per-input PDF byte cap (8 MiB) — generous for documents while bounding the
/// memory a single merge can pull into the wasm sandbox.
const MAX_INPUT_BYTES: usize = 8 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    /// At least two PDF sources, each exactly one of `url` or `ref`. Merged in
    /// the order given.
    inputs: Vec<SourceFields>,
}

/// Single-source param descriptor → chat schema (and CLI). See
/// docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.
/// `Input::None` because merge-pdf takes its sources as an array param, not the
/// scalar `url`⊕`ref` an `Input::Document` block emits; `inputs` is a required
/// array of at least two `{ url | ref }` source objects.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None).param(Param::source_list("inputs", 2).required().describe(
        "Ordered list of PDF sources to merge. Each item has exactly one of `url` or `ref`.",
    ))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct MergePdf;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/merge-pdf",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Combine multiple PDFs into one",
    requires = ["wafer-run/network"],
    skill(
        description = "Combine multiple PDF files into a single PDF, in the given order. Provide at least two PDF sources; each source is either a URL or a `ref` to an uploaded PDF attachment.",
        parameters = schema_json()
    ),
)]
impl MergePdf {
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

    let args: Args = serde_json::from_slice(&body).invalid_args("merge-pdf")?;
    if args.inputs.len() < 2 {
        return Err(SkillError::InvalidArgs(format!(
            "merge-pdf needs at least 2 inputs, got {}",
            args.inputs.len()
        )));
    }

    // Load every source's bytes, validated as the application/* document class.
    let mut pdfs: Vec<Vec<u8>> = Vec::with_capacity(args.inputs.len());
    for field in args.inputs {
        let (bytes, _mime, _filename) =
            resolve_source(field.into_inner(), AssetKind::Document, MAX_INPUT_BYTES)?;
        pdfs.push(bytes);
    }

    let merged = gizza_ai_merge_pdf_core::merge(&pdfs).map_err(SkillError::InvalidArgs)?;

    let merged_len = merged.len();
    let encoded = B64.encode(&merged);
    let data_url = format!("data:application/pdf;base64,{encoded}");

    let env = Envelope {
        for_llm: format!(
            "merged {} PDFs into a single {merged_len}-byte PDF (merged.pdf)",
            pdfs.len()
        ),
        for_ui: ForUi {
            data_url,
            mime: "application/pdf".to_string(),
            filename: "merged.pdf".to_string(),
        },
    };
    serde_json::to_vec(&env).map_err(|e| SkillError::Serialize(format!("serialize envelope: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Migration safety: the descriptor-derived chat schema must match the
    /// pre-retrofit authored schema, so the LLM sees no drift. Two intentional,
    /// documented deltas from the hand-authored blob:
    ///   - the per-item `url`/`ref` descriptions — `Param::source_list` emits
    ///     the centralized generic wording ("URL (HTTP/HTTPS). Use either url or
    ///     ref." / "Reference id from a prior tool call. Use either url or
    ///     ref."), so the expected blob uses those rather than merge-pdf's old
    ///     bespoke "…PDF…" strings (the same centralization precedent the other
    ///     retrofitted tools accepted for their scalar `url`/`ref`).
    ///   - the top-level `inputs` `description` — `Param::describe` renders the
    ///     authored summary, kept verbatim.
    /// The `inputs` array shape — `type: array`, `minItems: 2`, the `items`
    /// object (`additionalProperties: false`), `required: ["inputs"]`, and the
    /// top-level `additionalProperties: false` — matches the authored schema
    /// exactly. (The authored item objects carried no per-item `description`;
    /// the descriptor adds them — the only structural addition, and a
    /// documentation-only one.)
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "inputs": {
                        "type": "array",
                        "minItems": 2,
                        "description": "Ordered list of PDF sources to merge. Each item has exactly one of `url` or `ref`.",
                        "items": {
                            "type": "object",
                            "properties": {
                                "url": { "type": "string", "description": "URL (HTTP/HTTPS). Use either url or ref." },
                                "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." }
                            },
                            "additionalProperties": false
                        }
                    }
                },
                "required": ["inputs"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn args_parse_two_url_inputs() {
        let a: Args = serde_json::from_str(
            r#"{"inputs":[{"url":"https://x/a.pdf"},{"url":"https://x/b.pdf"}]}"#,
        )
        .unwrap();
        assert_eq!(a.inputs.len(), 2);
    }

    #[test]
    fn args_parse_mixed_url_and_ref() {
        let a: Args =
            serde_json::from_str(r#"{"inputs":[{"url":"https://x/a.pdf"},{"ref":"call_7"}]}"#)
                .unwrap();
        assert_eq!(a.inputs.len(), 2);
    }

    #[test]
    fn args_reject_item_with_both_url_and_ref() {
        let err =
            serde_json::from_str::<Args>(r#"{"inputs":[{"url":"u","ref":"r"}]}"#).unwrap_err();
        assert!(err.to_string().contains("exactly one"));
    }
}
