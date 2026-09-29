//! gizza-ai/docker-compose-validator — Compose-aware validator on the shared tool
//! abstraction. The chat schema is single-sourced from descriptor() (which also
//! drives the CLI); handle() delegates to block_utils::run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_preset")]
    preset: String,
    #[serde(default)]
    disable: String,
    #[serde(default)]
    strict_warnings: bool,
    #[serde(default = "default_min_severity")]
    min_severity: String,
    #[serde(default = "default_report_format")]
    report_format: String,
}

fn default_preset() -> String {
    "default".to_string()
}
fn default_min_severity() -> String {
    "hint".to_string()
}
fn default_report_format() -> String {
    "report".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("input").required().describe("The docker-compose.yml (or compose.yaml) document to validate. Only the first YAML document is analysed and nothing leaves the local runtime."))
        .param(Param::enumv("preset", ["essential", "default", "strict"]).default("default").describe("Rule set. essential reports only what breaks 'docker compose up' (syntax, undefined services/networks/volumes, port and volume syntax, dependency cycles, duplicate host ports). default adds deprecation, image-pinning and security warnings. strict adds hardening hints such as missing healthchecks, resource limits, logging options and unbound host ports."))
        .param(Param::string("disable").default("").describe("Comma, space or newline separated rule ids to switch off, such as image-tag, quote-ports or project-name. An unknown id is rejected rather than ignored."))
        .param(Param::boolean("strict_warnings").default(false).describe("Promote every warning to an error, matching the warnings-as-errors convention used in CI pipelines. Hints are left alone."))
        .param(Param::enumv("min_severity", ["hint", "warning", "error"]).default("hint").describe("Lowest severity to include in the output: hint shows everything, warning drops hints, error shows only blocking problems. Applied after strict_warnings."))
        .param(Param::enumv("report_format", ["report", "json"]).default("report").describe("Output format: report returns a human-readable summary plus line:column findings; json returns a machine-readable object with a valid flag, counts and a problems array for CI."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct DockerComposeValidator;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/docker-compose-validator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Validate docker-compose.yml structure, references and port syntax",
    skill(
        description = "Validate a docker-compose.yml beyond its YAML syntax: missing services/image/build keys, invalid short- and long-syntax port mappings, named volumes and networks that are referenced but never declared, depends_on targets that do not exist, dependency cycles, duplicate container names and duplicate published host ports, obsolete version fields, floating image tags, hard-coded secrets in environment, privileged and host-network services, and optional hardening hints. Anchors, aliases and merge keys are resolved first. Every finding carries a line and column, a rule id and what was expected. Choose essential/default/strict presets, disable individual rules, filter by severity, and return a readable report or JSON for CI.",
        parameters = schema_json()
    ),
)]
impl DockerComposeValidator {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "docker-compose-validator", |a: Args| {
            gizza_ai_docker_compose_validator_core::run(
                &a.input,
                &a.preset,
                &a.disable,
                a.strict_warnings,
                &a.min_severity,
                &a.report_format,
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

    /// Drift guard: the descriptor is the single source for the chat schema, the
    /// CLI and the page form, so pin exactly what it emits.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let authored: serde_json::Value = serde_json::json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["input"],
            "properties": {
                "input": {
                    "type": "string",
                    "description": "The docker-compose.yml (or compose.yaml) document to validate. Only the first YAML document is analysed and nothing leaves the local runtime."
                },
                "preset": {
                    "type": "string",
                    "enum": ["essential", "default", "strict"],
                    "default": "default",
                    "description": "Rule set. essential reports only what breaks 'docker compose up' (syntax, undefined services/networks/volumes, port and volume syntax, dependency cycles, duplicate host ports). default adds deprecation, image-pinning and security warnings. strict adds hardening hints such as missing healthchecks, resource limits, logging options and unbound host ports."
                },
                "disable": {
                    "type": "string",
                    "default": "",
                    "description": "Comma, space or newline separated rule ids to switch off, such as image-tag, quote-ports or project-name. An unknown id is rejected rather than ignored."
                },
                "strict_warnings": {
                    "type": "boolean",
                    "default": false,
                    "description": "Promote every warning to an error, matching the warnings-as-errors convention used in CI pipelines. Hints are left alone."
                },
                "min_severity": {
                    "type": "string",
                    "enum": ["hint", "warning", "error"],
                    "default": "hint",
                    "description": "Lowest severity to include in the output: hint shows everything, warning drops hints, error shows only blocking problems. Applied after strict_warnings."
                },
                "report_format": {
                    "type": "string",
                    "enum": ["report", "json"],
                    "default": "report",
                    "description": "Output format: report returns a human-readable summary plus line:column findings; json returns a machine-readable object with a valid flag, counts and a problems array for CI."
                }
            }
        });
        assert_eq!(derived, authored);
    }

    #[test]
    fn every_param_documents_itself() {
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = schema["properties"].as_object().unwrap();
        assert_eq!(props.len(), 6);
        for (name, spec) in props {
            let d = spec["description"].as_str().unwrap_or_default();
            assert!(d.len() > 40, "{name} needs a usable description");
        }
    }
}
