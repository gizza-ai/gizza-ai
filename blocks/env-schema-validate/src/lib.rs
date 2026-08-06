//! gizza-ai/env-schema-validate — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    env: String,
    schema: String,
    /// Schema dialect: auto | rules | json-schema | example.
    #[serde(default)]
    schema_format: String,
    /// What to do with keys present in the .env but absent from the schema.
    #[serde(default)]
    unknown_keys: String,
    /// Treat `KEY=` as not set (default true).
    #[serde(default = "default_true")]
    empty_is_missing: bool,
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
                .describe("The .env file contents to check (KEY=VALUE lines; # comments, blank lines, quotes, inline comments and an 'export ' prefix are understood; duplicate keys are last-wins). Nothing is uploaded — the file is only read to produce the report."),
        )
        .param(
            Param::string("schema")
                .required()
                .describe("The schema the .env must satisfy. Rules dialect: one 'KEY=rule|rule:arg' line per variable, e.g. 'PORT=required|port', 'NODE_ENV=required|enum:development,production', 'API_KEY=required|min:32', 'DB_PASSWORD=required|secure', 'RELEASE=pattern:^v[0-9]+$', 'TIMEOUT=number|min:1|max:60|default:30'. Rules: required, optional, string, number, integer, boolean, port, url, email, host, json, enum:a,b, min:N, max:N (value bound for numeric types, character-length bound otherwise), pattern:REGEX, secure, default:VALUE (documentation only). A bare 'KEY=' line just means required. JSON Schema is accepted too (see schema_format)."),
        )
        .param(
            Param::enumv("schema_format", ["auto", "rules", "json-schema", "example"])
                .default("auto")
                .describe("How to read `schema`: 'auto' (default — text starting with '{' is JSON Schema, anything else is the rules dialect), 'rules', 'json-schema' (a JSON Schema object using type/required/properties with enum, minimum/maximum, minLength/maxLength, pattern, format uri|email|hostname and default), or 'example' (paste a .env.example — every key it lists becomes required and its placeholder values are ignored)."),
        )
        .param(
            Param::enumv("unknown_keys", ["warn", "ignore", "error"])
                .default("warn")
                .describe("What to do with keys set in the .env but not declared in the schema: 'warn' (default, listed as a warning), 'ignore' (say nothing), or 'error' (strict mode — an undeclared key fails the check)."),
        )
        .param(
            Param::boolean("empty_is_missing")
                .default(true)
                .describe("Treat an empty value ('KEY=') as not set, so a required key with a blank value is reported as missing. Turn off to validate the empty string against the declared type and rules instead. Default true."),
        )
        .param(
            Param::enumv("output", ["report", "json"])
                .default("report")
                .describe("Output form: 'report' (default) a human-readable PASSED/FAILED summary plus one line per issue with the .env line number, severity and rule, or 'json' a structured {ok, declared_keys, file_keys, error_count, warning_count, issues[]} object for CI."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct EnvSchemaValidate;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/env-schema-validate",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Validate a .env file against a schema of required keys, types and allowed values.",
    skill(
        description = "Check a .env (dotenv) file against a DECLARED schema and report every variable that is missing, mistyped or outside its allowed values — before the app starts and fails with a confusing runtime error. Pass the file text in `env` and the schema in `schema`. The rules dialect is one 'KEY=rule|rule:arg' line per variable: required/optional; the types string, number, integer, boolean, port, url, email, host and json; enum:a,b,c for allowed values; min:N/max:N (numeric bound for numeric types, character-length bound otherwise); pattern:REGEX; secure (mixed-case + digit + symbol, 8+ chars); and default:VALUE (documentation only — a missing key that documents a default is a warning, not an error). A bare 'KEY=' line means required, so a .env.example pastes straight in (or set schema_format='example' to treat every key of a template as required). A JSON Schema object is accepted as well (type/required/properties with enum, minimum/maximum, minLength/maxLength, pattern, format and default) — schema_format='auto' detects it. `unknown_keys` controls whether keys set in the file but absent from the schema warn (default), are ignored, or fail. `empty_is_missing` (default true) treats 'KEY=' as not set. `output` selects 'report' (default, human-readable with line numbers and severities) or 'json' (structured, for CI). Values of secret-looking keys (SECRET/TOKEN/PASSWORD/KEY/AUTH) are masked in the report. Runs locally; nothing is uploaded.",
        parameters = schema_json()
    ),
)]
impl EnvSchemaValidate {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "env-schema-validate", |a: Args| {
            gizza_ai_env_schema_validate_core::validate(
                &a.env,
                &a.schema,
                &a.schema_format,
                &a.unknown_keys,
                a.empty_is_missing,
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
                    "env": { "type": "string", "description": "The .env file contents to check (KEY=VALUE lines; # comments, blank lines, quotes, inline comments and an 'export ' prefix are understood; duplicate keys are last-wins). Nothing is uploaded — the file is only read to produce the report." },
                    "schema": { "type": "string", "description": "The schema the .env must satisfy. Rules dialect: one 'KEY=rule|rule:arg' line per variable, e.g. 'PORT=required|port', 'NODE_ENV=required|enum:development,production', 'API_KEY=required|min:32', 'DB_PASSWORD=required|secure', 'RELEASE=pattern:^v[0-9]+$', 'TIMEOUT=number|min:1|max:60|default:30'. Rules: required, optional, string, number, integer, boolean, port, url, email, host, json, enum:a,b, min:N, max:N (value bound for numeric types, character-length bound otherwise), pattern:REGEX, secure, default:VALUE (documentation only). A bare 'KEY=' line just means required. JSON Schema is accepted too (see schema_format)." },
                    "schema_format": { "type": "string", "enum": ["auto", "rules", "json-schema", "example"], "default": "auto", "description": "How to read `schema`: 'auto' (default — text starting with '{' is JSON Schema, anything else is the rules dialect), 'rules', 'json-schema' (a JSON Schema object using type/required/properties with enum, minimum/maximum, minLength/maxLength, pattern, format uri|email|hostname and default), or 'example' (paste a .env.example — every key it lists becomes required and its placeholder values are ignored)." },
                    "unknown_keys": { "type": "string", "enum": ["warn", "ignore", "error"], "default": "warn", "description": "What to do with keys set in the .env but not declared in the schema: 'warn' (default, listed as a warning), 'ignore' (say nothing), or 'error' (strict mode — an undeclared key fails the check)." },
                    "empty_is_missing": { "type": "boolean", "default": true, "description": "Treat an empty value ('KEY=') as not set, so a required key with a blank value is reported as missing. Turn off to validate the empty string against the declared type and rules instead. Default true." },
                    "output": { "type": "string", "enum": ["report", "json"], "default": "report", "description": "Output form: 'report' (default) a human-readable PASSED/FAILED summary plus one line per issue with the .env line number, severity and rule, or 'json' a structured {ok, declared_keys, file_keys, error_count, warning_count, issues[]} object for CI." }
                },
                "required": ["env", "schema"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored);
    }

    /// Every param must carry a description an LLM/CLI user can act on.
    #[test]
    fn every_param_is_described() {
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        for (name, prop) in derived["properties"].as_object().unwrap() {
            let desc = prop["description"].as_str().unwrap_or_default();
            assert!(desc.len() > 30, "param '{name}' needs a real description");
        }
    }
}
