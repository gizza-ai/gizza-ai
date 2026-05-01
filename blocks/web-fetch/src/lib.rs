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
struct NetReq<'a> {
    method: &'a str,
    url: &'a str,
    headers: HashMap<String, String>,
}

#[derive(Deserialize)]
struct NetResp {
    status_code: u16,
    headers: HashMap<String, Vec<String>>,
    body: Vec<u8>,
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

        let net_req = NetReq {
            method: "GET",
            url: &args.url,
            headers: HashMap::new(),
        };
        let net_body = match serde_json::to_vec(&net_req) {
            Ok(b) => b,
            Err(e) => {
                return GuestResult::error(WaferError::new(
                    ErrorCode::INTERNAL,
                    format!("serialize network request: {e}"),
                ));
            }
        };

        let (_resp_msg, resp_bytes) =
            match call_block("wafer-run/network", Message::new("network.do"), &net_body) {
                Ok(ok) => ok,
                Err(CallBlockError::Error(e)) => return GuestResult::error(e),
                Err(other) => {
                    return GuestResult::error(WaferError::new(
                        ErrorCode::UNAVAILABLE,
                        format!("network call failed: {other:?}"),
                    ));
                }
            };

        let net: NetResp = match serde_json::from_slice(&resp_bytes) {
            Ok(n) => n,
            Err(e) => {
                return GuestResult::error(WaferError::new(
                    ErrorCode::INTERNAL,
                    format!("malformed network response: {e}"),
                ));
            }
        };

        let content_type = net
            .headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
            .and_then(|(_, vs)| vs.first().cloned());
        let truncated = net.body.len() > max_bytes;
        let bytes = if truncated {
            &net.body[..max_bytes]
        } else {
            &net.body[..]
        };
        let body_str = String::from_utf8_lossy(bytes).into_owned();

        let tool = ToolResp {
            status: net.status_code,
            url: args.url,
            content_type,
            body: body_str,
            truncated,
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
