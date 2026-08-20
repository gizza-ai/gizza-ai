//! gizza-ai/image-median-denoise — remove salt-and-pepper, dust and speckle
//! noise from an image with a median filter, on the shared tool abstraction.
//!
//! Input::Image emits a url⊕ref oneOf; run() uses resolve_source → core::plan →
//! dispatch_ffmpeg → build_media_envelope. Users pick a friendly `radius` (the
//! window is 2·radius+1), which specks to `target` (mapped onto the ffmpeg
//! median filter's percentile), which `channels` to filter (mapped onto its
//! planes bitmask), how many `passes`, and the output `format`/`quality`. The
//! chat schema is derived from `descriptor()` (the single source shared across
//! chat + CLI + page); the drift-guard test below proves it matches the
//! authored schema.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{
    build_media_envelope, mime_to_ext, AssetKind, Input, Param, SkillError, SourceFields,
    ToolDescriptor,
};
// resolve_source / dispatch_ffmpeg call host imports → wasm-only (like run() below).
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_image_median_denoise_core::{
    channels_name, parse_channels, parse_format, parse_target, resolve_passes, resolve_radius,
    target_name, window_size, OutFormat, CHANNELS, DEFAULT_CHANNELS, DEFAULT_FORMAT,
    DEFAULT_PASSES, DEFAULT_QUALITY, DEFAULT_RADIUS, DEFAULT_TARGET, FORMATS, MAX_PASSES,
    MAX_RADIUS, TARGETS,
};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_BYTES: usize = 8 * 1024 * 1024;

#[derive(Deserialize)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    radius: Option<f64>,
    #[serde(default)]
    target: Option<String>,
    #[serde(default)]
    channels: Option<String>,
    #[serde(default)]
    passes: Option<f64>,
    #[serde(default)]
    format: Option<String>,
    #[serde(default)]
    quality: Option<f64>,
    #[serde(default)]
    strip_metadata: Option<bool>,
}

fn descriptor() -> ToolDescriptor {
    // Input::Image → url⊕ref oneOf. Every knob is expressed in user terms
    // (window radius, which specks, which channels) and mapped onto the ffmpeg
    // median filter inside core, so raw percentile/planes numbers never leak.
    ToolDescriptor::new(Input::Image)
        .param(
            Param::number("radius")
                .min(1.0)
                .max(MAX_RADIUS)
                .default(DEFAULT_RADIUS)
                .describe(
                    "Median window radius in pixels, 1-20. The window is 2*radius+1 square: 1 \
                     (the default) is a 3x3 window that wipes single-pixel salt-and-pepper dots \
                     while keeping detail, 2-3 clears scanner dust and speckle, 5+ smooths heavy \
                     noise but starts looking painted. Example: 2 for a noisy phone photo.",
                ),
        )
        .param(
            Param::enumv("target", TARGETS)
                .default(DEFAULT_TARGET)
                .describe(
                    "Which specks to remove. both (default) is the true median and clears bright \
                     and dark specks alike; bright removes white dust/salt harder (and darkens \
                     the image slightly); dark removes black specks/pepper on light paper.",
                ),
        )
        .param(
            Param::enumv("channels", CHANNELS)
                .default(DEFAULT_CHANNELS)
                .describe(
                    "Which channels to filter. all (default) filters the whole image; luma \
                     smooths brightness noise but keeps color detail; chroma removes blotchy \
                     color noise while leaving luminance detail sharp (best for high-ISO photos).",
                ),
        )
        .param(
            Param::number("passes")
                .min(1.0)
                .max(MAX_PASSES)
                .default(DEFAULT_PASSES)
                .describe(
                    "How many times to run the filter, 1-3 (default 1). Two mild passes at \
                     radius 1 remove dense noise more gently than one pass at radius 3.",
                ),
        )
        .param(
            Param::enumv("format", FORMATS)
                .default(DEFAULT_FORMAT)
                .describe(
                    "Output format: keep (default) preserves the input format and any animation; \
                     png is lossless (best for scans and line art); jpg and webp are smaller. \
                     Converting an animated GIF keeps only its first frame.",
                ),
        )
        .param(
            Param::number("quality")
                .min(1.0)
                .max(100.0)
                .default(DEFAULT_QUALITY)
                .describe(
                    "Encoder quality 1-100 for jpg and webp output (default 92). Ignored for png, \
                     which is lossless. Example: 80 for a smaller web-sized jpg.",
                ),
        )
        .param(
            Param::boolean("strip_metadata")
                .default(false)
                .describe(
                    "false (default) carries the input's metadata (EXIF, camera info, comments) \
                     into the output; true drops all of it, e.g. before publishing a scan.",
                ),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/image-median-denoise",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Remove salt-and-pepper, dust and speckle noise from an image with an edge-preserving median filter.",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Remove salt-and-pepper, dust and speckle noise from an image with a median filter, which keeps edges sharp instead of blurring them. Provide either url (HTTP/HTTPS) or ref (id from a prior image tool call); optional radius 1-20 (default 1, window 2*radius+1), target both|bright|dark (default both), channels all|luma|chroma (default all), passes 1-3 (default 1), format keep|png|jpg|webp (default keep), quality 1-100 for jpg/webp (default 92), and strip_metadata (default false).",
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
        .map_err(|e| SkillError::InvalidArgs(format!("invalid image-median-denoise args: {e}")))?;
    let radius_in = args.radius.unwrap_or(DEFAULT_RADIUS);
    let passes_in = args.passes.unwrap_or(DEFAULT_PASSES);
    let quality = args.quality.unwrap_or(DEFAULT_QUALITY);
    let target = parse_target(args.target.as_deref()).map_err(SkillError::InvalidArgs)?;
    let channels = parse_channels(args.channels.as_deref()).map_err(SkillError::InvalidArgs)?;
    let format = parse_format(args.format.as_deref()).map_err(SkillError::InvalidArgs)?;
    let strip_metadata = args.strip_metadata.unwrap_or(false);
    // Resolve here too so the summary can quote the real window/pass counts.
    let radius = resolve_radius(radius_in).map_err(SkillError::InvalidArgs)?;
    let passes = resolve_passes(passes_in).map_err(SkillError::InvalidArgs)?;

    let (bytes, mime, in_name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_BYTES)?;
    let ext = mime_to_ext(&mime)
        .ok_or_else(|| SkillError::InvalidArgs(format!("unsupported mime: {mime}")))?;
    let in_path = format!("in.{ext}");
    let (argv, out_name) = gizza_ai_image_median_denoise_core::plan(
        &in_path,
        radius_in,
        target,
        channels,
        passes_in,
        format,
        quality,
        strip_metadata,
    )
    .map_err(SkillError::InvalidArgs)?;
    let output = dispatch_ffmpeg(argv, in_path, bytes, out_name)?;

    // The envelope must describe the OUTPUT: converting changes mime + name.
    let (out_mime, out_display) = match format {
        OutFormat::Keep => (mime.clone(), in_name.clone()),
        OutFormat::Png => ("image/png".to_string(), rename_ext(&in_name, "png")),
        OutFormat::Jpg => ("image/jpeg".to_string(), rename_ext(&in_name, "jpg")),
        OutFormat::Webp => ("image/webp".to_string(), rename_ext(&in_name, "webp")),
    };
    let win = window_size(radius);
    let mut for_llm = format!(
        "median-denoised {in_name} ({win}x{win} window, target {}, channels {}",
        target_name(target),
        channels_name(channels)
    );
    if passes > 1 {
        for_llm.push_str(&format!(", {passes} passes"));
    }
    for_llm.push(')');
    if format != OutFormat::Keep {
        for_llm.push_str(&format!(" and converted it to {out_display}"));
    }
    build_media_envelope(&output, &out_mime, out_display, for_llm, MAX_BYTES)
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
                    "radius": {
                        "type": "number",
                        "minimum": 1,
                        "maximum": 20,
                        "default": 1.0,
                        "description": "Median window radius in pixels, 1-20. The window is 2*radius+1 square: 1 (the default) is a 3x3 window that wipes single-pixel salt-and-pepper dots while keeping detail, 2-3 clears scanner dust and speckle, 5+ smooths heavy noise but starts looking painted. Example: 2 for a noisy phone photo."
                    },
                    "target": {
                        "type": "string",
                        "enum": ["both", "bright", "dark"],
                        "default": "both",
                        "description": "Which specks to remove. both (default) is the true median and clears bright and dark specks alike; bright removes white dust/salt harder (and darkens the image slightly); dark removes black specks/pepper on light paper."
                    },
                    "channels": {
                        "type": "string",
                        "enum": ["all", "luma", "chroma"],
                        "default": "all",
                        "description": "Which channels to filter. all (default) filters the whole image; luma smooths brightness noise but keeps color detail; chroma removes blotchy color noise while leaving luminance detail sharp (best for high-ISO photos)."
                    },
                    "passes": {
                        "type": "number",
                        "minimum": 1,
                        "maximum": 3,
                        "default": 1.0,
                        "description": "How many times to run the filter, 1-3 (default 1). Two mild passes at radius 1 remove dense noise more gently than one pass at radius 3."
                    },
                    "format": {
                        "type": "string",
                        "enum": ["keep", "png", "jpg", "webp"],
                        "default": "keep",
                        "description": "Output format: keep (default) preserves the input format and any animation; png is lossless (best for scans and line art); jpg and webp are smaller. Converting an animated GIF keeps only its first frame."
                    },
                    "quality": {
                        "type": "number",
                        "minimum": 1,
                        "maximum": 100,
                        "default": 92.0,
                        "description": "Encoder quality 1-100 for jpg and webp output (default 92). Ignored for png, which is lossless. Example: 80 for a smaller web-sized jpg."
                    },
                    "strip_metadata": {
                        "type": "boolean",
                        "default": false,
                        "description": "false (default) carries the input's metadata (EXIF, camera info, comments) into the output; true drops all of it, e.g. before publishing a scan."
                    }
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

    #[test]
    fn descriptor_exposes_every_enum_choice() {
        let json = schema_json();
        for name in TARGETS.iter().chain(CHANNELS.iter()).chain(FORMATS.iter()) {
            assert!(json.contains(name), "schema enum must list {name}");
        }
    }
}
