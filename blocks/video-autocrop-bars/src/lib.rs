//! gizza-ai/video-autocrop-bars — fetch a video (url⊕ref), auto-detect its
//! letterbox/pillarbox black bars, and crop them off.
//!
//! Two-pass flow (same shape as `video-silence-cut` / `video-target-filesize-
//! encoder`): pass 1 runs ffmpeg's `cropdetect` filter (`-f null -`, no output
//! file) and we parse its log for the accumulated `crop=W:H:X:Y` union box plus
//! the input dimensions; pass 2 applies `crop=…` with an H.264 re-encode
//! (CRF 18, audio stream-copied when the container is kept). A full-frame box
//! means "no bars" and returns a clear message instead of a pointless
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
use gizza_ai_video_autocrop_bars_core::{
    crop_argv, decide, detect_argv, validate, Decision, DEFAULT_ROUND, DEFAULT_THRESHOLD,
};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 25 * 1024 * 1024; // 25 MiB
const MAX_OUTPUT_BYTES: usize = 25 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    /// Black threshold 0-255 (cropdetect `limit`, default 24).
    #[serde(default)]
    threshold: Option<f64>,
    /// Snap crop dims to a multiple: "2"/"4"/"8"/"16" (default "2").
    #[serde(default)]
    round: Option<String>,
}

/// Single-source param descriptor → chat schema (and CLI + page). The
/// drift-guard test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
        .param(
            Param::integer("threshold")
                .min(0.0)
                .max(255.0)
                .default(24)
                .describe(
                    "Black-detection threshold 0-255 (default 24). Bars darker than this count \
                     as black; raise it (e.g. 48) for grey-ish bars from heavy compression, \
                     lower it if dark scenes get cropped into.",
                ),
        )
        .param(
            Param::enumv("round", ["2", "4", "8", "16"])
                .default("2")
                .describe(
                    "Snap the cropped width/height to a multiple of this: 2 (default, removes \
                     the most bar while keeping H.264-legal even dimensions) / 4 / 8 / 16 \
                     (classic encoder-macroblock friendly).",
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
    name = "gizza-ai/video-autocrop-bars",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Auto-detect and remove letterbox/pillarbox black bars from a video (two-pass cropdetect)",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Automatically detect and crop away letterbox (top/bottom) or pillarbox (left/right) black bars from a video. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). Two-pass: ffmpeg cropdetect measures the bars over the whole clip (union box, so fades from black are safe), then the crop is applied with an H.264 CRF 18 re-encode; audio is stream-copied when the container is kept (mp4/mov/m4v/mkv, else the output switches to mp4 with AAC audio). threshold (0-255, default 24) sets how dark counts as black; round (2/4/8/16, default 2) snaps the cropped dimensions for encoder compatibility. Reports 'no black bars detected' instead of re-encoding a full-frame video.",
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
/// read the log on the detect pass and the output bytes on the crop pass. Maps
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
    let args: Args = serde_json::from_slice(&body).invalid_args("video-autocrop-bars")?;
    let threshold = args.threshold.unwrap_or(DEFAULT_THRESHOLD);
    let round = args.round.as_deref().unwrap_or(DEFAULT_ROUND).to_string();
    let (threshold, round) = validate(threshold, &round)
        .map_err(|e| SkillError::InvalidArgs(format!("invalid video-autocrop-bars args: {e}")))?;

    // 2. Resolve source.
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp4");
    let ffmpeg_in = format!("in.{in_ext}");

    // 3. Pass 1 — measure the bars (no output file; the result is the log).
    let detect = run_pass(
        detect_argv(&ffmpeg_in, threshold, round),
        &ffmpeg_in,
        input_bytes.clone(),
        "detect.null",
    )?;
    let (w, h, x, y, in_w, in_h) = match decide(&detect.log)
        .map_err(SkillError::InvalidArgs)?
    {
        Decision::NoBars { in_w, in_h } => {
            return Err(SkillError::InvalidArgs(format!(
                "no black bars detected — the {in_w}x{in_h} frame is already full picture \
                 (raise threshold if the bars are dark grey rather than black)"
            )));
        }
        Decision::Crop { w, h, x, y, in_w, in_h } => (w, h, x, y, in_w, in_h),
    };

    // 4. Pass 2 — crop + re-encode.
    let (argv, out_name) = crop_argv(&ffmpeg_in, w, h, x, y);
    let crop = run_pass(argv, &ffmpeg_in, input_bytes, &out_name)?;

    // 5. Envelope. Output ext/mime follow the h264_out_ext container rule.
    let out_ext = out_name.rsplit_once('.').map(|(_, e)| e).unwrap_or("mp4");
    let out_mime = match out_ext {
        "mov" => "video/quicktime",
        "mkv" => "video/x-matroska",
        _ => "video/mp4",
    };
    let output_size = crop.output.len();
    let filename = filename_with_suffix(&in_filename, "-autocrop", out_ext);
    let for_llm = format!(
        "removed black bars from {in_filename}: {in_w}x{in_h} -> {w}x{h} (crop offset x={x}, \
         y={y}; threshold {threshold}, round {round}) — {output_size} bytes {out_mime}"
    );
    build_media_envelope(&crop.output, out_mime, filename, for_llm, MAX_OUTPUT_BYTES)
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
                    "url":       { "type": "string", "description": "Video URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":       { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "threshold": { "type": "integer", "minimum": 0, "maximum": 255, "default": 24, "description": "Black-detection threshold 0-255 (default 24). Bars darker than this count as black; raise it (e.g. 48) for grey-ish bars from heavy compression, lower it if dark scenes get cropped into." },
                    "round":     { "type": "string", "enum": ["2", "4", "8", "16"], "default": "2", "description": "Snap the cropped width/height to a multiple of this: 2 (default, removes the most bar while keeping H.264-legal even dimensions) / 4 / 8 / 16 (classic encoder-macroblock friendly)." }
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
    fn output_filename_uses_autocrop_suffix() {
        assert_eq!(
            filename_with_suffix("clip.mp4", "-autocrop", "mp4"),
            "clip-autocrop.mp4"
        );
    }
}
