//! gizza-ai/bank-statement-reconcile — reconcile statement and ledger CSV rows.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn stmt_date_default() -> String {
    "date".to_string()
}
fn stmt_amount_default() -> String {
    "amount".to_string()
}
fn stmt_memo_default() -> String {
    "memo".to_string()
}
fn ledger_date_default() -> String {
    "date".to_string()
}
fn ledger_amount_default() -> String {
    "amount".to_string()
}
fn ledger_memo_default() -> String {
    "memo".to_string()
}
fn date_tol_default() -> i64 {
    3
}
fn amount_tol_default() -> f64 {
    0.01
}
fn memo_threshold_default() -> u8 {
    70
}
fn delimiter_default() -> String {
    "comma".to_string()
}
fn output_default() -> String {
    "markdown".to_string()
}

#[derive(Deserialize)]
struct Args {
    statement_csv: String,
    ledger_csv: String,
    #[serde(default = "stmt_date_default")]
    stmt_date: String,
    #[serde(default = "stmt_amount_default")]
    stmt_amount: String,
    #[serde(default = "stmt_memo_default")]
    stmt_memo: String,
    #[serde(default = "ledger_date_default")]
    ledger_date: String,
    #[serde(default = "ledger_amount_default")]
    ledger_amount: String,
    #[serde(default = "ledger_memo_default")]
    ledger_memo: String,
    #[serde(default = "date_tol_default")]
    date_tolerance_days: i64,
    #[serde(default = "amount_tol_default")]
    amount_tolerance: f64,
    #[serde(default = "memo_threshold_default")]
    memo_threshold: u8,
    #[serde(default = "delimiter_default")]
    delimiter: String,
    #[serde(default = "output_default")]
    output: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("statement_csv").required().multiline().describe("Bank statement CSV text with a header row."))
        .param(Param::string("ledger_csv").required().multiline().describe("Bookkeeping ledger CSV text with a header row."))
        .param(Param::string("stmt_date").default("date").describe("Statement date column name or 1-based index."))
        .param(Param::string("stmt_amount").default("amount").describe("Statement signed amount column name or 1-based index."))
        .param(Param::string("stmt_memo").default("memo").describe("Statement memo/description column name or 1-based index."))
        .param(Param::string("ledger_date").default("date").describe("Ledger date column name or 1-based index."))
        .param(Param::string("ledger_amount").default("amount").describe("Ledger signed amount column name or 1-based index."))
        .param(Param::string("ledger_memo").default("memo").describe("Ledger memo/description column name or 1-based index."))
        .param(Param::integer("date_tolerance_days").default(3).min(0.0).max(30.0).describe("Maximum absolute date difference, in days, for a candidate match."))
        .param(Param::number("amount_tolerance").default(0.01).min(0.0).describe("Maximum absolute signed-amount difference for a candidate match."))
        .param(Param::integer("memo_threshold").default(70).min(0.0).max(100.0).describe("Minimum fuzzy memo token similarity (0–100) to classify a candidate as matched instead of suggested."))
        .param(Param::enumv("delimiter", ["comma", "tab", "semicolon", "pipe"]).default("comma").describe("CSV field delimiter used by both inputs."))
        .param(Param::enumv("output", ["markdown", "json", "csv"]).default("markdown").describe("Report format: markdown summary, JSON object, or flat CSV table."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct BankStatementReconcile;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/bank-statement-reconcile",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Reconcile bank statement and ledger CSV rows",
    skill(
        description = "Reconcile a bank-statement CSV against a bookkeeping ledger CSV by signed amount and date, then use fuzzy memo token similarity to classify exact matches, suggested matches, unmatched statement items, and unmatched ledger items. Column references can be header names or 1-based indices; amounts accept signs, currency symbols, thousands separators, and accounting parentheses. Outputs markdown, JSON, or CSV.",
        parameters = schema_json()
    ),
)]
impl BankStatementReconcile {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "bank-statement-reconcile", |a: Args| {
            gizza_ai_bank_statement_reconcile_core::reconcile(
                &a.statement_csv,
                &a.ledger_csv,
                &a.stmt_date,
                &a.stmt_amount,
                &a.stmt_memo,
                &a.ledger_date,
                &a.ledger_amount,
                &a.ledger_memo,
                a.date_tolerance_days,
                a.amount_tolerance,
                a.memo_threshold,
                &a.delimiter,
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

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "statement_csv":       { "type": "string", "description": "Bank statement CSV text with a header row." },
                    "ledger_csv":          { "type": "string", "description": "Bookkeeping ledger CSV text with a header row." },
                    "stmt_date":           { "type": "string", "default": "date", "description": "Statement date column name or 1-based index." },
                    "stmt_amount":         { "type": "string", "default": "amount", "description": "Statement signed amount column name or 1-based index." },
                    "stmt_memo":           { "type": "string", "default": "memo", "description": "Statement memo/description column name or 1-based index." },
                    "ledger_date":         { "type": "string", "default": "date", "description": "Ledger date column name or 1-based index." },
                    "ledger_amount":       { "type": "string", "default": "amount", "description": "Ledger signed amount column name or 1-based index." },
                    "ledger_memo":         { "type": "string", "default": "memo", "description": "Ledger memo/description column name or 1-based index." },
                    "date_tolerance_days": { "type": "integer", "minimum": 0, "maximum": 30, "default": 3, "description": "Maximum absolute date difference, in days, for a candidate match." },
                    "amount_tolerance":    { "type": "number", "minimum": 0, "default": 0.01, "description": "Maximum absolute signed-amount difference for a candidate match." },
                    "memo_threshold":      { "type": "integer", "minimum": 0, "maximum": 100, "default": 70, "description": "Minimum fuzzy memo token similarity (0–100) to classify a candidate as matched instead of suggested." },
                    "delimiter":           { "type": "string", "enum": ["comma", "tab", "semicolon", "pipe"], "default": "comma", "description": "CSV field delimiter used by both inputs." },
                    "output":              { "type": "string", "enum": ["markdown", "json", "csv"], "default": "markdown", "description": "Report format: markdown summary, JSON object, or flat CSV table." }
                },
                "required": ["statement_csv", "ledger_csv"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
