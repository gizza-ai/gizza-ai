//! gizza-ai/video-compress — fetch a video URL or attachment ref, shrink its
//! file size with a single-pass CRF re-encode (H.264/AAC), keep the container.

// The #[wafer_block] macro emits the impl gated to wasm32 (it generates a native
// registration call that requires ::new()). All the supporting imports,
// constants, and the Args type are only used inside the wasm32-gated impl, so
// they appear "unused" when running native unit tests. See image-resize for the
// full rationale.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use gizza_ai_block_utils::{
    dispatch_ffmpeg_runtime, filename_with_suffix, mime_to_ext, AssetKind, Envelope, FfmpegReq,
    FfmpegResp, ForUi, SkillError, SkillResultExt, Source, SourceFields,
};
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{fetch_from_url, load_from_attachment};
use gizza_ai_video_compress_core::{build_argv, clamp_crf};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 10 * 1024 * 1024; // 10 MiB
const MAX_OUTPUT_BYTES: usize = 10 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    /// CRF quality knob; omitted → core default (lower = higher quality/larger).
    #[serde(default)]
    crf: Option<f64>,
}

#[cfg(target_arch = "wasm32")]
struct VideoCompress;

// The #[wafer_block] macro emits a native registration call requiring ::new()
// on the impl; skill-style impls don't have one. Gate the struct + impl to
// wasm32 so the core unit tests still compile natively.
#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-compress",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Shrink a video's file size with a single-pass CRF re-encode, keeping the format",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    skill(
        description = "Shrink a video's file size by re-encoding it at a chosen quality (CRF), keeping the container format. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). Single-pass H.264/AAC re-encode — higher crf = smaller file / lower quality. This is a quality knob, not a target-size guarantee (true target-byte-size needs a 2-pass encode).",
        parameters = r#"{
            "type": "object",
            "properties": {
                "url": { "type": "string", "description": "Video URL (HTTP/HTTPS)." },
                "ref": { "type": "string", "description": "Reference id from a prior video tool call (e.g. \"call_42\"). Use either url or ref." },
                "crf": { "type": "number", "minimum": 18, "maximum": 34, "description": "Quality/size knob (default 28). Lower = higher quality/larger; higher = smaller. Clamped to 18-34." }
            },
            "oneOf": [
                { "required": ["url"] },
                { "required": ["ref"] }
            ]
        }"#
    ),
)]
impl VideoCompress {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    // 1. Parse args. `crf` is validated/clamped by core; an explicit non-finite
    //    value is the only thing we reject up front (a clear user error).
    let args: Args = serde_json::from_slice(&body).invalid_args("video-compress")?;
    if let Some(c) = args.crf {
        if !c.is_finite() {
            return Err(SkillError::InvalidArgs(
                "invalid video-compress args: crf must be a finite number".into(),
            ));
        }
    }
    // The page's "unset" sentinel is 0.0; the chat schema omits `crf` instead, so
    // map None → 0.0 to hit core's default path.
    let crf_req = args.crf.unwrap_or(0.0);
    let crf = clamp_crf(crf_req);

    // 2. Resolve source — URL fetch or attachment lookup.
    let (input_bytes, in_mime, in_filename) = match args.source.into_inner() {
        Source::Url(u) => fetch_from_url(&u, AssetKind::Video, MAX_INPUT_BYTES)?,
        Source::Ref(id) => load_from_attachment(&id, AssetKind::Video, MAX_INPUT_BYTES)?,
    };

    // 3. Build ffmpeg argv (core keeps the input container extension).
    let in_ext = mime_to_ext(&in_mime).unwrap_or("mp4");
    let ffmpeg_in = format!("in.{in_ext}");
    let (argv, ffmpeg_out) = build_argv(crf_req, &ffmpeg_in);

    // 4. Call ffmpeg-runtime.
    let req = FfmpegReq {
        args: argv,
        inputs: vec![(ffmpeg_in, input_bytes)],
        output: ffmpeg_out.clone(),
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

    // 5. Envelope. Output mime follows the produced extension (== input ext).
    let out_ext = ffmpeg_out.rsplit_once('.').map(|(_, e)| e).unwrap_or("mp4");
    let out_mime = ext_to_video_mime(out_ext);
    let output_size = ff.output.len();
    let encoded = B64.encode(&ff.output);
    let data_url = format!("data:{out_mime};base64,{encoded}");
    let filename = filename_with_suffix(&in_filename, "-compressed", out_ext);
    let env = Envelope {
        for_llm: format!("compressed {in_filename} at crf {crf} ({output_size} bytes {out_mime})"),
        for_ui: ForUi {
            data_url,
            mime: out_mime.to_string(),
            filename,
        },
    };
    serde_json::to_vec(&env).map_err(|e| SkillError::Serialize(format!("serialize envelope: {e}")))
}

/// Map an output container extension to its video MIME (mirrors `mime_to_ext`).
#[cfg(target_arch = "wasm32")]
fn ext_to_video_mime(ext: &str) -> &'static str {
    match ext {
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "mkv" => "video/x-matroska",
        _ => "video/mp4",
    }
}
