//! gizza-ai/elasticsearch-mapping-generator — infer an Elasticsearch index
//! mapping from a sample JSON document (or an array of sample documents).
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_elasticsearch_mapping_generator_core::{generate, Options};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    json: String,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default = "default_text_fields")]
    text_fields: String,
    #[serde(default = "default_ignore_above")]
    ignore_above: i64,
    #[serde(default)]
    analyzer: String,
    #[serde(default = "default_integer_type")]
    integer_type: String,
    #[serde(default = "default_float_type")]
    float_type: String,
    #[serde(default = "default_true")]
    date_detection: bool,
    #[serde(default)]
    numeric_detection: bool,
    #[serde(default)]
    detect_ip: bool,
    #[serde(default)]
    detect_geo_point: bool,
    #[serde(default = "default_array_objects")]
    array_objects: String,
    #[serde(default = "default_dynamic")]
    dynamic: String,
    #[serde(default = "default_one")]
    shards: i64,
    #[serde(default = "default_one")]
    replicas: i64,
}

fn default_output() -> String {
    "mappings".to_string()
}
fn default_text_fields() -> String {
    "text_keyword".to_string()
}
fn default_ignore_above() -> i64 {
    256
}
fn default_integer_type() -> String {
    "long".to_string()
}
fn default_float_type() -> String {
    "float".to_string()
}
fn default_array_objects() -> String {
    "object".to_string()
}
fn default_dynamic() -> String {
    "true".to_string()
}
fn default_true() -> bool {
    true
}
fn default_one() -> i64 {
    1
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("json")
                .required()
                .describe("A sample JSON document to infer the mapping from, e.g. {\"id\":1,\"title\":\"Hello\"}. Pass a JSON ARRAY of documents to merge several samples: every key seen in any document is mapped, and a field seen as both a whole and a fractional number widens to the float type."),
        )
        .param(
            Param::enumv("output", ["mappings", "create-index", "properties"])
                .default("mappings")
                .describe("What to wrap the inferred fields in. 'mappings' (default) emits {\"mappings\":{...}}; 'create-index' adds a settings block with shards/replicas so the whole body can be PUT to create an index; 'properties' emits only {\"properties\":{...}}."),
        )
        .param(
            Param::enumv("text_fields", ["text_keyword", "keyword", "text"])
                .default("text_keyword")
                .describe("How plain strings are mapped. 'text_keyword' (default) is Elasticsearch's own choice: a 'text' field with a '.keyword' sub-field for exact matching, sorting and aggregations. 'keyword' emits an exact-match-only field; 'text' emits a full-text-only field."),
        )
        .param(
            Param::integer("ignore_above")
                .default(256)
                .min(0.0)
                .max(32766.0)
                .describe("Maximum string length indexed by keyword fields; longer values are stored but not indexed. Default 256 (Elasticsearch's own default). Use 0 to omit ignore_above entirely. Only applies when text_fields includes keyword."),
        )
        .param(
            Param::string("analyzer")
                .default("")
                .describe("Analyzer name to set on every text field, e.g. 'english', 'french' or a custom analyzer you define in the index settings. Empty (default) leaves the standard analyzer in place. Ignored when text_fields is 'keyword'."),
        )
        .param(
            Param::enumv("integer_type", ["long", "integer", "short"])
                .default("long")
                .describe("Elasticsearch type for whole numbers. 'long' (default) matches dynamic mapping; 'integer' or 'short' save space when you know the value range."),
        )
        .param(
            Param::enumv("float_type", ["float", "double", "half_float"])
                .default("float")
                .describe("Elasticsearch type for fractional numbers. 'float' (default) matches dynamic mapping; 'double' for higher precision, 'half_float' for compact low-precision values such as scores."),
        )
        .param(
            Param::boolean("date_detection")
                .default(true)
                .describe("Map strings that look like dates to the 'date' type. Default true (matches Elasticsearch). Recognises ISO-8601 (2020-01-02, 2020-01-02T03:04:05Z, with optional fraction and offset) and slash dates (2020/01/02), which also get an explicit format. Turn off to keep every string as text/keyword."),
        )
        .param(
            Param::boolean("numeric_detection")
                .default(false)
                .describe("Map strings that hold numbers (\"42\", \"3.14\") to the numeric types instead of text. Default false, matching Elasticsearch. Date detection is applied first."),
        )
        .param(
            Param::boolean("detect_ip")
                .default(false)
                .describe("Map strings that are valid IPv4/IPv6 addresses to the 'ip' type, which supports CIDR range queries. Default false. Elasticsearch's dynamic mapping never does this, so it is off unless you ask."),
        )
        .param(
            Param::boolean("detect_geo_point")
                .default(false)
                .describe("Map objects that hold exactly a numeric 'lat' and 'lon' to the 'geo_point' type instead of an object with two number fields. Default false. Enables geo distance/bounding-box queries."),
        )
        .param(
            Param::enumv("array_objects", ["object", "nested"])
                .default("object")
                .describe("How an array of objects is mapped. 'object' (default) matches Elasticsearch and flattens the array, so two sub-fields can match across DIFFERENT elements. 'nested' indexes each element separately so nested queries match within one element, at a higher indexing cost."),
        )
        .param(
            Param::enumv("dynamic", ["true", "false", "strict", "runtime"])
                .default("true")
                .describe("Policy for fields that were not in the sample. 'true' (default) maps and indexes them; 'false' stores them without indexing; 'strict' rejects documents containing them; 'runtime' adds them as runtime fields, queryable but not indexed."),
        )
        .param(
            Param::integer("shards")
                .default(1)
                .min(1.0)
                .max(1024.0)
                .describe("number_of_shards for the settings block. Default 1. Only used when output is 'create-index'."),
        )
        .param(
            Param::integer("replicas")
                .default(1)
                .min(0.0)
                .max(100.0)
                .describe("number_of_replicas for the settings block. Default 1; use 0 for a single-node development cluster. Only used when output is 'create-index'."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn options_from(a: &Args) -> Options {
    Options {
        output: a.output.clone(),
        text_fields: a.text_fields.clone(),
        ignore_above: a.ignore_above,
        analyzer: a.analyzer.clone(),
        integer_type: a.integer_type.clone(),
        float_type: a.float_type.clone(),
        date_detection: a.date_detection,
        numeric_detection: a.numeric_detection,
        detect_ip: a.detect_ip,
        detect_geo_point: a.detect_geo_point,
        array_objects: a.array_objects.clone(),
        dynamic: a.dynamic.clone(),
        shards: a.shards,
        replicas: a.replicas,
    }
}

#[cfg(target_arch = "wasm32")]
struct ElasticsearchMappingGenerator;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/elasticsearch-mapping-generator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Infer an Elasticsearch index mapping from a sample JSON document",
    skill(
        description = "Infer an Elasticsearch index mapping from a sample JSON document, or from a JSON array of documents which are merged field by field. Follows Elasticsearch's own dynamic mapping rules as the baseline: booleans map to boolean, whole numbers to long, fractional numbers to float, objects recurse into object + properties, arrays take their element type, and null values and empty arrays add no field. Strings become text with a .keyword sub-field (ignore_above 256) unless date detection (on by default, ISO-8601 and slash dates) or numeric detection (off by default) claims them. Options: output (mappings default, create-index for a full PUT body with shards/replicas, or properties only), text_fields (text_keyword default, keyword, text), ignore_above, analyzer for text fields, integer_type/float_type widths, date_detection, numeric_detection, detect_ip (IPv4/IPv6 to the ip type), detect_geo_point ({lat,lon} objects to geo_point), array_objects (object default or nested) and dynamic (true default, false, strict, runtime). Returns the pretty-printed mapping JSON with properties sorted alphabetically.",
        parameters = schema_json()
    ),
)]
impl ElasticsearchMappingGenerator {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "elasticsearch-mapping-generator", |a: Args| {
            generate(&a.json, &options_from(&a)).map_err(SkillError::InvalidArgs)
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
                    "json":             { "type": "string", "description": "A sample JSON document to infer the mapping from, e.g. {\"id\":1,\"title\":\"Hello\"}. Pass a JSON ARRAY of documents to merge several samples: every key seen in any document is mapped, and a field seen as both a whole and a fractional number widens to the float type." },
                    "output":           { "type": "string", "enum": ["mappings", "create-index", "properties"], "default": "mappings", "description": "What to wrap the inferred fields in. 'mappings' (default) emits {\"mappings\":{...}}; 'create-index' adds a settings block with shards/replicas so the whole body can be PUT to create an index; 'properties' emits only {\"properties\":{...}}." },
                    "text_fields":      { "type": "string", "enum": ["text_keyword", "keyword", "text"], "default": "text_keyword", "description": "How plain strings are mapped. 'text_keyword' (default) is Elasticsearch's own choice: a 'text' field with a '.keyword' sub-field for exact matching, sorting and aggregations. 'keyword' emits an exact-match-only field; 'text' emits a full-text-only field." },
                    "ignore_above":     { "type": "integer", "default": 256, "minimum": 0, "maximum": 32766, "description": "Maximum string length indexed by keyword fields; longer values are stored but not indexed. Default 256 (Elasticsearch's own default). Use 0 to omit ignore_above entirely. Only applies when text_fields includes keyword." },
                    "analyzer":         { "type": "string", "default": "", "description": "Analyzer name to set on every text field, e.g. 'english', 'french' or a custom analyzer you define in the index settings. Empty (default) leaves the standard analyzer in place. Ignored when text_fields is 'keyword'." },
                    "integer_type":     { "type": "string", "enum": ["long", "integer", "short"], "default": "long", "description": "Elasticsearch type for whole numbers. 'long' (default) matches dynamic mapping; 'integer' or 'short' save space when you know the value range." },
                    "float_type":       { "type": "string", "enum": ["float", "double", "half_float"], "default": "float", "description": "Elasticsearch type for fractional numbers. 'float' (default) matches dynamic mapping; 'double' for higher precision, 'half_float' for compact low-precision values such as scores." },
                    "date_detection":   { "type": "boolean", "default": true, "description": "Map strings that look like dates to the 'date' type. Default true (matches Elasticsearch). Recognises ISO-8601 (2020-01-02, 2020-01-02T03:04:05Z, with optional fraction and offset) and slash dates (2020/01/02), which also get an explicit format. Turn off to keep every string as text/keyword." },
                    "numeric_detection":{ "type": "boolean", "default": false, "description": "Map strings that hold numbers (\"42\", \"3.14\") to the numeric types instead of text. Default false, matching Elasticsearch. Date detection is applied first." },
                    "detect_ip":        { "type": "boolean", "default": false, "description": "Map strings that are valid IPv4/IPv6 addresses to the 'ip' type, which supports CIDR range queries. Default false. Elasticsearch's dynamic mapping never does this, so it is off unless you ask." },
                    "detect_geo_point": { "type": "boolean", "default": false, "description": "Map objects that hold exactly a numeric 'lat' and 'lon' to the 'geo_point' type instead of an object with two number fields. Default false. Enables geo distance/bounding-box queries." },
                    "array_objects":    { "type": "string", "enum": ["object", "nested"], "default": "object", "description": "How an array of objects is mapped. 'object' (default) matches Elasticsearch and flattens the array, so two sub-fields can match across DIFFERENT elements. 'nested' indexes each element separately so nested queries match within one element, at a higher indexing cost." },
                    "dynamic":          { "type": "string", "enum": ["true", "false", "strict", "runtime"], "default": "true", "description": "Policy for fields that were not in the sample. 'true' (default) maps and indexes them; 'false' stores them without indexing; 'strict' rejects documents containing them; 'runtime' adds them as runtime fields, queryable but not indexed." },
                    "shards":           { "type": "integer", "default": 1, "minimum": 1, "maximum": 1024, "description": "number_of_shards for the settings block. Default 1. Only used when output is 'create-index'." },
                    "replicas":         { "type": "integer", "default": 1, "minimum": 0, "maximum": 100, "description": "number_of_replicas for the settings block. Default 1; use 0 for a single-node development cluster. Only used when output is 'create-index'." }
                },
                "required": ["json"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
