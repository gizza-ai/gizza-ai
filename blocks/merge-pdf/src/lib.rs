//! gizza-ai/merge-pdf — combine several PDFs into one, in order.
//!
//! Pipeline: parse `{ "inputs": [ {url|ref}, ... ] }` → load each PDF's bytes
//! via `block-utils` (`fetch_from_url` for a URL, `load_from_attachment` for an
//! uploaded `ref`), all validated as `application/pdf` → concatenate every page
//! with the pure `merge-pdf-core` → base64-encode → emit an envelope
//! `{_for_llm, _for_ui}` with a `data:application/pdf` URL the chat UI can
//! offer for download. No page surface — chat + CLI only.

// The #[wafer_block] macro emits wasm-only registration; supporting imports
// and the Args type are only used inside that impl.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use gizza_ai_block_utils::{Envelope, ForUi, SkillError, SkillResultExt, Source, SourceFields};
use serde::Deserialize;
use wafer_sdk::*;

/// Per-input PDF byte cap (8 MiB) — generous for documents while bounding the
/// memory a single merge can pull into the wasm sandbox.
const MAX_INPUT_BYTES: usize = 8 * 1024 * 1024;

#[derive(Deserialize)]
struct Args {
    /// At least two PDF sources, each exactly one of `url` or `ref`. Merged in
    /// the order given.
    inputs: Vec<SourceFields>,
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
        parameters = r#"{
            "type": "object",
            "properties": {
                "inputs": {
                    "type": "array",
                    "minItems": 2,
                    "description": "Ordered list of PDF sources to merge. Each item has exactly one of `url` or `ref`.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "url": { "type": "string", "description": "HTTP/HTTPS URL of a PDF to fetch." },
                            "ref": { "type": "string", "description": "Reference id of an uploaded PDF attachment." }
                        },
                        "additionalProperties": false
                    }
                }
            },
            "required": ["inputs"],
            "additionalProperties": false
        }"#
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
    use gizza_ai_block_utils::{fetch_from_url, load_from_attachment, AssetKind};

    let args: Args = serde_json::from_slice(&body).invalid_args("merge-pdf")?;
    if args.inputs.len() < 2 {
        return Err(SkillError::InvalidArgs(format!(
            "merge-pdf needs at least 2 inputs, got {}",
            args.inputs.len()
        )));
    }

    // Load every source's bytes, validated as application/pdf.
    let mut pdfs: Vec<Vec<u8>> = Vec::with_capacity(args.inputs.len());
    for field in args.inputs {
        let (bytes, _mime, _filename) = match field.into_inner() {
            Source::Url(url) => fetch_from_url(&url, AssetKind::Document, MAX_INPUT_BYTES)?,
            Source::Ref(id) => load_from_attachment(&id, AssetKind::Document, MAX_INPUT_BYTES)?,
        };
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
