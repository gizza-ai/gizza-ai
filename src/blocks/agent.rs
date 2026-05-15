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

mod chat;
mod dispatch;
mod messages;
mod slash;
mod sse;
mod uploads;

use async_trait::async_trait;
use serde::Deserialize;
use wafer_block::{
    block::Block,
    context::Context,
    core_types::{LifecycleEvent, Message, MetaEntry, WaferError},
    meta::{META_RESP_CONTENT_TYPE, META_RESP_STATUS},
    streams::{input::InputStream, output::OutputStream},
    types::{BlockInfo, SkillRole},
};

use chat::run_plain_chat;
use dispatch::run_skill_dispatch;
use slash::{build_skill_params, lookup_skill_tool, parse_slash, ParamExtraction};
use sse::{encode_sse_event, error_response, sse_response};
use uploads::{build_upload_history_prefix, decode_uploads};

/// The agent block's chat endpoint.
const AGENT_CHAT_PATH: &str = "/b/agent/chat";

/// The agent block's command-list endpoint.
const AGENT_COMMANDS_PATH: &str = "/b/agent/commands";

/// Prefix every gizza-built skill block carries. Slash commands map
/// `/<cmd>` → `<SKILL_PREFIX><cmd>` for registry lookup.
const SKILL_PREFIX: &str = "gizza-ai/";

use super::DEFAULT_MODEL_ID;

/// Internal errors for the agent block. Each variant maps to a distinct
/// failure mode in `decode_uploads` or `openai_json_to_chat_message`. The
/// caller uses `Display` to render the user-facing message, then wraps it in
/// an `error_response` or SSE `done` event.
#[derive(Debug, thiserror::Error)]
enum AgentError {
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
struct UploadEntry {
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
    // serde_json::Value always serializes successfully, but `.expect()` is a
    // hard trap on wasm32 — fall back to an empty array instead.
    let body = serde_json::to_vec(&entries).unwrap_or_else(|_| b"[]".to_vec());
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

#[cfg(test)]
mod tests {
    use super::*;
    use wafer_block::types::SkillTool;

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
}
