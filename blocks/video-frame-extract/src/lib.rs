//! gizza-ai/video-frame-extract — fetch a video URL or attachment ref, extract a single frame as PNG.

// The #[wafer_block] macro emits the impl gated to wasm32; supporting imports,
// constants, and the Args type are only used there. See image-resize for the
// full rationale.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use std::collections::HashMap;

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use gizza_ai_block_utils::{
    derive_filename, dispatch_ffmpeg_runtime, mime_to_ext, pick_source, Envelope, FfmpegReq,
    FfmpegResp, ForUi, Source,
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
        let args: Args = match serde_json::from_slice(&body) {
            Ok(a) => a,
            Err(e) => {
                return GuestResult::error(WaferError::new(
                    ErrorCode::INVALID_ARGUMENT,
                    format!("invalid video-frame-extract args: {e}"),
                ))
            }
        };
        if args.timestamp < 0.0 || !args.timestamp.is_finite() {
            return GuestResult::error(WaferError::new(
                ErrorCode::INVALID_ARGUMENT,
                format!("invalid video-frame-extract args: timestamp must be >= 0 and finite, got {}", args.timestamp),
            ));
        }

        let (input_bytes, in_mime, in_filename) = match pick_source(args.url.as_deref(), args.r#ref.as_deref()) {
            Err(e) => {
                return GuestResult::error(WaferError::new(
                    ErrorCode::INVALID_ARGUMENT,
                    format!("invalid video-frame-extract args: {e}"),
                ))
            }
            Ok(Source::Url(u)) => {
                let net = match wafer_sdk::clients::network::do_request("GET", &u, &HashMap::new(), None) {
                    Ok(r) => r,
                    Err(e) => return GuestResult::error(e),
                };
                if net.status_code >= 400 {
                    return GuestResult::error(WaferError::new(
                        ErrorCode::UNAVAILABLE,
                        format!("HTTP {} for {}", net.status_code, u),
                    ));
                }
                let raw_mime = net
                    .headers
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
                    .and_then(|(_, vs)| vs.first().cloned())
                    .unwrap_or_else(|| "application/octet-stream".to_string());
                let in_mime: String = raw_mime
                    .split(';')
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_lowercase();
                if !in_mime.starts_with("video/") {
                    return GuestResult::error(WaferError::new(
                        ErrorCode::INVALID_ARGUMENT,
                        format!("expected video/* content-type, got {in_mime}"),
                    ));
                }
                if let Some(cl) = net
                    .headers
                    .iter()
                    .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
                    .and_then(|(_, vs)| vs.first())
                    .and_then(|v| v.trim().parse::<usize>().ok())
                {
                    if cl > MAX_INPUT_BYTES {
                        return GuestResult::error(WaferError::new(
                            ErrorCode::OUT_OF_RANGE,
                            format!("input video too large: {cl} bytes (cap {MAX_INPUT_BYTES} bytes)"),
                        ));
                    }
                }
                if net.body.len() > MAX_INPUT_BYTES {
                    return GuestResult::error(WaferError::new(
                        ErrorCode::OUT_OF_RANGE,
                        format!(
                            "input video too large: {} bytes (cap {} bytes)",
                            net.body.len(),
                            MAX_INPUT_BYTES
                        ),
                    ));
                }
                let filename = derive_filename(&u, "video");
                (net.body, in_mime, filename)
            }
            Ok(Source::Ref(id)) => {
                let att = match wafer_sdk::lookup_attachment(&id) {
                    Ok(Some(a)) => a,
                    Ok(None) => {
                        return GuestResult::error(WaferError::new(
                            ErrorCode::NOT_FOUND,
                            format!("no attachment found for ref {:?}", id),
                        ))
                    }
                    Err(e) => {
                        return GuestResult::error(WaferError::new(
                            ErrorCode::INTERNAL,
                            e.to_string(),
                        ))
                    }
                };
                if !att.mime.starts_with("video/") {
                    return GuestResult::error(WaferError::new(
                        ErrorCode::INVALID_ARGUMENT,
                        format!("expected video/* attachment, got {}", att.mime),
                    ));
                }
                if att.bytes.len() > MAX_INPUT_BYTES {
                    return GuestResult::error(WaferError::new(
                        ErrorCode::OUT_OF_RANGE,
                        format!(
                            "input video too large: {} bytes (cap {} bytes)",
                            att.bytes.len(),
                            MAX_INPUT_BYTES
                        ),
                    ));
                }
                let filename = att.filename.unwrap_or_else(|| "video".into());
                (att.bytes, att.mime, filename)
            }
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
        let req_body = match serde_json::to_vec(&req) {
            Ok(b) => b,
            Err(e) => {
                return GuestResult::error(WaferError::new(
                    ErrorCode::INTERNAL,
                    format!("serialize ffmpeg request: {e}"),
                ))
            }
        };
        let ff_resp_bytes = match dispatch_ffmpeg_runtime(&req_body) {
            Ok(b) => b,
            Err(e) => return GuestResult::error(e),
        };
        let ff: FfmpegResp = match serde_json::from_slice(&ff_resp_bytes) {
            Ok(r) => r,
            Err(e) => {
                return GuestResult::error(WaferError::new(
                    ErrorCode::INTERNAL,
                    format!("malformed ffmpeg response: {e}"),
                ))
            }
        };

        if ff.exit_code != 0 {
            let snippet: String = ff.log.chars().take(200).collect();
            return GuestResult::error(WaferError::new(
                ErrorCode::INTERNAL,
                format!("ffmpeg failed (exit {}): {snippet}", ff.exit_code),
            ));
        }
        if ff.output.len() > MAX_OUTPUT_BYTES {
            return GuestResult::error(WaferError::new(
                ErrorCode::OUT_OF_RANGE,
                format!(
                    "output frame too large: {} bytes (cap {} bytes)",
                    ff.output.len(),
                    MAX_OUTPUT_BYTES
                ),
            ));
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
        match serde_json::to_vec(&env) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(WaferError::new(
                ErrorCode::INTERNAL,
                format!("serialize envelope: {e}"),
            )),
        }
    }
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
