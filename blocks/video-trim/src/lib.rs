//! gizza-ai/video-trim — fetch a video URL or attachment ref, trim to [start, start+duration], stream-copy to mp4.

// The #[wafer_block] macro emits the impl gated to wasm32; supporting imports,
// constants, and the Args type are only used there. See image-resize for the
// full rationale.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use gizza_ai_block_utils::{
    dispatch_ffmpeg_runtime, mime_to_ext, AssetKind, Envelope, FfmpegReq, FfmpegResp, ForUi,
    SkillError, SkillResultExt, Source, SourceFields,
};
use serde::Deserialize;
use wafer_sdk::*;

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{fetch_from_url, load_from_attachment};

const MAX_INPUT_BYTES: usize = 10 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 10 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    start: f64,
    duration: f64,
}

fn build_argv(in_name: &str, out_name: &str, start: f64, duration: f64) -> Vec<String> {
    vec![
        "-ss".into(),
        format!("{start}"),
        "-i".into(),
        in_name.into(),
        "-t".into(),
        format!("{duration}"),
        "-c".into(),
        "copy".into(),
        out_name.into(),
    ]
}

fn output_filename(in_filename: &str, out_ext: &str) -> String {
    let stem = in_filename.rsplit_once('.').map(|(s, _)| s).unwrap_or(in_filename);
    format!("{stem}.{out_ext}")
}

#[cfg(target_arch = "wasm32")]
struct VideoTrim;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-trim",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Trim a video to a [start, start+duration] window",
    capabilities(callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Trim a video to a [start, start+duration] window using stream-copy (no re-encode). Provide either url (HTTP/HTTPS) or ref (id from a prior tool call). Output is mp4. Stream-copy preserves the source codecs and is fast — but requires the source streams be mp4-compatible (h264/aac); otherwise ffmpeg will fail with a clear error.",
        parameters = r#"{
            "type": "object",
            "properties": {
                "url":      { "type": "string" },
                "ref":      { "type": "string" },
                "start":    { "type": "number", "minimum": 0, "description": "Start time in seconds." },
                "duration": { "type": "number", "exclusiveMinimum": 0, "description": "Duration in seconds." }
            },
            "required": ["start", "duration"],
            "oneOf": [
                { "required": ["url"] },
                { "required": ["ref"] }
            ]
        }"#
    ),
)]
impl VideoTrim {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("video-trim")?;
    if args.start < 0.0 || !args.start.is_finite() {
        return Err(SkillError::InvalidArgs(format!(
            "invalid video-trim args: start must be >= 0 and finite, got {}",
            args.start
        )));
    }
    if args.duration <= 0.0 || !args.duration.is_finite() {
        return Err(SkillError::InvalidArgs(format!(
            "invalid video-trim args: duration must be > 0 and finite, got {}",
            args.duration
        )));
    }

    let (input_bytes, in_mime, in_filename) = match args.source.into_inner() {
        Source::Url(u) => fetch_from_url(&u, AssetKind::Video, MAX_INPUT_BYTES)?,
        Source::Ref(id) => load_from_attachment(&id, AssetKind::Video, MAX_INPUT_BYTES)?,
    };

    let in_ext = mime_to_ext(&in_mime).unwrap_or("bin");
    let ffmpeg_in = format!("in.{in_ext}");
    let ffmpeg_out = "out.mp4".to_string();
    let argv = build_argv(&ffmpeg_in, &ffmpeg_out, args.start, args.duration);

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
    let filename = output_filename(&in_filename, "mp4");
    let env = Envelope {
        for_llm: format!(
            "trimmed {} to [{}s, {}s+{}s] ({} bytes mp4)",
            in_filename, args.start, args.start, args.duration, output_size
        ),
        for_ui: ForUi {
            data_url,
            mime: "video/mp4".to_string(),
            filename,
        },
    };
    serde_json::to_vec(&env).map_err(|e| SkillError::Serialize(format!("serialize envelope: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argv_default() {
        let argv = build_argv("in.mp4", "out.mp4", 1.5, 3.0);
        assert_eq!(argv[0], "-ss");
        assert_eq!(argv[1], "1.5");
        assert_eq!(argv[2], "-i");
        assert_eq!(argv[3], "in.mp4");
        assert_eq!(argv[4], "-t");
        assert_eq!(argv[5], "3");
        assert!(argv.iter().any(|a| a == "copy"));
        assert_eq!(argv.last().map(String::as_str), Some("out.mp4"));
    }
}
