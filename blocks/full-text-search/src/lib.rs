//! gizza-ai/full-text-search — chat skill block on the shared tool abstraction.
//! Ranks a pasted corpus of documents against a query with BM25 or TF-IDF,
//! Porter stemming, stop-word removal, phrase search and `-term` exclusion. The
//! chat schema is single-sourced from descriptor() (which also drives the CLI);
//! handle() delegates to block_utils::run_skill. Pure compute, no host calls —
//! runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_full_text_search_core::{
    search_text, Algorithm, MatchMode, Options, OutputFormat, Separator,
};
use serde::Deserialize;
use wafer_sdk::*;

fn default_separator() -> String {
    "dashes".to_string()
}
fn default_algorithm() -> String {
    "bm25".to_string()
}
fn default_match() -> String {
    "any".to_string()
}
fn default_output() -> String {
    "text".to_string()
}
fn default_true() -> bool {
    true
}
fn default_max_results() -> i64 {
    10
}
fn default_snippet_words() -> i64 {
    30
}
fn default_k1() -> f64 {
    1.2
}
fn default_b() -> f64 {
    0.75
}
fn default_title_boost() -> f64 {
    2.0
}

#[derive(Deserialize)]
struct Args {
    /// The pasted documents, one corpus string.
    corpus: String,
    /// The search query.
    query: String,
    /// "dashes" | "blank-line" | "form-feed".
    #[serde(default = "default_separator")]
    separator: String,
    /// "bm25" | "tfidf".
    #[serde(default = "default_algorithm")]
    algorithm: String,
    #[serde(default = "default_true")]
    stemming: bool,
    #[serde(default = "default_true")]
    stopwords: bool,
    /// "any" (OR) or "all" (AND).
    #[serde(rename = "match", default = "default_match")]
    match_mode: String,
    #[serde(default)]
    prefix: bool,
    #[serde(default = "default_max_results")]
    max_results: i64,
    #[serde(default = "default_snippet_words")]
    snippet_words: i64,
    #[serde(default = "default_k1")]
    k1: f64,
    #[serde(default = "default_b")]
    b: f64,
    #[serde(default = "default_title_boost")]
    title_boost: f64,
    /// "text" | "json".
    #[serde(default = "default_output")]
    output: String,
}

/// Build the core options from the parsed args, validating every enum.
fn options(a: &Args) -> Result<Options, String> {
    Ok(Options {
        separator: Separator::parse(&a.separator)?,
        algorithm: Algorithm::parse(&a.algorithm)?,
        stemming: a.stemming,
        stopwords: a.stopwords,
        match_mode: MatchMode::parse(&a.match_mode)?,
        prefix: a.prefix,
        max_results: a.max_results.max(0) as usize,
        snippet_words: a.snippet_words.max(0) as usize,
        k1: a.k1,
        b: a.b,
        title_boost: a.title_boost,
        output: OutputFormat::parse(&a.output)?,
    })
}

/// Single source for the chat schema (and CLI + page controls). Every param
/// carries a `.describe()` an LLM/CLI user can act on. `separator`/`algorithm`/
/// `match`/`output` are `enumv` (→ `<select>`); `stemming`/`stopwords`/`prefix`
/// are booleans (→ checkboxes); `k1`/`b`/`title_boost` are bounded numbers and
/// `max_results`/`snippet_words` bounded integers (→ sliders on the page).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("corpus")
                .required()
                .describe("The documents to search, pasted as one block of text and split into separate documents by `separator`. Each document's first non-blank line is treated as its title."),
        )
        .param(
            Param::string("query")
                .required()
                .describe("What to search for. Bare words are terms (e.g. 'refund policy'), \"quoted words\" must appear as an exact adjacent phrase, and a -word prefix excludes any document containing it."),
        )
        .param(
            Param::enumv("separator", ["dashes", "blank-line", "form-feed"])
                .default("dashes")
                .describe("How the pasted corpus is split into documents: dashes = a line of 3+ dashes (---), blank-line = one or more empty lines, form-feed = an ASCII form feed (\\f). Default dashes."),
        )
        .param(
            Param::enumv("algorithm", ["bm25", "tfidf"])
                .default("bm25")
                .describe("Relevance scoring: bm25 = Okapi BM25 with term-frequency saturation and length normalisation (recommended), tfidf = classic log-TF x smoothed IDF. Default bm25."),
        )
        .param(
            Param::boolean("stemming")
                .default(true)
                .describe("Reduce English words to their Porter stem so a search for 'run' also matches 'running' and 'runs'. Default on."),
        )
        .param(
            Param::boolean("stopwords")
                .default(true)
                .describe("Drop common English stop words (the, of, and, ...) from the query. Skipped automatically when it would leave nothing to search for. Default on."),
        )
        .param(
            Param::enumv("match", ["any", "all"])
                .default("any")
                .describe("Return documents containing ANY query term (OR) or ALL of them (AND). Quoted phrases count as one term. Default any."),
        )
        .param(
            Param::boolean("prefix")
                .default(false)
                .describe("Also match words that merely START WITH a query term, so 'moto' finds 'motorcycle'. Default off."),
        )
        .param(
            Param::integer("max_results")
                .min(1.0)
                .max(50.0)
                .default(10)
                .describe("Maximum number of ranked documents to return, 1-50. The total number of matches is reported either way. Default 10."),
        )
        .param(
            Param::integer("snippet_words")
                .min(0.0)
                .max(120.0)
                .default(30)
                .describe("Words of context shown around the best match in each hit, 0-120. Matched words are wrapped in guillemets. 0 returns ranked hits with no snippet. Default 30."),
        )
        .param(
            Param::number("k1")
                .min(0.0)
                .max(3.0)
                .default(1.2)
                .describe("BM25 term-frequency saturation, 0-3 (standard range 1.2-2.0). Higher means repeating a term keeps adding relevance; 0 makes one occurrence count the same as ten. Ignored by tfidf. Default 1.2."),
        )
        .param(
            Param::number("b")
                .min(0.0)
                .max(1.0)
                .default(0.75)
                .describe("BM25 document-length normalisation, 0-1 (standard 0.75). 1 penalises long documents fully, 0 disables the penalty. Ignored by tfidf. Default 0.75."),
        )
        .param(
            Param::number("title_boost")
                .min(1.0)
                .max(5.0)
                .default(2.0)
                .describe("How much more a match in a document's first line (its title) is worth than one in the body, 1-5. 1 = no boost. Default 2."),
        )
        .param(
            Param::enumv("output", ["text", "json"])
                .default("text")
                .describe("Result format: text = ranked list with highlighted snippets, json = the full structured result (scores, matched terms, per-document metadata). Default text."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/full-text-search",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "BM25/TF-IDF ranked full-text search over pasted documents",
    skill(
        description = "Full-text search across a set of pasted documents, ranked by relevance with BM25 (or classic TF-IDF) and returned with highlighted keyword-in-context snippets. Paste the documents as one block separated by --- rules, blank lines or form feeds; each document's first line is its title and can be boosted. Supports English Porter stemming (run finds running), stop-word removal, any/all term matching, opt-in prefix matching, \"quoted phrases\" that must match adjacently, and -term exclusion. Tune BM25's k1 and b. To search a PDF/DOCX, extract its text first with document-text-extract. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "full-text-search", |a: Args| {
            let opts = options(&a).map_err(SkillError::InvalidArgs)?;
            search_text(&a.query, &a.corpus, opts).map_err(SkillError::InvalidArgs)
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
                    "corpus": { "type": "string", "description": "The documents to search, pasted as one block of text and split into separate documents by `separator`. Each document's first non-blank line is treated as its title." },
                    "query": { "type": "string", "description": "What to search for. Bare words are terms (e.g. 'refund policy'), \"quoted words\" must appear as an exact adjacent phrase, and a -word prefix excludes any document containing it." },
                    "separator": { "type": "string", "enum": ["dashes", "blank-line", "form-feed"], "default": "dashes", "description": "How the pasted corpus is split into documents: dashes = a line of 3+ dashes (---), blank-line = one or more empty lines, form-feed = an ASCII form feed (\\f). Default dashes." },
                    "algorithm": { "type": "string", "enum": ["bm25", "tfidf"], "default": "bm25", "description": "Relevance scoring: bm25 = Okapi BM25 with term-frequency saturation and length normalisation (recommended), tfidf = classic log-TF x smoothed IDF. Default bm25." },
                    "stemming": { "type": "boolean", "default": true, "description": "Reduce English words to their Porter stem so a search for 'run' also matches 'running' and 'runs'. Default on." },
                    "stopwords": { "type": "boolean", "default": true, "description": "Drop common English stop words (the, of, and, ...) from the query. Skipped automatically when it would leave nothing to search for. Default on." },
                    "match": { "type": "string", "enum": ["any", "all"], "default": "any", "description": "Return documents containing ANY query term (OR) or ALL of them (AND). Quoted phrases count as one term. Default any." },
                    "prefix": { "type": "boolean", "default": false, "description": "Also match words that merely START WITH a query term, so 'moto' finds 'motorcycle'. Default off." },
                    "max_results": { "type": "integer", "minimum": 1, "maximum": 50, "default": 10, "description": "Maximum number of ranked documents to return, 1-50. The total number of matches is reported either way. Default 10." },
                    "snippet_words": { "type": "integer", "minimum": 0, "maximum": 120, "default": 30, "description": "Words of context shown around the best match in each hit, 0-120. Matched words are wrapped in guillemets. 0 returns ranked hits with no snippet. Default 30." },
                    "k1": { "type": "number", "minimum": 0, "maximum": 3, "default": 1.2, "description": "BM25 term-frequency saturation, 0-3 (standard range 1.2-2.0). Higher means repeating a term keeps adding relevance; 0 makes one occurrence count the same as ten. Ignored by tfidf. Default 1.2." },
                    "b": { "type": "number", "minimum": 0, "maximum": 1, "default": 0.75, "description": "BM25 document-length normalisation, 0-1 (standard 0.75). 1 penalises long documents fully, 0 disables the penalty. Ignored by tfidf. Default 0.75." },
                    "title_boost": { "type": "number", "minimum": 1, "maximum": 5, "default": 2.0, "description": "How much more a match in a document's first line (its title) is worth than one in the body, 1-5. 1 = no boost. Default 2." },
                    "output": { "type": "string", "enum": ["text", "json"], "default": "text", "description": "Result format: text = ranked list with highlighted snippets, json = the full structured result (scores, matched terms, per-document metadata). Default text." }
                },
                "required": ["corpus", "query"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn args_default_every_optional_param() {
        let a: Args = serde_json::from_str(r#"{"corpus":"a\n---\nb","query":"a"}"#).unwrap();
        assert_eq!(a.separator, "dashes");
        assert_eq!(a.algorithm, "bm25");
        assert_eq!(a.match_mode, "any");
        assert_eq!(a.output, "text");
        assert!(a.stemming);
        assert!(a.stopwords);
        assert!(!a.prefix);
        assert_eq!(a.max_results, 10);
        assert_eq!(a.snippet_words, 30);
        assert_eq!(a.k1, 1.2);
        assert_eq!(a.b, 0.75);
        assert_eq!(a.title_boost, 2.0);
        let opts = options(&a).unwrap();
        assert_eq!(opts.separator, Separator::Dashes);
        assert_eq!(opts.algorithm, Algorithm::Bm25);
        assert_eq!(opts.match_mode, MatchMode::Any);
        assert_eq!(opts.output, OutputFormat::Text);
    }

    #[test]
    fn options_reject_unknown_enum_values() {
        let a: Args =
            serde_json::from_str(r#"{"corpus":"a","query":"a","algorithm":"bm42"}"#).unwrap();
        let err = options(&a).unwrap_err();
        assert!(err.contains("unknown algorithm"), "got: {err}");
    }

    #[test]
    fn end_to_end_happy_path_through_the_args_shape() {
        let a: Args = serde_json::from_str(
            r#"{"corpus":"Refund policy\nRefunds take five days.\n---\nShipping\nOrders ship fast.","query":"refund","snippet_words":8}"#,
        )
        .unwrap();
        let text = search_text(&a.query, &a.corpus, options(&a).unwrap()).unwrap();
        assert_eq!(
            text,
            "BM25 · 1 of 2 documents match \"refund\"\n\n#1  doc 1  score 1.04  Refund policy\n    \u{00AB}Refund\u{00BB} policy \u{00AB}Refunds\u{00BB} take five days"
        );
    }
}
