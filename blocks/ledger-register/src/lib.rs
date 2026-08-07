//! gizza-ai/ledger-register — chat skill block on the shared tool abstraction.
//! Parses a ledger-cli / hledger plain-text journal and prints the register:
//! one line per matching posting, with a running total in the last column. The
//! chat schema is single-sourced from descriptor() (which also drives the CLI);
//! handle() delegates to block_utils::run_skill. Pure compute, no host calls —
//! runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn default_status() -> String {
    "all".to_string()
}
fn default_running_total() -> String {
    "period".to_string()
}
fn default_sort() -> String {
    "date".to_string()
}
fn default_limit_from() -> String {
    "first".to_string()
}
fn default_width() -> i64 {
    80
}
fn default_output_format() -> String {
    "text".to_string()
}

#[derive(Deserialize)]
struct Args {
    journal: String,
    #[serde(default)]
    account_filter: String,
    #[serde(default)]
    payee_filter: String,
    #[serde(default)]
    begin: String,
    #[serde(default)]
    end: String,
    #[serde(default = "default_status")]
    status: String,
    #[serde(default)]
    depth: i64,
    #[serde(default = "default_running_total")]
    running_total: String,
    #[serde(default)]
    related: bool,
    #[serde(default)]
    invert: bool,
    #[serde(default)]
    real_only: bool,
    #[serde(default)]
    cost_basis: bool,
    #[serde(default = "default_sort")]
    sort: String,
    #[serde(default)]
    limit: i64,
    #[serde(default = "default_limit_from")]
    limit_from: String,
    #[serde(default = "default_width")]
    width: i64,
    #[serde(default = "default_output_format")]
    output_format: String,
}

/// Single source for the chat schema (and CLI). Param order mirrors the core
/// `register` signature and the web `run` export so all surfaces stay in sync.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("journal")
                .required()
                .describe("The ledger-cli / hledger plain-text journal to register, e.g. '2024-01-05 * Groceries' followed by indented postings like '    Expenses:Food  $45.20'. Up to 5000 transactions per run."),
        )
        .param(
            Param::string("account_filter")
                .default("")
                .describe("Comma-separated case-insensitive substrings an account must contain, e.g. 'checking'. Prefix a pattern with 'not:' or '-' to exclude instead, e.g. 'expenses, not:coffee'. Blank (default) lists every posting."),
        )
        .param(
            Param::string("payee_filter")
                .default("")
                .describe("Same syntax as account_filter, but matched against the transaction description / payee, e.g. 'coffee' or 'rent, not:refund'. Blank (default) keeps every description."),
        )
        .param(
            Param::string("begin")
                .default("")
                .describe("Only list postings dated on or after this date, as YYYY-MM-DD, e.g. '2024-01-01'. Blank (default) starts at the earliest transaction."),
        )
        .param(
            Param::string("end")
                .default("")
                .describe("Only list postings dated BEFORE this date (exclusive, as ledger and hledger do), as YYYY-MM-DD, e.g. '2024-04-01' for Q1. Blank (default) runs to the latest transaction."),
        )
        .param(
            Param::enumv("status", ["all", "cleared", "pending", "unmarked"])
                .default("all")
                .describe("Keep only transactions with this status flag: all (default), cleared ('*'), pending ('!') or unmarked (no flag)."),
        )
        .param(
            Param::integer("depth")
                .default(0)
                .min(0.0)
                .max(10.0)
                .describe("Shorten account names to this many ':'-separated levels, e.g. 2 turns 'Expenses:Food:Coffee' into 'Expenses:Food'. 0 (default) prints the full name."),
        )
        .param(
            Param::enumv("running_total", ["period", "historical", "average", "none"])
                .default("period")
                .describe("What the last column shows: period (default) is the running total from the report start; historical carries in the balance of everything before 'begin'; average is the running average of the postings so far; none drops the column."),
        )
        .param(
            Param::boolean("related")
                .default(false)
                .describe("When true, print the OTHER side of every transaction that touches the filtered accounts — the categories a bank account's money went to — instead of the matching postings themselves. Requires account_filter. Default false."),
        )
        .param(
            Param::boolean("invert")
                .default(false)
                .describe("When true, flip the sign of every amount and running total, so an income or liability account reads as positive. Default false."),
        )
        .param(
            Param::boolean("real_only")
                .default(false)
                .describe("When true, ignore virtual postings written in '(…)' or '[…]' brackets (budget/envelope legs). Default false lists them."),
        )
        .param(
            Param::boolean("cost_basis")
                .default(false)
                .describe("When true, list postings that carry an '@' unit price or '@@' total price in the price's commodity instead of the original one, e.g. '10 AAPL @ $50.00' registers as $500.00. Default false keeps the original commodity."),
        )
        .param(
            Param::enumv("sort", ["date", "date-desc", "amount", "amount-asc", "account"])
                .default("date")
                .describe("Row order: date (default, oldest first — the order a running total is meant to be read in), date-desc (newest first), amount (largest first), amount-asc (smallest first) or account (alphabetical). The running total always accumulates in the order shown."),
        )
        .param(
            Param::integer("limit")
                .default(0)
                .min(0.0)
                .max(10000.0)
                .describe("Print at most this many rows, e.g. 20 for a quick look at a long register. 0 (default) prints every matching posting. The running total is computed over the whole register first, then the rows are sliced."),
        )
        .param(
            Param::enumv("limit_from", ["first", "last"])
                .default("first")
                .describe("Which end 'limit' keeps: first (default, the oldest rows) or last (the most recent rows, whose final running total is the current balance). Ignored when limit is 0."),
        )
        .param(
            Param::integer("width")
                .default(80)
                .min(40.0)
                .max(400.0)
                .describe("Total width of the text report in columns, 40-400 (default 80). The amount columns size to their contents; the description and account columns share what is left and are clipped with '..'. Ignored by the csv, json and markdown formats."),
        )
        .param(
            Param::enumv("output_format", ["text", "csv", "json", "markdown"])
                .default("text")
                .describe("How to render the register: text (default, aligned columns like the ledger CLI), csv (date,description,account,commodity,amount,total rows), json (structured postings) or markdown (a table to paste into notes)."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/ledger-register",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "List every posting of a ledger-cli / hledger journal with a running total, filtered by account, payee or date range",
    skill(
        description = "Produce a register report from a ledger-cli / hledger plain-text journal: one line per matching posting — date, description, account, amount — with a running total in the last column, exactly like the `ledger register` / `hledger register` report. Pass the pasted journal as 'journal'. account_filter and payee_filter take comma-separated case-insensitive substrings with a 'not:'/'-' prefix to exclude; begin/end select a date range (end is exclusive); status keeps all/cleared/pending/unmarked transactions; depth shortens account names; running_total is period (from the report start), historical (carry in the balance before 'begin'), average (running average) or none; related prints the other side of each matching transaction; invert flips every sign; real_only drops virtual '(…)'/'[…]' postings; cost_basis lists '@'/'@@' priced postings in their cost commodity; sort orders the rows; limit + limit_from keep the first or last N rows; width sets the text report width; output_format is text, csv, json or markdown. The parser handles multi-commodity amounts, both '1,234.56' and '1.234,56' number styles, an inferred amount-less posting, balance assertions (parsed, never counted), and the account/alias/commodity/D/Y/apply account/comment directives. 'include FILE' is skipped (no filesystem) and reported in the notes. Up to 5000 transactions per run. Runs locally; nothing is uploaded.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": ... }.
        match run_skill(&body, "ledger-register", |a: Args| {
            gizza_ai_ledger_register_core::register(
                &a.journal,
                &a.account_filter,
                &a.payee_filter,
                &a.begin,
                &a.end,
                &a.status,
                a.depth.max(0) as usize,
                &a.running_total,
                a.related,
                a.invert,
                a.real_only,
                a.cost_basis,
                &a.sort,
                a.limit.max(0) as usize,
                &a.limit_from,
                a.width.max(0) as usize,
                &a.output_format,
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
                    "journal": { "type": "string", "description": "The ledger-cli / hledger plain-text journal to register, e.g. '2024-01-05 * Groceries' followed by indented postings like '    Expenses:Food  $45.20'. Up to 5000 transactions per run." },
                    "account_filter": { "type": "string", "default": "", "description": "Comma-separated case-insensitive substrings an account must contain, e.g. 'checking'. Prefix a pattern with 'not:' or '-' to exclude instead, e.g. 'expenses, not:coffee'. Blank (default) lists every posting." },
                    "payee_filter": { "type": "string", "default": "", "description": "Same syntax as account_filter, but matched against the transaction description / payee, e.g. 'coffee' or 'rent, not:refund'. Blank (default) keeps every description." },
                    "begin": { "type": "string", "default": "", "description": "Only list postings dated on or after this date, as YYYY-MM-DD, e.g. '2024-01-01'. Blank (default) starts at the earliest transaction." },
                    "end": { "type": "string", "default": "", "description": "Only list postings dated BEFORE this date (exclusive, as ledger and hledger do), as YYYY-MM-DD, e.g. '2024-04-01' for Q1. Blank (default) runs to the latest transaction." },
                    "status": { "type": "string", "enum": ["all", "cleared", "pending", "unmarked"], "default": "all", "description": "Keep only transactions with this status flag: all (default), cleared ('*'), pending ('!') or unmarked (no flag)." },
                    "depth": { "type": "integer", "minimum": 0, "maximum": 10, "default": 0, "description": "Shorten account names to this many ':'-separated levels, e.g. 2 turns 'Expenses:Food:Coffee' into 'Expenses:Food'. 0 (default) prints the full name." },
                    "running_total": { "type": "string", "enum": ["period", "historical", "average", "none"], "default": "period", "description": "What the last column shows: period (default) is the running total from the report start; historical carries in the balance of everything before 'begin'; average is the running average of the postings so far; none drops the column." },
                    "related": { "type": "boolean", "default": false, "description": "When true, print the OTHER side of every transaction that touches the filtered accounts — the categories a bank account's money went to — instead of the matching postings themselves. Requires account_filter. Default false." },
                    "invert": { "type": "boolean", "default": false, "description": "When true, flip the sign of every amount and running total, so an income or liability account reads as positive. Default false." },
                    "real_only": { "type": "boolean", "default": false, "description": "When true, ignore virtual postings written in '(…)' or '[…]' brackets (budget/envelope legs). Default false lists them." },
                    "cost_basis": { "type": "boolean", "default": false, "description": "When true, list postings that carry an '@' unit price or '@@' total price in the price's commodity instead of the original one, e.g. '10 AAPL @ $50.00' registers as $500.00. Default false keeps the original commodity." },
                    "sort": { "type": "string", "enum": ["date", "date-desc", "amount", "amount-asc", "account"], "default": "date", "description": "Row order: date (default, oldest first — the order a running total is meant to be read in), date-desc (newest first), amount (largest first), amount-asc (smallest first) or account (alphabetical). The running total always accumulates in the order shown." },
                    "limit": { "type": "integer", "minimum": 0, "maximum": 10000, "default": 0, "description": "Print at most this many rows, e.g. 20 for a quick look at a long register. 0 (default) prints every matching posting. The running total is computed over the whole register first, then the rows are sliced." },
                    "limit_from": { "type": "string", "enum": ["first", "last"], "default": "first", "description": "Which end 'limit' keeps: first (default, the oldest rows) or last (the most recent rows, whose final running total is the current balance). Ignored when limit is 0." },
                    "width": { "type": "integer", "minimum": 40, "maximum": 400, "default": 80, "description": "Total width of the text report in columns, 40-400 (default 80). The amount columns size to their contents; the description and account columns share what is left and are clipped with '..'. Ignored by the csv, json and markdown formats." },
                    "output_format": { "type": "string", "enum": ["text", "csv", "json", "markdown"], "default": "text", "description": "How to render the register: text (default, aligned columns like the ledger CLI), csv (date,description,account,commodity,amount,total rows), json (structured postings) or markdown (a table to paste into notes)." }
                },
                "required": ["journal"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
