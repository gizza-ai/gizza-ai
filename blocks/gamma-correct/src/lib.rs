//! gizza-ai/gamma-correct — apply a gamma (power-law) correction to an image's
//! midtones via ffmpeg's `eq` filter, on the shared tool abstraction.
//!
//! Input::Image emits a url⊕ref oneOf; run() uses resolve_source → core::plan →
//! dispatch_ffmpeg → build_media_envelope. Gamma is the classic non-linear
//! midtone knob: `gamma > 1` brightens midtones (recovers a dark photo),
//! `gamma < 1` darkens them, `gamma = 1` is the identity — pure black and white
//! stay fixed, so it never clips the endpoints. Per-channel `gamma_r/g/b` correct
//! a color cast independently; `gamma_weight` (0-1) reduces the correction on the
//! brightest pixels to protect highlights from blowout; `format` can convert the
//! output to png/jpg/webp. The chat schema is derived from `descriptor()` (single
//! source — shared across chat + CLI + page) and the drift-guard test below proves
//! it matches the authored schema.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{
    build_media_envelope, mime_to_ext, AssetKind, Input, Param, SkillError, SourceFields,
    ToolDescriptor,
};
// resolve_source / dispatch_ffmpeg call host imports → wasm-only (like run() below).
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_gamma_correct_core::{
    parse_format, OutFormat, DEFAULT_FORMAT, DEFAULT_GAMMA, DEFAULT_WEIGHT, FORMATS, GAMMA_MAX,
    GAMMA_MIN, WEIGHT_MAX, WEIGHT_MIN,
};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_BYTES: usize = 8 * 1024 * 1024;

#[derive(Deserialize)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    gamma: Option<f64>,
    #[serde(default)]
    gamma_r: Option<f64>,
    #[serde(default)]
    gamma_g: Option<f64>,
    #[serde(default)]
    gamma_b: Option<f64>,
    #[serde(default)]
    gamma_weight: Option<f64>,
    #[serde(default)]
    format: Option<String>,
}

fn descriptor() -> ToolDescriptor {
    // Input::Image → url⊕ref oneOf. `gamma` is the overall midtone power-law knob
    // (>1 brightens, <1 darkens, 1 = no change); per-channel `gamma_r/g/b` correct
    // a color cast; `gamma_weight` protects highlights; `format` optionally
    // converts the output.
    ToolDescriptor::new(Input::Image)
        .param(
            Param::number("gamma")
                .min(GAMMA_MIN)
                .max(GAMMA_MAX)
                .default(DEFAULT_GAMMA)
                .describe(
                    "Overall gamma, 0.1-10. Above 1 brightens the midtones (lift a dark photo), \
                     below 1 darkens them; 1 (the default) leaves the image unchanged. Pure black \
                     and white stay fixed, so midtones move without clipping. Example: 1.8 to \
                     brighten an underexposed photo.",
                ),
        )
        .param(
            Param::number("gamma_r")
                .min(GAMMA_MIN)
                .max(GAMMA_MAX)
                .default(DEFAULT_GAMMA)
                .describe(
                    "Red-channel gamma, 0.1-10 (default 1 = no change). Raise or lower just the \
                     red midtones to correct a color cast; composes with the overall gamma.",
                ),
        )
        .param(
            Param::number("gamma_g")
                .min(GAMMA_MIN)
                .max(GAMMA_MAX)
                .default(DEFAULT_GAMMA)
                .describe(
                    "Green-channel gamma, 0.1-10 (default 1 = no change). Raise or lower just the \
                     green midtones to correct a color cast; composes with the overall gamma.",
                ),
        )
        .param(
            Param::number("gamma_b")
                .min(GAMMA_MIN)
                .max(GAMMA_MAX)
                .default(DEFAULT_GAMMA)
                .describe(
                    "Blue-channel gamma, 0.1-10 (default 1 = no change). Raise or lower just the \
                     blue midtones to correct a color cast; composes with the overall gamma.",
                ),
        )
        .param(
            Param::number("gamma_weight")
                .min(WEIGHT_MIN)
                .max(WEIGHT_MAX)
                .default(DEFAULT_WEIGHT)
                .describe(
                    "Highlight-protection weight, 0-1 (default 1 = full correction everywhere). \
                     Lower it to fade the gamma out on the brightest pixels so a strong gamma \
                     doesn't blow out highlights; 0 leaves the brightest areas untouched.",
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
    name = "gizza-ai/gamma-correct",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Brighten or darken an image's midtones with a gamma curve, with optional per-channel gamma and highlight protection.",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Apply a gamma (power-law) correction to an image's midtones. Provide either url (HTTP/HTTPS) or ref (id from a prior image tool call); optional gamma 0.1-10 (default 1; above 1 brightens midtones, below 1 darkens, 1 = no change), per-channel gamma_r/gamma_g/gamma_b 0.1-10 (default 1) to correct a color cast, gamma_weight 0-1 (default 1) to protect highlights from blowout, and format keep|png|jpg|webp (default keep). Black and white stay fixed so midtones move without clipping.",
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
        .map_err(|e| SkillError::InvalidArgs(format!("invalid gamma-correct args: {e}")))?;
    let gamma = args.gamma.unwrap_or(DEFAULT_GAMMA);
    let gamma_r = args.gamma_r.unwrap_or(DEFAULT_GAMMA);
    let gamma_g = args.gamma_g.unwrap_or(DEFAULT_GAMMA);
    let gamma_b = args.gamma_b.unwrap_or(DEFAULT_GAMMA);
    let gamma_weight = args.gamma_weight.unwrap_or(DEFAULT_WEIGHT);
    let format = parse_format(args.format.as_deref()).map_err(SkillError::InvalidArgs)?;
    let (bytes, mime, in_name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_BYTES)?;
    let ext = mime_to_ext(&mime)
        .ok_or_else(|| SkillError::InvalidArgs(format!("unsupported mime: {mime}")))?;
    let in_path = format!("in.{ext}");
    let (argv, out_name) = gizza_ai_gamma_correct_core::plan(
        &in_path, gamma, gamma_r, gamma_g, gamma_b, gamma_weight, format,
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
    let mut for_llm = format!("applied gamma {gamma} to {in_name}");
    if gamma_r != DEFAULT_GAMMA || gamma_g != DEFAULT_GAMMA || gamma_b != DEFAULT_GAMMA {
        for_llm.push_str(&format!(" (r={gamma_r} g={gamma_g} b={gamma_b})"));
    }
    if gamma_weight != DEFAULT_WEIGHT {
        for_llm.push_str(&format!(" (highlight weight {gamma_weight})"));
    }
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
                    "gamma": {
                        "type": "number",
                        "minimum": 0.1,
                        "maximum": 10,
                        "default": 1.0,
                        "description": "Overall gamma, 0.1-10. Above 1 brightens the midtones (lift a dark photo), below 1 darkens them; 1 (the default) leaves the image unchanged. Pure black and white stay fixed, so midtones move without clipping. Example: 1.8 to brighten an underexposed photo."
                    },
                    "gamma_r": {
                        "type": "number",
                        "minimum": 0.1,
                        "maximum": 10,
                        "default": 1.0,
                        "description": "Red-channel gamma, 0.1-10 (default 1 = no change). Raise or lower just the red midtones to correct a color cast; composes with the overall gamma."
                    },
                    "gamma_g": {
                        "type": "number",
                        "minimum": 0.1,
                        "maximum": 10,
                        "default": 1.0,
                        "description": "Green-channel gamma, 0.1-10 (default 1 = no change). Raise or lower just the green midtones to correct a color cast; composes with the overall gamma."
                    },
                    "gamma_b": {
                        "type": "number",
                        "minimum": 0.1,
                        "maximum": 10,
                        "default": 1.0,
                        "description": "Blue-channel gamma, 0.1-10 (default 1 = no change). Raise or lower just the blue midtones to correct a color cast; composes with the overall gamma."
                    },
                    "gamma_weight": {
                        "type": "number",
                        "minimum": 0,
                        "maximum": 1,
                        "default": 1.0,
                        "description": "Highlight-protection weight, 0-1 (default 1 = full correction everywhere). Lower it to fade the gamma out on the brightest pixels so a strong gamma doesn't blow out highlights; 0 leaves the brightest areas untouched."
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
    fn descriptor_exposes_every_format() {
        let json = schema_json();
        for name in FORMATS {
            assert!(json.contains(name), "schema format enum must list {name}");
        }
    }
}
