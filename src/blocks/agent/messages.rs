//! Convert an OpenAI-shaped JSON message into a wafer `ChatMessage`.

use wafer_core::clients::llm::{ChatContent, ChatMessage, ChatRole, ToolCall as LlmToolCall};

use super::AgentError;

pub(super) fn openai_json_to_chat_message(v: &serde_json::Value) -> Result<ChatMessage, AgentError> {
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
