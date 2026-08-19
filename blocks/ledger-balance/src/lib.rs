//! gizza-ai/ledger-balance — chat skill block on the shared tool abstraction.
//! Parses a ledger-cli / hledger plain-text journal and reports the balance of
//! every account and sub-account. The chat schema is single-sourced from
//! descriptor() (which also drives the CLI); handle() delegates to
//! block_utils::run_skill. Pure compute, no host calls — runs entirely inside
//! the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn default_layout() -> String {
    "tree".to_string()
}
fn default_sort() -> String {
    "account".to_string()
}
fn default_status() -> String {
    "all".to_string()
}
fn default_output_format() -> String {
    "text".to_string()
}
fn default_show_total() -> bool {
    true
}

#[derive(Deserialize)]
struct Args {
    journal: String,
    #[serde(default)]
    account_filter: String,
    #[serde(default)]
    depth: i64,
    #[serde(default = "default_layout")]
    layout: String,
    #[serde(default = "default_sort")]
    sort: String,
    #[serde(default)]
    begin: String,
    #[serde(default)]
    end: String,
    #[serde(default = "default_status")]
    status: String,
    #[serde(default)]
    include_empty: bool,
    #[serde(default)]
    real_only: bool,
    #[serde(default)]
    cost_basis: bool,
    #[serde(default = "default_show_total")]
    show_total: bool,
    #[serde(default)]
    percent: bool,
    #[serde(default = "default_output_format")]
    output_format: String,
}

/// Single source for the chat schema (and CLI). Param order mirrors the core
/// `balances` signature and the web `run` export so all surfaces stay in sync.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("journal")
                .required()
                .describe("The ledger-cli / hledger plain-text journal to total, e.g. '2024-01-05 * Groceries' followed by indented postings like '    Expenses:Food  $45.20'. Up to 5000 transactions per run."),
        )
        .param(
            Param::string("account_filter")
                .default("")
                .describe("Comma-separated case-insensitive substrings an account must contain, e.g. 'expenses, assets'. Prefix a pattern with 'not:' or '-' to exclude instead, e.g. 'expenses, not:coffee'. Blank (default) keeps every account."),
        )
        .param(
            Param::integer("depth")
                .default(0)
                .min(0.0)
                .max(10.0)
                .describe("Fold accounts deeper than this many ':'-separated levels into their ancestor, e.g. 1 reports only Assets/Expenses/Income. 0 (default) keeps the full hierarchy."),
        )
        .param(
            Param::enumv("layout", ["tree", "flat"])
                .default("tree")
                .describe("tree (default) indents sub-accounts under their parent and rolls their totals up; flat lists each posted account once under its full name with no roll-up."),
        )
        .param(
            Param::enumv("sort", ["account", "amount", "amount-asc"])
                .default("account")
                .describe("Row order: account (default, alphabetical), amount (largest balance first), or amount-asc (smallest first). Amount sorting ranks by the journal's most-used commodity."),
        )
        .param(
            Param::string("begin")
                .default("")
                .describe("Only count transactions dated on or after this date, as YYYY-MM-DD, e.g. '2024-01-01'. Blank (default) starts at the earliest transaction."),
        )
        .param(
            Param::string("end")
                .default("")
                .describe("Only count transactions dated BEFORE this date (exclusive, as ledger and hledger do), as YYYY-MM-DD, e.g. '2024-04-01' for Q1. Blank (default) runs to the latest transaction."),
        )
        .param(
            Param::enumv("status", ["all", "cleared", "pending", "unmarked"])
                .default("all")
                .describe("Keep only transactions with this status flag: all (default), cleared ('*'), pending ('!') or unmarked (no flag)."),
        )
        .param(
            Param::boolean("include_empty")
                .default(false)
                .describe("When true, keep accounts whose balance is zero and accounts merely declared with an 'account' directive. Default false hides them."),
        )
        .param(
            Param::boolean("real_only")
                .default(false)
                .describe("When true, ignore virtual postings written in '(…)' or '[…]' brackets (budget/envelope legs). Default false counts them."),
        )
        .param(
            Param::boolean("cost_basis")
                .default(false)
                .describe("When true, report postings that carry an '@' unit price or '@@' total price in the price's commodity instead of the original one, e.g. '10 AAPL @ $50.00' totals as $500.00. Default false keeps the original commodity."),
        )
        .param(
            Param::boolean("show_total")
                .default(true)
                .describe("Append the grand-total row under a dashed rule (a balanced journal totals zero). Default true; set false to suppress it."),
        )
        .param(
            Param::boolean("percent")
                .default(false)
                .describe("Add a column showing each row as a percentage of its own top-level account, so a balanced journal still gives useful shares. Default false."),
        )
        .param(
            Param::enumv("output_format", ["text", "csv", "json", "markdown"])
                .default("text")
                .describe("How to render the report: text (default, aligned amounts like the ledger CLI), csv (account,commodity,amount rows), json (structured accounts + totals), or markdown (a table to paste into notes)."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/ledger-balance",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Total a ledger-cli / hledger plain-text journal into a balance report for every account and sub-account",
    skill(
        description = "Compute account balances from a ledger-cli / hledger plain-text journal. Pass the pasted journal as 'journal'; every posting is summed into its account and rolled up the ':' hierarchy, exactly like the `ledger balance` / `hledger balance` report. layout is tree (indented, with roll-up) or flat (full account names); depth folds deeper accounts into their ancestor; account_filter takes comma-separated case-insensitive substrings with a 'not:'/'-' prefix to exclude; begin/end select a date range (end is exclusive); status keeps all/cleared/pending/unmarked transactions; include_empty keeps zero and merely-declared accounts; real_only drops virtual '(…)'/'[…]' postings; cost_basis reports '@'/'@@' priced postings in their cost commodity; show_total prints the grand total; percent adds a share-of-top-level-account column; output_format is text, csv, json or markdown. The parser handles multi-commodity amounts, both '1,234.56' and '1.234,56' number styles, an inferred amount-less posting, balance assertions (parsed, never counted), and the account/alias/commodity/D/Y/apply account/comment directives. 'include FILE' is skipped (no filesystem) and reported in the notes. Up to 5000 transactions per run. Runs locally; nothing is uploaded.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": ... }.
        match run_skill(&body, "ledger-balance", |a: Args| {
            gizza_ai_ledger_balance_core::balances(
                &a.journal,
                &a.account_filter,
                a.depth.max(0) as usize,
                &a.layout,
                &a.sort,
                &a.begin,
                &a.end,
                &a.status,
                a.include_empty,
                a.real_only,
                a.cost_basis,
                a.show_total,
                a.percent,
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
                    "journal": { "type": "string", "description": "The ledger-cli / hledger plain-text journal to total, e.g. '2024-01-05 * Groceries' followed by indented postings like '    Expenses:Food  $45.20'. Up to 5000 transactions per run." },
                    "account_filter": { "type": "string", "default": "", "description": "Comma-separated case-insensitive substrings an account must contain, e.g. 'expenses, assets'. Prefix a pattern with 'not:' or '-' to exclude instead, e.g. 'expenses, not:coffee'. Blank (default) keeps every account." },
                    "depth": { "type": "integer", "minimum": 0, "maximum": 10, "default": 0, "description": "Fold accounts deeper than this many ':'-separated levels into their ancestor, e.g. 1 reports only Assets/Expenses/Income. 0 (default) keeps the full hierarchy." },
                    "layout": { "type": "string", "enum": ["tree", "flat"], "default": "tree", "description": "tree (default) indents sub-accounts under their parent and rolls their totals up; flat lists each posted account once under its full name with no roll-up." },
                    "sort": { "type": "string", "enum": ["account", "amount", "amount-asc"], "default": "account", "description": "Row order: account (default, alphabetical), amount (largest balance first), or amount-asc (smallest first). Amount sorting ranks by the journal's most-used commodity." },
                    "begin": { "type": "string", "default": "", "description": "Only count transactions dated on or after this date, as YYYY-MM-DD, e.g. '2024-01-01'. Blank (default) starts at the earliest transaction." },
                    "end": { "type": "string", "default": "", "description": "Only count transactions dated BEFORE this date (exclusive, as ledger and hledger do), as YYYY-MM-DD, e.g. '2024-04-01' for Q1. Blank (default) runs to the latest transaction." },
                    "status": { "type": "string", "enum": ["all", "cleared", "pending", "unmarked"], "default": "all", "description": "Keep only transactions with this status flag: all (default), cleared ('*'), pending ('!') or unmarked (no flag)." },
                    "include_empty": { "type": "boolean", "default": false, "description": "When true, keep accounts whose balance is zero and accounts merely declared with an 'account' directive. Default false hides them." },
                    "real_only": { "type": "boolean", "default": false, "description": "When true, ignore virtual postings written in '(…)' or '[…]' brackets (budget/envelope legs). Default false counts them." },
                    "cost_basis": { "type": "boolean", "default": false, "description": "When true, report postings that carry an '@' unit price or '@@' total price in the price's commodity instead of the original one, e.g. '10 AAPL @ $50.00' totals as $500.00. Default false keeps the original commodity." },
                    "show_total": { "type": "boolean", "default": true, "description": "Append the grand-total row under a dashed rule (a balanced journal totals zero). Default true; set false to suppress it." },
                    "percent": { "type": "boolean", "default": false, "description": "Add a column showing each row as a percentage of its own top-level account, so a balanced journal still gives useful shares. Default false." },
                    "output_format": { "type": "string", "enum": ["text", "csv", "json", "markdown"], "default": "text", "description": "How to render the report: text (default, aligned amounts like the ledger CLI), csv (account,commodity,amount rows), json (structured accounts + totals), or markdown (a table to paste into notes)." }
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
