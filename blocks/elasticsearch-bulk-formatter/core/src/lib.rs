//! elasticsearch-bulk-formatter core — build the newline-delimited JSON (NDJSON)
//! request body for the Elasticsearch `_bulk` API from a JSON array of documents.
//!
//! Pure Rust (serde_json only) → runs on every backend, including the chat
//! Service Worker. No network, no Elasticsearch client: it only *shapes* the
//! request body so you can paste it into `curl -H 'Content-Type:
//! application/x-ndjson' --data-binary @body`, Kibana Dev Tools, or a client.
//!
//! The `_bulk` body is a sequence of lines:
//!   * an action/metadata line — `{ "<action>": { "_index": …, "_id": … } }`
//!   * for `index`/`create`/`update`, a following source line (the document,
//!     or `{ "doc": … }` for update); `delete` has NO source line.
//! Every line is compact (literal `\n` is the delimiter — no pretty-printing)
//! and the body MUST end with a trailing newline.

use serde_json::{Map, Value};

/// The four `_bulk` sub-operations.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Action {
    Index,
    Create,
    Update,
    Delete,
}

impl Action {
    fn parse(s: &str) -> Result<Action, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "index" => Ok(Action::Index),
            "create" => Ok(Action::Create),
            "update" => Ok(Action::Update),
            "delete" => Ok(Action::Delete),
            other => Err(format!(
                "invalid action {other:?}: expected \"index\", \"create\", \"update\", or \"delete\""
            )),
        }
    }

    /// The JSON key used on the action/metadata line.
    fn key(self) -> &'static str {
        match self {
            Action::Index => "index",
            Action::Create => "create",
            Action::Update => "update",
            Action::Delete => "delete",
        }
    }

    /// Whether this action needs `_id` present on every document.
    fn requires_id(self) -> bool {
        matches!(self, Action::Update | Action::Delete)
    }

    /// Whether this action emits a source line after its metadata line.
    fn has_source(self) -> bool {
        !matches!(self, Action::Delete)
    }
}

/// Build the `_bulk` NDJSON body from a JSON array of documents.
///
/// * `documents` — a JSON array of objects. Each object is one bulk operation.
/// * `action` — `index` | `create` | `update` | `delete`.
/// * `index` — target index name written to `_index` in every metadata line;
///   empty ⇒ omit `_index` (the caller targets `/<index>/_bulk`).
/// * `id_field` — name of a field whose value becomes `_id`; that field is
///   stripped from the emitted source (a document shouldn't repeat its `_id`).
///   Required for `update`/`delete`; optional for `index`/`create`.
/// * `doc_as_upsert` — for `update`, add `"doc_as_upsert": true` so a missing
///   document is inserted from `doc`. Ignored for other actions.
///
/// Returns the NDJSON body, always terminated by a trailing `\n`.
pub fn run(
    documents: &str,
    action: &str,
    index: &str,
    id_field: &str,
    doc_as_upsert: bool,
) -> Result<String, String> {
    let action = Action::parse(action)?;
    let index = index.trim();
    let id_field = id_field.trim();

    if action.requires_id() && id_field.is_empty() {
        return Err(format!(
            "action \"{}\" needs a document _id — set id_field to the field that holds each document's id",
            action.key()
        ));
    }

    let parsed: Value = serde_json::from_str(documents.trim())
        .map_err(|e| format!("documents is not valid JSON: {e}"))?;
    let items = parsed.as_array().ok_or_else(|| {
        "documents must be a JSON array of objects (e.g. [ { ... } ])".to_string()
    })?;

    if items.is_empty() {
        return Err("documents is an empty array — provide at least one document".into());
    }

    let mut out = String::new();
    for (i, item) in items.iter().enumerate() {
        let obj = item.as_object().ok_or_else(|| {
            format!(
                "document {i} must be a JSON object (got {})",
                type_name(item)
            )
        })?;

        // Extract the id (and a source with the id field removed) when id_field is set.
        let (id, source) = if id_field.is_empty() {
            (None, obj.clone())
        } else {
            let id = obj.get(id_field).cloned();
            let mut src = obj.clone();
            src.remove(id_field);
            (id, src)
        };

        if action.requires_id() && id.as_ref().map_or(true, |v| v.is_null()) {
            return Err(format!(
                "document {i} is missing id field {id_field:?} required for the \"{}\" action",
                action.key()
            ));
        }

        // Action/metadata line: { "<action>": { "_index": …, "_id": … } }.
        let mut meta = Map::new();
        if !index.is_empty() {
            meta.insert("_index".into(), Value::String(index.to_string()));
        }
        if let Some(id) = id {
            meta.insert("_id".into(), id);
        }
        let mut action_obj = Map::new();
        action_obj.insert(action.key().into(), Value::Object(meta));
        write_line(&mut out, &Value::Object(action_obj))?;

        // Source line (index/create/update; delete has none).
        if action.has_source() {
            let source_value = if action == Action::Update {
                let mut wrap = Map::new();
                wrap.insert("doc".into(), Value::Object(source));
                if doc_as_upsert {
                    wrap.insert("doc_as_upsert".into(), Value::Bool(true));
                }
                Value::Object(wrap)
            } else {
                Value::Object(source)
            };
            write_line(&mut out, &source_value)?;
        }
    }

    Ok(out)
}

/// Serialize one value compactly and append it followed by `\n`.
fn write_line(out: &mut String, v: &Value) -> Result<(), String> {
    let line = serde_json::to_string(v).map_err(|e| format!("failed to serialize output: {e}"))?;
    out.push_str(&line);
    out.push('\n');
    Ok(())
}

fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn index_with_id_field_strips_id_from_source() {
        let out = run(
            r#"[{"id":"1","title":"hello"},{"id":"2","title":"world"}]"#,
            "index",
            "my-index",
            "id",
            false,
        )
        .unwrap();
        assert_eq!(
            out,
            "{\"index\":{\"_index\":\"my-index\",\"_id\":\"1\"}}\n\
             {\"title\":\"hello\"}\n\
             {\"index\":{\"_index\":\"my-index\",\"_id\":\"2\"}}\n\
             {\"title\":\"world\"}\n"
        );
        assert!(out.ends_with('\n'), "body must end with a trailing newline");
    }

    #[test]
    fn index_without_index_or_id_omits_metadata() {
        let out = run(r#"[{"a":1}]"#, "index", "", "", false).unwrap();
        assert_eq!(out, "{\"index\":{}}\n{\"a\":1}\n");
    }

    #[test]
    fn create_action_uses_create_key() {
        let out = run(r#"[{"ref":"x","v":true}]"#, "create", "logs", "ref", false).unwrap();
        assert_eq!(
            out,
            "{\"create\":{\"_index\":\"logs\",\"_id\":\"x\"}}\n{\"v\":true}\n"
        );
    }

    #[test]
    fn update_wraps_doc_and_upsert() {
        let out = run(
            r#"[{"id":"7","status":"done"}]"#,
            "update",
            "tasks",
            "id",
            true,
        )
        .unwrap();
        assert_eq!(
            out,
            "{\"update\":{\"_index\":\"tasks\",\"_id\":\"7\"}}\n{\"doc\":{\"status\":\"done\"},\"doc_as_upsert\":true}\n"
        );
    }

    #[test]
    fn update_without_upsert_omits_flag() {
        let out = run(
            r#"[{"id":"7","status":"done"}]"#,
            "update",
            "tasks",
            "id",
            false,
        )
        .unwrap();
        assert_eq!(
            out,
            "{\"update\":{\"_index\":\"tasks\",\"_id\":\"7\"}}\n{\"doc\":{\"status\":\"done\"}}\n"
        );
    }

    #[test]
    fn delete_emits_no_source_line() {
        let out = run(r#"[{"id":"3"},{"id":"4"}]"#, "delete", "docs", "id", false).unwrap();
        assert_eq!(
            out,
            "{\"delete\":{\"_index\":\"docs\",\"_id\":\"3\"}}\n{\"delete\":{\"_index\":\"docs\",\"_id\":\"4\"}}\n"
        );
    }

    #[test]
    fn numeric_id_is_preserved_as_json() {
        let out = run(r#"[{"id":42,"a":"b"}]"#, "index", "n", "id", false).unwrap();
        assert_eq!(
            out,
            "{\"index\":{\"_index\":\"n\",\"_id\":42}}\n{\"a\":\"b\"}\n"
        );
    }

    #[test]
    fn err_on_invalid_json() {
        let e = run("not json", "index", "i", "", false).unwrap_err();
        assert!(e.contains("not valid JSON"), "got: {e}");
    }

    #[test]
    fn err_on_non_array_root() {
        let e = run(r#"{"a":1}"#, "index", "i", "", false).unwrap_err();
        assert!(e.contains("must be a JSON array"), "got: {e}");
    }

    #[test]
    fn err_on_non_object_item() {
        let e = run(r#"[1,2]"#, "index", "i", "", false).unwrap_err();
        assert!(e.contains("must be a JSON object"), "got: {e}");
    }

    #[test]
    fn err_on_empty_array() {
        let e = run(r#"[]"#, "index", "i", "", false).unwrap_err();
        assert!(e.contains("empty array"), "got: {e}");
    }

    #[test]
    fn err_on_invalid_action() {
        let e = run(r#"[{"a":1}]"#, "upsert", "i", "", false).unwrap_err();
        assert!(e.contains("invalid action"), "got: {e}");
    }

    #[test]
    fn err_when_delete_missing_id_field() {
        let e = run(r#"[{"a":1}]"#, "delete", "i", "", false).unwrap_err();
        assert!(e.contains("needs a document _id"), "got: {e}");
    }

    #[test]
    fn err_when_document_missing_id_value() {
        let e = run(r#"[{"other":1}]"#, "update", "i", "id", false).unwrap_err();
        assert!(e.contains("missing id field"), "got: {e}");
    }
}
