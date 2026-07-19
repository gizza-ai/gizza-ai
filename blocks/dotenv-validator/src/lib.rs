//! gizza-ai/dotenv-validator — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    env: String,
    /// Comma-separated variable names supplied externally (shell/CI); not
    /// flagged as missing interpolation references.
    #[serde(default)]
    allow_undefined: String,
    /// Validate `${VAR}`/`$VAR` interpolation references (default true).
    #[serde(default = "default_true")]
    check_interpolation: bool,
    /// Flag unquoted values that contain a space (default true).
    #[serde(default = "default_true")]
    require_quotes_for_spaces: bool,
    /// Output form: report | json.
    #[serde(default)]
    output: String,
}

fn default_true() -> bool {
    true
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("env")
                .required()
                .describe("The .env file contents to lint (KEY=VALUE lines; # comments, blank lines, quotes and an 'export ' prefix are understood). Nothing is uploaded — the file is only read to report issues."),
        )
        .param(
            Param::string("allow_undefined")
                .describe("Comma-separated variable names that are provided externally (from the shell or CI, e.g. 'HOME,PATH,CI') so references to them via ${VAR} are NOT reported as undefined. Matched case-sensitively. Default none."),
        )
        .param(
            Param::boolean("check_interpolation")
                .default(true)
                .describe("Validate ${VAR}/$VAR interpolation: flag unclosed '${', empty '${}', and references to variables not defined in the file. Default true."),
        )
        .param(
            Param::boolean("require_quotes_for_spaces")
                .default(true)
                .describe("Flag unquoted values that contain a space (most loaders keep only the first word unless the value is quoted). Default true."),
        )
        .param(
            Param::enumv("output", ["report", "json"])
                .default("report")
                .describe("Output form: 'report' (default) a human-readable list of issues with line numbers and severities, or 'json' a structured {ok, keys, error_count, warning_count, issues[]} object for CI."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct DotenvValidator;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/dotenv-validator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Lint a .env file for duplicate keys, unquoted spaces and bad interpolation.",
    skill(
        description = "Lint a .env (dotenv) file and report problems without rewriting it. Pass the file text in `env`; the tool parses KEY=VALUE lines (handling # comments, blank lines, single/double quotes, inline comments and an 'export ' prefix) and flags: duplicate keys (last value wins), unquoted values containing spaces, malformed ${VAR} interpolation (unclosed '${' or empty '${}'), and references to variables that are never defined in the file — plus invalid/lowercase key names, whitespace around '=', empty values, unterminated quotes and CRLF line endings. Each issue carries a line number, a severity (error/warning) and a rule name. Set `allow_undefined` to a comma-separated list of externally-provided variables (shell/CI) so their references are not reported. Turn off `check_interpolation` or `require_quotes_for_spaces` to silence those rule groups. `output` selects 'report' (default, human-readable) or 'json' (structured, for CI).",
        parameters = schema_json()
    ),
)]
impl DotenvValidator {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "dotenv-validator", |a: Args| {
            gizza_ai_dotenv_validator_core::validate(
                &a.env,
                &a.allow_undefined,
                a.check_interpolation,
                a.require_quotes_for_spaces,
                &a.output,
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
    /// schema, so any future change to the LLM-facing API is intentional and
    /// reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "env": { "type": "string", "description": "The .env file contents to lint (KEY=VALUE lines; # comments, blank lines, quotes and an 'export ' prefix are understood). Nothing is uploaded — the file is only read to report issues." },
                    "allow_undefined": { "type": "string", "description": "Comma-separated variable names that are provided externally (from the shell or CI, e.g. 'HOME,PATH,CI') so references to them via ${VAR} are NOT reported as undefined. Matched case-sensitively. Default none." },
                    "check_interpolation": { "type": "boolean", "default": true, "description": "Validate ${VAR}/$VAR interpolation: flag unclosed '${', empty '${}', and references to variables not defined in the file. Default true." },
                    "require_quotes_for_spaces": { "type": "boolean", "default": true, "description": "Flag unquoted values that contain a space (most loaders keep only the first word unless the value is quoted). Default true." },
                    "output": { "type": "string", "enum": ["report", "json"], "default": "report", "description": "Output form: 'report' (default) a human-readable list of issues with line numbers and severities, or 'json' a structured {ok, keys, error_count, warning_count, issues[]} object for CI." }
                },
                "required": ["env"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
