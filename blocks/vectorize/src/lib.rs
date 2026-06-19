//! gizza-ai/vectorize — trace a raster image into a clean scalable SVG.
//!
//! Pipeline: parse `{url|ref}` (+ optional `mode`/`color_precision`/
//! `filter_speckle`) → resolve image bytes via `block-utils`
//! (`resolve_source` + `AssetKind::Image`, the same path `image-*` blocks use)
//! → hand the bytes to `gizza-ai-vectorize-core::vectorize` (pure vtracer
//! trace) → wrap the SVG text as an `image/svg+xml` data-URL
//! `Envelope{_for_llm, _for_ui}` (via `build_media_envelope`) so the chat UI
//! renders it inline and offers a `*.svg` download.
//!
//! The chat schema is derived from `descriptor()` (single source — shared
//! shape across chat + CLI); the drift-guard test below proves the derived
//! schema matches the pre-retrofit authored one. See
//! docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.
//!
//! No host calls beyond the `url`-fetch (`wafer-run/network`) / attachment
//! lookup; the trace itself runs entirely inside the WASM sandbox. No page —
//! chat + CLI surface only (binary file in, SVG text out doesn't fit the
//! page system's text→text / file→media shapes).

// The #[wafer_block] macro emits the impl gated to wasm32 (the macro generates
// a native registration call that requires ::new()). The supporting imports,
// constants, and the Args type are only used inside that wasm32-gated impl, so
// they appear "unused" when running native unit tests. `descriptor()` /
// `schema_json()` stay native-compilable so the drift-guard exercises them.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{build_media_envelope, resolve_source, AssetKind};
use gizza_ai_block_utils::{
    replace_extension, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_vectorize_core::{vectorize as trace_svg, Mode, Options};
use serde::Deserialize;
use wafer_sdk::*;

/// Cap on the raster we'll fetch/trace. visioncortex clustering is O(pixels);
/// 4 MiB comfortably holds any logo / icon / sketch a user would vectorize.
const MAX_BYTES: usize = 4 * 1024 * 1024;
/// SVG output cap. Traced SVGs of a logo/icon stay well under this; the guard
/// just keeps a pathological trace from blowing up the chat history.
const MAX_OUTPUT_BYTES: usize = 8 * 1024 * 1024;

#[derive(Deserialize)]
struct Args {
    /// Exactly one of `url` / `ref` (validated at deserialize time).
    #[serde(flatten)]
    source: SourceFields,
    /// `"color"` (default) or `"bw"`.
    #[serde(default)]
    mode: Option<String>,
    /// Color-channel precision `1..=8` (color mode only). Defaults to 6.
    #[serde(default)]
    color_precision: Option<i32>,
    /// Speckle filter strength `0..=128`. Defaults to 4.
    #[serde(default)]
    filter_speckle: Option<usize>,
}

/// Single-source param descriptor → chat schema (and CLI). The drift-guard test
/// below proves the derived schema matches the pre-retrofit authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(Param::enumv("mode", ["color", "bw"]).describe(
            "Trace mode: 'color' (default, multi-color logos/icons) or 'bw' \
                 (black-and-white sketches/line art).",
        ))
        .param(
            Param::integer("color_precision")
                .min(1.0)
                .max(8.0)
                .describe(
                    "Color-channel precision in color mode, 1-8 (default 6). \
                     Higher = more colors/detail, larger SVG.",
                ),
        )
        .param(
            Param::integer("filter_speckle")
                .min(0.0)
                .max(128.0)
                .describe(
                    "Discard speckles smaller than this (default 4). Higher = cleaner output.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Vectorize;

// The #[wafer_block] macro emits a native registration call requiring ::new()
// on the impl; skill-style impls don't have one. Gate the struct + impl to
// wasm32 so unit tests can still compile natively.
#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/vectorize",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Trace a raster image into a scalable SVG",
    requires = ["wafer-run/network"],
    skill(
        description = "Trace a raster image (logo, sketch, icon) into a clean, scalable SVG. Provide the image as a `url` (http/https) or a `ref` to an uploaded attachment. Returns the SVG for inline display and download. Use this when the user asks to vectorize / convert an image to SVG / make a logo scalable.",
        parameters = schema_json()
    ),
)]
impl Vectorize {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("vectorize")?;

    let opts = build_options(&args)?;

    // Resolve the raster bytes via the shared image-class loader (URL → network
    // GET with content-type/size guards; ref → attachment lookup).
    let (bytes, _mime, filename) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_BYTES)?;

    let in_len = bytes.len();
    let svg = trace_svg(&bytes, opts).map_err(SkillError::InvalidArgs)?;

    let svg_bytes = svg.into_bytes();
    let out_len = svg_bytes.len();
    let out_name = replace_extension(&filename, "svg");
    let for_llm = format!("vectorized {in_len}-byte image into a {out_len}-byte SVG ({out_name})");

    build_media_envelope(
        &svg_bytes,
        "image/svg+xml",
        out_name,
        for_llm,
        MAX_OUTPUT_BYTES,
    )
}

/// Map the optional `mode` / `color_precision` / `filter_speckle` args onto a
/// validated core `Options`. Host-side range checks (`1..=8`, `0..=128`) live
/// in core; this only resolves the string `mode` so a bad value surfaces as an
/// `InvalidArgs` before the (heavier) fetch.
#[cfg(target_arch = "wasm32")]
fn build_options(args: &Args) -> Result<Options, SkillError> {
    let mut opts = Options::default();
    if let Some(m) = &args.mode {
        opts.mode = Mode::parse(m).map_err(SkillError::InvalidArgs)?;
    }
    if let Some(cp) = args.color_precision {
        opts.color_precision = cp;
    }
    if let Some(fs) = args.filter_speckle {
        opts.filter_speckle = fs;
    }
    Ok(opts)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Migration safety: the descriptor-derived chat schema must match the
    /// pre-retrofit authored schema, so the LLM sees no drift. The authored
    /// vectorize schema already carried `additionalProperties: false` (kept).
    /// The only intentional change is the `url`/`ref` property descriptions:
    /// they are now centralized in `to_schema_json`, so the expected JSON below
    /// uses that shared wording. The exclusive `oneOf` is also added uniformly
    /// by `to_schema_json` (the authored schema relied on the deserializer's
    /// "exactly one of url|ref" check, which the descriptor now mirrors in the
    /// schema). Tool-specific `mode`/`color_precision`/`filter_speckle`
    /// descriptions are preserved verbatim.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":  { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":  { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "mode": { "type": "string", "enum": ["color", "bw"], "description": "Trace mode: 'color' (default, multi-color logos/icons) or 'bw' (black-and-white sketches/line art)." },
                    "color_precision": { "type": "integer", "minimum": 1, "maximum": 8, "description": "Color-channel precision in color mode, 1-8 (default 6). Higher = more colors/detail, larger SVG." },
                    "filter_speckle":  { "type": "integer", "minimum": 0, "maximum": 128, "description": "Discard speckles smaller than this (default 4). Higher = cleaner output." }
                },
                "additionalProperties": false,
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
}
