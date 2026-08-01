//! gizza-ai/yaml-formatter — reindent and normalize YAML on the shared tool
//! abstraction. The chat schema is single-sourced from descriptor() (which also
//! drives the CLI); handle() delegates to block_utils::run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_yaml_formatter_core::{format_yaml, parse_sort, parse_style, Options};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    yaml: String,
    #[serde(default = "default_indent")]
    indent: u64,
    #[serde(default = "default_style")]
    style: String,
    #[serde(default = "default_sort")]
    sort_keys: String,
}

fn default_indent() -> u64 {
    2
}

fn default_style() -> String {
    "block".to_string()
}

fn default_sort() -> String {
    "preserve".to_string()
}

/// Resolve the raw args into core [`Options`] + run the formatter.
fn reformat(a: &Args) -> Result<String, String> {
    let opts = Options {
        indent: (a.indent as usize).clamp(1, 8),
        style: parse_style(&a.style)?,
        sort: parse_sort(&a.sort_keys)?,
    };
    format_yaml(&a.yaml, opts)
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("yaml")
                .required()
                .describe("The YAML document (or `---` separated multi-document stream) to reindent and normalize. Messy or inconsistently-indented input is fine."),
        )
        .param(
            Param::integer("indent")
                .min(1.0)
                .max(8.0)
                .default(2)
                .describe("Spaces of indentation per nesting level for block style (1-8). Ignored for flow style. YAML forbids tab indentation. Default 2."),
        )
        .param(
            Param::enumv("style", ["block", "flow"])
                .default("block")
                .describe("Output layout: 'block' is expanded multi-line YAML (beautify); 'flow' is compact single-line JSON-like YAML (minify). Default block."),
        )
        .param(
            Param::enumv("sort_keys", ["preserve", "asc", "desc"])
                .default("preserve")
                .describe("Mapping key order: 'preserve' keeps the original order; 'asc'/'desc' sort every mapping's keys alphabetically (recursively). Default preserve."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct YamlFormatter;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/yaml-formatter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Reindent and normalize YAML with key ordering and block/flow style",
    skill(
        description = "Reindent and normalize a YAML document with consistent spacing. indent is spaces per nesting level for block style (1-8, default 2; YAML forbids tabs). style is 'block' (expanded multi-line, default) or 'flow' (compact single-line). sort_keys is 'preserve' (default), 'asc' or 'desc' to alphabetize every mapping's keys recursively. Data is normalized round-trip; comments, blank lines and anchors are not preserved (aliases are expanded). Multi-document `---` streams are supported. Runs locally.",
        parameters = schema_json()
    ),
)]
impl YamlFormatter {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "yaml-formatter", |a: Args| {
            reformat(&a).map_err(SkillError::InvalidArgs)
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
                    "yaml":      { "type": "string", "description": "The YAML document (or `---` separated multi-document stream) to reindent and normalize. Messy or inconsistently-indented input is fine." },
                    "indent":    { "type": "integer", "minimum": 1, "maximum": 8, "default": 2, "description": "Spaces of indentation per nesting level for block style (1-8). Ignored for flow style. YAML forbids tab indentation. Default 2." },
                    "style":     { "type": "string", "enum": ["block", "flow"], "default": "block", "description": "Output layout: 'block' is expanded multi-line YAML (beautify); 'flow' is compact single-line JSON-like YAML (minify). Default block." },
                    "sort_keys": { "type": "string", "enum": ["preserve", "asc", "desc"], "default": "preserve", "description": "Mapping key order: 'preserve' keeps the original order; 'asc'/'desc' sort every mapping's keys alphabetically (recursively). Default preserve." }
                },
                "required": ["yaml"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn reformat_block_default() {
        let a = Args {
            yaml: "name:  gizza\ntags: [a, b]\n".to_string(),
            indent: 2,
            style: "block".to_string(),
            sort_keys: "preserve".to_string(),
        };
        assert_eq!(reformat(&a).unwrap(), "name: gizza\ntags:\n  - a\n  - b\n");
    }

    #[test]
    fn reformat_flow_and_sort() {
        let a = Args {
            yaml: "b: 1\na: 2\n".to_string(),
            indent: 2,
            style: "flow".to_string(),
            sort_keys: "asc".to_string(),
        };
        assert_eq!(reformat(&a).unwrap(), "{a: 2, b: 1}\n");
    }

    #[test]
    fn reformat_rejects_invalid() {
        let a = Args {
            yaml: "a: [1, 2".to_string(),
            indent: 2,
            style: "block".to_string(),
            sort_keys: "preserve".to_string(),
        };
        assert!(reformat(&a).unwrap_err().contains("invalid YAML"));
    }
}
