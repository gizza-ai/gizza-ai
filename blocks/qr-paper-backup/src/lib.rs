//! gizza-ai/qr-paper-backup — render printable numbered QR sheets for offline
//! paper archival. Chat schema single-sourced from descriptor(); handle()
//! delegates to run_skill. Pure → all backends, no host calls.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    input_encoding: String,
    #[serde(default)]
    chunk_bytes: u32,
    #[serde(default)]
    columns: u32,
    #[serde(default)]
    error_correction: String,
    #[serde(default = "default_true")]
    show_text: bool,
}

fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The secret or file bytes to archive. Use input_encoding=text for plain text, base64 for a file encoded with base64, or hex for hexadecimal bytes."),
        )
        .param(
            Param::enumv("input_encoding", ["text", "base64", "hex"])
                .default("text")
                .describe("How to decode input before chunking. 'text' stores UTF-8 text directly; 'base64' accepts whitespace-tolerant Base64 file bytes; 'hex' accepts hexadecimal bytes."),
        )
        .param(
            Param::integer("chunk_bytes")
                .default(300)
                .min(50.0)
                .max(1200.0)
                .describe("Maximum raw bytes per QR code before Base64 wrapping (50-1200). Smaller chunks scan more reliably; larger chunks use fewer codes. Default 300."),
        )
        .param(
            Param::integer("columns")
                .default(3)
                .min(1.0)
                .max(5.0)
                .describe("Number of QR codes per row in the printable SVG sheet (1-5). Default 3."),
        )
        .param(
            Param::enumv("error_correction", ["L", "M", "Q", "H"])
                .default("M")
                .describe("QR error-correction level. L is densest, M is default, Q/H add more redundancy but reduce capacity."),
        )
        .param(
            Param::boolean("show_text")
                .default(true)
                .describe("Print the exact QR payload line under each code as a human-readable fallback. Default true."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/qr-paper-backup",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Render a printable sheet of numbered QR codes for offline paper backups",
    skill(
        description = "Encode text or pasted file bytes into a deterministic printable SVG sheet of numbered QR codes for offline paper archival. The tool decodes input as text, base64, or hex; splits the raw bytes into chunk_bytes-sized parts; embeds each QR payload as QRB1|index|total|id|base64-chunk where id is the first 8 hex chars of the full SHA-256; and prints restore instructions plus optional human-readable payload lines. Tune columns for layout and error_correction (L/M/Q/H) for scan robustness. Restore by scanning all parts, sorting by index, concatenating the base64 chunks, and Base64-decoding. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "qr-paper-backup", |a: Args| {
            gizza_ai_qr_paper_backup_core::run(
                &a.input,
                &a.input_encoding,
                a.chunk_bytes,
                a.columns,
                &a.error_correction,
                a.show_text,
            )
            .map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
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
                    "input": { "type": "string", "description": "The secret or file bytes to archive. Use input_encoding=text for plain text, base64 for a file encoded with base64, or hex for hexadecimal bytes." },
                    "input_encoding": { "type": "string", "enum": ["text", "base64", "hex"], "default": "text", "description": "How to decode input before chunking. 'text' stores UTF-8 text directly; 'base64' accepts whitespace-tolerant Base64 file bytes; 'hex' accepts hexadecimal bytes." },
                    "chunk_bytes": { "type": "integer", "default": 300, "minimum": 50, "maximum": 1200, "description": "Maximum raw bytes per QR code before Base64 wrapping (50-1200). Smaller chunks scan more reliably; larger chunks use fewer codes. Default 300." },
                    "columns": { "type": "integer", "default": 3, "minimum": 1, "maximum": 5, "description": "Number of QR codes per row in the printable SVG sheet (1-5). Default 3." },
                    "error_correction": { "type": "string", "enum": ["L", "M", "Q", "H"], "default": "M", "description": "QR error-correction level. L is densest, M is default, Q/H add more redundancy but reduce capacity." },
                    "show_text": { "type": "boolean", "default": true, "description": "Print the exact QR payload line under each code as a human-readable fallback. Default true." }
                },
                "required": ["input"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
