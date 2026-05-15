//! Dispatch a single skill block and emit the resulting `tool_result` + `done`
//! SSE events, plus the response-envelope parser that bifurcates a skill's
//! body into an LLM-safe summary and an optional UI render hint.

use wafer_block::{
    context::Context, core_types::Message, meta::META_REQ_ACTION,
    streams::output::BufferedResponse, Attachment,
};

use super::{sse::encode_sse_event, SKILL_PREFIX};

/// Output of a skill dispatch split into an LLM-safe summary and an optional
/// UI render hint.
#[derive(Debug, Clone)]
pub(super) struct ToolOutcome {
    pub for_llm: String,
    pub for_ui: Option<serde_json::Value>,
}

/// Parse a skill's response body. A response is treated as an envelope iff
/// it parses as JSON, the top-level value is an object, and it has a
/// `_for_llm` field of type string. Otherwise the whole body becomes
/// `for_llm` and `for_ui` is None — preserving legacy plain-text behavior.
pub(super) fn parse_skill_response(body: &str) -> ToolOutcome {
    let Ok(v) = serde_json::from_str::<serde_json::Value>(body) else {
        return ToolOutcome {
            for_llm: body.to_string(),
            for_ui: None,
        };
    };
    let Some(for_llm) = v.get("_for_llm").and_then(|x| x.as_str()) else {
        return ToolOutcome {
            for_llm: body.to_string(),
            for_ui: None,
        };
    };
    let for_ui = v.get("_for_ui").cloned().filter(|x| x.is_object());
    ToolOutcome {
        for_llm: for_llm.to_string(),
        for_ui,
    }
}

/// Dispatch one skill and emit the resulting `tool_result` + `done` SSE
/// events into `sse`. `invocation` is a short string used to mint the
/// `tool_result.id` (e.g. "slash", "confirmed") to avoid id collisions with
/// the legacy `call_N` ids the chat history machinery still understands.
pub(super) async fn run_skill_dispatch(
    ctx: &dyn Context,
    cmd: &str,
    params: serde_json::Value,
    invocation: &str,
    staged_uploads: &[(String, Attachment, String)],
    sse: &mut String,
) {
    let block_name = format!("{SKILL_PREFIX}{cmd}");
    let id = format!("{invocation}_{cmd}");

    // Stash any user-uploaded refs so a slash command can forward them via
    // `{ref: "upload_N"}` in its params. Outgoing attachments are scoped per
    // dispatch — only the upload_* refs that match keys in `params.ref` flow
    // through.
    let mut outgoing: std::collections::BTreeMap<String, Attachment> =
        std::collections::BTreeMap::new();
    if let Some(ref_id) = params.get("ref").and_then(|v| v.as_str()) {
        if let Some((_, att, _)) = staged_uploads.iter().find(|(uid, _, _)| uid == ref_id) {
            outgoing.insert(ref_id.to_string(), att.clone());
        } else {
            sse.push_str(&encode_sse_event(
                "tool_result",
                &serde_json::json!({
                    "id": id,
                    "input": params,
                    "result": format!(r#"{{"error":"unknown_ref","message":"no attachment for ref {ref_id:?}"}}"#),
                }),
            ));
            sse.push_str(&encode_sse_event(
                "done",
                &serde_json::json!({ "reason": "stop" }),
            ));
            return;
        }
    }

    // serde_json::Value always serializes successfully, but `.expect()` is a
    // hard trap on wasm32 — fall back to an empty object instead. The skill
    // block will surface its own InvalidArgs if it requires a non-empty body.
    let args_bytes = serde_json::to_vec(&params).unwrap_or_else(|_| b"{}".to_vec());
    let mut msg = Message::new("http");
    msg.set_meta(META_REQ_ACTION, "create");
    msg.set_meta(wafer_block::meta::META_REQ_RESOURCE, format!("/b/{cmd}"));

    let outcome = match ctx
        .call_block_buffered_with_attachments(&block_name, msg, &args_bytes, outgoing)
        .await
    {
        Ok(BufferedResponse { body, .. }) => {
            let result_text = String::from_utf8_lossy(&body).to_string();
            parse_skill_response(&result_text)
        }
        Err(e) => ToolOutcome {
            for_llm: format!("{{\"error\": \"tool_failed\", \"message\": \"{e}\"}}"),
            for_ui: None,
        },
    };

    let mut payload = serde_json::json!({
        "id": id,
        "input": params,
        "result": outcome.for_llm,
    });
    if let Some(ref for_ui) = outcome.for_ui {
        payload["for_ui"] = for_ui.clone();
    }
    sse.push_str(&encode_sse_event("tool_result", &payload));
    sse.push_str(&encode_sse_event(
        "done",
        &serde_json::json!({ "reason": "stop" }),
    ));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_skill_response_envelope_full_payload() {
        let body = r#"{"_for_llm":"summary text","_for_ui":{"data_url":"data:image/png;base64,AAA","mime":"image/png"}}"#;
        let outcome = parse_skill_response(body);
        assert_eq!(outcome.for_llm, "summary text");
        let for_ui = outcome.for_ui.expect("for_ui present");
        assert_eq!(for_ui["data_url"], "data:image/png;base64,AAA");
        assert_eq!(for_ui["mime"], "image/png");
    }

    #[test]
    fn parse_skill_response_legacy_string_falls_back() {
        let body = "raw text response";
        let outcome = parse_skill_response(body);
        assert_eq!(outcome.for_llm, "raw text response");
        assert!(outcome.for_ui.is_none());
    }

    #[test]
    fn parse_skill_response_json_without_for_llm_falls_back() {
        let body = r#"{"foo":"bar","baz":42}"#;
        let outcome = parse_skill_response(body);
        assert_eq!(outcome.for_llm, body);
        assert!(outcome.for_ui.is_none());
    }

    #[test]
    fn parse_skill_response_for_ui_must_be_object() {
        let body = r#"{"_for_llm":"ok","_for_ui":"not-an-object"}"#;
        let outcome = parse_skill_response(body);
        assert_eq!(outcome.for_llm, "ok");
        assert!(outcome.for_ui.is_none());
    }
}
