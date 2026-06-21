//! gizza-ai/text-to-pdf — generate a clean, paginated PDF from plain text.
//!
//! Pure-Rust (lopdf, built-in Courier font), so it runs on ALL backends incl. the
//! chat Service Worker. The PDF is wrapped as an `application/pdf` data-URL
//! envelope. Surfaces: chat + CLI (text input + PDF bytes output → no page).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::build_media_envelope;
use gizza_ai_block_utils::{Input, Param, SkillError, SkillResultExt, ToolDescriptor};
use gizza_ai_text_to_pdf_core::text_to_pdf;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_OUTPUT_BYTES: usize = 32 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    text: String,
    #[serde(default = "default_font_size")]
    font_size: f64,
    #[serde(default = "default_margin")]
    margin: f64,
}
fn default_font_size() -> f64 {
    11.0
}
fn default_margin() -> f64 {
    72.0
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("text").required().describe("The plain text to render into a PDF."))
        .param(
            Param::number("font_size")
                .min(4.0)
                .max(96.0)
                .describe("Font size in points (default 11). Uses the built-in Courier monospace font."),
        )
        .param(
            Param::number("margin")
                .min(0.0)
                .max(300.0)
                .describe("Page margin in points (default 72 = 1 inch). 72 points = 1 inch."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct TextToPdf;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/text-to-pdf",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate a paginated PDF from plain text",
    skill(
        description = "Generate a clean, paginated PDF (US Letter) from plain text. Long lines wrap to the text width and the content flows across as many pages as needed, using the built-in Courier monospace font. font_size (points, default 11) and margin (points, default 72 = 1 inch) are configurable. Returns a PDF. Runs locally — the text never leaves the device.",
        parameters = schema_json()
    ),
)]
impl TextToPdf {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("text-to-pdf")?;
    let pdf = text_to_pdf(&args.text, args.font_size, args.margin).map_err(SkillError::InvalidArgs)?;
    build_media_envelope(
        &pdf,
        "application/pdf",
        "text.pdf".to_string(),
        format!("rendered text to a PDF ({} bytes)", pdf.len()),
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
                "type": "object",
                "properties": {
                    "text":      { "type": "string", "description": "The plain text to render into a PDF." },
                    "font_size": { "type": "number", "minimum": 4, "maximum": 96, "description": "Font size in points (default 11). Uses the built-in Courier monospace font." },
                    "margin":    { "type": "number", "minimum": 0, "maximum": 300, "description": "Page margin in points (default 72 = 1 inch). 72 points = 1 inch." }
                },
                "required": ["text"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
