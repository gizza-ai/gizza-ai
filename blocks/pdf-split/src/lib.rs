//! gizza-ai/pdf-split — keep specific pages of a PDF and drop the rest.
//!
//! Pipeline: load the source PDF via `block-utils` `resolve_source` (URL fetch
//! through `wafer-run/network`, or an uploaded attachment `ref`), validated as
//! the `application/*` document class → run the pure `pdf-split-core` (parse the
//! 1-based page spec, delete unselected pages, prune + renumber) → base64-encode
//! → emit a `{_for_llm, _for_ui}` envelope with a `data:application/pdf` URL.
//!
//! `Input::Document` (scalar `url`⊕`ref`) + a required `pages` spec. No page
//! surface (a page can't fetch an arbitrary PDF) — chat + CLI only.
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
    pages: String,
}

/// Single-source param descriptor → chat schema (and CLI). `Input::Document`
/// emits the scalar `url`⊕`ref` oneOf; `pages` is the required page spec.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Document).param(
        Param::string("pages").required().describe(
            "Which pages to keep, 1-based, as a comma list with inclusive ranges, e.g. '1,3-5,8'. Use 'all', 'odd', or 'even' for those page sets. Output order follows the original document.",
        ),
    )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct PdfSplit;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/pdf-split",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract or split specific pages out of a PDF",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Extract specific pages from a PDF into a new PDF. Give the page selection as a 1-based comma list with inclusive ranges, e.g. '1,3-5,8' (or 'all'/'odd'/'even'); output page order follows the original document. Provide the PDF as either a URL or a `ref` to an uploaded PDF attachment.",
        parameters = schema_json()
    ),
)]
impl PdfSplit {
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

    let args: Args = serde_json::from_slice(&body).invalid_args("pdf-split")?;
    let (bytes, _mime, filename) =
        resolve_source(args.source.into_inner(), AssetKind::Document, MAX_BYTES)?;

    let out = gizza_ai_pdf_split_core::split(&bytes, &args.pages).map_err(SkillError::InvalidArgs)?;
    let out_len = out.len();

    let encoded = B64.encode(&out);
    let data_url = format!("data:application/pdf;base64,{encoded}");
    let out_name = filename
        .strip_suffix(".pdf")
        .map(|stem| format!("{stem}-pages.pdf"))
        .unwrap_or_else(|| "pages.pdf".to_string());

    let env = Envelope {
        for_llm: format!("extracted pages '{}' into a {out_len}-byte PDF ({out_name})", args.pages),
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
    /// schema (Input::Document url⊕ref oneOf + required `pages`), so any future
    /// change to the LLM-facing API is intentional and reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "Document URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "pages": { "type": "string", "description": "Which pages to keep, 1-based, as a comma list with inclusive ranges, e.g. '1,3-5,8'. Use 'all', 'odd', or 'even' for those page sets. Output order follows the original document." }
                },
                "required": ["pages"],
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn args_parse_url_and_pages() {
        let a: Args = serde_json::from_str(r#"{"url":"https://x/d.pdf","pages":"1-3"}"#).unwrap();
        assert_eq!(a.pages, "1-3");
    }
}
