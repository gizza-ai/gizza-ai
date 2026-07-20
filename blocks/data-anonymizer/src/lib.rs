//! gizza-ai/data-anonymizer — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Redacts identifier
//! columns, generalizes quasi-identifier columns (numeric bins, date→year,
//! text prefix masking), and reports the dataset's k-anonymity level. Pure →
//! all backends. Distinct from `csv-pii-redactor` (masks/hashes chosen columns
//! with no generalization and no anonymity metric).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_data_anonymizer_core::{anonymize_csv, Options, Output};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    quasi: String,
    #[serde(default)]
    identifiers: String,
    #[serde(default)]
    sensitive: String,
    #[serde(default = "default_k")]
    k: u32,
    #[serde(default = "default_numeric_bin")]
    numeric_bin: f64,
    #[serde(default = "default_text_keep")]
    text_keep: u32,
    #[serde(default = "default_true")]
    dates_to_year: bool,
    #[serde(default)]
    suppress: bool,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default)]
    delimiter: String,
    #[serde(default = "default_label")]
    label: String,
    #[serde(default = "default_output")]
    output: String,
}
fn default_k() -> u32 {
    2
}
fn default_numeric_bin() -> f64 {
    10.0
}
fn default_text_keep() -> u32 {
    3
}
fn default_true() -> bool {
    true
}
fn default_label() -> String {
    "[REDACTED]".into()
}
fn default_output() -> String {
    "both".into()
}

/// Build the core [`Options`] from parsed argument values. Shared by the chat
/// and CLI entry point; the web wrapper builds the same struct from strings.
fn build_options(
    k: u32,
    numeric_bin: f64,
    text_keep: u32,
    dates_to_year: bool,
    suppress: bool,
    label: &str,
    output: &str,
) -> Result<Options, String> {
    Ok(Options {
        k: k as usize,
        numeric_bin,
        text_keep: text_keep as usize,
        dates_to_year,
        suppress,
        label: if label.is_empty() {
            "[REDACTED]".to_string()
        } else {
            label.to_string()
        },
        output: Output::parse(output)?,
    })
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("The CSV text to anonymize."))
        .param(Param::string("quasi").required().describe(
            "Comma-separated quasi-identifier columns to generalize: names (when header=true) or 1-based indices, e.g. 'age,zipcode,gender'. Add ':level' to override one column's generalization (bin width for numeric, kept characters for text): 'age,zipcode:100'.",
        ))
        .param(Param::string("identifiers").describe(
            "Comma-separated direct-identifier columns (name, email, SSN…) whose every value is replaced by the label. Default none.",
        ))
        .param(Param::string("sensitive").describe(
            "One column holding the sensitive attribute (e.g. diagnosis); the report then adds the distinct l-diversity (min distinct sensitive values per equivalence class). Default none.",
        ))
        .param(
            Param::integer("k")
                .min(2.0)
                .max(100.0)
                .default(2)
                .describe("Target k for the report and for suppression: every row should share its generalized quasi-identifier combination with at least k-1 others, 2-100. Default 2."),
        )
        .param(
            Param::number("numeric_bin")
                .min(0.001)
                .default(10.0)
                .describe("Bin width for numeric quasi-identifier columns: 34 with width 10 becomes '30-39'. Must be positive; default 10."),
        )
        .param(
            Param::integer("text_keep")
                .min(0.0)
                .max(32.0)
                .default(3)
                .describe("Leading characters kept in text quasi-identifier values; the rest become '*' ('London' with 3 → 'Lon***'). 0 replaces the whole value with a single '*'. Default 3."),
        )
        .param(
            Param::boolean("dates_to_year")
                .default(true)
                .describe("Generalize ISO-date quasi-identifier columns (YYYY-MM-DD or YYYY-MM) to the year, e.g. '1987-04-12' → '1987' (default true). With false such columns fall back to text prefix masking."),
        )
        .param(
            Param::boolean("suppress")
                .default(false)
                .describe("Drop every row whose equivalence class is smaller than k, so the remaining rows are k-anonymous; the report counts suppressed rows. Default false (keep all rows and report the risk instead)."),
        )
        .param(
            Param::boolean("header")
                .default(true)
                .describe("Treat the first row as a header so columns can be named and the header row passes through unchanged (default true). With false, address columns by 1-based index."),
        )
        .param(
            Param::string("delimiter")
                .default(",")
                .describe("Field separator: a single char or 'comma'/'tab'/'semicolon'/'pipe'. Default ','."),
        )
        .param(
            Param::string("label")
                .default("[REDACTED]")
                .describe("The fixed string every identifier-column value becomes. Default '[REDACTED]'."),
        )
        .param(
            Param::enumv("output", ["both", "csv", "report"])
                .default("both")
                .describe("What to return: 'both' = anonymized CSV plus the k-anonymity report (default), 'csv' = anonymized CSV only, 'report' = report only."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct DataAnonymizer;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/data-anonymizer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generalize quasi-identifiers and report a CSV's k-anonymity",
    skill(
        description = "Anonymize a CSV and measure how anonymous it really is. Direct-identifier columns (identifiers) are replaced with a fixed label; quasi-identifier columns (quasi) are generalized — numeric values into bins of width numeric_bin ('34' → '30-39'), ISO dates to the year, text to a kept prefix plus '*' padding — with optional per-column ':level' overrides ('zipcode:100'). Rows are then grouped by their generalized quasi-identifier combination and the report gives the achieved k-anonymity (smallest group size), the equivalence-class count, the share of rows below the target k, optional suppression of those rows, and — when a sensitive column is named — the distinct l-diversity. Handles up to 10000 data rows. Runs locally.",
        parameters = schema_json()
    ),
)]
impl DataAnonymizer {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "data-anonymizer", |a: Args| {
            let delim = if a.delimiter.is_empty() {
                ",".to_string()
            } else {
                a.delimiter
            };
            let opts = build_options(
                a.k,
                a.numeric_bin,
                a.text_keep,
                a.dates_to_year,
                a.suppress,
                &a.label,
                &a.output,
            )
            .map_err(SkillError::InvalidArgs)?;
            anonymize_csv(
                &a.data,
                &a.quasi,
                &a.identifiers,
                &a.sensitive,
                a.header,
                &delim,
                &opts,
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
                    "data": { "type": "string", "description": "The CSV text to anonymize." },
                    "quasi": { "type": "string", "description": "Comma-separated quasi-identifier columns to generalize: names (when header=true) or 1-based indices, e.g. 'age,zipcode,gender'. Add ':level' to override one column's generalization (bin width for numeric, kept characters for text): 'age,zipcode:100'." },
                    "identifiers": { "type": "string", "description": "Comma-separated direct-identifier columns (name, email, SSN…) whose every value is replaced by the label. Default none." },
                    "sensitive": { "type": "string", "description": "One column holding the sensitive attribute (e.g. diagnosis); the report then adds the distinct l-diversity (min distinct sensitive values per equivalence class). Default none." },
                    "k": { "type": "integer", "minimum": 2, "maximum": 100, "default": 2, "description": "Target k for the report and for suppression: every row should share its generalized quasi-identifier combination with at least k-1 others, 2-100. Default 2." },
                    "numeric_bin": { "type": "number", "minimum": 0.001, "default": 10.0, "description": "Bin width for numeric quasi-identifier columns: 34 with width 10 becomes '30-39'. Must be positive; default 10." },
                    "text_keep": { "type": "integer", "minimum": 0, "maximum": 32, "default": 3, "description": "Leading characters kept in text quasi-identifier values; the rest become '*' ('London' with 3 → 'Lon***'). 0 replaces the whole value with a single '*'. Default 3." },
                    "dates_to_year": { "type": "boolean", "default": true, "description": "Generalize ISO-date quasi-identifier columns (YYYY-MM-DD or YYYY-MM) to the year, e.g. '1987-04-12' → '1987' (default true). With false such columns fall back to text prefix masking." },
                    "suppress": { "type": "boolean", "default": false, "description": "Drop every row whose equivalence class is smaller than k, so the remaining rows are k-anonymous; the report counts suppressed rows. Default false (keep all rows and report the risk instead)." },
                    "header": { "type": "boolean", "default": true, "description": "Treat the first row as a header so columns can be named and the header row passes through unchanged (default true). With false, address columns by 1-based index." },
                    "delimiter": { "type": "string", "default": ",", "description": "Field separator: a single char or 'comma'/'tab'/'semicolon'/'pipe'. Default ','." },
                    "label": { "type": "string", "default": "[REDACTED]", "description": "The fixed string every identifier-column value becomes. Default '[REDACTED]'." },
                    "output": { "type": "string", "enum": ["both", "csv", "report"], "default": "both", "description": "What to return: 'both' = anonymized CSV plus the k-anonymity report (default), 'csv' = anonymized CSV only, 'report' = report only." }
                },
                "required": ["data", "quasi"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
