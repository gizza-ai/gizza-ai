//! gizza-ai/json-normalize — chat skill block on the shared tool abstraction.
//!
//! Normalize nested JSON into entity tables keyed by id, similar to the
//! normalizr-style `{ entities, result }` shape used by front-end stores and ETL
//! pipelines. The chat schema is single-sourced from `descriptor()` (which also
//! drives the CLI); `handle()` delegates to `block_utils::run_skill`.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    json: String,
    schema: String,
    root: String,
    #[serde(default)]
    path: String,
    #[serde(default = "default_id_field")]
    id_field: String,
    #[serde(default = "default_on_missing_id")]
    on_missing_id: String,
    #[serde(default = "default_on_conflict")]
    on_conflict: String,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default = "default_pretty")]
    pretty: bool,
    #[serde(default = "default_indent")]
    indent: usize,
}

fn default_id_field() -> String {
    "id".to_string()
}
fn default_on_missing_id() -> String {
    "error".to_string()
}
fn default_on_conflict() -> String {
    "merge".to_string()
}
fn default_output() -> String {
    "normalized".to_string()
}
fn default_pretty() -> bool {
    true
}
fn default_indent() -> usize {
    2
}

impl Args {
    fn run(&self) -> Result<String, String> {
        gizza_ai_json_normalize_core::normalize(
            &self.json,
            &self.schema,
            &self.root,
            &self.path,
            &self.id_field,
            &self.on_missing_id,
            &self.on_conflict,
            &self.output,
            self.pretty,
            self.indent,
        )
    }
}

/// Single source for the chat schema (and CLI). The schema can be the JSON
/// object form (`{"articles":{"author":"users"}}`) or shorthand lines
/// (`articles: author -> users`).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("json").required().describe(
            "The JSON document to normalize. It may be one entity object, an array of entity \
             objects, or a wrapper object when path points at the payload. Decoded input is capped \
             at 5 MB and nesting is capped at 100 levels.",
        ))
        .param(Param::string("schema").required().describe(
            "Entity schema. JSON form maps entity names to relation fields, e.g. \
             {\"articles\":{\"author\":\"users\",\"comments\":[\"comments\"]},\"users\":{}}. \
             Shorthand form is one line per entity: articles: author -> users, comments -> \
             [comments]. Undefined fields are copied through unchanged.",
        ))
        .param(Param::string("root").required().describe(
            "The schema entity represented by the document or by each item at path, such as \
             articles, comments, or users. It must be declared in schema.",
        ))
        .param(Param::string("path").describe(
            "Optional dotted/indexed path to the payload inside a wrapper document, for example \
             data.items or data.0.attributes. Leave empty to normalize the whole document.",
        ))
        .param(Param::string("id_field").default("id").describe(
            "Entity id field. Use one field name (id), a fallback list (id,_id,uuid), or a JSON \
             object mapping entity names to fields, with * as a default: {\"*\":\"id\",\"tweets\":\"id_str\"}. \
             Ids must be scalar JSON strings, numbers, or booleans.",
        ))
        .param(
            Param::enumv("on_missing_id", ["error", "index", "hash", "keep"])
                .default("error")
                .describe(
                    "What to do when an entity object lacks its id field. error (default) stops; \
                     index creates stable run-local ids like users-1; hash creates a content hash; \
                     keep leaves that nested object inline instead of extracting it.",
                ),
        )
        .param(
            Param::enumv("on_conflict", ["merge", "replace", "keep_first", "error"])
                .default("merge")
                .describe(
                    "How duplicate ids in the same table are handled. merge (default) shallowly \
                     merges with later keys winning; replace keeps the last entity; keep_first \
                     keeps the first; error rejects the document.",
                ),
        )
        .param(
            Param::enumv("output", ["normalized", "entities", "result", "report"])
                .default("normalized")
                .describe(
                    "Output shape. normalized returns {entities,result}; entities returns only \
                     lookup tables; result returns only the root id/id array; report returns a \
                     human-readable count summary for debugging schemas.",
                ),
        )
        .param(Param::boolean("pretty").default(true).describe(
            "Pretty-print JSON output. Turn off for compact exact JSON; report output is always \
             plain text.",
        ))
        .param(Param::integer("indent").default(2.0).min(0.0).max(8.0).describe(
            "Spaces per indent level when pretty is true, from 0 through 8. Values above 8 are \
             clamped to keep browser output readable.",
        ))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/json-normalize",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Normalize nested JSON into id-keyed entity tables",
    skill(
        description = "Normalize a nested JSON document into the normalizr-style {entities,result} shape used by front-end stores and ETL code. Provide an entity schema that names each table and the fields that point at nested entities; the tool extracts those nested objects into entities.<type>.<id> and replaces each occurrence with its id. The schema accepts either a JSON object form ({\"articles\":{\"author\":\"users\",\"comments\":[\"comments\"]},\"users\":{}}) or shorthand lines (articles: author -> users, comments -> [comments]). root names the entity represented by the document or by each item at path; path can point into wrappers such as data.items. id_field defaults to id but can be a fallback list or per-entity JSON map. Duplicate ids can merge, replace, keep first, or error. Missing ids can error, synthesize index ids, content-hash ids, or stay inline. Output can be {entities,result}, entities only, result only, or a report. Pure local Rust/WASM; no eval callbacks, no network, no denormalize step.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "json-normalize", |a: Args| {
            a.run().map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args() -> Args {
        Args {
            json: r#"{"id":"123","author":{"id":"1","name":"Paul"}}"#.into(),
            schema: "articles: author -> users\nusers:".into(),
            root: "articles".into(),
            path: String::new(),
            id_field: default_id_field(),
            on_missing_id: default_on_missing_id(),
            on_conflict: default_on_conflict(),
            output: default_output(),
            pretty: false,
            indent: default_indent(),
        }
    }

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type":"object",
                "properties":{
                    "json":{"type":"string","description":"The JSON document to normalize. It may be one entity object, an array of entity objects, or a wrapper object when path points at the payload. Decoded input is capped at 5 MB and nesting is capped at 100 levels."},
                    "schema":{"type":"string","description":"Entity schema. JSON form maps entity names to relation fields, e.g. {\"articles\":{\"author\":\"users\",\"comments\":[\"comments\"]},\"users\":{}}. Shorthand form is one line per entity: articles: author -> users, comments -> [comments]. Undefined fields are copied through unchanged."},
                    "root":{"type":"string","description":"The schema entity represented by the document or by each item at path, such as articles, comments, or users. It must be declared in schema."},
                    "path":{"type":"string","description":"Optional dotted/indexed path to the payload inside a wrapper document, for example data.items or data.0.attributes. Leave empty to normalize the whole document."},
                    "id_field":{"type":"string","default":"id","description":"Entity id field. Use one field name (id), a fallback list (id,_id,uuid), or a JSON object mapping entity names to fields, with * as a default: {\"*\":\"id\",\"tweets\":\"id_str\"}. Ids must be scalar JSON strings, numbers, or booleans."},
                    "on_missing_id":{"type":"string","enum":["error","index","hash","keep"],"default":"error","description":"What to do when an entity object lacks its id field. error (default) stops; index creates stable run-local ids like users-1; hash creates a content hash; keep leaves that nested object inline instead of extracting it."},
                    "on_conflict":{"type":"string","enum":["merge","replace","keep_first","error"],"default":"merge","description":"How duplicate ids in the same table are handled. merge (default) shallowly merges with later keys winning; replace keeps the last entity; keep_first keeps the first; error rejects the document."},
                    "output":{"type":"string","enum":["normalized","entities","result","report"],"default":"normalized","description":"Output shape. normalized returns {entities,result}; entities returns only lookup tables; result returns only the root id/id array; report returns a human-readable count summary for debugging schemas."},
                    "pretty":{"type":"boolean","default":true,"description":"Pretty-print JSON output. Turn off for compact exact JSON; report output is always plain text."},
                    "indent":{"type":"integer","default":2.0,"minimum":0,"maximum":8,"description":"Spaces per indent level when pretty is true, from 0 through 8. Values above 8 are clamped to keep browser output readable."}
                },
                "required":["json","schema","root"],
                "additionalProperties":false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn args_layer_normalizes_nested_json() {
        assert_eq!(
            args().run().unwrap(),
            r#"{"entities":{"articles":{"123":{"id":"123","author":"1"}},"users":{"1":{"id":"1","name":"Paul"}}},"result":"123"}"#
        );
    }

    #[test]
    fn report_output_surfaces_table_counts() {
        let mut a = args();
        a.output = "report".into();
        let out = a.run().unwrap();
        assert!(out.contains("Root entity: articles"), "{out}");
        assert!(out.contains("users: 1 entity"), "{out}");
    }

    #[test]
    fn invalid_schema_bubbles_to_the_skill_error() {
        let mut a = args();
        a.schema = "articles: author -> people".into();
        let err = a.run().unwrap_err();
        assert!(err.contains("unknown entity \"people\""), "{err}");
    }
}
