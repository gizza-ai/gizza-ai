//! gizza-ai/camt053-parse — parse an ISO 20022 CAMT bank statement XML
//! (camt.053, plus the sibling camt.052/camt.054 messages) into structured
//! statements (account, balances, entries + transaction details) and render as
//! JSON or CSV. Thin wrapper around the core; the chat schema is single-sourced
//! from descriptor() (which also drives the CLI); handle() delegates to
//! block_utils::run_skill. Pure.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_camt053_parse_core::run;
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
    #[serde(default = "default_true")]
    expand_details: bool,
}
fn default_true() -> bool { true }

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("The camt.053 bank statement XML text (an ISO 20022 <Document> with <BkToCstmrStmt>). The sibling camt.052 account report and camt.054 notification messages are accepted too; any schema version (camt.053.001.02 … .13) works."))
        .param(Param::enumv("output", ["json", "csv"]).default("json").describe("Output format: 'json' for full structured statements (account, balances, transactions) or 'csv' for a flat transaction table. Default 'json'."))
        .param(Param::enumv("date_format", ["iso", "us", "eu", "raw"]).default("iso").describe("How booking/value dates are rendered: 'iso' (YYYY-MM-DD), 'us' (MM/DD/YYYY), 'eu' (DD/MM/YYYY), or 'raw' (the source string verbatim, including any time part). Default 'iso'."))
        .param(Param::enumv("delimiter", ["comma", "semicolon", "tab", "pipe"]).default("comma").describe("CSV field separator, used only when output='csv': 'comma', 'semicolon', 'tab', or 'pipe'. Default 'comma'."))
        .param(Param::boolean("signed_amounts").default(true).describe("When true, DBIT (money out) amounts and balances are negative and CRDT positive; when false, amounts stay positive and the CRDT/DBIT indicator carries the direction. Default true."))
        .param(Param::boolean("expand_details").default(true).describe("When true, a batch entry (one <Ntry> holding several <TxDtls> payments) becomes one row per payment with its own amount and counterparty; when false, one row per entry with the batch total and a details_count. Default true."))
}
fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct Camt053Parse;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/camt053-parse",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Parse an ISO 20022 camt.053 bank statement XML into JSON or CSV transactions and balances",
    skill(
        description = "Parse an ISO 20022 CAMT bank statement XML — camt.053 (Bank-to-Customer Statement), camt.052 (Account Report), or camt.054 (Debit/Credit Notification), any schema version — into structured statements: account IBAN/currency/owner, every balance (OPBD opening, CLBD closing, OPAV/CLAV available, …), and each entry's booking & value dates, CRDT/DBIT direction, signed amount, status, bank transaction code, end-to-end & bank references, counterparty name/IBAN, remittance info, charges and FX rate. Batch entries (one <Ntry> with several <TxDtls>) can expand to one row per payment. Output full JSON (default) or a flat CSV transaction table; reformats dates (iso/us/eu/raw) and picks the CSV delimiter.",
        parameters = schema_json()
    ),
)]
impl Camt053Parse {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "camt053-parse", |a: Args| {
            run(&a.data, &a.output, &a.date_format, &a.delimiter, a.signed_amounts, a.expand_details)
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
                    "data":           { "type": "string", "description": "The camt.053 bank statement XML text (an ISO 20022 <Document> with <BkToCstmrStmt>). The sibling camt.052 account report and camt.054 notification messages are accepted too; any schema version (camt.053.001.02 … .13) works." },
                    "output":         { "type": "string", "enum": ["json", "csv"], "default": "json", "description": "Output format: 'json' for full structured statements (account, balances, transactions) or 'csv' for a flat transaction table. Default 'json'." },
                    "date_format":    { "type": "string", "enum": ["iso", "us", "eu", "raw"], "default": "iso", "description": "How booking/value dates are rendered: 'iso' (YYYY-MM-DD), 'us' (MM/DD/YYYY), 'eu' (DD/MM/YYYY), or 'raw' (the source string verbatim, including any time part). Default 'iso'." },
                    "delimiter":      { "type": "string", "enum": ["comma", "semicolon", "tab", "pipe"], "default": "comma", "description": "CSV field separator, used only when output='csv': 'comma', 'semicolon', 'tab', or 'pipe'. Default 'comma'." },
                    "signed_amounts": { "type": "boolean", "default": true, "description": "When true, DBIT (money out) amounts and balances are negative and CRDT positive; when false, amounts stay positive and the CRDT/DBIT indicator carries the direction. Default true." },
                    "expand_details": { "type": "boolean", "default": true, "description": "When true, a batch entry (one <Ntry> holding several <TxDtls> payments) becomes one row per payment with its own amount and counterparty; when false, one row per entry with the batch total and a details_count. Default true." }
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
