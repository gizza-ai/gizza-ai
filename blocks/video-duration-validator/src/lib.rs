//! gizza-ai/video-duration-validator — check a video's container duration
//! against a target length and report PASS/FAIL.
//!
//! URL/ref video validator: the descriptor single-sources the chat schema and
//! the CLI; the handler resolves the video, reads the container duration with
//! the pure-Rust symphonia demuxers (no decoding, no ffmpeg) and returns the
//! verdict as JSON.
//!
//! Pure Rust → runs on ALL backends including the chat Service Worker, and the
//! same core powers the standalone page, where the file never leaves the
//! browser.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::resolve_source;
use gizza_ai_block_utils::{
    AssetKind, Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_video_duration_validator_core::{check, Mode, Options, Report};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 256 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    target_seconds: f64,
    #[serde(default = "d_tolerance")]
    tolerance_seconds: f64,
    #[serde(default = "d_mode")]
    mode: String,
}

fn d_tolerance() -> f64 {
    Options::default().tolerance_seconds
}
fn d_mode() -> String {
    Mode::default().as_str().to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
        .param(
            Param::number("target_seconds")
                .required()
                .min(0.001)
                .max(86400.0)
                .describe("Required. The length the clip is supposed to be, in seconds (e.g. 30 for a 30-second ad, 59 for a Reel, 600 for a 10-minute lecture). Accepts decimals; 0.001-86400 (24 hours)."),
        )
        .param(
            Param::number("tolerance_seconds")
                .min(0.0)
                .max(86400.0)
                .default(0.5)
                .describe("Allowed slack around the target, in seconds (default 0.5). 0 demands an exact match to the millisecond; 0.5 absorbs the usual keyframe/frame-rate rounding. 0-86400."),
        )
        .param(
            Param::enumv("mode", ["within", "max", "min"])
                .default("within")
                .describe("Which comparison to apply: within (default) passes when the duration is inside target ± tolerance; max passes when it is at most target + tolerance (an upper limit, e.g. an ad slot); min passes when it is at least target - tolerance (a lower limit, e.g. a minimum watch time)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

/// Args → core options, so a bad rule fails before the video is fetched.
fn options(args: &Args) -> Result<Options, String> {
    let opts = Options {
        target_seconds: args.target_seconds,
        tolerance_seconds: args.tolerance_seconds,
        mode: Mode::parse(&args.mode)?,
    };
    opts.validate()?;
    Ok(opts)
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-duration-validator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Check a video's duration against a target and flag clips that are too long or too short.",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Check a video's container duration against a target length and report PASS or FAIL. Provide the video as url or ref. Reads the duration straight from the container header with pure-Rust demuxers — no re-encoding, no ffmpeg, nothing is uploaded anywhere. Parameters: target_seconds (required, 0.001-86400) is the intended length; tolerance_seconds (default 0.5) is the allowed slack; mode=within|max|min (default within) selects the comparison — within passes inside target ± tolerance, max passes at or under target + tolerance, min passes at or over target - tolerance. Returns status (PASS/FAIL), pass, reason (ok/too_long/too_short), actual_seconds and actual_duration, target_seconds, tolerance_seconds, delta_seconds (actual - target; positive means too long), overshoot_seconds (how far outside the window), the allowed window, the container, and a one-line summary. Containers: MP4/M4A/MOV, Matroska/WebM, OGG, WAV, AIFF, CAF, FLAC, MP3, AAC/ADTS. A file whose header records no duration (a broken MediaRecorder WebM, a damaged moov atom) is an error, not a FAIL — remux it with video-duration-fix-remux first.",
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
    let args: Args =
        serde_json::from_slice(&body).invalid_args("video-duration-validator")?;
    let opts = options(&args).map_err(SkillError::InvalidArgs)?;
    let (bytes, _mime, _name) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_INPUT_BYTES)?;
    let report = gizza_ai_video_duration_validator_core::validate(&bytes, &opts)
        .map_err(SkillError::InvalidArgs)?;
    serde_json::to_vec(&report).map_err(|e| {
        SkillError::Serialize(format!("serialize video-duration-validator response: {e}"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(json: &str) -> Args {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn defaults_match_the_descriptor() {
        assert_eq!(d_tolerance(), 0.5);
        assert_eq!(d_mode(), "within");
    }

    #[test]
    fn args_parse_from_a_url_plus_target_using_defaults() {
        let a = args(r#"{"url":"https://example.com/ad.mp4","target_seconds":30}"#);
        assert_eq!(a.target_seconds, 30.0);
        assert_eq!(a.tolerance_seconds, 0.5);
        assert_eq!(a.mode, "within");
        let o = options(&a).unwrap();
        assert_eq!(o.target_seconds, 30.0);
        assert_eq!(o.tolerance_seconds, 0.5);
        assert_eq!(o.mode, Mode::Within);
    }

    #[test]
    fn target_seconds_is_mandatory() {
        let e = serde_json::from_str::<Args>(r#"{"url":"https://example.com/a.mp4"}"#).unwrap_err();
        assert!(e.to_string().contains("target_seconds"), "{e}");
    }

    #[test]
    fn every_mode_is_accepted() {
        for m in ["within", "max", "min"] {
            let a = args(&format!(
                r#"{{"url":"https://example.com/a.mp4","target_seconds":60,"mode":"{m}"}}"#
            ));
            assert_eq!(options(&a).unwrap().mode.as_str(), m);
        }
    }

    #[test]
    fn bad_params_are_rejected_before_the_video_is_fetched() {
        let a = args(r#"{"url":"https://example.com/a.mp4","target_seconds":30,"mode":"exact"}"#);
        assert!(options(&a).unwrap_err().contains("within, max, min"));

        let a = args(r#"{"url":"https://example.com/a.mp4","target_seconds":0}"#);
        assert!(options(&a).unwrap_err().contains("target_seconds"));

        let a = args(r#"{"url":"https://example.com/a.mp4","target_seconds":-4}"#);
        assert!(options(&a).unwrap_err().contains("greater than 0"));

        let a = args(
            r#"{"url":"https://example.com/a.mp4","target_seconds":30,"tolerance_seconds":-1}"#,
        );
        assert!(options(&a).unwrap_err().contains("must not be negative"));

        let a = args(r#"{"url":"https://example.com/a.mp4","target_seconds":90000}"#);
        assert!(options(&a).unwrap_err().contains("24 hours"));
    }

    #[test]
    fn the_response_serializes_the_documented_fields() {
        let r: Report = check(31.25, &options(&args(
            r#"{"url":"https://example.com/a.mp4","target_seconds":30}"#,
        )).unwrap(), Some("MP4 / M4A (ISO BMFF)"))
        .unwrap();
        let v: serde_json::Value = serde_json::to_value(&r).unwrap();
        assert_eq!(v["status"], "FAIL");
        assert_eq!(v["pass"], false);
        assert_eq!(v["reason"], "too_long");
        assert_eq!(v["mode"], "within");
        assert_eq!(v["actual_seconds"], 31.25);
        assert_eq!(v["delta_seconds"], 1.25);
        assert_eq!(v["overshoot_seconds"], 0.75);
        assert_eq!(v["allowed_max_seconds"], 30.5);
        assert_eq!(v["container"], "MP4 / M4A (ISO BMFF)");
        assert!(v["summary"].as_str().unwrap().starts_with("FAIL —"));
    }

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "Video URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "target_seconds": { "type": "number", "minimum": 0.001, "maximum": 86400, "description": "Required. The length the clip is supposed to be, in seconds (e.g. 30 for a 30-second ad, 59 for a Reel, 600 for a 10-minute lecture). Accepts decimals; 0.001-86400 (24 hours)." },
                    "tolerance_seconds": { "type": "number", "minimum": 0, "maximum": 86400, "default": 0.5, "description": "Allowed slack around the target, in seconds (default 0.5). 0 demands an exact match to the millisecond; 0.5 absorbs the usual keyframe/frame-rate rounding. 0-86400." },
                    "mode": { "type": "string", "enum": ["within", "max", "min"], "default": "within", "description": "Which comparison to apply: within (default) passes when the duration is inside target ± tolerance; max passes when it is at most target + tolerance (an upper limit, e.g. an ad slot); min passes when it is at least target - tolerance (a lower limit, e.g. a minimum watch time)." }
                },
                "additionalProperties": false,
                "required": ["target_seconds"],
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
