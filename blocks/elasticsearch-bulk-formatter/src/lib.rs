//! gizza-ai/elasticsearch-bulk-formatter — build the NDJSON request body for the
//! Elasticsearch `_bulk` API from a JSON array of documents. Thin wrapper around
//! the core; chat schema single-sourced from descriptor(); handler delegates to
//! run_skill. Pure.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn action_default() -> String {
    "index".to_string()
}

#[derive(Deserialize)]
struct Args {
    documents: String,
    #[serde(default = "action_default")]
    action: String,
    #[serde(default)]
    index: String,
    #[serde(default)]
    id_field: String,
    #[serde(default)]
    doc_as_upsert: bool,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("documents").required().multiline().describe("JSON array of documents, e.g. [{\"id\":\"1\",\"title\":\"hello\"}]. Each object becomes one bulk operation."))
        .param(Param::enumv("action", ["index", "create", "update", "delete"]).default("index").describe("Bulk action per document. index = create-or-replace; create = fail if it exists; update = partial (wrapped as {\"doc\":…}); delete = remove (no source line). Default index."))
        .param(Param::string("index").describe("Target index name written to _index in every metadata line. Leave empty to omit _index when you POST to /<index>/_bulk."))
        .param(Param::string("id_field").describe("Name of the field whose value becomes each document's _id; that field is stripped from the emitted source. Required for update and delete; optional for index and create."))
        .param(Param::boolean("doc_as_upsert").default(false).describe("For the update action, add \"doc_as_upsert\": true so a missing document is inserted from doc. Ignored for other actions."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ElasticsearchBulkFormatter;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/elasticsearch-bulk-formatter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Build the Elasticsearch _bulk NDJSON body from a JSON array of documents",
    skill(
        description = "Turn a JSON array of documents into the newline-delimited JSON (NDJSON) request body for the Elasticsearch _bulk API. Each document becomes an action/metadata line ({ \"<action>\": { \"_index\": …, \"_id\": … } }) plus, for index/create/update, a compact source line (update wraps it as { \"doc\": … }, optionally with doc_as_upsert); delete emits no source line. Pick the action (index/create/update/delete), an optional target index, and the field that supplies each document's _id (stripped from the source; required for update/delete). Output is compact, one JSON value per line, ending with a trailing newline — paste it into curl --data-binary, Kibana Dev Tools, or a client. Errors on invalid JSON, a non-array root, a non-object or empty item, or a document missing the required _id.",
        parameters = schema_json()
    ),
)]
impl ElasticsearchBulkFormatter {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "elasticsearch-bulk-formatter", |a: Args| {
            gizza_ai_elasticsearch_bulk_formatter_core::run(
                &a.documents,
                &a.action,
                &a.index,
                &a.id_field,
                a.doc_as_upsert,
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

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type":"object",
                "properties":{
                    "documents":{"type":"string","description":"JSON array of documents, e.g. [{\"id\":\"1\",\"title\":\"hello\"}]. Each object becomes one bulk operation."},
                    "action":{"type":"string","enum":["index","create","update","delete"],"default":"index","description":"Bulk action per document. index = create-or-replace; create = fail if it exists; update = partial (wrapped as {\"doc\":…}); delete = remove (no source line). Default index."},
                    "index":{"type":"string","description":"Target index name written to _index in every metadata line. Leave empty to omit _index when you POST to /<index>/_bulk."},
                    "id_field":{"type":"string","description":"Name of the field whose value becomes each document's _id; that field is stripped from the emitted source. Required for update and delete; optional for index and create."},
                    "doc_as_upsert":{"type":"boolean","default":false,"description":"For the update action, add \"doc_as_upsert\": true so a missing document is inserted from doc. Ignored for other actions."}
                },
                "required":["documents"],
                "additionalProperties":false
            }"#,
        ).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
