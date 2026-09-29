//! gizza-ai/video-bitrate-checker — report a video's overall and per-stream
//! bitrate and flag it against a configured min/max range.
//!
//! URL/ref video checker: the descriptor single-sources the chat schema and the
//! CLI; the handler resolves the file, demuxes it with the pure-Rust symphonia
//! readers (no decoding, no ffmpeg) and returns the report as JSON.
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
use gizza_ai_video_bitrate_checker_core::{Options, Target, Units, MAX_BITRATE_KBPS};
use serde::Deserialize;
use wafer_sdk::*;

/// The whole file has to sit in the 64 MiB wasm sandbox next to the demuxer's
/// own buffers, so cap well below it and say so rather than trapping.
const MAX_INPUT_BYTES: usize = 32 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    min_bitrate: f64,
    #[serde(default)]
    max_bitrate: f64,
    #[serde(default = "d_units")]
    units: String,
    #[serde(default = "d_target")]
    target: String,
}

fn d_units() -> String {
    Units::default().as_str().to_string()
}
fn d_target() -> String {
    Target::default().as_str().to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
        .param(
            Param::number("min_bitrate")
                .min(0.0)
                .max(MAX_BITRATE_KBPS)
                .default(0.0)
                .describe("Lowest acceptable bitrate, written in units (default 0, which means no floor). Use it to catch over-compressed uploads, e.g. 128 kbps for an audio track or 6 Mbps for 1080p30."),
        )
        .param(
            Param::number("max_bitrate")
                .min(0.0)
                .max(MAX_BITRATE_KBPS)
                .default(0.0)
                .describe("Highest acceptable bitrate, written in units (default 0, which means no ceiling). Use it to enforce a delivery cap, e.g. 2000 kbps for an ad spec or 8 Mbps for a 1080p upload."),
        )
        .param(
            Param::enumv("units", ["kbps", "Mbps"])
                .default("kbps")
                .describe("The unit min_bitrate and max_bitrate are written in: kbps (default, how ad specs and audio tracks are quoted) or Mbps (how 1080p/4K upload guidance is quoted). The report always states both."),
        )
        .param(
            Param::enumv("target", ["overall", "video", "audio"])
                .default("overall")
                .describe("Which bitrate the range applies to: overall (default) is the whole file, size x 8 / duration; video is the video streams summed; audio is the audio streams summed. Every bitrate is reported either way — this only picks the one that is flagged."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

/// Args → core options, so a bad range fails before the file is fetched.
fn options(args: &Args) -> Result<Options, String> {
    let opts = Options {
        min_bitrate: args.min_bitrate,
        max_bitrate: args.max_bitrate,
        units: Units::parse(&args.units)?,
        target: Target::parse(&args.target)?,
    };
    opts.validate()?;
    Ok(opts)
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-bitrate-checker",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Report a video's overall and per-stream bitrate and flag it against a min/max range.",
    requires = ["wafer-run/network"],
    capabilities(network, callable_blocks = ["wafer-run/network"]),
    skill(
        description = "Report a video's overall and per-stream bitrate and flag whether it falls inside a configured min/max range. Provide the video as url or ref. The overall bitrate is file size x 8 / duration; the per-stream numbers are measured by demuxing the container and totalling each track's packet payload, so they are real measurements (matching ffprobe's stream bit_rate) and they work on WebM/Matroska, which stores no per-stream bitrate at all. Nothing is decoded and no ffmpeg is involved. Parameters: min_bitrate and max_bitrate (both default 0 = no bound) set the range; units=kbps|Mbps (default kbps) is the unit those two are written in; target=overall|video|audio (default overall) picks which bitrate the range applies to. With no range set the tool just reports (status INFO). Returns status (PASS/FAIL/INFO), pass, reason (ok/too_low/too_high/not_checked/unmeasurable), the checked bitrate in kbit/s and Mbit/s, the configured bounds, container, duration, file size, overall/video/audio bitrate, container overhead, a one-line summary, and a per-stream list with kind, codec, bitrate, byte share, packet count, picture size, measured frame rate, sample rate and channels. Containers: MP4/MOV/M4A, Matroska/WebM, OGG, WAV, AIFF, CAF, FLAC, MP3, AAC/ADTS. Input is capped at 32 MB because the whole file must fit in the sandbox. A file whose header records no duration cannot be rated - remux it with video-duration-fix-remux first.",
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
    let args: Args = serde_json::from_slice(&body).invalid_args("video-bitrate-checker")?;
    let opts = options(&args).map_err(SkillError::InvalidArgs)?;
    let (bytes, _mime, _name) =
        resolve_source(args.source.into_inner(), AssetKind::Any, MAX_INPUT_BYTES)?;
    let report = gizza_ai_video_bitrate_checker_core::analyze(bytes, &opts)
        .map_err(SkillError::InvalidArgs)?;
    serde_json::to_vec(&report)
        .map_err(|e| SkillError::Serialize(format!("serialize video-bitrate-checker response: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(json: &str) -> Args {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn defaults_match_the_descriptor() {
        assert_eq!(d_units(), "kbps");
        assert_eq!(d_target(), "overall");
        let a = args(r#"{"url":"https://example.com/clip.mp4"}"#);
        assert_eq!(a.min_bitrate, 0.0);
        assert_eq!(a.max_bitrate, 0.0);
        let o = options(&a).unwrap();
        assert_eq!(o, Options::default());
    }

    #[test]
    fn a_url_plus_a_range_parses() {
        let a = args(
            r#"{"url":"https://example.com/clip.mp4","min_bitrate":6,"max_bitrate":9,"units":"Mbps","target":"video"}"#,
        );
        let o = options(&a).unwrap();
        assert_eq!(o.units, Units::Mbps);
        assert_eq!(o.target, Target::Video);
        assert_eq!(o.min_kbps(), 6000.0);
        assert_eq!(o.max_kbps(), 9000.0);
    }

    #[test]
    fn every_enum_choice_is_accepted() {
        for u in ["kbps", "Mbps"] {
            let a = args(&format!(
                r#"{{"url":"https://example.com/a.mp4","units":"{u}"}}"#
            ));
            assert_eq!(options(&a).unwrap().units.as_str(), u);
        }
        for t in ["overall", "video", "audio"] {
            let a = args(&format!(
                r#"{{"url":"https://example.com/a.mp4","target":"{t}"}}"#
            ));
            assert_eq!(options(&a).unwrap().target.as_str(), t);
        }
    }

    #[test]
    fn bad_params_are_rejected_before_the_file_is_fetched() {
        let a = args(r#"{"url":"https://example.com/a.mp4","units":"mbit"}"#);
        assert!(options(&a).unwrap_err().contains("kbps, Mbps"));

        let a = args(r#"{"url":"https://example.com/a.mp4","target":"subtitle"}"#);
        assert!(options(&a).unwrap_err().contains("overall, video, audio"));

        let a = args(r#"{"url":"https://example.com/a.mp4","min_bitrate":-5}"#);
        assert!(options(&a).unwrap_err().contains("must not be negative"));

        let a =
            args(r#"{"url":"https://example.com/a.mp4","min_bitrate":900,"max_bitrate":800}"#);
        assert!(options(&a).unwrap_err().contains("must not be above"));

        let a = args(r#"{"url":"https://example.com/a.mp4","max_bitrate":500000,"units":"Mbps"}"#);
        assert!(options(&a).unwrap_err().contains("check the units"));
    }

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "Video URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "min_bitrate": { "type": "number", "minimum": 0, "maximum": 10000000, "default": 0.0, "description": "Lowest acceptable bitrate, written in units (default 0, which means no floor). Use it to catch over-compressed uploads, e.g. 128 kbps for an audio track or 6 Mbps for 1080p30." },
                    "max_bitrate": { "type": "number", "minimum": 0, "maximum": 10000000, "default": 0.0, "description": "Highest acceptable bitrate, written in units (default 0, which means no ceiling). Use it to enforce a delivery cap, e.g. 2000 kbps for an ad spec or 8 Mbps for a 1080p upload." },
                    "units": { "type": "string", "enum": ["kbps", "Mbps"], "default": "kbps", "description": "The unit min_bitrate and max_bitrate are written in: kbps (default, how ad specs and audio tracks are quoted) or Mbps (how 1080p/4K upload guidance is quoted). The report always states both." },
                    "target": { "type": "string", "enum": ["overall", "video", "audio"], "default": "overall", "description": "Which bitrate the range applies to: overall (default) is the whole file, size x 8 / duration; video is the video streams summed; audio is the audio streams summed. Every bitrate is reported either way — this only picks the one that is flagged." }
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
