//! gizza-ai/config-merge — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_config_merge_core::{merge_config, Request};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    /// Lowest-priority layer — the base config file.
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
    /// auto | json | yaml | toml | env.
    #[serde(default = "default_input_format")]
    input_format: String,
    /// json | yaml | toml | env | report.
    #[serde(default = "default_output")]
    output: String,
    /// deep | shallow.
    #[serde(default = "default_object_merge")]
    object_merge: String,
    /// replace | append | unique.
    #[serde(default = "default_array_merge")]
    array_merge: String,
    /// match | preserve.
    #[serde(default = "default_key_case")]
    key_case: String,
    /// A null in a later layer removes the key (default true).
    #[serde(default = "default_true")]
    null_deletes: bool,
    /// Resolve ${VAR} references after merging (default true).
    #[serde(default = "default_true")]
    substitute: bool,
    /// Extra KEY=VALUE substitution variables that never appear in the output.
    #[serde(default)]
    vars: String,
    /// Sort every mapping's keys A-Z (default false).
    #[serde(default)]
    sort_keys: bool,
    /// Output indentation, 1-8 spaces.
    #[serde(default = "default_indent")]
    indent: i64,
}

fn default_true() -> bool {
    true
}
fn default_input_format() -> String {
    "auto".into()
}
fn default_output() -> String {
    "json".into()
}
fn default_object_merge() -> String {
    "deep".into()
}
fn default_array_merge() -> String {
    "replace".into()
}
fn default_key_case() -> String {
    "match".into()
}
fn default_indent() -> i64 {
    2
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("layer1")
                .required()
                .describe("The LOWEST-priority layer — the base config file (e.g. config.json, defaults.yaml, Cargo-style config.toml or a .env). Its syntax is detected automatically unless input_format says otherwise. Must be a mapping at the top level."),
        )
        .param(
            Param::string("layer2")
                .describe("Second layer; its keys override layer1 (e.g. config.staging.yaml). May be in a DIFFERENT format than layer1 — a YAML override on a JSON base is fine. Leave blank if this file doesn't exist; a blank layer is skipped, not an error."),
        )
        .param(
            Param::string("layer3")
                .describe("Third layer; its keys override layer1 and layer2 (e.g. config.local.toml). Leave blank if absent."),
        )
        .param(
            Param::string("layer4")
                .describe("Fourth and HIGHEST-priority layer; its keys override every other layer. Paste the deployment's .env or CI variables here to model 'the environment beats the files'. Leave blank if absent."),
        )
        .param(
            Param::string("layer_names")
                .describe("Comma-separated display names for the four slots, used by output=report to say which file set each value (e.g. 'config.json,config.prod.yaml,.env'). Names map positionally; blank entries fall back to base, override, environment, local."),
        )
        .param(
            Param::enumv("input_format", ["auto", "json", "yaml", "toml", "env"])
                .default("auto")
                .describe("Syntax of the layers. 'auto' (default) sniffs each layer independently: valid JSON starting with { or [ is JSON; a [section] header means TOML; a block of KEY=value lines with no spaces around '=' is .env; TOML's 'key = value' spacing means TOML; anything else is parsed as YAML. Set json/yaml/toml/env to force ONE syntax for every layer when a file would otherwise be sniffed wrong."),
        )
        .param(
            Param::enumv("output", ["json", "yaml", "toml", "env", "report"])
                .default("json")
                .describe("Format of the merged result: 'json' (default), 'yaml', 'toml' (nulls are dropped — TOML has none), 'env' (flattened UPPER_CASE keys joined with __, e.g. DB__HOST=localhost), or 'report' — the resolved values annotated with the layer that set each one, plus the override chain, keys removed by null, and every variable that was substituted or left unresolved."),
        )
        .param(
            Param::enumv("object_merge", ["deep", "shallow"])
                .default("deep")
                .describe("How nested mappings combine: 'deep' (default) merges them recursively key by key, so an override file only needs the keys it changes; 'shallow' merges top-level keys only and replaces the whole value below them."),
        )
        .param(
            Param::enumv("array_merge", ["replace", "append", "unique"])
                .default("replace")
                .describe("How lists combine: 'replace' (default) = the later layer's list wins outright; 'append' = concatenate base then override; 'unique' = concatenate then drop items already present."),
        )
        .param(
            Param::enumv("key_case", ["match", "preserve"])
                .default("match")
                .describe("How later layers match keys whose spelling differs only by case. 'match' (default) lets an env key such as DB__HOST override db.host while keeping the base layer's spelling. 'preserve' treats DB and db as separate keys."),
        )
        .param(
            Param::boolean("null_deletes")
                .default(true)
                .describe("true (default) = a key set to null in a later layer REMOVES it from the result, so an override file can delete an inherited default; false = null is kept as an ordinary value."),
        )
        .param(
            Param::boolean("substitute")
                .default(true)
                .describe("true (default) = after merging, expand ${VAR}, ${VAR:-default} (default when unset OR empty) and ${VAR-default} (default only when unset) inside string values, resolving against 'vars' first and then the merged document itself (${db.host} or ${DB__HOST}). '$$' is a literal '$'; a bare $VAR is left alone; an unresolvable reference is left exactly as written and listed under Unresolved in output=report. false = leave every ${...} untouched."),
        )
        .param(
            Param::string("vars")
                .describe("Extra substitution variables as KEY=VALUE lines (one per line, quotes and # comments allowed), e.g. 'DB_PASSWORD=s3cr3t'. They outrank values from the merged document and are used ONLY for substitution — they never appear in the output, which is how secrets stay out of the merged file."),
        )
        .param(
            Param::boolean("sort_keys")
                .default(false)
                .describe("false (default) = keep the base layer's key order, then newly added keys in the order the later layers introduced them; true = sort every mapping's keys A-Z recursively, which makes two merged results diffable."),
        )
        .param(
            Param::integer("indent")
                .min(1.0)
                .max(8.0)
                .default(2)
                .describe("Indentation in spaces per nesting level for the json and yaml outputs (1-8). Default 2. Ignored by toml, env and report."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn merge(a: &Args) -> Result<String, String> {
    merge_config(Request {
        layers: [&a.layer1, &a.layer2, &a.layer3, &a.layer4],
        layer_names: &a.layer_names,
        input_format: &a.input_format,
        output: &a.output,
        object_merge: &a.object_merge,
        array_merge: &a.array_merge,
        key_case: &a.key_case,
        null_deletes: a.null_deletes,
        substitute: a.substitute,
        vars: &a.vars,
        sort_keys: a.sort_keys,
        indent: a.indent.clamp(1, 8) as usize,
    })
}

#[cfg(target_arch = "wasm32")]
struct ConfigMerge;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/config-merge",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Merge layered config files across JSON, YAML, TOML and .env",
    skill(
        description = "Merge up to four layered configuration files into one document, even when the layers are in DIFFERENT formats — a JSON base, a YAML staging override and a .env deployment layer merge in a single pass. Each layer's syntax (JSON, YAML, TOML or .env) is detected automatically, or forced with input_format. Layers apply left to right: layer1 is the base and layer4 has the final say. Mappings merge recursively, lists are replaced (or appended / de-duplicated), and a key set to null in a later layer deletes it. After merging, ${VAR}, ${VAR:-default} and ${VAR-default} references inside string values are expanded against the 'vars' list first and then against the merged document itself; unresolvable references are left verbatim and reported. Flat env keys split on '__' into nested paths (DB__HOST=x becomes db.host). Emits json, yaml, toml, env, or a report naming the layer that set each value plus the override chain. Comments and YAML anchors are not preserved. Max 256 KiB of input across all layers. Runs locally.",
        parameters = schema_json()
    ),
)]
impl ConfigMerge {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "config-merge", |a: Args| {
            merge(&a).map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(layer1: &str) -> Args {
        Args {
            layer1: layer1.to_string(),
            layer2: String::new(),
            layer3: String::new(),
            layer4: String::new(),
            layer_names: String::new(),
            input_format: default_input_format(),
            output: default_output(),
            object_merge: default_object_merge(),
            array_merge: default_array_merge(),
            key_case: default_key_case(),
            null_deletes: true,
            substitute: true,
            vars: String::new(),
            sort_keys: false,
            indent: default_indent(),
        }
    }

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "layer1":       { "type": "string", "description": "The LOWEST-priority layer — the base config file (e.g. config.json, defaults.yaml, Cargo-style config.toml or a .env). Its syntax is detected automatically unless input_format says otherwise. Must be a mapping at the top level." },
                    "layer2":       { "type": "string", "description": "Second layer; its keys override layer1 (e.g. config.staging.yaml). May be in a DIFFERENT format than layer1 — a YAML override on a JSON base is fine. Leave blank if this file doesn't exist; a blank layer is skipped, not an error." },
                    "layer3":       { "type": "string", "description": "Third layer; its keys override layer1 and layer2 (e.g. config.local.toml). Leave blank if absent." },
                    "layer4":       { "type": "string", "description": "Fourth and HIGHEST-priority layer; its keys override every other layer. Paste the deployment's .env or CI variables here to model 'the environment beats the files'. Leave blank if absent." },
                    "layer_names":  { "type": "string", "description": "Comma-separated display names for the four slots, used by output=report to say which file set each value (e.g. 'config.json,config.prod.yaml,.env'). Names map positionally; blank entries fall back to base, override, environment, local." },
                    "input_format": { "type": "string", "enum": ["auto", "json", "yaml", "toml", "env"], "default": "auto", "description": "Syntax of the layers. 'auto' (default) sniffs each layer independently: valid JSON starting with { or [ is JSON; a [section] header means TOML; a block of KEY=value lines with no spaces around '=' is .env; TOML's 'key = value' spacing means TOML; anything else is parsed as YAML. Set json/yaml/toml/env to force ONE syntax for every layer when a file would otherwise be sniffed wrong." },
                    "output":       { "type": "string", "enum": ["json", "yaml", "toml", "env", "report"], "default": "json", "description": "Format of the merged result: 'json' (default), 'yaml', 'toml' (nulls are dropped — TOML has none), 'env' (flattened UPPER_CASE keys joined with __, e.g. DB__HOST=localhost), or 'report' — the resolved values annotated with the layer that set each one, plus the override chain, keys removed by null, and every variable that was substituted or left unresolved." },
                    "object_merge": { "type": "string", "enum": ["deep", "shallow"], "default": "deep", "description": "How nested mappings combine: 'deep' (default) merges them recursively key by key, so an override file only needs the keys it changes; 'shallow' merges top-level keys only and replaces the whole value below them." },
                    "array_merge":  { "type": "string", "enum": ["replace", "append", "unique"], "default": "replace", "description": "How lists combine: 'replace' (default) = the later layer's list wins outright; 'append' = concatenate base then override; 'unique' = concatenate then drop items already present." },
                    "key_case":     { "type": "string", "enum": ["match", "preserve"], "default": "match", "description": "How later layers match keys whose spelling differs only by case. 'match' (default) lets an env key such as DB__HOST override db.host while keeping the base layer's spelling. 'preserve' treats DB and db as separate keys." },
                    "null_deletes": { "type": "boolean", "default": true, "description": "true (default) = a key set to null in a later layer REMOVES it from the result, so an override file can delete an inherited default; false = null is kept as an ordinary value." },
                    "substitute":   { "type": "boolean", "default": true, "description": "true (default) = after merging, expand ${VAR}, ${VAR:-default} (default when unset OR empty) and ${VAR-default} (default only when unset) inside string values, resolving against 'vars' first and then the merged document itself (${db.host} or ${DB__HOST}). '$$' is a literal '$'; a bare $VAR is left alone; an unresolvable reference is left exactly as written and listed under Unresolved in output=report. false = leave every ${...} untouched." },
                    "vars":         { "type": "string", "description": "Extra substitution variables as KEY=VALUE lines (one per line, quotes and # comments allowed), e.g. 'DB_PASSWORD=s3cr3t'. They outrank values from the merged document and are used ONLY for substitution — they never appear in the output, which is how secrets stay out of the merged file." },
                    "sort_keys":    { "type": "boolean", "default": false, "description": "false (default) = keep the base layer's key order, then newly added keys in the order the later layers introduced them; true = sort every mapping's keys A-Z recursively, which makes two merged results diffable." },
                    "indent":       { "type": "integer", "minimum": 1, "maximum": 8, "default": 2, "description": "Indentation in spaces per nesting level for the json and yaml outputs (1-8). Default 2. Ignored by toml, env and report." }
                },
                "required": ["layer1"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn merges_a_json_base_with_a_yaml_override() {
        let mut a = args(r#"{"db":{"host":"localhost","port":5432}}"#);
        a.layer2 = "db:\n  host: prod.db\n".to_string();
        assert_eq!(
            merge(&a).unwrap(),
            "{\n  \"db\": {\n    \"host\": \"prod.db\",\n    \"port\": 5432\n  }\n}\n"
        );
    }

    #[test]
    fn invalid_option_is_rejected() {
        let mut a = args(r#"{"a":1}"#);
        a.output = "xml".to_string();
        let err = merge(&a).unwrap_err();
        assert!(err.contains("unknown output 'xml'"), "{err}");
    }
}
