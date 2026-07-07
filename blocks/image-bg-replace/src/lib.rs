//! gizza-ai/image-bg-replace — chroma-key background replacement via ffmpeg,
//! on the shared tool abstraction.
//!
//! Input::Image emits a url⊕ref oneOf; run() uses resolve_source → core::plan →
//! dispatch_ffmpeg → build_media_envelope. A `key_color` (default green) is
//! keyed out with `similarity`/`blend`, and the subject is composited onto a
//! transparent, solid, or two-color gradient background. The chat schema is
//! derived from `descriptor()` (single source — shared across chat + CLI + page)
//! and the drift-guard test below proves it matches the authored schema.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{
    build_media_envelope, mime_to_ext, AssetKind, Input, Param, SkillError, SourceFields,
    ToolDescriptor,
};
// resolve_source / dispatch_ffmpeg call host imports → wasm-only (like run() below).
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_image_bg_replace_core::{
    parse_bg_type, parse_color, parse_direction, parse_format, rename_ext, BgType, OutFormat,
    BG_TYPES, DEFAULT_BG_COLOR, DEFAULT_BG_COLOR2, DEFAULT_BG_TYPE, DEFAULT_BLEND, DEFAULT_DIRECTION,
    DEFAULT_FORMAT, DEFAULT_KEY_COLOR, DEFAULT_SIMILARITY, DIRECTIONS, FORMATS,
};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_BYTES: usize = 8 * 1024 * 1024;

#[derive(Deserialize)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    key_color: Option<String>,
    #[serde(default)]
    similarity: Option<f64>,
    #[serde(default)]
    blend: Option<f64>,
    #[serde(default)]
    bg_type: Option<String>,
    #[serde(default)]
    bg_color: Option<String>,
    #[serde(default)]
    bg_color2: Option<String>,
    #[serde(default)]
    direction: Option<String>,
    #[serde(default)]
    format: Option<String>,
}

fn descriptor() -> ToolDescriptor {
    // Input::Image → url⊕ref oneOf. key_color is the background color removed by
    // ffmpeg's colorkey; similarity/blend tune the key; bg_type picks how the
    // now-empty background is refilled (transparent / solid / gradient).
    ToolDescriptor::new(Input::Image)
        .param(Param::string("key_color").default(DEFAULT_KEY_COLOR).describe(
            "Background color to remove (chroma key): a name (green, lime, blue, cyan, white, \
             black, …) or hex like #00FF00 / #0F0. Default #00ff00 (green-screen green). Set it \
             to your backdrop's color — this works on solid/green/blue-screen backgrounds, not \
             busy photos.",
        ))
        .param(
            Param::number("similarity")
                .min(0.0)
                .max(100.0)
                .default(DEFAULT_SIMILARITY)
                .describe(
                    "How wide a range of colors near the key color counts as background, 0-100. \
                     Low (10-20) removes only near-exact matches; high (50+) also removes similar \
                     shades but may eat the subject. Default 30.",
                ),
        )
        .param(
            Param::number("blend")
                .min(0.0)
                .max(100.0)
                .default(DEFAULT_BLEND)
                .describe(
                    "Edge softness, 0-100. 0 gives a hard cut-out; higher values feather the \
                     cut-out edge to hide jaggies (and any color fringe). Default 10.",
                ),
        )
        .param(Param::enumv("bg_type", BG_TYPES).default(DEFAULT_BG_TYPE).describe(
            "New background: solid (default) fills a single color; gradient blends two colors; \
             transparent keeps the cut-out alpha (needs png or webp output).",
        ))
        .param(Param::string("bg_color").default(DEFAULT_BG_COLOR).describe(
            "Solid fill color, or the START color of a gradient: a name or hex. Default #ffffff \
             (white). Ignored when bg_type=transparent.",
        ))
        .param(Param::string("bg_color2").default(DEFAULT_BG_COLOR2).describe(
            "The END color of a gradient (only used when bg_type=gradient): a name or hex. \
             Default #000000 (black).",
        ))
        .param(
            Param::enumv("direction", DIRECTIONS)
                .default(DEFAULT_DIRECTION)
                .describe(
                    "Gradient direction (only used when bg_type=gradient): vertical (top→bottom, \
                     default) or horizontal (left→right).",
                ),
        )
        .param(Param::enumv("format", FORMATS).default(DEFAULT_FORMAT).describe(
            "Output format: png (default, lossless, keeps transparency), webp (small, keeps \
             transparency), jpg (small, no transparency), or keep (same container as the input). \
             A transparent background needs png or webp.",
        ))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/image-bg-replace",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Chroma-key a photo's solid or green-screen background out and composite the subject onto a transparent, solid, or gradient background.",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    skill(
        description = "Remove a photo's solid-color / green-screen background by chroma-keying a color, then composite the subject onto a transparent, solid, or two-color gradient background. Provide either url (HTTP/HTTPS) or ref (id from a prior image tool call); optional key_color (name or hex, default #00ff00 green), similarity 0-100 (default 30), blend 0-100 (default 10), bg_type transparent|solid|gradient (default solid), bg_color (default #ffffff), bg_color2 for gradients (default #000000), direction vertical|horizontal (default vertical), and format png|webp|jpg|keep (default png). Works on solid/green/blue-screen backgrounds, not busy photos; no AI subject detection.",
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
        .map_err(|e| SkillError::InvalidArgs(format!("invalid image-bg-replace args: {e}")))?;
    let key = parse_color(args.key_color.as_deref(), DEFAULT_KEY_COLOR)
        .map_err(SkillError::InvalidArgs)?;
    let similarity = args.similarity.unwrap_or(DEFAULT_SIMILARITY);
    let blend = args.blend.unwrap_or(DEFAULT_BLEND);
    let bg_type = parse_bg_type(args.bg_type.as_deref()).map_err(SkillError::InvalidArgs)?;
    let bg_color =
        parse_color(args.bg_color.as_deref(), DEFAULT_BG_COLOR).map_err(SkillError::InvalidArgs)?;
    let bg_color2 = parse_color(args.bg_color2.as_deref(), DEFAULT_BG_COLOR2)
        .map_err(SkillError::InvalidArgs)?;
    let direction = parse_direction(args.direction.as_deref()).map_err(SkillError::InvalidArgs)?;
    let format = parse_format(args.format.as_deref()).map_err(SkillError::InvalidArgs)?;

    let (bytes, mime, in_name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_BYTES)?;
    let ext = mime_to_ext(&mime)
        .ok_or_else(|| SkillError::InvalidArgs(format!("unsupported mime: {mime}")))?;
    let in_path = format!("in.{ext}");
    let (argv, out_name) = gizza_ai_image_bg_replace_core::plan(
        &in_path, key, similarity, blend, bg_type, bg_color, bg_color2, direction, format,
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
    let (kr, kg, kb) = key;
    let bg_desc = match bg_type {
        BgType::Transparent => "a transparent background".to_string(),
        BgType::Solid => {
            let (r, g, b) = bg_color;
            format!("a solid #{r:02X}{g:02X}{b:02X} background")
        }
        BgType::Gradient => {
            let (r1, g1, b1) = bg_color;
            let (r2, g2, b2) = bg_color2;
            format!("a #{r1:02X}{g1:02X}{b1:02X}→#{r2:02X}{g2:02X}{b2:02X} gradient background")
        }
    };
    let for_llm = format!(
        "removed the #{kr:02X}{kg:02X}{kb:02X} background from {in_name} and composited the \
         subject onto {bg_desc}, output {out_display}"
    );
    build_media_envelope(&output, &out_mime, out_display, for_llm, MAX_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift-guard: the descriptor-derived chat schema must match the authored
    /// schema below, so the LLM-facing tool definition never silently changes.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r##"{
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "key_color": {
                        "type": "string",
                        "default": "#00ff00",
                        "description": "Background color to remove (chroma key): a name (green, lime, blue, cyan, white, black, …) or hex like #00FF00 / #0F0. Default #00ff00 (green-screen green). Set it to your backdrop's color — this works on solid/green/blue-screen backgrounds, not busy photos."
                    },
                    "similarity": {
                        "type": "number",
                        "minimum": 0,
                        "maximum": 100,
                        "default": 30.0,
                        "description": "How wide a range of colors near the key color counts as background, 0-100. Low (10-20) removes only near-exact matches; high (50+) also removes similar shades but may eat the subject. Default 30."
                    },
                    "blend": {
                        "type": "number",
                        "minimum": 0,
                        "maximum": 100,
                        "default": 10.0,
                        "description": "Edge softness, 0-100. 0 gives a hard cut-out; higher values feather the cut-out edge to hide jaggies (and any color fringe). Default 10."
                    },
                    "bg_type": {
                        "type": "string",
                        "enum": ["transparent", "solid", "gradient"],
                        "default": "solid",
                        "description": "New background: solid (default) fills a single color; gradient blends two colors; transparent keeps the cut-out alpha (needs png or webp output)."
                    },
                    "bg_color": {
                        "type": "string",
                        "default": "#ffffff",
                        "description": "Solid fill color, or the START color of a gradient: a name or hex. Default #ffffff (white). Ignored when bg_type=transparent."
                    },
                    "bg_color2": {
                        "type": "string",
                        "default": "#000000",
                        "description": "The END color of a gradient (only used when bg_type=gradient): a name or hex. Default #000000 (black)."
                    },
                    "direction": {
                        "type": "string",
                        "enum": ["vertical", "horizontal"],
                        "default": "vertical",
                        "description": "Gradient direction (only used when bg_type=gradient): vertical (top→bottom, default) or horizontal (left→right)."
                    },
                    "format": {
                        "type": "string",
                        "enum": ["keep", "png", "jpg", "webp"],
                        "default": "png",
                        "description": "Output format: png (default, lossless, keeps transparency), webp (small, keeps transparency), jpg (small, no transparency), or keep (same container as the input). A transparent background needs png or webp."
                    }
                },
                "additionalProperties": false,
                "oneOf": [
                    { "required": ["url"] },
                    { "required": ["ref"] }
                ]
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn descriptor_exposes_every_bg_type() {
        let json = schema_json();
        for name in BG_TYPES {
            assert!(json.contains(name), "schema enum must list {name}");
        }
    }

    #[test]
    fn descriptor_exposes_every_format_and_direction() {
        let json = schema_json();
        for name in FORMATS {
            assert!(json.contains(name), "schema format enum must list {name}");
        }
        for name in DIRECTIONS {
            assert!(json.contains(name), "schema direction enum must list {name}");
        }
    }
}
