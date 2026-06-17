//! gizza-ai/video-silence-cut — fetch a video URL or attachment ref, remove silent
//! gaps via ffmpeg `silenceremove`, return an mp4 envelope.
//!
//! SINGLE-PASS APPROXIMATION: this de-silences the audio and truncates the video to
//! the kept audio length, but a single ffmpeg invocation cannot drop the matching
//! video frames, so the video drifts out of sync after the first removed gap. A
//! correct jump-cut needs a two-pass (silencedetect → select/atrim) flow the gizza
//! runtime does not support today. See `core/src/lib.rs` for the full rationale.

// The #[wafer_block] macro emits the impl gated to wasm32; supporting imports,
// constants, and the Args type are only used there. See image-resize for the
// full rationale.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use gizza_ai_block_utils::{
    dispatch_ffmpeg_runtime, mime_to_ext, replace_extension, AssetKind, Envelope, FfmpegReq,
    FfmpegResp, ForUi, SkillError, SkillResultExt, Source, SourceFields,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{fetch_from_url, load_from_attachment};
use gizza_ai_video_silence_cut_core::build_argv;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 10 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 10 * 1024 * 1024;

const DEFAULT_THRESHOLD_DB: f64 = -30.0;
const DEFAULT_MIN_SILENCE_S: f64 = 0.5;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    #[serde(default)]
    threshold_db: Option<f64>,
    #[serde(default)]
    min_silence: Option<f64>,
}

#[cfg(target_arch = "wasm32")]
struct VideoSilenceCut;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-silence-cut",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Remove silent gaps from a video (single-pass approximation)",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    skill(
        description = "Remove silent gaps from a video to tighten pacing (jump-cut style). Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). threshold_db sets the silence loudness cutoff in dB (default -30; quieter than this counts as silence) and min_silence is the shortest gap in seconds to trim (default 0.5). Output is mp4. NOTE: this is a single-pass APPROXIMATION — the audio is de-silenced correctly but the video drifts out of sync after the first removed gap, because one ffmpeg pass cannot drop the matching video frames. A correct cut needs two-pass support the runtime does not yet have.",
        parameters = r#"{
            "type": "object",
            "properties": {
                "url":          { "type": "string", "description": "Video URL (HTTP/HTTPS)." },
                "ref":          { "type": "string", "description": "Reference id from a prior video tool call. Use either url or ref." },
                "threshold_db": { "type": "number", "maximum": 0, "description": "Silence threshold in dB (default -30). Audio quieter than this counts as silence." },
                "min_silence":  { "type": "number", "exclusiveMinimum": 0, "description": "Minimum silent-gap length in seconds to trim (default 0.5)." }
            },
            "oneOf": [
                { "required": ["url"] },
                { "required": ["ref"] }
            ]
        }"#
    ),
)]
impl VideoSilenceCut {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("video-silence-cut")?;
    let threshold_db = args.threshold_db.unwrap_or(DEFAULT_THRESHOLD_DB);
    let min_silence = args.min_silence.unwrap_or(DEFAULT_MIN_SILENCE_S);
    if !threshold_db.is_finite() || threshold_db > 0.0 {
        return Err(SkillError::InvalidArgs(format!(
            "invalid video-silence-cut args: threshold_db must be a finite number <= 0, got {threshold_db}"
        )));
    }
    if !min_silence.is_finite() || min_silence <= 0.0 {
        return Err(SkillError::InvalidArgs(format!(
            "invalid video-silence-cut args: min_silence must be a finite number > 0, got {min_silence}"
        )));
    }

    let (input_bytes, in_mime, in_filename) = match args.source.into_inner() {
        Source::Url(u) => fetch_from_url(&u, AssetKind::Video, MAX_INPUT_BYTES)?,
        Source::Ref(id) => load_from_attachment(&id, AssetKind::Video, MAX_INPUT_BYTES)?,
    };

    let in_ext = mime_to_ext(&in_mime).unwrap_or("bin");
    let ffmpeg_in = format!("in.{in_ext}");
    let ffmpeg_out = "out.mp4".to_string();
    let argv = build_argv(&ffmpeg_in, &ffmpeg_out, threshold_db, min_silence);

    let req = FfmpegReq {
        args: argv,
        inputs: vec![(ffmpeg_in, input_bytes)],
        output: ffmpeg_out,
    };
    let req_body = serde_json::to_vec(&req)
        .map_err(|e| SkillError::Serialize(format!("serialize ffmpeg request: {e}")))?;
    let ff_resp_bytes = dispatch_ffmpeg_runtime(&req_body)?;
    let ff: FfmpegResp = serde_json::from_slice(&ff_resp_bytes)
        .map_err(|e| SkillError::Serialize(format!("malformed ffmpeg response: {e}")))?;

    if ff.exit_code != 0 {
        let snippet: String = ff.log.chars().take(200).collect();
        return Err(SkillError::FfmpegExitNonZero {
            exit: ff.exit_code,
            snippet,
        });
    }
    if ff.output.len() > MAX_OUTPUT_BYTES {
        return Err(SkillError::TooLarge {
            kind: "output video",
            bytes: ff.output.len(),
            cap: MAX_OUTPUT_BYTES,
        });
    }

    let output_size = ff.output.len();
    let encoded = B64.encode(&ff.output);
    let data_url = format!("data:video/mp4;base64,{encoded}");
    let filename = replace_extension(&in_filename, "mp4");
    let env = Envelope {
        for_llm: format!(
            "removed silence from {in_filename} (threshold {threshold_db}dB, min gap {min_silence}s) — {output_size} bytes mp4. NOTE: audio is de-silenced but video may be out of sync (single-pass approximation)."
        ),
        for_ui: ForUi {
            data_url,
            mime: "video/mp4".to_string(),
            filename,
        },
    };
    serde_json::to_vec(&env).map_err(|e| SkillError::Serialize(format!("serialize envelope: {e}")))
}
