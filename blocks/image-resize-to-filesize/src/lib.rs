//! gizza-ai/image-resize-to-filesize — fetch an image (url⊕ref), then binary-search
//! the encoder quality (optionally shrinking the width first) to produce a JPEG or
//! WebP at or under a target file size in KB.
//!
//! The chat schema is single-sourced from `descriptor()` (shared across chat + CLI
//! + page). Unlike a single-pass ffmpeg tool, `run()` calls `dispatch_ffmpeg` in a
//! LOOP: the shared `core::search_quality` drives ~7 encode attempts (via
//! `core::plan_attempt`) to find the highest quality that fits the budget. The page
//! mirrors the same search in `page/custom.js`.
//!
//! Chat note: ffmpeg cannot run inside the chat Service Worker, so this tool is
//! functional on its standalone PAGE and the CLI. See the shared-tool spec:
//! docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, mime_to_ext, AssetKind, Input, Param, SkillError,
    SourceFields, ToolDescriptor,
};
// resolve_source / dispatch_ffmpeg call host imports → wasm-only (like run() below).
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_image_resize_to_filesize_core::{
    plan_attempt, search_quality, target_kb_to_bytes, Fmt,
};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_BYTES: usize = 8 * 1024 * 1024;
const DEFAULT_FORMAT: &str = "jpg";

#[derive(Deserialize)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    /// Target maximum output size in KB (required).
    #[serde(default)]
    target_kb: Option<f64>,
    /// Output format: "jpg" (default) or "webp".
    #[serde(default)]
    format: Option<String>,
    /// Optional width cap in px (0 = keep original; shrinks only).
    #[serde(default)]
    max_width: Option<u32>,
}

/// Single-source param descriptor → chat schema (and CLI + page). The drift-guard
/// test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::number("target_kb")
                .required()
                .min(1.0)
                .describe(
                    "Target maximum output size in KB, e.g. 200. The tool searches the highest \
                     encoder quality whose file is at or under this budget (1 MB = 1024 KB).",
                ),
        )
        .param(
            Param::enumv("format", ["jpg", "webp"])
                .default("jpg")
                .describe(
                    "Output format for the lossy quality search: \"jpg\" (widest support) or \
                     \"webp\" (smaller at equal quality). Default jpg.",
                ),
        )
        .param(
            Param::integer("max_width")
                .min(0.0)
                .default(0)
                .describe(
                    "Optional cap on output width in pixels (0 = keep original size). Shrinks \
                     only, never upscales. Use a smaller value when the target can't be reached \
                     at full resolution.",
                ),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

/// One-line summary for the LLM: what was produced, the achieved size + quality,
/// and whether the target was met.
fn summary(
    source: &str,
    target_kb: f64,
    output_size: usize,
    fmt: Fmt,
    quality: u8,
    fit: bool,
) -> String {
    let out_kb = output_size as f64 / 1024.0;
    if fit {
        format!(
            "resized {source} to {out_kb:.1} KB ({} at quality {quality}), under the {target_kb:.0} KB target",
            fmt.mime()
        )
    } else {
        format!(
            "could not reach the {target_kb:.0} KB target for {source}; smallest was {out_kb:.1} KB \
             ({} at quality {quality}) — try a smaller max_width",
            fmt.mime()
        )
    }
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/image-resize-to-filesize",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Compress an image to a target file size in KB by searching the best JPEG/WebP quality",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Compress an image to a target file size in KB. Provide either url (HTTP/HTTPS) or ref (id from a prior image tool call), a target_kb budget, and optionally format (jpg/webp) and max_width. The tool binary-searches the encoder quality for the highest quality whose file is at or under target_kb, optionally shrinking the width first. Output is JPEG or WebP (PNG has no quality knob to search).",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    use std::collections::HashMap;

    // 1. Validate args.
    let args: Args = serde_json::from_slice(&body).map_err(|e| {
        SkillError::InvalidArgs(format!("invalid image-resize-to-filesize args: {e}"))
    })?;
    let target_kb = args
        .target_kb
        .ok_or_else(|| SkillError::InvalidArgs("target_kb is required (target size in KB)".into()))?;
    let target_bytes = target_kb_to_bytes(target_kb).map_err(SkillError::InvalidArgs)?;
    let fmt = Fmt::from_arg(args.format.as_deref().unwrap_or(DEFAULT_FORMAT))
        .map_err(SkillError::InvalidArgs)?;
    let max_width = args.max_width.unwrap_or(0);

    // 2. Resolve source once (URL fetch or attachment).
    let (input_bytes, mime, in_name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_BYTES)?;
    let in_ext = mime_to_ext(&mime)
        .ok_or_else(|| SkillError::InvalidArgs(format!("unsupported input mime: {mime}")))?;
    let in_ff = format!("in.{in_ext}");

    // 3. Binary-search quality, caching the encoded bytes per probed quality so
    //    the winning attempt costs no extra encode pass.
    let mut cache: HashMap<u8, Vec<u8>> = HashMap::new();
    let outcome = search_quality(target_bytes, |q| {
        let (argv, out_name) = plan_attempt(fmt, q, max_width, &in_ff);
        let out = dispatch_ffmpeg(argv, in_ff.clone(), input_bytes.clone(), out_name)?;
        let len = out.len();
        cache.insert(q, out);
        Ok::<usize, SkillError>(len)
    })?;
    let output = cache
        .remove(&outcome.quality)
        .expect("the chosen quality was probed and cached");

    // 4. Envelope (output mime/ext follow the chosen format, not the input).
    let out_display = filename_with_suffix(&in_name, "-target", fmt.ext());
    let for_llm = summary(&in_name, target_kb, output.len(), fmt, outcome.quality, outcome.fit);
    build_media_envelope(&output, fmt.mime(), out_display, for_llm, MAX_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift-guard: the descriptor-derived chat schema must match the authored
    /// schema, so the LLM sees no drift. Regenerate this literal (never hand-patch
    /// the old one) whenever `descriptor()` changes.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":       { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":       { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "target_kb": { "type": "number", "minimum": 1, "description": "Target maximum output size in KB, e.g. 200. The tool searches the highest encoder quality whose file is at or under this budget (1 MB = 1024 KB)." },
                    "format":    { "type": "string", "enum": ["jpg", "webp"], "default": "jpg", "description": "Output format for the lossy quality search: \"jpg\" (widest support) or \"webp\" (smaller at equal quality). Default jpg." },
                    "max_width": { "type": "integer", "minimum": 0, "default": 0, "description": "Optional cap on output width in pixels (0 = keep original size). Shrinks only, never upscales. Use a smaller value when the target can't be reached at full resolution." }
                },
                "additionalProperties": false,
                "required": ["target_kb"],
                "oneOf": [
                    { "required": ["url"] },
                    { "required": ["ref"] }
                ]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn summary_reports_fit_and_size() {
        let s = summary("cat.png", 200.0, 150 * 1024, Fmt::Jpeg, 62, true);
        assert!(s.contains("cat.png"));
        assert!(s.contains("150.0 KB"));
        assert!(s.contains("quality 62"));
        assert!(s.contains("200 KB target"));
    }

    #[test]
    fn summary_flags_unreached_target() {
        let s = summary("big.png", 10.0, 42 * 1024, Fmt::Webp, 5, false);
        assert!(s.contains("could not reach"));
        assert!(s.contains("max_width"));
    }
}
