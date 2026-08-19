//! gizza-ai/image-watermark-tile — ffmpeg chat skill.
//!
//! Stamps a repeating (tiled) text watermark across the WHOLE image — the
//! stock-agency anti-theft pattern that survives cropping. The standalone page
//! and the CLI are the verified surfaces; chat ffmpeg is unavailable in the
//! Service Worker.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{
    build_media_envelope, mime_to_ext, replace_extension, AssetKind, Input, Param, SkillError,
    SkillResultExt, SourceFields, ToolDescriptor,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg_inputs, resolve_source};
use gizza_ai_image_watermark_tile_core::{
    parse_format, parse_pattern, pattern_name, plan, OutFormat, DEFAULT_ANGLE, DEFAULT_COLOR,
    DEFAULT_COLUMNS, DEFAULT_FONT_SIZE, DEFAULT_OPACITY, DEFAULT_ROWS, FONT_BYTES, FONT_FILE,
    TEXT_FILE,
};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 25 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 25 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    text: String,
    #[serde(default)]
    font_size: Option<f64>,
    #[serde(default)]
    color: Option<String>,
    #[serde(default)]
    opacity: Option<f64>,
    #[serde(default)]
    angle: Option<f64>,
    #[serde(default)]
    columns: Option<f64>,
    #[serde(default)]
    rows: Option<f64>,
    #[serde(default)]
    pattern: Option<String>,
    #[serde(default)]
    outline: Option<bool>,
    #[serde(default)]
    format: Option<String>,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::string("text")
                .required()
                .describe("The watermark text repeated over the whole image, e.g. \"SAMPLE\", \"© 2026 Studio\" or \"CONFIDENTIAL\". Drawn literally from a text file, so quotes, colons and commas are safe. Max 120 characters; use \\n for a second line."),
        )
        .param(
            Param::integer("font_size")
                .min(6.0)
                .max(400.0)
                .default(32)
                .describe("Watermark text size in pixels (6-400). Default 32; use ~2% of the image width as a starting point (about 40 for a 2000px-wide photo)."),
        )
        .param(
            Param::string("color")
                .default("#ffffff")
                .describe("Watermark color as a CSS color name or hex (#ffffff, #fff, black, navy). Default #ffffff — white reads well on photos, black on light scans."),
        )
        .param(
            Param::number("opacity")
                .min(0.02)
                .max(1.0)
                .default(0.3)
                .describe("Watermark opacity from 0.02 (barely visible) to 1.0 (solid). Default 0.3 — visible enough to deter reuse without hiding the picture."),
        )
        .param(
            Param::number("angle")
                .min(-90.0)
                .max(90.0)
                .default(30.0)
                .describe("Rotation of the whole tiled pattern in degrees, -90 to 90. Default 30 (the classic diagonal); 0 draws straight horizontal rows."),
        )
        .param(
            Param::integer("columns")
                .min(1.0)
                .max(12.0)
                .default(4)
                .describe("How many watermark tiles across the image (1-12). Default 4. Higher = denser, harder to crop or clone out."),
        )
        .param(
            Param::integer("rows")
                .min(1.0)
                .max(12.0)
                .default(5)
                .describe("How many watermark tiles down the image (1-12). Default 5. Higher = denser coverage."),
        )
        .param(
            Param::enumv("pattern", ["grid", "brick"])
                .default("brick")
                .describe("Tile layout: brick (default) offsets alternate rows by half a cell for staggered, checker-wise coverage; grid aligns every row in a plain lattice."),
        )
        .param(
            Param::boolean("outline")
                .default(false)
                .describe("Add a black outline around each watermark so it stays legible over both light and dark areas. Default false."),
        )
        .param(
            Param::enumv("format", ["keep", "png", "jpg", "webp"])
                .default("keep")
                .describe("Output container: keep (default) reuses the input's format, or convert to png (lossless), jpg (small, no transparency) or webp."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/image-watermark-tile",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Tile a repeating text watermark across a whole image",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Stamp a repeating (tiled) text watermark across an entire image so it cannot be cropped off — the anti-theft pattern stock agencies use on previews. Provide the image as url or ref plus the watermark text, and optionally font_size, color, opacity, angle (default 30 = diagonal), columns/rows tile density, pattern (brick or grid), outline, and output format. Text is drawn with a bundled font via ffmpeg drawtext using textfile/fontfile, so no escaping is needed. Tile positions are relative, so the same settings look identical on any image size. Returns the watermarked image. Note: runs on the standalone page and CLI; chat ffmpeg is unavailable.",
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
    let args: Args = serde_json::from_slice(&body).invalid_args("image-watermark-tile")?;
    let pattern = parse_pattern(args.pattern.as_deref()).map_err(SkillError::InvalidArgs)?;
    let format = parse_format(args.format.as_deref()).map_err(SkillError::InvalidArgs)?;
    let (bytes, mime, in_display) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
    let ext = mime_to_ext(&mime)
        .ok_or_else(|| SkillError::InvalidArgs(format!("unsupported mime: {mime}")))?;
    let in_path = format!("in.{ext}");
    let (argv, out_name) = plan(
        &in_path,
        &args.text,
        args.font_size.unwrap_or(DEFAULT_FONT_SIZE),
        args.color.as_deref().unwrap_or(DEFAULT_COLOR),
        args.opacity.unwrap_or(DEFAULT_OPACITY),
        args.angle.unwrap_or(DEFAULT_ANGLE),
        args.columns.unwrap_or(DEFAULT_COLUMNS as f64),
        args.rows.unwrap_or(DEFAULT_ROWS as f64),
        pattern,
        args.outline.unwrap_or(false),
        format,
    )
    .map_err(SkillError::InvalidArgs)?;

    let inputs = vec![
        (in_path, bytes),
        (FONT_FILE.to_string(), FONT_BYTES.to_vec()),
        (TEXT_FILE.to_string(), args.text.clone().into_bytes()),
    ];
    let output = dispatch_ffmpeg_inputs(argv, inputs, out_name)?;

    // The envelope must describe the OUTPUT: converting changes mime + name.
    let (out_mime, out_display) = match format {
        OutFormat::Keep => (mime.clone(), in_display.clone()),
        OutFormat::Png => ("image/png".to_string(), replace_extension(&in_display, "png")),
        OutFormat::Jpg => ("image/jpeg".to_string(), replace_extension(&in_display, "jpg")),
        OutFormat::Webp => ("image/webp".to_string(), replace_extension(&in_display, "webp")),
    };
    let for_llm = format!(
        "tiled the watermark \"{}\" across {in_display} ({}x{} {} pattern, {}° rotation)",
        args.text,
        args.columns.unwrap_or(DEFAULT_COLUMNS as f64).round(),
        args.rows.unwrap_or(DEFAULT_ROWS as f64).round(),
        pattern_name(pattern),
        args.angle.unwrap_or(DEFAULT_ANGLE),
    );
    build_media_envelope(&output, &out_mime, out_display, for_llm, MAX_OUTPUT_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r##"{
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "text": { "type": "string", "description": "The watermark text repeated over the whole image, e.g. \"SAMPLE\", \"© 2026 Studio\" or \"CONFIDENTIAL\". Drawn literally from a text file, so quotes, colons and commas are safe. Max 120 characters; use \\n for a second line." },
                    "font_size": { "type": "integer", "minimum": 6, "maximum": 400, "default": 32, "description": "Watermark text size in pixels (6-400). Default 32; use ~2% of the image width as a starting point (about 40 for a 2000px-wide photo)." },
                    "color": { "type": "string", "default": "#ffffff", "description": "Watermark color as a CSS color name or hex (#ffffff, #fff, black, navy). Default #ffffff — white reads well on photos, black on light scans." },
                    "opacity": { "type": "number", "minimum": 0.02, "maximum": 1, "default": 0.3, "description": "Watermark opacity from 0.02 (barely visible) to 1.0 (solid). Default 0.3 — visible enough to deter reuse without hiding the picture." },
                    "angle": { "type": "number", "minimum": -90, "maximum": 90, "default": 30.0, "description": "Rotation of the whole tiled pattern in degrees, -90 to 90. Default 30 (the classic diagonal); 0 draws straight horizontal rows." },
                    "columns": { "type": "integer", "minimum": 1, "maximum": 12, "default": 4, "description": "How many watermark tiles across the image (1-12). Default 4. Higher = denser, harder to crop or clone out." },
                    "rows": { "type": "integer", "minimum": 1, "maximum": 12, "default": 5, "description": "How many watermark tiles down the image (1-12). Default 5. Higher = denser coverage." },
                    "pattern": { "type": "string", "enum": ["grid", "brick"], "default": "brick", "description": "Tile layout: brick (default) offsets alternate rows by half a cell for staggered, checker-wise coverage; grid aligns every row in a plain lattice." },
                    "outline": { "type": "boolean", "default": false, "description": "Add a black outline around each watermark so it stays legible over both light and dark areas. Default false." },
                    "format": { "type": "string", "enum": ["keep", "png", "jpg", "webp"], "default": "keep", "description": "Output container: keep (default) reuses the input's format, or convert to png (lossless), jpg (small, no transparency) or webp." }
                },
                "required": ["text"],
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn converted_output_renames_the_display_filename() {
        assert_eq!(replace_extension("holiday.jpg", "png"), "holiday.png");
        assert_eq!(replace_extension("scan", "webp"), "scan.webp");
    }
}
