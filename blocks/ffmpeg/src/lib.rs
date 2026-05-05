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
        // Skill input parsing: LLM tool-call args (JSON wire format).
        let args: Args = match serde_json::from_slice(&body) {
            Ok(a) => a,
            Err(e) => {
                return GuestResult::error(WaferError::new(
                    ErrorCode::INVALID_ARGUMENT,
                    format!("invalid ffmpeg args: {e}"),
                ));
            }
        };

        // 1. Fetch bytes via wafer-run/network (typed binary transport).
        let net = match wafer_sdk::clients::network::do_request(
            "GET",
            &args.url,
            &HashMap::new(),
            None,
        ) {
            Ok(r) => r,
            Err(e) => return GuestResult::error(e),
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

        // 2. Hand bytes to gizza-ai/ffmpeg-runtime.
        //
        // Note: this call site uses the consumer-controlled custom protocol
        // (FfmpegReq/FfmpegResp serde_json) — NOT a wafer-run service. The
        // ffmpeg-runtime block (`src/blocks/ffmpeg.rs`) decodes via
        // `serde_json::from_slice`, so the skill must encode via serde_json.
        // Migrate together with that block when/if it adopts codec.
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
        // Dispatch via raw streaming ABI: the wire format is opaque serde_json
        // (consumer-controlled custom protocol, not a wafer-run service), but
        // the transport mechanics use the new streaming ABI. Open call → write
        // single chunk → finish → drain response chunks → concatenate.
        let ff_resp_bytes = match dispatch_ffmpeg_runtime(&ffreq) {
            Ok(b) => b,
            Err(e) => return GuestResult::error(e),
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
        // Skill output emission: LLM tool-call result (JSON wire format).
        match serde_json::to_vec(&tool) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(WaferError::new(
                ErrorCode::INTERNAL,
                format!("serialize tool response: {e}"),
            )),
        }
    }
}

/// Dispatch a request to `gizza-ai/ffmpeg-runtime` via the raw streaming ABI.
///
/// The ffmpeg-runtime block uses a consumer-controlled JSON wire format (not a
/// wafer-run service), so we hand it an opaque `Vec<u8>` payload and accept
/// opaque chunks back. The transport (CallStream/ResponseStream) is still the
/// new binary-transport ABI; only the encoding inside the chunks is JSON.
fn dispatch_ffmpeg_runtime(payload: &[u8]) -> Result<Vec<u8>, WaferError> {
    let msg = Message::new("ffmpeg.exec");
    let mut call = wafer_sdk::stream::CallStream::open("gizza-ai/ffmpeg-runtime", &msg)?;
    call.write_chunk(payload)?;
    let mut resp = call.finish()?;
    let mut out = Vec::new();
    while let Some(chunk) = resp.next_chunk()? {
        out.extend(chunk);
    }
    Ok(out)
}
