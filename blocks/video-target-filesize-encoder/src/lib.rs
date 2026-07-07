//! gizza-ai/video-target-filesize-encoder — fetch a video (url⊕ref), then
//! re-encode it to land **under a chosen file-size budget** (target MB).
//!
//! Unlike a single-shot ffmpeg tool, `run()` dispatches ffmpeg **twice**: a probe
//! pass (`ffmpeg -i … -f null -`) whose log gives the clip duration, then an
//! encode pass at a video bitrate computed from `(target, duration, audio)` so
//! the muxed MP4 lands under the cap. The gizza ffmpeg bridge returns each exec's
//! log and the block can dispatch twice (same flow as `video-silence-cut`), so
//! this runs in chat + CLI. The standalone page mirrors it in `page/custom.js`
//! (it reads `<video>.duration`, so no probe pass is needed there).
//!
//! Single-pass encode (`-b:v` + `-maxrate`/`-bufsize`): the bridge is one exec
//! per call with no persisted passlog, so true two-pass VBR is out of model. The
//! pure bitrate math + argv construction live in `core`. See
//! docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, mime_to_ext, AssetKind, Input, Param, SkillError,
    SkillResultExt, SourceFields, ToolDescriptor,
};
// resolve_source / the ffmpeg dispatch call host imports → wasm-only (like run()).
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg_runtime, resolve_source, FfmpegReq, FfmpegResp};
use gizza_ai_video_target_filesize_encoder_core::{build_argv, parse_duration, probe_argv};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 25 * 1024 * 1024; // 25 MiB
const MAX_OUTPUT_BYTES: usize = 25 * 1024 * 1024;

/// Block-level target cap (MB). The tool is built for "fit under Discord 10 MB /
/// email 25 MB"-style limits; a target above the output envelope is pointless.
/// The descriptor advertises the same bound.
const MAX_TARGET_MB: f64 = 25.0;
const MIN_TARGET_MB: f64 = 0.1;

const DEFAULT_AUDIO: &str = "128";
const DEFAULT_SCALE: &str = "keep";

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    /// Target maximum output size in MB (required).
    #[serde(default)]
    target_mb: Option<f64>,
    /// Audio bitrate keyword: none/64/96/128/192/320 (default 128).
    #[serde(default)]
    audio_kbps: Option<String>,
    /// Max output height: keep/1080/720/480/360 (default keep).
    #[serde(default)]
    scale: Option<String>,
}

/// Single-source param descriptor → chat schema (and CLI + page). The drift-guard
/// test below proves the derived schema matches the authored one.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
        .param(
            Param::number("target_mb")
                .required()
                .min(MIN_TARGET_MB)
                .max(MAX_TARGET_MB)
                .describe(
                    "Target maximum output size in MB, e.g. 10 (Discord) or 25 (email). The tool \
                     computes the H.264 bitrate from the clip duration so the MP4 lands just \
                     under this budget. Range 0.1-25 MB.",
                ),
        )
        .param(
            Param::enumv("audio_kbps", ["none", "64", "96", "128", "192", "320"])
                .default("128")
                .describe(
                    "Audio bitrate in kbps, or \"none\" to drop audio and spend the whole budget \
                     on video: none / 64 (voice) / 96 / 128 (default) / 192 / 320.",
                ),
        )
        .param(
            Param::enumv("scale", ["keep", "1080", "720", "480", "360"])
                .default("keep")
                .describe(
                    "Cap the output height (shrink only, keeps aspect): keep (default, source \
                     size) / 1080 / 720 / 480 / 360. Lower it when the target can't be met at full \
                     resolution.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

/// Validate the target here so the LLM/CLI gets a clear message before any fetch;
/// core re-checks against its absolute guard during argv construction.
fn validate_target(target_mb: f64) -> Result<(), SkillError> {
    if !target_mb.is_finite() || target_mb < MIN_TARGET_MB || target_mb > MAX_TARGET_MB {
        return Err(SkillError::InvalidArgs(format!(
            "target_mb must be between {MIN_TARGET_MB} and {MAX_TARGET_MB} MB"
        )));
    }
    Ok(())
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-target-filesize-encoder",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Re-encode a video to land under a target file size (MB) by computing the bitrate from its duration",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    skill(
        description = "Re-encode a video so its file lands UNDER a chosen size budget (target_mb, e.g. 10 for Discord or 25 for email). Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). The tool probes the clip duration, computes the H.264 video bitrate that fits the budget minus the audio (audio_kbps: none/64/96/128/192/320), optionally caps the height (scale: keep/1080/720/480/360), and outputs MP4 (H.264/AAC). Single-pass bitrate targeting — highly-compressible clips land comfortably under, never over. Not true two-pass VBR.",
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

/// Run one ffmpeg-runtime exec, returning the full response so the caller can read
/// the log on the probe pass and the output bytes on the encode pass. Maps a
/// non-zero exit to `FfmpegExitNonZero`.
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
    // 1. Parse + validate args.
    let args: Args = serde_json::from_slice(&body).invalid_args("video-target-filesize-encoder")?;
    let target_mb = args.target_mb.ok_or_else(|| {
        SkillError::InvalidArgs("target_mb is required (target size in MB, e.g. 10)".into())
    })?;
    validate_target(target_mb)?;
    let audio = args.audio_kbps.as_deref().unwrap_or(DEFAULT_AUDIO).to_string();
    let scale = args.scale.as_deref().unwrap_or(DEFAULT_SCALE).to_string();

    // 2. Resolve source.
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp4");
    let ffmpeg_in = format!("in.{in_ext}");

    // 3. Probe pass — read the clip duration from the log.
    let probe = run_pass(
        probe_argv(&ffmpeg_in),
        &ffmpeg_in,
        input_bytes.clone(),
        "probe.null",
    )?;
    let duration = parse_duration(&probe.log).ok_or_else(|| SkillError::FfmpegExitNonZero {
        exit: 0,
        snippet: "could not read clip duration from ffmpeg output".into(),
    })?;

    // 4. Compute bitrate + build the encode argv (core validates the math).
    let (argv, out_name) =
        build_argv(target_mb, duration, &audio, &scale, &ffmpeg_in).map_err(SkillError::InvalidArgs)?;

    // 5. Encode pass.
    let encode = run_pass(argv, &ffmpeg_in, input_bytes, &out_name)?;

    // 6. Envelope (always MP4).
    let output_size = encode.output.len();
    let out_mb = output_size as f64 / (1024.0 * 1024.0);
    let filename = filename_with_suffix(&in_filename, "-target", "mp4");
    let for_llm = format!(
        "encoded {in_filename} to {out_mb:.2} MB (video/mp4), under the {target_mb} MB target \
         ({duration:.1}s clip, audio {audio})"
    );
    build_media_envelope(&encode.output, "video/mp4", filename, for_llm, MAX_OUTPUT_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift-guard: the descriptor-derived chat schema must match the authored
    /// schema, so the LLM sees no drift. Regenerate this literal (never hand-patch
    /// it) whenever `descriptor()` changes.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":        { "type": "string", "description": "Video URL (HTTP/HTTPS). Use either url or ref." },
                    "ref":        { "type": "string", "description": "Reference id from a prior tool call. Use either url or ref." },
                    "target_mb":  { "type": "number", "minimum": 0.1, "maximum": 25, "description": "Target maximum output size in MB, e.g. 10 (Discord) or 25 (email). The tool computes the H.264 bitrate from the clip duration so the MP4 lands just under this budget. Range 0.1-25 MB." },
                    "audio_kbps": { "type": "string", "enum": ["none", "64", "96", "128", "192", "320"], "default": "128", "description": "Audio bitrate in kbps, or \"none\" to drop audio and spend the whole budget on video: none / 64 (voice) / 96 / 128 (default) / 192 / 320." },
                    "scale":      { "type": "string", "enum": ["keep", "1080", "720", "480", "360"], "default": "keep", "description": "Cap the output height (shrink only, keeps aspect): keep (default, source size) / 1080 / 720 / 480 / 360. Lower it when the target can't be met at full resolution." }
                },
                "additionalProperties": false,
                "required": ["target_mb"],
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
    fn validate_target_rejects_out_of_range() {
        assert!(validate_target(10.0).is_ok());
        assert!(validate_target(0.0).is_err());
        assert!(validate_target(50.0).is_err());
        assert!(validate_target(f64::NAN).is_err());
    }

    #[test]
    fn output_filename_uses_target_suffix() {
        assert_eq!(
            filename_with_suffix("clip.mov", "-target", "mp4"),
            "clip-target.mp4"
        );
    }
}
