//! gizza-ai/env-file-merger — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    /// Lowest-priority layer — the base `.env`.
    layer1: String,
    /// Layers 2-4, each overriding the ones before it. Blank = absent.
    #[serde(default)]
    layer2: String,
    #[serde(default)]
    layer3: String,
    #[serde(default)]
    layer4: String,
    /// Comma-separated display names for the four slots.
    #[serde(default)]
    layer_names: String,
    /// Output form: report | env | json | shell | markdown | conflicts.
    #[serde(default)]
    output: String,
    /// Mask values of sensitive-looking keys (default true).
    #[serde(default = "default_true")]
    mask_secrets: bool,
    /// Emit keys alphabetically (default false).
    #[serde(default)]
    sort_keys: bool,
    /// Keep only keys starting with this prefix.
    #[serde(default)]
    prefix_filter: String,
    /// Resolve `${VAR}` references against the merged result (default false).
    #[serde(default)]
    expand_vars: bool,
}

fn default_true() -> bool {
    true
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("layer1")
                .required()
                .describe("The LOWEST-priority .env file's contents — the base file (conventionally .env). KEY=VALUE lines; # comments, blank lines, single/double quotes, inline comments and an 'export ' prefix are all handled."),
        )
        .param(
            Param::string("layer2")
                .describe("Second .env layer; its keys override layer1 (conventionally .env.local). Leave blank if this file doesn't exist — a blank layer is skipped, not an error."),
        )
        .param(
            Param::string("layer3")
                .describe("Third .env layer; its keys override layer1 and layer2 (conventionally .env.<mode>, e.g. .env.production). Leave blank if absent."),
        )
        .param(
            Param::string("layer4")
                .describe("Fourth and HIGHEST-priority .env layer; its keys override every other layer (conventionally .env.<mode>.local). Paste shell/CI variables here to model 'the environment beats the files'. Leave blank if absent."),
        )
        .param(
            Param::string("layer_names")
                .describe("Comma-separated display names for the four slots, used to say which file set each value (e.g. '.env,.env.local,.env.staging,.env.staging.local'). Names map positionally; blank entries fall back to the conventional cascade names .env, .env.local, .env.production, .env.production.local."),
        )
        .param(
            Param::enumv(
                "output",
                ["report", "env", "json", "shell", "markdown", "conflicts"],
            )
            .default("report")
            .describe("Output form: 'report' (default) the resolved environment with a '# set by <file>' provenance comment per key, plus the override chain and warnings; 'env' a plain merged .env file; 'json' a structured document with per-key source and the values it overrode; 'shell' export KEY='value' lines; 'markdown' a Key/Value/Set by/Overrides table; 'conflicts' only the keys set by more than one layer."),
        )
        .param(
            Param::boolean("mask_secrets")
                .default(true)
                .describe("Mask values of sensitive-looking keys (names containing SECRET, TOKEN, PASSWORD, KEY, AUTH, etc.) in every output. Turn off to get usable values. Default true."),
        )
        .param(
            Param::boolean("sort_keys")
                .default(false)
                .describe("Emit keys in alphabetical order instead of first-seen cascade order. Default false."),
        )
        .param(
            Param::string("prefix_filter")
                .describe("Keep only keys starting with this prefix, matched case-sensitively (e.g. 'VITE_' or 'NEXT_PUBLIC_' to see exactly what a framework exposes to the client). Default blank — keep every key."),
        )
        .param(
            Param::boolean("expand_vars")
                .default(false)
                .describe("Resolve ${VAR}, $VAR and ${VAR:-fallback} references against the merged result, so a reference picks up the winning value from any layer. Single-quoted values stay literal; unresolved and circular names become empty strings and are reported as warnings. Default false."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct EnvFileMerger;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/env-file-merger",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Merge layered .env files and show which file set each value.",
    skill(
        description = "Merge up to four layered .env files into one resolved environment and report which file set each value. Paste the files LOWEST priority first: `layer1` is the base .env, then `layer2`, `layer3`, `layer4` each override the ones before them — the same cascade dotenv-flow, Vite and Next.js use (.env < .env.local < .env.<mode> < .env.<mode>.local). Blank layers are skipped, so a missing file is not an error. Every KEY=VALUE line is parsed with the usual dotenv rules (# comments, blank lines, single/double quotes, inline comments, an optional 'export ' prefix), and the last layer that assigns a key wins. Unlike a plain overlay merge, each shadowed value is kept, so the tool can show the full override chain per key. `layer_names` renames the slots for provenance. `output` selects the shape: 'report' (default) the resolved environment with a '# set by <file>' comment per key plus the override chain and warnings; 'env' a merged .env file; 'json' per-key source and overrides; 'shell' export lines; 'markdown' a table; 'conflicts' only the multi-layer keys. `mask_secrets` (default true) masks sensitive-looking values, `sort_keys` sorts alphabetically, `prefix_filter` keeps only keys with a prefix like 'VITE_', and `expand_vars` resolves ${VAR} references against the merged result.",
        parameters = schema_json()
    ),
)]
impl EnvFileMerger {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "env-file-merger", |a: Args| {
            gizza_ai_env_file_merger_core::merge(
                &[&a.layer1, &a.layer2, &a.layer3, &a.layer4],
                &a.layer_names,
                if a.output.is_empty() {
                    "report"
                } else {
                    &a.output
                },
                a.mask_secrets,
                a.sort_keys,
                &a.prefix_filter,
                a.expand_vars,
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
                    "layer1": { "type": "string", "description": "The LOWEST-priority .env file's contents — the base file (conventionally .env). KEY=VALUE lines; # comments, blank lines, single/double quotes, inline comments and an 'export ' prefix are all handled." },
                    "layer2": { "type": "string", "description": "Second .env layer; its keys override layer1 (conventionally .env.local). Leave blank if this file doesn't exist — a blank layer is skipped, not an error." },
                    "layer3": { "type": "string", "description": "Third .env layer; its keys override layer1 and layer2 (conventionally .env.<mode>, e.g. .env.production). Leave blank if absent." },
                    "layer4": { "type": "string", "description": "Fourth and HIGHEST-priority .env layer; its keys override every other layer (conventionally .env.<mode>.local). Paste shell/CI variables here to model 'the environment beats the files'. Leave blank if absent." },
                    "layer_names": { "type": "string", "description": "Comma-separated display names for the four slots, used to say which file set each value (e.g. '.env,.env.local,.env.staging,.env.staging.local'). Names map positionally; blank entries fall back to the conventional cascade names .env, .env.local, .env.production, .env.production.local." },
                    "output": { "type": "string", "enum": ["report", "env", "json", "shell", "markdown", "conflicts"], "default": "report", "description": "Output form: 'report' (default) the resolved environment with a '# set by <file>' provenance comment per key, plus the override chain and warnings; 'env' a plain merged .env file; 'json' a structured document with per-key source and the values it overrode; 'shell' export KEY='value' lines; 'markdown' a Key/Value/Set by/Overrides table; 'conflicts' only the keys set by more than one layer." },
                    "mask_secrets": { "type": "boolean", "default": true, "description": "Mask values of sensitive-looking keys (names containing SECRET, TOKEN, PASSWORD, KEY, AUTH, etc.) in every output. Turn off to get usable values. Default true." },
                    "sort_keys": { "type": "boolean", "default": false, "description": "Emit keys in alphabetical order instead of first-seen cascade order. Default false." },
                    "prefix_filter": { "type": "string", "description": "Keep only keys starting with this prefix, matched case-sensitively (e.g. 'VITE_' or 'NEXT_PUBLIC_' to see exactly what a framework exposes to the client). Default blank — keep every key." },
                    "expand_vars": { "type": "boolean", "default": false, "description": "Resolve ${VAR}, $VAR and ${VAR:-fallback} references against the merged result, so a reference picks up the winning value from any layer. Single-quoted values stay literal; unresolved and circular names become empty strings and are reported as warnings. Default false." }
                },
                "required": ["layer1"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
