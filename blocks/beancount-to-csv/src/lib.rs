//! gizza-ai/beancount-to-csv — chat skill block on the shared tool abstraction.
//! Flattens a Beancount/Ledger journal into a flat CSV of dated postings (and
//! rebuilds a simple journal from that CSV). The chat schema is single-sourced
//! from descriptor() (which also drives the CLI); handle() delegates to
//! block_utils::run_skill. Pure compute, no host calls — runs entirely inside
//! the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn default_direction() -> String {
    "to-csv".to_string()
}
fn default_journal_format() -> String {
    "beancount".to_string()
}
fn default_delimiter() -> String {
    "comma".to_string()
}

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_direction")]
    direction: String,
    #[serde(default = "default_journal_format")]
    journal_format: String,
    #[serde(default = "default_delimiter")]
    delimiter: String,
}

/// Single source for the chat schema (and CLI). Param order mirrors the core
/// `convert` signature and the web `run` export so all surfaces stay in sync.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::enumv("direction", ["to-csv", "from-csv"])
                .default("to-csv")
                .describe("to-csv flattens a Beancount/Ledger journal into a CSV (one row per posting); from-csv rebuilds a journal from that same CSV shape. Default to-csv."),
        )
        .param(
            Param::string("input")
                .required()
                .describe("The text to convert: a Beancount/Ledger journal (for to-csv) or a flat CSV with a header row (for from-csv). The CSV columns are date,flag,payee,narration,account,amount,currency,cost,price,comment; only 'date' and 'account' are required, others may be omitted."),
        )
        .param(
            Param::enumv("journal_format", ["beancount", "ledger"])
                .default("beancount")
                .describe("Plain-text-accounting dialect written by from-csv: beancount uses 2-space indent and quoted \"payee\" \"narration\"; ledger uses 4-space indent and a single description with symbol currencies prefixed ($-4.50). Ignored for to-csv (parsing accepts both dialects). Default beancount."),
        )
        .param(
            Param::enumv("delimiter", ["comma", "semicolon", "tab", "pipe"])
                .default("comma")
                .describe("CSV field separator — the output separator for to-csv, the input separator for from-csv. Default comma."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/beancount-to-csv",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Flatten a Beancount/Ledger journal into a CSV of dated postings, and back",
    skill(
        description = "Convert between a Beancount/Ledger (plain-text-accounting) journal and a flat CSV of dated postings, for spreadsheet use. Pass the text as 'input'. direction=to-csv (default) parses the journal and emits one CSV row per posting with columns date,flag,payee,narration,account,amount,currency,cost,price,comment — the transaction header fields repeat across its postings. direction=from-csv reads that same CSV shape (only 'date' and 'account' columns are required; a blank date continues the previous transaction) and rebuilds a journal. journal_format selects the dialect written by from-csv (beancount = 2-space indent + quoted payee/narration; ledger = 4-space indent + single description, symbol currencies prefixed). delimiter is the CSV separator (comma/semicolon/tab/pipe). Amounts are split into a numeric column and a currency column; cost {…} and price @… expressions are carried through verbatim. This is not a full Beancount engine: non-transaction directives (open/close/balance/price/…) are ignored, elided amounts are left blank, and balances are not asserted. Up to 20,000 postings per call. Runs locally; nothing is uploaded.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": ... }.
        match run_skill(&body, "beancount-to-csv", |a: Args| {
            gizza_ai_beancount_to_csv_core::convert(
                &a.input,
                &a.direction,
                &a.journal_format,
                &a.delimiter,
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
                    "direction": { "type": "string", "enum": ["to-csv", "from-csv"], "default": "to-csv", "description": "to-csv flattens a Beancount/Ledger journal into a CSV (one row per posting); from-csv rebuilds a journal from that same CSV shape. Default to-csv." },
                    "input": { "type": "string", "description": "The text to convert: a Beancount/Ledger journal (for to-csv) or a flat CSV with a header row (for from-csv). The CSV columns are date,flag,payee,narration,account,amount,currency,cost,price,comment; only 'date' and 'account' are required, others may be omitted." },
                    "journal_format": { "type": "string", "enum": ["beancount", "ledger"], "default": "beancount", "description": "Plain-text-accounting dialect written by from-csv: beancount uses 2-space indent and quoted \"payee\" \"narration\"; ledger uses 4-space indent and a single description with symbol currencies prefixed ($-4.50). Ignored for to-csv (parsing accepts both dialects). Default beancount." },
                    "delimiter": { "type": "string", "enum": ["comma", "semicolon", "tab", "pipe"], "default": "comma", "description": "CSV field separator — the output separator for to-csv, the input separator for from-csv. Default comma." }
                },
                "required": ["input"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
