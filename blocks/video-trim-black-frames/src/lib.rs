//! gizza-ai/video-trim-black-frames — fetch a video (url⊕ref), detect the
//! leading/trailing fully-black frames, and trim them off.
//!
//! Two-pass flow (same shape as `video-autocrop-bars` / `video-silence-cut`):
//! pass 1 runs ffmpeg's `blackdetect` filter (`-f null -`, no output file) and
//! we parse its log for the black runs + clip duration; pass 2 keeps
//! `[start, end]` (start = end of the leading black run, end = start of the
//! trailing black run) with an H.264 CRF 18 re-encode + AAC audio. A clip with
//! no black at the requested ends returns a clear message instead of a pointless
//! re-encode. The standalone page mirrors this in `page/custom.js`.
//!
//! The chat schema is derived from `descriptor()` (single source, shared with
//! the CLI + page); the pure argv/parse/decision logic lives in `core`.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, mime_to_ext, AssetKind, Input, Param, SkillError,
    SkillResultExt, SourceFields, ToolDescriptor,
};
// resolve_source / the ffmpeg dispatch call host imports → wasm-only (like run()).
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg_runtime, resolve_source, FfmpegReq, FfmpegResp};
use gizza_ai_video_trim_black_frames_core::{
    decide, detect_argv, removed, trim_argv, validate, TrimDecision, DEFAULT_BLACK_RATIO,
    DEFAULT_ENDS, DEFAULT_MIN_DURATION, DEFAULT_PIXEL_THRESHOLD,
};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 25 * 1024 * 1024; // 25 MiB
const MAX_OUTPUT_BYTES: usize = 25 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    /// Pixel blackness threshold 0-1 (blackdetect pix_th, default 0.10).
    #[serde(default)]
    pixel_threshold: Option<f64>,
    /// Fraction of black pixels for a frame to count as black (pic_th, default 0.98).
    #[serde(default)]
    black_ratio: Option<f64>,
    /// Minimum black run to trim, seconds (blackdetect d, default 0.10).
    #[serde(default)]
    min_duration: Option<f64>,
    /// Which ends to trim: "both" / "start" / "end" (default "both").
    #[serde(default)]
    ends: Option<String>,
}

/// Single-source param descriptor → chat schema (and CLI + page). The
/// drift-guard test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
        .param(
            Param::number("pixel_threshold")
                .min(0.0)
                .max(1.0)
                .default(0.1)
                .describe(
                    "How dark a pixel must be to count as black, 0-1 (blackdetect pix_th, \
                     default 0.10). 0 is pure black; raise it (e.g. 0.15) to treat dark-grey \
                     fades as black, lower it if dark scenes get trimmed.",
                ),
        )
        .param(
            Param::number("black_ratio")
                .min(0.0)
                .max(1.0)
                .default(0.98)
                .describe(
                    "Fraction of a frame's pixels that must be black for the whole frame to \
                     count as black, 0-1 (blackdetect pic_th, default 0.98). Lower it (e.g. 0.90) \
                     if a logo or timestamp keeps a black frame from being detected.",
                ),
        )
        .param(
            Param::number("min_duration")
                .min(0.0)
                .max(60.0)
                .default(0.1)
                .describe(
                    "Shortest black run to trim, in seconds (blackdetect d, default 0.10). \
                     Raise it to ignore brief black flashes; lower it to catch a single black \
                     frame.",
                ),
        )
        .param(
            Param::enumv("ends", ["both", "start", "end"])
                .default("both")
                .describe(
                    "Which ends to trim: both (default), start (leading black only), or end \
                     (trailing black only).",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-trim-black-frames",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Detect and trim leading and trailing fully-black frames from a video (two-pass blackdetect)",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Detect and trim the leading and/or trailing fully-black frames from a video (a black intro or outro), leaving the real picture. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). Two-pass: ffmpeg blackdetect measures the black runs over the whole clip, then the clip is trimmed to the non-black span [start, end] with an H.264 CRF 18 re-encode and AAC audio (the container is kept for mp4/mov/m4v/mkv, else the output switches to mp4). pixel_threshold (0-1, default 0.10) sets how dark counts as black; black_ratio (0-1, default 0.98) sets what fraction of a frame must be black; min_duration (seconds, default 0.10) sets the shortest black run to trim; ends (both/start/end, default both) picks which edges to trim. Reports 'no black frames to trim' instead of re-encoding a clip that has none, and refuses to trim a clip that is black end to end. Only the two edges are trimmed — black in the middle of the clip is kept.",
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

/// Run one ffmpeg-runtime exec, returning the full response so the caller can
/// read the log on the detect pass and the output bytes on the trim pass. Maps
/// a non-zero exit to `FfmpegExitNonZero`.
#[cfg(target_arch = "wasm32")]
fn run_pass(
    argv: Vec<String>,
    in_name: &str,
    in_bytes: Vec<u8>,
    out_name: &str,
) -> Result<FfmpegResp, SkillError> {
    let req = FfmpegReq {
        args: argv,
        inputs: vec![(in_name.to_string(), in_bytes)],
        output: out_name.to_string(),
    };
    let req_body = serde_json::to_vec(&req)
        .map_err(|e| SkillError::Serialize(format!("serialize ffmpeg request: {e}")))?;
    let resp_bytes = dispatch_ffmpeg_runtime(&req_body)?;
    let resp: FfmpegResp = serde_json::from_slice(&resp_bytes)
        .map_err(|e| SkillError::Serialize(format!("malformed ffmpeg response: {e}")))?;
    if resp.exit_code != 0 {
        return Err(SkillError::FfmpegExitNonZero {
            exit: resp.exit_code,
            snippet: resp.log.chars().take(200).collect(),
        });
    }
    Ok(resp)
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Parse + validate args (before any fetch, so bad args fail fast).
    let args: Args = serde_json::from_slice(&body).invalid_args("video-trim-black-frames")?;
    let pixel_threshold = args.pixel_threshold.unwrap_or(DEFAULT_PIXEL_THRESHOLD);
    let black_ratio = args.black_ratio.unwrap_or(DEFAULT_BLACK_RATIO);
    let min_duration = args.min_duration.unwrap_or(DEFAULT_MIN_DURATION);
    let ends_str = args.ends.as_deref().unwrap_or(DEFAULT_ENDS).to_string();
    let (pixel_threshold, black_ratio, min_duration, ends) =
        validate(pixel_threshold, black_ratio, min_duration, &ends_str).map_err(|e| {
            SkillError::InvalidArgs(format!("invalid video-trim-black-frames args: {e}"))
        })?;

    // 2. Resolve source.
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp4");
    let ffmpeg_in = format!("in.{in_ext}");

    // 3. Pass 1 — measure the black runs (no output file; the result is the log).
    let detect = run_pass(
        detect_argv(&ffmpeg_in, pixel_threshold, black_ratio, min_duration),
        &ffmpeg_in,
        input_bytes.clone(),
        "detect.null",
    )?;
    let (start, end, duration) = match decide(&detect.log, ends).map_err(SkillError::InvalidArgs)? {
        TrimDecision::NoEdges { duration } => {
            return Err(SkillError::InvalidArgs(format!(
                "no black frames to trim — the {duration:.2}s clip has no fully-black run at the \
                 requested ends (raise pixel_threshold if the black is dark grey, or lower \
                 black_ratio if a logo sits over it)"
            )));
        }
        TrimDecision::Trim { start, end, duration } => (start, end, duration),
    };

    // 4. Pass 2 — trim + re-encode.
    let (argv, out_name) = trim_argv(&ffmpeg_in, start, end);
    let trim = run_pass(argv, &ffmpeg_in, input_bytes, &out_name)?;

    // 5. Envelope. Output ext/mime follow the h264_out_ext container rule.
    let out_ext = out_name.rsplit_once('.').map(|(_, e)| e).unwrap_or("mp4");
    let out_mime = match out_ext {
        "mov" => "video/quicktime",
        "mkv" => "video/x-matroska",
        _ => "video/mp4",
    };
    let output_size = trim.output.len();
    let (front, back) = removed(start, end, duration);
    let filename = filename_with_suffix(&in_filename, "-trimmed", out_ext);
    let for_llm = format!(
        "trimmed black frames from {in_filename}: kept {start:.2}s-{end:.2}s of {duration:.2}s \
         (removed {front:.2}s from the start, {back:.2}s from the end) — {output_size} bytes \
         {out_mime}"
    );
    build_media_envelope(&trim.output, out_mime, filename, for_llm, MAX_OUTPUT_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift-guard: the descriptor-derived chat schema must match the authored
    /// schema, so the LLM sees no drift. Regenerate this literal (never
    /// hand-patch it) whenever `descriptor()` changes.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":             { "type": "string", "description": "Video URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":             { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "pixel_threshold": { "type": "number", "minimum": 0, "maximum": 1, "default": 0.1, "description": "How dark a pixel must be to count as black, 0-1 (blackdetect pix_th, default 0.10). 0 is pure black; raise it (e.g. 0.15) to treat dark-grey fades as black, lower it if dark scenes get trimmed." },
                    "black_ratio":     { "type": "number", "minimum": 0, "maximum": 1, "default": 0.98, "description": "Fraction of a frame's pixels that must be black for the whole frame to count as black, 0-1 (blackdetect pic_th, default 0.98). Lower it (e.g. 0.90) if a logo or timestamp keeps a black frame from being detected." },
                    "min_duration":    { "type": "number", "minimum": 0, "maximum": 60, "default": 0.1, "description": "Shortest black run to trim, in seconds (blackdetect d, default 0.10). Raise it to ignore brief black flashes; lower it to catch a single black frame." },
                    "ends":            { "type": "string", "enum": ["both", "start", "end"], "default": "both", "description": "Which ends to trim: both (default), start (leading black only), or end (trailing black only)." }
                },
                "additionalProperties": false,
                "oneOf": [
                    { "required": ["url"] },
                    { "required": ["ref"] }
                ]
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn output_filename_uses_trimmed_suffix() {
        assert_eq!(filename_with_suffix("clip.mp4", "-trimmed", "mp4"), "clip-trimmed.mp4");
    }
}
