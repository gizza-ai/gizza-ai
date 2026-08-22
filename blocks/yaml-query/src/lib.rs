//! gizza-ai/yaml-query — read, filter and transform YAML with jq-style (yq-style)
//! expressions, and convert the result between YAML and JSON. Thin wrapper around
//! the core; chat schema single-sourced from descriptor(); handler delegates to
//! run_skill. Pure → all backends incl. chat SW.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_yaml_query_core::{run_query_text, DocMode, InFormat, Options, OutFormat};
use serde::Deserialize;
use wafer_sdk::*;

fn default_input_format() -> String {
    "auto".to_string()
}
fn default_output_format() -> String {
    "yaml".to_string()
}
fn default_documents() -> String {
    "each".to_string()
}
fn default_true() -> bool {
    true
}

#[derive(Deserialize)]
struct Args {
    yaml: String,
    query: String,
    #[serde(default = "default_input_format")]
    input_format: String,
    #[serde(default = "default_output_format")]
    output_format: String,
    #[serde(default = "default_documents")]
    documents: String,
    #[serde(default = "default_true")]
    pretty: bool,
    #[serde(default)]
    raw_output: bool,
}

impl Args {
    fn options(&self) -> Result<Options, String> {
        Ok(Options {
            input_format: InFormat::parse(&self.input_format)?,
            output_format: OutFormat::parse(&self.output_format)?,
            documents: DocMode::parse(&self.documents)?,
            pretty: self.pretty,
            raw_output: self.raw_output,
        })
    }
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("yaml").required().describe("The YAML document to query, pasted as text. JSON is accepted too (YAML is a superset of it). Multi-document streams separated by '---' are supported, anchors/aliases and '<<' merge keys are resolved on read, and custom tags such as !Ref or !!str are unwrapped to their value."))
        .param(Param::string("query").required().describe("The jq-style filter to run — the same expression language yq uses. Examples: '.' (the whole document), '.services.web.ports', '.services | keys', '[.spec.template.spec.containers[].image]', 'to_entries | map(.key)', '.jobs | with_entries(select(.value.needs))'. The jq standard library (map, select, sort_by, group_by, to_entries, add, length, unique, …) is available. A filter yields a stream of zero, one, or many values and every value is emitted."))
        .param(Param::enumv("input_format", ["auto", "yaml", "json"]).default("auto").describe("How to parse the input. 'auto' (default) sniffs: a document starting with '{' or '[' is tried as JSON first and falls back to YAML, anything else is parsed as YAML. 'yaml' or 'json' force one parser, so a mis-typed document is reported as a parse error instead of guessed at."))
        .param(Param::enumv("output_format", ["yaml", "json"]).default("yaml").describe("How to serialize each result value. 'yaml' (default) emits block-style YAML; 'json' emits JSON. Use 'json' to pipe the result into a JSON tool, 'yaml' to paste it back into a config file."))
        .param(Param::enumv("documents", ["each", "slurp"]).default("each").describe("How a multi-document YAML stream ('---' separated, the Kubernetes shape) is fed to the filter. 'each' (default) runs the filter once per document and concatenates the outputs, matching yq. 'slurp' collects every document into a single array and runs the filter once over that, matching jq's --slurp/-s — use it to filter or count across documents."))
        .param(Param::boolean("pretty").default(true).describe("Indent JSON output by 2 spaces. Default true; set false for compact single-line JSON. Ignored when output_format is yaml, which is always block style."))
        .param(Param::boolean("raw_output").default(false).describe("Emit string results without quotes or escaping, like jq -r / yq -r. Handy for feeding an image name or a port into a shell. Non-string results (objects, arrays, numbers, booleans, null) are unaffected. Default false."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct YamlQuery;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/yaml-query",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Query and transform YAML with jq-style expressions",
    skill(
        description = "Read, filter and transform a YAML document with a jq-style filter — the expression language yq uses — and get the result back as YAML or JSON. Pass the document in `yaml` and the filter in `query`: '.services.web.ports' pulls a value out of a docker-compose file, '.services | keys' lists the service names, '[.spec.template.spec.containers[].image]' collects every container image in a Kubernetes manifest, 'to_entries | map(.key)' turns a mapping into its key list. The whole jq standard library (map, select, sort_by, group_by, to_entries, add, length, unique, with_entries) is available, and a filter yields a stream of zero, one, or many values — all of them are returned, joined as a '---' YAML document stream when there is more than one. JSON input is accepted as well (input_format=auto sniffs, or force yaml/json). Multi-document YAML streams are supported: documents=each (default) runs the filter per document like yq, documents=slurp feeds all documents to the filter as one array like jq -s. output_format=json returns JSON instead of YAML (pretty=false makes it compact), and raw_output=true emits string results unquoted like jq -r. Anchors, aliases and '<<' merge keys are resolved on read and custom tags are unwrapped. Everything is computed locally; nothing is fetched or uploaded.",
        parameters = schema_json()
    ),
)]
impl YamlQuery {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "yaml-query", |a: Args| {
            let opts = a.options().map_err(SkillError::InvalidArgs)?;
            run_query_text(&a.yaml, &a.query, &opts).map_err(SkillError::InvalidArgs)
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
                    "yaml": { "type": "string", "description": "The YAML document to query, pasted as text. JSON is accepted too (YAML is a superset of it). Multi-document streams separated by '---' are supported, anchors/aliases and '<<' merge keys are resolved on read, and custom tags such as !Ref or !!str are unwrapped to their value." },
                    "query": { "type": "string", "description": "The jq-style filter to run — the same expression language yq uses. Examples: '.' (the whole document), '.services.web.ports', '.services | keys', '[.spec.template.spec.containers[].image]', 'to_entries | map(.key)', '.jobs | with_entries(select(.value.needs))'. The jq standard library (map, select, sort_by, group_by, to_entries, add, length, unique, …) is available. A filter yields a stream of zero, one, or many values and every value is emitted." },
                    "input_format": { "type": "string", "enum": ["auto", "yaml", "json"], "default": "auto", "description": "How to parse the input. 'auto' (default) sniffs: a document starting with '{' or '[' is tried as JSON first and falls back to YAML, anything else is parsed as YAML. 'yaml' or 'json' force one parser, so a mis-typed document is reported as a parse error instead of guessed at." },
                    "output_format": { "type": "string", "enum": ["yaml", "json"], "default": "yaml", "description": "How to serialize each result value. 'yaml' (default) emits block-style YAML; 'json' emits JSON. Use 'json' to pipe the result into a JSON tool, 'yaml' to paste it back into a config file." },
                    "documents": { "type": "string", "enum": ["each", "slurp"], "default": "each", "description": "How a multi-document YAML stream ('---' separated, the Kubernetes shape) is fed to the filter. 'each' (default) runs the filter once per document and concatenates the outputs, matching yq. 'slurp' collects every document into a single array and runs the filter once over that, matching jq's --slurp/-s — use it to filter or count across documents." },
                    "pretty": { "type": "boolean", "default": true, "description": "Indent JSON output by 2 spaces. Default true; set false for compact single-line JSON. Ignored when output_format is yaml, which is always block style." },
                    "raw_output": { "type": "boolean", "default": false, "description": "Emit string results without quotes or escaping, like jq -r / yq -r. Handy for feeding an image name or a port into a shell. Non-string results (objects, arrays, numbers, booleans, null) are unaffected. Default false." }
                },
                "required": ["yaml", "query"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn omitted_options_match_the_descriptor_defaults() {
        let a: Args = serde_json::from_str(r#"{"yaml":"a: 1","query":"."}"#).unwrap();
        assert_eq!(a.input_format, "auto");
        assert_eq!(a.output_format, "yaml");
        assert_eq!(a.documents, "each");
        assert!(
            a.pretty,
            "pretty must default to true, matching the descriptor"
        );
        assert!(
            !a.raw_output,
            "raw_output must default to false, matching the descriptor"
        );

        let opts = a.options().unwrap();
        assert_eq!(opts.input_format, InFormat::Auto);
        assert_eq!(opts.output_format, OutFormat::Yaml);
        assert_eq!(opts.documents, DocMode::Each);
    }

    #[test]
    fn args_drive_the_core_end_to_end() {
        let a: Args = serde_json::from_str(
            r#"{"yaml":"services:\n  web:\n    image: nginx\n","query":".services | keys","output_format":"json","pretty":false}"#,
        )
        .unwrap();
        let out = run_query_text(&a.yaml, &a.query, &a.options().unwrap()).unwrap();
        assert_eq!(out, r#"["web"]"#);
    }

    #[test]
    fn an_unknown_enum_value_is_an_error_not_a_silent_default() {
        let a: Args =
            serde_json::from_str(r#"{"yaml":"a: 1","query":".","output_format":"toml"}"#).unwrap();
        let e = a.options().unwrap_err();
        assert!(e.contains("unknown output_format"), "{e}");
    }
}
