//! gizza-ai/mt940-statement-parse — parse a SWIFT MT940 customer statement into
//! structured statements (opening/closing balances + `:61:`/`:86:` transaction
//! lines) and render as JSON or CSV. Thin wrapper around the core; the chat schema
//! is single-sourced from descriptor() (which also drives the CLI); handle()
//! delegates to block_utils::run_skill. Pure.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_mt940_statement_parse_core::run;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default)]
    output: String,
    #[serde(default)]
    date_format: String,
    #[serde(default)]
    delimiter: String,
    #[serde(default = "default_true")]
    signed_amounts: bool,
}
fn default_true() -> bool { true }

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("The raw MT940 statement text (the SWIFT tagged fields — :20: :25: :28C: :60F: :61: :86: :62F: …). One or more statements; block headers and the '-' trailer are ignored."))
        .param(Param::enumv("output", ["json", "csv"]).default("json").describe("Output format: 'json' for full structured statements (balances + transactions) or 'csv' for a flat transaction table. Default 'json'."))
        .param(Param::enumv("date_format", ["iso", "us", "eu", "raw"]).default("iso").describe("How value/entry dates are rendered: 'iso' (YYYY-MM-DD), 'us' (MM/DD/YYYY), 'eu' (DD/MM/YYYY), or 'raw' (the source YYMMDD). Default 'iso'."))
        .param(Param::enumv("delimiter", ["comma", "semicolon", "tab", "pipe"]).default("comma").describe("CSV field separator, used only when output='csv': 'comma', 'semicolon', 'tab', or 'pipe'. Default 'comma'."))
        .param(Param::boolean("signed_amounts").default(true).describe("When true, debit amounts and balances are negative and credits positive; when false, amounts stay positive and the D/C mark carries the direction. Default true."))
}
fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct Mt940StatementParse;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/mt940-statement-parse",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Parse a SWIFT MT940 bank statement into JSON or CSV transactions and balances",
    skill(
        description = "Parse a SWIFT MT940 customer bank statement (the tagged-field format — :20: :25: :28C: :60F: :61: :86: :62F: :64: :65:) into structured statements: opening/closing/available balances plus each transaction's value & entry dates, debit/credit mark, signed amount, currency, 4-char transaction-type code, customer & bank references, and :86: narrative. Output full JSON (default) or a flat CSV transaction table. Handles multi-statement files, reversal marks (RC/RD), reformats dates (iso/us/eu/raw), and picks the CSV delimiter.",
        parameters = schema_json()
    ),
)]
impl Mt940StatementParse {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "mt940-statement-parse", |a: Args| {
            run(&a.data, &a.output, &a.date_format, &a.delimiter, a.signed_amounts)
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
                    "data":           { "type": "string", "description": "The raw MT940 statement text (the SWIFT tagged fields — :20: :25: :28C: :60F: :61: :86: :62F: …). One or more statements; block headers and the '-' trailer are ignored." },
                    "output":         { "type": "string", "enum": ["json", "csv"], "default": "json", "description": "Output format: 'json' for full structured statements (balances + transactions) or 'csv' for a flat transaction table. Default 'json'." },
                    "date_format":    { "type": "string", "enum": ["iso", "us", "eu", "raw"], "default": "iso", "description": "How value/entry dates are rendered: 'iso' (YYYY-MM-DD), 'us' (MM/DD/YYYY), 'eu' (DD/MM/YYYY), or 'raw' (the source YYMMDD). Default 'iso'." },
                    "delimiter":      { "type": "string", "enum": ["comma", "semicolon", "tab", "pipe"], "default": "comma", "description": "CSV field separator, used only when output='csv': 'comma', 'semicolon', 'tab', or 'pipe'. Default 'comma'." },
                    "signed_amounts": { "type": "boolean", "default": true, "description": "When true, debit amounts and balances are negative and credits positive; when false, amounts stay positive and the D/C mark carries the direction. Default true." }
                },
                "required": ["data"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
