//! gizza-ai/dotenv-to-shell — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    /// The source text: a `.env` file (to-shell) or shell statements (to-env).
    input: String,
    /// Conversion direction: to-shell (default) | to-env.
    #[serde(default)]
    direction: String,
    /// Target shell dialect for to-shell: posix | bash | fish.
    #[serde(default)]
    shell: String,
    /// Value quoting for to-shell: auto | single.
    #[serde(default)]
    quote: String,
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The source text. For to-shell: a .env file (KEY=VALUE lines; # comments, blank lines, single/double quotes, inline comments and an 'export ' prefix are handled). For to-env: shell statements (export KEY=…, set -gx KEY …, setenv KEY …)."),
        )
        .param(
            Param::enumv("direction", ["to-shell", "to-env"])
                .default("to-shell")
                .describe("Conversion direction: 'to-shell' (default) turns a .env file into shell statements; 'to-env' parses shell export statements back into a plain .env file."),
        )
        .param(
            Param::enumv("shell", ["posix", "bash", "fish"])
                .default("posix")
                .describe("Target shell dialect for to-shell. 'posix' and 'bash' emit identical `export KEY=value` statements (also works in zsh/sh); 'fish' emits `set -gx KEY value`. Ignored for to-env (which auto-detects the input dialect)."),
        )
        .param(
            Param::enumv("quote", ["auto", "single"])
                .default("auto")
                .describe("Value quoting for to-shell. 'auto' (default) leaves safe values unquoted and single-quotes only values containing spaces or special characters; 'single' always single-quotes. Special characters ($, backtick, quotes, #, newlines) are always kept literal. Ignored for to-env."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct DotenvToShell;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/dotenv-to-shell",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert a .env file into export-prefixed shell statements, and back.",
    skill(
        description = "Convert a .env file into export-prefixed shell statements (and back), handling quoting and special characters. Pass the source text in `input`. With `direction=to-shell` (default) each KEY=VALUE line becomes a shell statement for the chosen `shell` dialect — 'posix'/'bash' emit `export KEY=value`, 'fish' emits `set -gx KEY value` — with values safely quoted so spaces, `$`, backticks, `#`, quotes and newlines stay literal when the output is sourced. `quote=auto` (default) leaves safe values unquoted and single-quotes the rest; `quote=single` always quotes. Full-line comments and blank lines are preserved, an inline `# comment` and an 'export ' prefix on input are stripped, and keys that aren't valid shell identifiers are skipped with a note. With `direction=to-env` it parses `export`/`set -gx`/`setenv` statements back into a plain .env file. Everything runs locally; nothing is uploaded.",
        parameters = schema_json()
    ),
)]
impl DotenvToShell {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "dotenv-to-shell", |a: Args| {
            gizza_ai_dotenv_to_shell_core::convert(&a.input, &a.direction, &a.shell, &a.quote)
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
    /// schema, so any future change to the LLM-facing API is intentional and
    /// reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "The source text. For to-shell: a .env file (KEY=VALUE lines; # comments, blank lines, single/double quotes, inline comments and an 'export ' prefix are handled). For to-env: shell statements (export KEY=…, set -gx KEY …, setenv KEY …)." },
                    "direction": { "type": "string", "enum": ["to-shell", "to-env"], "default": "to-shell", "description": "Conversion direction: 'to-shell' (default) turns a .env file into shell statements; 'to-env' parses shell export statements back into a plain .env file." },
                    "shell": { "type": "string", "enum": ["posix", "bash", "fish"], "default": "posix", "description": "Target shell dialect for to-shell. 'posix' and 'bash' emit identical `export KEY=value` statements (also works in zsh/sh); 'fish' emits `set -gx KEY value`. Ignored for to-env (which auto-detects the input dialect)." },
                    "quote": { "type": "string", "enum": ["auto", "single"], "default": "auto", "description": "Value quoting for to-shell. 'auto' (default) leaves safe values unquoted and single-quotes only values containing spaces or special characters; 'single' always single-quotes. Special characters ($, backtick, quotes, #, newlines) are always kept literal. Ignored for to-env." }
                },
                "required": ["input"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
