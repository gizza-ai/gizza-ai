//! gizza-ai/agent — tool-using chat agent block.
//!
//! Protocol (route POST /b/agent/chat):
//!   Request body: { "user_message": "...", "messages": [ ...prior... ] }
//!   Response:     text/event-stream with events:
//!                 event: token        data: { "delta": "..." }
//!                 event: tool_call    data: { "id": "...", "name": "gizza-ai/clock", "arguments": "{}" }
//!                 event: tool_result  data: { "id": "...", "result": "...", "for_ui"?: { ... } }
//!                                            // `for_ui` is an opt-in render hint emitted when
//!                                            // a skill returns a `{_for_llm, _for_ui}` envelope
//!                                            // (e.g. `{data_url, mime, filename}` for images).
//!                                            // The LLM only sees `result`; bytes never enter history.
//!                 event: done         data: { "reason": "stop" | "max_rounds_exceeded" | "error" }
//!
//! Algorithm:
//! 1. Enumerate every registered block whose `role == Some(SkillRole::Skill)` and build
//!    a `Vec<ToolDefinition>` from each `BlockInfo::tool`.
//! 2. Convert the incoming OpenAI-format messages to `ChatMessage` values.
//! 3. Call `ctx.call_block("wafer-run/llm", ...)` with `ServiceOp::LLM_CHAT`
//!    and a `ChatRequest` body.  Consume the streaming `ChatChunk` frames.
//! 4. Forward `ChunkDelta::Text` chunks as `token` SSE events. Accumulate
//!    `ToolCallStart / ToolCallArguments / ToolCallComplete` chunks and emit
//!    `tool_call` events once each call is finalised.
//! 5. Dispatch finalised tool calls via `ctx.call_block`, emit `tool_result`,
//!    append tool-call + tool-result messages to history, and loop.
//! 6. Loop up to `MAX_ROUNDS = 5`. When the LLM returns no tool calls, emit
//!    `event: done` with `reason: "stop"` and terminate.

use std::collections::HashMap;

use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use futures::{pin_mut, StreamExt};
use serde::Deserialize;
use wafer_block::{
    block::Block,
    common::ServiceOp,
    context::Context,
    core_types::{ErrorCode, LifecycleEvent, Message, MetaEntry, WaferError},
    meta::{META_REQ_ACTION, META_RESP_CONTENT_TYPE, META_RESP_STATUS},
    streams::{
        input::InputStream,
        output::{BufferedResponse, OutputStream},
    },
    types::{BlockInfo, SkillRole},
    Attachment,
};
use wafer_core::clients::llm::{
    ChatChunk, ChatContent, ChatMessage, ChatParams, ChatRequest, ChatRole, ChunkDelta,
    FinishReason, ToolCall as LlmToolCall, ToolDefinition,
};

/// Maximum agent-loop rounds before giving up.
const MAX_ROUNDS: u32 = 5;

/// Per-frame timeout for the LLM body stream.
///
/// Wraps each `stream.next()` — resets on every frame. That differs
/// semantically from the old `sw-llm-bridge.js` `setTimeout(300_000)` which
/// capped the entire request. In practice both catch the same failure mode
/// (GPU stall produces zero frames); a healthy stream emits tokens every
/// ~0–1s and never approaches the budget.

/// The agent block's own chat endpoint.
const AGENT_CHAT_PATH: &str = "/b/agent/chat";

/// The WebLLM model id used for MVP. Picked for small size + tool-call support.
/// Plan C makes this user-pickable.
const MVP_MODEL_ID: &str = "Qwen2.5-1.5B-Instruct-q4f32_1-MLC";

pub struct AgentBlock;

#[derive(Debug, Deserialize)]
struct AgentRequest {
    #[serde(default)]
    user_message: String,
    #[serde(default)]
    messages: Vec<serde_json::Value>,
    /// Optional WebLLM model id. If absent or empty, falls back to
    /// `MVP_MODEL_ID`. Allows the UI's model-picker to drive which model the
    /// LLM service loads/uses without per-request server-side config.
    #[serde(default)]
    model_id: Option<String>,
    #[serde(default)]
    uploads: Vec<UploadEntry>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct UploadEntry {
    id: String,
    mime: String,
    #[serde(default)]
    filename: Option<String>,
    bytes_base64: String,
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
            "Drives a tool-use loop against wafer-run/llm, dispatching \
             skill blocks as OpenAI-format tools.",
        )
    }

    async fn handle(&self, ctx: &dyn Context, msg: Message, input: InputStream) -> OutputStream {
        let action = msg.action();
        let path = msg.path();

        if action == "create" && path == AGENT_CHAT_PATH {
            return handle_chat(ctx, input).await;
        }

        error_response(404, "not_found", "unknown agent endpoint")
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
// /b/agent/chat handler
// ---------------------------------------------------------------------------

async fn handle_chat(ctx: &dyn Context, input: InputStream) -> OutputStream {
    // 1. Parse request body.
    let body_bytes = input.collect_to_bytes().await;
    let req: AgentRequest = if body_bytes.is_empty() {
        AgentRequest {
            user_message: String::new(),
            messages: Vec::new(),
            model_id: None,
            uploads: Vec::new(),
        }
    } else {
        match serde_json::from_slice(&body_bytes) {
            Ok(v) => v,
            Err(e) => {
                return error_response(400, "bad_request", &format!("invalid JSON body: {e}"));
            }
        }
    };

    if req.user_message.trim().is_empty() && req.messages.is_empty() {
        return error_response(400, "bad_request", "user_message or messages required");
    }

    // 2a. Decode uploads. Reject the request on any invalid entry.
    let staged_uploads = match decode_uploads(&req.uploads) {
        Ok(v) => v,
        Err(e) => return error_response(400, "bad_request", &e),
    };

    // 2b. Build ToolDefinitions from registered skill blocks.
    //     WebLLM 0.2.74 rejects ChatCompletionRequest.tools for any model
    //     not on its short function-calling allowlist (UnsupportedModelIdError).
    //     The 3 prefixes below cover all 5 currently-allowed variants:
    //       Hermes-2-Pro-Llama-3-8B-{q4f16_1, q4f32_1}-MLC
    //       Hermes-2-Pro-Mistral-7B-q4f16_1-MLC
    //       Hermes-3-Llama-3.1-8B-{q4f32_1, q4f16_1}-MLC
    //     Keep in sync with site/model-picker.js's TOOL_SUPPORT_HINTS.
    let model_id_for_tools = req
        .model_id
        .as_deref()
        .unwrap_or(MVP_MODEL_ID);
    let model_supports_tools = [
        "Hermes-2-Pro-Llama-3-8B",
        "Hermes-2-Pro-Mistral-7B",
        "Hermes-3-Llama-3.1-8B",
    ]
    .iter()
    .any(|hint| model_id_for_tools.contains(hint));
    let tools = if model_supports_tools {
        build_tools(&ctx.registered_blocks())
    } else {
        Vec::new()
    };

    // 3. Compose conversation history: prior messages + new user turn.
    let mut history = req.messages;
    if !req.user_message.is_empty() {
        history.push(serde_json::json!({
            "role": "user",
            "content": req.user_message,
        }));
    }

    // 4. Resolve model id — request override or MVP default.
    let model_id = req
        .model_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(MVP_MODEL_ID)
        .to_string();

    // 5. Run the tool-use loop, buffering the SSE output.
    let sse_body = run_agent_loop(ctx, history, tools, &model_id, staged_uploads).await;

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
            key: format!(
                "{}cache-control",
                wafer_block::meta::META_RESP_HEADER_PREFIX
            ),
            value: "no-cache".to_string(),
        },
    ];
    OutputStream::respond_with_meta(sse_body.into_bytes(), meta)
}

// ---------------------------------------------------------------------------
// Agent loop
// ---------------------------------------------------------------------------

/// Internal accumulator for a streaming tool call.
struct ToolCallAccumulator {
    name: String,
    arguments: String,
}

/// Finalised tool call ready to dispatch.
#[derive(Debug, Clone)]
struct PendingToolCall {
    id: String,
    name: String,
    /// Accumulated raw JSON-string argument payload.
    arguments: String,
}

/// Run the full tool-use loop. Returns the SSE-encoded response body (UTF-8).
async fn run_agent_loop(
    ctx: &dyn Context,
    mut history: Vec<serde_json::Value>,
    tools: Vec<ToolDefinition>,
    model_id: &str,
    staged_uploads: Vec<(String, Attachment, String)>,
) -> String {
    let mut out = String::new();
    let mut attachments: HashMap<String, Attachment> = HashMap::new();
    // Pre-seed with uploads so dispatch_tool can resolve {ref:"upload_N"}.
    for (id, att, _display) in &staged_uploads {
        attachments.insert(id.clone(), att.clone());
    }
    // Inject synthetic upload history BEFORE the user message.
    // history at entry contains [...prior messages, current user message] —
    // pop the user, splice the prefix, push the user back.
    if !staged_uploads.is_empty() {
        let user_msg = history.pop();
        history.extend(build_upload_history_prefix(&staged_uploads));
        if let Some(u) = user_msg {
            history.push(u);
        }
    }

    for round in 1..=MAX_ROUNDS {
        // Convert OpenAI-format history to ChatMessage vec.
        let mut chat_messages: Vec<ChatMessage> = Vec::with_capacity(history.len());
        for v in &history {
            match openai_json_to_chat_message(v) {
                Ok(m) => chat_messages.push(m),
                Err(e) => {
                    out.push_str(&encode_sse_event(
                        "done",
                        &serde_json::json!({
                            "reason": "error",
                            "error": format!("invalid history message: {e}"),
                        }),
                    ));
                    return out;
                }
            }
        }

        // Build wire ChatRequest. Service types were dropped in the binary-
        // transport migration; the runtime now expects MessagePack-encoded
        // wire types via the codec, and `wafer_core::clients::llm::chat_stream`
        // handles both ends of that for us.
        let req = ChatRequest {
            backend_id: "webllm".to_string(),
            model: model_id.to_string(),
            messages: chat_messages,
            params: ChatParams::default(),
            tools: tools.clone(),
            extra: serde_json::Value::Null,
        };

        // Stream chat completion frames via the typed client.
        let mut stream = match wafer_core::clients::llm::chat_stream(ctx, &req).await {
            Ok(s) => s,
            Err(e) => {
                out.push_str(&encode_sse_event(
                    "done",
                    &serde_json::json!({
                        "reason": "error",
                        "error": format!("chat_stream start failed: {e}"),
                    }),
                ));
                return out;
            }
        };

        // Accumulators for in-flight tool calls (keyed by call id).
        let mut accumulator: HashMap<String, ToolCallAccumulator> = HashMap::new();
        // Finalised tool calls in order of completion.
        let mut tool_calls: Vec<PendingToolCall> = Vec::new();
        let mut finish_reason: Option<FinishReason> = None;
        let mut round_error: Option<String> = None;

        'chunks: loop {
            let item: Result<ChatChunk, _> = match stream.next().await {
                Some(item) => item,
                None => break 'chunks,
            };

            match item {
                Err(e) => {
                    round_error = Some(format!("{e}"));
                    break 'chunks;
                }
                Ok(chunk) => {
                    // Capture finish reason if present.
                    if let Some(reason) = chunk.finish_reason {
                        finish_reason = Some(reason);
                    }

                    match chunk.delta {
                        ChunkDelta::Text(t) if !t.is_empty() => {
                            out.push_str(&encode_sse_event(
                                "token",
                                &serde_json::json!({ "delta": t }),
                            ));
                        }
                        ChunkDelta::ToolCallStart { id, name } => {
                            accumulator.insert(
                                id,
                                ToolCallAccumulator {
                                    name,
                                    arguments: String::new(),
                                },
                            );
                        }
                        ChunkDelta::ToolCallArguments {
                            id,
                            arguments_delta,
                        } => {
                            if let Some(acc) = accumulator.get_mut(&id) {
                                acc.arguments.push_str(&arguments_delta);
                            }
                        }
                        ChunkDelta::ToolCallComplete { id } => {
                            if let Some(acc) = accumulator.remove(&id) {
                                let tc = PendingToolCall {
                                    id: id.clone(),
                                    name: acc.name.clone(),
                                    arguments: acc.arguments.clone(),
                                };
                                // Emit tool_call event to UI immediately.
                                out.push_str(&encode_sse_event(
                                    "tool_call",
                                    &serde_json::json!({
                                        "id": tc.id,
                                        "name": tc.name,
                                        "arguments": tc.arguments,
                                    }),
                                ));
                                tool_calls.push(tc);
                            }
                        }
                        // Text(empty), Empty, and other non-exhaustive variants.
                        _ => {}
                    }

                    // Once we have a finish reason, stop reading this round.
                    if finish_reason.is_some() {
                        break 'chunks;
                    }
                }
            }
        }

        // Handle round-level error.
        if let Some(err) = round_error {
            out.push_str(&encode_sse_event(
                "done",
                &serde_json::json!({
                    "reason": "error",
                    "error": err,
                }),
            ));
            return out;
        }

        // No tool calls: emit done and terminate.
        if tool_calls.is_empty() {
            let reason = match finish_reason {
                Some(FinishReason::Stop) | None => "stop",
                Some(FinishReason::Length) => "length",
                Some(FinishReason::ToolCall) => "stop",
                Some(FinishReason::ContentFilter) => "content_filter",
                Some(FinishReason::Error) => "error",
                // non_exhaustive catch-all
                _ => "stop",
            };
            out.push_str(&encode_sse_event(
                "done",
                &serde_json::json!({ "reason": reason }),
            ));
            return out;
        }

        // Dispatch each tool call and accumulate results.
        let mut tc_records: Vec<(PendingToolCall, String)> = Vec::with_capacity(tool_calls.len());
        for tc in &tool_calls {
            let ToolOutcome { for_llm, for_ui } = dispatch_tool(ctx, tc, &attachments).await;
            let mut for_llm = for_llm;

            // Stash attachment if for_ui carries a decodable data: URL, and
            // append a hint to for_llm so the LLM can chain.
            if let Some(att) = for_ui.as_ref().and_then(extract_attachment) {
                let hint = format_ref_hint(&tc.id, &att);
                for_llm = format!("{for_llm}\n\n{hint}");
                attachments.insert(tc.id.clone(), att);
            }

            let mut payload = serde_json::json!({
                "id": tc.id,
                "result": for_llm.clone(),
            });
            if let Some(ref for_ui_val) = for_ui {
                payload["for_ui"] = for_ui_val.clone();
            }
            out.push_str(&encode_sse_event("tool_result", &payload));
            tc_records.push((tc.clone(), for_llm));
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

    // Defensive fall-through — should be unreachable.
    out.push_str(&encode_sse_event(
        "done",
        &serde_json::json!({ "reason": "max_rounds_exceeded" }),
    ));
    out
}

// ---------------------------------------------------------------------------
// Skill-response bifurcation
// ---------------------------------------------------------------------------

/// Output of a skill dispatch, split into an LLM-safe summary and an optional
/// UI render hint. The summary always goes into the tool-role history entry
/// the LLM sees on the next round; the render hint, if present, rides the
/// SSE `tool_result` event to the UI but is *not* echoed back to the LLM.
///
/// This is the bifurcation point that lets a skill return megabytes of base64
/// image data without blowing the LLM's context window.
#[derive(Debug, Clone)]
pub(crate) struct ToolOutcome {
    pub for_llm: String,
    pub for_ui: Option<serde_json::Value>,
}

/// Parse a skill's response body. A response is treated as an envelope iff
/// it parses as JSON, the top-level value is an object, and it has a
/// `_for_llm` field of type string. Otherwise the whole body becomes
/// `for_llm` and `for_ui` is None — preserving legacy plain-text behavior.
pub(crate) fn parse_skill_response(body: &str) -> ToolOutcome {
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

/// Decode an [`UploadEntry`] vec into staged-upload tuples
/// `(id, Attachment, display_name)`. Returns an error string suitable for the
/// 4xx error_response on the first invalid entry. v1 caps each upload at
/// 10 MiB; only `image/*` and `video/*` MIMEs are accepted; ids must start
/// with `"upload_"`.
pub(crate) fn decode_uploads(
    entries: &[UploadEntry],
) -> Result<Vec<(String, Attachment, String)>, String> {
    const MAX_UPLOAD_BYTES: usize = 10 * 1024 * 1024;
    let mut staged = Vec::with_capacity(entries.len());
    for u in entries {
        if !u.id.starts_with("upload_") {
            return Err(format!(
                "invalid upload id {:?}: must start with \"upload_\"",
                u.id
            ));
        }
        if !(u.mime.starts_with("image/") || u.mime.starts_with("video/")) {
            return Err(format!(
                "upload {:?}: only image/* and video/* are accepted, got {}",
                u.id, u.mime
            ));
        }
        let bytes = B64
            .decode(&u.bytes_base64)
            .map_err(|e| format!("upload {:?}: base64 decode failed: {e}", u.id))?;
        if bytes.len() > MAX_UPLOAD_BYTES {
            return Err(format!(
                "upload {:?}: {} bytes exceeds 10 MiB cap",
                u.id,
                bytes.len()
            ));
        }
        let display_name = u.filename.clone().unwrap_or_else(|| {
            if u.mime.starts_with("image/") {
                "image".into()
            } else if u.mime.starts_with("video/") {
                "video".into()
            } else {
                "file".into()
            }
        });
        let att = Attachment {
            mime: u.mime.clone(),
            bytes,
            filename: u.filename.clone(),
        };
        staged.push((u.id.clone(), att, display_name));
    }
    Ok(staged)
}

/// Decode the SP4 envelope's `_for_ui` value into an `Attachment`, if it is
/// a `{data_url, mime, filename}` shape with a `data:` URL. Returns `None`
/// for any other shape — the caller treats that as "skill produced a
/// non-bytes for_ui; don't stash."
pub(crate) fn extract_attachment(for_ui: &serde_json::Value) -> Option<Attachment> {
    let data_url = for_ui.get("data_url")?.as_str()?;
    let comma = data_url.find(',')?;
    let header = &data_url[..comma];
    let b64 = &data_url[comma + 1..];
    if !header.starts_with("data:") || !header.ends_with(";base64") {
        return None;
    }
    let mime = for_ui
        .get("mime")
        .and_then(|v| v.as_str())
        .map(str::to_string)
        .unwrap_or_else(|| {
            // Recover from the data: URL header if mime not provided separately.
            header[5..header.len() - 7].to_string()
        });
    let bytes = B64.decode(b64).ok()?;
    let filename = for_ui
        .get("filename")
        .and_then(|v| v.as_str())
        .map(str::to_string);
    Some(Attachment {
        mime,
        bytes,
        filename,
    })
}

/// Format a hint string the agent appends to a tool result's `_for_llm`
/// summary, telling the LLM the result is stashed and how to chain another
/// image op against it via `{ref: "<id>"}`.
pub(crate) fn format_ref_hint(id: &str, att: &Attachment) -> String {
    let filename_clause = match &att.filename {
        Some(f) => format!(", \"{f}\""),
        None => String::new(),
    };
    format!(
        "Saved as ref \"{id}\" ({mime}, {size} bytes{fn_clause}). \
         To chain, pass {{\"ref\": \"{id}\"}} instead of \"url\" in the next image tool call.",
        id = id,
        mime = att.mime,
        size = att.bytes.len(),
        fn_clause = filename_clause,
    )
}

/// Build the synthetic assistant + tool history-prefix entries the agent
/// injects ahead of the user message when the request carries uploads.
///
/// Returns two `serde_json::Value` entries per upload:
///   1. `{role: "assistant", tool_calls: [{id, function: {name: "user_upload", arguments: "{}"}}]}`
///   2. `{role: "tool", tool_call_id: id, content: "ref upload_1 saved (...). Pass {\"ref\":\"upload_1\"} to a skill."}`
///
/// The phrasing mirrors `format_ref_hint` so the LLM treats `upload_N` ids
/// the same as chained-call ids (`call_N`) — same `{ref}` mechanism.
pub(crate) fn build_upload_history_prefix(
    staged: &[(String, Attachment, String)],
) -> Vec<serde_json::Value> {
    let mut out = Vec::with_capacity(staged.len() * 2);
    for (id, att, display) in staged {
        out.push(serde_json::json!({
            "role": "assistant",
            "tool_calls": [{
                "id": id,
                "type": "function",
                "function": { "name": "user_upload", "arguments": "{}" }
            }]
        }));
        let content = format!(
            "ref {id} saved ({mime}, {size} bytes, {display:?}). \
             Pass {{\"ref\": \"{id}\"}} to a skill.",
            id = id,
            mime = att.mime,
            size = att.bytes.len(),
            display = display,
        );
        out.push(serde_json::json!({
            "role": "tool",
            "tool_call_id": id,
            "content": content,
        }));
    }
    out
}

// ---------------------------------------------------------------------------
// Tool dispatch
// ---------------------------------------------------------------------------

/// Dispatch a single tool call to the corresponding block.
///
/// Tool `name` is treated as either a fully-qualified block name (`org/block`)
/// or a short name that we default to `gizza-ai/{name}`. Returns a
/// [`ToolOutcome`] splitting the response into an LLM-safe summary (`for_llm`)
/// and an optional UI render hint (`for_ui`). Error branches set `for_ui` to
/// `None`; only the success path calls [`parse_skill_response`].
///
/// If the parsed args contain a `{"ref": "..."}` key, the attachment is looked
/// up from `attachments` and forwarded via
/// [`Context::call_block_buffered_with_attachments`]. If the ref is unknown, an
/// error `ToolOutcome` is returned immediately without dispatching.
async fn dispatch_tool(
    ctx: &dyn Context,
    tc: &PendingToolCall,
    attachments: &HashMap<String, Attachment>,
) -> ToolOutcome {
    let block_name = if tc.name.contains('/') {
        tc.name.clone()
    } else {
        format!("gizza-ai/{}", tc.name)
    };

    let mut msg = Message::new("http");
    msg.set_meta(META_REQ_ACTION, "create");
    msg.set_meta(
        wafer_block::meta::META_REQ_RESOURCE,
        format!("/b/{}", short_name(&block_name)),
    );

    // Inspect args for {"ref": "..."} and prepare an outgoing attachment map.
    let mut outgoing: std::collections::BTreeMap<String, Attachment> =
        std::collections::BTreeMap::new();
    if let Ok(args_value) = serde_json::from_str::<serde_json::Value>(&tc.arguments) {
        if let Some(ref_id) = args_value.get("ref").and_then(|v| v.as_str()) {
            match attachments.get(ref_id) {
                Some(att) => {
                    outgoing.insert(ref_id.to_string(), att.clone());
                }
                None => {
                    return ToolOutcome {
                        for_llm: format!(
                            r#"{{"error":"unknown_ref","message":"no attachment for ref {ref_id:?}"}}"#
                        ),
                        for_ui: None,
                    };
                }
            }
        }
    }

    let args_bytes = tc.arguments.as_bytes().to_vec();
    match ctx
        .call_block_buffered_with_attachments(&block_name, msg, &args_bytes, outgoing)
        .await
    {
        Ok(BufferedResponse { body, .. }) => {
            let result_text = String::from_utf8_lossy(&body).to_string();
            parse_skill_response(&result_text)
        }
        Err(e) => ToolOutcome {
            for_llm: format!("{{\"error\": \"tool_failed\", \"message\": \"{}\"}}", e),
            for_ui: None,
        },
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
/// and `tool.is_some()`, build a `Vec<ToolDefinition>` for the LLM request.
fn build_tools(blocks: &[BlockInfo]) -> Vec<ToolDefinition> {
    blocks
        .iter()
        .filter_map(|info| match (&info.role, &info.tool) {
            (Some(SkillRole::Skill), Some(tool)) => Some(ToolDefinition {
                name: info.name.clone(),
                description: tool.description.clone(),
                parameters: tool.parameters.clone(),
            }),
            _ => None,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Message conversion
// ---------------------------------------------------------------------------

/// Convert an OpenAI-format message JSON value to a `ChatMessage`.
///
/// Handles all four roles (`system`, `user`, `assistant`, `tool`), text
/// `content`, `tool_call_id`, and the `tool_calls` array.
fn openai_json_to_chat_message(v: &serde_json::Value) -> Result<ChatMessage, String> {
    let role_str = v
        .get("role")
        .and_then(|r| r.as_str())
        .ok_or("missing role")?;

    let role = match role_str {
        "system" => ChatRole::System,
        "user" => ChatRole::User,
        "assistant" => ChatRole::Assistant,
        "tool" => ChatRole::Tool,
        other => return Err(format!("unknown role: {other}")),
    };

    let content = ChatContent::Text(
        v.get("content")
            .and_then(|c| c.as_str())
            .unwrap_or("")
            .to_string(),
    );

    let tool_call_id = v
        .get("tool_call_id")
        .and_then(|x| x.as_str())
        .map(|s| s.to_string());

    if role == ChatRole::Tool {
        let id = tool_call_id.unwrap_or_default();
        let text = match &content {
            ChatContent::Text(t) => t.clone(),
            _ => String::new(),
        };
        return Ok(ChatMessage {
            role: ChatRole::Tool,
            content: ChatContent::Text(text),
            tool_call_id: Some(id),
            tool_calls: Vec::new(),
        });
    }

    // Parse tool_calls array (OpenAI format).
    let tool_calls: Vec<LlmToolCall> = v
        .get("tool_calls")
        .and_then(|arr| arr.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|entry| {
                    let id = entry.get("id")?.as_str()?.to_string();
                    let func = entry.get("function")?;
                    let name = func.get("name")?.as_str()?.to_string();
                    // arguments is a JSON string in OpenAI format; parse it.
                    let arguments = func
                        .get("arguments")
                        .and_then(|a| a.as_str())
                        .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
                        .unwrap_or(serde_json::Value::Object(serde_json::Map::new()));
                    Some(LlmToolCall {
                        id,
                        name,
                        arguments,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(ChatMessage {
        role,
        content,
        tool_call_id: None,
        tool_calls,
    })
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

    // ---------------------------------------------------------------------------
    // encode_sse_event tests
    // ---------------------------------------------------------------------------

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

    // ---------------------------------------------------------------------------
    // build_tools tests
    // ---------------------------------------------------------------------------

    #[test]
    fn build_tools_filters_on_skill_role_and_tool_presence() {
        let skill = BlockInfo::new("gizza-ai/clock", "0.1.0", "handler@v1", "clock skill")
            .role(SkillRole::Skill)
            .tool(SkillTool {
                description: "Returns the current time".to_string(),
                parameters: serde_json::json!({ "type": "object", "properties": {} }),
            });
        let non_skill = BlockInfo::new(
            "gizza-ai/ui",
            "0.1.0",
            "handler@v1",
            "ui block, not a skill",
        );
        let skill_without_tool = BlockInfo::new(
            "gizza-ai/x",
            "0.1.0",
            "handler@v1",
            "declared skill but no tool",
        )
        .role(SkillRole::Skill);

        let tools = build_tools(&[skill, non_skill, skill_without_tool]);
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "gizza-ai/clock");
        assert_eq!(tools[0].description, "Returns the current time");
        assert_eq!(
            tools[0].parameters,
            serde_json::json!({ "type": "object", "properties": {} })
        );
    }

    #[test]
    fn build_tools_returns_empty_when_no_skills() {
        let blocks = vec![BlockInfo::new("x/y", "0.1.0", "h@v1", "")];
        assert!(build_tools(&blocks).is_empty());
    }

    // ---------------------------------------------------------------------------
    // openai_json_to_chat_message tests
    // ---------------------------------------------------------------------------

    #[test]
    fn openai_json_to_chat_message_system() {
        let v = serde_json::json!({ "role": "system", "content": "You are helpful." });
        let msg = openai_json_to_chat_message(&v).expect("valid");
        assert_eq!(msg.role, ChatRole::System);
        assert_eq!(
            msg.content,
            ChatContent::Text("You are helpful.".to_string())
        );
    }

    #[test]
    fn openai_json_to_chat_message_user() {
        let v = serde_json::json!({ "role": "user", "content": "Hello!" });
        let msg = openai_json_to_chat_message(&v).expect("valid");
        assert_eq!(msg.role, ChatRole::User);
        assert_eq!(msg.content, ChatContent::Text("Hello!".to_string()));
    }

    #[test]
    fn openai_json_to_chat_message_assistant() {
        let v = serde_json::json!({ "role": "assistant", "content": "Hi there." });
        let msg = openai_json_to_chat_message(&v).expect("valid");
        assert_eq!(msg.role, ChatRole::Assistant);
        assert_eq!(msg.content, ChatContent::Text("Hi there.".to_string()));
    }

    #[test]
    fn openai_json_to_chat_message_tool() {
        let v = serde_json::json!({
            "role": "tool",
            "tool_call_id": "call_1",
            "content": "2026-04-20T12:00:00Z",
        });
        let msg = openai_json_to_chat_message(&v).expect("valid");
        assert_eq!(msg.role, ChatRole::Tool);
        assert_eq!(msg.tool_call_id, Some("call_1".to_string()));
        assert_eq!(
            msg.content,
            ChatContent::Text("2026-04-20T12:00:00Z".to_string())
        );
    }

    #[test]
    fn openai_json_to_chat_message_unknown_role_errors() {
        let v = serde_json::json!({ "role": "unknown", "content": "" });
        assert!(openai_json_to_chat_message(&v).is_err());
    }

    #[test]
    fn openai_json_to_chat_message_assistant_with_tool_calls() {
        let v = serde_json::json!({
            "role": "assistant",
            "content": "",
            "tool_calls": [
                {
                    "id": "call_42",
                    "type": "function",
                    "function": {
                        "name": "gizza-ai/clock",
                        "arguments": "{\"tz\":\"UTC\"}",
                    }
                }
            ]
        });
        let msg = openai_json_to_chat_message(&v).expect("valid");
        assert_eq!(msg.role, ChatRole::Assistant);
        assert_eq!(msg.tool_calls.len(), 1);
        assert_eq!(msg.tool_calls[0].id, "call_42");
        assert_eq!(msg.tool_calls[0].name, "gizza-ai/clock");
        assert_eq!(
            msg.tool_calls[0].arguments,
            serde_json::json!({ "tz": "UTC" })
        );
    }

    #[test]
    fn openai_json_to_chat_message_tool_calls_invalid_args_defaults_to_empty_object() {
        let v = serde_json::json!({
            "role": "assistant",
            "content": "",
            "tool_calls": [
                {
                    "id": "call_1",
                    "type": "function",
                    "function": {
                        "name": "gizza-ai/clock",
                        "arguments": "not-valid-json",
                    }
                }
            ]
        });
        let msg = openai_json_to_chat_message(&v).expect("valid");
        assert_eq!(msg.tool_calls[0].arguments, serde_json::json!({}));
    }

    // ---------------------------------------------------------------------------
    // short_name tests
    // ---------------------------------------------------------------------------

    #[test]
    fn short_name_returns_trailing_segment() {
        assert_eq!(short_name("gizza-ai/clock"), "clock");
        assert_eq!(short_name("suppers-ai/local-llm"), "local-llm");
        assert_eq!(short_name("plain"), "plain");
    }

    // ---------------------------------------------------------------------------
    // parse_skill_response tests
    // ---------------------------------------------------------------------------

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
        // _for_llm present but _for_ui is a string — drop _for_ui silently.
        let body = r#"{"_for_llm":"ok","_for_ui":"not-an-object"}"#;
        let outcome = parse_skill_response(body);
        assert_eq!(outcome.for_llm, "ok");
        assert!(
            outcome.for_ui.is_none(),
            "non-object _for_ui must be dropped"
        );
    }

    // ---------------------------------------------------------------------------
    // SSE payload construction tests
    // ---------------------------------------------------------------------------

    #[test]
    fn dispatch_tool_outcome_with_for_ui_serialises_into_sse_payload() {
        // Smoke test for the round-loop construction: given a ToolOutcome with
        // for_ui set, the JSON we emit on the SSE wire must include the for_ui
        // field. Mirrors the construction in the round loop.
        let outcome = ToolOutcome {
            for_llm: "summary".to_string(),
            for_ui: Some(
                serde_json::json!({"data_url":"data:image/png;base64,AAA","mime":"image/png"}),
            ),
        };
        let mut payload = serde_json::json!({
            "id": "abc",
            "result": outcome.for_llm,
        });
        if let Some(for_ui) = &outcome.for_ui {
            payload["for_ui"] = for_ui.clone();
        }
        assert_eq!(payload["id"], "abc");
        assert_eq!(payload["result"], "summary");
        assert_eq!(payload["for_ui"]["mime"], "image/png");
    }

    #[test]
    fn dispatch_tool_outcome_without_for_ui_omits_field_from_sse_payload() {
        let outcome = ToolOutcome {
            for_llm: "plain".to_string(),
            for_ui: None,
        };
        let mut payload = serde_json::json!({
            "id": "abc",
            "result": outcome.for_llm,
        });
        if let Some(for_ui) = &outcome.for_ui {
            payload["for_ui"] = for_ui.clone();
        }
        assert!(
            payload.get("for_ui").is_none(),
            "for_ui field must be absent when None"
        );
    }

    // ---------------------------------------------------------------------------
    // extract_attachment tests
    // ---------------------------------------------------------------------------

    #[test]
    fn extract_attachment_decodes_data_url() {
        let for_ui = serde_json::json!({
            "data_url": "data:image/png;base64,AAEC",
            "mime": "image/png",
            "filename": "x.png",
        });
        let att = extract_attachment(&for_ui).expect("decoded");
        assert_eq!(att.mime, "image/png");
        assert_eq!(att.bytes, vec![0u8, 1u8, 2u8]);
        assert_eq!(att.filename.as_deref(), Some("x.png"));
    }

    #[test]
    fn extract_attachment_returns_none_when_data_url_missing() {
        let for_ui = serde_json::json!({ "something_else": "xyz" });
        assert!(extract_attachment(&for_ui).is_none());
    }

    #[test]
    fn extract_attachment_returns_none_for_non_data_url() {
        let for_ui = serde_json::json!({ "data_url": "https://example.com/x.png" });
        assert!(extract_attachment(&for_ui).is_none());
    }

    // ---------------------------------------------------------------------------
    // format_ref_hint tests
    // ---------------------------------------------------------------------------

    #[test]
    fn format_ref_hint_contains_id_mime_size_and_filename() {
        let att = Attachment {
            mime: "image/png".into(),
            bytes: vec![0u8; 12345],
            filename: Some("in.png".into()),
        };
        let h = format_ref_hint("call_42", &att);
        assert!(h.contains("call_42"));
        assert!(h.contains("image/png"));
        assert!(h.contains("12345"));
        assert!(h.contains("in.png"));
        assert!(h.contains(r#"{"ref": "call_42"}"#));
    }

    #[test]
    fn format_ref_hint_omits_filename_clause_when_none() {
        let att = Attachment {
            mime: "image/jpeg".into(),
            bytes: vec![0u8; 7],
            filename: None,
        };
        let h = format_ref_hint("call_1", &att);
        // No quoted filename when filename is None.
        // (The test in the spec asserts !h.contains("\"") — that's too aggressive
        //  because the hint legitimately contains quotes around `ref` and `url`.
        //  Use a more focused assertion: the hint should NOT contain a quoted
        //  bareword like ", \"x\")" before the period.)
        assert!(h.contains("call_1"));
        assert!(h.contains("image/jpeg"));
        assert!(h.contains("7 bytes"));
        // Filename clause is `, "filename"` between the byte count and the closing
        // paren. Detect its absence:
        assert!(
            !h.contains("bytes, \""),
            "filename clause must be omitted when None: got {h:?}"
        );
    }

    // ---------------------------------------------------------------------------
    // decode_uploads tests
    // ---------------------------------------------------------------------------

    #[test]
    fn decode_uploads_seeds_attachments_with_upload_ids() {
        let raw = b"\x89PNG\r\n\x1a\n";
        let b64 = B64.encode(raw);
        let entries = vec![UploadEntry {
            id: "upload_1".into(),
            mime: "image/png".into(),
            filename: Some("cat.png".into()),
            bytes_base64: b64,
        }];
        let staged = decode_uploads(&entries).expect("decoded");
        assert_eq!(staged.len(), 1);
        assert_eq!(staged[0].0, "upload_1");
        assert_eq!(staged[0].1.mime, "image/png");
        assert_eq!(staged[0].1.bytes, raw);
        assert_eq!(staged[0].1.filename.as_deref(), Some("cat.png"));
        assert_eq!(staged[0].2, "cat.png");
    }

    #[test]
    fn decode_uploads_rejects_non_upload_id() {
        let entries = vec![UploadEntry {
            id: "call_1".into(),
            mime: "image/png".into(),
            filename: None,
            bytes_base64: B64.encode(b"x"),
        }];
        let r = decode_uploads(&entries);
        assert!(r.is_err());
        let msg = r.unwrap_err();
        assert!(
            msg.contains("upload_"),
            "error must mention id format: {msg}"
        );
    }

    #[test]
    fn decode_uploads_rejects_unsupported_mime() {
        let entries = vec![UploadEntry {
            id: "upload_1".into(),
            mime: "application/pdf".into(),
            filename: None,
            bytes_base64: B64.encode(b"x"),
        }];
        let r = decode_uploads(&entries);
        assert!(r.is_err());
        let msg = r.unwrap_err();
        assert!(msg.contains("image/") || msg.contains("video/"));
    }

    #[test]
    fn decode_uploads_rejects_oversize_decoded_bytes() {
        let big = vec![0u8; 10 * 1024 * 1024 + 1]; // 10 MiB + 1
        let entries = vec![UploadEntry {
            id: "upload_1".into(),
            mime: "image/png".into(),
            filename: None,
            bytes_base64: B64.encode(&big),
        }];
        let r = decode_uploads(&entries);
        assert!(r.is_err());
        let msg = r.unwrap_err();
        assert!(msg.contains("10 MiB") || msg.contains("cap"));
    }

    #[test]
    fn decode_uploads_uses_fallback_display_name_when_filename_missing() {
        let entries = vec![UploadEntry {
            id: "upload_1".into(),
            mime: "image/png".into(),
            filename: None,
            bytes_base64: B64.encode(b"x"),
        }];
        let staged = decode_uploads(&entries).expect("decoded");
        assert_eq!(staged[0].2, "image");

        let entries = vec![UploadEntry {
            id: "upload_2".into(),
            mime: "video/mp4".into(),
            filename: None,
            bytes_base64: B64.encode(b"x"),
        }];
        let staged = decode_uploads(&entries).expect("decoded");
        assert_eq!(staged[0].2, "video");
    }

    #[test]
    fn build_upload_history_prefix_emits_assistant_and_tool_pair_per_upload() {
        let staged = vec![
            (
                "upload_1".to_string(),
                Attachment {
                    mime: "image/png".into(),
                    bytes: vec![0u8; 1228800],
                    filename: Some("cat.png".into()),
                },
                "cat.png".to_string(),
            ),
            (
                "upload_2".to_string(),
                Attachment {
                    mime: "video/mp4".into(),
                    bytes: vec![0u8; 5_000_000],
                    filename: None,
                },
                "video".to_string(),
            ),
        ];
        let prefix = build_upload_history_prefix(&staged);
        // Two messages per upload (assistant tool_call + tool result).
        assert_eq!(prefix.len(), 4);
        // First upload: assistant tool_calls entry.
        assert_eq!(prefix[0]["role"], "assistant");
        assert_eq!(prefix[0]["tool_calls"][0]["id"], "upload_1");
        assert_eq!(
            prefix[0]["tool_calls"][0]["function"]["name"],
            "user_upload"
        );
        // First upload: tool result references the same id.
        assert_eq!(prefix[1]["role"], "tool");
        assert_eq!(prefix[1]["tool_call_id"], "upload_1");
        let content = prefix[1]["content"].as_str().unwrap();
        assert!(content.contains("upload_1"));
        assert!(content.contains("image/png"));
        assert!(content.contains("1228800"));
        assert!(content.contains("cat.png"));
        assert!(content.contains(r#"{"ref": "upload_1"}"#));
        // Second upload: video, no filename — content uses fallback display name.
        assert_eq!(prefix[3]["tool_call_id"], "upload_2");
        let content = prefix[3]["content"].as_str().unwrap();
        assert!(content.contains("upload_2"));
        assert!(content.contains("video/mp4"));
        assert!(content.contains("video")); // display name fallback
    }

    #[test]
    fn build_upload_history_prefix_empty_for_no_uploads() {
        let staged: Vec<(String, Attachment, String)> = Vec::new();
        let prefix = build_upload_history_prefix(&staged);
        assert!(prefix.is_empty());
    }
}
