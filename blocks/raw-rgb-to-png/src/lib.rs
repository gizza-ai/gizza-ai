//! gizza-ai/raw-rgb-to-png — assemble raw RGB/RGBA pixel bytes plus width and
//! height into a PNG image. No page: binary image output is returned as a media
//! envelope for chat/CLI download.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::build_media_envelope;
use gizza_ai_block_utils::{Input, Param, SkillError, SkillResultExt, ToolDescriptor};
use gizza_ai_raw_rgb_to_png_core::{assemble, Encoding, PixelFormat};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_OUTPUT_BYTES: usize = 64 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    data: String,
    width: u32,
    height: u32,
    #[serde(default = "default_pixel_format")]
    pixel_format: String,
    #[serde(default = "default_encoding")]
    encoding: String,
    #[serde(default)]
    row_stride: u32,
}

fn default_pixel_format() -> String {
    "rgb".to_string()
}
fn default_encoding() -> String {
    "hex".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data").required().describe(
                "Raw pixel bytes as text. encoding=hex accepts hex byte pairs like ff0000 or 0xff,0x00,0x00; encoding=base64 accepts standard or URL-safe base64 and data:...;base64,... prefixes; encoding=decimal accepts 0-255 byte values separated by commas/spaces. Required.",
            ),
        )
        .param(
            Param::integer("width")
                .required()
                .min(1.0)
                .max(8192.0)
                .describe("Image width in pixels, 1-8192. Required."),
        )
        .param(
            Param::integer("height")
                .required()
                .min(1.0)
                .max(8192.0)
                .describe("Image height in pixels, 1-8192. Required. width × height must be at most 16,000,000 pixels."),
        )
        .param(
            Param::enumv("pixel_format", ["rgb", "rgba"])
                .default("rgb")
                .describe("How to read each pixel: rgb = 3 bytes per pixel (red, green, blue); rgba = 4 bytes per pixel (red, green, blue, alpha). Default rgb."),
        )
        .param(
            Param::enumv("encoding", ["hex", "base64", "decimal"])
                .default("hex")
                .describe("How the data string is encoded: hex byte pairs (default), base64 bytes, or decimal byte values 0-255."),
        )
        .param(
            Param::integer("row_stride")
                .min(0.0)
                .default(0)
                .describe("Bytes from the start of one row to the start of the next when rows are padded. 0 means tightly packed width × bytes-per-pixel. Must be at least one row of pixels when set. Default 0."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct RawRgbToPng;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/raw-rgb-to-png",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Assemble raw RGB/RGBA bytes plus width and height into a PNG",
    skill(
        description = "Assemble raw pixel bytes into a viewable PNG image. Provide data as hex byte pairs, base64, or decimal byte values; provide width and height; choose pixel_format rgb (3 bytes per pixel) or rgba (4 bytes per pixel); optionally set row_stride when rows are padded. The tool validates dimensions, byte counts, stride, and malformed input, then returns an image/png media envelope named raw-rgb.png. It is for headerless framebuffer dumps, texture bytes, embedded raw pixel arrays, and debugging output from image pipelines; it does not decode file formats or infer dimensions.",
        parameters = schema_json()
    ),
)]
impl RawRgbToPng {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("raw-rgb-to-png")?;
    let format = PixelFormat::parse(&args.pixel_format).map_err(SkillError::InvalidArgs)?;
    let encoding = Encoding::parse(&args.encoding).map_err(SkillError::InvalidArgs)?;
    let assembled = assemble(
        &args.data,
        args.width,
        args.height,
        format,
        encoding,
        args.row_stride,
    )
    .map_err(SkillError::InvalidArgs)?;
    let for_llm = format!(
        "assembled a {}x{} {} PNG from {} {} byte(s), row_stride {} ({} PNG bytes)",
        assembled.width,
        assembled.height,
        assembled.format.label(),
        assembled.input_bytes,
        encoding.label(),
        assembled.row_stride,
        assembled.bytes.len()
    );
    build_media_envelope(
        &assembled.bytes,
        "image/png",
        "raw-rgb.png".to_string(),
        for_llm,
        MAX_OUTPUT_BYTES,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type":"object",
                "properties":{
                    "data":{"type":"string","description":"Raw pixel bytes as text. encoding=hex accepts hex byte pairs like ff0000 or 0xff,0x00,0x00; encoding=base64 accepts standard or URL-safe base64 and data:...;base64,... prefixes; encoding=decimal accepts 0-255 byte values separated by commas/spaces. Required."},
                    "width":{"type":"integer","minimum":1,"maximum":8192,"description":"Image width in pixels, 1-8192. Required."},
                    "height":{"type":"integer","minimum":1,"maximum":8192,"description":"Image height in pixels, 1-8192. Required. width × height must be at most 16,000,000 pixels."},
                    "pixel_format":{"type":"string","enum":["rgb","rgba"],"default":"rgb","description":"How to read each pixel: rgb = 3 bytes per pixel (red, green, blue); rgba = 4 bytes per pixel (red, green, blue, alpha). Default rgb."},
                    "encoding":{"type":"string","enum":["hex","base64","decimal"],"default":"hex","description":"How the data string is encoded: hex byte pairs (default), base64 bytes, or decimal byte values 0-255."},
                    "row_stride":{"type":"integer","default":0,"minimum":0,"description":"Bytes from the start of one row to the start of the next when rows are padded. 0 means tightly packed width × bytes-per-pixel. Must be at least one row of pixels when set. Default 0."}
                },
                "required":["data","width","height"],
                "additionalProperties":false
            }"#,
        ).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
