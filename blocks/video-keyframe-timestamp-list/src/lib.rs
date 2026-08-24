//! gizza-ai/video-keyframe-timestamp-list — list every keyframe (I-frame)
//! timestamp in a video, as text, CSV, or JSON.
//!
//! Pipeline: resolve the video source (URL/ref, `AssetKind::Video`) → run ONE
//! ffmpeg pass via the gizza-ai/ffmpeg-runtime bridge that keeps only I-frames and
//! prints a `showinfo` line for each (`core::detect_argv`, no output file) →
//! `core::parse_keyframes` reads the flagged frames' `pts_time` values out of the
//! log → `core::round_dedup` / `core::stats` / `core::render` turn them into the
//! requested rendering → flat JSON `Resp`.
//!
//! This is the same detect-only shape as `video-scene-cut-diff` (log in, numbers
//! out, no media written), with one input instead of two — no ffprobe is involved,
//! only the ffmpeg log. Keyframes are where a seek lands and where a stream-copy
//! cut can start losslessly, which is what the list is for.
//!
//! Surfaces: chat + CLI (the ffmpeg bridge runs on both). No standalone page — the
//! generic page driver expects a media file out of its ffmpeg pass, and this tool's
//! output is text.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{
    dispatch_ffmpeg_runtime, mime_to_ext, resolve_source, AssetKind, FfmpegReq, FfmpegResp,
};
use gizza_ai_block_utils::{
    Input, Param, SkillError, SkillResultExt, SourceFields, ToolDescriptor,
};
use gizza_ai_video_keyframe_timestamp_list_core as core;
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

/// Input cap — matches the video tool family (compress or trim longer footage).
const MAX_INPUT_BYTES: usize = 32 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    format: Option<String>,
    #[serde(default)]
    precision: Option<i64>,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
        .param(
            Param::enumv("format", core::FORMATS)
                .default(core::DEFAULT_FORMAT)
                .describe(
                    "How the timestamp list is rendered in `output` (default json). 'json' is an \
                     array of {index, seconds, timecode, gap_seconds} objects; 'csv' is the same \
                     columns as a header row plus one row per keyframe; 'text' is one keyframe per \
                     line (index, seconds, timecode, gap) for pasting into a seek list. All three \
                     carry the same data — the parsed numbers are always also returned in the \
                     `keyframes` array.",
                ),
        )
        .param(
            Param::integer("precision")
                .min(0.0)
                .max(core::MAX_PRECISION as f64)
                .default(core::DEFAULT_PRECISION as i64)
                .describe(
                    "Decimal places on every timestamp and gap, 0-6 (default 3 = milliseconds). 3 \
                     is enough to seek to the exact frame at any normal frame rate; 0 rounds to \
                     whole seconds (keyframes that land in the same second are then listed once).",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[derive(Serialize)]
struct Resp {
    /// Every keyframe timestamp in seconds, ascending, rounded to `precision`.
    keyframes: Vec<f64>,
    count: usize,
    first: Option<f64>,
    last: Option<f64>,
    min_gap: Option<f64>,
    max_gap: Option<f64>,
    avg_gap: Option<f64>,
    format: String,
    precision: u32,
    /// The list rendered in `format` — the text/CSV/JSON the caller asked for.
    output: String,
    summary: String,
}

/// Build the flat JSON response from a parsed keyframe list (shared by wasm + tests).
fn list_json(raw: &[f64], format: &str, precision: u32) -> Result<Vec<u8>, SkillError> {
    let times = core::round_dedup(raw, precision);
    let st = core::stats(&times, precision);
    let output = core::render(&times, format, precision).map_err(SkillError::InvalidArgs)?;
    let resp = Resp {
        keyframes: times.clone(),
        count: st.count,
        first: st.first,
        last: st.last,
        min_gap: st.min_gap,
        max_gap: st.max_gap,
        avg_gap: st.avg_gap,
        format: format.to_string(),
        precision,
        output,
        summary: core::summary(&st, precision),
    };
    serde_json::to_vec(&resp).map_err(|e| {
        SkillError::Serialize(format!(
            "serialize video-keyframe-timestamp-list response: {e}"
        ))
    })
}

/// Validate the two scalar params up front, so a bad enum/precision fails before
/// the video is fetched and decoded.
fn checked_params(format: Option<&str>, precision: Option<i64>) -> Result<(String, u32), String> {
    let format = format.unwrap_or(core::DEFAULT_FORMAT).trim().to_lowercase();
    if !core::FORMATS.contains(&format.as_str()) {
        return Err(format!(
            "format must be one of: {} (got '{format}')",
            core::FORMATS.join(", ")
        ));
    }
    let precision = precision.unwrap_or(core::DEFAULT_PRECISION as i64);
    if !(0..=core::MAX_PRECISION as i64).contains(&precision) {
        return Err(format!(
            "precision must be between 0 and {} decimal places, got {precision}",
            core::MAX_PRECISION
        ));
    }
    Ok((format, precision as u32))
}

#[cfg(target_arch = "wasm32")]
struct VideoKeyframeTimestampList;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-keyframe-timestamp-list",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "List every keyframe (I-frame) timestamp in a video as text, CSV, or JSON",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "List the exact timestamps of every KEYFRAME (I-frame) in a video — the only points a player can seek to instantly and the only points a stream-copy cut can start on without re-encoding, so this is the map for lossless trimming, split planning, and checking GOP spacing. Provide the video as a url (HTTP/HTTPS) or a `ref` from a prior tool call; any container ffmpeg reads (MP4/MOV, MKV/WebM, AVI, …) with a video track, up to 32 MiB — compress or trim longer footage first. Only the first video stream is read; audio, subtitles, and cover art are ignored. `format` chooses the rendering of the `output` field: json (default, an array of {index, seconds, timecode, gap_seconds}), csv (the same columns with a header row), or text (one keyframe per line). `precision` sets the decimal places on every timestamp, 0-6 (default 3 = milliseconds). This tool MEASURES only — it never trims, re-encodes, or rewrites the video. Returns flat JSON: keyframes[] (the raw seconds), count, first, last, min_gap/max_gap/avg_gap (the spacing between consecutive keyframes), the rendered output string, and a one-line summary.",
        parameters = schema_json()
    ),
)]
impl VideoKeyframeTimestampList {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

/// Run the single detect pass and return ffmpeg's LOG — the keyframe timestamps
/// are printed by `showinfo`, and no output file is written. A non-zero exit maps
/// to `FfmpegExitNonZero` with the last 200 log characters.
#[cfg(target_arch = "wasm32")]
fn detect_pass(argv: Vec<String>, in_name: &str, in_bytes: Vec<u8>) -> Result<String, SkillError> {
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
        let tail: String = {
            let chars: Vec<char> = resp.log.chars().collect();
            let start = chars.len().saturating_sub(200);
            chars[start..].iter().collect()
        };
        return Err(SkillError::FfmpegExitNonZero {
            exit: resp.exit_code,
            snippet: tail,
        });
    }
    Ok(resp.log)
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("video-keyframe-timestamp-list")?;
    let (format, precision) =
        checked_params(args.format.as_deref(), args.precision).map_err(SkillError::InvalidArgs)?;

    let (bytes, mime, _name) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;
    let ext = mime_to_ext(&mime).unwrap_or("mp4");
    let in_name = format!("in.{ext}");
    let log = detect_pass(core::detect_argv(&in_name), &in_name, bytes)?;

    let raw = core::parse_keyframes(&log);
    if raw.len() > core::MAX_KEYFRAMES {
        return Err(SkillError::InvalidArgs(format!(
            "{} keyframes found (cap {}) — this looks like all-intra footage (ProRes, DNxHD, \
             MJPEG, or -g 1), where every frame is a keyframe; trim the clip first",
            raw.len(),
            core::MAX_KEYFRAMES
        )));
    }
    list_json(&raw, &format, precision)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r##"{
                "type": "object",
                "properties": {
                    "url": { "type": "string", "description": "Video URL (HTTP/HTTPS). Use either url or ref." },
                    "ref": { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "format": {
                        "type": "string",
                        "enum": ["json", "csv", "text"],
                        "default": "json",
                        "description": "How the timestamp list is rendered in `output` (default json). 'json' is an array of {index, seconds, timecode, gap_seconds} objects; 'csv' is the same columns as a header row plus one row per keyframe; 'text' is one keyframe per line (index, seconds, timecode, gap) for pasting into a seek list. All three carry the same data — the parsed numbers are always also returned in the `keyframes` array."
                    },
                    "precision": {
                        "type": "integer",
                        "minimum": 0,
                        "maximum": 6,
                        "default": 3,
                        "description": "Decimal places on every timestamp and gap, 0-6 (default 3 = milliseconds). 3 is enough to seek to the exact frame at any normal frame rate; 0 rounds to whole seconds (keyframes that land in the same second are then listed once)."
                    }
                },
                "additionalProperties": false,
                "oneOf": [{ "required": ["url"] }, { "required": ["ref"] }]
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    /// The showinfo log an I-frame select pass produces for a 3-keyframe clip.
    const SAMPLE_LOG: &str = "\
[Parsed_showinfo_1 @ 0x561] n:0 pts:0 pts_time:0 pos:48 fmt:yuv420p type:I
Input #0, mov,mp4,m4a: Duration: 00:00:06.00, start: 0.000000, bitrate: 700 kb/s
[Parsed_showinfo_1 @ 0x561] n:1 pts:48048 pts_time:2.002 pos:9012 fmt:yuv420p type:I
[Parsed_showinfo_1 @ 0x561] n:2 pts:96096 pts_time:4.004 pos:19000 fmt:yuv420p type:I
frame=    3 fps=0.0 q=-0.0 Lsize=N/A time=00:00:06.00 bitrate=N/A speed=12x";

    #[test]
    fn response_is_flat_and_complete_for_json() {
        let raw = core::parse_keyframes(SAMPLE_LOG);
        let v: serde_json::Value =
            serde_json::from_slice(&list_json(&raw, "json", 3).unwrap()).unwrap();

        assert_eq!(v["keyframes"], serde_json::json!([0.0, 2.002, 4.004]));
        assert_eq!(v["count"], 3);
        assert_eq!(v["first"], 0.0);
        assert_eq!(v["last"], 4.004);
        assert_eq!(v["min_gap"], 2.002);
        assert_eq!(v["max_gap"], 2.002);
        assert_eq!(v["avg_gap"], 2.002);
        assert_eq!(v["format"], "json");
        assert_eq!(v["precision"], 3);
        assert!(v["summary"].as_str().unwrap().contains("3 keyframes"));
        // `output` holds the requested rendering — here, parseable JSON rows.
        let rows: serde_json::Value = serde_json::from_str(v["output"].as_str().unwrap()).unwrap();
        assert_eq!(rows[2]["timecode"], "00:00:04.004");
    }

    #[test]
    fn response_output_switches_with_format() {
        let raw = core::parse_keyframes(SAMPLE_LOG);

        let csv: serde_json::Value =
            serde_json::from_slice(&list_json(&raw, "csv", 3).unwrap()).unwrap();
        assert_eq!(csv["format"], "csv");
        let csv_out = csv["output"].as_str().unwrap();
        assert!(
            csv_out.starts_with("index,seconds,timecode,gap_seconds\n"),
            "{csv_out}"
        );
        assert!(csv_out.contains("2,2.002,00:00:02.002,2.002"), "{csv_out}");

        let text: serde_json::Value =
            serde_json::from_slice(&list_json(&raw, "text", 3).unwrap()).unwrap();
        assert_eq!(text["format"], "text");
        assert_eq!(text["output"].as_str().unwrap().lines().count(), 3);
        // The parsed numbers stay identical whatever the rendering.
        assert_eq!(csv["keyframes"], text["keyframes"]);
    }

    #[test]
    fn response_for_a_video_with_no_detected_keyframes() {
        let v: serde_json::Value =
            serde_json::from_slice(&list_json(&[], "csv", 3).unwrap()).unwrap();
        assert_eq!(v["count"], 0);
        assert_eq!(v["keyframes"], serde_json::json!([]));
        assert!(v["first"].is_null());
        assert!(v["avg_gap"].is_null());
        assert!(v["summary"].as_str().unwrap().contains("No keyframes"));
    }

    #[test]
    fn precision_zero_rounds_and_collapses_the_list() {
        let raw = vec![0.0, 0.417, 2.002];
        let v: serde_json::Value =
            serde_json::from_slice(&list_json(&raw, "json", 0).unwrap()).unwrap();
        assert_eq!(v["keyframes"], serde_json::json!([0.0, 2.0]));
        assert_eq!(v["count"], 2);
        assert_eq!(v["precision"], 0);
    }

    #[test]
    fn checked_params_defaults_and_normalises() {
        assert_eq!(checked_params(None, None).unwrap(), ("json".into(), 3));
        assert_eq!(
            checked_params(Some(" CSV "), Some(0)).unwrap(),
            ("csv".into(), 0)
        );
        assert_eq!(checked_params(Some("text"), Some(6)).unwrap().1, 6);
    }

    #[test]
    fn checked_params_rejects_bad_values() {
        let e = checked_params(Some("srt"), None).unwrap_err();
        assert!(e.contains("format must be one of"), "{e}");
        let e = checked_params(None, Some(7)).unwrap_err();
        assert!(e.contains("precision must be between 0 and 6"), "{e}");
        let e = checked_params(None, Some(-1)).unwrap_err();
        assert!(e.contains("precision must be between 0 and 6"), "{e}");
    }
}
