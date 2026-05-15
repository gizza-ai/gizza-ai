//! SSE event encoding + canned response builders for the agent block.

use wafer_block::{
    core_types::{ErrorCode, MetaEntry, WaferError},
    meta::{META_RESP_CONTENT_TYPE, META_RESP_STATUS},
    streams::output::OutputStream,
};

pub(super) fn encode_sse_event(event_name: &str, data: &serde_json::Value) -> String {
    let data_str = serde_json::to_string(data).unwrap_or_else(|_| "null".to_string());
    format!("event: {event_name}\ndata: {data_str}\n\n")
}

pub(super) fn sse_response(body: String) -> OutputStream {
    OutputStream::respond_with_meta(
        body.into_bytes(),
        vec![
            MetaEntry {
                key: META_RESP_STATUS.to_string(),
                value: "200".to_string(),
            },
            MetaEntry {
                key: META_RESP_CONTENT_TYPE.to_string(),
                value: "text/event-stream".to_string(),
            },
            MetaEntry {
                key: format!(
                    "{}cache-control",
                    wafer_block::meta::META_RESP_HEADER_PREFIX
                ),
                value: "no-cache".to_string(),
            },
        ],
    )
}

pub(super) fn error_response(status: u16, code: &str, message: &str) -> OutputStream {
    let body = serde_json::json!({
        "error": code,
        "message": message,
    });
    match serde_json::to_vec(&body) {
        Ok(bytes) => OutputStream::respond_with_meta(
            bytes,
            vec![
                MetaEntry {
                    key: META_RESP_STATUS.to_string(),
                    value: status.to_string(),
                },
                MetaEntry {
                    key: META_RESP_CONTENT_TYPE.to_string(),
                    value: "application/json".to_string(),
                },
            ],
        ),
        Err(e) => OutputStream::error(WaferError {
            code: ErrorCode::Internal,
            message: format!("failed to serialise error body: {e}"),
            meta: vec![],
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_sse_event_basic_shape() {
        let s = encode_sse_event("token", &serde_json::json!({ "delta": "hi" }));
        assert_eq!(s, "event: token\ndata: {\"delta\":\"hi\"}\n\n");
    }

    #[test]
    fn encode_sse_event_done_reason() {
        let s = encode_sse_event("done", &serde_json::json!({ "reason": "stop" }));
        assert_eq!(s, "event: done\ndata: {\"reason\":\"stop\"}\n\n");
    }
}
