//! gizza-ai/blurhash — turn an image into a compact BlurHash or ThumbHash
//! placeholder string plus the preview it decodes to.
//!
//! Pipeline: resolve the image source (URL/ref) → `core::encode` (decode, sample
//! down, run the DCT, render the preview PNG) → flat JSON the LLM reads directly.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (image input + text report — the no-page
//! file-input pattern shared with image-info / image-average-color /
//! image-color-picker; the generator's pure-tool page can't hand uploaded bytes
//! to a wasm decoder, only the ffmpeg runtime takes a file upload).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_blurhash_core::{Algorithm, Options};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

const MAX_BYTES: usize = 32 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    algorithm: Option<String>,
    /// f64 everywhere (the wasm BigInt gotcha) — validated in the core.
    #[serde(default)]
    components_x: Option<f64>,
    #[serde(default)]
    components_y: Option<f64>,
    #[serde(default)]
    punch: Option<f64>,
    #[serde(default)]
    preview_size: Option<f64>,
}

#[derive(Serialize)]
struct Color {
    hex: String,
    rgb: String,
    r: u8,
    g: u8,
    b: u8,
    a: u8,
}

#[derive(Serialize)]
struct Resp {
    /// "blurhash" or "thumbhash".
    algorithm: &'static str,
    /// The placeholder string — base83 for BlurHash, base64 for ThumbHash.
    hash: String,
    /// Hex form of the ThumbHash bytes (ThumbHash only).
    #[serde(skip_serializing_if = "Option::is_none")]
    hash_hex: Option<String>,
    /// Characters in `hash`.
    hash_length: usize,
    /// Bytes to store (BlurHash: ASCII chars; ThumbHash: the binary hash).
    hash_bytes: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    components_x: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    components_y: Option<u32>,
    width: u32,
    height: u32,
    /// width / height — BlurHash does NOT store this, so keep it alongside.
    aspect_ratio: f32,
    source_format: String,
    /// Dimensions the DCT was sampled at after the internal downscale.
    sampled_width: u32,
    sampled_height: u32,
    /// The hash's own average colour — a one-pixel fallback.
    average_color: Color,
    /// True when the average colour is dark (use white text/spinners on top).
    is_dark: bool,
    preview_width: u32,
    preview_height: u32,
    /// `data:image/png;base64,...` — drop straight into an `<img src>`.
    preview_data_url: String,
    /// Ready-to-paste CSS showing the preview until the real image loads.
    css_background_image: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    filename: Option<String>,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::enumv("algorithm", ["blurhash", "thumbhash"])
                .default("blurhash")
                .describe(
                    "Which placeholder format to produce. \"blurhash\" (default) is Wolt's \
                     base83 ASCII string (28 characters at the default 4x3 grid) — widest \
                     library support, but it stores no aspect ratio or alpha. \"thumbhash\" \
                     is Evan Wallace's ~25-byte binary hash (returned as base64 and hex) — \
                     smaller, sharper, and it does carry the aspect ratio and transparency.",
                ),
        )
        .param(
            Param::integer("components_x")
                .default(4)
                .min(1.0)
                .max(9.0)
                .describe(
                    "BlurHash horizontal detail: how many DCT components across, 1-9. \
                     Optional — defaults to 4. More components mean more detail and a \
                     longer hash (length = 6 + 2*(components_x*components_y - 1)). \
                     Ignored when algorithm=thumbhash, which picks its own grid.",
                ),
        )
        .param(
            Param::integer("components_y")
                .default(3)
                .min(1.0)
                .max(9.0)
                .describe(
                    "BlurHash vertical detail: how many DCT components down, 1-9. \
                     Optional — defaults to 3. Match it roughly to the aspect ratio \
                     (4x3 for landscape, 3x4 for portrait). Ignored when \
                     algorithm=thumbhash.",
                ),
        )
        .param(
            Param::number("punch")
                .default(1.0)
                .min(0.1)
                .max(10.0)
                .describe(
                    "Contrast of the returned BlurHash preview: 1.0 (default) is the plain \
                     decode, values above 1 push the colours further apart, below 1 flattens \
                     them. Affects the preview only — the hash string is unchanged. Ignored \
                     when algorithm=thumbhash.",
                ),
        )
        .param(
            Param::integer("preview_size")
                .default(64)
                .min(8.0)
                .max(512.0)
                .describe(
                    "Longest edge, in pixels, of the rendered preview PNG returned as \
                     preview_data_url. Optional — defaults to 64. The source aspect ratio \
                     is preserved; placeholders are blurry by design, so small is fine.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

const DESCRIPTION: &str = "Generate a compact BlurHash or ThumbHash placeholder string for an image, for progressive/lazy image loading: store the tiny hash next to the image and render its blurred preview instantly while the full image downloads. Returns the hash (BlurHash = base83 ASCII, e.g. 28 characters at the default 4x3 grid; ThumbHash = a ~25-byte binary hash given as base64 and hex), its length and storage size, the source width/height and aspect ratio (BlurHash does not encode aspect ratio — keep it alongside), the hash's average color as #rrggbb / rgb() plus an is_dark flag, and a decoded preview as a data:image/png;base64 URL with a ready-to-paste CSS background-image rule. Tune BlurHash detail with components_x/components_y (1-9 each, default 4x3), preview contrast with punch, and the preview resolution with preview_size. Accepts PNG, JPEG, WebP, GIF and BMP. Provide the image as either url (HTTP/HTTPS) or ref (id from a prior tool call).";

#[cfg(target_arch = "wasm32")]
struct Blurhash;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/blurhash",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate a compact BlurHash or ThumbHash image placeholder string and preview",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Generate a compact BlurHash or ThumbHash placeholder string for an image, for progressive/lazy image loading: store the tiny hash next to the image and render its blurred preview instantly while the full image downloads. Returns the hash (BlurHash = base83 ASCII, e.g. 28 characters at the default 4x3 grid; ThumbHash = a ~25-byte binary hash given as base64 and hex), its length and storage size, the source width/height and aspect ratio (BlurHash does not encode aspect ratio — keep it alongside), the hash's average color as #rrggbb / rgb() plus an is_dark flag, and a decoded preview as a data:image/png;base64 URL with a ready-to-paste CSS background-image rule. Tune BlurHash detail with components_x/components_y (1-9 each, default 4x3), preview contrast with punch, and the preview resolution with preview_size. Accepts PNG, JPEG, WebP, GIF and BMP. Provide the image as either url (HTTP/HTTPS) or ref (id from a prior tool call).",
        parameters = schema_json()
    ),
)]
impl Blurhash {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("blurhash")?;

    let algorithm = match args.algorithm.as_deref() {
        Some(s) if !s.trim().is_empty() => Algorithm::parse(s).map_err(SkillError::InvalidArgs)?,
        _ => Algorithm::BlurHash,
    };
    let opts = Options {
        algorithm,
        components_x: whole("components_x", args.components_x, 4)?,
        components_y: whole("components_y", args.components_y, 3)?,
        punch: args.punch.unwrap_or(1.0) as f32,
        preview_size: whole("preview_size", args.preview_size, 64)?,
    };

    let (bytes, _mime, filename) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_BYTES)?;

    let out = gizza_ai_blurhash_core::encode(&bytes, &opts).map_err(SkillError::InvalidArgs)?;

    let resp = Resp {
        algorithm: out.algorithm,
        hash: out.hash,
        hash_hex: out.hash_hex,
        hash_length: out.hash_length,
        hash_bytes: out.hash_bytes,
        components_x: out.components_x,
        components_y: out.components_y,
        width: out.width,
        height: out.height,
        aspect_ratio: out.aspect_ratio,
        source_format: out.source_format,
        sampled_width: out.sampled_width,
        sampled_height: out.sampled_height,
        average_color: Color {
            hex: out.average_color.hex,
            rgb: out.average_color.rgb,
            r: out.average_color.r,
            g: out.average_color.g,
            b: out.average_color.b,
            a: out.average_color.a,
        },
        is_dark: out.is_dark,
        preview_width: out.preview_width,
        preview_height: out.preview_height,
        preview_data_url: out.preview_data_url,
        css_background_image: out.css_background_image,
        filename: (!filename.is_empty()).then_some(filename),
    };
    serde_json::to_vec(&resp)
        .map_err(|e| SkillError::Serialize(format!("serialize blurhash response: {e}")))
}

/// Accept a whole-number argument arriving as a JSON float (the wasm BigInt
/// gotcha) and reject fractional values with a message that names the field.
#[cfg(target_arch = "wasm32")]
fn whole(name: &str, v: Option<f64>, default: u32) -> Result<u32, SkillError> {
    let Some(v) = v else { return Ok(default) };
    if !v.is_finite() || v.fract() != 0.0 || v < 0.0 || v > 4096.0 {
        return Err(SkillError::InvalidArgs(format!(
            "{name} must be a whole number (got {v})"
        )));
    }
    Ok(v as u32)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "algorithm": {
                        "type": "string",
                        "enum": ["blurhash", "thumbhash"],
                        "default": "blurhash",
                        "description": "Which placeholder format to produce. \"blurhash\" (default) is Wolt's base83 ASCII string (28 characters at the default 4x3 grid) — widest library support, but it stores no aspect ratio or alpha. \"thumbhash\" is Evan Wallace's ~25-byte binary hash (returned as base64 and hex) — smaller, sharper, and it does carry the aspect ratio and transparency."
                    },
                    "components_x": {
                        "type": "integer",
                        "default": 4,
                        "minimum": 1,
                        "maximum": 9,
                        "description": "BlurHash horizontal detail: how many DCT components across, 1-9. Optional — defaults to 4. More components mean more detail and a longer hash (length = 6 + 2*(components_x*components_y - 1)). Ignored when algorithm=thumbhash, which picks its own grid."
                    },
                    "components_y": {
                        "type": "integer",
                        "default": 3,
                        "minimum": 1,
                        "maximum": 9,
                        "description": "BlurHash vertical detail: how many DCT components down, 1-9. Optional — defaults to 3. Match it roughly to the aspect ratio (4x3 for landscape, 3x4 for portrait). Ignored when algorithm=thumbhash."
                    },
                    "punch": {
                        "type": "number",
                        "default": 1.0,
                        "minimum": 0.1,
                        "maximum": 10,
                        "description": "Contrast of the returned BlurHash preview: 1.0 (default) is the plain decode, values above 1 push the colours further apart, below 1 flattens them. Affects the preview only — the hash string is unchanged. Ignored when algorithm=thumbhash."
                    },
                    "preview_size": {
                        "type": "integer",
                        "default": 64,
                        "minimum": 8,
                        "maximum": 512,
                        "description": "Longest edge, in pixels, of the rendered preview PNG returned as preview_data_url. Optional — defaults to 64. The source aspect ratio is preserved; placeholders are blurry by design, so small is fine."
                    }
                },
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
