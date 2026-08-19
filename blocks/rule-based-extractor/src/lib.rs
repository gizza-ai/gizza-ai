//! gizza-ai/rule-based-extractor — apply a set of named regex/pattern rules to
//! text and return the captured fields as JSON, CSV, a readable listing, or a
//! rule-by-rule report. Thin wrapper around the core; the chat schema is
//! single-sourced from descriptor() (which also drives the CLI); handle()
//! delegates to block_utils::run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_rule_based_extractor_core::{
    extract, DEFAULT_MAX_MATCHES, DEFAULT_MAX_RECORDS, MAX_MATCHES_LIMIT, MAX_RECORDS_LIMIT,
};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    rules: String,
    #[serde(default = "default_split")]
    split: String,
    #[serde(default)]
    split_pattern: String,
    #[serde(default = "default_matches")]
    matches: String,
    #[serde(default)]
    ignore_case: bool,
    #[serde(default)]
    multiline: bool,
    #[serde(default)]
    dotall: bool,
    #[serde(default = "default_true")]
    trim: bool,
    #[serde(default)]
    unique: bool,
    #[serde(default = "default_on_missing")]
    on_missing: String,
    #[serde(default = "default_true")]
    skip_empty_records: bool,
    #[serde(default = "default_max_records")]
    max_records: usize,
    #[serde(default = "default_max_matches")]
    max_matches: usize,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default)]
    pretty: bool,
}
fn default_split() -> String {
    "whole".to_string()
}
fn default_matches() -> String {
    "first".to_string()
}
fn default_on_missing() -> String {
    "skip".to_string()
}
fn default_output() -> String {
    "json".to_string()
}
fn default_true() -> bool {
    true
}
fn default_max_records() -> usize {
    DEFAULT_MAX_RECORDS
}
fn default_max_matches() -> usize {
    DEFAULT_MAX_MATCHES
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .multiline()
                .placeholder("Paste the log lines, invoice text or records to extract from…")
                .describe("The text to extract from. Up to 1 MB."),
        )
        .param(
            Param::string("rules")
                .required()
                .multiline()
                .placeholder("date = %{DATE_ISO}\nlevel = %{LOGLEVEL}\nclient = %{IPV4}")
                .describe(
                    "One rule per line. 'field = regex' captures one named field; a bare regex line captures every named group in it. '@NAME = regex' defines a reusable pattern, and '%{NAME}' / '%{NAME:field}' inserts a built-in or user pattern (WORD, INT, NUMBER, EMAIL, URL, IPV4, IPV6, MAC, UUID, HASH, DATE_ISO, DATE_US, TIME, TIMESTAMP_ISO, SYSLOG_TIME, YEAR, LOGLEVEL, HTTP_METHOD, HTTP_STATUS, HOSTNAME, PATH, QUOTED, MONEY, PERCENT, PHONE, ZIP_US, SEMVER, TICKET, GREEDYDATA, DATA, HEX, NOTSPACE, SPACE, IP). Lines starting with # or // are comments. Max 200 rules.",
                ),
        )
        .param(
            Param::enumv("split", ["whole", "lines", "paragraphs", "pattern"])
                .default("whole")
                .describe("How the text is divided into records before the rules run. 'whole' (default) treats the input as one record; 'lines' and 'paragraphs' produce one output object per line or blank-line-separated block; 'pattern' splits on the regex in split_pattern."),
        )
        .param(
            Param::string("split_pattern")
                .default("")
                .placeholder("^-{3,}$")
                .describe("Record-separator regex, used only when split='pattern'. For example '^-{3,}$' with multiline=true splits on a line of dashes."),
        )
        .param(
            Param::enumv("matches", ["first", "all"])
                .default("first")
                .describe("Whether each rule keeps only its first match per record (default) or every match. 'all' makes each field a JSON array."),
        )
        .param(
            Param::boolean("ignore_case")
                .default(false)
                .describe("Match rules case-insensitively (the regex 'i' flag). Per-rule inline flags like (?i) also work."),
        )
        .param(
            Param::boolean("multiline")
                .default(false)
                .describe("Make ^ and $ match at every line break inside a record instead of only at its start and end (the regex 'm' flag)."),
        )
        .param(
            Param::boolean("dotall")
                .default(false)
                .describe("Let '.' match newlines too (the regex 's' flag), so a rule can span several lines of a record."),
        )
        .param(
            Param::boolean("trim")
                .default(true)
                .describe("Strip leading and trailing whitespace from every captured value. On by default."),
        )
        .param(
            Param::boolean("unique")
                .default(false)
                .describe("With matches='all', drop repeated values within a field, keeping first-seen order."),
        )
        .param(
            Param::enumv("on_missing", ["skip", "null", "error"])
                .default("skip")
                .describe("What to do when a rule matches nothing in a record: 'skip' (default) omits the key, 'null' emits it as null/empty so every record has the same shape, 'error' fails with the field name."),
        )
        .param(
            Param::boolean("skip_empty_records")
                .default(true)
                .describe("Drop records where no rule matched at all. On by default; ignored when split='whole'."),
        )
        .param(
            Param::integer("max_records")
                .default(DEFAULT_MAX_RECORDS as i64)
                .min(1.0)
                .max(MAX_RECORDS_LIMIT as f64)
                .describe("Safety cap on how many records the input may split into. Exceeding it is an error, never a silent truncation."),
        )
        .param(
            Param::integer("max_matches")
                .default(DEFAULT_MAX_MATCHES as i64)
                .min(1.0)
                .max(MAX_MATCHES_LIMIT as f64)
                .describe("Safety cap on how many times one rule may match inside a single record when matches='all'. Exceeding it is an error."),
        )
        .param(
            Param::enumv("output", ["json", "csv", "text", "report"])
                .default("json")
                .describe("Result format: 'json' (default) objects of named fields, 'csv' one row per record, 'text' a readable field listing, or 'report' a per-rule hit count that shows which rules never matched."),
        )
        .param(
            Param::boolean("pretty")
                .default(false)
                .describe("Indent the JSON output. Applies to output='json' only."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct RuleBasedExtractor;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/rule-based-extractor",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract named fields from text with regex rules",
    skill(
        description = "Apply a set of named regex rules to unstructured text and return the captured fields as JSON, CSV, a readable listing, or a rule-by-rule report. Write one rule per line as 'field = regex'; a bare regex line contributes every named group it contains. Grok-style placeholders keep rules short — %{DATE_ISO:date}, %{IPV4:client}, %{LOGLEVEL:level} and ~30 other built-ins — and '@NAME = regex' defines your own. Set split to lines/paragraphs/pattern to run the rules per record and get one object per record, matches='all' to collect every hit as an array, and output='report' to see which rules never matched. Good for turning log lines, invoices, emails or scraped text into structured data.",
        parameters = schema_json()
    ),
)]
impl RuleBasedExtractor {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "rule-based-extractor", |a: Args| {
            extract(
                &a.text,
                &a.rules,
                &a.split,
                &a.split_pattern,
                &a.matches,
                a.ignore_case,
                a.multiline,
                a.dotall,
                a.trim,
                a.unique,
                &a.on_missing,
                a.skip_empty_records,
                a.max_records,
                a.max_matches,
                &a.output,
                a.pretty,
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
    use gizza_ai_block_utils::ParamKind;

    fn enum_variants(name: &str) -> Vec<String> {
        let d = descriptor();
        let p = d.params.iter().find(|p| p.name == name).unwrap().clone();
        match p.kind {
            ParamKind::Enum(v) => v,
            other => panic!("{name} should be an enum, got {other:?}"),
        }
    }

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "The text to extract from. Up to 1 MB." },
                    "rules": { "type": "string", "description": "One rule per line. 'field = regex' captures one named field; a bare regex line captures every named group in it. '@NAME = regex' defines a reusable pattern, and '%{NAME}' / '%{NAME:field}' inserts a built-in or user pattern (WORD, INT, NUMBER, EMAIL, URL, IPV4, IPV6, MAC, UUID, HASH, DATE_ISO, DATE_US, TIME, TIMESTAMP_ISO, SYSLOG_TIME, YEAR, LOGLEVEL, HTTP_METHOD, HTTP_STATUS, HOSTNAME, PATH, QUOTED, MONEY, PERCENT, PHONE, ZIP_US, SEMVER, TICKET, GREEDYDATA, DATA, HEX, NOTSPACE, SPACE, IP). Lines starting with # or // are comments. Max 200 rules." },
                    "split": {
                        "type": "string",
                        "enum": ["whole", "lines", "paragraphs", "pattern"],
                        "default": "whole",
                        "description": "How the text is divided into records before the rules run. 'whole' (default) treats the input as one record; 'lines' and 'paragraphs' produce one output object per line or blank-line-separated block; 'pattern' splits on the regex in split_pattern."
                    },
                    "split_pattern": { "type": "string", "default": "", "description": "Record-separator regex, used only when split='pattern'. For example '^-{3,}$' with multiline=true splits on a line of dashes." },
                    "matches": {
                        "type": "string",
                        "enum": ["first", "all"],
                        "default": "first",
                        "description": "Whether each rule keeps only its first match per record (default) or every match. 'all' makes each field a JSON array."
                    },
                    "ignore_case": { "type": "boolean", "default": false, "description": "Match rules case-insensitively (the regex 'i' flag). Per-rule inline flags like (?i) also work." },
                    "multiline": { "type": "boolean", "default": false, "description": "Make ^ and $ match at every line break inside a record instead of only at its start and end (the regex 'm' flag)." },
                    "dotall": { "type": "boolean", "default": false, "description": "Let '.' match newlines too (the regex 's' flag), so a rule can span several lines of a record." },
                    "trim": { "type": "boolean", "default": true, "description": "Strip leading and trailing whitespace from every captured value. On by default." },
                    "unique": { "type": "boolean", "default": false, "description": "With matches='all', drop repeated values within a field, keeping first-seen order." },
                    "on_missing": {
                        "type": "string",
                        "enum": ["skip", "null", "error"],
                        "default": "skip",
                        "description": "What to do when a rule matches nothing in a record: 'skip' (default) omits the key, 'null' emits it as null/empty so every record has the same shape, 'error' fails with the field name."
                    },
                    "skip_empty_records": { "type": "boolean", "default": true, "description": "Drop records where no rule matched at all. On by default; ignored when split='whole'." },
                    "max_records": { "type": "integer", "minimum": 1, "maximum": 50000, "default": 5000, "description": "Safety cap on how many records the input may split into. Exceeding it is an error, never a silent truncation." },
                    "max_matches": { "type": "integer", "minimum": 1, "maximum": 10000, "default": 1000, "description": "Safety cap on how many times one rule may match inside a single record when matches='all'. Exceeding it is an error." },
                    "output": {
                        "type": "string",
                        "enum": ["json", "csv", "text", "report"],
                        "default": "json",
                        "description": "Result format: 'json' (default) objects of named fields, 'csv' one row per record, 'text' a readable field listing, or 'report' a per-rule hit count that shows which rules never matched."
                    },
                    "pretty": { "type": "boolean", "default": false, "description": "Indent the JSON output. Applies to output='json' only." }
                },
                "required": ["text", "rules"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn descriptor_enums_match_the_core_lists() {
        use gizza_ai_rule_based_extractor_core as core;
        for (name, list) in [
            ("split", core::SPLITS),
            ("matches", core::MATCHES),
            ("on_missing", core::ON_MISSING),
            ("output", core::OUTPUTS),
        ] {
            assert_eq!(
                enum_variants(name),
                list.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
                "{name} enum drifted from the core list"
            );
        }
    }

    #[test]
    fn every_param_is_described() {
        for p in descriptor().params {
            assert!(!p.description.is_empty(), "{} needs a description", p.name);
        }
    }

    #[test]
    fn text_and_rules_are_the_only_required_params() {
        let required: Vec<String> = descriptor()
            .params
            .iter()
            .filter(|p| p.required)
            .map(|p| p.name.clone())
            .collect();
        assert_eq!(required, vec!["text".to_string(), "rules".to_string()]);
    }

    #[test]
    fn serde_defaults_match_the_descriptor_defaults() {
        let a: Args = serde_json::from_str(r#"{"text":"t","rules":"r"}"#).unwrap();
        assert_eq!(a.split, "whole");
        assert_eq!(a.matches, "first");
        assert_eq!(a.on_missing, "skip");
        assert_eq!(a.output, "json");
        assert!(a.trim);
        assert!(a.skip_empty_records);
        assert!(!a.ignore_case && !a.multiline && !a.dotall && !a.unique && !a.pretty);
        assert_eq!(a.max_records, DEFAULT_MAX_RECORDS);
        assert_eq!(a.max_matches, DEFAULT_MAX_MATCHES);
    }
}
