//! gizza-ai/vectorize — trace a raster image into a clean scalable SVG.
//!
//! Pipeline: parse `{url|ref}` (+ optional `mode`/`color_precision`/
//! `filter_speckle`) → fetch image bytes via `block-utils` (`AssetKind::Image`,
//! the same path `image-*` blocks use) → hand the bytes to
//! `gizza-ai-vectorize-core::vectorize` (pure vtracer trace) → wrap the SVG
//! text as an `image/svg+xml` data-URL `Envelope{_for_llm, _for_ui}` so the
//! chat UI renders it inline and offers a `*.svg` download.
//!
//! No host calls beyond the `url`-fetch (`wafer-run/network`) / attachment
//! lookup; the trace itself runs entirely inside the WASM sandbox. No page —
//! chat + CLI surface only (binary file in, SVG text out doesn't fit the
//! page system's text→text / file→media shapes).

// The #[wafer_block] macro emits wasm-only registration; supporting imports
// and the Args type are only used inside that impl.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use gizza_ai_block_utils::SourceFields;
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{
    fetch_from_url, load_from_attachment, replace_extension, AssetKind, Envelope, ForUi,
    SkillError, SkillResultExt, Source,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_vectorize_core::{vectorize as trace_svg, Mode, Options};
use serde::Deserialize;
#[cfg(target_arch = "wasm32")]
use wafer_sdk::*;

/// Cap on the raster we'll fetch/trace. visioncortex clustering is O(pixels);
/// 4 MiB comfortably holds any logo / icon / sketch a user would vectorize.
const MAX_BYTES: usize = 4 * 1024 * 1024;

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

#[cfg(target_arch = "wasm32")]
struct Vectorize;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/vectorize",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Trace a raster image into a scalable SVG",
    requires = ["wafer-run/network"],
    skill(
        description = "Trace a raster image (logo, sketch, icon) into a clean, scalable SVG. Provide the image as a `url` (http/https) or a `ref` to an uploaded attachment. Returns the SVG for inline display and download. Use this when the user asks to vectorize / convert an image to SVG / make a logo scalable.",
        parameters = r#"{
            "type": "object",
            "properties": {
                "url":  { "type": "string", "description": "HTTP/HTTPS URL of the raster image (PNG/JPEG/WebP/GIF) to vectorize." },
                "ref":  { "type": "string", "description": "Reference id of an uploaded image attachment to vectorize (alternative to url)." },
                "mode": { "type": "string", "enum": ["color", "bw"], "description": "Trace mode: 'color' (default, multi-color logos/icons) or 'bw' (black-and-white sketches/line art)." },
                "color_precision": { "type": "integer", "minimum": 1, "maximum": 8, "description": "Color-channel precision in color mode, 1-8 (default 6). Higher = more colors/detail, larger SVG." },
                "filter_speckle":  { "type": "integer", "minimum": 0, "maximum": 128, "description": "Discard speckles smaller than this (default 4). Higher = cleaner output." }
            },
            "additionalProperties": false
        }"#
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

    // Fetch the raster bytes via the shared image-class loader (URL → network
    // GET with content-type/size guards; ref → attachment lookup).
    let (bytes, _mime, filename) = match args.source.into_inner() {
        Source::Url(url) => fetch_from_url(&url, AssetKind::Image, MAX_BYTES)?,
        Source::Ref(id) => load_from_attachment(&id, AssetKind::Image, MAX_BYTES)?,
    };

    let in_len = bytes.len();
    let svg = trace_svg(&bytes, opts).map_err(SkillError::InvalidArgs)?;

    let svg_bytes = svg.into_bytes();
    let out_len = svg_bytes.len();
    let encoded = B64.encode(&svg_bytes);
    let data_url = format!("data:image/svg+xml;base64,{encoded}");
    let out_name = replace_extension(&filename, "svg");

    let env = Envelope {
        for_llm: format!("vectorized {in_len}-byte image into a {out_len}-byte SVG ({out_name})"),
        for_ui: ForUi {
            data_url,
            mime: "image/svg+xml".to_string(),
            filename: out_name,
        },
    };
    serde_json::to_vec(&env).map_err(|e| SkillError::Serialize(format!("serialize envelope: {e}")))
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
