//! gizza-ai/shell-script-linter — chat skill block on the shared tool abstraction.
//!
//! Lints a pasted bash/sh script for common pitfalls — unquoted expansions, useless
//! cat, missing `set -euo pipefail`, subshell scope traps and friends — without ever
//! executing it. The chat schema is single-sourced from descriptor(), which also
//! drives the CLI.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    script: String,
    #[serde(default)]
    shell: String,
    #[serde(default)]
    min_severity: String,
    #[serde(default)]
    ignore: String,
    #[serde(default)]
    format: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("script")
                .required()
                .describe("Shell script text to lint, up to 200000 bytes. Comments, single-quoted strings and here-doc bodies are masked first, so examples inside a comment do not produce findings. The script is never executed."),
        )
        .param(
            Param::enumv("shell", ["auto", "bash", "sh", "dash", "zsh"])
                .default("auto")
                .describe("Which shell to lint for. 'auto' reads the shebang and falls back to bash. 'sh' and 'dash' enable the SH-BASHISM rule and skip the bash-only LEGACY-TEST hint; 'bash' and 'zsh' do the opposite."),
        )
        .param(
            Param::enumv("min_severity", ["all", "warning", "error"])
                .default("all")
                .describe("Minimum severity to report. 'all' includes info-level style hints, 'warning' hides them, 'error' shows only structural problems such as unclosed blocks, spaced assignments and risky rm -rf."),
        )
        .param(
            Param::string("ignore")
                .default("")
                .describe("Optional comma- or space-separated rule codes to suppress, e.g. 'LEGACY-TEST, USELESS-CAT'. Valid codes: SYNTAX, MISSING-SHEBANG, STRICT-MODE, UNQUOTED-VAR, USELESS-CAT, BACKTICKS, SUBSHELL-SCOPE, UNCHECKED-CD, PARSE-LS, ASSIGN-SPACES, LEGACY-TEST, RM-RISK, SH-BASHISM."),
        )
        .param(
            Param::enumv("format", ["text", "json"])
                .default("text")
                .describe("Output format. 'text' is a human-readable report with line numbers, severities and the offending source line; 'json' returns the resolved shell, summary counts and a findings array for CI."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ShellScriptLinter;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/shell-script-linter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Lint a bash or sh script for unquoted variables, useless cat, missing set -euo pipefail, and subshell scope traps.",
    skill(
        description = "Lint a pasted bash/sh script without running it. Reports block-structure errors (unclosed if/do/case, unterminated quotes) plus common pitfalls: unquoted expansions that word-split, useless use of cat, legacy backticks, missing set -e/-u/-o pipefail, pipe-to-while subshell scope loss, unguarded cd, parsing ls output, spaced assignments, single-bracket tests, risky rm -rf on an interpolated path, and bashisms under a POSIX sh shebang. Parameters: script text, shell (auto/bash/sh/dash/zsh), min_severity filter, ignore list of rule codes, and text/json output.",
        parameters = schema_json()
    ),
)]
impl ShellScriptLinter {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "shell-script-linter", |a: Args| {
            gizza_ai_shell_script_linter_core::lint(
                &a.script,
                &a.shell,
                &a.min_severity,
                &a.ignore,
                &a.format,
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
                    "script": { "type": "string", "description": "Shell script text to lint, up to 200000 bytes. Comments, single-quoted strings and here-doc bodies are masked first, so examples inside a comment do not produce findings. The script is never executed." },
                    "shell": { "type": "string", "enum": ["auto", "bash", "sh", "dash", "zsh"], "default": "auto", "description": "Which shell to lint for. 'auto' reads the shebang and falls back to bash. 'sh' and 'dash' enable the SH-BASHISM rule and skip the bash-only LEGACY-TEST hint; 'bash' and 'zsh' do the opposite." },
                    "min_severity": { "type": "string", "enum": ["all", "warning", "error"], "default": "all", "description": "Minimum severity to report. 'all' includes info-level style hints, 'warning' hides them, 'error' shows only structural problems such as unclosed blocks, spaced assignments and risky rm -rf." },
                    "ignore": { "type": "string", "default": "", "description": "Optional comma- or space-separated rule codes to suppress, e.g. 'LEGACY-TEST, USELESS-CAT'. Valid codes: SYNTAX, MISSING-SHEBANG, STRICT-MODE, UNQUOTED-VAR, USELESS-CAT, BACKTICKS, SUBSHELL-SCOPE, UNCHECKED-CD, PARSE-LS, ASSIGN-SPACES, LEGACY-TEST, RM-RISK, SH-BASHISM." },
                    "format": { "type": "string", "enum": ["text", "json"], "default": "text", "description": "Output format. 'text' is a human-readable report with line numbers, severities and the offending source line; 'json' returns the resolved shell, summary counts and a findings array for CI." }
                },
                "required": ["script"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
