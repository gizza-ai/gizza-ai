//! gizza-ai/code-formatter — one beautifier for HTML, CSS, JavaScript and JSON.
//! Chat schema single-sourced from descriptor() (which also drives the CLI);
//! handle() delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_code_formatter_core::{detect_language, format, parse_language, Indent};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    code: String,
    #[serde(default = "default_language")]
    language: String,
    #[serde(default = "default_indent")]
    indent: u64,
    #[serde(default = "default_indent_char")]
    indent_char: String,
}

fn default_language() -> String {
    "auto".to_string()
}

fn default_indent() -> u64 {
    2
}

fn default_indent_char() -> String {
    "space".to_string()
}

/// Resolve the requested indentation into a core [`Indent`].
fn resolve_indent(indent: u64, indent_char: &str) -> Indent {
    if indent_char.eq_ignore_ascii_case("tab") {
        Indent::Tab
    } else {
        Indent::Spaces(indent as usize)
    }
}

/// Shared happy-path: resolve the language (`auto` → detect) then beautify.
fn beautify(a: &Args) -> Result<String, String> {
    let lang = if a.language.eq_ignore_ascii_case("auto") {
        detect_language(&a.code)
    } else {
        parse_language(&a.language)
            .ok_or_else(|| format!("unknown language '{}'", a.language))?
    };
    format(&a.code, lang, resolve_indent(a.indent, &a.indent_char))
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("code")
                .required()
                .describe("The HTML, CSS, JavaScript or JSON snippet to beautify and re-indent (minified or messy input is fine)."),
        )
        .param(
            Param::enumv("language", ["auto", "html", "css", "javascript", "json"])
                .default("auto")
                .describe("Which formatter to use. 'auto' detects the language from the snippet; pick one explicitly if auto-detect guesses wrong. Default auto."),
        )
        .param(
            Param::integer("indent")
                .min(1.0)
                .max(8.0)
                .default(2)
                .describe("Spaces of indentation per nesting level (1-8). Ignored when indent_char is 'tab'. Default 2."),
        )
        .param(
            Param::enumv("indent_char", ["space", "tab"])
                .default("space")
                .describe("Indent with spaces or a tab character per level. Default space."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct CodeFormatter;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/code-formatter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Beautify and re-indent HTML, CSS, JavaScript or JSON",
    skill(
        description = "Beautify and re-indent an HTML, CSS, JavaScript or JSON snippet. Set language to auto (detect), html, css, javascript or json; indent is spaces per nesting level (1-8, default 2), or set indent_char to 'tab' for tabs. HTML/CSS/JS are only re-whitespaced — nothing is renamed, reordered or dropped; JSON is validated and re-serialized with key order preserved. Runs locally.",
        parameters = schema_json()
    ),
)]
impl CodeFormatter {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "code-formatter", |a: Args| {
            beautify(&a).map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "code":        { "type": "string", "description": "The HTML, CSS, JavaScript or JSON snippet to beautify and re-indent (minified or messy input is fine)." },
                    "language":    { "type": "string", "enum": ["auto", "html", "css", "javascript", "json"], "default": "auto", "description": "Which formatter to use. 'auto' detects the language from the snippet; pick one explicitly if auto-detect guesses wrong. Default auto." },
                    "indent":      { "type": "integer", "minimum": 1, "maximum": 8, "default": 2, "description": "Spaces of indentation per nesting level (1-8). Ignored when indent_char is 'tab'. Default 2." },
                    "indent_char": { "type": "string", "enum": ["space", "tab"], "default": "space", "description": "Indent with spaces or a tab character per level. Default space." }
                },
                "required": ["code"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn beautify_auto_detects_json() {
        let a = Args {
            code: r#"{"a":1}"#.to_string(),
            language: "auto".to_string(),
            indent: 2,
            indent_char: "space".to_string(),
        };
        assert_eq!(beautify(&a).unwrap(), "{\n  \"a\": 1\n}");
    }

    #[test]
    fn beautify_rejects_unknown_language() {
        let a = Args {
            code: "x".to_string(),
            language: "ruby".to_string(),
            indent: 2,
            indent_char: "space".to_string(),
        };
        assert!(beautify(&a).unwrap_err().contains("unknown language"));
    }
}
