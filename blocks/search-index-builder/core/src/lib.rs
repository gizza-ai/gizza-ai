//! search-index-builder core — pure, deterministic full-text search-index builder.
//! No wafer/wasm-bindgen deps. Shared by the chat skill block and the web page.
//!
//! Takes a JSON array of document objects (the in-model stand-in for a "folder of
//! documents" — a static-site build step reads the folder and pastes the array
//! here) and produces a serialized **inverted index** JSON that a static site can
//! load to search offline. Every indexed field is tokenized, and the index maps
//! each token to the documents it appears in, per field, with a document
//! frequency (`df`) and per-document term frequency (`postings`) — everything a
//! client needs to rank results with TF-IDF. An optional document store carries
//! display fields (title, url, …) alongside the index, and optional per-field
//! boosts are recorded for the query-time ranker.
//!
//! The shape mirrors elasticlunr / MiniSearch closely enough to be legible while
//! staying pure and deterministic (BTreeMap ordering everywhere, so the CLI/page
//! output is stable and testable).

use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::BTreeMap;

/// Hard cap on `min_length` (also the page/schema max).
pub const MIN_LENGTH_CAP: usize = 20;

/// A small, conventional English stop-word list, applied only when
/// `remove_stopwords` is set. Kept short and uncontroversial on purpose.
const STOP_WORDS: &[&str] = &[
    "a", "an", "and", "are", "as", "at", "be", "but", "by", "for", "if", "in", "into", "is", "it",
    "no", "not", "of", "on", "or", "over", "such", "that", "the", "their", "then", "there",
    "these", "they", "this", "to", "was", "will", "with",
];

/// One token's postings within a single field.
#[derive(Debug, Clone, PartialEq, Serialize)]
struct FieldPostings {
    /// Document frequency: how many documents contain this token in this field.
    df: usize,
    /// document ref -> term frequency (count of the token in that document's field).
    postings: BTreeMap<String, u32>,
}

/// The serialized inverted index (what every surface returns).
#[derive(Debug, Clone, PartialEq, Serialize)]
struct IndexOutput {
    /// Number of documents indexed.
    #[serde(rename = "documentCount")]
    document_count: usize,
    /// The document fields that were tokenized into the index, sorted.
    fields: Vec<String>,
    /// The field used as each document's ref.
    #[serde(rename = "ref")]
    ref_field: String,
    /// Number of distinct tokens (index terms).
    #[serde(rename = "tokenCount")]
    token_count: usize,
    /// Per-field query-time ranking weights, present only when `boosts` was given.
    #[serde(rename = "boosts", skip_serializing_if = "BTreeMap::is_empty")]
    boosts: BTreeMap<String, f64>,
    /// The inverted index: token -> field -> { df, postings: { ref -> tf } }.
    index: BTreeMap<String, BTreeMap<String, FieldPostings>>,
    /// The document store: ref -> the chosen display fields for that document.
    documents: BTreeMap<String, Map<String, Value>>,
}

/// Split a comma-separated option list into trimmed, non-empty names.
fn split_list(s: &str) -> Vec<String> {
    s.split(',')
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .map(|p| p.to_string())
        .collect()
}

/// Parse a `field:weight,field2:weight2` boost spec. Unparseable weights error.
fn parse_boosts(s: &str) -> Result<BTreeMap<String, f64>, String> {
    let mut out = BTreeMap::new();
    for pair in s.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let (field, weight) = pair
            .split_once(':')
            .ok_or_else(|| format!("boost {pair:?} must be field:weight, e.g. title:2"))?;
        let field = field.trim();
        if field.is_empty() {
            return Err(format!("boost {pair:?} has an empty field name"));
        }
        let weight: f64 = weight
            .trim()
            .parse()
            .map_err(|_| format!("boost weight in {pair:?} must be a number"))?;
        out.insert(field.to_string(), weight);
    }
    Ok(out)
}

/// Stringify a scalar JSON value for use as a document ref. Objects/arrays/null
/// are not valid refs.
fn scalar_to_ref(v: &Value) -> Option<String> {
    match v {
        Value::String(s) => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

/// Tokenize `text`: split on non-alphanumeric boundaries, optionally lowercase,
/// drop tokens shorter than `min_length`, and (optionally) drop stop words.
fn tokenize(text: &str, lowercase: bool, remove_stopwords: bool, min_length: usize) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| {
            if lowercase {
                t.to_lowercase()
            } else {
                t.to_string()
            }
        })
        .filter(|t| t.chars().count() >= min_length)
        .filter(|t| !(remove_stopwords && STOP_WORDS.contains(&t.as_str())))
        .collect()
}

/// Build a serialized inverted-index JSON from a JSON array of document objects.
///
/// - `documents_json`: a JSON array of objects, e.g.
///   `[{"id":"1","title":"Intro","body":"Hello world"}]`.
/// - `fields`: comma-separated field names to full-text index; empty = every
///   string-valued field found across the documents (except `id_field`).
/// - `id_field`: which field supplies each document's ref; when a document lacks
///   it, the 0-based array position is used. Refs must be unique.
/// - `store_fields`: comma-separated fields to copy into the `documents` store
///   for display alongside results; empty = store nothing.
/// - `boosts`: optional `field:weight` list recorded for the query-time ranker.
/// - `lowercase`: case-fold tokens (recommended for case-insensitive search).
/// - `remove_stopwords`: drop common English stop words.
/// - `min_length`: drop tokens shorter than this many characters (clamped 1..=20).
/// - `pretty`: pretty-print the JSON (else a compact single line).
#[allow(clippy::too_many_arguments)]
pub fn build_index(
    documents_json: &str,
    fields: &str,
    id_field: &str,
    store_fields: &str,
    boosts: &str,
    lowercase: bool,
    remove_stopwords: bool,
    min_length: usize,
    pretty: bool,
) -> Result<String, String> {
    let min_length = min_length.clamp(1, MIN_LENGTH_CAP);
    let id_field = if id_field.trim().is_empty() {
        "id"
    } else {
        id_field.trim()
    };
    let boosts = parse_boosts(boosts)?;

    let parsed: Value = serde_json::from_str(documents_json.trim())
        .map_err(|e| format!("documents must be valid JSON: {e}"))?;
    let docs = match parsed {
        Value::Array(a) => a,
        _ => {
            return Err(
                "documents must be a JSON array of objects, e.g. [{\"id\":\"1\",\"title\":\"…\"}]"
                    .into(),
            )
        }
    };
    if docs.is_empty() {
        return Err("documents array is empty — provide at least one document".into());
    }

    let want_fields = split_list(fields);
    let store = split_list(store_fields);

    // token -> field -> ref -> tf
    let mut index: BTreeMap<String, BTreeMap<String, BTreeMap<String, u32>>> = BTreeMap::new();
    let mut documents: BTreeMap<String, Map<String, Value>> = BTreeMap::new();
    let mut field_set: BTreeMap<String, ()> = BTreeMap::new();

    for (i, doc) in docs.iter().enumerate() {
        let obj = doc
            .as_object()
            .ok_or_else(|| format!("document at position {i} is not a JSON object"))?;

        // Resolve this document's ref.
        let doc_ref = match obj.get(id_field).and_then(scalar_to_ref) {
            Some(r) => r,
            None => i.to_string(),
        };
        if documents.contains_key(&doc_ref) {
            return Err(format!(
                "duplicate document ref {doc_ref:?} (from field {id_field:?}) — refs must be unique"
            ));
        }

        // Which fields to tokenize for this document.
        let doc_fields: Vec<String> = if want_fields.is_empty() {
            obj.iter()
                .filter(|(k, v)| k.as_str() != id_field && v.is_string())
                .map(|(k, _)| k.clone())
                .collect()
        } else {
            want_fields.clone()
        };

        for f in &doc_fields {
            let Some(Value::String(text)) = obj.get(f) else {
                continue;
            };
            field_set.insert(f.clone(), ());
            for tok in tokenize(text, lowercase, remove_stopwords, min_length) {
                *index
                    .entry(tok)
                    .or_default()
                    .entry(f.clone())
                    .or_default()
                    .entry(doc_ref.clone())
                    .or_insert(0) += 1;
            }
        }

        // Build this document's store entry.
        let mut stored = Map::new();
        for f in &store {
            if let Some(v) = obj.get(f) {
                stored.insert(f.clone(), v.clone());
            }
        }
        documents.insert(doc_ref, stored);
    }

    // Fold the raw tf maps into { df, postings } per (token, field).
    let index: BTreeMap<String, BTreeMap<String, FieldPostings>> = index
        .into_iter()
        .map(|(tok, per_field)| {
            let per_field = per_field
                .into_iter()
                .map(|(field, postings)| {
                    (
                        field,
                        FieldPostings {
                            df: postings.len(),
                            postings,
                        },
                    )
                })
                .collect();
            (tok, per_field)
        })
        .collect();

    let fields_out: Vec<String> = field_set.into_keys().collect();
    let out = IndexOutput {
        document_count: docs.len(),
        token_count: index.len(),
        ref_field: id_field.to_string(),
        fields: fields_out,
        boosts,
        index,
        documents,
    };

    let json = if pretty {
        serde_json::to_string_pretty(&out)
    } else {
        serde_json::to_string(&out)
    }
    .map_err(|e| format!("failed to serialize index: {e}"))?;
    Ok(json)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_inverted_index_with_term_frequencies_and_df() {
        let docs = r#"[
            {"id":"a","title":"Hello world","body":"world world"},
            {"id":"b","title":"Goodbye","body":"cruel world"}
        ]"#;
        let out = build_index(docs, "", "id", "title", "", true, false, 1, false).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["documentCount"], 2);
        assert_eq!(v["ref"], "id");
        // "world": in title of a (tf 1), in body of a (tf 2) and b (tf 1).
        assert_eq!(v["index"]["world"]["title"]["postings"]["a"], 1);
        assert_eq!(v["index"]["world"]["title"]["df"], 1);
        assert_eq!(v["index"]["world"]["body"]["postings"]["a"], 2);
        assert_eq!(v["index"]["world"]["body"]["postings"]["b"], 1);
        assert_eq!(v["index"]["world"]["body"]["df"], 2);
        // "hello" only in doc a's title.
        assert_eq!(v["index"]["hello"]["title"]["postings"]["a"], 1);
        // Auto-detected fields, sorted, id excluded.
        assert_eq!(v["fields"], serde_json::json!(["body", "title"]));
        // Store carries title.
        assert_eq!(v["documents"]["a"]["title"], "Hello world");
        // No boosts given → key omitted.
        assert!(v.get("boosts").is_none());
    }

    #[test]
    fn lowercase_off_keeps_case_and_stopwords_are_dropped() {
        let docs = r#"[{"id":"1","body":"The Cat and the cat"}]"#;
        // case-sensitive, stop words removed → "The"/"and"/"the" dropped, "Cat" != "cat".
        let out = build_index(docs, "body", "id", "", "", false, true, 1, false).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["index"]["Cat"]["body"]["postings"]["1"], 1);
        assert_eq!(v["index"]["cat"]["body"]["postings"]["1"], 1);
        assert!(v["index"].get("the").is_none());
        assert!(v["index"].get("and").is_none());
    }

    #[test]
    fn min_length_drops_short_tokens() {
        let docs = r#"[{"id":"1","body":"I am a go programmer"}]"#;
        let out = build_index(docs, "body", "id", "", "", true, false, 3, false).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        // "i", "am", "a", "go" are <3 chars and dropped; "programmer" stays.
        assert!(v["index"].get("go").is_none());
        assert!(v["index"].get("programmer").is_some());
        assert_eq!(v["tokenCount"], 1);
    }

    #[test]
    fn missing_id_falls_back_to_array_index() {
        let docs = r#"[{"body":"alpha"},{"body":"beta"}]"#;
        let out = build_index(docs, "body", "id", "", "", true, false, 1, false).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["index"]["alpha"]["body"]["postings"]["0"], 1);
        assert_eq!(v["index"]["beta"]["body"]["postings"]["1"], 1);
    }

    #[test]
    fn boosts_are_recorded() {
        let docs = r#"[{"id":"1","title":"hi","body":"there"}]"#;
        let out = build_index(docs, "title,body", "id", "", "title:2,body:1", true, false, 1, false)
            .unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["boosts"]["title"], 2.0);
        assert_eq!(v["boosts"]["body"], 1.0);
    }

    #[test]
    fn err_on_non_array() {
        let err =
            build_index(r#"{"id":"1"}"#, "", "id", "", "", true, false, 1, false).unwrap_err();
        assert!(err.contains("JSON array"), "got: {err}");
    }

    #[test]
    fn err_on_invalid_json() {
        let err = build_index("not json", "", "id", "", "", true, false, 1, false).unwrap_err();
        assert!(err.contains("valid JSON"), "got: {err}");
    }

    #[test]
    fn err_on_duplicate_ref() {
        let docs = r#"[{"id":"x","body":"one"},{"id":"x","body":"two"}]"#;
        let err = build_index(docs, "body", "id", "", "", true, false, 1, false).unwrap_err();
        assert!(err.contains("duplicate document ref"), "got: {err}");
    }

    #[test]
    fn err_on_empty_array() {
        let err = build_index("[]", "", "id", "", "", true, false, 1, false).unwrap_err();
        assert!(err.contains("empty"), "got: {err}");
    }

    #[test]
    fn err_on_bad_boost() {
        let docs = r#"[{"id":"1","body":"hi"}]"#;
        let err = build_index(docs, "body", "id", "", "title:notanumber", true, false, 1, false)
            .unwrap_err();
        assert!(err.contains("must be a number"), "got: {err}");
    }
}
