//! gizza-ai/web-fetch — fetches a URL via wafer-run/network.

use std::collections::HashMap;

use gizza_ai_block_utils::{SkillError, SkillResultExt};
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
    skill(
        description = "Fetch a URL and return its body as text. Optionally limit the response size.",
        parameters = r#"{
            "type": "object",
            "properties": {
                "url":       { "type": "string", "description": "HTTP/HTTPS URL to fetch." },
                "max_bytes": { "type": "integer", "minimum": 1, "description": "Maximum number of bytes to return (default: 1048576). Response is truncated if larger." }
            },
            "required": ["url"],
            "additionalProperties": false
        }"#
    ),
)]
impl WebFetch {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("web-fetch")?;
    let max_bytes = args.max_bytes.unwrap_or(DEFAULT_MAX_BYTES);

    let resp = wafer_sdk::clients::network::do_request("GET", &args.url, &HashMap::new(), None)?;

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
    serde_json::to_vec(&tool).map_err(|e| SkillError::Serialize(format!("serialize tool response: {e}")))
}
