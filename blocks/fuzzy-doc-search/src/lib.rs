//! gizza-ai/fuzzy-doc-search — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI + page); handle() delegates to block_utils::run_skill and returns the
//! structured ranked-hits JSON.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_fuzzy_doc_search_core::{search, Options, Unit};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    /// The words to search for.
    query: String,
    /// The document text to search.
    text: String,
    /// "any" (OR) or "all" (AND).
    #[serde(rename = "match", default = "default_match")]
    match_mode: String,
    /// Max typos tolerated per term (0..=3).
    #[serde(default = "default_fuzziness")]
    fuzziness: i64,
    #[serde(default)]
    case_sensitive: bool,
    #[serde(default)]
    whole_word: bool,
    /// "line" | "sentence" | "paragraph".
    #[serde(default = "default_unit")]
    unit: String,
    /// Max ranked snippets to return (1..=50).
    #[serde(default = "default_max_results")]
    max_results: i64,
}

fn default_match() -> String {
    "any".to_string()
}
fn default_fuzziness() -> i64 {
    1
}
fn default_unit() -> String {
    "line".to_string()
}
fn default_max_results() -> i64 {
    10
}

/// Parse `match` ("any"/"all") into the AND flag.
fn match_all(mode: &str) -> Result<bool, String> {
    match mode.trim().to_ascii_lowercase().as_str() {
        "any" => Ok(false),
        "all" => Ok(true),
        other => Err(format!("unknown match {other:?}: expected any or all")),
    }
}

/// Single source for the chat schema (and CLI + page controls). Every param
/// carries a `.describe()` an LLM/CLI user can act on. `match`/`unit` are
/// `enumv` (→ `<select>`); `case_sensitive`/`whole_word` are booleans
/// (→ checkboxes); `fuzziness`/`max_results` are bounded integers.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("query")
                .required()
                .describe("The word(s) to search for, space-separated (e.g. 'receive parcel')."),
        )
        .param(
            Param::string("text")
                .required()
                .describe("The document text to search. Paste plain text or markdown; concatenate several documents to search across them all."),
        )
        .param(
            Param::enumv("match", ["any", "all"])
                .default("any")
                .describe("Match snippets containing ANY query word (OR) or ALL query words (AND). Default any."),
        )
        .param(
            Param::integer("fuzziness")
                .min(0.0)
                .max(3.0)
                .default(1)
                .describe("Typo tolerance: max edit distance per word, 0–3. 0 = exact/prefix only; 1 catches one typo. Default 1."),
        )
        .param(
            Param::boolean("case_sensitive")
                .default(false)
                .describe("Match upper/lower case exactly. Default off (case-insensitive)."),
        )
        .param(
            Param::boolean("whole_word")
                .default(false)
                .describe("Match whole words only. Default off, so a term also matches inside a longer word (prefix/substring)."),
        )
        .param(
            Param::enumv("unit", ["line", "sentence", "paragraph"])
                .default("line")
                .describe("Snippet granularity: rank one snippet per line, sentence, or paragraph. Default line."),
        )
        .param(
            Param::integer("max_results")
                .min(1.0)
                .max(50.0)
                .default(10)
                .describe("Maximum number of ranked snippets to return, 1–50. Default 10."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/fuzzy-doc-search",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Fuzzy full-text search returning ranked snippets from pasted text",
    skill(
        description = "Full-text fuzzy search over a block of text: finds the passages that best match your query even with typos, and returns them ranked by relevance with the matched words highlighted. Paste plain text or markdown (concatenate several documents to search across all of them). Tune fuzziness (0–3 typos per word), match any/all query words, whole-word or substring, case sensitivity, and whether to rank by line, sentence, or paragraph. To search a PDF/DOCX, extract its text first with document-text-extract, then search here. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "fuzzy-doc-search", |a: Args| {
            let opts = Options {
                match_all: match_all(&a.match_mode).map_err(SkillError::InvalidArgs)?,
                fuzziness: a.fuzziness.max(0) as usize,
                case_sensitive: a.case_sensitive,
                whole_word: a.whole_word,
                unit: Unit::parse(&a.unit).map_err(SkillError::InvalidArgs)?,
                max_results: a.max_results.max(1) as usize,
            };
            search(&a.query, &a.text, opts).map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Migration/consistency guard: the descriptor-derived chat schema must
    /// match this authored blob, so the LLM (and the page form, which reads the
    /// synced manifest) sees a stable shape. Regenerate this literal whenever
    /// the descriptor changes on purpose.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "query": { "type": "string", "description": "The word(s) to search for, space-separated (e.g. 'receive parcel')." },
                    "text": { "type": "string", "description": "The document text to search. Paste plain text or markdown; concatenate several documents to search across them all." },
                    "match": { "type": "string", "enum": ["any", "all"], "default": "any", "description": "Match snippets containing ANY query word (OR) or ALL query words (AND). Default any." },
                    "fuzziness": { "type": "integer", "minimum": 0, "maximum": 3, "default": 1, "description": "Typo tolerance: max edit distance per word, 0–3. 0 = exact/prefix only; 1 catches one typo. Default 1." },
                    "case_sensitive": { "type": "boolean", "default": false, "description": "Match upper/lower case exactly. Default off (case-insensitive)." },
                    "whole_word": { "type": "boolean", "default": false, "description": "Match whole words only. Default off, so a term also matches inside a longer word (prefix/substring)." },
                    "unit": { "type": "string", "enum": ["line", "sentence", "paragraph"], "default": "line", "description": "Snippet granularity: rank one snippet per line, sentence, or paragraph. Default line." },
                    "max_results": { "type": "integer", "minimum": 1, "maximum": 50, "default": 10, "description": "Maximum number of ranked snippets to return, 1–50. Default 10." }
                },
                "required": ["query", "text"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn match_all_parses() {
        assert_eq!(match_all("any").unwrap(), false);
        assert_eq!(match_all("ALL").unwrap(), true);
        assert!(match_all("both").is_err());
    }

    #[test]
    fn args_default_optional_params() {
        let a: Args = serde_json::from_str(r#"{"query":"x","text":"y"}"#).unwrap();
        assert_eq!(a.match_mode, "any");
        assert_eq!(a.fuzziness, 1);
        assert_eq!(a.unit, "line");
        assert_eq!(a.max_results, 10);
        assert!(!a.case_sensitive);
        assert!(!a.whole_word);
    }
}
