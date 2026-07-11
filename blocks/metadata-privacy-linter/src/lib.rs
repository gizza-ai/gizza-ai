//! gizza-ai/metadata-privacy-linter — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Pure — no host calls.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    image_base64: String,
    #[serde(default = "default_min_risk")]
    min_risk: String,
    #[serde(default)]
    reveal_values: bool,
    #[serde(default = "default_output")]
    output: String,
}

fn default_min_risk() -> String {
    "all".to_string()
}
fn default_output() -> String {
    "report".to_string()
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("image_base64")
                .required()
                .describe("Image bytes as base64 text, or a data URL such as data:image/jpeg;base64,... . JPEG, PNG, TIFF, WebP, HEIF, and GIF containers are recognized; EXIF/XMP/IPTC metadata is scanned when present."),
        )
        .param(
            Param::enumv("min_risk", ["all", "medium", "high"])
                .default("all")
                .describe("Minimum risk level to include: all (low+medium+high), medium, or high. Default all."),
        )
        .param(
            Param::boolean("reveal_values")
                .default(false)
                .describe("Include the actual metadata values (GPS coordinates, owner names, serials, software). Default false so the report itself is safe to share."),
        )
        .param(
            Param::enumv("output", ["report", "json"])
                .default("report")
                .describe("Output format: report (plain-English text) or json (structured report). Default report."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/metadata-privacy-linter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Scan image metadata for privacy leaks before sharing.",
    skill(
        description = "Scan image metadata for privacy-sensitive fields before sharing. Provide image_base64 as raw base64 bytes or a data URL. The tool detects JPEG/PNG/TIFF/WebP/HEIF/GIF containers and scans EXIF/TIFF, XMP, and IPTC/IIM metadata for GPS coordinates, camera or lens serials, owner/creator names, copyright, capture timestamps, editing software, descriptions and keywords. min_risk filters findings (all/medium/high). reveal_values defaults false so the report omits concrete metadata values and is safe to share; set true to include values and decoded GPS/map links. output is report or json. Pure and deterministic; no upload or network.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "metadata-privacy-linter", |a: Args| {
            gizza_ai_metadata_privacy_linter_core::run(
                &a.image_base64,
                &a.min_risk,
                a.reveal_values,
                &a.output,
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
                    "image_base64": { "type": "string", "description": "Image bytes as base64 text, or a data URL such as data:image/jpeg;base64,... . JPEG, PNG, TIFF, WebP, HEIF, and GIF containers are recognized; EXIF/XMP/IPTC metadata is scanned when present." },
                    "min_risk": { "type": "string", "enum": ["all", "medium", "high"], "default": "all", "description": "Minimum risk level to include: all (low+medium+high), medium, or high. Default all." },
                    "reveal_values": { "type": "boolean", "default": false, "description": "Include the actual metadata values (GPS coordinates, owner names, serials, software). Default false so the report itself is safe to share." },
                    "output": { "type": "string", "enum": ["report", "json"], "default": "report", "description": "Output format: report (plain-English text) or json (structured report). Default report." }
                },
                "required": ["image_base64"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
