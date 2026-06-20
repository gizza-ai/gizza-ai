//! gizza-ai/pdf-compress — shrink a PDF by recompressing its streams.
//!
//! Pipeline: load the source PDF's bytes via `block-utils` `resolve_source`
//! (URL fetch through `wafer-run/network`, or an uploaded attachment `ref`),
//! validated as the `application/*` document class → run the pure
//! `pdf-compress-core` (prune orphans + Flate-compress streams + renumber) →
//! base64-encode → emit a `{_for_llm, _for_ui}` envelope with a
//! `data:application/pdf` URL the chat UI can offer for download.
//!
//! `Input::Document` (scalar `url`⊕`ref`), no other params. No page surface —
//! a standalone page can't fetch an arbitrary PDF — so this is chat + CLI only.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    Envelope, ForUi, Input, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use serde::Deserialize;
use wafer_sdk::*;

/// PDF byte cap (16 MiB) — generous for documents while bounding the memory a
/// single compress pulls into the wasm sandbox.
const MAX_BYTES: usize = 16 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
}

/// Single-source param descriptor → chat schema (and CLI). `Input::Document`
/// emits the scalar `url`⊕`ref` oneOf; there are no other parameters.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Document)
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct PdfCompress;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/pdf-compress",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Shrink a PDF by recompressing its streams",
    requires = ["wafer-run/network"],
    skill(
        description = "Shrink a PDF's file size by dropping unused objects and Flate-compressing its streams (content and images), losslessly — text and vector graphics are preserved exactly and already-compressed image data is kept as-is (it does not re-encode images to a lossier codec). Provide the PDF as either a URL or a `ref` to an uploaded PDF attachment.",
        parameters = schema_json()
    ),
)]
impl PdfCompress {
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

    let args: Args = serde_json::from_slice(&body).invalid_args("pdf-compress")?;
    let (bytes, _mime, filename) =
        resolve_source(args.source.into_inner(), AssetKind::Document, MAX_BYTES)?;
    let original_len = bytes.len();

    let compressed = gizza_ai_pdf_compress_core::compress(&bytes).map_err(SkillError::InvalidArgs)?;
    let new_len = compressed.len();
    // Never hand back a larger file than we got: if recompression didn't help
    // (already-optimized PDF), return the original bytes unchanged.
    let (out, used_original) = if new_len < original_len {
        (compressed, false)
    } else {
        (bytes, true)
    };
    let out_len = out.len();

    let encoded = B64.encode(&out);
    let data_url = format!("data:application/pdf;base64,{encoded}");
    let pct = if original_len > 0 {
        (1.0 - (out_len as f64 / original_len as f64)) * 100.0
    } else {
        0.0
    };
    let for_llm = if used_original {
        format!("PDF was already optimally compressed ({original_len} bytes); returned unchanged.")
    } else {
        format!(
            "compressed {filename} from {original_len} to {out_len} bytes ({pct:.1}% smaller)"
        )
    };

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

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema (an `Input::Document` url⊕ref oneOf, no other params), so any
    /// future change to the LLM-facing API is intentional and reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "Document URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." }
                },
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn args_parse_url() {
        let a: Args = serde_json::from_str(r#"{"url":"https://x/doc.pdf"}"#).unwrap();
        assert!(matches!(a.source.into_inner(), gizza_ai_block_utils::Source::Url(_)));
    }

    #[test]
    fn args_reject_both_url_and_ref() {
        let err = serde_json::from_str::<Args>(r#"{"url":"u","ref":"r"}"#).unwrap_err();
        assert!(err.to_string().contains("exactly one"));
    }
}
