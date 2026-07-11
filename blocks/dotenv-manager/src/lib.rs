//! gizza-ai/dotenv-manager — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    env: String,
    /// Optional overlay `.env` whose keys override the primary file.
    #[serde(default)]
    merge: String,
    /// Comma-separated key names that must be present in the merged result.
    #[serde(default)]
    required_keys: String,
    /// Mask values of sensitive-looking keys (default true).
    #[serde(default = "default_true")]
    mask_secrets: bool,
    /// Emit keys alphabetically (default false).
    #[serde(default)]
    sort_keys: bool,
    /// Output form: report | normalized | example | json.
    #[serde(default)]
    output: String,
}

fn default_true() -> bool {
    true
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("env")
                .required()
                .describe("The primary .env file contents (KEY=VALUE lines; # comments, blank lines, quotes and an 'export ' prefix are handled)."),
        )
        .param(
            Param::string("merge")
                .describe("Optional second .env file to overlay; its keys OVERRIDE matching keys from `env` (last-file-wins). Leave blank to validate a single file."),
        )
        .param(
            Param::string("required_keys")
                .describe("Comma-separated key names that must be present after merging (e.g. 'DATABASE_URL,API_KEY'). Any missing ones are reported. Matched case-sensitively. Default none."),
        )
        .param(
            Param::boolean("mask_secrets")
                .default(true)
                .describe("Mask values of sensitive-looking keys (names containing SECRET, TOKEN, PASSWORD, KEY, AUTH, etc.) in every value-bearing output. Default true."),
        )
        .param(
            Param::boolean("sort_keys")
                .default(false)
                .describe("Emit keys in alphabetical order instead of first-seen order. Default false."),
        )
        .param(
            Param::enumv("output", ["report", "normalized", "example", "json"])
                .default("report")
                .describe("Output form: 'report' (default) a diagnostic summary with duplicates/missing/lint warnings and masked values; 'normalized' deduped KEY=value lines (last value wins); 'example' a .env.example with blanked values; 'json' a JSON object of the merged pairs."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct DotenvManager;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/dotenv-manager",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Parse, validate, merge and secret-mask .env files.",
    skill(
        description = "Parse, validate, merge and secret-mask .env (dotenv) files. Pass the file text in `env`; the tool handles KEY=VALUE lines, # comments, blank lines, single/double quotes, inline comments and an 'export ' prefix. It flags duplicate keys (last value wins), keys missing from the comma-separated `required_keys` list, and lint issues (non-UPPER_SNAKE_CASE names, empty values, whitespace around '='). Set `merge` to a second .env to overlay it (its keys override — last-file-wins). `mask_secrets` (default true) masks values of sensitive-looking keys. `output` selects the shape: 'report' (default) a diagnostic summary; 'normalized' deduped KEY=value lines; 'example' a .env.example with blanked values; 'json' a JSON object of the merged pairs. Set `sort_keys` to emit keys alphabetically.",
        parameters = schema_json()
    ),
)]
impl DotenvManager {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "dotenv-manager", |a: Args| {
            gizza_ai_dotenv_manager_core::manage(
                &a.env,
                &a.merge,
                &a.required_keys,
                a.mask_secrets,
                a.sort_keys,
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
                    "env": { "type": "string", "description": "The primary .env file contents (KEY=VALUE lines; # comments, blank lines, quotes and an 'export ' prefix are handled)." },
                    "merge": { "type": "string", "description": "Optional second .env file to overlay; its keys OVERRIDE matching keys from `env` (last-file-wins). Leave blank to validate a single file." },
                    "required_keys": { "type": "string", "description": "Comma-separated key names that must be present after merging (e.g. 'DATABASE_URL,API_KEY'). Any missing ones are reported. Matched case-sensitively. Default none." },
                    "mask_secrets": { "type": "boolean", "default": true, "description": "Mask values of sensitive-looking keys (names containing SECRET, TOKEN, PASSWORD, KEY, AUTH, etc.) in every value-bearing output. Default true." },
                    "sort_keys": { "type": "boolean", "default": false, "description": "Emit keys in alphabetical order instead of first-seen order. Default false." },
                    "output": { "type": "string", "enum": ["report", "normalized", "example", "json"], "default": "report", "description": "Output form: 'report' (default) a diagnostic summary with duplicates/missing/lint warnings and masked values; 'normalized' deduped KEY=value lines (last value wins); 'example' a .env.example with blanked values; 'json' a JSON object of the merged pairs." }
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
