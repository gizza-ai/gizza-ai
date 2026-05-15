//! Plain LLM chat path — no slash, no tool advertisement. Buffers the
//! stream into the SSE response body as `token` events.

use futures::{pin_mut, StreamExt};
use wafer_block::context::Context;
use wafer_core::clients::llm::{
    ChatMessage, ChatParams, ChatRequest, ChunkDelta, FinishReason,
};

use super::messages::openai_json_to_chat_message;
use super::sse::encode_sse_event;

pub(super) async fn run_plain_chat(
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
