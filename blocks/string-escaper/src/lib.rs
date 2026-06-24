//! gizza-ai/string-escaper — escape (or unescape) a string for a chosen target
//! syntax: JSON, JavaScript, HTML, URL, shell, SQL, or regex. Chat schema is
//! single-sourced from descriptor() (which also drives the CLI); handle()
//! delegates to block_utils::run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_string_escaper_core::run as escape_run;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    target: String,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default)]
    quotes: bool,
}

fn default_mode() -> String {
    "escape".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The string to escape or unescape."),
        )
        .param(
            Param::enumv(
                "target",
                ["json", "javascript", "html", "url", "shell", "sql", "regex"],
            )
            .required()
            .describe("The syntax to escape the string for: 'json' (JSON string body), 'javascript' (JS string literal), 'html' (HTML/XML entities), 'url' (percent-encoding), 'shell' (POSIX single-quoted argument), 'sql' (single-quoted SQL literal), or 'regex' (escape regex metacharacters)."),
        )
        .param(
            Param::enumv("mode", ["escape", "unescape"])
                .default("escape")
                .describe("'escape' (default) makes the text safe for the target; 'unescape' reverses it. Unescape is only supported for json, javascript, html, and url; shell, sql, and regex are one-way."),
        )
        .param(
            Param::boolean("quotes")
                .default(false)
                .describe("Escape mode only, json/javascript/sql targets: also wrap the result in the target's surrounding quotes."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct StringEscaper;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/string-escaper",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Escape or unescape a string for a chosen target syntax",
    skill(
        description = "Escape (or unescape) a string for a chosen target syntax. Set target to one of: 'json' (escape quotes/backslashes/control chars for a JSON string body), 'javascript' (a JS string literal, also escaping ' \" ` and U+2028/U+2029), 'html' (HTML/XML entities: & < > \" '), 'url' (percent-encode a component), 'shell' (wrap as a safe POSIX single-quoted argument), 'sql' (double single quotes for a SQL string literal), or 'regex' (backslash-escape regex metacharacters). mode=escape (default) or mode=unescape; unescape works for json/javascript/html/url only (shell/sql/regex are one-way). Set quotes=true to also wrap json/javascript/sql output in surrounding quotes. Runs locally.",
        parameters = schema_json()
    ),
)]
impl StringEscaper {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "string-escaper", |a: Args| {
            escape_run(&a.text, &a.target, &a.mode, a.quotes).map_err(SkillError::InvalidArgs)
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
                    "text":   { "type": "string", "description": "The string to escape or unescape." },
                    "target": { "type": "string", "enum": ["json", "javascript", "html", "url", "shell", "sql", "regex"], "description": "The syntax to escape the string for: 'json' (JSON string body), 'javascript' (JS string literal), 'html' (HTML/XML entities), 'url' (percent-encoding), 'shell' (POSIX single-quoted argument), 'sql' (single-quoted SQL literal), or 'regex' (escape regex metacharacters)." },
                    "mode":   { "type": "string", "enum": ["escape", "unescape"], "default": "escape", "description": "'escape' (default) makes the text safe for the target; 'unescape' reverses it. Unescape is only supported for json, javascript, html, and url; shell, sql, and regex are one-way." },
                    "quotes": { "type": "boolean", "default": false, "description": "Escape mode only, json/javascript/sql targets: also wrap the result in the target's surrounding quotes." }
                },
                "required": ["text", "target"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
