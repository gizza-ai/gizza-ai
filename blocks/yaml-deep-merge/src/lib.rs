//! gizza-ai/yaml-deep-merge — deep-merge a `---`-separated stream of YAML
//! documents left to right, the way Helm layers values files. The chat schema is
//! single-sourced from descriptor() (which also drives the CLI); handle()
//! delegates to block_utils::run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_yaml_deep_merge_core::merge_documents;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    documents: String,
    #[serde(default = "default_precedence")]
    precedence: String,
    #[serde(default = "default_object_merge")]
    object_merge: String,
    #[serde(default = "default_array_merge")]
    array_merge: String,
    #[serde(default = "default_array_key")]
    array_key: String,
    #[serde(default = "default_true")]
    null_deletes: bool,
    #[serde(default)]
    sort_keys: bool,
    #[serde(default = "default_indent")]
    indent: u64,
}

fn default_precedence() -> String {
    "last".to_string()
}
fn default_object_merge() -> String {
    "deep".to_string()
}
fn default_array_merge() -> String {
    "replace".to_string()
}
fn default_array_key() -> String {
    "name".to_string()
}
fn default_true() -> bool {
    true
}
fn default_indent() -> u64 {
    2
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("documents").required().describe(
            "Two or more YAML documents to merge, separated by a line containing '---', e.g. 'image:\\n  tag: 1.0\\n---\\nimage:\\n  tag: 2.0'. Merged left-to-right like Helm values files: the rightmost document has the final say. Max 20 documents / 1 MiB.",
        ))
        .param(
            Param::enumv("precedence", ["last", "first", "error"])
                .default("last")
                .describe(
                    "Who wins when two documents set the same key to different values: 'last' (default, Helm's rule) = the later document; 'first' = keep the base value and only let later documents add new keys; 'error' = refuse to merge and report the conflicting key path.",
                ),
        )
        .param(
            Param::enumv("object_merge", ["deep", "shallow"])
                .default("deep")
                .describe(
                    "How nested mappings combine: 'deep' (default) merges them recursively, key by key; 'shallow' merges only top-level keys and replaces the whole value below them.",
                ),
        )
        .param(
            Param::enumv("array_merge", ["replace", "append", "unique", "by_key"])
                .default("replace")
                .describe(
                    "How lists combine: 'replace' (default, Helm's rule) = the winning document's list replaces the other; 'append' = concatenate; 'unique' = concatenate then drop duplicate items; 'by_key' = merge items that share the same array_key field (e.g. Kubernetes 'env'/'containers' entries matched on 'name').",
                ),
        )
        .param(
            Param::string("array_key")
                .default("name")
                .describe(
                    "Field name used to line list items up when array_merge=by_key, e.g. 'name' (default) for Kubernetes env/containers lists, or 'id'. Ignored by the other array strategies.",
                ),
        )
        .param(
            Param::boolean("null_deletes")
                .default(true)
                .describe(
                    "true (default) = a key set to null in a later document REMOVES it from the result, matching how Helm deletes a default key; false = null is kept as an ordinary value. Applies in every precedence mode.",
                ),
        )
        .param(
            Param::boolean("sort_keys")
                .default(false)
                .describe(
                    "false (default) = keep the base document's key order; true = sort every mapping's keys A-Z, recursively (handy for diffing two merged results).",
                ),
        )
        .param(
            Param::integer("indent")
                .min(1.0)
                .max(8.0)
                .default(2)
                .describe("Output indentation in spaces per nesting level (1-8). Default 2."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn merge(a: &Args) -> Result<String, String> {
    merge_documents(
        &a.documents,
        &a.precedence,
        &a.object_merge,
        &a.array_merge,
        &a.array_key,
        a.null_deletes,
        a.sort_keys,
        a.indent as usize,
    )
}

#[cfg(target_arch = "wasm32")]
struct YamlDeepMerge;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/yaml-deep-merge",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Deep-merge YAML documents with Helm-style override semantics",
    skill(
        description = "Deep-merge two or more YAML documents (pasted as one '---' separated stream) left-to-right, the way Helm layers values files: mappings merge recursively, the rightmost document wins a conflict, and a key set to null in a later document is deleted. Lists can be replaced (default), appended, de-duplicated, or merged item-by-item on a shared key such as 'name'. Precedence can be flipped to first-wins or set to error to report conflicts instead of resolving them. Output indentation and key sorting are configurable. Returns the merged YAML. Comments and anchors are not preserved. Max 20 documents / 1 MiB. Runs locally.",
        parameters = schema_json()
    ),
)]
impl YamlDeepMerge {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "yaml-deep-merge", |a: Args| {
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

    fn args(documents: &str) -> Args {
        Args {
            documents: documents.to_string(),
            precedence: default_precedence(),
            object_merge: default_object_merge(),
            array_merge: default_array_merge(),
            array_key: default_array_key(),
            null_deletes: true,
            sort_keys: false,
            indent: 2,
        }
    }

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "documents":    { "type": "string", "description": "Two or more YAML documents to merge, separated by a line containing '---', e.g. 'image:\\n  tag: 1.0\\n---\\nimage:\\n  tag: 2.0'. Merged left-to-right like Helm values files: the rightmost document has the final say. Max 20 documents / 1 MiB." },
                    "precedence":   { "type": "string", "enum": ["last", "first", "error"], "default": "last", "description": "Who wins when two documents set the same key to different values: 'last' (default, Helm's rule) = the later document; 'first' = keep the base value and only let later documents add new keys; 'error' = refuse to merge and report the conflicting key path." },
                    "object_merge": { "type": "string", "enum": ["deep", "shallow"], "default": "deep", "description": "How nested mappings combine: 'deep' (default) merges them recursively, key by key; 'shallow' merges only top-level keys and replaces the whole value below them." },
                    "array_merge":  { "type": "string", "enum": ["replace", "append", "unique", "by_key"], "default": "replace", "description": "How lists combine: 'replace' (default, Helm's rule) = the winning document's list replaces the other; 'append' = concatenate; 'unique' = concatenate then drop duplicate items; 'by_key' = merge items that share the same array_key field (e.g. Kubernetes 'env'/'containers' entries matched on 'name')." },
                    "array_key":    { "type": "string", "default": "name", "description": "Field name used to line list items up when array_merge=by_key, e.g. 'name' (default) for Kubernetes env/containers lists, or 'id'. Ignored by the other array strategies." },
                    "null_deletes": { "type": "boolean", "default": true, "description": "true (default) = a key set to null in a later document REMOVES it from the result, matching how Helm deletes a default key; false = null is kept as an ordinary value. Applies in every precedence mode." },
                    "sort_keys":    { "type": "boolean", "default": false, "description": "false (default) = keep the base document's key order; true = sort every mapping's keys A-Z, recursively (handy for diffing two merged results)." },
                    "indent":       { "type": "integer", "minimum": 1, "maximum": 8, "default": 2, "description": "Output indentation in spaces per nesting level (1-8). Default 2." }
                },
                "required": ["documents"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn merges_with_defaults() {
        let out =
            merge(&args("image:\n  repo: app\n  tag: \"1.0\"\n---\nimage:\n  tag: \"2.0\"\n"))
                .unwrap();
        assert_eq!(out, "image:\n  repo: app\n  tag: \"2.0\"\n");
    }

    #[test]
    fn honours_array_and_precedence_params() {
        let mut a = args("ports: [80]\n---\nports: [443]\n");
        a.array_merge = "append".to_string();
        assert_eq!(merge(&a).unwrap(), "ports:\n  - 80\n  - 443\n");

        let mut b = args("a: 1\n---\na: 2\n");
        b.precedence = "first".to_string();
        assert_eq!(merge(&b).unwrap(), "a: 1\n");
    }

    #[test]
    fn rejects_unknown_enum_value() {
        let mut a = args("a: 1\n");
        a.array_merge = "sideways".to_string();
        assert!(merge(&a).unwrap_err().contains("unknown array_merge"));
    }
}
