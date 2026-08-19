//! gizza-ai/regex-literal-escape — escape arbitrary text into a regex-safe literal for a
//! chosen regex flavor. Chat schema single-sourced from descriptor(); handler delegates to
//! run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_regex_literal_escape_core::run as escape_run;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default = "default_flavor")]
    flavor: String,
    #[serde(default)]
    delimiter: String,
    #[serde(default)]
    escape_whitespace: bool,
    #[serde(default)]
    string_literal: bool,
}

fn default_flavor() -> String {
    "pcre".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The literal text to escape, e.g. 'a.b*c+(d)'. Up to 100000 characters."),
        )
        .param(
            Param::enumv(
                "flavor",
                [
                    "pcre",
                    "javascript",
                    "javascript-strict",
                    "python",
                    "re2",
                    "dotnet",
                    "java",
                    "ruby",
                    "rust",
                ],
            )
            .default("pcre")
            .describe("Which engine's escaping rules to reproduce: 'pcre' (PHP preg_quote, default), 'javascript' (the escapeRegExp idiom for new RegExp), 'javascript-strict' (ES2025 RegExp.escape, hex-escapes the first letter/digit), 'python' (re.escape), 're2' (Go regexp.QuoteMeta), 'dotnet' (.NET Regex.Escape), 'java' (Pattern.quote, wraps in \\Q…\\E), 'ruby' (Regexp.escape), or 'rust' (regex::escape). The metacharacter sets really do differ."),
        )
        .param(
            Param::string("delimiter")
                .default("")
                .describe("Extra punctuation characters to escape as well, like the second argument of preg_quote — usually your pattern delimiter, e.g. '/' or '#'. Letters, digits, underscores, and whitespace are rejected. Default: none."),
        )
        .param(
            Param::boolean("escape_whitespace")
                .default(false)
                .describe("Emit portable escapes for whitespace (\\t \\n \\r \\f \\v and \\x20 for a space) instead of the flavor's native handling. Turn this on for PCRE/Ruby extended (/x) mode, where a raw space is ignored. Default: false."),
        )
        .param(
            Param::boolean("string_literal")
                .default(false)
                .describe("Also escape the result for pasting between the quotes of a source-code string literal: backslashes are doubled and double quotes escaped. Use for Java/C#/Go/JS string literals, not for raw strings. Default: false."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct RegexLiteralEscape;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/regex-literal-escape",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Escape text into a regex-safe literal for a chosen regex flavor",
    skill(
        description = "Escape arbitrary text so a regex engine matches it literally, using the exact rules of the flavor you pick. flavor=pcre (PHP preg_quote, default), javascript (the escapeRegExp idiom), javascript-strict (ES2025 RegExp.escape), python (re.escape), re2 (Go regexp.QuoteMeta), dotnet (.NET Regex.Escape), java (Pattern.quote, \\Q…\\E), ruby (Regexp.escape), or rust (regex::escape) — the metacharacter sets differ, e.g. Go leaves '-' and '#' alone while PCRE escapes them. Set delimiter='/' to also escape your pattern delimiter, escape_whitespace=true for portable \\t/\\n/\\x20 escapes (needed in /x extended mode), and string_literal=true to double backslashes for pasting inside a source-code string. Returns the escaped literal. Runs locally.",
        parameters = schema_json()
    ),
)]
impl RegexLiteralEscape {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "regex-literal-escape", |a: Args| {
            escape_run(
                &a.text,
                &a.flavor,
                &a.delimiter,
                a.escape_whitespace,
                a.string_literal,
            )
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

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "The literal text to escape, e.g. 'a.b*c+(d)'. Up to 100000 characters." },
                    "flavor": { "type": "string", "enum": ["pcre", "javascript", "javascript-strict", "python", "re2", "dotnet", "java", "ruby", "rust"], "default": "pcre", "description": "Which engine's escaping rules to reproduce: 'pcre' (PHP preg_quote, default), 'javascript' (the escapeRegExp idiom for new RegExp), 'javascript-strict' (ES2025 RegExp.escape, hex-escapes the first letter/digit), 'python' (re.escape), 're2' (Go regexp.QuoteMeta), 'dotnet' (.NET Regex.Escape), 'java' (Pattern.quote, wraps in \\Q…\\E), 'ruby' (Regexp.escape), or 'rust' (regex::escape). The metacharacter sets really do differ." },
                    "delimiter": { "type": "string", "default": "", "description": "Extra punctuation characters to escape as well, like the second argument of preg_quote — usually your pattern delimiter, e.g. '/' or '#'. Letters, digits, underscores, and whitespace are rejected. Default: none." },
                    "escape_whitespace": { "type": "boolean", "default": false, "description": "Emit portable escapes for whitespace (\\t \\n \\r \\f \\v and \\x20 for a space) instead of the flavor's native handling. Turn this on for PCRE/Ruby extended (/x) mode, where a raw space is ignored. Default: false." },
                    "string_literal": { "type": "boolean", "default": false, "description": "Also escape the result for pasting between the quotes of a source-code string literal: backslashes are doubled and double quotes escaped. Use for Java/C#/Go/JS string literals, not for raw strings. Default: false." }
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
