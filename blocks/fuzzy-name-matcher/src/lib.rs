//! gizza-ai/fuzzy-name-matcher — match & deduplicate person / organization names.
//! Thin wrapper around the core; chat schema single-sourced from descriptor();
//! handler delegates to run_skill. Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_fuzzy_name_matcher_core::match_names;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    names: String,
    #[serde(default)]
    algorithm: String,
    #[serde(default)]
    mode: String,
    #[serde(default = "default_threshold")]
    threshold: f64,
    #[serde(default = "default_true")]
    normalize_case: bool,
    #[serde(default = "default_true")]
    ignore_titles: bool,
    #[serde(default)]
    output: String,
}
fn default_true() -> bool { true }
fn default_threshold() -> f64 { 85.0 }

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("names").required().describe("The names to match: one name per line, or a single-column CSV (the first field of each row is taken as the name). Person or organization names."))
        .param(Param::enumv("algorithm", ["jaro_winkler", "levenshtein", "soundex"]).default("jaro_winkler").describe("Similarity algorithm. jaro_winkler (default) = prefix-weighted, best for typos in short names; levenshtein = raw edit-distance ratio; soundex = phonetic, matches names that sound alike (Smith/Smyth) even when spelled differently."))
        .param(Param::enumv("mode", ["groups", "pairs"]).default("groups").describe("groups (default) = cluster the list into match groups with a canonical (most frequent) name each; pairs = list every matching name pair with its similarity score, best first."))
        .param(Param::integer("threshold").default(85).min(0.0).max(100.0).describe("Similarity cutoff 0–100. Names scoring at or above it are treated as the same entity. Higher = stricter. Default 85; drop toward 80 to catch more variants, raise it if unrelated names merge."))
        .param(Param::boolean("normalize_case").default(true).describe("Ignore letter case when comparing (so 'ACME' and 'acme' match). Default true."))
        .param(Param::boolean("ignore_titles").default(true).describe("Ignore honorifics (Mr, Mrs, Dr, Prof…) and generational/credential suffixes (Jr, Sr, III, PhD…) when comparing, so 'Dr. John Adams' matches 'John Adams'. Default true."))
        .param(Param::enumv("output", ["table", "csv", "json"]).default("table").describe("Result format: table (markdown groups/pairs + a mapping table), csv (a flat mapping you can join back), or json (structured groups/pairs with scores and counts)."))
}

fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct FuzzyNameMatcher;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/fuzzy-name-matcher",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Match & deduplicate names with Jaro-Winkler, Levenshtein or Soundex",
    skill(
        description = "Match and deduplicate person or organization names that refer to the same entity but are spelled differently — typos, casing, honorifics, and phonetic variants (Smith/Smyth, Jon/John). Paste `names` one per line (or a single-column CSV). Pick an `algorithm`: jaro_winkler (default, prefix-weighted for short names), levenshtein (edit-distance ratio), or soundex (phonetic — matches names that sound alike). `mode` = groups (cluster into match groups with a canonical name each) or pairs (every matching pair scored, best first). Two names are the 'same' when their similarity (0–100) is at least `threshold` (default 85). `normalize_case` folds case; `ignore_titles` drops honorifics (Mr/Dr…) and suffixes (Jr/III…) before comparing. `output` is table, csv, or json. Runs locally.",
        parameters = schema_json()
    )
)]
impl FuzzyNameMatcher {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "fuzzy-name-matcher", |a: Args| {
            let algorithm = if a.algorithm.is_empty() { "jaro_winkler".to_string() } else { a.algorithm };
            let mode = if a.mode.is_empty() { "groups".to_string() } else { a.mode };
            match_names(
                &a.names,
                &algorithm,
                &mode,
                a.threshold,
                a.normalize_case,
                a.ignore_titles,
                &a.output,
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
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "names":          { "type": "string", "description": "The names to match: one name per line, or a single-column CSV (the first field of each row is taken as the name). Person or organization names." },
                    "algorithm":      { "type": "string", "enum": ["jaro_winkler", "levenshtein", "soundex"], "default": "jaro_winkler", "description": "Similarity algorithm. jaro_winkler (default) = prefix-weighted, best for typos in short names; levenshtein = raw edit-distance ratio; soundex = phonetic, matches names that sound alike (Smith/Smyth) even when spelled differently." },
                    "mode":           { "type": "string", "enum": ["groups", "pairs"], "default": "groups", "description": "groups (default) = cluster the list into match groups with a canonical (most frequent) name each; pairs = list every matching name pair with its similarity score, best first." },
                    "threshold":      { "type": "integer", "minimum": 0, "maximum": 100, "default": 85, "description": "Similarity cutoff 0–100. Names scoring at or above it are treated as the same entity. Higher = stricter. Default 85; drop toward 80 to catch more variants, raise it if unrelated names merge." },
                    "normalize_case": { "type": "boolean", "default": true, "description": "Ignore letter case when comparing (so 'ACME' and 'acme' match). Default true." },
                    "ignore_titles":  { "type": "boolean", "default": true, "description": "Ignore honorifics (Mr, Mrs, Dr, Prof…) and generational/credential suffixes (Jr, Sr, III, PhD…) when comparing, so 'Dr. John Adams' matches 'John Adams'. Default true." },
                    "output":         { "type": "string", "enum": ["table", "csv", "json"], "default": "table", "description": "Result format: table (markdown groups/pairs + a mapping table), csv (a flat mapping you can join back), or json (structured groups/pairs with scores and counts)." }
                },
                "required": ["names"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
