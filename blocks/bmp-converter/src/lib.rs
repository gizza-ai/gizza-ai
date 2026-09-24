//! gizza-ai/bmp-converter — convert an image to an uncompressed Windows BMP at
//! a chosen bit depth (or back out of BMP to PNG/JPEG), on the shared tool
//! abstraction.
//!
//! Input::Image emits a url⊕ref oneOf; run() uses resolve_source → core::plan →
//! dispatch_ffmpeg → build_media_envelope. The chat schema is derived from
//! `descriptor()` (single source — shared across chat + CLI + page) and the
//! drift-guard test below proves it matches the authored schema.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{
    build_media_envelope, mime_to_ext, AssetKind, Input, Param, SkillError, SourceFields,
    ToolDescriptor,
};
// resolve_source / dispatch_ffmpeg call host imports → wasm-only (like run() below).
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_bmp_converter_core::{
    normalize_color, parse_bool, parse_colors, parse_depth, parse_dither, parse_format, summary,
    OutFormat, BIT_DEPTHS, DEFAULT_BACKGROUND, DEFAULT_BIT_DEPTH, DEFAULT_COLORS, DEFAULT_DITHER,
    DEFAULT_FORMAT, DEFAULT_GRAYSCALE, DITHERS, FORMATS, MAX_COLORS, MIN_COLORS,
};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_BYTES: usize = 8 * 1024 * 1024;

#[derive(Deserialize)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    format: Option<String>,
    #[serde(default)]
    bit_depth: Option<String>,
    #[serde(default)]
    colors: Option<f64>,
    #[serde(default)]
    dither: Option<String>,
    #[serde(default)]
    grayscale: Option<bool>,
    #[serde(default)]
    background: Option<String>,
}

fn descriptor() -> ToolDescriptor {
    // Input::Image → url⊕ref oneOf. Param ORDER is load-bearing: the page's
    // meta.toml field order and web/src/lib.rs build_argv must match it.
    ToolDescriptor::new(Input::Image)
        .param(
            Param::enumv("format", FORMATS)
                .default(DEFAULT_FORMAT)
                .describe(
                    "Output format: bmp (default) writes an uncompressed Windows BMP at the \
                     chosen bit_depth; png or jpeg convert the other way, out of BMP into a \
                     modern format. bit_depth, colors and dither only apply to bmp.",
                ),
        )
        .param(
            Param::enumv("bit_depth", BIT_DEPTHS)
                .default(DEFAULT_BIT_DEPTH)
                .describe(
                    "BMP colour depth in bits per pixel. 24 (default) is classic true colour; \
                     32 keeps transparency; 16-565 and 16-555 are high colour (5:6:5 uses \
                     BITFIELDS masks, 5:5:5 is the most widely readable 16-bit layout); 8 is a \
                     256-colour indexed palette (or greyscale with grayscale=true); 1 is black \
                     and white. Example: 8 for a small retro-style bitmap.",
                ),
        )
        .param(
            Param::integer("colors")
                .min(MIN_COLORS as f64)
                .max(MAX_COLORS as f64)
                .default(DEFAULT_COLORS)
                .describe(
                    "Palette size for 8-bit indexed output, 2-256 (default 256). Ignored at any \
                     other bit_depth and when grayscale is true. Example: 16 for a 16-colour \
                     poster look.",
                ),
        )
        .param(
            Param::enumv("dither", DITHERS)
                .default(DEFAULT_DITHER)
                .describe(
                    "How to hide banding when colours are reduced (8-bit indexed and 1-bit \
                     output only). floyd_steinberg (default) and sierra2_4a are error-diffusion \
                     — finer, photo-friendly; bayer is an ordered cross-hatch pattern; none \
                     gives hard flat bands. Example: none for pixel-art or logos.",
                ),
        )
        .param(
            Param::boolean("grayscale")
                .default(DEFAULT_GRAYSCALE)
                .describe(
                    "true converts to grey before writing. At bit_depth 8 this writes a true \
                     8-bit greyscale BMP (no colour palette); at other depths the image is grey \
                     but stored in that depth. Default false (keep colour).",
                ),
        )
        .param(
            Param::string("background")
                .default(DEFAULT_BACKGROUND)
                .describe(
                    "Colour used to flatten transparency, as hex (#ffffff, #f00) or an ffmpeg \
                     colour name (white, black, navy). Default #ffffff. Applies whenever the \
                     output cannot store alpha — every bmp depth except 32, and jpeg. Ignored \
                     for 32-bit BMP and png output, which keep transparency.",
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
    name = "gizza-ai/bmp-converter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert an image to an uncompressed BMP at a chosen bit depth (1, 8, 16, 24 or 32), or back out of BMP to PNG or JPEG.",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Convert an image to or from an uncompressed Windows BMP. Provide either url (HTTP/HTTPS) or ref (id from a prior image tool call); optional format bmp|png|jpeg (default bmp), bit_depth 1|8|16-555|16-565|24|32 (default 24), colors 2-256 for 8-bit palettes (default 256), dither none|bayer|floyd_steinberg|sierra2_4a (default floyd_steinberg), grayscale true|false (default false), and background hex/name used to flatten transparency (default #ffffff).",
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
        .map_err(|e| SkillError::InvalidArgs(format!("invalid bmp-converter args: {e}")))?;
    let format = parse_format(args.format.as_deref()).map_err(SkillError::InvalidArgs)?;
    let depth = parse_depth(args.bit_depth.as_deref()).map_err(SkillError::InvalidArgs)?;
    let colors = parse_colors(args.colors.unwrap_or(DEFAULT_COLORS as f64))
        .map_err(SkillError::InvalidArgs)?;
    let dither = parse_dither(args.dither.as_deref()).map_err(SkillError::InvalidArgs)?;
    let grayscale = args.grayscale.unwrap_or(DEFAULT_GRAYSCALE);
    let background =
        normalize_color(args.background.as_deref()).map_err(SkillError::InvalidArgs)?;

    let (bytes, mime, in_name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_BYTES)?;
    let ext = mime_to_ext(&mime)
        .ok_or_else(|| SkillError::InvalidArgs(format!("unsupported mime: {mime}")))?;
    let in_path = format!("in.{ext}");
    let (argv, out_name) = gizza_ai_bmp_converter_core::plan(
        &in_path,
        format,
        depth,
        colors,
        dither,
        grayscale,
        &background,
    )
    .map_err(SkillError::InvalidArgs)?;
    let output = dispatch_ffmpeg(argv, in_path, bytes, out_name)?;
    // The envelope must describe the OUTPUT, which always changes format here.
    let (out_mime, out_ext) = match format {
        OutFormat::Bmp => ("image/bmp", "bmp"),
        OutFormat::Png => ("image/png", "png"),
        OutFormat::Jpeg => ("image/jpeg", "jpg"),
    };
    let out_display = rename_ext(&in_name, out_ext);
    let for_llm = format!(
        "converted {in_name} to {out_display} ({})",
        summary(format, depth, grayscale)
    );
    build_media_envelope(&output, out_mime, out_display, for_llm, MAX_BYTES)
}

/// `photo.jpeg` + `bmp` → `photo.bmp` (append when there is no extension).
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
            r##"{
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "format": {
                        "type": "string",
                        "enum": ["bmp", "png", "jpeg"],
                        "default": "bmp",
                        "description": "Output format: bmp (default) writes an uncompressed Windows BMP at the chosen bit_depth; png or jpeg convert the other way, out of BMP into a modern format. bit_depth, colors and dither only apply to bmp."
                    },
                    "bit_depth": {
                        "type": "string",
                        "enum": ["1", "8", "16-555", "16-565", "24", "32"],
                        "default": "24",
                        "description": "BMP colour depth in bits per pixel. 24 (default) is classic true colour; 32 keeps transparency; 16-565 and 16-555 are high colour (5:6:5 uses BITFIELDS masks, 5:5:5 is the most widely readable 16-bit layout); 8 is a 256-colour indexed palette (or greyscale with grayscale=true); 1 is black and white. Example: 8 for a small retro-style bitmap."
                    },
                    "colors": {
                        "type": "integer",
                        "minimum": 2,
                        "maximum": 256,
                        "default": 256,
                        "description": "Palette size for 8-bit indexed output, 2-256 (default 256). Ignored at any other bit_depth and when grayscale is true. Example: 16 for a 16-colour poster look."
                    },
                    "dither": {
                        "type": "string",
                        "enum": ["none", "bayer", "floyd_steinberg", "sierra2_4a"],
                        "default": "floyd_steinberg",
                        "description": "How to hide banding when colours are reduced (8-bit indexed and 1-bit output only). floyd_steinberg (default) and sierra2_4a are error-diffusion — finer, photo-friendly; bayer is an ordered cross-hatch pattern; none gives hard flat bands. Example: none for pixel-art or logos."
                    },
                    "grayscale": {
                        "type": "boolean",
                        "default": false,
                        "description": "true converts to grey before writing. At bit_depth 8 this writes a true 8-bit greyscale BMP (no colour palette); at other depths the image is grey but stored in that depth. Default false (keep colour)."
                    },
                    "background": {
                        "type": "string",
                        "default": "#ffffff",
                        "description": "Colour used to flatten transparency, as hex (#ffffff, #f00) or an ffmpeg colour name (white, black, navy). Default #ffffff. Applies whenever the output cannot store alpha — every bmp depth except 32, and jpeg. Ignored for 32-bit BMP and png output, which keep transparency."
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
    fn descriptor_exposes_every_advertised_value() {
        let json = schema_json();
        for name in FORMATS {
            assert!(json.contains(name), "schema format enum must list {name}");
        }
        for name in BIT_DEPTHS {
            assert!(
                json.contains(name),
                "schema bit_depth enum must list {name}"
            );
        }
        for name in DITHERS {
            assert!(json.contains(name), "schema dither enum must list {name}");
        }
    }

    #[test]
    fn every_param_has_a_description() {
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = derived["properties"].as_object().unwrap();
        assert_eq!(props.len(), 8, "url + ref + 6 params");
        for (name, prop) in props {
            let d = prop["description"].as_str().unwrap_or("");
            assert!(d.len() > 20, "{name} needs a real .describe()");
        }
    }

    #[test]
    fn defaults_are_wired_through() {
        assert_eq!(DEFAULT_FORMAT, "bmp");
        assert_eq!(DEFAULT_BIT_DEPTH, "24");
        assert_eq!(DEFAULT_COLORS, 256);
        assert_eq!(DEFAULT_DITHER, "floyd_steinberg");
        assert_eq!(DEFAULT_BACKGROUND, "#ffffff");
        assert!(!DEFAULT_GRAYSCALE);
        assert_eq!(parse_format(None).unwrap(), OutFormat::Bmp);
        assert_eq!(parse_depth(None).unwrap().bits(), 24);
        assert_eq!(parse_colors(DEFAULT_COLORS as f64).unwrap(), 256);
        assert!(!parse_bool(None, DEFAULT_GRAYSCALE).unwrap());
        assert_eq!(normalize_color(None).unwrap(), "0xffffff");
    }
}
