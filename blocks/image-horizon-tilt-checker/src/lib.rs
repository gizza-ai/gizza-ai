//! gizza-ai/image-horizon-tilt-checker — detect the tilt angle of a photo's
//! horizon or dominant vertical lines and report the correction needed to level it.
//! Pure Rust image analysis; chat + CLI text/JSON report (no standalone page).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor};
use gizza_ai_image_horizon_tilt_checker_core::{detect, Reference, TiltResult};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 32 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default = "d_reference")]
    reference: String,
    #[serde(default = "d_max_angle")]
    max_angle: f64,
    #[serde(default = "d_tolerance")]
    tolerance: f64,
}

fn d_reference() -> String {
    "horizon".into()
}
fn d_max_angle() -> f64 {
    15.0
}
fn d_tolerance() -> f64 {
    1.0
}

#[derive(Serialize)]
struct Resp {
    angle_degrees: f64,
    suggested_rotation_degrees: f64,
    reference: String,
    direction: String,
    is_level: bool,
    confidence: f64,
    edges_analyzed: u64,
    note: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(
            Param::enumv("reference", ["horizon", "vertical"])
                .default("horizon")
                .describe("Reference structure to analyze: horizon for near-horizontal lines, vertical for upright architecture or poles (default horizon)."),
        )
        .param(
            Param::number("max_angle")
                .min(1.0)
                .max(45.0)
                .default(15.0)
                .describe("Maximum absolute tilt, in degrees, to search around the selected reference axis (default 15, range 1–45)."),
        )
        .param(
            Param::number("tolerance")
                .min(0.0)
                .max(10.0)
                .default(1.0)
                .describe("Angle in degrees considered already level; |angle| at or below this reports is_level=true (default 1)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ImageHorizonTiltChecker;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/image-horizon-tilt-checker",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Detect a photo horizon or dominant vertical-line tilt and suggest a leveling rotation",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Analyze an image and report the tilt angle of its dominant horizon (near-horizontal line) or vertical structure so the photo can be leveled. Params: reference=horizon|vertical (default horizon), max_angle in degrees (1–45, default 15), tolerance in degrees considered already level (0–10, default 1). Returns JSON with angle_degrees (positive = clockwise / right side lower), suggested_rotation_degrees (apply this with rotate-image to level), direction, is_level, confidence, and edges_analyzed. Provide the image as either url (HTTP/HTTPS) or ref from a prior tool call.",
        parameters = schema_json()
    ),
)]
impl ImageHorizonTiltChecker {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("image-horizon-tilt-checker")?;
    let reference = Reference::parse(&args.reference).map_err(SkillError::InvalidArgs)?;
    let (bytes, _mime, _name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
    let result = detect(&bytes, reference, args.max_angle, args.tolerance)
        .map_err(SkillError::InvalidArgs)?;
    let resp = response(result);
    serde_json::to_vec(&resp).map_err(|e| {
        SkillError::Serialize(format!("serialize image-horizon-tilt-checker response: {e}"))
    })
}

fn response(result: TiltResult) -> Resp {
    let note = if result.direction == "undetermined" {
        "No confident in-range dominant line was found; try a higher-contrast image or larger max_angle.".to_string()
    } else if result.is_level {
        format!("Already level within tolerance; suggested rotation is {}°.", result.suggested_rotation)
    } else {
        format!("Apply suggested_rotation_degrees ({}°) with rotate-image to level the image.", result.suggested_rotation)
    };
    Resp {
        angle_degrees: result.angle,
        suggested_rotation_degrees: result.suggested_rotation,
        reference: result.reference.to_string(),
        direction: result.direction.to_string(),
        is_level: result.is_level,
        confidence: result.confidence,
        edges_analyzed: result.edges_analyzed,
        note,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_args() -> Args {
        Args {
            source: serde_json::from_str(r#"{"url":"https://example.com/photo.png"}"#).unwrap(),
            reference: d_reference(),
            max_angle: d_max_angle(),
            tolerance: d_tolerance(),
        }
    }

    #[test]
    fn reference_defaults_and_validation() {
        let a = default_args();
        assert_eq!(Reference::parse(&a.reference).unwrap(), Reference::Horizon);
        assert_eq!(a.max_angle, 15.0);
        assert_eq!(a.tolerance, 1.0);
        assert!(Reference::parse("diagonal").is_err());
    }

    #[test]
    fn response_contains_correction_note() {
        let r = TiltResult {
            angle: 3.4,
            suggested_rotation: -3.4,
            reference: "horizon",
            direction: "clockwise",
            confidence: 0.71,
            is_level: false,
            edges_analyzed: 4120,
        };
        let resp = response(r);
        assert_eq!(resp.angle_degrees, 3.4);
        assert_eq!(resp.suggested_rotation_degrees, -3.4);
        assert!(resp.note.contains("rotate-image"));
    }

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "Image URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "reference": { "type": "string", "enum": ["horizon", "vertical"], "default": "horizon", "description": "Reference structure to analyze: horizon for near-horizontal lines, vertical for upright architecture or poles (default horizon)." },
                    "max_angle": { "type": "number", "minimum": 1, "maximum": 45, "default": 15.0, "description": "Maximum absolute tilt, in degrees, to search around the selected reference axis (default 15, range 1–45)." },
                    "tolerance": { "type": "number", "minimum": 0, "maximum": 10, "default": 1.0, "description": "Angle in degrees considered already level; |angle| at or below this reports is_level=true (default 1)." }
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
