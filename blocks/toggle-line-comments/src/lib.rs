//! gizza-ai/toggle-line-comments — chat skill block on the shared tool abstraction.
//! Comments or uncomments a block of code using the correct line-comment syntax
//! for its language (`//`, `#`, `--`, `;`, `%`, `'`, `REM`, or a `<!-- -->` /
//! `/* */` pair for markup). The chat schema is single-sourced from descriptor()
//! (which also drives the CLI); handle() delegates to run_skill.
//! Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_toggle_line_comments_core::{toggle, Align, Mode};
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    code: String,
    #[serde(default = "default_language")]
    language: String,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default)]
    marker: String,
    #[serde(default = "default_true")]
    space_after_marker: bool,
    #[serde(default = "default_align")]
    align: String,
    #[serde(default)]
    comment_blank_lines: bool,
}

fn default_language() -> String {
    "auto".to_string()
}
fn default_mode() -> String {
    "toggle".to_string()
}
fn default_align() -> String {
    "indent".to_string()
}
fn default_true() -> bool {
    true
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("code")
                .required()
                .describe("The block of code to comment or uncomment, pasted as text. Lines are split on newlines and indentation is preserved. Example: 'const a = 1;\\nconst b = 2;'."),
        )
        .param(
            Param::enumv(
                "language",
                [
                    "auto", "javascript", "typescript", "java", "csharp", "c", "cpp", "go", "rust",
                    "swift", "kotlin", "scala", "php", "python", "ruby", "perl", "shell",
                    "powershell", "yaml", "toml", "r", "dockerfile", "makefile", "sql", "lua",
                    "haskell", "ini", "clojure", "latex", "vb", "batch", "css", "html", "xml",
                ],
            )
            .default("auto")
            .describe("Which language's line-comment syntax to use. 'auto' (default) guesses from a shebang, an existing comment marker, or distinctive keywords. Markers by family: // (javascript, typescript, java, csharp, c, cpp, go, rust, swift, kotlin, scala, php), # (python, ruby, perl, shell, powershell, yaml, toml, r, dockerfile, makefile), -- (sql, lua, haskell), ; (ini), ;; (clojure), % (latex), ' (vb), REM (batch). css wraps each line in /* */ and html/xml in <!-- --> because they have no line comment. Name the language when auto guesses wrong."),
        )
        .param(
            Param::enumv("mode", ["toggle", "comment", "uncomment"])
                .default("toggle")
                .describe("What to do. 'toggle' (default) uncomments when EVERY considered line is already commented and otherwise comments the whole block — the same rule an editor's Ctrl+/ uses. 'comment' always adds markers; 'uncomment' always removes them and leaves lines that have none untouched."),
        )
        .param(
            Param::string("marker")
                .default("")
                .describe("An explicit comment marker that overrides the language's own, for a syntax the language list does not cover — e.g. '//' , '#', '--' or '@'. Must contain no whitespace. Default empty (use the language's marker). A custom marker is always treated as a line comment, never a pair."),
        )
        .param(
            Param::boolean("space_after_marker")
                .default(true)
                .describe("Write '// code' rather than '//code'. Default true, which is what most linters and formatters expect. Uncommenting removes at most one such space, so deliberate indentation inside a comment survives a round trip."),
        )
        .param(
            Param::enumv("align", ["indent", "column0"])
                .default("indent")
                .describe("Where the marker goes when commenting. 'indent' (default) puts it at the block's SHALLOWEST indentation so the code keeps its relative shape. 'column0' puts every marker flush against the left margin. Ignored when uncommenting, which always keeps the original indentation."),
        )
        .param(
            Param::boolean("comment_blank_lines")
                .default(false)
                .describe("Also mark blank and whitespace-only lines, which are otherwise passed through untouched. Default false (editor behavior). When true a blank line becomes the bare marker with no trailing space, so no trailing whitespace is introduced."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/toggle-line-comments",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Comment or uncomment a block of code in the right syntax for its language",
    skill(
        description = "Comment or uncomment a block of code using the correct line-comment syntax for its language — the job an editor's Ctrl+/ does, for pasted code. Covers 33 languages plus 'auto' detection: // (javascript, typescript, java, csharp, c, cpp, go, rust, swift, kotlin, scala, php), # (python, ruby, perl, shell, powershell, yaml, toml, r, dockerfile, makefile), -- (sql, lua, haskell), ; (ini), ;; (clojure), % (latex), ' (vb), REM (batch), plus /* */ for css and <!-- --> for html/xml, which have no line comment. mode='toggle' (default) uncomments when every considered line already carries the marker and otherwise comments the whole block, so the operation is reversible; 'comment' and 'uncomment' force a direction. align='indent' (default) inserts the marker at the block's shallowest indentation to keep relative indentation intact, 'column0' puts it flush left. space_after_marker (default true) writes '// code'; comment_blank_lines (default false) also marks blank lines. marker overrides the language's syntax for anything exotic. Returns the transformed code plus the action taken, the resolved language, the marker used, and total/changed line counts. This is a lexical transform, not a parser: a marker inside a string literal is not recognized, and input is capped at 2,000,000 characters. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "toggle-line-comments", |a: Args| {
            let mode = Mode::parse(&a.mode).map_err(SkillError::InvalidArgs)?;
            let align = Align::parse(&a.align).map_err(SkillError::InvalidArgs)?;
            toggle(
                &a.code,
                &a.language,
                mode,
                &a.marker,
                a.space_after_marker,
                align,
                a.comment_blank_lines,
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

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "code": { "type": "string", "description": "The block of code to comment or uncomment, pasted as text. Lines are split on newlines and indentation is preserved. Example: 'const a = 1;\\nconst b = 2;'." },
                    "language": {
                        "type": "string",
                        "enum": ["auto", "javascript", "typescript", "java", "csharp", "c", "cpp", "go", "rust", "swift", "kotlin", "scala", "php", "python", "ruby", "perl", "shell", "powershell", "yaml", "toml", "r", "dockerfile", "makefile", "sql", "lua", "haskell", "ini", "clojure", "latex", "vb", "batch", "css", "html", "xml"],
                        "default": "auto",
                        "description": "Which language's line-comment syntax to use. 'auto' (default) guesses from a shebang, an existing comment marker, or distinctive keywords. Markers by family: // (javascript, typescript, java, csharp, c, cpp, go, rust, swift, kotlin, scala, php), # (python, ruby, perl, shell, powershell, yaml, toml, r, dockerfile, makefile), -- (sql, lua, haskell), ; (ini), ;; (clojure), % (latex), ' (vb), REM (batch). css wraps each line in /* */ and html/xml in <!-- --> because they have no line comment. Name the language when auto guesses wrong."
                    },
                    "mode": {
                        "type": "string",
                        "enum": ["toggle", "comment", "uncomment"],
                        "default": "toggle",
                        "description": "What to do. 'toggle' (default) uncomments when EVERY considered line is already commented and otherwise comments the whole block — the same rule an editor's Ctrl+/ uses. 'comment' always adds markers; 'uncomment' always removes them and leaves lines that have none untouched."
                    },
                    "marker": { "type": "string", "default": "", "description": "An explicit comment marker that overrides the language's own, for a syntax the language list does not cover — e.g. '//' , '#', '--' or '@'. Must contain no whitespace. Default empty (use the language's marker). A custom marker is always treated as a line comment, never a pair." },
                    "space_after_marker": { "type": "boolean", "default": true, "description": "Write '// code' rather than '//code'. Default true, which is what most linters and formatters expect. Uncommenting removes at most one such space, so deliberate indentation inside a comment survives a round trip." },
                    "align": {
                        "type": "string",
                        "enum": ["indent", "column0"],
                        "default": "indent",
                        "description": "Where the marker goes when commenting. 'indent' (default) puts it at the block's SHALLOWEST indentation so the code keeps its relative shape. 'column0' puts every marker flush against the left margin. Ignored when uncommenting, which always keeps the original indentation."
                    },
                    "comment_blank_lines": { "type": "boolean", "default": false, "description": "Also mark blank and whitespace-only lines, which are otherwise passed through untouched. Default false (editor behavior). When true a blank line becomes the bare marker with no trailing space, so no trailing whitespace is introduced." }
                },
                "required": ["code"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    /// The descriptor's advertised language list must stay in step with the
    /// core's profile table — a name in one and not the other is a broken enum.
    #[test]
    fn descriptor_languages_match_the_core_profiles() {
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let advertised: Vec<String> = schema["properties"]["language"]["enum"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().to_string())
            .collect();
        let mut from_core = gizza_ai_toggle_line_comments_core::language_names();
        from_core.sort_unstable();
        let mut sorted = advertised.clone();
        sorted.sort();
        assert_eq!(sorted, from_core, "descriptor enum vs core language table");
    }
}
