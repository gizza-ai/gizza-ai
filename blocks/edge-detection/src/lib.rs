//! gizza-ai/edge-detection — detect the edges in an image with Canny or Sobel
//! via ffmpeg, on the shared tool abstraction.
//!
//! Input::Image emits a url⊕ref oneOf; run() uses resolve_source → core::plan →
//! dispatch_ffmpeg → build_media_envelope. `method` picks the detector (canny =
//! thresholded 1-px wires, sobel = raw gradient magnitude, colormix = edges
//! painted over the original colors), `low`/`high` are the Canny hysteresis
//! thresholds as 0–1 fractions, `blur` is an optional Gaussian pre-pass that
//! kills noise-edges, `invert` flips to black-on-white line art and `format`
//! chooses the output encoding. The chat schema is derived from `descriptor()`
//! (single source — shared across chat + CLI + page) and the drift-guard test
//! below proves it matches the authored schema.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{
    build_media_envelope, mime_to_ext, AssetKind, Input, Param, SkillError, SourceFields,
    ToolDescriptor,
};
// resolve_source / dispatch_ffmpeg call host imports → wasm-only (like run() below).
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_edge_detection_core::{
    format_ext, method_name, parse_format, parse_method, DEFAULT_BLUR, DEFAULT_FORMAT, DEFAULT_HIGH,
    DEFAULT_LOW, DEFAULT_METHOD, FORMATS, MAX_BLUR, METHODS,
};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_BYTES: usize = 8 * 1024 * 1024;

#[derive(Deserialize)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    method: Option<String>,
    #[serde(default)]
    low: Option<f64>,
    #[serde(default)]
    high: Option<f64>,
    #[serde(default)]
    blur: Option<f64>,
    #[serde(default)]
    invert: Option<bool>,
    #[serde(default)]
    format: Option<String>,
}

fn descriptor() -> ToolDescriptor {
    // Input::Image → url⊕ref oneOf. The thresholds are 0–1 fractions of full
    // brightness (ffmpeg's edgedetect scale), not 0–255, so they are resolution
    // and bit-depth independent.
    ToolDescriptor::new(Input::Image)
        .param(
            Param::enumv("method", METHODS)
                .default(DEFAULT_METHOD)
                .describe(
                    "Edge detector: canny (default) applies Gaussian smoothing, Sobel gradients, \
                     thinning and hysteresis for clean 1-pixel white edges on black; sobel is the \
                     raw gradient magnitude (soft grey edges, no thresholds, better on blurry \
                     photos); colormix keeps the original colors and paints the edges over them \
                     for an inked/cartoon look.",
                ),
        )
        .param(
            Param::number("low")
                .min(0.0)
                .max(1.0)
                .default(DEFAULT_LOW)
                .describe(
                    "Canny lower hysteresis threshold as a fraction of full brightness, 0-1 \
                     (default 0.078, i.e. 20/255). Weak edges above this are kept only when they \
                     touch a strong edge. Lower it to 0.03 to keep faint detail; raise it to 0.2 \
                     for a cleaner outline. Ignored when method=sobel.",
                ),
        )
        .param(
            Param::number("high")
                .min(0.0)
                .max(1.0)
                .default(DEFAULT_HIGH)
                .describe(
                    "Canny upper hysteresis threshold as a fraction of full brightness, 0-1 \
                     (default 0.196, i.e. 50/255). Any gradient above this starts an edge. Must \
                     be >= low; a common ratio is high = 2-3x low. Ignored when method=sobel.",
                ),
        )
        .param(
            Param::number("blur")
                .min(0.0)
                .max(MAX_BLUR)
                .default(DEFAULT_BLUR)
                .describe(
                    "Gaussian noise-reduction radius applied before detection, in pixels of \
                     sigma, 0-10 (default 0 = off). Try 1-2 on grainy phone photos or JPEG \
                     artifacts to stop noise being detected as edges; large values erase fine \
                     detail.",
                ),
        )
        .param(Param::boolean("invert").default(false).describe(
            "false (default) returns white edges on black, the standard edge map. true returns \
             black lines on white — the look you want for printing, coloring pages, laser \
             engraving or vector tracing.",
        ))
        .param(Param::enumv("format", FORMATS).default(DEFAULT_FORMAT).describe(
            "Output image format: png (default, lossless — best for high-contrast line art), jpg \
             (smaller, but rings around thin lines) or webp. Animated inputs keep only their \
             first frame.",
        ))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/edge-detection",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Detect the edges in an image with Canny or Sobel and return the edge map.",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Detect the edges in an image and return the edge map. Provide either url (HTTP/HTTPS) or ref (id from a prior image tool call); optional method canny|sobel|colormix (default canny), low/high Canny hysteresis thresholds as 0-1 fractions (defaults 0.078/0.196, high >= low), blur 0-10 pixels of Gaussian pre-smoothing to suppress noise (default 0), invert true for black lines on white (default false), and format png|jpg|webp (default png).",
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
    let args: Args = serde_json::from_slice(&body)
        .map_err(|e| SkillError::InvalidArgs(format!("invalid edge-detection args: {e}")))?;
    let method = parse_method(args.method.as_deref()).map_err(SkillError::InvalidArgs)?;
    let format = parse_format(args.format.as_deref()).map_err(SkillError::InvalidArgs)?;
    let low = args.low.unwrap_or(DEFAULT_LOW);
    let high = args.high.unwrap_or(DEFAULT_HIGH);
    let blur = args.blur.unwrap_or(DEFAULT_BLUR);
    let invert = args.invert.unwrap_or(false);
    let (bytes, mime, in_name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_BYTES)?;
    let ext = mime_to_ext(&mime)
        .ok_or_else(|| SkillError::InvalidArgs(format!("unsupported mime: {mime}")))?;
    let in_path = format!("in.{ext}");
    let (argv, out_name) = gizza_ai_edge_detection_core::plan(
        &in_path, method, low, high, blur, invert, format,
    )
    .map_err(SkillError::InvalidArgs)?;
    let output = dispatch_ffmpeg(argv, in_path, bytes, out_name)?;
    let out_ext = format_ext(format);
    let out_mime = match out_ext {
        "jpg" => "image/jpeg",
        "webp" => "image/webp",
        _ => "image/png",
    };
    let out_display = rename_ext(&in_name, out_ext);
    let mut for_llm = format!(
        "detected edges in {in_name} with the {} method",
        method_name(method)
    );
    if blur > 0.0 {
        for_llm.push_str(&format!(" (blur sigma {blur})"));
    }
    if invert {
        for_llm.push_str(" as black lines on white");
    }
    for_llm.push_str(&format!(" and saved the edge map as {out_display}"));
    build_media_envelope(&output, out_mime, out_display, for_llm, MAX_BYTES)
}

/// `photo.jpeg` + `png` → `photo.png` (append when there is no extension).
#[cfg(target_arch = "wasm32")]
fn rename_ext(name: &str, ext: &str) -> String {
    match name.rsplit_once('.') {
        Some((stem, _)) if !stem.is_empty() => format!("{stem}.{ext}"),
        _ => format!("{name}.{ext}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift-guard: the descriptor-derived chat schema must match the authored
    /// schema below, so the LLM-facing tool definition never silently changes.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "method": {
                        "type": "string",
                        "enum": ["canny", "sobel", "colormix"],
                        "default": "canny",
                        "description": "Edge detector: canny (default) applies Gaussian smoothing, Sobel gradients, thinning and hysteresis for clean 1-pixel white edges on black; sobel is the raw gradient magnitude (soft grey edges, no thresholds, better on blurry photos); colormix keeps the original colors and paints the edges over them for an inked/cartoon look."
                    },
                    "low": {
                        "type": "number",
                        "minimum": 0,
                        "maximum": 1,
                        "default": 0.0784313725490196,
                        "description": "Canny lower hysteresis threshold as a fraction of full brightness, 0-1 (default 0.078, i.e. 20/255). Weak edges above this are kept only when they touch a strong edge. Lower it to 0.03 to keep faint detail; raise it to 0.2 for a cleaner outline. Ignored when method=sobel."
                    },
                    "high": {
                        "type": "number",
                        "minimum": 0,
                        "maximum": 1,
                        "default": 0.196078431372549,
                        "description": "Canny upper hysteresis threshold as a fraction of full brightness, 0-1 (default 0.196, i.e. 50/255). Any gradient above this starts an edge. Must be >= low; a common ratio is high = 2-3x low. Ignored when method=sobel."
                    },
                    "blur": {
                        "type": "number",
                        "minimum": 0,
                        "maximum": 10,
                        "default": 0.0,
                        "description": "Gaussian noise-reduction radius applied before detection, in pixels of sigma, 0-10 (default 0 = off). Try 1-2 on grainy phone photos or JPEG artifacts to stop noise being detected as edges; large values erase fine detail."
                    },
                    "invert": {
                        "type": "boolean",
                        "default": false,
                        "description": "false (default) returns white edges on black, the standard edge map. true returns black lines on white — the look you want for printing, coloring pages, laser engraving or vector tracing."
                    },
                    "format": {
                        "type": "string",
                        "enum": ["png", "jpg", "webp"],
                        "default": "png",
                        "description": "Output image format: png (default, lossless — best for high-contrast line art), jpg (smaller, but rings around thin lines) or webp. Animated inputs keep only their first frame."
                    }
                },
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }],
                "additionalProperties": false
            }"#,
        )
        .expect("authored schema is valid JSON");
        let derived: serde_json::Value =
            serde_json::from_str(&schema_json()).expect("derived schema is valid JSON");
        assert_eq!(derived, authored);
    }
}
