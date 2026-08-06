//! gizza-ai/json-remove-nulls — recursively strip null (and, opt-in, empty)
//! values out of a JSON document, validating the input.
//! Chat schema single-sourced from descriptor() (which also drives the CLI);
//! handler delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_json_remove_nulls_core::{remove_nulls, Arrays, Options};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    json: String,
    #[serde(default)]
    remove_empty_strings: bool,
    #[serde(default)]
    remove_empty_arrays: bool,
    #[serde(default)]
    remove_empty_objects: bool,
    #[serde(default)]
    trim_strings: bool,
    #[serde(default = "default_arrays")]
    arrays: String,
    #[serde(default = "default_indent")]
    indent: u64,
}

fn default_arrays() -> String {
    "compact".into()
}
fn default_indent() -> u64 {
    2
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("json")
                .required()
                .describe("The JSON text to clean, e.g. {\"a\": 1, \"b\": null, \"c\": {\"d\": null}}."),
        )
        .param(
            Param::boolean("remove_empty_strings")
                .default(false)
                .describe("Also remove values that are the empty string \"\". Off by default, so \"\" is kept as a real value."),
        )
        .param(
            Param::boolean("remove_empty_arrays")
                .default(false)
                .describe("Also remove values that are the empty array []. Off by default. Applies after pruning, so an array emptied by the prune is removed too."),
        )
        .param(
            Param::boolean("remove_empty_objects")
                .default(false)
                .describe("Also remove values that are the empty object {}. Off by default. Removal cascades bottom-up: an object left empty by the prune disappears, which can empty its parent in turn."),
        )
        .param(
            Param::boolean("trim_strings")
                .default(false)
                .describe("Trim leading/trailing whitespace from every string value first, so a whitespace-only string becomes \"\" (and is removable when remove_empty_strings is on). Off by default."),
        )
        .param(
            Param::enumv("arrays", ["compact", "keep"])
                .default("compact")
                .describe("What to do with removable values sitting directly inside an array: 'compact' (default) drops them and closes the gap, so [1, null, 2] becomes [1, 2]; 'keep' leaves array elements untouched so indices stay stable, while still pruning objects nested inside the array."),
        )
        .param(
            Param::integer("indent")
                .min(0.0)
                .max(8.0)
                .default(2)
                .describe("Spaces of indentation per level (1-8). Use 0 to minify to a single compact line. Default 2."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn build_options(a: &Args) -> Options {
    Options {
        remove_empty_strings: a.remove_empty_strings,
        remove_empty_arrays: a.remove_empty_arrays,
        remove_empty_objects: a.remove_empty_objects,
        trim_strings: a.trim_strings,
        arrays: Arrays::parse(&a.arrays),
        indent: a.indent as usize,
    }
}

#[cfg(target_arch = "wasm32")]
struct JsonRemoveNulls;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/json-remove-nulls",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Recursively remove null and optionally empty values from JSON",
    skill(
        description = "Recursively remove every object key whose value is null from a JSON document, validating it in the process (returns a line/column error if invalid). Opt in to also drop empty strings (remove_empty_strings), empty arrays (remove_empty_arrays) and empty objects (remove_empty_objects) — removal cascades bottom-up, so a container emptied by the prune is dropped too. trim_strings trims whitespace from string values first. arrays is 'compact' (default, [1, null, 2] becomes [1, 2]) or 'keep' (array positions stay stable). indent is spaces per level (1-8, default 2), or 0 to minify. false and 0 are never treated as empty; key order is preserved. Runs locally.",
        parameters = schema_json()
    ),
)]
impl JsonRemoveNulls {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "json-remove-nulls", |a: Args| {
            let opts = build_options(&a);
            remove_nulls(&a.json, opts).map_err(SkillError::InvalidArgs)
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
                    "json":                 { "type": "string", "description": "The JSON text to clean, e.g. {\"a\": 1, \"b\": null, \"c\": {\"d\": null}}." },
                    "remove_empty_strings": { "type": "boolean", "default": false, "description": "Also remove values that are the empty string \"\". Off by default, so \"\" is kept as a real value." },
                    "remove_empty_arrays":  { "type": "boolean", "default": false, "description": "Also remove values that are the empty array []. Off by default. Applies after pruning, so an array emptied by the prune is removed too." },
                    "remove_empty_objects": { "type": "boolean", "default": false, "description": "Also remove values that are the empty object {}. Off by default. Removal cascades bottom-up: an object left empty by the prune disappears, which can empty its parent in turn." },
                    "trim_strings":         { "type": "boolean", "default": false, "description": "Trim leading/trailing whitespace from every string value first, so a whitespace-only string becomes \"\" (and is removable when remove_empty_strings is on). Off by default." },
                    "arrays":               { "type": "string", "enum": ["compact", "keep"], "default": "compact", "description": "What to do with removable values sitting directly inside an array: 'compact' (default) drops them and closes the gap, so [1, null, 2] becomes [1, 2]; 'keep' leaves array elements untouched so indices stay stable, while still pruning objects nested inside the array." },
                    "indent":               { "type": "integer", "minimum": 0, "maximum": 8, "default": 2, "description": "Spaces of indentation per level (1-8). Use 0 to minify to a single compact line. Default 2." }
                },
                "required": ["json"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn build_options_maps_args() {
        let a = Args {
            json: "{}".into(),
            remove_empty_strings: true,
            remove_empty_arrays: false,
            remove_empty_objects: true,
            trim_strings: true,
            arrays: "keep".into(),
            indent: 4,
        };
        let o = build_options(&a);
        assert!(o.remove_empty_strings && o.remove_empty_objects && o.trim_strings);
        assert!(!o.remove_empty_arrays);
        assert_eq!(o.arrays, Arrays::Keep);
        assert_eq!(o.indent, 4);
    }
}
