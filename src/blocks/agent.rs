//! gizza-ai/agent — chat agent block with user-invoked slash-command skills.
//!
//! Routes:
//!   POST /b/agent/chat
//!     Request:  { "user_message": "...", "messages": [...], "model_id"?, "uploads"?, "confirm_yes"? }
//!     Response: text/event-stream:
//!       event: token        data: { "delta": "..." }                    — assistant LLM text
//!       event: tool_result  data: { "id", "input", "result", "for_ui"? } — skill output
//!         `input`  — the extracted params dispatched to the skill (lets
//!                    the UI show users what the LLM understood from their
//!                    slash command in a side-by-side Input/Output view).
//!       event: confirm      data: { "question", "yes": {cmd, params} }  — PR 5 stub
//!       event: done         data: { "reason": "stop" | "error", "error"? }
//!
//!   GET /b/agent/commands
//!     Response: application/json
//!       [ { "cmd": "<short-name>", "description": "..." }, ... ]
//!
//! Slash-command flow (when user_message starts with `/`):
//!   1. Parse leading `/<cmd>` + remainder text.
//!   2. Look up `gizza-ai/<cmd>` in the block registry. Unknown command →
//!      `done` event with an error reason.
//!   3. Read the block's `BlockInfo::tool` (description + JSON-Schema
//!      parameters), then build the params payload:
//!      - Verbatim path: schema is `{"prompt": string, required:["prompt"]}`-
//!        shaped → `{"prompt": "<remainder>"}`.
//!      - LLM extraction path: schema has other/more fields → ask the LLM to
//!        emit a JSON object matching the schema. Stub for PR 5: if the LLM
//!        returns `{"__unsure": "..."}`, emit a `confirm` SSE event and end.
//!   4. Dispatch via `ctx.call_block_buffered_with_attachments`.
//!   5. Parse the skill's response envelope (`_for_llm` / `_for_ui`) and emit
//!      a single `tool_result` SSE event, then `done`.
//!
//! Non-slash flow: plain LLM chat, no `tools[]` advertisement. Tokens are
//! buffered into the SSE response body as `token` events. There is no
//! multi-round agent loop — that surface is replaced by user-invoked slash
//! commands.

use async_trait::async_trait;
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use futures::{pin_mut, StreamExt};
use serde::Deserialize;
use wafer_block::{
    block::Block,
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
    FinishReason, ToolCall as LlmToolCall,
};

/// The agent block's chat endpoint.
const AGENT_CHAT_PATH: &str = "/b/agent/chat";

/// The agent block's command-list endpoint.
const AGENT_COMMANDS_PATH: &str = "/b/agent/commands";

/// Prefix every gizza-built skill block carries. Slash commands map
/// `/<cmd>` → `<SKILL_PREFIX><cmd>` for registry lookup.
const SKILL_PREFIX: &str = "gizza-ai/";

/// Default WebLLM model id. Used when the request omits `model_id`.
const DEFAULT_MODEL_ID: &str = "Qwen2.5-1.5B-Instruct-q4f32_1-MLC";

/// Internal errors for the agent block. Each variant maps to a distinct
/// failure mode in `decode_uploads` or `openai_json_to_chat_message`. The
/// caller uses `Display` to render the user-facing message, then wraps it in
/// an `error_response` or SSE `done` event.
#[derive(Debug, thiserror::Error)]
pub(crate) enum AgentError {
    #[error("invalid upload id {0:?}: must start with \"upload_\"")]
    UploadIdInvalid(String),
    #[error("upload {id:?}: only image/* and video/* are accepted, got {mime}")]
    UploadUnsupportedMime { id: String, mime: String },
    #[error("upload {id:?}: base64 decode failed: {source}")]
    UploadBase64 {
        id: String,
        source: base64::DecodeError,
    },
    #[error("upload {id:?}: {bytes} bytes exceeds 10 MiB cap")]
    UploadTooLarge { id: String, bytes: usize },
    #[error("missing role")]
    MissingRole,
    #[error("unknown role: {0}")]
    UnknownRole(String),
}

pub struct AgentBlock;

#[derive(Debug, Deserialize)]
struct AgentRequest {
    #[serde(default)]
    user_message: String,
    #[serde(default)]
    messages: Vec<serde_json::Value>,
    /// Optional WebLLM model id. If absent or empty, falls back to
    /// `DEFAULT_MODEL_ID`. Lets the UI's model picker drive which model the
    /// LLM service loads/uses without per-request server-side config.
    #[serde(default)]
    model_id: Option<String>,
    #[serde(default)]
    uploads: Vec<UploadEntry>,
    /// PR 5 stub: when set, skip slash parsing + LLM extraction and dispatch
    /// the named skill directly with the pre-extracted params. Lets the
    /// frontend complete a `confirm` SSE round-trip without re-running the
    /// LLM extraction.
    #[serde(default)]
    confirm_yes: Option<ConfirmYes>,
}

#[derive(Debug, Deserialize)]
struct ConfirmYes {
    cmd: String,
    params: serde_json::Value,
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
            "Slash-command chat agent",
        )
        .category(wafer_run::BlockCategory::Feature)
        .description(
            "Drives plain chat against wafer-run/llm and dispatches \
             user-invoked slash commands (/<skill>) to registered skill blocks.",
        )
    }

    async fn handle(&self, ctx: &dyn Context, msg: Message, input: InputStream) -> OutputStream {
        let action = msg.action();
        let path = msg.path();

        if action == "create" && path == AGENT_CHAT_PATH {
            return handle_chat(ctx, input).await;
        }
        if action == "retrieve" && path == AGENT_COMMANDS_PATH {
            return handle_commands(ctx);
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
// GET /b/agent/commands
// ---------------------------------------------------------------------------

/// Enumerate every registered block whose name starts with `gizza-ai/` and
/// has `role == Some(SkillRole::Skill)` with a tool descriptor. Returns the
/// list as a JSON body for the frontend slash-autocomplete dropdown (PR 6).
fn handle_commands(ctx: &dyn Context) -> OutputStream {
    let entries: Vec<serde_json::Value> = list_skill_commands(&ctx.registered_blocks())
        .into_iter()
        .map(|(cmd, desc)| serde_json::json!({ "cmd": cmd, "description": desc }))
        .collect();
    let body = serde_json::to_vec(&entries).expect("serde_json::Value always serializes");
    OutputStream::respond_with_meta(
        body,
        vec![
            MetaEntry {
                key: META_RESP_STATUS.to_string(),
                value: "200".to_string(),
            },
            MetaEntry {
                key: META_RESP_CONTENT_TYPE.to_string(),
                value: "application/json".to_string(),
            },
        ],
    )
}

/// Pure helper: filter the registry down to `(cmd, description)` pairs for
/// every gizza-ai skill block. Sorted by `cmd` for deterministic output.
fn list_skill_commands(blocks: &[BlockInfo]) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = blocks
        .iter()
        .filter_map(|info| match (&info.role, &info.tool) {
            (Some(SkillRole::Skill), Some(tool)) => {
                let cmd = info.name.strip_prefix(SKILL_PREFIX)?;
                Some((cmd.to_string(), tool.description.clone()))
            }
            _ => None,
        })
        .collect();
    out.sort_by(|a, b| a.0.cmp(&b.0));
    out
}

// ---------------------------------------------------------------------------
// POST /b/agent/chat
// ---------------------------------------------------------------------------

async fn handle_chat(ctx: &dyn Context, input: InputStream) -> OutputStream {
    let body_bytes = input.collect_to_bytes().await;
    let req: AgentRequest = if body_bytes.is_empty() {
        AgentRequest {
            user_message: String::new(),
            messages: Vec::new(),
            model_id: None,
            uploads: Vec::new(),
            confirm_yes: None,
        }
    } else {
        match serde_json::from_slice(&body_bytes) {
            Ok(v) => v,
            Err(e) => {
                return error_response(400, "bad_request", &format!("invalid JSON body: {e}"));
            }
        }
    };

    if req.user_message.trim().is_empty() && req.messages.is_empty() && req.confirm_yes.is_none() {
        return error_response(
            400,
            "bad_request",
            "user_message, messages, or confirm_yes required",
        );
    }

    let staged_uploads = match decode_uploads(&req.uploads) {
        Ok(v) => v,
        Err(e) => return error_response(400, "bad_request", &e.to_string()),
    };

    let model_id = req
        .model_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_MODEL_ID)
        .to_string();

    let mut sse = String::new();

    // Confirmation round-trip path: skip slash parsing + extraction and
    // dispatch the named skill with the pre-extracted params. Frontend uses
    // this after the user clicks [Yes] on a confirm bubble (PR 5).
    if let Some(confirm) = req.confirm_yes {
        run_skill_dispatch(
            ctx,
            &confirm.cmd,
            confirm.params,
            "confirmed",
            &staged_uploads,
            &mut sse,
        )
        .await;
        return sse_response(sse);
    }

    // Slash-command path: leading `/<cmd>` in user_message.
    if let Some((cmd, rest)) = parse_slash(&req.user_message) {
        let cmd = cmd.to_string();
        let rest = rest.to_string();
        let blocks = ctx.registered_blocks();
        let Some(tool) = lookup_skill_tool(&blocks, &cmd) else {
            sse.push_str(&encode_sse_event(
                "done",
                &serde_json::json!({
                    "reason": "error",
                    "error": format!("Unknown command: /{cmd}"),
                }),
            ));
            return sse_response(sse);
        };

        let params = match build_skill_params(ctx, &tool.parameters, &rest, &model_id).await {
            ParamExtraction::Verbatim(v) | ParamExtraction::Extracted(v) => v,
            ParamExtraction::Empty => {
                sse.push_str(&encode_sse_event(
                    "done",
                    &serde_json::json!({
                        "reason": "error",
                        "error": format!("/{cmd} needs an argument"),
                    }),
                ));
                return sse_response(sse);
            }
            ParamExtraction::Unsure { question, params } => {
                sse.push_str(&encode_sse_event(
                    "confirm",
                    &serde_json::json!({
                        "question": question,
                        "yes": { "cmd": cmd, "params": params },
                        "no": serde_json::Value::Null,
                    }),
                ));
                sse.push_str(&encode_sse_event(
                    "done",
                    &serde_json::json!({ "reason": "stop" }),
                ));
                return sse_response(sse);
            }
            ParamExtraction::Error(e) => {
                sse.push_str(&encode_sse_event(
                    "done",
                    &serde_json::json!({ "reason": "error", "error": e }),
                ));
                return sse_response(sse);
            }
        };

        run_skill_dispatch(ctx, &cmd, params, "slash", &staged_uploads, &mut sse).await;
        return sse_response(sse);
    }

    // Plain chat path: forward to wafer-run/llm with no tool advertisement.
    let mut history = req.messages;
    if !req.user_message.is_empty() {
        history.push(serde_json::json!({
            "role": "user",
            "content": req.user_message,
        }));
    }
    // Inject upload history prefix BEFORE the current user message so the LLM
    // sees the upload references in chronological order.
    if !staged_uploads.is_empty() {
        let user_msg = history.pop();
        history.extend(build_upload_history_prefix(&staged_uploads));
        if let Some(u) = user_msg {
            history.push(u);
        }
    }

    run_plain_chat(ctx, history, &model_id, &mut sse).await;
    sse_response(sse)
}

// ---------------------------------------------------------------------------
// Slash parser
// ---------------------------------------------------------------------------

/// Extract a leading `/<cmd>` from the user message and return
/// `(cmd, remainder)`. Embedded slashes (e.g. `please /imagine a cat`) are
/// not slash commands — only the very first non-whitespace character must be
/// `/`. Returns `None` if the message has no leading slash, or the slash is
/// followed by whitespace / empty.
fn parse_slash(user_message: &str) -> Option<(&str, &str)> {
    let trimmed = user_message.trim_start();
    let rest = trimmed.strip_prefix('/')?;
    // `/` alone or `/ word` is not a slash command — treat as plain chat.
    let mut parts = rest.splitn(2, char::is_whitespace);
    let cmd = parts.next()?.trim();
    if cmd.is_empty() {
        return None;
    }
    let args = parts.next().unwrap_or("").trim();
    Some((cmd, args))
}

/// Find the `SkillTool` descriptor for `gizza-ai/<cmd>` in the registry,
/// requiring `role == SkillRole::Skill`.
fn lookup_skill_tool<'a>(
    blocks: &'a [BlockInfo],
    cmd: &str,
) -> Option<&'a wafer_block::types::SkillTool> {
    let full = format!("{SKILL_PREFIX}{cmd}");
    blocks
        .iter()
        .find(|info| info.name == full && matches!(info.role, Some(SkillRole::Skill)))
        .and_then(|info| info.tool.as_ref())
}

// ---------------------------------------------------------------------------
// Parameter extraction
// ---------------------------------------------------------------------------

/// Outcome of converting `/cmd <args text>` into a JSON params object for the
/// skill block.
#[derive(Debug, Clone)]
enum ParamExtraction {
    /// Schema was `{prompt: string}`-shaped; params is `{prompt: <rest>}`.
    Verbatim(serde_json::Value),
    /// LLM produced a JSON object matching the schema.
    Extracted(serde_json::Value),
    /// `/cmd` was passed with no argument text and the schema requires one.
    Empty,
    /// LLM emitted `{__unsure: "..."}` — kick the question back to the UI as
    /// a confirm chip carrying the LLM's best-guess params.
    Unsure {
        question: String,
        params: serde_json::Value,
    },
    /// Hard failure (LLM call errored, JSON decode failed, etc.).
    Error(String),
}

/// Decide whether the schema is `{prompt: string}` only with `required:
/// ["prompt"]`. If so, callers can pass `args` verbatim as `{prompt: args}`
/// without invoking the LLM at all.
fn is_prompt_shaped(schema: &serde_json::Value) -> bool {
    let Some(props) = schema.get("properties").and_then(|p| p.as_object()) else {
        return false;
    };
    if props.len() != 1 {
        return false;
    }
    let Some(prompt) = props.get("prompt") else {
        return false;
    };
    if prompt.get("type").and_then(|t| t.as_str()) != Some("string") {
        return false;
    }
    let Some(required) = schema.get("required").and_then(|r| r.as_array()) else {
        return false;
    };
    required.len() == 1 && required[0].as_str() == Some("prompt")
}

async fn build_skill_params(
    ctx: &dyn Context,
    schema: &serde_json::Value,
    args_text: &str,
    model_id: &str,
) -> ParamExtraction {
    let trimmed = args_text.trim();
    if trimmed.is_empty() {
        // No args. Only OK when the schema accepts an empty object — i.e. no
        // required fields. Otherwise the caller will emit an empty-arg error.
        let no_required = schema
            .get("required")
            .and_then(|r| r.as_array())
            .is_none_or(|arr| arr.is_empty());
        return if no_required {
            ParamExtraction::Extracted(serde_json::json!({}))
        } else {
            ParamExtraction::Empty
        };
    }

    if is_prompt_shaped(schema) {
        return ParamExtraction::Verbatim(serde_json::json!({ "prompt": trimmed }));
    }

    extract_via_llm(ctx, schema, trimmed, model_id).await
}

async fn extract_via_llm(
    ctx: &dyn Context,
    schema: &serde_json::Value,
    args_text: &str,
    model_id: &str,
) -> ParamExtraction {
    let schema_str = serde_json::to_string(schema).unwrap_or_else(|_| "{}".to_string());
    let system = format!(
        "Extract a single JSON object matching this JSON Schema from the user message. \
         Respond with ONLY the JSON object — no prose, no markdown fences. \
         If you cannot decide a required field, respond with \
         {{\"__unsure\": \"<short question>\", \"__params\": <best-guess object>}}.\n\n\
         Schema: {schema_str}"
    );
    let messages = vec![
        ChatMessage {
            role: ChatRole::System,
            content: ChatContent::Text(system),
            tool_call_id: None,
            tool_calls: Vec::new(),
        },
        ChatMessage {
            role: ChatRole::User,
            content: ChatContent::Text(args_text.to_string()),
            tool_call_id: None,
            tool_calls: Vec::new(),
        },
    ];
    let req = ChatRequest {
        backend_id: "webllm".to_string(),
        model: model_id.to_string(),
        messages,
        params: ChatParams::default(),
        tools: Vec::new(),
        extra: serde_json::Value::Null,
    };

    let stream = match wafer_core::clients::llm::chat_stream(ctx, &req).await {
        Ok(s) => s,
        Err(e) => return ParamExtraction::Error(format!("LLM extraction call failed: {e}")),
    };

    let text = match collect_chat_text(stream).await {
        Ok(t) => t,
        Err(e) => return ParamExtraction::Error(e),
    };

    let json = match strip_to_json(&text) {
        Some(j) => j,
        None => {
            return ParamExtraction::Error(format!(
                "LLM extraction did not return JSON. Got: {}",
                truncate(&text, 200)
            ));
        }
    };

    let value: serde_json::Value = match serde_json::from_str(&json) {
        Ok(v) => v,
        Err(e) => {
            return ParamExtraction::Error(format!(
                "LLM extraction returned invalid JSON: {e}. Got: {}",
                truncate(&json, 200)
            ));
        }
    };

    if let Some(question) = value.get("__unsure").and_then(|v| v.as_str()) {
        let params = value
            .get("__params")
            .cloned()
            .unwrap_or_else(|| serde_json::Value::Object(serde_json::Map::new()));
        return ParamExtraction::Unsure {
            question: question.to_string(),
            params,
        };
    }

    ParamExtraction::Extracted(value)
}

/// Pull the first balanced top-level JSON object substring from `text`.
/// Tolerates leading markdown fences and trailing prose, which 1.5B-class
/// instruction-tuned models routinely emit despite the system prompt.
fn strip_to_json(text: &str) -> Option<String> {
    let start = text.find('{')?;
    let bytes = text.as_bytes();
    let mut depth = 0i32;
    let mut in_str = false;
    let mut escape = false;
    for (i, &b) in bytes.iter().enumerate().skip(start) {
        if escape {
            escape = false;
            continue;
        }
        match b {
            b'\\' if in_str => escape = true,
            b'"' => in_str = !in_str,
            b'{' if !in_str => depth += 1,
            b'}' if !in_str => {
                depth -= 1;
                if depth == 0 {
                    return Some(text[start..=i].to_string());
                }
            }
            _ => {}
        }
    }
    None
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}…", &s[..max])
    }
}

// ---------------------------------------------------------------------------
// Skill dispatch
// ---------------------------------------------------------------------------

/// Dispatch one skill and emit the resulting `tool_result` + `done` SSE
/// events into `sse`. `invocation` is a short string used to mint the
/// `tool_result.id` (e.g. "slash", "confirmed") to avoid id collisions with
/// the legacy `call_N` ids the chat history machinery still understands.
async fn run_skill_dispatch(
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

    let args_bytes = serde_json::to_vec(&params).expect("serde_json::Value always serializes");
    let mut msg = Message::new("http");
    msg.set_meta(META_REQ_ACTION, "create");
    msg.set_meta(
        wafer_block::meta::META_REQ_RESOURCE,
        format!("/b/{cmd}"),
    );

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

// ---------------------------------------------------------------------------
// Plain chat (no slash, no tool advertisement)
// ---------------------------------------------------------------------------

async fn run_plain_chat(
    ctx: &dyn Context,
    history: Vec<serde_json::Value>,
    model_id: &str,
    sse: &mut String,
) {
    let mut chat_messages: Vec<ChatMessage> = Vec::with_capacity(history.len());
    for v in &history {
        match openai_json_to_chat_message(v) {
            Ok(m) => chat_messages.push(m),
            Err(e) => {
                sse.push_str(&encode_sse_event(
                    "done",
                    &serde_json::json!({
                        "reason": "error",
                        "error": format!("invalid history message: {e}"),
                    }),
                ));
                return;
            }
        }
    }

    let req = ChatRequest {
        backend_id: "webllm".to_string(),
        model: model_id.to_string(),
        messages: chat_messages,
        params: ChatParams::default(),
        tools: Vec::new(),
        extra: serde_json::Value::Null,
    };

    let stream = match wafer_core::clients::llm::chat_stream(ctx, &req).await {
        Ok(s) => s,
        Err(e) => {
            sse.push_str(&encode_sse_event(
                "done",
                &serde_json::json!({
                    "reason": "error",
                    "error": format!("chat_stream start failed: {e}"),
                }),
            ));
            return;
        }
    };
    pin_mut!(stream);

    let mut finish_reason: Option<FinishReason> = None;
    while let Some(item) = stream.next().await {
        match item {
            Err(e) => {
                sse.push_str(&encode_sse_event(
                    "done",
                    &serde_json::json!({
                        "reason": "error",
                        "error": format!("{e}"),
                    }),
                ));
                return;
            }
            Ok(chunk) => {
                if let Some(reason) = chunk.finish_reason {
                    finish_reason = Some(reason);
                }
                if let ChunkDelta::Text(t) = &chunk.delta {
                    if !t.is_empty() {
                        sse.push_str(&encode_sse_event(
                            "token",
                            &serde_json::json!({ "delta": t }),
                        ));
                    }
                }
                if finish_reason.is_some() {
                    break;
                }
            }
        }
    }

    let reason = match finish_reason {
        Some(FinishReason::Stop) | Some(FinishReason::ToolCall) | None => "stop",
        Some(FinishReason::Length) => "length",
        Some(FinishReason::ContentFilter) => "content_filter",
        Some(FinishReason::Error) => "error",
    };
    sse.push_str(&encode_sse_event(
        "done",
        &serde_json::json!({ "reason": reason }),
    ));
}

// ---------------------------------------------------------------------------
// LLM helpers (extraction path)
// ---------------------------------------------------------------------------

async fn collect_chat_text(
    stream: impl futures::Stream<Item = Result<ChatChunk, WaferError>>,
) -> Result<String, String> {
    pin_mut!(stream);
    let mut text = String::new();
    while let Some(item) = stream.next().await {
        match item {
            Err(e) => return Err(format!("extraction stream error: {e}")),
            Ok(chunk) => {
                if let ChunkDelta::Text(t) = chunk.delta {
                    text.push_str(&t);
                }
                if chunk.finish_reason.is_some() {
                    break;
                }
            }
        }
    }
    Ok(text)
}

// ---------------------------------------------------------------------------
// Skill-response bifurcation (LLM-history vs UI render)
// ---------------------------------------------------------------------------

/// Output of a skill dispatch split into an LLM-safe summary and an optional
/// UI render hint.
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

// ---------------------------------------------------------------------------
// Uploads
// ---------------------------------------------------------------------------

/// Decode an [`UploadEntry`] vec into staged-upload tuples
/// `(id, Attachment, display_name)`. v1 caps each upload at 10 MiB; only
/// `image/*` and `video/*` MIMEs are accepted; ids must start with
/// `"upload_"`.
pub(crate) fn decode_uploads(
    entries: &[UploadEntry],
) -> Result<Vec<(String, Attachment, String)>, AgentError> {
    const MAX_UPLOAD_BYTES: usize = 10 * 1024 * 1024;
    let mut staged = Vec::with_capacity(entries.len());
    for u in entries {
        if !u.id.starts_with("upload_") {
            return Err(AgentError::UploadIdInvalid(u.id.clone()));
        }
        if !(u.mime.starts_with("image/") || u.mime.starts_with("video/")) {
            return Err(AgentError::UploadUnsupportedMime {
                id: u.id.clone(),
                mime: u.mime.clone(),
            });
        }
        let bytes = B64
            .decode(&u.bytes_base64)
            .map_err(|source| AgentError::UploadBase64 {
                id: u.id.clone(),
                source,
            })?;
        if bytes.len() > MAX_UPLOAD_BYTES {
            return Err(AgentError::UploadTooLarge {
                id: u.id.clone(),
                bytes: bytes.len(),
            });
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

/// Build the synthetic assistant + tool history-prefix entries the agent
/// injects ahead of the user message when the request carries uploads.
/// Lets the LLM see the upload refs in context for plain-chat turns.
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
             Pass {{\"ref\": \"{id}\"}} to a slash command.",
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
// Message conversion
// ---------------------------------------------------------------------------

fn openai_json_to_chat_message(v: &serde_json::Value) -> Result<ChatMessage, AgentError> {
    let role_str = v
        .get("role")
        .and_then(|r| r.as_str())
        .ok_or(AgentError::MissingRole)?;

    let role = match role_str {
        "system" => ChatRole::System,
        "user" => ChatRole::User,
        "assistant" => ChatRole::Assistant,
        "tool" => ChatRole::Tool,
        other => return Err(AgentError::UnknownRole(other.to_string())),
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

    let tool_calls: Vec<LlmToolCall> = v
        .get("tool_calls")
        .and_then(|arr| arr.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|entry| {
                    let id = entry.get("id")?.as_str()?.to_string();
                    let func = entry.get("function")?;
                    let name = func.get("name")?.as_str()?.to_string();
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
// SSE emission + response helpers
// ---------------------------------------------------------------------------

fn encode_sse_event(event_name: &str, data: &serde_json::Value) -> String {
    let data_str = serde_json::to_string(data).unwrap_or_else(|_| "null".to_string());
    format!("event: {event_name}\ndata: {data_str}\n\n")
}

fn sse_response(body: String) -> OutputStream {
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

    // -----------------------------------------------------------------------
    // parse_slash
    // -----------------------------------------------------------------------

    #[test]
    fn parse_slash_extracts_cmd_and_remainder() {
        assert_eq!(parse_slash("/imagine a cat"), Some(("imagine", "a cat")));
        assert_eq!(parse_slash("/calculator 2+2"), Some(("calculator", "2+2")));
    }

    #[test]
    fn parse_slash_handles_leading_whitespace() {
        assert_eq!(parse_slash("  /clock"), Some(("clock", "")));
    }

    #[test]
    fn parse_slash_returns_none_for_plain_text() {
        assert!(parse_slash("hello there").is_none());
        assert!(parse_slash("please /imagine a cat").is_none());
    }

    #[test]
    fn parse_slash_returns_none_for_empty_cmd() {
        assert!(parse_slash("/").is_none());
        assert!(parse_slash("/ foo").is_none());
    }

    #[test]
    fn parse_slash_no_args_is_empty_string() {
        assert_eq!(parse_slash("/clock"), Some(("clock", "")));
        assert_eq!(parse_slash("/clock   "), Some(("clock", "")));
    }

    // -----------------------------------------------------------------------
    // is_prompt_shaped
    // -----------------------------------------------------------------------

    #[test]
    fn is_prompt_shaped_recognises_single_required_string() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": { "prompt": { "type": "string" } },
            "required": ["prompt"],
        });
        assert!(is_prompt_shaped(&schema));
    }

    #[test]
    fn is_prompt_shaped_rejects_extra_properties() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "prompt": { "type": "string" },
                "seed": { "type": "integer" }
            },
            "required": ["prompt"],
        });
        assert!(!is_prompt_shaped(&schema));
    }

    #[test]
    fn is_prompt_shaped_rejects_when_required_missing() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": { "prompt": { "type": "string" } },
        });
        assert!(!is_prompt_shaped(&schema));
    }

    #[test]
    fn is_prompt_shaped_rejects_non_string_prompt() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": { "prompt": { "type": "integer" } },
            "required": ["prompt"],
        });
        assert!(!is_prompt_shaped(&schema));
    }

    // -----------------------------------------------------------------------
    // list_skill_commands
    // -----------------------------------------------------------------------

    #[test]
    fn list_skill_commands_filters_to_skills_strips_prefix_and_sorts() {
        let skill_a = BlockInfo::new("gizza-ai/imagine", "0.1.0", "handler@v1", "")
            .role(SkillRole::Skill)
            .tool(SkillTool {
                description: "Generate image".into(),
                parameters: serde_json::json!({}),
            });
        let skill_b = BlockInfo::new("gizza-ai/clock", "0.1.0", "handler@v1", "")
            .role(SkillRole::Skill)
            .tool(SkillTool {
                description: "Current time".into(),
                parameters: serde_json::json!({}),
            });
        let non_skill = BlockInfo::new("gizza-ai/ui", "0.1.0", "handler@v1", "");
        let other_prefix = BlockInfo::new("suppers-ai/other", "0.1.0", "handler@v1", "")
            .role(SkillRole::Skill)
            .tool(SkillTool {
                description: "should be filtered".into(),
                parameters: serde_json::json!({}),
            });
        let cmds = list_skill_commands(&[skill_a, skill_b, non_skill, other_prefix]);
        assert_eq!(
            cmds,
            vec![
                ("clock".to_string(), "Current time".to_string()),
                ("imagine".to_string(), "Generate image".to_string()),
            ]
        );
    }

    #[test]
    fn list_skill_commands_skips_skills_without_tool() {
        let bare = BlockInfo::new("gizza-ai/x", "0.1.0", "handler@v1", "").role(SkillRole::Skill);
        let cmds = list_skill_commands(&[bare]);
        assert!(cmds.is_empty());
    }

    // -----------------------------------------------------------------------
    // lookup_skill_tool
    // -----------------------------------------------------------------------

    #[test]
    fn lookup_skill_tool_resolves_short_name() {
        let blocks = [BlockInfo::new("gizza-ai/imagine", "0.1.0", "handler@v1", "")
            .role(SkillRole::Skill)
            .tool(SkillTool {
                description: "Generate image".into(),
                parameters: serde_json::json!({}),
            })];
        let tool = lookup_skill_tool(&blocks, "imagine").expect("found");
        assert_eq!(tool.description, "Generate image");
    }

    #[test]
    fn lookup_skill_tool_returns_none_for_missing_or_non_skill() {
        let non_skill = BlockInfo::new("gizza-ai/ui", "0.1.0", "handler@v1", "");
        assert!(lookup_skill_tool(&[non_skill], "ui").is_none());
        assert!(lookup_skill_tool(&[], "missing").is_none());
    }

    // -----------------------------------------------------------------------
    // strip_to_json
    // -----------------------------------------------------------------------

    #[test]
    fn strip_to_json_extracts_balanced_object_from_prose() {
        let s = "Sure! Here is the JSON: {\"prompt\":\"a cat\"} done.";
        assert_eq!(
            strip_to_json(s).as_deref(),
            Some(r#"{"prompt":"a cat"}"#)
        );
    }

    #[test]
    fn strip_to_json_handles_nested_objects() {
        let s = r#"```json
{"outer": {"inner": "x"}, "list": [1, 2]}
```"#;
        assert_eq!(
            strip_to_json(s).as_deref(),
            Some(r#"{"outer": {"inner": "x"}, "list": [1, 2]}"#)
        );
    }

    #[test]
    fn strip_to_json_handles_braces_inside_strings() {
        let s = r#"prefix {"text": "a } b"} suffix"#;
        assert_eq!(strip_to_json(s).as_deref(), Some(r#"{"text": "a } b"}"#));
    }

    #[test]
    fn strip_to_json_returns_none_when_no_object() {
        assert!(strip_to_json("just text").is_none());
        assert!(strip_to_json("{unbalanced").is_none());
    }

    // -----------------------------------------------------------------------
    // encode_sse_event
    // -----------------------------------------------------------------------

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

    // -----------------------------------------------------------------------
    // openai_json_to_chat_message
    // -----------------------------------------------------------------------

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
    fn openai_json_to_chat_message_tool() {
        let v = serde_json::json!({
            "role": "tool",
            "tool_call_id": "call_1",
            "content": "2026-04-20T12:00:00Z",
        });
        let msg = openai_json_to_chat_message(&v).expect("valid");
        assert_eq!(msg.role, ChatRole::Tool);
        assert_eq!(msg.tool_call_id, Some("call_1".to_string()));
    }

    #[test]
    fn openai_json_to_chat_message_unknown_role_errors() {
        let v = serde_json::json!({ "role": "unknown", "content": "" });
        assert!(openai_json_to_chat_message(&v).is_err());
    }

    // -----------------------------------------------------------------------
    // parse_skill_response
    // -----------------------------------------------------------------------

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

    // -----------------------------------------------------------------------
    // decode_uploads
    // -----------------------------------------------------------------------

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
    }

    #[test]
    fn decode_uploads_rejects_oversize_decoded_bytes() {
        let big = vec![0u8; 10 * 1024 * 1024 + 1];
        let entries = vec![UploadEntry {
            id: "upload_1".into(),
            mime: "image/png".into(),
            filename: None,
            bytes_base64: B64.encode(&big),
        }];
        let r = decode_uploads(&entries);
        assert!(r.is_err());
    }

    #[test]
    fn build_upload_history_prefix_emits_assistant_and_tool_pair_per_upload() {
        let staged = vec![(
            "upload_1".to_string(),
            Attachment {
                mime: "image/png".into(),
                bytes: vec![0u8; 1228800],
                filename: Some("cat.png".into()),
            },
            "cat.png".to_string(),
        )];
        let prefix = build_upload_history_prefix(&staged);
        assert_eq!(prefix.len(), 2);
        assert_eq!(prefix[0]["role"], "assistant");
        assert_eq!(prefix[0]["tool_calls"][0]["id"], "upload_1");
        assert_eq!(prefix[1]["role"], "tool");
        assert_eq!(prefix[1]["tool_call_id"], "upload_1");
        let content = prefix[1]["content"].as_str().unwrap();
        assert!(content.contains("upload_1"));
        assert!(content.contains("image/png"));
        assert!(content.contains("slash command"));
    }
}

