//! gizza-ai/video-frame-extract — fetch a video URL or attachment ref, extract a single frame as PNG.

// The #[wafer_block] macro emits the impl gated to wasm32; supporting imports,
// constants, and the Args type are only used there. See image-resize for the
// full rationale.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use std::collections::HashMap;

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use gizza_ai_block_utils::{
    derive_filename, dispatch_ffmpeg_runtime, mime_to_ext, pick_source, Envelope, FfmpegReq,
    FfmpegResp, ForUi, SkillError, SkillResultExt, Source,
};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_INPUT_BYTES: usize = 10 * 1024 * 1024;
const MAX_OUTPUT_BYTES: usize = 10 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    r#ref: Option<String>,
    timestamp: f64,
}

fn in_filename_stem(name: &str) -> &str {
    name.rsplit_once('.').map(|(s, _)| s).unwrap_or(name)
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
    capabilities(callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
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

    let (input_bytes, in_mime, in_filename) =
        match pick_source(args.url.as_deref(), args.r#ref.as_deref())
            .invalid_args("video-frame-extract")?
        {
            Source::Url(u) => fetch_video_from_url(&u)?,
            Source::Ref(id) => load_video_from_attachment(&id)?,
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
    let stem = in_filename_stem(&in_filename);
    let filename = format!("{stem}-frame-{}.png", args.timestamp);

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

#[cfg(target_arch = "wasm32")]
fn fetch_video_from_url(url: &str) -> Result<(Vec<u8>, String, String), SkillError> {
    let net = wafer_sdk::clients::network::do_request("GET", url, &HashMap::new(), None)?;
    if net.status_code >= 400 {
        return Err(SkillError::HttpStatus {
            status: net.status_code,
            url: url.to_string(),
        });
    }
    let raw_mime = net
        .headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
        .and_then(|(_, vs)| vs.first().cloned())
        .unwrap_or_else(|| "application/octet-stream".to_string());
    let mime: String = raw_mime
        .split(';')
        .next()
        .unwrap_or("")
        .trim()
        .to_lowercase();
    if !mime.starts_with("video/") {
        return Err(SkillError::UnexpectedMime {
            expected: "video/*",
            actual: mime,
        });
    }
    if let Some(cl) = net
        .headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
        .and_then(|(_, vs)| vs.first())
        .and_then(|v| v.trim().parse::<usize>().ok())
    {
        if cl > MAX_INPUT_BYTES {
            return Err(SkillError::TooLarge {
                kind: "input video",
                bytes: cl,
                cap: MAX_INPUT_BYTES,
            });
        }
    }
    if net.body.len() > MAX_INPUT_BYTES {
        return Err(SkillError::TooLarge {
            kind: "input video",
            bytes: net.body.len(),
            cap: MAX_INPUT_BYTES,
        });
    }
    let filename = derive_filename(url, "video");
    Ok((net.body, mime, filename))
}

#[cfg(target_arch = "wasm32")]
fn load_video_from_attachment(id: &str) -> Result<(Vec<u8>, String, String), SkillError> {
    let att = wafer_sdk::lookup_attachment(id)
        .map_err(|e| SkillError::Serialize(e.to_string()))?
        .ok_or_else(|| SkillError::AttachmentNotFound(id.to_string()))?;
    if !att.mime.starts_with("video/") {
        return Err(SkillError::UnexpectedMime {
            expected: "video/* attachment",
            actual: att.mime,
        });
    }
    if att.bytes.len() > MAX_INPUT_BYTES {
        return Err(SkillError::TooLarge {
            kind: "input video",
            bytes: att.bytes.len(),
            cap: MAX_INPUT_BYTES,
        });
    }
    let filename = att.filename.unwrap_or_else(|| "video".into());
    Ok((att.bytes, att.mime, filename))
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
