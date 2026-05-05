//! gizza-ai/web-fetch — fetches a URL via wafer-run/network.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use wafer_sdk::*;

const DEFAULT_MAX_BYTES: usize = 1 << 20; // 1 MiB

#[derive(Deserialize)]
struct Args {
    url: String,
    #[serde(default)]
    max_bytes: Option<usize>,
}

#[derive(Serialize)]
struct ToolResp {
    status: u16,
    url: String,
    content_type: Option<String>,
    body: String,
    truncated: bool,
}

struct WebFetch;

#[wafer_block(
    name = "gizza-ai/web-fetch",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Fetch a URL and return its body",
    capabilities(callable_blocks = ["wafer-run/network"]),
)]
impl WebFetch {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // Skill input parsing: LLM tool-call args (JSON wire format).
        let args: Args = match serde_json::from_slice(&body) {
            Ok(a) => a,
            Err(e) => {
                return GuestResult::error(WaferError::new(
                    ErrorCode::INVALID_ARGUMENT,
                    format!("invalid web-fetch args: {e}"),
                ));
            }
        };
        let max_bytes = args.max_bytes.unwrap_or(DEFAULT_MAX_BYTES);

        let resp = match wafer_sdk::clients::network::do_request(
            "GET",
            &args.url,
            &HashMap::new(),
            None,
        ) {
            Ok(r) => r,
            Err(e) => return GuestResult::error(e),
        };

        let content_type = resp
            .headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
            .and_then(|(_, vs)| vs.first().cloned());
        let truncated = resp.body.len() > max_bytes;
        let bytes = if truncated {
            &resp.body[..max_bytes]
        } else {
            &resp.body[..]
        };
        let body_str = String::from_utf8_lossy(bytes).into_owned();

        let tool = ToolResp {
            status: resp.status_code,
            url: args.url,
            content_type,
            body: body_str,
            truncated,
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
