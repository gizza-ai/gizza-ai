//! gizza-ai/log-pattern-miner — chat skill block on the shared tool abstraction.
//! Clusters raw log lines into a handful of Drain-style message templates with
//! occurrence counts. The chat schema is single-sourced from descriptor() (which
//! also drives the CLI); handle() delegates to block_utils::run_skill and the
//! pure logic lives in gizza-ai-log-pattern-miner-core. No host calls — the
//! whole pipeline runs in the sandbox, and the same log always yields the same
//! templates.
//!
//! Stated limits (also in the skill description so an LLM can relay them):
//!   * 2,000,000 characters / 200,000 lines per run;
//!   * one-shot batch mining — no persisted tree, no cluster ids across runs;
//!   * masking uses a fixed typed placeholder set, not user-supplied regexes.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn default_format() -> String {
    "table".into()
}
fn default_similarity() -> f64 {
    0.4
}
fn default_depth() -> u32 {
    4
}
fn default_max_children() -> u32 {
    100
}
fn default_max_patterns() -> u32 {
    20
}
fn default_min_count() -> u32 {
    1
}
fn default_mask() -> String {
    "typed".into()
}

#[derive(Deserialize)]
struct Args {
    logs: String,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default = "default_similarity")]
    similarity: f64,
    #[serde(default = "default_depth")]
    depth: u32,
    #[serde(default = "default_max_children")]
    max_children: u32,
    #[serde(default = "default_max_patterns")]
    max_patterns: u32,
    #[serde(default = "default_min_count")]
    min_count: u32,
    #[serde(default = "default_mask")]
    mask: String,
    #[serde(default)]
    extra_delimiters: String,
    #[serde(default)]
    skip_tokens: u32,
}

/// Single source for the chat schema (and CLI). `logs` is the pasted batch;
/// every other param is one of the reference Drain knobs (similarity, depth,
/// max_children) or a presentation/filter control.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("logs")
                .required()
                .multiline()
                .describe("The raw log lines to mine, newline-separated — paste the file as-is, timestamps and all. Up to 2000000 characters and 200000 lines per run. Blank lines are counted but not mined."),
        )
        .param(
            Param::enumv("format", ["table", "json", "lines"])
                .default("table")
                .describe("How to render the ranked templates. 'table' (default) = tab-separated rows with a header (count, percent, first, last, template) where first/last are 1-based line numbers; 'json' = a full report {total_lines, mined_lines, blank_lines, patterns_found, patterns_shown, coverage_percent, settings, patterns:[{rank, count, percent, template, first_index, first_line, last_index, last_line, examples, variables:[{position, placeholder, values}]}]}; 'lines' = one template per line, nothing else (pipe-friendly)."),
        )
        .param(
            Param::number("similarity")
                .min(0.0)
                .max(1.0)
                .default(0.4)
                .describe("Merge threshold, 0-1 (default 0.4, the reference Drain value). A line joins an existing template when at least this fraction of the template's token positions still match; disagreeing positions collapse to <*>. Raise it (0.7-0.9) to keep similar-but-distinct messages apart, lower it (0.2-0.3) to fold more variants into one template."),
        )
        .param(
            Param::integer("depth")
                .min(2.0)
                .max(8.0)
                .default(4)
                .describe("Parse-tree depth, 2-8 (default 4). The first layer is the token count and the next depth-2 layers are the leading tokens, so a line only ever merges with templates sharing that prefix. Deeper = stricter and faster, shallower = more merging. Use 2 when the log's leading tokens are variable (e.g. a bare timestamp) and skip_tokens is not an option."),
        )
        .param(
            Param::integer("max_children")
                .min(2.0)
                .max(1000.0)
                .default(100)
                .describe("Branches allowed per parse-tree node, 2-1000 (default 100, the reference value). Once a node is full, further distinct tokens fall into its shared <*> branch instead of growing the tree. Lower it on very high-cardinality logs to force more merging."),
        )
        .param(
            Param::integer("max_patterns")
                .min(1.0)
                .max(500.0)
                .default(20)
                .describe("How many of the highest-count templates to return, 1-500 (default 20). Ranking is by occurrence count descending, ties broken by first appearance. The JSON report also carries patterns_found (the untruncated count) and coverage_percent, so a cap is never mistaken for the whole log."),
        )
        .param(
            Param::integer("min_count")
                .min(1.0)
                .default(1)
                .describe("Drop templates seen fewer than this many times (default 1, keep everything). Set 2+ to hide one-off lines and see only the recurring message shapes. It is an error if no template reaches the threshold."),
        )
        .param(
            Param::enumv("mask", ["typed", "wildcard", "none"])
                .default("typed")
                .describe("How variable parts of a token are rendered before clustering. 'typed' (default) = named placeholders <NUM> <HEX> <IP> <MAC> <UUID> <DATE> <TIME> <PATH> <URL> <EMAIL> <STR>; 'wildcard' = every masked value renders as <*>, matching the reference tools' output; 'none' = no pre-masking, so only the similarity merge introduces <*> (useful when the literal values are the thing you are grouping by)."),
        )
        .param(
            Param::string("extra_delimiters")
                .default("")
                .describe("Extra characters that split tokens on top of whitespace, up to 16 — e.g. '=' turns 'status=500' into two tokens so the value masks to <NUM>, and '=,:' also splits CSV-ish and key:value payloads. Default empty (whitespace only). Double-quoted runs stay one token either way."),
        )
        .param(
            Param::integer("skip_tokens")
                .min(0.0)
                .max(16.0)
                .default(0)
                .describe("Drop this many leading whitespace tokens from every line before mining, 0-16 (default 0). Use it to cut a fixed prefix — '2024-05-06 07:08:09 INFO' is 3 tokens — so the parse tree branches on the message instead of the timestamp."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/log-pattern-miner",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Cluster raw log lines into ranked message templates with occurrence counts.",
    skill(
        description = "Mine a batch of raw log lines into a handful of ranked message templates (Drain-style fixed-depth parse tree), each with how many lines it covers. Variable parts of every token are masked first — numbers with their units (250ms), hex blobs, IPv4/IPv6, MACs, UUIDs, dates, clock times, file paths, URLs, e-mail addresses and quoted strings — then lines are clustered by token count plus leading-token prefix and merged into a template when enough token positions still agree; disagreeing positions become <*>. Answers 'what is actually being logged, and how often' for a pasted log file. format='table' (default) is tab-separated count/percent/first-line/last-line/template rows; 'json' adds coverage, the untruncated pattern count, first/last and example source lines, and the sampled raw values behind each placeholder; 'lines' is one template per line. Tune with similarity (0-1, default 0.4), depth (2-8, default 4), max_children (default 100), max_patterns (default 20), min_count, mask ('typed' named placeholders, 'wildcard' plain <*>, or 'none'), extra_delimiters (extra token-splitting characters such as '=') and skip_tokens (drop a fixed timestamp/host prefix). Deterministic and stateless: one pass in input order, same input always gives the same templates, nothing is persisted between runs, so there are no stable cluster ids and no streaming/online mode. Up to 2000000 characters and 200000 lines per run; masking uses the built-in placeholder set, not user-supplied regexes.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "log-pattern-miner", |a: Args| {
            gizza_ai_log_pattern_miner_core::mine(
                &a.logs,
                &a.format,
                a.similarity,
                a.depth,
                a.max_children,
                a.max_patterns,
                a.min_count,
                &a.mask,
                &a.extra_delimiters,
                a.skip_tokens,
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
                    "logs": {
                        "type": "string",
                        "description": "The raw log lines to mine, newline-separated — paste the file as-is, timestamps and all. Up to 2000000 characters and 200000 lines per run. Blank lines are counted but not mined."
                    },
                    "format": {
                        "type": "string",
                        "enum": ["table", "json", "lines"],
                        "default": "table",
                        "description": "How to render the ranked templates. 'table' (default) = tab-separated rows with a header (count, percent, first, last, template) where first/last are 1-based line numbers; 'json' = a full report {total_lines, mined_lines, blank_lines, patterns_found, patterns_shown, coverage_percent, settings, patterns:[{rank, count, percent, template, first_index, first_line, last_index, last_line, examples, variables:[{position, placeholder, values}]}]}; 'lines' = one template per line, nothing else (pipe-friendly)."
                    },
                    "similarity": {
                        "type": "number",
                        "minimum": 0,
                        "maximum": 1,
                        "default": 0.4,
                        "description": "Merge threshold, 0-1 (default 0.4, the reference Drain value). A line joins an existing template when at least this fraction of the template's token positions still match; disagreeing positions collapse to <*>. Raise it (0.7-0.9) to keep similar-but-distinct messages apart, lower it (0.2-0.3) to fold more variants into one template."
                    },
                    "depth": {
                        "type": "integer",
                        "minimum": 2,
                        "maximum": 8,
                        "default": 4,
                        "description": "Parse-tree depth, 2-8 (default 4). The first layer is the token count and the next depth-2 layers are the leading tokens, so a line only ever merges with templates sharing that prefix. Deeper = stricter and faster, shallower = more merging. Use 2 when the log's leading tokens are variable (e.g. a bare timestamp) and skip_tokens is not an option."
                    },
                    "max_children": {
                        "type": "integer",
                        "minimum": 2,
                        "maximum": 1000,
                        "default": 100,
                        "description": "Branches allowed per parse-tree node, 2-1000 (default 100, the reference value). Once a node is full, further distinct tokens fall into its shared <*> branch instead of growing the tree. Lower it on very high-cardinality logs to force more merging."
                    },
                    "max_patterns": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 500,
                        "default": 20,
                        "description": "How many of the highest-count templates to return, 1-500 (default 20). Ranking is by occurrence count descending, ties broken by first appearance. The JSON report also carries patterns_found (the untruncated count) and coverage_percent, so a cap is never mistaken for the whole log."
                    },
                    "min_count": {
                        "type": "integer",
                        "minimum": 1,
                        "default": 1,
                        "description": "Drop templates seen fewer than this many times (default 1, keep everything). Set 2+ to hide one-off lines and see only the recurring message shapes. It is an error if no template reaches the threshold."
                    },
                    "mask": {
                        "type": "string",
                        "enum": ["typed", "wildcard", "none"],
                        "default": "typed",
                        "description": "How variable parts of a token are rendered before clustering. 'typed' (default) = named placeholders <NUM> <HEX> <IP> <MAC> <UUID> <DATE> <TIME> <PATH> <URL> <EMAIL> <STR>; 'wildcard' = every masked value renders as <*>, matching the reference tools' output; 'none' = no pre-masking, so only the similarity merge introduces <*> (useful when the literal values are the thing you are grouping by)."
                    },
                    "extra_delimiters": {
                        "type": "string",
                        "default": "",
                        "description": "Extra characters that split tokens on top of whitespace, up to 16 — e.g. '=' turns 'status=500' into two tokens so the value masks to <NUM>, and '=,:' also splits CSV-ish and key:value payloads. Default empty (whitespace only). Double-quoted runs stay one token either way."
                    },
                    "skip_tokens": {
                        "type": "integer",
                        "minimum": 0,
                        "maximum": 16,
                        "default": 0,
                        "description": "Drop this many leading whitespace tokens from every line before mining, 0-16 (default 0). Use it to cut a fixed prefix — '2024-05-06 07:08:09 INFO' is 3 tokens — so the parse tree branches on the message instead of the timestamp."
                    }
                },
                "required": ["logs"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
