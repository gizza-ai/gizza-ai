//! gizza-ai/video-silence-cut — fetch a video URL or attachment ref, remove its
//! silent gaps with a **correct two-pass** ffmpeg flow, return an mp4 envelope.
//!
//! Pass 1 runs `silencedetect` (no output file) and we parse its log for the
//! silent ranges + clip duration. Pass 2 keeps only the complementary non-silent
//! segments of *both* streams (`select`/`aselect` + `setpts`/`asetpts`), so audio
//! and video stay in sync — unlike a single-pass `silenceremove`, which desyncs
//! the video. The gizza ffmpeg bridge returns each exec's log and the block can
//! dispatch twice, so this runs in chat + CLI. There is no standalone page (the
//! generic page driver is single-pass); this is a chat+CLI tool like `web-fetch`.
//!
//! The chat schema is derived from `descriptor()` (single source — shared across
//! chat + CLI). The pure argv/parse/segment logic lives in `core`. See
//! docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.

// The #[wafer_block] macro emits the impl gated to wasm32 (it generates a native
// registration call that requires ::new()). Supporting imports, constants, and
// the Args type are only used inside that impl, so they look "unused" under a
// native build; `descriptor()`/`schema_json()` stay native-compilable so the
// sanity test below can exercise them. See video-compress for the full rationale.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{
    build_media_envelope, filename_with_suffix, mime_to_ext, AssetKind, Input, Param, SkillError,
    SkillResultExt, ToolDescriptor,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg_runtime, resolve_source, FfmpegReq, FfmpegResp};
use gizza_ai_video_silence_cut_core::{
    cut_argv, detect_argv, keep_segments, parse_duration, parse_silences, validate,
};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 10 * 1024 * 1024; // 10 MiB
const MAX_OUTPUT_BYTES: usize = 10 * 1024 * 1024;

const DEFAULT_THRESHOLD_DB: f64 = -30.0;
const DEFAULT_MIN_SILENCE_S: f64 = 0.5;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: gizza_ai_block_utils::SourceFields,
    #[serde(default)]
    threshold_db: Option<f64>,
    #[serde(default)]
    min_silence: Option<f64>,
}

/// Single-source param descriptor → chat schema (and CLI). `Input::Video` adds
/// the `url`⊕`ref` `oneOf`. `min_silence`'s schema minimum is 0 (inclusive);
/// `validate()` enforces the real `> 0` at runtime with a clear message.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Video)
        .param(
            Param::number("threshold_db")
                .max(0.0)
                .describe("Silence threshold in dB (default -30). Audio quieter than this counts as silence."),
        )
        .param(
            Param::number("min_silence")
                .min(0.0)
                .describe("Minimum silent-gap length in seconds to trim (default 0.5). Must be > 0."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct VideoSilenceCut;

// The #[wafer_block] macro emits a native registration call requiring ::new();
// skill-style impls don't have one. Gate the struct + impl to wasm32 so the
// descriptor sanity test still compiles natively.
#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-silence-cut",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Remove silent gaps from a video (correct two-pass, A/V stays in sync)",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Remove silent gaps from a video to tighten pacing (jump-cut style), keeping audio and video in sync. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). threshold_db sets the silence loudness cutoff in dB (default -30; quieter than this counts as silence) and min_silence is the shortest gap in seconds to trim (default 0.5). Output is mp4 (re-encoded H.264/AAC). Uses a correct two-pass detect-then-cut flow.",
        parameters = schema_json()
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

/// Run one ffmpeg-runtime exec and return the full response (so callers can read
/// the log on pass 1 and the output bytes on pass 2). Maps a non-zero exit to
/// `FfmpegExitNonZero`.
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
    let args: Args = serde_json::from_slice(&body).invalid_args("video-silence-cut")?;
    let threshold_db = args.threshold_db.unwrap_or(DEFAULT_THRESHOLD_DB);
    let min_silence = args.min_silence.unwrap_or(DEFAULT_MIN_SILENCE_S);
    validate(threshold_db, min_silence)
        .map_err(|e| SkillError::InvalidArgs(format!("invalid video-silence-cut args: {e}")))?;

    // 2. Resolve source.
    let (input_bytes, in_mime, in_filename) =
        resolve_source(args.source.into_inner(), AssetKind::Video, MAX_INPUT_BYTES)?;
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp4");
    let ffmpeg_in = format!("in.{in_ext}");

    // 3. Pass 1 — detect silences (no output file; the result is the log).
    let detect = run_pass(
        detect_argv(&ffmpeg_in, threshold_db, min_silence),
        &ffmpeg_in,
        input_bytes.clone(),
        "detect.null",
    )?;
    let duration = parse_duration(&detect.log).ok_or_else(|| {
        SkillError::FfmpegExitNonZero {
            exit: 0,
            snippet: "could not read clip duration from ffmpeg output".into(),
        }
    })?;
    let silences = parse_silences(&detect.log);
    let removed = silences.len();
    let keeps = keep_segments(duration, &silences);
    if keeps.is_empty() {
        return Err(SkillError::InvalidArgs(
            "the entire clip is silent at this threshold — nothing to keep".into(),
        ));
    }

    // 4. Pass 2 — keep only the non-silent segments of both streams.
    let out_name = "out.mp4";
    let cut = run_pass(
        cut_argv(&ffmpeg_in, out_name, &keeps),
        &ffmpeg_in,
        input_bytes,
        out_name,
    )?;

    // 5. Envelope.
    let output_size = cut.output.len();
    let kept: f64 = keeps.iter().map(|(s, e)| e - s).sum();
    let filename = filename_with_suffix(&in_filename, "-cut", "mp4");
    let for_llm = format!(
        "cut {removed} silent gap(s) from {in_filename}: kept {kept:.2}s of {duration:.2}s \
         ({output_size} bytes video/mp4)"
    );
    build_media_envelope(&cut.output, "video/mp4", filename, for_llm, MAX_OUTPUT_BYTES)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The descriptor renders to a valid JSON schema with both numeric params and
    /// the `url`⊕`ref` `oneOf` (from `Input::Video`).
    #[test]
    fn schema_json_is_valid_and_has_params() {
        let v: serde_json::Value = serde_json::from_str(&schema_json()).expect("valid JSON schema");
        let props = v.get("properties").expect("properties");
        assert!(props.get("threshold_db").is_some());
        assert!(props.get("min_silence").is_some());
        assert!(props.get("url").is_some());
        assert!(props.get("ref").is_some());
        assert!(v.get("oneOf").is_some(), "Input::Video adds url/ref oneOf");
        assert_eq!(props["threshold_db"]["maximum"], serde_json::json!(0));
    }
}
