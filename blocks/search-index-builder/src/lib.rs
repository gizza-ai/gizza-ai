//! gizza-ai/search-index-builder — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Pure compute, no host
//! calls — runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn default_true() -> bool {
    true
}
fn default_min_length() -> u32 {
    1
}

#[derive(Deserialize)]
struct Args {
    documents: String,
    #[serde(default)]
    fields: String,
    #[serde(default)]
    id_field: String,
    #[serde(default)]
    store_fields: String,
    #[serde(default)]
    boosts: String,
    #[serde(default = "default_true")]
    lowercase: bool,
    #[serde(default)]
    remove_stopwords: bool,
    #[serde(default = "default_min_length")]
    min_length: u32,
    #[serde(default)]
    pretty: bool,
}

/// Single-source param descriptor → chat schema (and CLI). See
/// docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("documents")
                .required()
                .describe("A JSON array of document objects to index, e.g. [{\"id\":\"1\",\"title\":\"Intro\",\"body\":\"Hello world\"}]. This is the in-model stand-in for a folder of documents: a static-site build step reads the folder and passes the array here."),
        )
        .param(
            Param::string("fields")
                .default("")
                .describe("Comma-separated field names to full-text index (e.g. 'title,body'). Leave empty to index every string-valued field found across the documents, except the id field."),
        )
        .param(
            Param::string("id_field")
                .default("id")
                .describe("Which field supplies each document's ref (the identifier stored in the index). Default 'id'. When a document lacks this field, its 0-based position in the array is used instead. Refs must be unique."),
        )
        .param(
            Param::string("store_fields")
                .default("")
                .describe("Comma-separated fields to copy verbatim into the output 'documents' store for display alongside search results (e.g. 'title,url'). Empty (default) stores nothing but the ref."),
        )
        .param(
            Param::string("boosts")
                .default("")
                .describe("Optional per-field ranking weights recorded in the output for the query-time ranker, as a 'field:weight' list (e.g. 'title:2,body:1'). Empty (default) records no boosts."),
        )
        .param(
            Param::boolean("lowercase")
                .default(true)
                .describe("Case-fold tokens to lowercase for case-insensitive search. Default true; set false to keep tokens case-sensitive."),
        )
        .param(
            Param::boolean("remove_stopwords")
                .default(false)
                .describe("Drop common English stop words (the, and, of, to, …) from the index. Default false."),
        )
        .param(
            Param::integer("min_length")
                .default(1)
                .min(1.0)
                .max(gizza_ai_search_index_builder_core::MIN_LENGTH_CAP as f64)
                .describe("Drop tokens shorter than this many characters, 1-20. Default 1 (keep all). Use 2 or 3 to skip single letters and short noise."),
        )
        .param(
            Param::boolean("pretty")
                .default(false)
                .describe("Pretty-print the index JSON with indentation. Default false (compact single line, smaller for shipping to a static site)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/search-index-builder",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Build a serialized full-text inverted-index JSON from a JSON array of documents.",
    skill(
        description = "Build a serialized full-text search index (an inverted index, as JSON) from a JSON array of document objects, so a static site can search offline with no server. Every indexed field is tokenized (split on non-alphanumeric boundaries) and the index maps each token to the documents it appears in, per field, with a document frequency (df) and a per-document term frequency — everything a client needs to rank with TF-IDF. Pass 'documents' as a JSON array like [{\"id\":\"1\",\"title\":\"Intro\",\"body\":\"Hello world\"}]. Choose which fields to index, which field is the id/ref, and which fields to store for display. Toggle lowercase folding, English stop-word removal, and a minimum token length; record optional per-field ranking boosts. Output is deterministic JSON.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": ... }.
        match run_skill(&body, "search-index-builder", |a: Args| {
            gizza_ai_search_index_builder_core::build_index(
                &a.documents,
                &a.fields,
                &a.id_field,
                &a.store_fields,
                &a.boosts,
                a.lowercase,
                a.remove_stopwords,
                a.min_length as usize,
                a.pretty,
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
                    "documents": { "type": "string", "description": "A JSON array of document objects to index, e.g. [{\"id\":\"1\",\"title\":\"Intro\",\"body\":\"Hello world\"}]. This is the in-model stand-in for a folder of documents: a static-site build step reads the folder and passes the array here." },
                    "fields": { "type": "string", "default": "", "description": "Comma-separated field names to full-text index (e.g. 'title,body'). Leave empty to index every string-valued field found across the documents, except the id field." },
                    "id_field": { "type": "string", "default": "id", "description": "Which field supplies each document's ref (the identifier stored in the index). Default 'id'. When a document lacks this field, its 0-based position in the array is used instead. Refs must be unique." },
                    "store_fields": { "type": "string", "default": "", "description": "Comma-separated fields to copy verbatim into the output 'documents' store for display alongside search results (e.g. 'title,url'). Empty (default) stores nothing but the ref." },
                    "boosts": { "type": "string", "default": "", "description": "Optional per-field ranking weights recorded in the output for the query-time ranker, as a 'field:weight' list (e.g. 'title:2,body:1'). Empty (default) records no boosts." },
                    "lowercase": { "type": "boolean", "default": true, "description": "Case-fold tokens to lowercase for case-insensitive search. Default true; set false to keep tokens case-sensitive." },
                    "remove_stopwords": { "type": "boolean", "default": false, "description": "Drop common English stop words (the, and, of, to, …) from the index. Default false." },
                    "min_length": { "type": "integer", "minimum": 1, "maximum": 20, "default": 1, "description": "Drop tokens shorter than this many characters, 1-20. Default 1 (keep all). Use 2 or 3 to skip single letters and short noise." },
                    "pretty": { "type": "boolean", "default": false, "description": "Pretty-print the index JSON with indentation. Default false (compact single line, smaller for shipping to a static site)." }
                },
                "required": ["documents"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
