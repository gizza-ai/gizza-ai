//! gizza-ai/video-frame-extract — fetch a video URL or attachment ref, extract a single frame as PNG.

// The #[wafer_block] macro emits the impl gated to wasm32; supporting imports,
// constants, and the Args type are only used there. See image-resize for the
// full rationale.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use gizza_ai_block_utils::{
    dispatch_ffmpeg_runtime, filename_with_suffix, mime_to_ext, AssetKind, Envelope, FfmpegReq,
    FfmpegResp, ForUi, SkillError, SkillResultExt, Source, SourceFields,
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
    timestamp: f64,
}


fn build_argv(in_name: &str, out_name: &str, timestamp: f64) -> Vec<String> {
    vec![
        "-ss".into(),
        format!("{timestamp}"),
        "-i".into(),
        in_name.into(),
        "-frames:v".into(),
        "1".into(),
        "-update".into(),
        "1".into(),
        "-y".into(),
        out_name.into(),
    ]
}

#[cfg(target_arch = "wasm32")]
struct VideoFrameExtract;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/video-frame-extract",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract a single frame from a video at a given timestamp",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    skill(
        description = "Extract a single frame from a video at the given timestamp (seconds), output as PNG. The PNG is naturally chainable into image-resize, image-crop, or image-convert via ref. Provide either url (HTTP/HTTPS) or ref (id from a prior tool call).",
        parameters = r#"{
            "type": "object",
            "properties": {
                "url":       { "type": "string" },
                "ref":       { "type": "string" },
                "timestamp": { "type": "number", "minimum": 0, "description": "Timestamp in seconds." }
            },
            "required": ["timestamp"],
            "oneOf": [
                { "required": ["url"] },
                { "required": ["ref"] }
            ]
        }"#
    ),
)]
impl VideoFrameExtract {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("video-frame-extract")?;
    if args.timestamp < 0.0 || !args.timestamp.is_finite() {
        return Err(SkillError::InvalidArgs(format!(
            "invalid video-frame-extract args: timestamp must be >= 0 and finite, got {}",
            args.timestamp
        )));
    }

    let (input_bytes, in_mime, in_filename) = match args.source.into_inner() {
        Source::Url(u) => fetch_from_url(&u, AssetKind::Video, MAX_INPUT_BYTES)?,
        Source::Ref(id) => load_from_attachment(&id, AssetKind::Video, MAX_INPUT_BYTES)?,
    };

    let in_ext = mime_to_ext(&in_mime).unwrap_or("bin");
    let ffmpeg_in = format!("in.{in_ext}");
    let ffmpeg_out = "out.png".to_string();
    let argv = build_argv(&ffmpeg_in, &ffmpeg_out, args.timestamp);

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
            kind: "output frame",
            bytes: ff.output.len(),
            cap: MAX_OUTPUT_BYTES,
        });
    }

    let output_size = ff.output.len();
    let encoded = B64.encode(&ff.output);
    let data_url = format!("data:image/png;base64,{encoded}");
    let filename = filename_with_suffix(&in_filename, &format!("-frame-{}", args.timestamp), "png");

    let env = Envelope {
        for_llm: format!(
            "extracted frame at {}s from {} (PNG, {} bytes)",
            args.timestamp, in_filename, output_size
        ),
        for_ui: ForUi {
            data_url,
            mime: "image/png".to_string(),
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
        let argv = build_argv("in.mp4", "out.png", 5.0);
        assert_eq!(argv[0], "-ss");
        assert_eq!(argv[1], "5");
        assert_eq!(argv[2], "-i");
        assert_eq!(argv[3], "in.mp4");
        assert!(argv.iter().any(|a| a == "-frames:v"));
        assert!(argv.iter().any(|a| a == "1"));
        assert_eq!(argv.last().map(String::as_str), Some("out.png"));
    }
}
