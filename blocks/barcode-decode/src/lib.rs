//! gizza-ai/barcode-decode — read 1D barcodes from an image (URL/ref).
//!
//! Pipeline: resolve the image source (URL/ref) → `core::run` (image + rxing) →
//! JSON report with every detected 1D barcode. Pure Rust, so it runs in chat and
//! CLI; no standalone page because current pages do not support image-in/text-out.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 32 * 1024 * 1024;

#[derive(Deserialize)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default = "default_try_harder")]
    try_harder: bool,
}

fn default_format() -> String {
    "auto".into()
}

fn default_try_harder() -> bool {
    true
}

#[derive(Serialize)]
struct BarcodeResp {
    format: String,
    text: String,
}

#[derive(Serialize)]
struct Resp {
    count: usize,
    barcodes: Vec<BarcodeResp>,
    #[serde(skip_serializing_if = "Option::is_none")]
    filename: Option<String>,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::enumv("format", ["auto", "ean-13", "ean-8", "upc-a", "code-128", "code-39"])
                .default("auto")
                .describe("Barcode symbology to decode. auto (default) tries common 1D formats (EAN-13, EAN-8, UPC-A/UPC-E, Code 128, Code 39, Code 93, Codabar, ITF); choose a specific format to reduce false positives. QR and other 2D codes are out of scope — use qr-decode for QR."),
        )
        .param(
            Param::boolean("try_harder")
                .default(true)
                .describe("Spend extra effort scanning rotations and harder binarizations. Default true; set false for faster decoding of clean, straight scans."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct BarcodeDecode;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/barcode-decode",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Decode 1D barcodes from an image",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Read 1D barcodes from an image (PNG/JPEG/GIF/WebP/BMP) and return every detected code as JSON with format and text. Provide the image as either url (HTTP/HTTPS) or ref from a prior tool call. Params: format=auto|ean-13|ean-8|upc-a|code-128|code-39 (default auto tries common 1D symbologies including UPC-E, Code 93, Codabar, ITF), try_harder=true|false (default true for tougher scans). This tool is 1D-only; use qr-decode for QR codes.",
        parameters = schema_json()
    ),
)]
impl BarcodeDecode {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("barcode-decode")?;
    let (bytes, _mime, filename) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
    let decoded = gizza_ai_barcode_decode_core::run(&bytes, &args.format, args.try_harder)
        .map_err(SkillError::InvalidArgs)?;
    let barcodes = decoded
        .into_iter()
        .map(|d| BarcodeResp {
            format: d.format,
            text: d.text,
        })
        .collect::<Vec<_>>();
    let resp = Resp {
        count: barcodes.len(),
        barcodes,
        filename: (!filename.is_empty()).then_some(filename),
    };
    serde_json::to_vec(&resp)
        .map_err(|e| SkillError::Serialize(format!("serialize barcode-decode response: {e}")))
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
                    "format": { "type": "string", "enum": ["auto", "ean-13", "ean-8", "upc-a", "code-128", "code-39"], "default": "auto", "description": "Barcode symbology to decode. auto (default) tries common 1D formats (EAN-13, EAN-8, UPC-A/UPC-E, Code 128, Code 39, Code 93, Codabar, ITF); choose a specific format to reduce false positives. QR and other 2D codes are out of scope — use qr-decode for QR." },
                    "try_harder": { "type": "boolean", "default": true, "description": "Spend extra effort scanning rotations and harder binarizations. Default true; set false for faster decoding of clean, straight scans." }
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
