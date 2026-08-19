//! gizza-ai/image-dither — apply Floyd-Steinberg / ordered (Bayer) / Atkinson &
//! friends dithering to an image, against either an image-derived palette or a
//! fixed one (1-bit mono, e-ink grayscale ramps, retro looks, or a custom hex
//! list).
//!
//! `Input::Image` emits the url⊕ref oneOf; `run()` is
//! `resolve_source` → `core::plan` → `dispatch_ffmpeg` → `build_media_envelope`.
//! The whole transform is one `-filter_complex` graph built by the pure `core`
//! crate, which the standalone page shares through the `web` wrapper — so the
//! chat schema, the CLI, and the page can never disagree about what a parameter
//! means.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, format_to_mime_and_ext, mime_to_ext, AssetKind,
    Input, Param, SkillError, SourceFields, ToolDescriptor,
};
// resolve_source / dispatch_ffmpeg call host imports → wasm-only (like run() below).
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use gizza_ai_image_dither_core::{plan, ALGORITHMS, FORMATS, PALETTES};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_BYTES: usize = 16 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "d_algorithm")]
    algorithm: String,
    #[serde(default = "d_palette")]
    palette: String,
    #[serde(default = "d_colors")]
    colors: u32,
    #[serde(default)]
    palette_colors: String,
    #[serde(default = "d_bayer_scale")]
    bayer_scale: u32,
    #[serde(default = "d_pixel_scale")]
    pixel_scale: u32,
    #[serde(default = "d_contrast")]
    contrast: f64,
    #[serde(default = "d_format")]
    format: String,
}

fn d_algorithm() -> String {
    "floyd_steinberg".to_string()
}
fn d_palette() -> String {
    "auto".to_string()
}
fn d_colors() -> u32 {
    16
}
fn d_bayer_scale() -> u32 {
    2
}
fn d_pixel_scale() -> u32 {
    1
}
fn d_contrast() -> f64 {
    1.0
}
fn d_format() -> String {
    "png".to_string()
}

const ALGORITHM_DESC: &str = "Dithering kernel. floyd_steinberg (default) is the classic error-diffusion look. bayer is ordered dithering: a deterministic 8x8 matrix that gives the flat crosshatch pattern used for retro/print looks (pair it with bayer_scale). atkinson diffuses only part of the error for a high-contrast, sparse early-Mac look. burkes, sierra2, sierra3, and sierra2_4a are further error-diffusion kernels, from soft to sharp. heckbert is a simple error diffusion. none skips dithering entirely and snaps each pixel to the nearest palette colour, which shows the banding dithering exists to hide.";
const PALETTE_DESC: &str = "Which palette to dither against. auto (default) derives one from the image itself with `colors` entries. mono is pure 1-bit black and white. gray4 and gray16 are evenly spaced grayscale ramps, matching common e-ink/e-paper displays. green4 is a 4-shade green reflective-LCD ramp and amber2 an amber-on-black terminal look. cga4 is the 4-colour CGA display palette. custom uses the hex list in palette_colors. Single-hue palettes (mono, gray4, gray16, green4, amber2) convert the image to luma first so brightness, not RGB distance, decides each pixel.";
const COLORS_DESC: &str = "Palette size, 2-256 (default 16). Only used when palette=auto: the palette is generated from the image with at most this many colours. Small values (2-8) give the strongest dithered texture; 32+ starts to look like the original. Ignored for every fixed palette, whose size is fixed by the palette itself.";
const PALETTE_COLORS_DESC: &str = "Custom palette as a comma-separated hex list, 2-16 colours, e.g. \"#1b1b1b,#e8e8e8\" or \"000000,ff5555,55ffff,ffffff\". Both #rgb and #rrggbb forms are accepted, with or without the leading #. Only used when palette=custom.";
const BAYER_SCALE_DESC: &str = "Coarseness of the ordered-dither matrix, 0-5 (default 2). 0 is the finest, tightest pattern; 5 is the largest, most visible crosshatch. Only used when algorithm=bayer.";
const PIXEL_SCALE_DESC: &str = "Chunky-pixel factor, 1-16 (default 1 = off). The image is scaled down by this factor with nearest-neighbour, dithered, then scaled back up, so each dithered dot becomes an N x N block. Output dimensions stay the same; use 4-8 for a pixel-art look.";
const CONTRAST_DESC: &str = "Contrast applied before dithering, 0.5-3.0 (default 1.0 = unchanged). Flat or hazy photos dither into mud at small palettes; 1.3-1.8 usually gives a much more legible 1-bit or 4-colour result.";
const FORMAT_DESC: &str = "Output format: png (default, lossless — recommended, since lossy compression smears the single-pixel dither pattern), gif (lossless and small for tiny palettes), webp (written losslessly), jpeg (accepted but will visibly blur the pattern), or same to keep the upload's own format.";

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::enumv("algorithm", ALGORITHMS)
                .default("floyd_steinberg")
                .describe(ALGORITHM_DESC),
        )
        .param(
            Param::enumv("palette", PALETTES)
                .default("auto")
                .describe(PALETTE_DESC),
        )
        .param(
            Param::integer("colors")
                .min(2.0)
                .max(256.0)
                .default(16)
                .describe(COLORS_DESC),
        )
        .param(
            Param::string("palette_colors")
                .default("")
                .describe(PALETTE_COLORS_DESC),
        )
        .param(
            Param::integer("bayer_scale")
                .min(0.0)
                .max(5.0)
                .default(2)
                .describe(BAYER_SCALE_DESC),
        )
        .param(
            Param::integer("pixel_scale")
                .min(1.0)
                .max(16.0)
                .default(1)
                .describe(PIXEL_SCALE_DESC),
        )
        .param(
            Param::number("contrast")
                .min(0.5)
                .max(3.0)
                .default(1.0)
                .describe(CONTRAST_DESC),
        )
        .param(
            Param::enumv("format", FORMATS)
                .default("png")
                .describe(FORMAT_DESC),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

/// Map an output `format` to its mime + extension.
///
/// `block_utils::format_to_mime_and_ext` covers jpeg/png/webp for images but has
/// no GIF entry (no other image block emits one). GIF matters here — a 4- or
/// 16-colour dithered image is exactly what GIF's palette container is good at —
/// so it is mapped locally rather than by widening the shared helper.
fn image_format_to_mime_and_ext(fmt: &str) -> Option<(&'static str, &'static str)> {
    match fmt {
        "gif" => Some(("image/gif", "gif")),
        other => format_to_mime_and_ext(AssetKind::Image, other),
    }
}

#[cfg(target_arch = "wasm32")]
struct ImageDither;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/image-dither",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Dither an image with Floyd-Steinberg, ordered/Bayer or Atkinson against a reduced or fixed palette",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Apply dithering to an image for retro, pixel-art, e-ink or palette-reduction looks. algorithm picks the kernel (floyd_steinberg default, plus bayer/ordered, atkinson, burkes, sierra2, sierra3, sierra2_4a, heckbert, or none for plain quantization). palette picks what to dither against: auto (default, derived from the image with `colors` entries, 2-256), mono (1-bit black and white), gray4/gray16 (e-ink grayscale ramps), green4, amber2, cga4, or custom with a hex list in palette_colors. bayer_scale (0-5) sets the ordered matrix coarseness, pixel_scale (1-16) gives chunky pixel-art dots, and contrast (0.5-3.0) pre-boosts flat photos. format is png (default, lossless) / gif / webp / jpeg / same. Provide the image as either url (HTTP/HTTPS) or ref from a prior tool call.",
        parameters = schema_json()
    ),
)]
impl ImageDither {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Validate args.
    let args: Args = serde_json::from_slice(&body)
        .map_err(|e| SkillError::InvalidArgs(format!("invalid image-dither args: {e}")))?;

    // 2. Resolve source — URL fetch or attachment lookup.
    let (bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_BYTES)?;

    // 3. Build the ffmpeg argv (shared pure core).
    let in_ext = mime_to_ext(&in_mime)
        .ok_or_else(|| SkillError::InvalidArgs(format!("unsupported input mime: {in_mime}")))?;
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) = plan(
        &ffmpeg_in,
        &args.algorithm,
        &args.palette,
        args.colors,
        &args.palette_colors,
        args.bayer_scale,
        args.pixel_scale,
        args.contrast,
        &args.format,
    )
    .map_err(SkillError::InvalidArgs)?;

    // 4. Dispatch to ffmpeg-runtime.
    let output = dispatch_ffmpeg(argv, ffmpeg_in, bytes, ffmpeg_out)?;

    // 5. Envelope — the output mime/extension follow the chosen format.
    let (out_mime, out_ext) = match args.format.as_str() {
        "same" => (in_mime.as_str(), in_ext),
        other => image_format_to_mime_and_ext(other).ok_or_else(|| {
            SkillError::InvalidArgs(format!(
                "format {other:?} not supported (same|png|jpeg|webp|gif)"
            ))
        })?,
    };
    let filename = filename_with_suffix(&in_filename, "-dithered", out_ext);
    let against = if args.palette == "auto" {
        format!("a {}-colour palette derived from the image", args.colors)
    } else {
        format!("the {} palette", args.palette)
    };
    let for_llm = format!(
        "dithered {in_filename} with the {} kernel against {against} ({} bytes, {out_mime})",
        args.algorithm,
        output.len()
    );
    build_media_envelope(&output, out_mime, filename, for_llm, MAX_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match the authored
    /// one, so the LLM-facing surface can't change by accident.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(&format!(
            r#"{{
                "type": "object",
                "properties": {{
                    "url":            {{ "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." }},
                    "ref":            {{ "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." }},
                    "algorithm":      {{ "type": "string", "enum": ["floyd_steinberg","bayer","atkinson","burkes","sierra2","sierra3","sierra2_4a","heckbert","none"], "default": "floyd_steinberg", "description": {alg} }},
                    "palette":        {{ "type": "string", "enum": ["auto","mono","gray4","gray16","green4","amber2","cga4","custom"], "default": "auto", "description": {pal} }},
                    "colors":         {{ "type": "integer", "minimum": 2, "maximum": 256, "default": 16, "description": {col} }},
                    "palette_colors": {{ "type": "string", "default": "", "description": {pc} }},
                    "bayer_scale":    {{ "type": "integer", "minimum": 0, "maximum": 5, "default": 2, "description": {bs} }},
                    "pixel_scale":    {{ "type": "integer", "minimum": 1, "maximum": 16, "default": 1, "description": {ps} }},
                    "contrast":       {{ "type": "number", "minimum": 0.5, "maximum": 3, "default": 1.0, "description": {con} }},
                    "format":         {{ "type": "string", "enum": ["same","png","jpeg","webp","gif"], "default": "png", "description": {fmt} }}
                }},
                "additionalProperties": false,
                "oneOf": [
                    {{ "required": ["url"] }},
                    {{ "required": ["ref"] }}
                ]
            }}"#,
            alg = serde_json::to_string(ALGORITHM_DESC).unwrap(),
            pal = serde_json::to_string(PALETTE_DESC).unwrap(),
            col = serde_json::to_string(COLORS_DESC).unwrap(),
            pc = serde_json::to_string(PALETTE_COLORS_DESC).unwrap(),
            bs = serde_json::to_string(BAYER_SCALE_DESC).unwrap(),
            ps = serde_json::to_string(PIXEL_SCALE_DESC).unwrap(),
            con = serde_json::to_string(CONTRAST_DESC).unwrap(),
            fmt = serde_json::to_string(FORMAT_DESC).unwrap(),
        ))
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    /// Every advertised enum value must be a value the core actually accepts —
    /// a descriptor that offers a choice the graph builder rejects would only
    /// fail at ffmpeg time.
    #[test]
    fn every_advertised_enum_value_builds_a_plan() {
        for alg in ALGORITHMS {
            plan("in.png", alg, "auto", 16, "", 2, 1, 1.0, "png")
                .unwrap_or_else(|e| panic!("algorithm {alg}: {e}"));
        }
        for pal in PALETTES {
            let custom = if pal == "custom" { "#000,#fff" } else { "" };
            plan("in.png", "bayer", pal, 16, custom, 2, 1, 1.0, "png")
                .unwrap_or_else(|e| panic!("palette {pal}: {e}"));
        }
        for fmt in FORMATS {
            plan("in.png", "bayer", "auto", 16, "", 2, 1, 1.0, fmt)
                .unwrap_or_else(|e| panic!("format {fmt}: {e}"));
        }
    }

    #[test]
    fn output_filename_gets_the_dithered_suffix() {
        assert_eq!(
            filename_with_suffix("photo.jpg", "-dithered", "png"),
            "photo-dithered.png"
        );
        assert_eq!(
            filename_with_suffix("photo.png", "-dithered", "png"),
            "photo-dithered.png"
        );
    }

    #[test]
    fn format_values_all_map_to_a_mime_and_extension() {
        for (fmt, mime, ext) in [
            ("png", "image/png", "png"),
            ("jpeg", "image/jpeg", "jpg"),
            ("webp", "image/webp", "webp"),
            ("gif", "image/gif", "gif"),
        ] {
            assert_eq!(
                image_format_to_mime_and_ext(fmt),
                Some((mime, ext)),
                "format {fmt}"
            );
        }
    }
}
