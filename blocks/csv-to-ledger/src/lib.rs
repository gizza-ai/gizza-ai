//! gizza-ai/csv-to-ledger — chat skill block on the shared tool abstraction.
//! Turns a bank / credit-card CSV export into double-entry ledger-cli / hledger
//! journal transactions. The chat schema is single-sourced from descriptor()
//! (which also drives the CLI); handle() delegates to block_utils::run_skill.
//! Pure compute, no host calls — runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn default_currency() -> String {
    "$".to_string()
}
fn default_date_format() -> String {
    "auto".to_string()
}
fn default_output_format() -> String {
    "ledger".to_string()
}
fn default_delimiter() -> String {
    "auto".to_string()
}

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default)]
    date_column: String,
    #[serde(default)]
    description_column: String,
    #[serde(default)]
    amount_column: String,
    #[serde(default)]
    debit_column: String,
    #[serde(default)]
    credit_column: String,
    #[serde(default)]
    asset_account: String,
    #[serde(default)]
    default_expense_account: String,
    #[serde(default)]
    default_income_account: String,
    #[serde(default)]
    account_rules: String,
    #[serde(default = "default_currency")]
    currency: String,
    #[serde(default = "default_date_format")]
    date_format: String,
    #[serde(default = "default_output_format")]
    output_format: String,
    #[serde(default = "default_delimiter")]
    delimiter: String,
    #[serde(default)]
    invert_amount: bool,
}

/// Single source for the chat schema (and CLI). Param order mirrors the core
/// `to_ledger` signature and the web `run` export so all surfaces stay in sync.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe("The bank or credit-card CSV export to convert, including its header row. Each row becomes one balanced ledger transaction."),
        )
        .param(
            Param::string("date_column")
                .default("")
                .describe("Header name of the transaction-date column. Blank (default) auto-detects a column named like date/posted/posting."),
        )
        .param(
            Param::string("description_column")
                .default("")
                .describe("Header name of the description/payee column. Blank (default) auto-detects description/payee/narration/details/memo/name/reference/particulars; missing → '(unknown)'."),
        )
        .param(
            Param::string("amount_column")
                .default("")
                .describe("Header name of a single signed amount column (money out negative, money in positive). Blank (default) auto-detects amount/value, else falls back to debit_column/credit_column."),
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
            Param::string("asset_account")
                .default("")
                .describe("The bank/card account this statement represents, used as one leg of every transaction. Blank (default) uses 'Assets:Bank:Checking'."),
        )
        .param(
            Param::string("default_expense_account")
                .default("")
                .describe("Fallback counter-account for money-out rows that match no rule or keyword. Blank (default) uses 'Expenses:Unknown'."),
        )
        .param(
            Param::string("default_income_account")
                .default("")
                .describe("Fallback counter-account for money-in rows that match no rule or keyword. Blank (default) uses 'Income:Unknown'."),
        )
        .param(
            Param::string("account_rules")
                .default("")
                .describe("Your own categorization rules, one 'pattern = Account:Name' per line (case-insensitive substring match; '=', '=>' or '->' as the separator; '#' comments allowed). These override the built-in keyword table, e.g. 'starbucks = Expenses:Coffee'."),
        )
        .param(
            Param::string("currency")
                .default("$")
                .describe("Commodity to print on amounts. A symbol ($, £, €) is prefixed ($-4.50); an alphabetic code (USD) is suffixed (-4.50 USD). Blank prints the bare number. Default '$'."),
        )
        .param(
            Param::enumv("date_format", ["auto", "ymd", "mdy", "dmy"])
                .default("auto")
                .describe("How to read numeric dates: auto (detect), ymd (2024-01-31), mdy (US 01/31/2024), or dmy (EU 31/01/2024). Default auto."),
        )
        .param(
            Param::enumv("output_format", ["ledger", "hledger"])
                .default("ledger")
                .describe("Journal style: ledger prints an explicit amount on both postings (fully balanced); hledger omits the amount on the asset posting and lets the tool infer it (compact). Both are valid ledger-cli/hledger. Default ledger."),
        )
        .param(
            Param::enumv("delimiter", ["auto", "comma", "semicolon", "tab", "pipe"])
                .default("auto")
                .describe("Field separator of the CSV. Default auto sniffs it from the header row; set it explicitly when detection guesses wrong."),
        )
        .param(
            Param::boolean("invert_amount")
                .default(false)
                .describe("Flip the sign of every amount. Use when a statement exports spending as positive and income as negative (so amounts land on the correct expense/income side). Default false."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/csv-to-ledger",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert a bank/credit-card CSV export into double-entry ledger-cli / hledger journal entries",
    skill(
        description = "Convert a bank or credit-card CSV export into double-entry ledger-cli / hledger journal transactions. Pass the pasted CSV (with a header row) as 'data'. Each row becomes one balanced transaction: a posting to your asset/bank account and a balancing posting to a guessed counter-account. The date, description/payee, and amount columns are auto-detected (or name them with date_column/description_column/amount_column); a statement that splits money out and money in into two columns is handled via debit_column/credit_column. The counter-account is chosen by your own 'pattern = Account' account_rules first, then a built-in keyword table (groceries, transport, utilities, salary, …), then a sign-based fallback (default_expense_account for money out, default_income_account for money in). Set asset_account to the account the statement represents. currency prints as a $ prefix or a USD suffix; date_format is auto/ymd/mdy/dmy; output_format is ledger (explicit amounts) or hledger (compact); delimiter is auto/comma/semicolon/tab/pipe; invert_amount flips the sign when a bank exports spending as positive. Runs locally; nothing is uploaded.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": ... }.
        match run_skill(&body, "csv-to-ledger", |a: Args| {
            gizza_ai_csv_to_ledger_core::to_ledger(
                &a.data,
                &a.date_column,
                &a.description_column,
                &a.amount_column,
                &a.debit_column,
                &a.credit_column,
                &a.asset_account,
                &a.default_expense_account,
                &a.default_income_account,
                &a.account_rules,
                &a.currency,
                &a.date_format,
                &a.output_format,
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
                    "data": { "type": "string", "description": "The bank or credit-card CSV export to convert, including its header row. Each row becomes one balanced ledger transaction." },
                    "date_column": { "type": "string", "default": "", "description": "Header name of the transaction-date column. Blank (default) auto-detects a column named like date/posted/posting." },
                    "description_column": { "type": "string", "default": "", "description": "Header name of the description/payee column. Blank (default) auto-detects description/payee/narration/details/memo/name/reference/particulars; missing → '(unknown)'." },
                    "amount_column": { "type": "string", "default": "", "description": "Header name of a single signed amount column (money out negative, money in positive). Blank (default) auto-detects amount/value, else falls back to debit_column/credit_column." },
                    "debit_column": { "type": "string", "default": "", "description": "Header name of the money-out column, when the statement splits amounts into two columns instead of one signed one. Blank (default) auto-detects debit/withdrawal/paid out/outflow." },
                    "credit_column": { "type": "string", "default": "", "description": "Header name of the money-in column, paired with debit_column. Blank (default) auto-detects credit/deposit/paid in/inflow." },
                    "asset_account": { "type": "string", "default": "", "description": "The bank/card account this statement represents, used as one leg of every transaction. Blank (default) uses 'Assets:Bank:Checking'." },
                    "default_expense_account": { "type": "string", "default": "", "description": "Fallback counter-account for money-out rows that match no rule or keyword. Blank (default) uses 'Expenses:Unknown'." },
                    "default_income_account": { "type": "string", "default": "", "description": "Fallback counter-account for money-in rows that match no rule or keyword. Blank (default) uses 'Income:Unknown'." },
                    "account_rules": { "type": "string", "default": "", "description": "Your own categorization rules, one 'pattern = Account:Name' per line (case-insensitive substring match; '=', '=>' or '->' as the separator; '#' comments allowed). These override the built-in keyword table, e.g. 'starbucks = Expenses:Coffee'." },
                    "currency": { "type": "string", "default": "$", "description": "Commodity to print on amounts. A symbol ($, £, €) is prefixed ($-4.50); an alphabetic code (USD) is suffixed (-4.50 USD). Blank prints the bare number. Default '$'." },
                    "date_format": { "type": "string", "enum": ["auto", "ymd", "mdy", "dmy"], "default": "auto", "description": "How to read numeric dates: auto (detect), ymd (2024-01-31), mdy (US 01/31/2024), or dmy (EU 31/01/2024). Default auto." },
                    "output_format": { "type": "string", "enum": ["ledger", "hledger"], "default": "ledger", "description": "Journal style: ledger prints an explicit amount on both postings (fully balanced); hledger omits the amount on the asset posting and lets the tool infer it (compact). Both are valid ledger-cli/hledger. Default ledger." },
                    "delimiter": { "type": "string", "enum": ["auto", "comma", "semicolon", "tab", "pipe"], "default": "auto", "description": "Field separator of the CSV. Default auto sniffs it from the header row; set it explicitly when detection guesses wrong." },
                    "invert_amount": { "type": "boolean", "default": false, "description": "Flip the sign of every amount. Use when a statement exports spending as positive and income as negative (so amounts land on the correct expense/income side). Default false." }
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
