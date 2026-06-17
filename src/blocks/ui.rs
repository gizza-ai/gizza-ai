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

use super::DEFAULT_MODEL_ID;

pub struct UiBlock;

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl Block for UiBlock {
    fn info(&self) -> BlockInfo {
        BlockInfo::new(
            "gizza-ai/ui",
            "0.1.0",
            "http-handler@v1",
            "Public chat page",
        )
        .category(wafer_run::BlockCategory::Feature)
    }

    async fn handle(&self, _ctx: &dyn Context, msg: Message, _input: InputStream) -> OutputStream {
        let action = msg.action();
        let path = msg.path();

        // GET-equivalent for a page render — accept "retrieve" action.
        if action == "retrieve" && (path == "/" || path == "/b/ui/" || path == "/b/ui") {
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
                link rel="stylesheet" href="https://site-kit.suppers.ai/dist/design-system.css";
                script type="module" src="https://site-kit.suppers.ai/dist/components/sa-header.js" {}
                script type="module" src="https://site-kit.suppers.ai/dist/components/sa-chat.js" {}
                // Markdown renderer for assistant messages.
                script src="https://cdn.jsdelivr.net/npm/marked@13.0.0/marked.min.js" {}
                // Syntax highlighting for fenced code blocks.
                link rel="stylesheet" href="https://cdn.jsdelivr.net/gh/highlightjs/cdn-release@11.9.0/build/styles/github-dark.min.css";
                script src="https://cdn.jsdelivr.net/gh/highlightjs/cdn-release@11.9.0/build/highlight.min.js" {}
                link rel="stylesheet" href="/gizza.css";
                link rel="stylesheet" href="/model-picker.css";
                link rel="stylesheet" href="/tools-modal.css";
            }
            body {
                sa-header {
                    a slot="brand" href="/" class="brand" {
                        div class="brand-logo brand-mascot" data-pose="resting" {
                            img id="brand-still" class="brand-still" src="/gis_no_eyes.png" alt="";
                            video id="brand-video" class="brand-video" muted playsinline preload="auto" hidden {}
                            div class="brand-eyes" aria-hidden="true" {
                                div class="brand-eye-socket brand-eye-left" {
                                    img class="brand-eye" src="/eye.png" alt="";
                                }
                                div class="brand-eye-socket brand-eye-right" {
                                    img class="brand-eye" src="/eye.png" alt="";
                                }
                            }
                        }
                        h1 { "gizza-ai" }
                    }
                    // Empty button — gizza.css draws a three-dot horizontal icon
                    // via `::before { mask-image }` once the button lands in the
                    // composer. Clicking it opens the composer popup menu.
                    button slot="actions" id="open-settings" type="button" aria-label="Menu" {}
                }
                sa-chat {
                    div slot="messages" id="messages" {
                        div class="empty" {
                            span class="empty-msg" {
                                "I can't do much without a brain, please "
                                button id="empty-state-cta" type="button" class="empty-state-link" { "choose a model" }
                                "."
                            }
                        }
                    }
                    form slot="composer" id="composer" autocomplete="off" {
                        div id="upload-chips" class="upload-chips empty" {}
                        textarea id="user-input" name="user_message" placeholder="Ask anything…" rows="2" {}
                        button id="open-brain-picker" type="button" aria-label="Choose model" title="Choose model" {}
                        button id="open-tools" type="button" aria-label="Tools" title="Browse tools" {}
                        button id="attach" type="button" aria-label="Attach file" title="Attach image or video" {}
                        button id="send" type="submit" disabled { "Send" }
                        input id="file-picker" type="file" accept="image/*,video/*" multiple style="display:none;";
                    }
                    // Disclaimer below the composer. Also slotted into sa-chat's
                    // "composer" slot (slots accept multiple elements), so it sits
                    // directly under the input card.
                    p slot="composer" class="composer-note" {
                        "I can be wrong — don't get crabby 🦀. The "
                        a href="https://github.com/gizza-ai/gizza-ai/blob/main/cli/README.md" target="_blank" rel="noopener" { "CLI tool" }
                        " and "
                        a href="https://github.com/gizza-ai/gizza-ai/blob/main/SKILL.md" target="_blank" rel="noopener" { "SKILL.md" }
                        " slip up too; double-check the important bits."
                    }
                }
                // Composer popup menu — positioned near the ⋮ button by JS.
                div id="composer-menu" role="menu" hidden {
                    button id="menu-info" type="button" role="menuitem" { "Info" }
                    button id="menu-webgpu" type="button" role="menuitem" { "WebGPU help" }
                    a id="menu-discord" role="menuitem" href="https://discord.gg/jKqMcbrVzm" target="_blank" rel="noopener" { "Join Discord" }
                    button id="menu-clear" type="button" role="menuitem" { "Clear conversation" }
                    button id="menu-close" type="button" role="menuitem" { "Close" }
                }
                // About / Info modal.
                dialog id="info-dialog" {
                    form method="dialog" {
                        h2 { "About gizza.ai" }
                        p {
                            "gizza.ai is a browser-local AI chat. All inference runs on "
                            "your device via WebGPU + WebAssembly — your conversations "
                            "never leave your browser."
                        }
                        p class="help" {
                            "Source on GitHub: "
                            a href="https://github.com/gizza-ai/gizza-ai" target="_blank" rel="noopener" {
                                "github.com/gizza-ai/gizza-ai"
                            }
                        }
                        p class="help" {
                            "Prefer the terminal? Run the same tools with the "
                            a href="https://github.com/gizza-ai/gizza-ai/blob/main/cli/README.md" target="_blank" rel="noopener" {
                                "gizza CLI"
                            }
                            " — or let an LLM agent drive them via "
                            a href="https://github.com/gizza-ai/gizza-ai/blob/main/SKILL.md" target="_blank" rel="noopener" {
                                "SKILL.md"
                            }
                            "."
                        }
                        button value="close" { "Close" }
                    }
                }
                // Searchable tools directory — opened by the composer hammer (#open-tools).
                dialog id="tools-modal" aria-label="Tools" {
                    div class="tools-modal-head" {
                        input id="tools-search" type="search" placeholder="Search tools…" autocomplete="off" aria-label="Search tools";
                        // Native dialog close — works even if tools-modal.js fails to load.
                        form method="dialog" class="tools-close-form" {
                            button id="tools-close" type="submit" value="close" aria-label="Close" { "✕" }
                        }
                    }
                    ul id="tools-results" {}
                    p id="tools-empty" class="tools-empty" hidden { "No tools match your search." }
                    p id="tools-error" class="tools-error" hidden {
                        "Couldn't load tools. "
                        button id="tools-retry" type="button" { "Retry" }
                    }
                }
                dialog id="settings" {
                    form method="dialog" {
                        h2 { "WebGPU help" }

                        // WebGPU instructions block. Now always visible — the
                        // dialog opens from the ⋮ menu's "WebGPU help" item, so
                        // users browse the tabs even when WebGPU works.
                        // gizza-app.js still toggles the `data-warn` state when
                        // an adapter is missing (used for top-of-dialog banner
                        // styling, if any).
                        div id="webgpu-warning" {
                            p class="help" {
                                "Model inference requires WebGPU. Follow the steps for your browser:"
                            }
                            div class="tabs" role="tablist" {
                                button type="button" class="tab active" data-tab="chrome" role="tab" { "Chrome / Edge" }
                                button type="button" class="tab" data-tab="firefox" role="tab" { "Firefox" }
                                button type="button" class="tab" data-tab="safari" role="tab" { "Safari" }
                                button type="button" class="tab" data-tab="other" role="tab" { "Other" }
                            }
                            div class="tab-panel" data-tab="chrome" {
                                ol {
                                    li {
                                        "Open this URL in a new tab: "
                                        button type="button" class="copy-url" data-url="chrome://flags/#enable-unsafe-webgpu" title="Click to copy" {
                                            "chrome://flags/#enable-unsafe-webgpu"
                                        }
                                        " (browsers block " code { "chrome://" } " links; click to copy)."
                                    }
                                    li { "Set the flag to " strong { "Enabled" } "." }
                                    li { "Restart the browser, then reload this page." }
                                    li {
                                        "If it still fails, visit " a href="https://webgpureport.org/" target="_blank" rel="noopener" { "webgpureport.org" }
                                        " — if that page reports no adapter, your GPU/driver doesn't support WebGPU yet."
                                    }
                                }
                            }
                            div class="tab-panel" data-tab="firefox" hidden {
                                ol {
                                    li { "Firefox 141+ ships WebGPU on Windows by default. On Linux/macOS, enable it: " }
                                    li {
                                        "Open "
                                        button type="button" class="copy-url" data-url="about:config" title="Click to copy" { "about:config" }
                                        " and accept the warning."
                                    }
                                    li {
                                        "Find "
                                        button type="button" class="copy-url" data-url="dom.webgpu.enabled" title="Click to copy" { "dom.webgpu.enabled" }
                                        " and set it to " strong { "true" } "."
                                    }
                                    li { "Restart Firefox, then reload this page." }
                                    li {
                                        "If it still fails, try " a href="https://www.mozilla.org/firefox/channel/desktop/#nightly" target="_blank" rel="noopener" { "Firefox Nightly" } ", which has the most up-to-date WebGPU support."
                                    }
                                }
                            }
                            div class="tab-panel" data-tab="safari" hidden {
                                ol {
                                    li { "Safari 17+ (macOS Sonoma / iOS 17+) supports WebGPU behind a flag." }
                                    li { "Safari → Settings → Feature Flags → enable " strong { "WebGPU" } "." }
                                    li { "Close the tab and reopen this page." }
                                    li { "On iOS, enable in Settings → Safari → Advanced → Feature Flags → WebGPU." }
                                }
                            }
                            div class="tab-panel" data-tab="other" hidden {
                                p {
                                    "WebGPU is a browser-delivered API. If your browser doesn't support it, try Chrome, Edge, Firefox Nightly, or Safari 17+. "
                                    "A compatible GPU and up-to-date driver are also required — check " a href="https://webgpureport.org/" target="_blank" rel="noopener" { "webgpureport.org" } " to verify."
                                }
                                p class="help" {
                                    "Known unsupported: remote desktops (RDP/VNC), most headless Chromium runs, some VMs without GPU passthrough, and old hardware (pre-2014 integrated graphics)."
                                }
                            }
                        }

                        // Keep #clear-convo in the DOM (the menu's Clear item
                        // delegates to it) but hide it visually inside this
                        // dialog — Clear belongs in the menu, not here.
                        button id="clear-convo" type="button" hidden { "Clear conversation" }
                        button value="close" { "Close" }
                    }
                }
                // Expose the model id to gizza-app.js without parsing the DOM string.
                script {
                    (PreEscaped(format!(
                        "window.__GIZZA_MODEL_ID = {:?};",
                        DEFAULT_MODEL_ID
                    )))
                }
                // Framework-provided page-side WebLLM engine. Two consumers:
                // (1) Page-direct load: gizza-app.js imports loadEngine()
                //     from /webllm-engine.js and calls it directly — no SW.
                // (2) SW-routed chat: gizza-app.js POSTs to /b/agent/chat,
                //     which routes through ctx.call_block("wafer-run/llm",
                //     ...) → BrowserLlmService::chat → postMessage page.
                script type="module" src="/webllm-engine.js" {}
                // Page-side text-to-image (imagine) engine — image-* bridge
                // already exists upstream; loading it is the whole fix.
                script type="module" src="/t2i-engine.js" {}
                // Page-side chat ffmpeg engine — runs ffmpeg in the window and
                // replies to the SW (see js/ffmpeg-bridge.js / js/ffmpeg-engine.js).
                script type="module" src="/ffmpeg-engine.js" {}
                script type="module" src="/gizza-app.js" {}
                script type="module" src="/tools-modal.js" {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_tools_button_and_modal() {
        let s = render_chat().into_string();
        assert!(s.contains(r#"id="open-tools""#), "hammer button present");
        assert!(s.contains(r#"id="tools-modal""#), "tools modal present");
        assert!(s.contains(r#"id="tools-search""#), "search input present");
        assert!(!s.contains("class=\"gizza-tools\""), "old inline list removed");
    }

    #[test]
    fn about_dialog_links_to_cli_and_skill() {
        let s = render_chat().into_string();
        assert!(s.contains("gizza CLI"), "About dialog mentions the CLI");
        assert!(s.contains("blob/main/cli/README.md"), "links to the CLI docs");
        assert!(s.contains("blob/main/SKILL.md"), "links to SKILL.md for agents");
    }

    #[test]
    fn renders_composer_disclaimer() {
        let s = render_chat().into_string();
        assert!(s.contains(r#"class="composer-note""#), "disclaimer present");
        assert!(s.contains("don't get crabby"), "disclaimer text present");
        assert!(s.contains("blob/main/cli/README.md"), "links to the CLI tool");
        // slotted into the composer slot (sa-chat) so it sits under the input card.
        assert!(s.contains(r#"p slot="composer" class="composer-note""#));
    }

    #[test]
    fn loads_page_side_engines_for_chat_ffmpeg_and_imagine() {
        let s = render_chat().into_string();
        assert!(
            s.contains(r#"src="/ffmpeg-engine.js""#),
            "chat ffmpeg page engine loaded"
        );
        assert!(
            s.contains(r#"src="/t2i-engine.js""#),
            "imagine (t2i) page engine loaded"
        );
    }
}
