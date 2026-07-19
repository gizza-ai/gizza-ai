//! gizza-ai/video-speech-segment-detector — fetch a video URL or attachment
//! ref, run energy-based voice-activity detection on its audio track (ffmpeg
//! `silencedetect`, optionally band-passed to the 200–3000 Hz voice band), and
//! return the speech / non-speech segment timestamps as a TEXT report — no
//! transcription, no re-encode, the video itself is never modified.
//!
//! Single detect pass (`-f null -`): the result is the ffmpeg log, which the
//! shared `core` parses into a labeled timeline and renders as a human report,
//! CSV, SRT, or an Audacity label track. Same pass-1 architecture as
//! `video-silence-cut` (whose core this one reuses); that tool cuts the video,
//! this one reports the timestamps for editors/scripts to consume.
//!
//! The chat schema is derived from `descriptor()` (single source — shared with
//! the CLI). Like every ffmpeg tool, the browser chat runtime can't run ffmpeg
//! in a Service Worker — the verified surfaces are the standalone page + CLI.

// The #[wafer_block] macro emits the impl gated to wasm32 (it generates a
// native registration call that requires ::new()). Supporting imports and the
// Args type are only used inside that impl; `descriptor()`/`schema_json()`
// stay native-compilable so the drift-guard test below can exercise them.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{
    filename_with_suffix, mime_to_ext, AssetKind, Input, Param, SkillError, SkillResultExt,
    SourceFields, ToolDescriptor,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg_runtime, resolve_source, FfmpegReq, FfmpegResp};
use gizza_ai_video_speech_segment_detector_core as core;
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 10 * 1024 * 1024; // 10 MiB (video-tool family cap)

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    threshold_db: Option<f64>,
    #[serde(default)]
    min_silence: Option<f64>,
    #[serde(default)]
    min_speech: Option<f64>,
    #[serde(default)]
    pad: Option<f64>,
    #[serde(default)]
    voice_band: Option<bool>,
    #[serde(default)]
    segments: Option<String>,
    #[serde(default)]
    output: Option<String>,
}

/// Single-source param descriptor → chat schema (and CLI). `Input::Video` adds
/// the `url`⊕`ref` `oneOf`.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
        .param(
            Param::number("threshold_db")
                .min(-90.0)
                .max(0.0)
                .default(-30.0)
                .describe("Loudness threshold in dB below which audio counts as non-speech (default -30). Raise it (e.g. -20) for noisy recordings so hiss stops counting as speech; lower it (e.g. -45) for quiet speakers."),
        )
        .param(
            Param::number("min_silence")
                .min(0.05)
                .max(30.0)
                .default(0.5)
                .describe("Shortest pause in seconds that splits two speech segments (default 0.5). Smaller values split on every breath; larger values keep whole sentences together."),
        )
        .param(
            Param::number("min_speech")
                .min(0.0)
                .max(30.0)
                .default(0.25)
                .describe("Shortest burst in seconds that still counts as speech (default 0.25). Detected bursts shorter than this — clicks, coughs, door slams — are folded back into the surrounding non-speech."),
        )
        .param(
            Param::number("pad")
                .min(0.0)
                .max(5.0)
                .default(0.0)
                .describe("Seconds added before and after every speech segment (default 0). Use 0.1–0.3 to protect soft word edges when the timestamps drive a cutter; padded segments that overlap are merged."),
        )
        .param(
            Param::boolean("voice_band")
                .default(true)
                .describe("Measure loudness only in the 200–3000 Hz voice band (default true), so low rumble and high hiss don't count as speech. Disable to detect any sound (music, effects)."),
        )
        .param(
            Param::enumv("segments", ["both", "speech", "non-speech"])
                .default("both")
                .describe("Which segments to list: both (full labeled timeline, default), speech only, or non-speech only. The report's summary always covers both classes."),
        )
        .param(
            Param::enumv("output", ["report", "csv", "srt", "audacity"])
                .default("report")
                .describe("Output format: report (human-readable summary + numbered list, default), csv (start,end,duration,label), srt (subtitle-timed segments, handy as a transcription scaffold), or audacity (tab-separated label track importable via its label-track import)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[derive(Serialize)]
struct Resp {
    /// The rendered output in the requested `output` format.
    report: String,
    output: String,
    segments: String,
    filename: String,
    duration_seconds: f64,
    speech_segments: usize,
    non_speech_segments: usize,
    speech_seconds: f64,
    non_speech_seconds: f64,
    speech_percent: f64,
}

#[cfg(target_arch = "wasm32")]
struct VideoSpeechSegmentDetector;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-speech-segment-detector",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Detect speech vs non-speech segments in a video and report the timestamps",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Run voice-activity detection on a video's audio track and report the timestamps of speech vs non-speech segments (no transcription; the video is not modified). Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). threshold_db is the loudness cutoff in dB (default -30), min_silence the shortest pause that splits segments (default 0.5s), min_speech the shortest burst kept as speech (default 0.25s), pad extra seconds kept around each speech segment (default 0), voice_band restricts detection to the 200-3000 Hz band (default true). segments picks both/speech/non-speech rows; output picks report/csv/srt/audacity. Energy-based (ffmpeg silencedetect), not an ML transcriber.",
        parameters = schema_json()
    ),
)]
impl VideoSpeechSegmentDetector {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

/// Run the single detect exec and return the full response (the result is the
/// LOG, not output bytes). A missing-audio-stream failure maps to a clear
/// user-facing message; other non-zero exits surface the log tail.
#[cfg(target_arch = "wasm32")]
fn run_detect(argv: Vec<String>, in_name: &str, in_bytes: Vec<u8>) -> Result<FfmpegResp, SkillError> {
    let req = FfmpegReq {
        args: argv,
        inputs: vec![(in_name.to_string(), in_bytes)],
        output: "detect.null".to_string(),
    };
    let req_body = serde_json::to_vec(&req)
        .map_err(|e| SkillError::Serialize(format!("serialize ffmpeg request: {e}")))?;
    let resp_bytes = dispatch_ffmpeg_runtime(&req_body)?;
    let resp: FfmpegResp = serde_json::from_slice(&resp_bytes)
        .map_err(|e| SkillError::Serialize(format!("malformed ffmpeg response: {e}")))?;
    if resp.exit_code != 0 {
        if core::no_audio_error(&resp.log) {
            return Err(SkillError::InvalidArgs(
                "the file has no audio track to analyze — speech detection needs an audio stream"
                    .into(),
            ));
        }
        return Err(SkillError::FfmpegExitNonZero {
            exit: resp.exit_code,
            snippet: resp.log.chars().take(200).collect(),
        });
    }
    Ok(resp)
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Parse + validate args.
    let args: Args = serde_json::from_slice(&body).invalid_args("video-speech-segment-detector")?;
    let threshold_db = args.threshold_db.unwrap_or(core::DEFAULT_THRESHOLD_DB);
    let min_silence = args.min_silence.unwrap_or(core::DEFAULT_MIN_SILENCE_S);
    let min_speech = args.min_speech.unwrap_or(core::DEFAULT_MIN_SPEECH_S);
    let pad = args.pad.unwrap_or(core::DEFAULT_PAD_S);
    let voice_band = args.voice_band.unwrap_or(true);
    core::validate(threshold_db, min_silence, min_speech, pad)
        .map_err(|e| SkillError::InvalidArgs(format!("invalid video-speech-segment-detector args: {e}")))?;
    let filter = core::SegmentFilter::parse(args.segments.as_deref().unwrap_or("both"))
        .map_err(SkillError::InvalidArgs)?;
    let format = core::OutputFormat::parse(args.output.as_deref().unwrap_or("report"))
        .map_err(SkillError::InvalidArgs)?;

    // 2. Resolve source.
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp4");
    let ffmpeg_in = format!("in.{in_ext}");

    // 3. Detect pass — the result is the log.
    let detect = run_detect(
        core::detect_argv(&ffmpeg_in, threshold_db, min_silence, voice_band),
        &ffmpeg_in,
        input_bytes,
    )?;
    let (duration, silences) = core::parse_log(&detect.log).map_err(|e| {
        SkillError::FfmpegExitNonZero { exit: 0, snippet: e }
    })?;

    // 4. Segment + render.
    let segs = core::segments(duration, &silences, min_speech, pad);
    let report = core::render(&segs, duration, filter, format);
    let round3 = |v: f64| (v * 1000.0).round() / 1000.0;
    let speech_seconds = core::speech_seconds(&segs);
    let non_speech_seconds: f64 = segs
        .iter()
        .filter(|s| !s.speech)
        .fold(0.0, |a, s| a + s.duration());
    let resp = Resp {
        report,
        output: args.output.unwrap_or_else(|| "report".into()),
        segments: args.segments.unwrap_or_else(|| "both".into()),
        filename: filename_with_suffix(&in_filename, "-speech-segments", format.extension()),
        duration_seconds: round3(duration),
        speech_segments: segs.iter().filter(|s| s.speech).count(),
        non_speech_segments: segs.iter().filter(|s| !s.speech).count(),
        speech_seconds: round3(speech_seconds),
        non_speech_seconds: round3(non_speech_seconds),
        speech_percent: if duration > 0.0 {
            round3(100.0 * speech_seconds / duration)
        } else {
            0.0
        },
    };
    serde_json::to_vec(&resp).map_err(|e| SkillError::Serialize(format!("serialize response: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema equals the authored
    /// schema below (the exact JSON the chat surface consumes). Number defaults
    /// serialize as floats (`-30.0`), integers as integers — keep them as-is.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "Video URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "threshold_db": { "type": "number", "minimum": -90, "maximum": 0, "default": -30.0, "description": "Loudness threshold in dB below which audio counts as non-speech (default -30). Raise it (e.g. -20) for noisy recordings so hiss stops counting as speech; lower it (e.g. -45) for quiet speakers." },
                    "min_silence": { "type": "number", "minimum": 0.05, "maximum": 30, "default": 0.5, "description": "Shortest pause in seconds that splits two speech segments (default 0.5). Smaller values split on every breath; larger values keep whole sentences together." },
                    "min_speech": { "type": "number", "minimum": 0, "maximum": 30, "default": 0.25, "description": "Shortest burst in seconds that still counts as speech (default 0.25). Detected bursts shorter than this — clicks, coughs, door slams — are folded back into the surrounding non-speech." },
                    "pad": { "type": "number", "minimum": 0, "maximum": 5, "default": 0.0, "description": "Seconds added before and after every speech segment (default 0). Use 0.1–0.3 to protect soft word edges when the timestamps drive a cutter; padded segments that overlap are merged." },
                    "voice_band": { "type": "boolean", "default": true, "description": "Measure loudness only in the 200–3000 Hz voice band (default true), so low rumble and high hiss don't count as speech. Disable to detect any sound (music, effects)." },
                    "segments": { "type": "string", "enum": ["both", "speech", "non-speech"], "default": "both", "description": "Which segments to list: both (full labeled timeline, default), speech only, or non-speech only. The report's summary always covers both classes." },
                    "output": { "type": "string", "enum": ["report", "csv", "srt", "audacity"], "default": "report", "description": "Output format: report (human-readable summary + numbered list, default), csv (start,end,duration,label), srt (subtitle-timed segments, handy as a transcription scaffold), or audacity (tab-separated label track importable via its label-track import)." }
                },
                "oneOf": [ { "required": ["url"] }, { "required": ["ref"] } ],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    /// Every fixed-choice param is a real enum in the schema and every param
    /// carries a description (the page + CLI render from these).
    #[test]
    fn schema_enums_and_descriptions_present() {
        let v: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = v.get("properties").unwrap().as_object().unwrap();
        assert!(props["segments"]["enum"].is_array());
        assert!(props["output"]["enum"].is_array());
        for (name, p) in props {
            let desc = p["description"].as_str().unwrap_or("");
            assert!(!desc.is_empty(), "param {name} is missing .describe()");
        }
    }
}
