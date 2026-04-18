//! gizza-ai/ui — public chat page.
//!
//! Serves GET / and GET /b/ui/ with a maud-rendered chat UI.
//! The page loads gizza-app.js which runs the model + streams
//! responses from /b/agent/chat via the fetch API + SSE parsing.

use async_trait::async_trait;
use maud::{html, PreEscaped, DOCTYPE};
use wafer_block::{
    block::Block,
    context::Context,
    core_types::{LifecycleEvent, Message, MetaEntry, WaferError},
    meta::{META_RESP_CONTENT_TYPE, META_RESP_STATUS},
    streams::{input::InputStream, output::OutputStream},
    types::BlockInfo,
};

/// The WebLLM model id used for MVP. Picked for small size + tool-call support.
/// Plan C makes this user-pickable.
const MVP_MODEL_ID: &str = "Qwen2.5-1.5B-Instruct-q4f32_1-MLC";

pub struct UiBlock;

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl Block for UiBlock {
    fn info(&self) -> BlockInfo {
        BlockInfo::new("gizza-ai/ui", "0.1.0", "http-handler@v1", "Public chat page")
            .category(wafer_run::BlockCategory::Feature)
    }

    async fn handle(
        &self,
        _ctx: &dyn Context,
        msg: Message,
        _input: InputStream,
    ) -> OutputStream {
        let action = msg.action();
        let path = msg.path();

        // GET-equivalent for a page render — accept "retrieve" action.
        if action == "retrieve"
            && (path == "/" || path == "/b/ui/" || path == "/b/ui")
        {
            let markup = render_chat();
            let html_bytes = markup.into_string().into_bytes();
            return OutputStream::respond_with_meta(
                html_bytes,
                vec![
                    MetaEntry {
                        key: META_RESP_STATUS.to_string(),
                        value: "200".to_string(),
                    },
                    MetaEntry {
                        key: META_RESP_CONTENT_TYPE.to_string(),
                        value: "text/html; charset=utf-8".to_string(),
                    },
                ],
            );
        }

        // Unknown route for this block.
        let err = serde_json::json!({ "error": "not_found", "path": path });
        let body = serde_json::to_vec(&err).unwrap_or_default();
        OutputStream::respond_with_meta(
            body,
            vec![
                MetaEntry {
                    key: META_RESP_STATUS.to_string(),
                    value: "404".to_string(),
                },
                MetaEntry {
                    key: META_RESP_CONTENT_TYPE.to_string(),
                    value: "application/json".to_string(),
                },
            ],
        )
    }

    async fn lifecycle(
        &self,
        _ctx: &dyn Context,
        _event: LifecycleEvent,
    ) -> Result<(), WaferError> {
        Ok(())
    }
}

fn render_chat() -> maud::Markup {
    html! {
        (DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width, initial-scale=1";
                title { "gizza-ai" }
                link rel="stylesheet" href="/gizza.css";
            }
            body {
                header class="topbar" {
                    h1 { "gizza-ai" }
                    button id="open-settings" type="button" aria-label="Settings" { "⚙" }
                }
                main id="chat" {
                    div id="messages" {
                        div class="empty" { "Load a model in settings to start." }
                    }
                }
                form id="composer" autocomplete="off" {
                    textarea id="user-input" name="user_message" placeholder="Ask anything…" rows="2" {}
                    button id="send" type="submit" disabled { "Send" }
                }
                dialog id="settings" {
                    form method="dialog" {
                        h2 { "Settings" }
                        p {
                            "Model: " code { (MVP_MODEL_ID) }
                        }
                        p class="help" { "Model downloads once per browser (~1.2 GB). Tools: supported." }
                        button id="load-model" type="button" { "Load model" }
                        button id="clear-convo" type="button" { "Clear conversation" }
                        button value="close" { "Close" }
                    }
                }
                // Expose the model id to gizza-app.js without parsing the DOM string.
                script {
                    (PreEscaped(format!(
                        "window.__GIZZA_MODEL_ID = {:?};",
                        MVP_MODEL_ID
                    )))
                }
                script type="module" src="/gizza-app.js" {}
            }
        }
    }
}
