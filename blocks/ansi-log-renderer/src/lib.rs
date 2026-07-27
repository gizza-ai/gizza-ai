//! gizza-ai/ansi-log-renderer — parse raw terminal output containing ANSI escape
//! codes and render it to clean colored HTML, or strip the codes to plain text.
//! Thin chat-skill wrapper around `gizza-ai-ansi-log-renderer-core`; the chat
//! schema is single-sourced from `descriptor()` (shared with the CLI) and the
//! handler delegates to `block_utils::run_skill`. No host calls — runs entirely
//! inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default)]
    output: String,
    #[serde(default)]
    theme: String,
    #[serde(default)]
    styles: String,
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The raw terminal/log output containing ANSI escape codes to render, e.g. \\x1b[31mERROR\\x1b[0m."),
        )
        .param(
            Param::enumv("output", ["html", "text"])
                .default("html")
                .describe("Output format. 'html' (default) renders the colors/styles as a self-contained <pre> of styled <span>s. 'text' strips every escape sequence and returns plain readable text."),
        )
        .param(
            Param::enumv("theme", ["dark", "light"])
                .default("dark")
                .describe("Background theme for HTML output — sets the default foreground/background colors and the <pre> background. 'dark' (default) = light text on #0c0c0c; 'light' = dark text on #ffffff. Ignored when output='text'."),
        )
        .param(
            Param::enumv("styles", ["inline", "classes"])
                .default("inline")
                .describe("How HTML colors are applied. 'inline' (default) emits self-contained style=\"...\" attributes. 'classes' emits class=\"ansi-...\" spans plus a <style> block (basic colors become classes; 256/RGB colors stay inline). Ignored when output='text'."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct AnsiLogRenderer;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/ansi-log-renderer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Render ANSI terminal output as colored HTML, or strip it to plain text",
    skill(
        description = "Parse raw terminal output containing ANSI escape codes and render it to clean colored HTML, or strip the codes to plain text. Pass the captured terminal/log text as 'text'. output='html' (default) reproduces the SGR colors and styles — 16 basic + bright colors, the 256-color xterm palette, 24-bit RGB truecolor, and bold/dim/italic/underline/inverse/strikethrough — as styled <span>s inside a themed <pre>; non-SGR control (cursor moves, screen erase, OSC titles/hyperlinks) is dropped. output='text' strips every escape sequence and returns plain text. theme='dark'|'light' sets the default colors/background for HTML. styles='inline' (self-contained style attributes) or 'classes' (class names + a <style> block). Unicode and newlines are preserved.",
        parameters = schema_json()
    ),
)]
impl AnsiLogRenderer {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": … }.
        match run_skill(&body, "ansi-log-renderer", |a: Args| {
            gizza_ai_ansi_log_renderer_core::render(&a.text, &a.output, &a.theme, &a.styles)
                .map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any change to the LLM-facing API is intentional and reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "The raw terminal/log output containing ANSI escape codes to render, e.g. \\x1b[31mERROR\\x1b[0m." },
                    "output": { "type": "string", "enum": ["html", "text"], "default": "html", "description": "Output format. 'html' (default) renders the colors/styles as a self-contained <pre> of styled <span>s. 'text' strips every escape sequence and returns plain readable text." },
                    "theme": { "type": "string", "enum": ["dark", "light"], "default": "dark", "description": "Background theme for HTML output — sets the default foreground/background colors and the <pre> background. 'dark' (default) = light text on #0c0c0c; 'light' = dark text on #ffffff. Ignored when output='text'." },
                    "styles": { "type": "string", "enum": ["inline", "classes"], "default": "inline", "description": "How HTML colors are applied. 'inline' (default) emits self-contained style=\"...\" attributes. 'classes' emits class=\"ansi-...\" spans plus a <style> block (basic colors become classes; 256/RGB colors stay inline). Ignored when output='text'." }
                },
                "required": ["text"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
