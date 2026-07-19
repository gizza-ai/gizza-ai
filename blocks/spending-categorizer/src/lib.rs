//! gizza-ai/spending-categorizer — chat skill block on the shared tool abstraction.
//! Auto-categorizes a bank/credit-card CSV export by merchant keywords and
//! summarizes spending by category. The chat schema is single-sourced from
//! descriptor() (which also drives the CLI); handle() delegates to
//! block_utils::run_skill. Pure compute, no host calls.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn default_output() -> String {
    "both".to_string()
}
fn default_currency() -> String {
    "$".to_string()
}
fn default_delimiter() -> String {
    "auto".to_string()
}

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default)]
    description_column: String,
    #[serde(default)]
    amount_column: String,
    #[serde(default)]
    debit_column: String,
    #[serde(default)]
    credit_column: String,
    #[serde(default)]
    date_column: String,
    #[serde(default)]
    rules: String,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default = "default_currency")]
    currency: String,
    #[serde(default = "default_delimiter")]
    delimiter: String,
    #[serde(default)]
    invert_amount: bool,
}

/// Single source for the chat schema (and CLI). Param order mirrors the core
/// `categorize_spending` signature and the web `run` export.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe("The bank or credit-card CSV export to categorize, including its header row. Money out should be negative (set invert_amount if your bank exports spending as positive). Max 10000 rows."),
        )
        .param(
            Param::string("description_column")
                .default("")
                .describe("Header name of the description/merchant column. Blank (default) auto-detects description/payee/narration/details/memo/merchant/name/reference/particulars, plus common European names (Beschreibung, Omschrijving, Libellé, Concepto)."),
        )
        .param(
            Param::string("amount_column")
                .default("")
                .describe("Header name of a single signed amount column (money out negative, money in positive). Blank (default) auto-detects amount/value (plus Betrag, Bedrag, Montant, Importe), else falls back to debit_column/credit_column."),
        )
        .param(
            Param::string("debit_column")
                .default("")
                .describe("Header name of the money-out column, when the statement splits amounts into two columns instead of one signed one. Blank (default) auto-detects debit/withdrawal/paid out/outflow."),
        )
        .param(
            Param::string("credit_column")
                .default("")
                .describe("Header name of the money-in column, paired with debit_column. Blank (default) auto-detects credit/deposit/paid in/inflow."),
        )
        .param(
            Param::string("date_column")
                .default("")
                .describe("Header name of the transaction-date column, echoed into the categorized CSV. Blank (default) auto-detects a column named like date/posted/posting/datum/fecha; if none exists the output simply has no Date column."),
        )
        .param(
            Param::string("rules")
                .default("")
                .describe("Your own categorization rules, one 'keyword = Category' per line (case-insensitive substring match; '=', '=>' or '->' as the separator; '#' comments allowed). Checked before the built-in keyword table, e.g. 'starbucks = Coffee'."),
        )
        .param(
            Param::enumv("output", ["both", "summary", "csv"])
                .default("both")
                .describe("What to return: summary (per-category totals, share of spending, txn counts and a bar chart), csv (the rows with a Category column appended, ready to import), or both. Default both."),
        )
        .param(
            Param::string("currency")
                .default("$")
                .describe("Currency to print in the summary. A symbol ($, £, €) is prefixed ($42.50); an alphabetic code (USD) is suffixed (42.50 USD). Blank prints bare numbers. Default '$'."),
        )
        .param(
            Param::enumv("delimiter", ["auto", "comma", "semicolon", "tab", "pipe"])
                .default("auto")
                .describe("Field separator of the CSV. Default auto sniffs it from the header row; set it explicitly when detection guesses wrong."),
        )
        .param(
            Param::boolean("invert_amount")
                .default(false)
                .describe("Flip the sign of every amount. Use when a statement exports spending as positive and income as negative (common for card exports). Default false."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/spending-categorizer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Auto-categorize a bank/card CSV by merchant keywords and summarize spending by category",
    skill(
        description = "Auto-categorize a bank or credit-card CSV export and summarize spending by category. Pass the pasted CSV (with a header row) as 'data'. The description/merchant, amount and date columns are auto-detected (or name them with description_column/amount_column/date_column); statements that split money out and money in into two columns are handled via debit_column/credit_column. Each row's category is chosen by your own 'keyword = Category' rules first, then a built-in merchant keyword table (groceries, dining, transport, fuel, subscriptions, utilities, rent, insurance, health, entertainment, travel, fees, transfers, …), then a sign-based fallback (Other for money out, Income for money in). output picks what comes back: summary (per-category totals, share of total spending, transaction counts and a proportional text bar chart, plus Total spending / Income / Net cash flow), csv (the original rows with a Category column appended, ready to import into a spreadsheet), or both (default). currency prints as a $ prefix or a USD suffix; delimiter is auto/comma/semicolon/tab/pipe; invert_amount flips signs when a bank exports spending as positive. Handles US (1,234.56) and EU (1.234,56) amounts, (parentheses) and DR/CR negatives. Max 10000 rows. Runs locally; nothing is uploaded.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": ... }.
        match run_skill(&body, "spending-categorizer", |a: Args| {
            gizza_ai_spending_categorizer_core::categorize_spending(
                &a.data,
                &a.description_column,
                &a.amount_column,
                &a.debit_column,
                &a.credit_column,
                &a.date_column,
                &a.rules,
                &a.output,
                &a.currency,
                &a.delimiter,
                a.invert_amount,
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
                    "data": { "type": "string", "description": "The bank or credit-card CSV export to categorize, including its header row. Money out should be negative (set invert_amount if your bank exports spending as positive). Max 10000 rows." },
                    "description_column": { "type": "string", "default": "", "description": "Header name of the description/merchant column. Blank (default) auto-detects description/payee/narration/details/memo/merchant/name/reference/particulars, plus common European names (Beschreibung, Omschrijving, Libellé, Concepto)." },
                    "amount_column": { "type": "string", "default": "", "description": "Header name of a single signed amount column (money out negative, money in positive). Blank (default) auto-detects amount/value (plus Betrag, Bedrag, Montant, Importe), else falls back to debit_column/credit_column." },
                    "debit_column": { "type": "string", "default": "", "description": "Header name of the money-out column, when the statement splits amounts into two columns instead of one signed one. Blank (default) auto-detects debit/withdrawal/paid out/outflow." },
                    "credit_column": { "type": "string", "default": "", "description": "Header name of the money-in column, paired with debit_column. Blank (default) auto-detects credit/deposit/paid in/inflow." },
                    "date_column": { "type": "string", "default": "", "description": "Header name of the transaction-date column, echoed into the categorized CSV. Blank (default) auto-detects a column named like date/posted/posting/datum/fecha; if none exists the output simply has no Date column." },
                    "rules": { "type": "string", "default": "", "description": "Your own categorization rules, one 'keyword = Category' per line (case-insensitive substring match; '=', '=>' or '->' as the separator; '#' comments allowed). Checked before the built-in keyword table, e.g. 'starbucks = Coffee'." },
                    "output": { "type": "string", "enum": ["both", "summary", "csv"], "default": "both", "description": "What to return: summary (per-category totals, share of spending, txn counts and a bar chart), csv (the rows with a Category column appended, ready to import), or both. Default both." },
                    "currency": { "type": "string", "default": "$", "description": "Currency to print in the summary. A symbol ($, £, €) is prefixed ($42.50); an alphabetic code (USD) is suffixed (42.50 USD). Blank prints bare numbers. Default '$'." },
                    "delimiter": { "type": "string", "enum": ["auto", "comma", "semicolon", "tab", "pipe"], "default": "auto", "description": "Field separator of the CSV. Default auto sniffs it from the header row; set it explicitly when detection guesses wrong." },
                    "invert_amount": { "type": "boolean", "default": false, "description": "Flip the sign of every amount. Use when a statement exports spending as positive and income as negative (common for card exports). Default false." }
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
