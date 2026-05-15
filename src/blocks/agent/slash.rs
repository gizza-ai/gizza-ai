//! Slash-command parsing, skill lookup, and `/<cmd> <args>`-to-JSON parameter
//! extraction (either verbatim or via an LLM call against the skill's schema).

use futures::{pin_mut, StreamExt};
use wafer_block::{
    context::Context,
    core_types::WaferError,
    types::{BlockInfo, SkillRole, SkillTool},
};
use wafer_core::clients::llm::{
    ChatChunk, ChatContent, ChatMessage, ChatParams, ChatRequest, ChatRole, ChunkDelta,
};

use super::SKILL_PREFIX;

/// Extract a leading `/<cmd>` from the user message and return
/// `(cmd, remainder)`. Embedded slashes (e.g. `please /imagine a cat`) are
/// not slash commands — only the very first non-whitespace character must be
/// `/`. Returns `None` if the message has no leading slash, or the slash is
/// followed by whitespace / empty.
pub(super) fn parse_slash(user_message: &str) -> Option<(&str, &str)> {
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
pub(super) fn lookup_skill_tool<'a>(blocks: &'a [BlockInfo], cmd: &str) -> Option<&'a SkillTool> {
    let full = format!("{SKILL_PREFIX}{cmd}");
    blocks
        .iter()
        .find(|info| info.name == full && matches!(info.role, Some(SkillRole::Skill)))
        .and_then(|info| info.tool.as_ref())
}

/// Outcome of converting `/cmd <args text>` into a JSON params object for the
/// skill block.
#[derive(Debug, Clone)]
pub(super) enum ParamExtraction {
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
    Error(ExtractionError),
}

/// Typed failures from `extract_via_llm` / `collect_chat_text`. Renders to a
/// human-readable string for the SSE `done.error` payload.
#[derive(Debug, Clone, thiserror::Error)]
pub(super) enum ExtractionError {
    #[error("LLM extraction call failed: {0}")]
    ChatCallFailed(String),
    #[error("extraction stream error: {0}")]
    Stream(String),
    #[error("LLM extraction did not return JSON. Got: {0}")]
    NotJson(String),
    #[error("LLM extraction returned invalid JSON: {err}. Got: {got}")]
    InvalidJson { err: String, got: String },
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

pub(super) async fn build_skill_params(
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
        Err(e) => return ParamExtraction::Error(ExtractionError::ChatCallFailed(e.to_string())),
    };

    let text = match collect_chat_text(stream).await {
        Ok(t) => t,
        Err(e) => return ParamExtraction::Error(e),
    };

    let Some(json) = strip_to_json(&text) else {
        return ParamExtraction::Error(ExtractionError::NotJson(truncate(&text, 200).to_string()));
    };

    let value: serde_json::Value = match serde_json::from_str(&json) {
        Ok(v) => v,
        Err(e) => {
            return ParamExtraction::Error(ExtractionError::InvalidJson {
                err: e.to_string(),
                got: truncate(&json, 200).to_string(),
            });
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

async fn collect_chat_text(
    stream: impl futures::Stream<Item = Result<ChatChunk, WaferError>>,
) -> Result<String, ExtractionError> {
    pin_mut!(stream);
    let mut text = String::new();
    while let Some(item) = stream.next().await {
        match item {
            Err(e) => return Err(ExtractionError::Stream(e.to_string())),
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

#[cfg(test)]
mod tests {
    use super::*;

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
    // lookup_skill_tool
    // -----------------------------------------------------------------------

    #[test]
    fn lookup_skill_tool_resolves_short_name() {
        let blocks = [
            BlockInfo::new("gizza-ai/imagine", "0.1.0", "handler@v1", "")
                .role(SkillRole::Skill)
                .tool(SkillTool {
                    description: "Generate image".into(),
                    parameters: serde_json::json!({}),
                }),
        ];
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
        assert_eq!(strip_to_json(s).as_deref(), Some(r#"{"prompt":"a cat"}"#));
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
}
