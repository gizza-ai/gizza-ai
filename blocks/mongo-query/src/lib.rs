//! gizza-ai/mongo-query — run a MongoDB-style query document against a pasted JSON
//! array of documents.
//!
//! Thin chat-skill wrapper around `gizza-ai-mongo-query-core`. The chat schema is
//! derived from `descriptor()` (single source — shared shape across chat + CLI); the
//! handler delegates to `block_utils::run_skill`. No host calls — runs entirely inside
//! the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default)]
    query: String,
    #[serde(default)]
    projection: String,
    #[serde(default)]
    sort: String,
    #[serde(default)]
    skip: u32,
    #[serde(default)]
    limit: u32,
    #[serde(default)]
    format: String,
    #[serde(default = "yes")]
    pretty: bool,
}

fn yes() -> bool {
    true
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe("The documents to query: a JSON array of objects, e.g. [{\"name\":\"Ada\",\"age\":36},{\"name\":\"Bo\",\"age\":24}]. A single JSON object is treated as a one-document collection, and NDJSON / JSON Lines (one JSON value per line) is accepted too. Maximum 5000000 bytes and 50000 documents."),
        )
        .param(
            Param::string("query")
                .default("{}")
                .describe("The MongoDB find filter, e.g. {\"age\":{\"$gt\":21},\"status\":\"active\"}; blank or {} matches every document. Supported operators: $eq $ne $gt $gte $lt $lte $in $nin $exists $type $regex/$options $mod $all $size $elemMatch $not $and $or $nor. Dotted paths (team.name) work and a predicate on an array field matches when ANY element matches. Relaxed shell syntax is accepted: unquoted keys, single quotes, // and /* */ comments, trailing commas, /pattern/flags regex literals and the ObjectId()/ISODate()/NumberLong() helpers. $where, $expr, $jsonSchema, $text, geospatial and bitwise operators are rejected with a message. Maximum 20000 bytes."),
        )
        .param(
            Param::string("projection")
                .default("")
                .describe("Fields to keep or drop; blank returns whole documents. Mongo form: {\"name\":1,\"email\":1,\"_id\":0} to keep, {\"password\":0} to drop. Short form: 'name, email' to keep and '-password' to drop. Dotted paths (team.name) select nested fields. As in MongoDB, _id is kept by a keep-projection unless you set _id to 0, and mixing kept and dropped fields (other than _id) is an error."),
        )
        .param(
            Param::string("sort")
                .default("")
                .describe("Sort order for the matched documents; blank keeps the input order. Mongo form: {\"age\":-1,\"name\":1} (1 ascending, -1 descending). Short form: 'age:desc, name' (a leading '-' also means descending). Values sort in MongoDB's cross-type order: missing/null first, then numbers, strings, objects, arrays, booleans."),
        )
        .param(
            Param::integer("skip")
                .default(0)
                .min(0.0)
                .describe("Number of matching documents to skip before returning any, like cursor.skip(). Default 0. Combine with 'limit' to page through results."),
        )
        .param(
            Param::integer("limit")
                .default(0)
                .min(0.0)
                .describe("Maximum number of documents to return, like cursor.limit(); 0 (default) returns every match. The 'count' format always reports the FULL match count, ignoring skip and limit."),
        )
        .param(
            Param::enumv("format", ["json", "ndjson", "csv", "count"])
                .default("json")
                .describe("Output format. 'json' (default): a JSON array of the matching documents. 'ndjson': one compact JSON document per line. 'csv': RFC-4180 CSV whose columns are the first-seen union of the returned documents' top-level keys (nested values are written as JSON). 'count': just the number of matching documents."),
        )
        .param(
            Param::boolean("pretty")
                .default(true)
                .describe("Indent the 'json' output for reading (default true). Turn it off for a single compact line. Ignored by the ndjson, csv and count formats."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct MongoQuery;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/mongo-query",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Filter a JSON array of documents with a MongoDB-style query document.",
    skill(
        description = "Run a MongoDB find filter against a pasted JSON array of documents and return the matches. 'data' is a JSON array of objects (a single object or NDJSON is accepted too); 'query' is the Mongo filter, e.g. {\"age\":{\"$gt\":21},\"tags\":{\"$in\":[\"code\"]}}. Supports $eq $ne $gt $gte $lt $lte $in $nin $exists $type $regex/$options $mod $all $size $elemMatch $not $and $or $nor, dotted paths, and MongoDB's array semantics (a predicate on an array field matches when any element matches) and missing-field semantics ({f: null}, $ne, $nin and $not all match documents lacking the field). Relaxed shell syntax (unquoted keys, single quotes, comments, /regex/i literals, ObjectId()/ISODate() helpers) is accepted. 'projection' keeps or drops fields, 'sort'/'skip'/'limit' reproduce the cursor chain, 'format' is json (default), ndjson, csv or count, and 'pretty' indents the JSON. $where, $expr, $jsonSchema, $text, geospatial and bitwise operators are rejected with an explicit message; aggregation and update operators are out of scope.",
        parameters = schema_json()
    ),
)]
impl MongoQuery {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "mongo-query", |a: Args| {
            gizza_ai_mongo_query_core::run(
                &a.data,
                &a.query,
                &a.projection,
                &a.sort,
                a.skip as usize,
                a.limit as usize,
                &a.format,
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
                    "data": { "type": "string", "description": "The documents to query: a JSON array of objects, e.g. [{\"name\":\"Ada\",\"age\":36},{\"name\":\"Bo\",\"age\":24}]. A single JSON object is treated as a one-document collection, and NDJSON / JSON Lines (one JSON value per line) is accepted too. Maximum 5000000 bytes and 50000 documents." },
                    "query": { "type": "string", "default": "{}", "description": "The MongoDB find filter, e.g. {\"age\":{\"$gt\":21},\"status\":\"active\"}; blank or {} matches every document. Supported operators: $eq $ne $gt $gte $lt $lte $in $nin $exists $type $regex/$options $mod $all $size $elemMatch $not $and $or $nor. Dotted paths (team.name) work and a predicate on an array field matches when ANY element matches. Relaxed shell syntax is accepted: unquoted keys, single quotes, // and /* */ comments, trailing commas, /pattern/flags regex literals and the ObjectId()/ISODate()/NumberLong() helpers. $where, $expr, $jsonSchema, $text, geospatial and bitwise operators are rejected with a message. Maximum 20000 bytes." },
                    "projection": { "type": "string", "default": "", "description": "Fields to keep or drop; blank returns whole documents. Mongo form: {\"name\":1,\"email\":1,\"_id\":0} to keep, {\"password\":0} to drop. Short form: 'name, email' to keep and '-password' to drop. Dotted paths (team.name) select nested fields. As in MongoDB, _id is kept by a keep-projection unless you set _id to 0, and mixing kept and dropped fields (other than _id) is an error." },
                    "sort": { "type": "string", "default": "", "description": "Sort order for the matched documents; blank keeps the input order. Mongo form: {\"age\":-1,\"name\":1} (1 ascending, -1 descending). Short form: 'age:desc, name' (a leading '-' also means descending). Values sort in MongoDB's cross-type order: missing/null first, then numbers, strings, objects, arrays, booleans." },
                    "skip": { "type": "integer", "minimum": 0, "default": 0, "description": "Number of matching documents to skip before returning any, like cursor.skip(). Default 0. Combine with 'limit' to page through results." },
                    "limit": { "type": "integer", "minimum": 0, "default": 0, "description": "Maximum number of documents to return, like cursor.limit(); 0 (default) returns every match. The 'count' format always reports the FULL match count, ignoring skip and limit." },
                    "format": { "type": "string", "enum": ["json", "ndjson", "csv", "count"], "default": "json", "description": "Output format. 'json' (default): a JSON array of the matching documents. 'ndjson': one compact JSON document per line. 'csv': RFC-4180 CSV whose columns are the first-seen union of the returned documents' top-level keys (nested values are written as JSON). 'count': just the number of matching documents." },
                    "pretty": { "type": "boolean", "default": true, "description": "Indent the 'json' output for reading (default true). Turn it off for a single compact line. Ignored by the ndjson, csv and count formats." }
                },
                "required": ["data"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
