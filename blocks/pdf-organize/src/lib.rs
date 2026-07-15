//! gizza-ai/pdf-organize — reorder, duplicate, delete, and rotate PDF pages in
//! one operation.
//!
//! Pipeline: load the source PDF via `block-utils` `resolve_source` (URL fetch
//! through `wafer-run/network`, or an uploaded attachment `ref`), validated as
//! the document class → run the pure `pdf-organize-core` (`order` sequence for
//! reorder/duplicate/delete + optional `rotate` on the selected originals) →
//! base64-encode → emit a `{_for_llm, _for_ui}` envelope with a
//! `data:application/pdf` URL.
//!
//! `Input::Document` (scalar `url`⊕`ref`) + optional `order`/`rotate`/
//! `rotate_pages`. No page surface (a page can't fetch an arbitrary PDF, and a
//! PDF output has no in-page render) — chat + CLI only.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    Envelope, ForUi, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_BYTES: usize = 16 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "default_all")]
    order: String,
    #[serde(default)]
    rotate: i64,
    #[serde(default = "default_all")]
    rotate_pages: String,
}
fn default_all() -> String {
    "all".to_string()
}

/// Single-source param descriptor → chat schema (and CLI). `Input::Document`
/// emits the scalar `url`⊕`ref` oneOf; every operation is optional so the tool
/// no-ops to a pass-through when called with just a PDF.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Document)
        .param(Param::string("order").default("all").describe(
            "Output page order: a comma list of 1-based source page numbers, in the exact order you want them. Reorder ('3,1,2'), duplicate by repeating ('1,1,2'), delete by omitting ('1,3' drops page 2), and use ranges ('2-4', or '4-2' to count down). 'all' (default) keeps the original order; 'reverse' flips it.",
        ))
        .param(Param::integer("rotate").default(0).describe(
            "Degrees to add to the selected pages' rotation, a multiple of 90 (90, 180, 270, or -90). 0 (default) leaves rotation unchanged.",
        ))
        .param(Param::string("rotate_pages").default("all").describe(
            "Which ORIGINAL pages to rotate by `rotate`, 1-based, e.g. '1,3-5' or 'all' (default). Applied before reordering, so duplicated pages inherit the rotation. Ignored when rotate is 0.",
        ))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct PdfOrganize;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/pdf-organize",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Reorder, duplicate, delete, and rotate PDF pages",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Reorder, duplicate, delete, and rotate the pages of a PDF in one operation. `order` is the output sequence of 1-based source page numbers ('3,1,2' reorders, '1,1,2' duplicates, '1,3' deletes page 2, ranges like '2-4' and 'reverse' work; 'all' keeps the original order). Optionally add `rotate` degrees (a multiple of 90) to the pages named by `rotate_pages`. Provide the PDF as either url (HTTP/HTTPS) or ref (id from a prior tool call).",
        parameters = schema_json()
    ),
)]
impl PdfOrganize {
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

    let args: Args = serde_json::from_slice(&body).invalid_args("pdf-organize")?;
    let (bytes, _mime, filename) =
        resolve_source(args.source.into_inner(), AssetKind::Document, MAX_BYTES)?;

    let out = gizza_ai_pdf_organize_core::organize(&bytes, &args.order, args.rotate, &args.rotate_pages)
        .map_err(SkillError::InvalidArgs)?;
    let out_len = out.len();

    let encoded = B64.encode(&out);
    let data_url = format!("data:application/pdf;base64,{encoded}");
    let out_name = filename
        .strip_suffix(".pdf")
        .map(|stem| format!("{stem}-organized.pdf"))
        .unwrap_or_else(|| "organized.pdf".to_string());

    let rotate_note = if args.rotate != 0 {
        format!(", rotated pages '{}' by {} deg", args.rotate_pages, args.rotate)
    } else {
        String::new()
    };
    let env = Envelope {
        for_llm: format!(
            "reorganized {filename} to page order '{}'{rotate_note} — {out_len}-byte PDF ({out_name})",
            args.order
        ),
        for_ui: ForUi {
            data_url,
            mime: "application/pdf".to_string(),
            filename: out_name,
        },
    };
    serde_json::to_vec(&env).map_err(|e| SkillError::Serialize(format!("serialize envelope: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema (Input::Document url⊕ref oneOf + optional order/rotate/rotate_pages),
    /// so any future change to the LLM-facing API is intentional and reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":          { "type": "string", "description": "Document URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":          { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "order":        { "type": "string", "default": "all", "description": "Output page order: a comma list of 1-based source page numbers, in the exact order you want them. Reorder ('3,1,2'), duplicate by repeating ('1,1,2'), delete by omitting ('1,3' drops page 2), and use ranges ('2-4', or '4-2' to count down). 'all' (default) keeps the original order; 'reverse' flips it." },
                    "rotate":       { "type": "integer", "default": 0, "description": "Degrees to add to the selected pages' rotation, a multiple of 90 (90, 180, 270, or -90). 0 (default) leaves rotation unchanged." },
                    "rotate_pages": { "type": "string", "default": "all", "description": "Which ORIGINAL pages to rotate by `rotate`, 1-based, e.g. '1,3-5' or 'all' (default). Applied before reordering, so duplicated pages inherit the rotation. Ignored when rotate is 0." }
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
    fn args_default_to_passthrough() {
        let a: Args = serde_json::from_str(r#"{"url":"https://x/d.pdf"}"#).unwrap();
        assert_eq!(a.order, "all");
        assert_eq!(a.rotate, 0);
        assert_eq!(a.rotate_pages, "all");
    }

    #[test]
    fn args_parse_order_and_rotate() {
        let a: Args =
            serde_json::from_str(r#"{"url":"https://x/d.pdf","order":"3,1,2","rotate":90,"rotate_pages":"1"}"#)
                .unwrap();
        assert_eq!(a.order, "3,1,2");
        assert_eq!(a.rotate, 90);
        assert_eq!(a.rotate_pages, "1");
    }
}
