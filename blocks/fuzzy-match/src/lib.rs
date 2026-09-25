//! gizza-ai/fuzzy-match — chat skill block on the shared tool abstraction.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_fuzzy_match_core::run;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    query: String,
    candidates: String,
    #[serde(default = "default_algorithm")]
    algorithm: String,
    #[serde(default = "default_limit")]
    limit: i64,
    #[serde(default)]
    threshold: f64,
    #[serde(default)]
    case_sensitive: bool,
    #[serde(default = "default_true")]
    include_reasons: bool,
    #[serde(default = "default_format")]
    output_format: String,
}

fn default_algorithm() -> String {
    "hybrid".to_string()
}
fn default_limit() -> i64 {
    10
}
fn default_true() -> bool {
    true
}
fn default_format() -> String {
    "text".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("query").required().describe(
                "Search text to rank candidates against. Example: 'apple' will prefer 'apple pie' and 'applet' over unrelated strings. Limited to 256 characters.",
            ),
        )
        .param(
            Param::string("candidates").required().describe(
                "Candidate strings, one per line. A single comma-separated line also works. Blank lines and '#' comments are ignored, and simple list markers such as '-', '*' and '1.' are stripped. Limit 1000 candidates, 256 characters each.",
            ),
        )
        .param(
            Param::enumv("algorithm", ["hybrid", "levenshtein", "subsequence"])
                .default("hybrid")
                .describe(
                    "Scoring method. hybrid (default) uses the best of edit-distance, contains and subsequence/fuzzy-finder signals. levenshtein uses normalized edit distance only. subsequence rewards query characters that appear in order, like a fuzzy finder.",
                ),
        )
        .param(
            Param::integer("limit")
                .default(10)
                .min(1.0)
                .max(1000.0)
                .describe("Maximum number of matches to return, from 1 to 1000. Default 10."),
        )
        .param(
            Param::number("threshold")
                .default(0.0)
                .min(0.0)
                .max(100.0)
                .describe("Minimum score from 0 to 100. Default 0 keeps all candidates before the limit is applied; 80-90 is a stricter review range."),
        )
        .param(
            Param::boolean("case_sensitive")
                .default(false)
                .describe("When true, uppercase/lowercase differences count. Default false lowercases query and candidates before scoring."),
        )
        .param(
            Param::boolean("include_reasons")
                .default(true)
                .describe("Include a short reason such as edit distance, exact/contains match, or subsequence span in the output. Default true."),
        )
        .param(
            Param::enumv("output_format", ["text", "csv", "json"])
                .default("text")
                .describe("Output format: text (ranked table), csv (rank,score,candidate,edit_distance,reason), or json (array of match objects)."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/fuzzy-match",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Rank candidate strings by fuzzy similarity to a query",
    skill(
        description = "Rank a list of candidate strings by fuzzy similarity to a query. `query` is required; `candidates` is one candidate per line (comma-separated input, list markers, blank lines and '#' comments are accepted). algorithm=hybrid (default best-of contains/edit-distance/subsequence), levenshtein, or subsequence. Scores are 0-100, sorted descending with edit-distance and alphabetic tie-breakers. Use `threshold` to drop weak matches and `limit` (1-1000, default 10) to cap the result count. `case_sensitive` defaults false. `include_reasons` adds a short reason column. output_format=text, csv or json. Pure deterministic local matching for quick fuzzy lookup and candidate review; not a full two-table join or phonetic/ML matcher.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "fuzzy-match", |a: Args| {
            run(
                &a.query,
                &a.candidates,
                &a.algorithm,
                a.limit,
                a.threshold,
                a.case_sensitive,
                a.include_reasons,
                &a.output_format,
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
                "type": "object",
                "properties": {
                    "algorithm": { "type": "string", "enum": ["hybrid", "levenshtein", "subsequence"], "default": "hybrid", "description": "Scoring method. hybrid (default) uses the best of edit-distance, contains and subsequence/fuzzy-finder signals. levenshtein uses normalized edit distance only. subsequence rewards query characters that appear in order, like a fuzzy finder." },
                    "candidates": { "type": "string", "description": "Candidate strings, one per line. A single comma-separated line also works. Blank lines and '#' comments are ignored, and simple list markers such as '-', '*' and '1.' are stripped. Limit 1000 candidates, 256 characters each." },
                    "case_sensitive": { "type": "boolean", "default": false, "description": "When true, uppercase/lowercase differences count. Default false lowercases query and candidates before scoring." },
                    "include_reasons": { "type": "boolean", "default": true, "description": "Include a short reason such as edit distance, exact/contains match, or subsequence span in the output. Default true." },
                    "limit": { "type": "integer", "minimum": 1, "maximum": 1000, "default": 10, "description": "Maximum number of matches to return, from 1 to 1000. Default 10." },
                    "output_format": { "type": "string", "enum": ["text", "csv", "json"], "default": "text", "description": "Output format: text (ranked table), csv (rank,score,candidate,edit_distance,reason), or json (array of match objects)." },
                    "query": { "type": "string", "description": "Search text to rank candidates against. Example: 'apple' will prefer 'apple pie' and 'applet' over unrelated strings. Limited to 256 characters." },
                    "threshold": { "type": "number", "minimum": 0, "maximum": 100, "default": 0.0, "description": "Minimum score from 0 to 100. Default 0 keeps all candidates before the limit is applied; 80-90 is a stricter review range." }
                },
                "required": ["query", "candidates"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
