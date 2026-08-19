//! gizza-ai/image-to-embedded-array — turn an image into a C/C++ byte array for
//! an embedded TFT / OLED / e-paper display.
//!
//! Pipeline: resolve the image source (URL/ref) → `core::convert` (decode +
//! alpha flatten + resize + pixel packing + source emission) → flat JSON the
//! LLM reads directly.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker. Surfaces:
//! chat + CLI. No standalone page (image input + text report; the no-page
//! file-input pattern shared with image-to-ascii / extract-rgba).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

use gizza_ai_image_to_embedded_array_core::{
    convert, parse_color, BitOrder, ByteOrder, NumberFormat, Options, PixelFormat, Style,
};

const MAX_BYTES: usize = 16 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    format: String,
    #[serde(default)]
    width: u32,
    #[serde(default)]
    height: u32,
    #[serde(default)]
    style: String,
    #[serde(default = "default_name")]
    name: String,
    #[serde(default)]
    byte_order: String,
    #[serde(default)]
    bit_order: String,
    #[serde(default)]
    number_format: String,
    #[serde(default = "default_per_line")]
    per_line: u32,
    #[serde(default = "default_threshold")]
    threshold: u32,
    #[serde(default)]
    dither: bool,
    #[serde(default)]
    invert: bool,
    #[serde(default = "default_background")]
    background: String,
}

fn default_name() -> String {
    "image".into()
}
fn default_per_line() -> u32 {
    12
}
fn default_threshold() -> u32 {
    128
}
fn default_background() -> String {
    "#000000".into()
}

#[derive(Serialize)]
struct Resp {
    /// The generated C/C++ source, ready to paste into firmware.
    code: String,
    /// Which pixel format was emitted.
    format: String,
    /// Emitted image dimensions in pixels (after any resize).
    width: u32,
    height: u32,
    /// Dimensions of the decoded source image.
    src_width: u32,
    src_height: u32,
    /// Flash cost of the pixel data in bytes.
    bytes: usize,
    /// Number of array elements.
    elements: usize,
    /// C type of one array element.
    data_type: String,
    bits_per_pixel: u32,
    /// The array's C identifier (`<name>_bits` for xbm).
    name: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::enumv(
                "format",
                ["rgb565", "bgr565", "rgb888", "rgb332", "grayscale", "mono", "xbm"],
            )
            .default("rgb565")
            .describe(
                "Pixel packing: rgb565 (default, 16-bit 5R-6G-5B — the usual TFT format), bgr565 (red/blue swapped), rgb888 (24-bit, 3 bytes per pixel), rgb332 (8-bit), grayscale (8-bit luminance), mono (1 bit per pixel, rows padded to bytes), or xbm (1-bit X BitMap source file).",
            ),
        )
        .param(
            Param::integer("width")
                .min(0.0)
                .max(4096.0)
                .default(0)
                .describe(
                    "Resize the image to this width in pixels before packing (e.g. 240 for a 240x240 panel). 0 (default) keeps the source width, or derives it from `height` to preserve the aspect ratio.",
                ),
        )
        .param(
            Param::integer("height")
                .min(0.0)
                .max(4096.0)
                .default(0)
                .describe(
                    "Resize the image to this height in pixels before packing. 0 (default) derives the height from `width` and the source aspect ratio; set both to stretch to an exact size.",
                ),
        )
        .param(
            Param::enumv("style", ["array", "arduino", "header", "raw"])
                .default("array")
                .describe(
                    "Declaration wrapped around the numbers: array (default, `static const uint16_t name[] = {…};`), arduino (adds PROGMEM, <pgmspace.h>, and NAME_WIDTH/NAME_HEIGHT defines), header (a full .h file with an include guard and <stdint.h>), or raw (the comma-separated values only). Ignored for format=xbm, which has its own fixed syntax.",
                ),
        )
        .param(
            Param::string("name")
                .default("image")
                .describe(
                    "C identifier for the generated array (letters, digits, and underscores; must not start with a digit). Default \"image\"; xbm emits `<name>_bits` plus `<name>_width`/`<name>_height`.",
                ),
        )
        .param(
            Param::enumv("byte_order", ["word", "big", "little"])
                .default("word")
                .describe(
                    "How each 16-bit pixel reaches the array (rgb565/bgr565 only): word (default, one uint16_t per pixel like 0xF800 — what pushImage/drawRGBBitmap expect), big (two uint8_t, high byte first: 0xF8, 0x00), or little (low byte first: 0x00, 0xF8). Ignored by the 8/24/1-bit formats.",
                ),
        )
        .param(
            Param::enumv("number_format", ["hex", "decimal"])
                .default("hex")
                .describe("How each element is spelled: hex (default, 0xF800) or decimal (63488)."),
        )
        .param(
            Param::integer("per_line")
                .min(0.0)
                .max(64.0)
                .default(12)
                .describe(
                    "Array elements per source line (1-64, default 12). Use 0 to put exactly one image row on each line, which makes the array visually match the picture.",
                ),
        )
        .param(
            Param::integer("threshold")
                .min(0.0)
                .max(255.0)
                .default(128)
                .describe(
                    "Brightness cutoff for the 1-bit formats (mono/xbm), 0-255, default 128. Pixels darker than this become set (ink) bits; raise it to capture more of the image, lower it to keep only the darkest areas.",
                ),
        )
        .param(
            Param::boolean("dither")
                .default(false)
                .describe(
                    "Apply Floyd-Steinberg error diffusion before the 1-bit threshold (mono/xbm only), so photos and gradients keep their shading instead of flattening to solid blocks.",
                ),
        )
        .param(
            Param::boolean("invert")
                .default(false)
                .describe(
                    "Swap ink and background bits for the 1-bit formats (mono/xbm), for panels that draw a 0 bit as lit.",
                ),
        )
        .param(
            Param::enumv("bit_order", ["auto", "msb", "lsb"])
                .default("auto")
                .describe(
                    "Which bit of each byte holds the leftmost pixel of a 1-bit row: auto (default — msb for mono, matching Adafruit GFX; lsb for xbm, matching the XBM spec), msb (leftmost pixel in bit 7), or lsb (leftmost pixel in bit 0).",
                ),
        )
        .param(
            Param::string("background")
                .default("#000000")
                .describe(
                    "Opaque colour that transparent pixels are composited over, as a hex value (#101820 or #fff) or a name (black, white, red, green, blue, magenta). Default #000000, the usual backdrop for a dark panel.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ImageToEmbeddedArray;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/image-to-embedded-array",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert an image into a C/C++ byte array for embedded TFT and OLED displays",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Convert an image into a C/C++ byte array you can paste into embedded firmware for a TFT, OLED, or e-paper display. Choose the pixel packing with `format`: rgb565 (default, the usual 16-bit TFT format), bgr565, rgb888, rgb332, grayscale, mono (1 bit per pixel), or xbm. Resize to the panel with `width`/`height` (either alone preserves the aspect ratio), and pick the declaration with `style`: array, arduino (PROGMEM + width/height defines), header (a full .h file), or raw values. `byte_order` selects uint16_t words or split big/little-endian bytes for the 16-bit formats; `threshold`, `dither`, `invert`, and `bit_order` control the 1-bit formats; `background` is the colour transparent pixels are flattened onto. Returns the source text plus the emitted dimensions, element count, and flash cost in bytes. Output is capped at 1048576 pixels — set `width` to resize a large photo first. Provide the image as either url (HTTP/HTTPS) or ref (id from a prior tool call).",
        parameters = schema_json()
    ),
)]
impl ImageToEmbeddedArray {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("image-to-embedded-array")?;

    let format = PixelFormat::parse(&args.format).map_err(SkillError::InvalidArgs)?;
    let opts = Options {
        format,
        width: args.width,
        height: args.height,
        byte_order: ByteOrder::parse(&args.byte_order).map_err(SkillError::InvalidArgs)?,
        bit_order: BitOrder::parse(&args.bit_order).map_err(SkillError::InvalidArgs)?,
        style: Style::parse(&args.style).map_err(SkillError::InvalidArgs)?,
        number_format: NumberFormat::parse(&args.number_format).map_err(SkillError::InvalidArgs)?,
        name: args.name.clone(),
        per_line: args.per_line,
        threshold: args.threshold.min(255) as u8,
        dither: args.dither,
        invert: args.invert,
        background: parse_color(&args.background).map_err(SkillError::InvalidArgs)?,
    };

    let (bytes, _mime, _name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_BYTES)?;
    let g = convert(&bytes, &opts).map_err(SkillError::InvalidArgs)?;

    let resp = Resp {
        code: g.code,
        format: format.label().to_string(),
        width: g.width,
        height: g.height,
        src_width: g.src_width,
        src_height: g.src_height,
        bytes: g.bytes,
        elements: g.elements,
        data_type: g.data_type,
        bits_per_pixel: g.bits_per_pixel,
        name: args.name,
    };
    serde_json::to_vec(&resp).map_err(|e| {
        SkillError::Serialize(format!("serialize image-to-embedded-array response: {e}"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r##"{
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "format": { "type": "string", "enum": ["rgb565", "bgr565", "rgb888", "rgb332", "grayscale", "mono", "xbm"], "default": "rgb565", "description": "Pixel packing: rgb565 (default, 16-bit 5R-6G-5B — the usual TFT format), bgr565 (red/blue swapped), rgb888 (24-bit, 3 bytes per pixel), rgb332 (8-bit), grayscale (8-bit luminance), mono (1 bit per pixel, rows padded to bytes), or xbm (1-bit X BitMap source file)." },
                    "width": { "type": "integer", "minimum": 0, "maximum": 4096, "default": 0, "description": "Resize the image to this width in pixels before packing (e.g. 240 for a 240x240 panel). 0 (default) keeps the source width, or derives it from `height` to preserve the aspect ratio." },
                    "height": { "type": "integer", "minimum": 0, "maximum": 4096, "default": 0, "description": "Resize the image to this height in pixels before packing. 0 (default) derives the height from `width` and the source aspect ratio; set both to stretch to an exact size." },
                    "style": { "type": "string", "enum": ["array", "arduino", "header", "raw"], "default": "array", "description": "Declaration wrapped around the numbers: array (default, `static const uint16_t name[] = {…};`), arduino (adds PROGMEM, <pgmspace.h>, and NAME_WIDTH/NAME_HEIGHT defines), header (a full .h file with an include guard and <stdint.h>), or raw (the comma-separated values only). Ignored for format=xbm, which has its own fixed syntax." },
                    "name": { "type": "string", "default": "image", "description": "C identifier for the generated array (letters, digits, and underscores; must not start with a digit). Default \"image\"; xbm emits `<name>_bits` plus `<name>_width`/`<name>_height`." },
                    "byte_order": { "type": "string", "enum": ["word", "big", "little"], "default": "word", "description": "How each 16-bit pixel reaches the array (rgb565/bgr565 only): word (default, one uint16_t per pixel like 0xF800 — what pushImage/drawRGBBitmap expect), big (two uint8_t, high byte first: 0xF8, 0x00), or little (low byte first: 0x00, 0xF8). Ignored by the 8/24/1-bit formats." },
                    "number_format": { "type": "string", "enum": ["hex", "decimal"], "default": "hex", "description": "How each element is spelled: hex (default, 0xF800) or decimal (63488)." },
                    "per_line": { "type": "integer", "minimum": 0, "maximum": 64, "default": 12, "description": "Array elements per source line (1-64, default 12). Use 0 to put exactly one image row on each line, which makes the array visually match the picture." },
                    "threshold": { "type": "integer", "minimum": 0, "maximum": 255, "default": 128, "description": "Brightness cutoff for the 1-bit formats (mono/xbm), 0-255, default 128. Pixels darker than this become set (ink) bits; raise it to capture more of the image, lower it to keep only the darkest areas." },
                    "dither": { "type": "boolean", "default": false, "description": "Apply Floyd-Steinberg error diffusion before the 1-bit threshold (mono/xbm only), so photos and gradients keep their shading instead of flattening to solid blocks." },
                    "invert": { "type": "boolean", "default": false, "description": "Swap ink and background bits for the 1-bit formats (mono/xbm), for panels that draw a 0 bit as lit." },
                    "bit_order": { "type": "string", "enum": ["auto", "msb", "lsb"], "default": "auto", "description": "Which bit of each byte holds the leftmost pixel of a 1-bit row: auto (default — msb for mono, matching Adafruit GFX; lsb for xbm, matching the XBM spec), msb (leftmost pixel in bit 7), or lsb (leftmost pixel in bit 0)." },
                    "background": { "type": "string", "default": "#000000", "description": "Opaque colour that transparent pixels are composited over, as a hex value (#101820 or #fff) or a name (black, white, red, green, blue, magenta). Default #000000, the usual backdrop for a dark panel." }
                },
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
