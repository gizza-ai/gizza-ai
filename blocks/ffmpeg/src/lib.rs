//! gizza-ai/ffmpeg — fetch a URL, run `ffmpeg -i input` on the bytes,
//! return the log.

// The #[wafer_block] macro emits wasm-only registration; supporting imports
// and the Args type are only used inside that impl. See image-resize for
// the full rationale.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use std::collections::HashMap;

use gizza_ai_block_utils::{
    dispatch_ffmpeg_runtime, FfmpegReq, FfmpegResp, SkillError, SkillResultExt,
};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

const MAX_BYTES: usize = 16 * 1024 * 1024; // 16 MiB

#[derive(Deserialize)]
struct Args {
    url: String,
}

#[derive(Serialize)]
struct ToolResp {
    url: String,
    info: String,
}

/// argv for an ffprobe-style inspection — read `input` from ffmpeg's
/// virtual FS, emit format/stream metadata on stderr, no output file.
fn build_inspect_argv() -> Vec<String> {
    vec!["-i".into(), "input".into()]
}

#[cfg(target_arch = "wasm32")]
struct FfmpegSkill;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/ffmpeg",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Inspect a media file via ffmpeg",
    capabilities(callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(
        description = "Run ffprobe on a media URL and return format/stream metadata.",
        parameters = r#"{
            "type": "object",
            "properties": {
                "url": { "type": "string", "description": "HTTP/HTTPS URL of the media file to inspect." }
            },
            "required": ["url"],
            "additionalProperties": false
        }"#
    ),
)]
impl FfmpegSkill {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("ffmpeg")?;

    let net = wafer_sdk::clients::network::do_request("GET", &args.url, &HashMap::new(), None)?;
    if net.status_code >= 400 {
        return Err(SkillError::HttpStatus {
            status: net.status_code,
            url: args.url,
        });
    }
    if net.body.len() > MAX_BYTES {
        return Err(SkillError::TooLarge {
            kind: "media file",
            bytes: net.body.len(),
            cap: MAX_BYTES,
        });
    }

    // Hand bytes to gizza-ai/ffmpeg-runtime (consumer-controlled JSON
    // protocol, NOT a wafer-run service — must encode via serde_json).
    let ffreq = serde_json::to_vec(&FfmpegReq {
        args: build_inspect_argv(),
        inputs: vec![("input".into(), net.body)],
        output: String::new(),
    })
    .map_err(|e| SkillError::Serialize(format!("serialize ffmpeg request: {e}")))?;
    let ff_resp_bytes = dispatch_ffmpeg_runtime(&ffreq)?;
    let ff: FfmpegResp = serde_json::from_slice(&ff_resp_bytes)
        .map_err(|e| SkillError::Serialize(format!("malformed ffmpeg response: {e}")))?;
    // ffmpeg's exit code is expected to be non-zero when no output file is
    // produced; the log is what we want regardless.

    let tool = ToolResp {
        url: args.url,
        info: ff.log,
    };
    serde_json::to_vec(&tool)
        .map_err(|e| SkillError::Serialize(format!("serialize tool response: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn argv_uses_input_filename_with_dash_i() {
        let argv = build_inspect_argv();
        assert_eq!(argv, vec!["-i", "input"]);
    }

    #[test]
    fn tool_resp_serializes_url_and_info() {
        let tool = ToolResp {
            url: "https://x.test/a.mp4".to_string(),
            info: "Stream #0: Video".to_string(),
        };
        let json: serde_json::Value = serde_json::from_slice(&serde_json::to_vec(&tool).unwrap()).unwrap();
        assert_eq!(json["url"], "https://x.test/a.mp4");
        assert_eq!(json["info"], "Stream #0: Video");
    }
}
