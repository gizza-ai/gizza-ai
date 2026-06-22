//! gizza-ai/syntax-highlighter — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Thin wrapper around
//! `gizza-ai-syntax-highlighter-core` — no host calls, runs entirely inside the
//! WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_syntax_highlighter_core::{highlight, parse_format, HighlightError};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    code: String,
    #[serde(default)]
    language: String,
    #[serde(default)]
    theme: String,
    #[serde(default)]
    format: String,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("code")
                .required()
                .describe("The source code to highlight. Required."),
        )
        .param(Param::string("language").describe(
            "Language hint for highlighting (e.g. \"rust\", \"python\", \"javascript\", \"bash\", \"json\"). Optional — unknown or omitted falls back to plain (uncolored) text.",
        ))
        .param(Param::string("theme").describe(
            "Color theme name (e.g. \"base16-ocean.dark\", \"Solarized (dark)\", \"InspiredGitHub\"). Optional — defaults to a dark theme; unknown names fall back to the default.",
        ))
        .param(
            Param::enumv("format", ["html", "ansi"]).default("html").describe(
                "Output format. 'html' (default) returns inline-styled HTML (a self-contained <pre> with colored <span>s, no external CSS); 'ansi' returns ANSI 24-bit terminal escape codes for printing in a terminal.",
            ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

/// Map a core `HighlightError` onto a user-facing `SkillError`.
fn map_err(e: HighlightError) -> SkillError {
    SkillError::InvalidArgs(format!("invalid syntax-highlighter args: {e}"))
}

#[cfg(target_arch = "wasm32")]
struct SyntaxHighlighter;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/syntax-highlighter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Syntax-highlight a code snippet to styled HTML or ANSI",
    skill(
        description = "Highlight a code snippet for a given language and return styled output. format='html' (default) returns self-contained inline-styled HTML (a <pre> with colored <span>s, no external stylesheet) ready to embed; format='ansi' returns ANSI 24-bit terminal escape codes for printing in a terminal. Set 'language' to a hint like 'rust', 'python', 'javascript', 'bash', or 'json' (unknown/omitted falls back to uncolored plain text), and 'theme' to a syntect theme such as 'base16-ocean.dark' (default), 'Solarized (dark)', or 'InspiredGitHub'.",
        parameters = schema_json()
    ),
)]
impl SyntaxHighlighter {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "syntax-highlighter", |a: Args| {
            let fmt = parse_format(&a.format).map_err(map_err)?;
            let language = (!a.language.trim().is_empty()).then_some(a.language.as_str());
            let theme = (!a.theme.trim().is_empty()).then_some(a.theme.as_str());
            highlight(&a.code, language, theme, fmt).map_err(map_err)
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
    /// schema, so any future change to the LLM-facing API is intentional and
    /// reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "code": { "type": "string", "description": "The source code to highlight. Required." },
                    "language": { "type": "string", "description": "Language hint for highlighting (e.g. \"rust\", \"python\", \"javascript\", \"bash\", \"json\"). Optional — unknown or omitted falls back to plain (uncolored) text." },
                    "theme": { "type": "string", "description": "Color theme name (e.g. \"base16-ocean.dark\", \"Solarized (dark)\", \"InspiredGitHub\"). Optional — defaults to a dark theme; unknown names fall back to the default." },
                    "format": { "type": "string", "enum": ["html", "ansi"], "default": "html", "description": "Output format. 'html' (default) returns inline-styled HTML (a self-contained <pre> with colored <span>s, no external CSS); 'ansi' returns ANSI 24-bit terminal escape codes for printing in a terminal." }
                },
                "required": ["code"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
