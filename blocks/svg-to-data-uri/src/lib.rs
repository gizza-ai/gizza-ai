//! gizza-ai/svg-to-data-uri — encode SVG markup into an inline `data:` URI and
//! a ready-to-paste CSS/HTML/JSX snippet.
//!
//! Thin chat-skill wrapper around `gizza-ai-svg-to-data-uri-core`. The chat
//! schema is single-sourced from `descriptor()` (shared with the CLI); the
//! handler delegates to `block_utils::run_skill`. No host calls — runs entirely
//! inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    svg: String,
    #[serde(default)]
    encoding: String,
    #[serde(default)]
    output: String,
    #[serde(default)]
    quotes: String,
    #[serde(default = "yes")]
    minify: bool,
    #[serde(default = "yes")]
    add_xmlns: bool,
}

/// Both booleans default ON — they only ever shrink the URI or make it render.
fn yes() -> bool {
    true
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("svg")
                .required()
                .describe("The SVG markup to encode, e.g. '<svg viewBox=\"0 0 16 16\"><circle cx=\"8\" cy=\"8\" r=\"7\" fill=\"#0af\"/></svg>'. Must contain a root <svg> element. Limit 1000000 bytes."),
        )
        .param(
            Param::enumv("encoding", ["url", "base64"])
                .default("url")
                .describe("Payload encoding. 'url' (default) applies the minimal percent-encoding that is safe inside a quoted CSS url(\"...\") or HTML attribute and is usually 20-30% shorter for SVG; 'base64' emits 'data:image/svg+xml;base64,...', about 33% larger but opaque to tooling that rewrites markup."),
        )
        .param(
            Param::enumv("output", ["uri", "css", "mask", "img", "jsx", "compare"])
                .default("uri")
                .describe("Which snippet to return. 'uri' (default) the bare data: URI; 'css' a background-image declaration; 'mask' mask-image plus the -webkit- prefixed twin; 'img' an HTML <img> tag; 'jsx' a small React component; 'compare' a size report of both encodings naming the shorter one."),
        )
        .param(
            Param::enumv("quotes", ["single", "encode"])
                .default("single")
                .describe("How the double quotes around SVG attribute values are handled in the 'url' encoding, since the URI has to sit inside url(\"...\"). 'single' (default) rewrites them to apostrophes — one byte each and still valid XML; 'encode' percent-encodes them as %22, leaving the markup byte-identical. Ignored when encoding is 'base64'."),
        )
        .param(
            Param::boolean("minify")
                .default(true)
                .describe("Strip the XML declaration, DOCTYPE and comments and collapse redundant whitespace before encoding (default true). Turn it off to keep the markup byte-for-byte, e.g. when <text> content relies on runs of spaces."),
        )
        .param(
            Param::boolean("add_xmlns")
                .default(true)
                .describe("Add xmlns=\"http://www.w3.org/2000/svg\" to the root element when it is missing (default true). Without it a data-URI SVG renders as nothing in CSS url(), which is the most common cause of a blank result."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/svg-to-data-uri",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Encode SVG markup into a URL-escaped or base64 data URI plus a ready-to-paste CSS, HTML or JSX snippet.",
    skill(
        description = "Encode SVG markup into an inline 'data:image/svg+xml' URI for CSS url(...), an <img src>, a mask-image or a JSX component — no extra HTTP request. Pass the markup as 'svg'. 'encoding' is 'url' (default: minimal percent-encoding, usually the shorter form for SVG) or 'base64' (~33% larger). 'output' picks the snippet: uri (default), css, mask, img, jsx, or compare (a size report naming the shorter encoding). 'quotes' controls the attribute double quotes in the url form: 'single' (default, rewritten to apostrophes) or 'encode' (%22). 'minify' (default true) strips the XML declaration, DOCTYPE, comments and redundant whitespace; 'add_xmlns' (default true) injects the SVG namespace when the root element lacks it, without which the URI renders blank. Returns the snippet plus the data URI, both encodings' lengths, which is smaller, and the byte counts. To decode a data URI back, use data-uri-decode; to shrink the markup itself first, use svg-optimize; for non-SVG content use data-uri-encode or file-to-data-uri.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "svg-to-data-uri", |a: Args| {
            gizza_ai_svg_to_data_uri_core::run(
                &a.svg,
                &a.encoding,
                &a.output,
                &a.quotes,
                a.minify,
                a.add_xmlns,
            )
            .map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional and
    /// reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r##"{
                "type": "object",
                "properties": {
                    "svg": { "type": "string", "description": "The SVG markup to encode, e.g. '<svg viewBox=\"0 0 16 16\"><circle cx=\"8\" cy=\"8\" r=\"7\" fill=\"#0af\"/></svg>'. Must contain a root <svg> element. Limit 1000000 bytes." },
                    "encoding": { "type": "string", "enum": ["url", "base64"], "default": "url", "description": "Payload encoding. 'url' (default) applies the minimal percent-encoding that is safe inside a quoted CSS url(\"...\") or HTML attribute and is usually 20-30% shorter for SVG; 'base64' emits 'data:image/svg+xml;base64,...', about 33% larger but opaque to tooling that rewrites markup." },
                    "output": { "type": "string", "enum": ["uri", "css", "mask", "img", "jsx", "compare"], "default": "uri", "description": "Which snippet to return. 'uri' (default) the bare data: URI; 'css' a background-image declaration; 'mask' mask-image plus the -webkit- prefixed twin; 'img' an HTML <img> tag; 'jsx' a small React component; 'compare' a size report of both encodings naming the shorter one." },
                    "quotes": { "type": "string", "enum": ["single", "encode"], "default": "single", "description": "How the double quotes around SVG attribute values are handled in the 'url' encoding, since the URI has to sit inside url(\"...\"). 'single' (default) rewrites them to apostrophes — one byte each and still valid XML; 'encode' percent-encodes them as %22, leaving the markup byte-identical. Ignored when encoding is 'base64'." },
                    "minify": { "type": "boolean", "default": true, "description": "Strip the XML declaration, DOCTYPE and comments and collapse redundant whitespace before encoding (default true). Turn it off to keep the markup byte-for-byte, e.g. when <text> content relies on runs of spaces." },
                    "add_xmlns": { "type": "boolean", "default": true, "description": "Add xmlns=\"http://www.w3.org/2000/svg\" to the root element when it is missing (default true). Without it a data-URI SVG renders as nothing in CSS url(), which is the most common cause of a blank result." }
                },
                "required": ["svg"],
                "additionalProperties": false
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
