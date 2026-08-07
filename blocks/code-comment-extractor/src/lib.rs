//! gizza-ai/code-comment-extractor — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the CLI);
//! handle() delegates to block_utils::run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_code_comment_extractor_core::extract;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    code: String,
    #[serde(default = "default_auto")]
    language: String,
    #[serde(default = "default_comments")]
    output: String,
    #[serde(default = "default_all")]
    kind: String,
    #[serde(default = "default_true")]
    strip_markers: bool,
    #[serde(default)]
    line_numbers: bool,
    #[serde(default)]
    min_length: i64,
    #[serde(default = "default_true")]
    docstrings: bool,
}
fn default_auto() -> String {
    "auto".to_string()
}
fn default_comments() -> String {
    "comments".to_string()
}
fn default_all() -> String {
    "all".to_string()
}
fn default_true() -> bool {
    true
}

/// Every value the `language` param accepts — shared by the descriptor so the
/// chat schema, the CLI and the page `<select>` can never drift apart.
const LANGUAGES: [&str; 18] = [
    "auto",
    "javascript",
    "typescript",
    "python",
    "java",
    "csharp",
    "c",
    "cpp",
    "go",
    "rust",
    "php",
    "ruby",
    "shell",
    "sql",
    "html",
    "css",
    "lua",
    "yaml",
];

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("code")
                .required()
                .describe("The source code to scan, pasted as text. Any length; comment markers inside string and character literals are ignored, so `const u = \"https://x/y\"` is not read as a comment. Example: 'const a = 1; // set the counter'."),
        )
        .param(
            Param::enumv("language", LANGUAGES)
                .default("auto")
                .describe("Comment syntax to use. 'auto' (default) guesses from the code's shape. Named values: javascript, typescript, python, java, csharp, c, cpp, go, rust, php, ruby, shell, sql, html, css, lua, yaml. Pick one when auto-detection guesses wrong — e.g. force 'sql' so `--` is a comment rather than a Lua/SQL toss-up."),
        )
        .param(
            Param::enumv("output", ["comments", "stripped", "json", "markdown", "stats"])
                .default("comments")
                .describe("What to return. 'comments' (default) = the comment text, one per line. 'stripped' = the original source with the matched comments removed (blank lines kept so line numbers do not shift). 'json' = an array of {line, column, end_line, kind, text}. 'markdown' = a | Line | Kind | Comment | table. 'stats' = detected language plus comment/code/blank line counts and comment density."),
        )
        .param(
            Param::enumv("kind", ["all", "line", "block", "doc"])
                .default("all")
                .describe("Which comment kinds to keep. 'all' (default); 'line' = single-line comments only (// # -- …); 'block' = delimited comments only (/* */, <!-- -->, --[[ ]], =begin/=end); 'doc' = documentation comments only (/** */, ///, //!, ##, Python docstrings). With output='stripped' this selects what gets REMOVED — kind='line' strips line comments and keeps doc blocks."),
        )
        .param(
            Param::boolean("strip_markers")
                .default(true)
                .describe("Remove the comment delimiters from the reported text, so `// note` becomes 'note' and a Javadoc block loses its leading `*` on each line. Default true; set false to get each comment verbatim, markers included. Ignored by output='stripped'."),
        )
        .param(
            Param::boolean("line_numbers")
                .default(false)
                .describe("Prefix each entry of the plain 'comments' list with its 1-based source line, as '[L12] note'. Default false. The json and markdown outputs always carry line numbers, so this only affects output='comments'."),
        )
        .param(
            Param::integer("min_length")
                .default(0)
                .min(0.0)
                .describe("Drop comments whose text is shorter than this many characters, measured after strip_markers is applied. Default 0 (keep everything); 5 is a good value for filtering noise like '// x' or a commented-out brace."),
        )
        .param(
            Param::boolean("docstrings")
                .default(true)
                .describe("Treat a Python triple-quoted string that starts its own line (a module/class/function docstring) as a doc comment. Default true. Set false to report only real `#` comments; Python has no block-comment syntax, so with this off a docstring-only file returns nothing. Has no effect on other languages."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct CodeCommentExtractor;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/code-comment-extractor",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract every comment from pasted source code, or strip them out",
    skill(
        description = "Pull the comments out of pasted source code — line, block and documentation comments — or return the same source with them removed. Covers 17 comment syntaxes (javascript, typescript, python, java, csharp, c, cpp, go, rust, php, ruby, shell, sql, html, css, lua, yaml) plus 'auto' detection. A string/char-literal-aware tokenizer means a `//` or `#` inside a string is never mistaken for a comment, and Go backtick strings, Rust raw strings and Python triple-quoted strings are handled. Each comment is classified as line, block or doc (/** */, ///, //!, ##, Python docstrings) and carries its line, column and end line. Options: language, output (comments list, stripped source, json, markdown table, or stats with comment density), kind filter, strip_markers, line_numbers, min_length, docstrings. This is a tokenizer, not a full parser: an unterminated /* runs to the end of input, and output is capped at 50,000 comments.",
        parameters = schema_json()
    ),
)]
impl CodeCommentExtractor {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "code-comment-extractor", |a: Args| {
            extract(
                &a.code,
                &a.language,
                &a.output,
                &a.kind,
                a.strip_markers,
                a.line_numbers,
                a.min_length,
                a.docstrings,
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
                    "code":          { "type": "string", "description": "The source code to scan, pasted as text. Any length; comment markers inside string and character literals are ignored, so `const u = \"https://x/y\"` is not read as a comment. Example: 'const a = 1; // set the counter'." },
                    "language":      { "type": "string", "enum": ["auto", "javascript", "typescript", "python", "java", "csharp", "c", "cpp", "go", "rust", "php", "ruby", "shell", "sql", "html", "css", "lua", "yaml"], "default": "auto", "description": "Comment syntax to use. 'auto' (default) guesses from the code's shape. Named values: javascript, typescript, python, java, csharp, c, cpp, go, rust, php, ruby, shell, sql, html, css, lua, yaml. Pick one when auto-detection guesses wrong — e.g. force 'sql' so `--` is a comment rather than a Lua/SQL toss-up." },
                    "output":        { "type": "string", "enum": ["comments", "stripped", "json", "markdown", "stats"], "default": "comments", "description": "What to return. 'comments' (default) = the comment text, one per line. 'stripped' = the original source with the matched comments removed (blank lines kept so line numbers do not shift). 'json' = an array of {line, column, end_line, kind, text}. 'markdown' = a | Line | Kind | Comment | table. 'stats' = detected language plus comment/code/blank line counts and comment density." },
                    "kind":          { "type": "string", "enum": ["all", "line", "block", "doc"], "default": "all", "description": "Which comment kinds to keep. 'all' (default); 'line' = single-line comments only (// # -- …); 'block' = delimited comments only (/* */, <!-- -->, --[[ ]], =begin/=end); 'doc' = documentation comments only (/** */, ///, //!, ##, Python docstrings). With output='stripped' this selects what gets REMOVED — kind='line' strips line comments and keeps doc blocks." },
                    "strip_markers": { "type": "boolean", "default": true, "description": "Remove the comment delimiters from the reported text, so `// note` becomes 'note' and a Javadoc block loses its leading `*` on each line. Default true; set false to get each comment verbatim, markers included. Ignored by output='stripped'." },
                    "line_numbers":  { "type": "boolean", "default": false, "description": "Prefix each entry of the plain 'comments' list with its 1-based source line, as '[L12] note'. Default false. The json and markdown outputs always carry line numbers, so this only affects output='comments'." },
                    "min_length":    { "type": "integer", "default": 0, "minimum": 0, "description": "Drop comments whose text is shorter than this many characters, measured after strip_markers is applied. Default 0 (keep everything); 5 is a good value for filtering noise like '// x' or a commented-out brace." },
                    "docstrings":    { "type": "boolean", "default": true, "description": "Treat a Python triple-quoted string that starts its own line (a module/class/function docstring) as a doc comment. Default true. Set false to report only real `#` comments; Python has no block-comment syntax, so with this off a docstring-only file returns nothing. Has no effect on other languages." }
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
