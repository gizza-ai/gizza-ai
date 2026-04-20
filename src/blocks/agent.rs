//! gizza-ai/agent — tool-using chat agent block.
//!
//! Protocol (route POST /b/agent/chat):
//!   Request body: { "user_message": "...", "messages": [ ...prior... ] }
//!   Response:     text/event-stream with events:
//!                 event: token        data: { "delta": "..." }
//!                 event: tool_call    data: { "id": "...", "name": "gizza-ai/clock", "arguments": "{}" }
//!                 event: tool_result  data: { "id": "...", "result": "..." }
//!                 event: done         data: { "reason": "stop" | "max_rounds_exceeded" | "error" }
//!
//! Algorithm:
//! 1. Enumerate every registered block whose `role == Some(SkillRole::Skill)` and build
//!    an OpenAI-format `tools` array from each `BlockInfo::tool`.
//! 2. Invoke `suppers-ai/local-llm`'s `chat_stream` endpoint with
//!    `{ messages, tools }` and buffer the SSE response per round.
//! 3. Forward `token` events into the agent's own SSE output. Collect any
//!    `tool_call` events and, after the LLM finishes the round, dispatch each
//!    to `gizza-ai/<name>` via `ctx.call_block`, emit `tool_result`, then
//!    append the tool-call + tool-result pair to the conversation history.
//! 4. Loop up to `MAX_ROUNDS = 5`. When the LLM returns no tool calls, emit
//!    `event: done` with `reason: "stop"` and terminate.
//!
//! Buffering note: for MVP we buffer each LLM round (tool calls only arrive
//! at the end of a round anyway) and emit the agent's response as one
//! OutputStream at the end. Incremental cross-round streaming is a future
//! optimisation.
//!
//! LLM reachability: `suppers-ai/local-llm`'s Rust-side `handle()` returns
//! 501 for all `/b/local-llm/api/*` paths because WebLLM runs on the main
//! thread and the SW intercepts those requests before they reach the WASM
//! runtime. The agent block therefore invokes the SW's `handleLocalLlm`
//! function directly via the wasm-bindgen bridge in `solobase_browser::bridge`. See
//! `solobase-browser/src/bridge.rs` and `site/bridge.js` for the JS glue.

// gizza-ai-specific JS bridge: the local-llm chat_stream binding is not part
// of solobase-browser (which only provides platform-service bridges). Declare
// it here, bound to the same /site/bridge.js that the framework bridges use.
#[cfg(target_arch = "wasm32")]
mod bridge {
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen(module = "/site/sw-llm-bridge.js")]
    extern "C" {
        /// Invoke the SW's local-llm chat_stream handler and collect the SSE
        /// response into a Uint8Array. `body_json` is the OpenAI-format chat
        /// request body. Returns `Result<JsValue, JsValue>` where the Ok variant
        /// is a `Uint8Array` of the full SSE text.
        #[wasm_bindgen(js_name = localLlmChatStream, catch)]
        pub async fn local_llm_chat_stream(body_json: &str) -> Result<JsValue, JsValue>;
    }
}

use async_trait::async_trait;
use serde::Deserialize;
use wafer_block::{
    block::Block,
    context::Context,
    core_types::{ErrorCode, LifecycleEvent, Message, MetaEntry, WaferError},
    meta::{META_REQ_ACTION, META_REQ_RESOURCE, META_RESP_CONTENT_TYPE, META_RESP_STATUS},
    streams::{
        input::InputStream,
        output::{BufferedResponse, OutputStream},
    },
    types::{BlockInfo, SkillRole},
};

/// Maximum agent-loop rounds before giving up.
const MAX_ROUNDS: u32 = 5;

/// The agent block's own chat endpoint.
const AGENT_CHAT_PATH: &str = "/b/agent/chat";

pub struct AgentBlock;

#[derive(Debug, Deserialize)]
struct AgentRequest {
    #[serde(default)]
    user_message: String,
    #[serde(default)]
    messages: Vec<serde_json::Value>,
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl Block for AgentBlock {
    fn info(&self) -> BlockInfo {
        BlockInfo::new(
            "gizza-ai/agent",
            "0.1.0",
            "http-handler@v1",
            "Tool-using chat agent",
        )
        .category(wafer_run::BlockCategory::Feature)
        .description(
            "Drives a tool-use loop against suppers-ai/local-llm, dispatching \
             skill blocks as OpenAI-format tools.",
        )
    }

    async fn handle(&self, ctx: &dyn Context, msg: Message, input: InputStream) -> OutputStream {
        // 1. Route check: only POST /b/agent/chat is supported.
        let action = msg.action();
        let path = msg.path();
        if !(action == "create" && path == AGENT_CHAT_PATH) {
            return error_response(404, "not_found", "unknown agent endpoint");
        }

        // 2. Parse request body.
        let body_bytes = input.collect_to_bytes().await;
        let req: AgentRequest = if body_bytes.is_empty() {
            AgentRequest {
                user_message: String::new(),
                messages: Vec::new(),
            }
        } else {
            match serde_json::from_slice(&body_bytes) {
                Ok(v) => v,
                Err(e) => {
                    return error_response(
                        400,
                        "bad_request",
                        &format!("invalid JSON body: {e}"),
                    );
                }
            }
        };

        if req.user_message.trim().is_empty() && req.messages.is_empty() {
            return error_response(400, "bad_request", "user_message or messages required");
        }

        // 3. Build OpenAI-format tools array from registered skill blocks.
        let tools = build_tools(&ctx.registered_blocks());

        // 4. Compose conversation history: prior messages + new user turn.
        let mut history = req.messages;
        if !req.user_message.is_empty() {
            history.push(serde_json::json!({
                "role": "user",
                "content": req.user_message,
            }));
        }

        // 5. Run the tool-use loop, buffering the SSE output the whole time.
        let sse_body = run_agent_loop(ctx, history, tools).await;

        // 6. Respond with text/event-stream.
        let meta = vec![
            MetaEntry {
                key: META_RESP_STATUS.to_string(),
                value: "200".to_string(),
            },
            MetaEntry {
                key: META_RESP_CONTENT_TYPE.to_string(),
                value: "text/event-stream".to_string(),
            },
            MetaEntry {
                key: format!("{}cache-control", wafer_block::meta::META_RESP_HEADER_PREFIX),
                value: "no-cache".to_string(),
            },
        ];
        OutputStream::respond_with_meta(sse_body.into_bytes(), meta)
    }

    async fn lifecycle(
        &self,
        _ctx: &dyn Context,
        _event: LifecycleEvent,
    ) -> Result<(), WaferError> {
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Agent loop
// ---------------------------------------------------------------------------

/// Run the full tool-use loop. Returns the SSE-encoded response body (UTF-8).
///
/// On native targets this returns an error immediately — the local-llm bridge
/// is wasm32-only. All unit tests exercise helpers (`parse_sse`, `build_tools`,
/// etc.) without going through this function.
#[cfg_attr(
    not(target_arch = "wasm32"),
    allow(
        unused_variables,
        unused_mut,
        unreachable_code,
        clippy::needless_return,
        clippy::unused_unit
    )
)]
async fn run_agent_loop(
    ctx: &dyn Context,
    mut history: Vec<serde_json::Value>,
    tools: Vec<serde_json::Value>,
) -> String {
    let mut out = String::new();

    for round in 1..=MAX_ROUNDS {
        // Build the body for the local-llm call.
        let llm_body = serde_json::json!({
            "messages": history,
            "tools": tools,
        });
        let llm_body_str = match serde_json::to_string(&llm_body) {
            Ok(s) => s,
            Err(e) => {
                out.push_str(&encode_sse_event(
                    "done",
                    &serde_json::json!({
                        "reason": "error",
                        "error": format!("serialize llm body: {e}"),
                    }),
                ));
                return out;
            }
        };

        // Invoke local-llm/chat_stream via the wasm-bindgen JS bridge.
        // The Rust-side local-llm block returns 501 for /b/local-llm/api/*
        // because WebLLM runs in the main thread and the SW handles those
        // paths before they reach the WASM runtime. We go directly to JS.
        #[cfg(target_arch = "wasm32")]
        let sse_text_owned: String = {
            let body_str = llm_body_str;
            match bridge::local_llm_chat_stream(&body_str).await {
                Ok(js_val) => {
                    let u8_array = js_sys::Uint8Array::new(&js_val);
                    let bytes = u8_array.to_vec();
                    match String::from_utf8(bytes) {
                        Ok(s) => s,
                        Err(e) => {
                            out.push_str(&encode_sse_event(
                                "done",
                                &serde_json::json!({
                                    "reason": "error",
                                    "error": format!("local-llm response not utf8: {e}"),
                                }),
                            ));
                            return out;
                        }
                    }
                }
                Err(e) => {
                    out.push_str(&encode_sse_event(
                        "done",
                        &serde_json::json!({
                            "reason": "error",
                            "error": format!("local-llm bridge error: {e:?}"),
                        }),
                    ));
                    return out;
                }
            }
        };

        // Native builds have no LLM bridge — unit tests exercise parse_sse /
        // encode_sse_event / build_tools directly without invoking this path.
        #[cfg(not(target_arch = "wasm32"))]
        let sse_text_owned: String = {
            drop(llm_body_str); // not used on native; suppress unused-variable lint
            out.push_str(&encode_sse_event(
                "done",
                &serde_json::json!({
                    "reason": "error",
                    "error": "local-llm bridge not available on native targets",
                }),
            ));
            return out;
        };

        // Parse the LLM's SSE frames.
        let (events, llm_done_reason) = parse_sse_response(&sse_text_owned);

        // Forward token events and collect tool calls from this round.
        let mut tool_calls: Vec<ToolCall> = Vec::new();
        for (name, data) in events {
            match name.as_str() {
                "token" => {
                    out.push_str(&encode_sse_event("token", &data));
                }
                "tool_call" => {
                    // Forward upstream tool_call to UI immediately.
                    out.push_str(&encode_sse_event("tool_call", &data));
                    if let Some(tc) = ToolCall::from_json(&data) {
                        tool_calls.push(tc);
                    }
                }
                "error" => {
                    out.push_str(&encode_sse_event(
                        "done",
                        &serde_json::json!({
                            "reason": "error",
                            "error": data,
                        }),
                    ));
                    return out;
                }
                _ => {
                    // Unknown events are dropped — they're not part of the contract.
                }
            }
        }

        // No tool calls: forward a done event and terminate.
        if tool_calls.is_empty() {
            out.push_str(&encode_sse_event(
                "done",
                &serde_json::json!({
                    "reason": llm_done_reason.unwrap_or_else(|| "stop".to_string()),
                }),
            ));
            return out;
        }

        // Dispatch each tool call.
        let mut tc_records = Vec::with_capacity(tool_calls.len());
        for tc in &tool_calls {
            let result_text = dispatch_tool(ctx, tc).await;
            out.push_str(&encode_sse_event(
                "tool_result",
                &serde_json::json!({
                    "id": tc.id,
                    "result": result_text,
                }),
            ));
            tc_records.push((tc.clone(), result_text));
        }

        // Append assistant tool-call + tool-result messages to history for the
        // next round. Use the OpenAI function-calling shape.
        let tool_calls_json: Vec<serde_json::Value> = tc_records
            .iter()
            .map(|(tc, _)| {
                serde_json::json!({
                    "id": tc.id,
                    "type": "function",
                    "function": {
                        "name": tc.name,
                        "arguments": tc.arguments,
                    },
                })
            })
            .collect();
        history.push(serde_json::json!({
            "role": "assistant",
            "tool_calls": tool_calls_json,
        }));
        for (tc, result) in tc_records {
            history.push(serde_json::json!({
                "role": "tool",
                "tool_call_id": tc.id,
                "content": result,
            }));
        }

        if round == MAX_ROUNDS {
            out.push_str(&encode_sse_event(
                "done",
                &serde_json::json!({ "reason": "max_rounds_exceeded" }),
            ));
            return out;
        }
    }

    // Defensive fall-through — should be unreachable because the loop bounds
    // are handled above, but Rust can't see that.
    out.push_str(&encode_sse_event(
        "done",
        &serde_json::json!({ "reason": "max_rounds_exceeded" }),
    ));
    out
}

/// Dispatch a single tool call to the corresponding block.
///
/// Tool `name` is treated as either a fully-qualified block name (`org/block`)
/// or a short name that we default to `gizza-ai/{name}`. Returns the tool's
/// response body as UTF-8 text (or an error message, also as text, so the LLM
/// can still see the failure and recover).
async fn dispatch_tool(ctx: &dyn Context, tc: &ToolCall) -> String {
    let block_name = if tc.name.contains('/') {
        tc.name.clone()
    } else {
        format!("gizza-ai/{}", tc.name)
    };

    let mut msg = Message::new("http");
    msg.set_meta(META_REQ_ACTION, "create");
    msg.set_meta(META_REQ_RESOURCE, format!("/b/{}", short_name(&block_name)));

    let args_bytes = tc.arguments.as_bytes().to_vec();
    match ctx.call_block_buffered(&block_name, msg, &args_bytes).await {
        Ok(BufferedResponse { body, .. }) => String::from_utf8_lossy(&body).to_string(),
        Err(e) => format!("{{\"error\": \"tool_failed\", \"message\": \"{}\"}}", e),
    }
}

/// Pull the trailing short name from a `{org}/{block}` block name. Falls back
/// to the full name if no `/` is present.
fn short_name(block: &str) -> &str {
    match block.rsplit_once('/') {
        Some((_, short)) => short,
        None => block,
    }
}

// ---------------------------------------------------------------------------
// Tool enumeration
// ---------------------------------------------------------------------------

/// Enumerate every registered block with `role == Some(SkillRole::Skill)`
/// and `tool.is_some()`, build an OpenAI-compatible tools array.
fn build_tools(blocks: &[BlockInfo]) -> Vec<serde_json::Value> {
    blocks
        .iter()
        .filter_map(|info| match (&info.role, &info.tool) {
            (Some(SkillRole::Skill), Some(tool)) => Some(serde_json::json!({
                "type": "function",
                "function": {
                    "name": info.name,
                    "description": tool.description,
                    "parameters": tool.parameters,
                },
            })),
            _ => None,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// SSE parsing
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct ToolCall {
    id: String,
    name: String,
    /// Raw JSON-string argument payload exactly as emitted by the LLM.
    arguments: String,
}

impl ToolCall {
    fn from_json(v: &serde_json::Value) -> Option<Self> {
        let obj = v.as_object()?;
        let id = obj
            .get("id")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        let name = obj.get("name").and_then(|x| x.as_str())?.to_string();
        let arguments = obj
            .get("arguments")
            .and_then(|x| x.as_str())
            .unwrap_or("{}")
            .to_string();
        Some(Self {
            id,
            name,
            arguments,
        })
    }
}

/// Parse an SSE-framed text payload. Returns `(event_name, data_json)` pairs
/// in order. Lines that don't match the `event:`/`data:` SSE form are
/// ignored. Data payloads that don't parse as JSON are returned as string
/// `Value`s so callers can still inspect them.
fn parse_sse(text: &str) -> Vec<(String, serde_json::Value)> {
    let mut out = Vec::new();
    for frame in text.split("\n\n") {
        let frame = frame.trim_matches(|c: char| c == '\n' || c == '\r');
        if frame.is_empty() {
            continue;
        }
        let mut event_name: Option<String> = None;
        let mut data_lines: Vec<&str> = Vec::new();
        for line in frame.lines() {
            if let Some(rest) = line.strip_prefix("event:") {
                event_name = Some(rest.trim().to_string());
            } else if let Some(rest) = line.strip_prefix("data:") {
                // SSE data lines strip a single leading space if present.
                let trimmed = rest.strip_prefix(' ').unwrap_or(rest);
                data_lines.push(trimmed);
            }
        }
        let Some(name) = event_name else { continue };
        let data_raw = data_lines.join("\n");
        let data_val = serde_json::from_str::<serde_json::Value>(&data_raw)
            .unwrap_or_else(|_| serde_json::Value::String(data_raw));
        out.push((name, data_val));
    }
    out
}

/// Parse the LLM's SSE response text, returning the event list and the
/// reason carried by the final `done` event (if any).
fn parse_sse_response(text: &str) -> (Vec<(String, serde_json::Value)>, Option<String>) {
    let all = parse_sse(text);
    let mut done_reason: Option<String> = None;
    let mut events = Vec::with_capacity(all.len());
    for (name, data) in all {
        if name == "done" {
            done_reason = data
                .get("reason")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
        } else {
            events.push((name, data));
        }
    }
    (events, done_reason)
}

// ---------------------------------------------------------------------------
// SSE emission
// ---------------------------------------------------------------------------

/// Encode a single SSE frame — `event: <name>\ndata: <json>\n\n`.
///
/// JSON is serialised in compact form; no pretty-printing. The frame always
/// ends with the mandatory blank-line separator.
fn encode_sse_event(event_name: &str, data: &serde_json::Value) -> String {
    let data_str = serde_json::to_string(data).unwrap_or_else(|_| "null".to_string());
    format!("event: {event_name}\ndata: {data_str}\n\n")
}

// ---------------------------------------------------------------------------
// Error helper
// ---------------------------------------------------------------------------

/// Build a JSON error response as an `OutputStream`. Distinct from
/// `OutputStream::error(WaferError)` — we want to return a structured JSON
/// body with a specific HTTP status rather than the runtime's error terminal.
fn error_response(status: u16, code: &str, message: &str) -> OutputStream {
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use wafer_block::types::SkillTool;

    #[test]
    fn parse_sse_single_frame() {
        let input = "event: token\ndata: {\"delta\":\"Hi\"}\n\n";
        let parsed = parse_sse(input);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].0, "token");
        assert_eq!(parsed[0].1, serde_json::json!({ "delta": "Hi" }));
    }

    #[test]
    fn parse_sse_multiple_frames_in_order() {
        let input = "\
event: token\ndata: {\"delta\":\"Hello\"}\n\n\
event: tool_call\ndata: {\"id\":\"call_1\",\"name\":\"clock\",\"arguments\":\"{}\"}\n\n\
event: done\ndata: {\"reason\":\"tool_calls\"}\n\n";
        let parsed = parse_sse(input);
        assert_eq!(parsed.len(), 3);
        assert_eq!(parsed[0].0, "token");
        assert_eq!(parsed[1].0, "tool_call");
        assert_eq!(parsed[2].0, "done");
        assert_eq!(
            parsed[1].1,
            serde_json::json!({
                "id": "call_1",
                "name": "clock",
                "arguments": "{}",
            })
        );
    }

    #[test]
    fn parse_sse_strips_leading_space_after_data_colon() {
        let input = "event: token\ndata: {\"delta\":\"x\"}\n\n";
        let parsed = parse_sse(input);
        assert_eq!(parsed[0].1, serde_json::json!({ "delta": "x" }));
    }

    #[test]
    fn parse_sse_non_json_data_falls_back_to_string() {
        let input = "event: comment\ndata: hello world\n\n";
        let parsed = parse_sse(input);
        assert_eq!(parsed.len(), 1);
        assert_eq!(
            parsed[0].1,
            serde_json::Value::String("hello world".to_string())
        );
    }

    #[test]
    fn parse_sse_ignores_empty_frames_and_unknown_fields() {
        let input = "\n\n: this is a comment\n\nevent: x\ndata: 1\n\n\n\n";
        let parsed = parse_sse(input);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].0, "x");
    }

    #[test]
    fn parse_sse_skips_frame_without_event_name() {
        // SSE technically allows data-only frames (default event "message")
        // but our protocol requires an `event:` line — skip otherwise.
        let input = "data: {\"x\":1}\n\nevent: token\ndata: {\"delta\":\"a\"}\n\n";
        let parsed = parse_sse(input);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].0, "token");
    }

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

    #[test]
    fn encode_sse_event_roundtrips_through_parser() {
        let original = serde_json::json!({
            "id": "call_1",
            "name": "gizza-ai/clock",
            "arguments": "{\"tz\":\"UTC\"}",
        });
        let encoded = encode_sse_event("tool_call", &original);
        let parsed = parse_sse(&encoded);
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].0, "tool_call");
        assert_eq!(parsed[0].1, original);
    }

    #[test]
    fn build_tools_filters_on_skill_role_and_tool_presence() {
        let skill = BlockInfo::new("gizza-ai/clock", "0.1.0", "handler@v1", "clock skill")
            .role(SkillRole::Skill)
            .tool(SkillTool {
                description: "Returns the current time".to_string(),
                parameters: serde_json::json!({ "type": "object", "properties": {} }),
            });
        let non_skill =
            BlockInfo::new("gizza-ai/ui", "0.1.0", "handler@v1", "ui block, not a skill");
        let skill_without_tool =
            BlockInfo::new("gizza-ai/x", "0.1.0", "handler@v1", "declared skill but no tool")
                .role(SkillRole::Skill);

        let tools = build_tools(&[skill, non_skill, skill_without_tool]);
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["type"], "function");
        assert_eq!(tools[0]["function"]["name"], "gizza-ai/clock");
        assert_eq!(
            tools[0]["function"]["description"],
            "Returns the current time"
        );
        assert_eq!(
            tools[0]["function"]["parameters"],
            serde_json::json!({ "type": "object", "properties": {} })
        );
    }

    #[test]
    fn build_tools_returns_empty_when_no_skills() {
        let blocks = vec![BlockInfo::new("x/y", "0.1.0", "h@v1", "")];
        assert!(build_tools(&blocks).is_empty());
    }

    #[test]
    fn tool_call_from_json_handles_defaults() {
        let v = serde_json::json!({ "name": "clock" });
        let tc = ToolCall::from_json(&v).expect("valid");
        assert_eq!(tc.name, "clock");
        assert_eq!(tc.id, "");
        assert_eq!(tc.arguments, "{}");
    }

    #[test]
    fn tool_call_from_json_preserves_raw_arguments_string() {
        let v = serde_json::json!({
            "id": "call_7",
            "name": "gizza-ai/clock",
            "arguments": "{\"tz\":\"UTC\"}",
        });
        let tc = ToolCall::from_json(&v).expect("valid");
        assert_eq!(tc.id, "call_7");
        assert_eq!(tc.name, "gizza-ai/clock");
        assert_eq!(tc.arguments, "{\"tz\":\"UTC\"}");
    }

    #[test]
    fn short_name_returns_trailing_segment() {
        assert_eq!(short_name("gizza-ai/clock"), "clock");
        assert_eq!(short_name("suppers-ai/local-llm"), "local-llm");
        assert_eq!(short_name("plain"), "plain");
    }
}
