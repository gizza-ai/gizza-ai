//! gizza-ai/ffmpeg — fetch a URL, run `ffmpeg -i input` on the bytes,
//! return the log.

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

struct FfmpegSkill;

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
        args: vec!["-i".into(), "input".into()],
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
