//! gizza-ai/combinations-permutations — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Pure.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_combinations_permutations_core::{
    compute, parse_item_separator, parse_join_separator, parse_mode, parse_output_format,
};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    #[serde(default)]
    items: String,
    #[serde(default)]
    n: u64,
    r: u64,
    #[serde(default)]
    mode: String,
    #[serde(default)]
    repetition: bool,
    #[serde(default)]
    output_format: String,
    #[serde(default)]
    item_separator: String,
    #[serde(default)]
    dedupe: bool,
    #[serde(default)]
    join_separator: String,
    #[serde(default)]
    custom_join_separator: String,
    #[serde(default = "default_max_results")]
    max_results: u64,
}
fn default_max_results() -> u64 {
    10_000
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("items").default("").describe("Optional pool of actual items to draw from, e.g. 'a, b, c, d' or one per line. When given it sets n (n = the number of items) and lets output_format=lines/csv/json list the real selections. Leave empty to work from n alone; enumeration then uses the numbers 1..n."))
        .param(Param::integer("n").default(0).min(0.0).max(1000.0).describe("How many items there are to choose from, when no items list is pasted, e.g. 49 for a 6/49 lottery. Ignored if items is non-empty. Max 1000."))
        .param(Param::integer("r").required().min(0.0).max(1000.0).describe("How many items are chosen or arranged, e.g. 6 for a 6/49 lottery draw. Must be at least 1 to enumerate; r = 0 counts as exactly one empty selection. Max 1000."))
        .param(Param::enumv("mode", ["combinations", "permutations", "circular_permutations"]).default("combinations").describe("'combinations' (default): order does not matter — nCr. 'permutations': order matters — nPr. 'circular_permutations': seatings around a round table, where rotations of the same order count once — C(n, r) * (r - 1)!; it does not accept repetition."))
        .param(Param::boolean("repetition").default(false).describe("Allow the same item to be picked more than once (with replacement). Combinations then use C(n + r - 1, r) and permutations use n^r. Not available for circular_permutations."))
        .param(Param::enumv("output_format", ["summary", "count", "lines", "csv", "json"]).default("summary").describe("'summary' (default): the count with thousands separators plus the formula, the substituted values and the 1-in-N odds. 'count': the bare number only, no separators. 'lines': one selection per line, items joined by join_separator. 'csv': one RFC 4180 row per selection. 'json': array of arrays. The three enumerating formats need r >= 1 and obey max_results; summary and count never enumerate, so they are exempt."))
        .param(Param::enumv("item_separator", ["auto", "comma", "newline", "semicolon", "pipe", "tab", "space"]).default("auto").describe("How the items list is split. 'auto' (default) tries tab, then newline, comma, semicolon, pipe, and falls back to spaces. 'tab' also splits on line breaks so a pasted spreadsheet column works. Items are trimmed and blanks dropped."))
        .param(Param::boolean("dedupe").default(false).describe("Remove duplicate items from the pool before generating (keeps the first occurrence), so 'a, b, a' behaves as 'a, b'."))
        .param(Param::enumv("join_separator", ["comma", "space", "none", "dash", "underscore", "pipe", "slash", "plus", "dot", "custom"]).default("comma").describe("What joins the items of each selection in 'lines' output: comma ', ', space ' ', none '', dash '-', underscore '_', pipe '|', slash '/', plus '+', dot '.', or 'custom' (see custom_join_separator). Ignored for csv/json/count/summary."))
        .param(Param::string("custom_join_separator").default("").describe("Join string used when join_separator is 'custom', e.g. ' -> '."))
        .param(Param::integer("max_results").default(10000).min(1.0).max(100000.0).describe("Safety cap on the number of ENUMERATED selections. Exceeding it is an error that reports the exact count, so nothing is silently truncated. Default 10000, hard cap 100000. Counting (summary/count) is never capped."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn run_args(a: Args) -> Result<String, SkillError> {
    let mode = parse_mode(&a.mode).map_err(SkillError::InvalidArgs)?;
    let out_format = parse_output_format(&a.output_format).map_err(SkillError::InvalidArgs)?;
    let item_sep = parse_item_separator(&a.item_separator).map_err(SkillError::InvalidArgs)?;
    let join_sep = parse_join_separator(&a.join_separator).map_err(SkillError::InvalidArgs)?;
    compute(
        &a.items,
        a.n,
        a.r,
        mode,
        a.repetition,
        item_sep,
        a.dedupe,
        out_format,
        join_sep,
        &a.custom_join_separator,
        a.max_results,
    )
    .map_err(SkillError::InvalidArgs)
}

#[cfg(target_arch = "wasm32")]
struct CombinationsPermutations;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/combinations-permutations",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Count nCr/nPr exactly and optionally list every combination or permutation.",
    skill(
        description = "Combinatorics counter and generator. Computes how many ways r items can be chosen from n — combinations (nCr, order does not matter), permutations (nPr, order matters), or circular permutations (round-table seatings, rotations counted once) — with or without repetition, using exact arbitrary-precision arithmetic so C(1000, 500) prints all 299 digits. The default 'summary' output shows the count with thousands separators, the formula, the substituted values and the 1-in-N odds (lottery/poker style). Paste a pool of real items (or leave it empty to use 1..n) and switch output_format to lines, csv or json to enumerate the actual selections, joined by any separator. max_results (default 10000, hard cap 100000) guards enumeration; exceeding it reports the exact count instead of truncating. n and r max 1000.",
        parameters = schema_json()
    ),
)]
impl CombinationsPermutations {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "combinations-permutations", run_args) {
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
                    "items":                 { "type": "string", "default": "", "description": "Optional pool of actual items to draw from, e.g. 'a, b, c, d' or one per line. When given it sets n (n = the number of items) and lets output_format=lines/csv/json list the real selections. Leave empty to work from n alone; enumeration then uses the numbers 1..n." },
                    "n":                     { "type": "integer", "default": 0, "minimum": 0, "maximum": 1000, "description": "How many items there are to choose from, when no items list is pasted, e.g. 49 for a 6/49 lottery. Ignored if items is non-empty. Max 1000." },
                    "r":                     { "type": "integer", "minimum": 0, "maximum": 1000, "description": "How many items are chosen or arranged, e.g. 6 for a 6/49 lottery draw. Must be at least 1 to enumerate; r = 0 counts as exactly one empty selection. Max 1000." },
                    "mode":                  { "type": "string", "enum": ["combinations", "permutations", "circular_permutations"], "default": "combinations", "description": "'combinations' (default): order does not matter — nCr. 'permutations': order matters — nPr. 'circular_permutations': seatings around a round table, where rotations of the same order count once — C(n, r) * (r - 1)!; it does not accept repetition." },
                    "repetition":            { "type": "boolean", "default": false, "description": "Allow the same item to be picked more than once (with replacement). Combinations then use C(n + r - 1, r) and permutations use n^r. Not available for circular_permutations." },
                    "output_format":         { "type": "string", "enum": ["summary", "count", "lines", "csv", "json"], "default": "summary", "description": "'summary' (default): the count with thousands separators plus the formula, the substituted values and the 1-in-N odds. 'count': the bare number only, no separators. 'lines': one selection per line, items joined by join_separator. 'csv': one RFC 4180 row per selection. 'json': array of arrays. The three enumerating formats need r >= 1 and obey max_results; summary and count never enumerate, so they are exempt." },
                    "item_separator":        { "type": "string", "enum": ["auto", "comma", "newline", "semicolon", "pipe", "tab", "space"], "default": "auto", "description": "How the items list is split. 'auto' (default) tries tab, then newline, comma, semicolon, pipe, and falls back to spaces. 'tab' also splits on line breaks so a pasted spreadsheet column works. Items are trimmed and blanks dropped." },
                    "dedupe":                { "type": "boolean", "default": false, "description": "Remove duplicate items from the pool before generating (keeps the first occurrence), so 'a, b, a' behaves as 'a, b'." },
                    "join_separator":        { "type": "string", "enum": ["comma", "space", "none", "dash", "underscore", "pipe", "slash", "plus", "dot", "custom"], "default": "comma", "description": "What joins the items of each selection in 'lines' output: comma ', ', space ' ', none '', dash '-', underscore '_', pipe '|', slash '/', plus '+', dot '.', or 'custom' (see custom_join_separator). Ignored for csv/json/count/summary." },
                    "custom_join_separator": { "type": "string", "default": "", "description": "Join string used when join_separator is 'custom', e.g. ' -> '." },
                    "max_results":           { "type": "integer", "default": 10000, "minimum": 1, "maximum": 100000, "description": "Safety cap on the number of ENUMERATED selections. Exceeding it is an error that reports the exact count, so nothing is silently truncated. Default 10000, hard cap 100000. Counting (summary/count) is never capped." }
                },
                "required": ["r"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn run_args_counts_a_lottery_draw() {
        let a: Args =
            serde_json::from_str(r#"{"n": 49, "r": 6, "output_format": "count"}"#).unwrap();
        assert_eq!(run_args(a).unwrap(), "13983816");
    }

    #[test]
    fn run_args_enumerates_a_pool() {
        let a: Args =
            serde_json::from_str(r#"{"items": "a, b, c", "r": 2, "output_format": "lines"}"#)
                .unwrap();
        assert_eq!(run_args(a).unwrap(), "a, b\na, c\nb, c");
    }

    #[test]
    fn run_args_defaults_to_the_summary_view() {
        let a: Args = serde_json::from_str(r#"{"n": 10, "r": 3}"#).unwrap();
        let out = run_args(a).unwrap();
        assert!(out.starts_with("C(10, 3) = 120\n"), "{out}");
        assert!(
            out.contains("Formula: C(n, r) = n! / (r! * (n - r)!) = 10! / (3! * 7!)"),
            "{out}"
        );
    }

    #[test]
    fn run_args_rejects_bad_enum() {
        let a: Args = serde_json::from_str(r#"{"n": 5, "r": 2, "mode": "triangles"}"#).unwrap();
        assert!(run_args(a).is_err());
    }

    #[test]
    fn run_args_rejects_circular_with_repetition() {
        let a: Args = serde_json::from_str(
            r#"{"n": 5, "r": 3, "mode": "circular_permutations", "repetition": true}"#,
        )
        .unwrap();
        assert!(run_args(a).is_err());
    }
}
