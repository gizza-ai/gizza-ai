//! gizza-ai/image-vignette — add a soft darkened (or lightened) vignette
//! around the edges of an image via ffmpeg, on the shared tool abstraction.
//!
//! Input::Image emits a url⊕ref oneOf; run() uses resolve_source → core::plan →
//! dispatch_ffmpeg → build_media_envelope. Users pass a friendly `strength`
//! 0–100; the core maps it onto the ffmpeg `vignette` filter's angle (radians,
//! max PI/2) — raw radians are never exposed. A non-black `color` (darken mode)
//! switches the core to a masked-merge tint chain, and `format` can convert the
//! output to png/jpg/webp. The chat schema is derived from `descriptor()`
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
use gizza_ai_image_vignette_core::{
    mode_name, parse_color, parse_format, parse_mode, OutFormat, DEFAULT_CENTER_PCT,
    DEFAULT_COLOR, DEFAULT_FORMAT, DEFAULT_MODE, DEFAULT_STRENGTH, FORMATS, MODES,
};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_BYTES: usize = 8 * 1024 * 1024;

#[derive(Deserialize)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    strength: Option<f64>,
    #[serde(default)]
    mode: Option<String>,
    #[serde(default)]
    color: Option<String>,
    #[serde(default)]
    center_x: Option<f64>,
    #[serde(default)]
    center_y: Option<f64>,
    #[serde(default)]
    format: Option<String>,
}

fn descriptor() -> ToolDescriptor {
    // Input::Image → url⊕ref oneOf. `strength` is the friendly 0–100 knob the
    // core maps onto the ffmpeg vignette angle; `mode` picks dark vs light
    // edges; `color` tints darken-mode edges (black = the classic plain path);
    // the center is a percentage so it works for any resolution; `format`
    // optionally converts the output.
    ToolDescriptor::new(Input::Image)
        .param(
            Param::number("strength")
                .min(0.0)
                .max(100.0)
                .default(DEFAULT_STRENGTH)
                .describe(
                    "Vignette strength, 0-100. 0 leaves the image unchanged, 40 (the default) is \
                     a classic soft vignette, 100 fades the corners fully to black (or white in \
                     lighten mode). Example: 80 for a dramatic look.",
                ),
        )
        .param(Param::enumv("mode", MODES).default(DEFAULT_MODE).describe(
            "darken (default) darkens the edges like a classic lens vignette; lighten brightens \
             them instead for a faded, hazy border.",
        ))
        .param(
            Param::string("color")
                .default(DEFAULT_COLOR)
                .describe(
                    "Vignette color for darken mode: a name (black, white, gray, sepia, navy, \
                     red, …) or hex like #1A2B3C / #A52. black (the default) is the classic \
                     look; try sepia for a vintage fade. Not combinable with mode=lighten.",
                ),
        )
        .param(
            Param::number("center_x")
                .min(0.0)
                .max(100.0)
                .default(DEFAULT_CENTER_PCT)
                .describe(
                    "Horizontal center of the vignette as a percent of the image width: 0 = left \
                     edge, 50 (default) = middle, 100 = right edge.",
                ),
        )
        .param(
            Param::number("center_y")
                .min(0.0)
                .max(100.0)
                .default(DEFAULT_CENTER_PCT)
                .describe(
                    "Vertical center of the vignette as a percent of the image height: 0 = top \
                     edge, 50 (default) = middle, 100 = bottom edge.",
                ),
        )
        .param(Param::enumv("format", FORMATS).default(DEFAULT_FORMAT).describe(
            "Output format: keep (default) preserves the input format (and any animation); \
             png, jpg or webp convert — an animated GIF keeps only its first frame when \
             converting.",
        ))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/image-vignette",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Add a soft darkened, lightened, or colored vignette with adjustable strength around an image's edges.",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Add a soft darkened, lightened, or colored vignette around the edges of an image. Provide either url (HTTP/HTTPS) or ref (id from a prior image tool call); optional strength 0-100 (default 40), mode darken|lighten (default darken), color (name or hex, darken mode only, default black), center_x/center_y as percent of the image size (default 50/50, the middle), and format keep|png|jpg|webp (default keep).",
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
        .map_err(|e| SkillError::InvalidArgs(format!("invalid image-vignette args: {e}")))?;
    let strength = args.strength.unwrap_or(DEFAULT_STRENGTH);
    let mode = parse_mode(args.mode.as_deref()).map_err(SkillError::InvalidArgs)?;
    let color = parse_color(args.color.as_deref()).map_err(SkillError::InvalidArgs)?;
    let center_x = args.center_x.unwrap_or(DEFAULT_CENTER_PCT);
    let center_y = args.center_y.unwrap_or(DEFAULT_CENTER_PCT);
    let format = parse_format(args.format.as_deref()).map_err(SkillError::InvalidArgs)?;
    let (bytes, mime, in_name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_BYTES)?;
    let ext = mime_to_ext(&mime)
        .ok_or_else(|| SkillError::InvalidArgs(format!("unsupported mime: {mime}")))?;
    let in_path = format!("in.{ext}");
    let (argv, out_name) = gizza_ai_image_vignette_core::plan(
        &in_path, strength, mode, center_x, center_y, color, format,
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
    let colored = color != (0, 0, 0);
    let mut for_llm = format!(
        "applied a {} vignette (strength {strength}",
        mode_name(mode)
    );
    if colored {
        let (r, g, b) = color;
        for_llm.push_str(&format!(", color #{r:02X}{g:02X}{b:02X}"));
    }
    for_llm.push_str(&format!(") to {in_name}"));
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
                    "strength": {
                        "type": "number",
                        "minimum": 0,
                        "maximum": 100,
                        "default": 40.0,
                        "description": "Vignette strength, 0-100. 0 leaves the image unchanged, 40 (the default) is a classic soft vignette, 100 fades the corners fully to black (or white in lighten mode). Example: 80 for a dramatic look."
                    },
                    "mode": {
                        "type": "string",
                        "enum": ["darken", "lighten"],
                        "default": "darken",
                        "description": "darken (default) darkens the edges like a classic lens vignette; lighten brightens them instead for a faded, hazy border."
                    },
                    "color": {
                        "type": "string",
                        "default": "black",
                        "description": "Vignette color for darken mode: a name (black, white, gray, sepia, navy, red, …) or hex like #1A2B3C / #A52. black (the default) is the classic look; try sepia for a vintage fade. Not combinable with mode=lighten."
                    },
                    "center_x": {
                        "type": "number",
                        "minimum": 0,
                        "maximum": 100,
                        "default": 50.0,
                        "description": "Horizontal center of the vignette as a percent of the image width: 0 = left edge, 50 (default) = middle, 100 = right edge."
                    },
                    "center_y": {
                        "type": "number",
                        "minimum": 0,
                        "maximum": 100,
                        "default": 50.0,
                        "description": "Vertical center of the vignette as a percent of the image height: 0 = top edge, 50 (default) = middle, 100 = bottom edge."
                    },
                    "format": {
                        "type": "string",
                        "enum": ["keep", "png", "jpg", "webp"],
                        "default": "keep",
                        "description": "Output format: keep (default) preserves the input format (and any animation); png, jpg or webp convert — an animated GIF keeps only its first frame when converting."
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
    fn descriptor_exposes_every_mode() {
        let json = schema_json();
        for name in MODES {
            assert!(json.contains(name), "schema enum must list {name}");
        }
    }

    #[test]
    fn descriptor_exposes_every_format() {
        let json = schema_json();
        for name in FORMATS {
            assert!(json.contains(name), "schema format enum must list {name}");
        }
    }
}
