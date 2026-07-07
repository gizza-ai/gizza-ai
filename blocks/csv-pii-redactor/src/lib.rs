//! gizza-ai/csv-pii-redactor — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the CLI);
//! handle() delegates to block_utils::run_skill. Column-scoped PII redaction over a
//! CSV: mask / salted-hash / uniform-redact the values in chosen columns. Pure → all
//! backends. Distinct from `redact-pii` (free-text masking) and `pii-tokenize`
//! (free-text pseudonyms), which do not operate on chosen tabular columns.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_csv_pii_redactor_core::{redact_csv, Mode, Options};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    columns: String,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default = "default_true")]
    header: bool,
    #[serde(default)]
    delimiter: String,
    #[serde(default = "default_mask_char")]
    mask_char: String,
    #[serde(default)]
    keep_last: u32,
    #[serde(default)]
    salt: String,
    #[serde(default = "default_hash_length")]
    hash_length: u32,
    #[serde(default = "default_label")]
    label: String,
}
fn default_mode() -> String {
    "mask".into()
}
fn default_true() -> bool {
    true
}
fn default_mask_char() -> String {
    "*".into()
}
fn default_hash_length() -> u32 {
    8
}
fn default_label() -> String {
    "[REDACTED]".into()
}

/// Build the core [`Options`] from parsed argument values. Shared by the chat and CLI
/// entry point; the web wrapper builds the same struct from string fields.
fn build_options(
    mode: &str,
    mask_char: &str,
    keep_last: u32,
    salt: &str,
    hash_length: u32,
    label: &str,
) -> Result<Options, String> {
    Ok(Options {
        mode: Mode::parse(mode)?,
        mask_char: mask_char.chars().next().unwrap_or('*'),
        keep_last: keep_last as usize,
        salt: salt.to_string(),
        hash_length: hash_length as usize,
        label: label.to_string(),
    })
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("The CSV text to redact."))
        .param(Param::string("columns").required().describe(
            "Comma-separated columns to redact: column names (when header=true) or 1-based indices, e.g. 'email,phone' or '2,4'. Only these columns are changed; others pass through.",
        ))
        .param(
            Param::enumv("mode", ["mask", "hash", "redact"])
                .default("mask")
                .describe("How to replace each selected cell: 'mask' = characters → mask_char (length kept, last keep_last visible); 'hash' = salted SHA-256 hex code (deterministic, joinable); 'redact' = the fixed label. Default 'mask'."),
        )
        .param(
            Param::boolean("header")
                .default(true)
                .describe("Treat the first row as a header so columns can be named and the header row is left unchanged (default true). With false, address columns by 1-based index."),
        )
        .param(
            Param::string("delimiter")
                .default(",")
                .describe("Field separator: a single char or 'comma'/'tab'/'semicolon'/'pipe'. Default ','."),
        )
        .param(
            Param::string("mask_char")
                .default("*")
                .describe("mask mode: the character each hidden character becomes (single char). Default '*'."),
        )
        .param(
            Param::integer("keep_last")
                .min(0.0)
                .max(64.0)
                .default(0)
                .describe("mask mode: leave the last N characters of each value visible, masking the rest (e.g. 4 → ****1234). Default 0 (mask everything)."),
        )
        .param(
            Param::string("salt")
                .describe("hash mode: salt prepended before hashing so codes aren't reversible via rainbow tables; the same salt+value always yields the same code (values stay joinable across files). Default empty."),
        )
        .param(
            Param::integer("hash_length")
                .min(4.0)
                .max(64.0)
                .default(8)
                .describe("hash mode: how many hex characters of the SHA-256 code to keep, 4–64 (8 ≈ short code, 64 = full digest). Default 8."),
        )
        .param(
            Param::string("label")
                .default("[REDACTED]")
                .describe("redact mode: the fixed string every selected cell becomes. Default '[REDACTED]'."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct CsvPiiRedactor;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/csv-pii-redactor",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Mask, salted-hash, or redact chosen CSV columns",
    skill(
        description = "Redact personally-identifiable data in the columns you choose of a CSV. Name columns by header (header=true) or 1-based index; the mode is 'mask' (replace characters with mask_char, keeping the last keep_last visible), 'hash' (salted SHA-256 hex code, deterministic so equal values map to equal codes), or 'redact' (replace with a fixed label). Non-selected columns and the header row are unchanged. delimiter is a single char or comma/tab/semicolon/pipe. Runs locally.",
        parameters = schema_json()
    ),
)]
impl CsvPiiRedactor {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "csv-pii-redactor", |a: Args| {
            let delim = if a.delimiter.is_empty() {
                ",".to_string()
            } else {
                a.delimiter
            };
            let opts = build_options(
                &a.mode,
                &a.mask_char,
                a.keep_last,
                &a.salt,
                a.hash_length,
                &a.label,
            )
            .map_err(SkillError::InvalidArgs)?;
            redact_csv(&a.data, &a.columns, a.header, &delim, &opts).map_err(SkillError::InvalidArgs)
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
                    "data": { "type": "string", "description": "The CSV text to redact." },
                    "columns": { "type": "string", "description": "Comma-separated columns to redact: column names (when header=true) or 1-based indices, e.g. 'email,phone' or '2,4'. Only these columns are changed; others pass through." },
                    "mode": { "type": "string", "enum": ["mask", "hash", "redact"], "default": "mask", "description": "How to replace each selected cell: 'mask' = characters → mask_char (length kept, last keep_last visible); 'hash' = salted SHA-256 hex code (deterministic, joinable); 'redact' = the fixed label. Default 'mask'." },
                    "header": { "type": "boolean", "default": true, "description": "Treat the first row as a header so columns can be named and the header row is left unchanged (default true). With false, address columns by 1-based index." },
                    "delimiter": { "type": "string", "default": ",", "description": "Field separator: a single char or 'comma'/'tab'/'semicolon'/'pipe'. Default ','." },
                    "mask_char": { "type": "string", "default": "*", "description": "mask mode: the character each hidden character becomes (single char). Default '*'." },
                    "keep_last": { "type": "integer", "minimum": 0, "maximum": 64, "default": 0, "description": "mask mode: leave the last N characters of each value visible, masking the rest (e.g. 4 → ****1234). Default 0 (mask everything)." },
                    "salt": { "type": "string", "description": "hash mode: salt prepended before hashing so codes aren't reversible via rainbow tables; the same salt+value always yields the same code (values stay joinable across files). Default empty." },
                    "hash_length": { "type": "integer", "minimum": 4, "maximum": 64, "default": 8, "description": "hash mode: how many hex characters of the SHA-256 code to keep, 4–64 (8 ≈ short code, 64 = full digest). Default 8." },
                    "label": { "type": "string", "default": "[REDACTED]", "description": "redact mode: the fixed string every selected cell becomes. Default '[REDACTED]'." }
                },
                "required": ["data", "columns"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
