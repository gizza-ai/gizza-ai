//! gizza-ai/document-skew-detector — estimate the skew (rotation) angle of a
//! scanned document or text image and report the correction needed to deskew it.
//! Pure Rust image analysis; chat + CLI text/JSON report (no standalone page).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_document_skew_detector_core::{detect, SkewResult};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 32 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    threshold: f64,
    #[serde(default = "d_max_angle")]
    max_angle: f64,
    #[serde(default = "d_tolerance")]
    tolerance: f64,
}

fn d_max_angle() -> f64 {
    15.0
}
fn d_tolerance() -> f64 {
    0.5
}

#[derive(Serialize)]
struct Resp {
    angle_degrees: f64,
    suggested_rotation_degrees: f64,
    direction: String,
    is_straight: bool,
    confidence: f64,
    threshold_used: u8,
    ink_pixels: u64,
    note: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::integer("threshold")
                .min(0.0)
                .max(99.0)
                .default(0)
                .describe("Ink/paper brightness cutoff as a percentage 0-99: pixels darker than this percent of full white count as text. 0 (default) picks the threshold automatically (Otsu). Try 40-60 for faint pencil or low-contrast scans."),
        )
        .param(
            Param::number("max_angle")
                .min(1.0)
                .max(45.0)
                .default(15.0)
                .describe("Maximum absolute skew, in degrees, to search for (default 15, range 1-45). Typical scanner-feed skew is under 5; raise toward 45 for badly rotated photos of pages."),
        )
        .param(
            Param::number("tolerance")
                .min(0.0)
                .max(10.0)
                .default(0.5)
                .describe("Angle in degrees considered already straight; |angle| at or below this reports is_straight=true (default 0.5)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct DocumentSkewDetector;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/document-skew-detector",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Estimate the skew angle of a scanned document or text image and suggest the deskew rotation",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Estimate the skew (rotation) angle of a scanned document or text-heavy image using text-line projection analysis, so the page can be deskewed. Works on pages with no ruled lines: it keys on the text lines themselves. Params: threshold = ink brightness cutoff percent 0-99 (0 = automatic Otsu, the default), max_angle = search range in degrees (1-45, default 15), tolerance = degrees considered already straight (0-10, default 0.5). Returns JSON with angle_degrees (positive = clockwise, text lines' right end lower), suggested_rotation_degrees (apply this with rotate-image to deskew), direction (straight|clockwise|counterclockwise|undetermined), is_straight, confidence (0-1), threshold_used (0-255 gray level), and ink_pixels. Provide the image as either url (HTTP/HTTPS) or ref from a prior tool call. For photo horizons use image-horizon-tilt-checker instead; for perspective correction of phone photos use document-scan.",
        parameters = schema_json()
    ),
)]
impl DocumentSkewDetector {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("document-skew-detector")?;
    let (bytes, _mime, _name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
    let result = detect(&bytes, args.threshold, args.max_angle, args.tolerance)
        .map_err(SkillError::InvalidArgs)?;
    let resp = response(result);
    serde_json::to_vec(&resp)
        .map_err(|e| SkillError::Serialize(format!("serialize document-skew-detector response: {e}")))
}

fn response(result: SkewResult) -> Resp {
    let note = if result.direction == "undetermined" {
        "No confident text-line structure was found (blank page or too little ink); try an explicit threshold (e.g. 50) or a higher-resolution scan.".to_string()
    } else if result.is_straight {
        format!(
            "Already straight within tolerance; suggested rotation is {}°.",
            result.suggested_rotation
        )
    } else {
        format!(
            "Apply suggested_rotation_degrees ({}°) with rotate-image to deskew the page.",
            result.suggested_rotation
        )
    };
    Resp {
        angle_degrees: result.angle,
        suggested_rotation_degrees: result.suggested_rotation,
        direction: result.direction.to_string(),
        is_straight: result.is_straight,
        confidence: result.confidence,
        threshold_used: result.threshold_used,
        ink_pixels: result.ink_pixels,
        note,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_args() -> Args {
        serde_json::from_str(r#"{"url":"https://example.com/scan.png"}"#).unwrap()
    }

    #[test]
    fn defaults_applied() {
        let a = default_args();
        assert_eq!(a.threshold, 0.0);
        assert_eq!(a.max_angle, 15.0);
        assert_eq!(a.tolerance, 0.5);
    }

    #[test]
    fn response_contains_correction_note() {
        let r = SkewResult {
            angle: 2.75,
            suggested_rotation: -2.75,
            direction: "clockwise",
            is_straight: false,
            confidence: 0.83,
            threshold_used: 142,
            ink_pixels: 51234,
        };
        let resp = response(r);
        assert_eq!(resp.angle_degrees, 2.75);
        assert_eq!(resp.suggested_rotation_degrees, -2.75);
        assert!(resp.note.contains("rotate-image"));
    }

    #[test]
    fn undetermined_note_suggests_threshold() {
        let r = SkewResult {
            angle: 0.0,
            suggested_rotation: 0.0,
            direction: "undetermined",
            is_straight: false,
            confidence: 0.0,
            threshold_used: 250,
            ink_pixels: 12,
        };
        let resp = response(r);
        assert!(resp.note.contains("threshold"));
    }

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "threshold": { "type": "integer", "minimum": 0, "maximum": 99, "default": 0, "description": "Ink/paper brightness cutoff as a percentage 0-99: pixels darker than this percent of full white count as text. 0 (default) picks the threshold automatically (Otsu). Try 40-60 for faint pencil or low-contrast scans." },
                    "max_angle": { "type": "number", "minimum": 1, "maximum": 45, "default": 15.0, "description": "Maximum absolute skew, in degrees, to search for (default 15, range 1-45). Typical scanner-feed skew is under 5; raise toward 45 for badly rotated photos of pages." },
                    "tolerance": { "type": "number", "minimum": 0, "maximum": 10, "default": 0.5, "description": "Angle in degrees considered already straight; |angle| at or below this reports is_straight=true (default 0.5)." }
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
