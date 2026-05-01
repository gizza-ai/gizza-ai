//! gizza-ai/ffmpeg — fetch a URL, run `ffmpeg -i input` on the bytes,
//! return the log.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use wafer_sdk::*;

const MAX_BYTES: usize = 16 * 1024 * 1024; // 16 MiB

#[derive(Deserialize)]
struct Args {
    url: String,
}

#[derive(Serialize)]
struct NetReq<'a> {
    method: &'a str,
    url: &'a str,
    headers: HashMap<String, String>,
}

#[derive(Deserialize)]
struct NetResp {
    status_code: u16,
    #[allow(dead_code)]
    headers: HashMap<String, Vec<String>>,
    body: Vec<u8>,
}

#[derive(Serialize)]
struct FfmpegReq {
    args: Vec<String>,
    inputs: Vec<(String, Vec<u8>)>,
    output: String,
}

#[derive(Deserialize)]
struct FfmpegResp {
    #[allow(dead_code)]
    exit_code: i32,
    #[allow(dead_code)]
    output: Vec<u8>,
    log: String,
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
)]
impl FfmpegSkill {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        let args: Args = match serde_json::from_slice(&body) {
            Ok(a) => a,
            Err(e) => {
                return GuestResult::error(WaferError::new(
                    ErrorCode::INVALID_ARGUMENT,
                    format!("invalid ffmpeg args: {e}"),
                ));
            }
        };

        // 1. Fetch bytes via wafer-run/network.
        let net_body = match serde_json::to_vec(&NetReq {
            method: "GET",
            url: &args.url,
            headers: HashMap::new(),
        }) {
            Ok(v) => v,
            Err(e) => {
                return GuestResult::error(WaferError::new(
                    ErrorCode::INTERNAL,
                    format!("serialize network request: {e}"),
                ));
            }
        };
        let (_msg, net_resp_bytes) =
            match call_block("wafer-run/network", Message::new("network.do"), &net_body) {
                Ok(v) => v,
                Err(CallBlockError::Error(e)) => return GuestResult::error(e),
                Err(other) => {
                    return GuestResult::error(WaferError::new(
                        ErrorCode::UNAVAILABLE,
                        format!("network call failed: {other:?}"),
                    ));
                }
            };
        let net: NetResp = match serde_json::from_slice(&net_resp_bytes) {
            Ok(v) => v,
            Err(e) => {
                return GuestResult::error(WaferError::new(
                    ErrorCode::INTERNAL,
                    format!("malformed network response: {e}"),
                ));
            }
        };
        if net.status_code >= 400 {
            return GuestResult::error(WaferError::new(
                ErrorCode::UNAVAILABLE,
                format!("HTTP {} for {}", net.status_code, args.url),
            ));
        }
        if net.body.len() > MAX_BYTES {
            return GuestResult::error(WaferError::new(
                ErrorCode::OUT_OF_RANGE,
                format!(
                    "media file too large: {} bytes (cap {})",
                    net.body.len(),
                    MAX_BYTES
                ),
            ));
        }

        // 2. Hand bytes to ffmpeg-runtime.
        let ffreq = match serde_json::to_vec(&FfmpegReq {
            args: vec!["-i".into(), "input".into()],
            inputs: vec![("input".into(), net.body)],
            output: String::new(),
        }) {
            Ok(v) => v,
            Err(e) => {
                return GuestResult::error(WaferError::new(
                    ErrorCode::INTERNAL,
                    format!("serialize ffmpeg request: {e}"),
                ));
            }
        };
        let (_msg, ff_resp_bytes) = match call_block(
            "gizza-ai/ffmpeg-runtime",
            Message::new("ffmpeg.exec"),
            &ffreq,
        ) {
            Ok(v) => v,
            Err(CallBlockError::Error(e)) => return GuestResult::error(e),
            Err(other) => {
                return GuestResult::error(WaferError::new(
                    ErrorCode::UNAVAILABLE,
                    format!("ffmpeg call failed: {other:?}"),
                ));
            }
        };
        let ff: FfmpegResp = match serde_json::from_slice(&ff_resp_bytes) {
            Ok(v) => v,
            Err(e) => {
                return GuestResult::error(WaferError::new(
                    ErrorCode::INTERNAL,
                    format!("malformed ffmpeg response: {e}"),
                ));
            }
        };
        // ffmpeg's exit code is expected to be non-zero when no output file
        // is produced; the log is what we want regardless.

        let tool = ToolResp {
            url: args.url,
            info: ff.log,
        };
        match serde_json::to_vec(&tool) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(WaferError::new(
                ErrorCode::INTERNAL,
                format!("serialize tool response: {e}"),
            )),
        }
    }
}
