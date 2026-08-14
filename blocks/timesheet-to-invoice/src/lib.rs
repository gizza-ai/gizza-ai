//! gizza-ai/timesheet-to-invoice — turn tracked hours into a formatted invoice.
//! Thin wrapper around the pure core; chat schema single-sourced from descriptor();
//! handler delegates to run_skill. Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_timesheet_to_invoice_core::{generate, GroupBy, Options, OutputFormat};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    entries: String,
    #[serde(default = "default_rate")]
    rate: f64,
    #[serde(default = "default_currency")]
    currency: String,
    #[serde(default)]
    business: String,
    #[serde(default)]
    client: String,
    #[serde(default = "default_invoice_number")]
    invoice_number: String,
    #[serde(default)]
    issue_date: String,
    #[serde(default)]
    due_date: String,
    #[serde(default = "default_terms")]
    payment_terms: u32,
    #[serde(default = "default_tax_label")]
    tax_label: String,
    #[serde(default)]
    tax_rate: f64,
    #[serde(default)]
    discount_percent: f64,
    #[serde(default)]
    round: u32,
    #[serde(default = "default_group_by")]
    group_by: String,
    #[serde(default)]
    notes: String,
    #[serde(default = "default_format")]
    format: String,
}

fn default_rate() -> f64 {
    100.0
}
fn default_currency() -> String {
    "$".into()
}
fn default_invoice_number() -> String {
    "INV-001".into()
}
fn default_terms() -> u32 {
    30
}
fn default_tax_label() -> String {
    "Tax".into()
}
fn default_group_by() -> String {
    "entry".into()
}
fn default_format() -> String {
    "markdown".into()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("entries").required().describe("Tracked-hours lines, one billable row each, fields separated by '|' or a tab: 'description | hours', 'description | hours | rate', or with a leading service date 'YYYY-MM-DD | description | hours [| rate]'. Hours accept decimals (3.5), '2h 30m', '2:30', or a clock range such as '09:00-12:30' or '9am-5pm'. Blank lines and lines starting with # or // are ignored. Maximum 1000000 bytes and 500 billable rows."))
        .param(Param::number("rate").min(0.0).max(1000000.0).default(100.0).describe("Default hourly rate applied to every row that has no per-row rate, in the chosen currency. Default 100."))
        .param(Param::string("currency").default("$").describe("Currency symbol or code placed before each amount, such as $, £, € or 'USD '. Default '$'."))
        .param(Param::string("business").default("").describe("Your business block printed under 'From', one detail per line: name, address, email, tax id. Blank omits the block."))
        .param(Param::string("client").default("").describe("Client block printed under 'Bill to', one detail per line: company, contact, address. Blank omits the block."))
        .param(Param::string("invoice_number").default("INV-001").describe("Invoice reference shown in the heading, such as INV-001 or 2026-014. Default 'INV-001'."))
        .param(Param::string("issue_date").default("").describe("Invoice date in YYYY-MM-DD form, for example 2026-08-14. Blank omits the issue date and disables the automatic due date."))
        .param(Param::string("due_date").default("").describe("Explicit due date in YYYY-MM-DD form. Blank computes it as the issue date plus payment_terms days."))
        .param(Param::integer("payment_terms").min(0.0).max(365.0).default(30).describe("Net payment days used to compute the due date from the issue date (0 = no terms line). Common values are 7, 14, 30 and 60. Default 30."))
        .param(Param::string("tax_label").default("Tax").describe("Name of the tax line, such as Tax, VAT, GST or Sales tax. Default 'Tax'."))
        .param(Param::number("tax_rate").min(0.0).max(100.0).default(0.0).describe("Tax percentage applied to the subtotal after any discount (0 = no tax line). Default 0."))
        .param(Param::number("discount_percent").min(0.0).max(100.0).default(0.0).describe("Discount percentage applied to the subtotal before tax (0 = no discount line). Default 0."))
        .param(Param::integer("round").min(0.0).max(60.0).default(0).describe("Billing increment in minutes: each row's tracked time is rounded up to the next multiple before pricing (0 = bill exactly what was tracked). Common increments are 6 and 15."))
        .param(Param::enumv("group_by", ["entry", "description", "date"]).default("entry").describe("How rows are billed: entry keeps one line per input row, description merges rows with the same description and rate, date merges rows with the same service date and rate. Default entry."))
        .param(Param::string("notes").default("").describe("Notes or payment instructions printed at the end of the invoice, such as bank details or a late-fee policy. Blank omits the section."))
        .param(Param::enumv("format", ["markdown", "text", "csv", "json"]).default("markdown").describe("Output document: markdown (tables and bold totals), text (fixed-width plain text), csv (spreadsheet rows plus total rows), or json (full structured invoice). Default markdown."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct TimesheetToInvoice;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/timesheet-to-invoice",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Turn tracked hours and an hourly rate into a Markdown or plain-text invoice",
    skill(
        description = "Turn tracked hours into a client-ready invoice with line items, totals and payment terms. Each entry line is 'description | hours' with optional per-row rate and a leading YYYY-MM-DD service date; hours accept decimals, '2h 30m', '2:30', or clock ranges such as 09:00-12:30 and 9am-5pm. Options cover the default hourly rate, currency symbol, from/bill-to blocks, invoice number, issue date, explicit or Net-terms due date, a labelled tax percentage, a discount applied before tax, billing-increment rounding, merging rows by description or date, notes, and the output format (markdown, text, csv, json). Totals show billed hours, subtotal, discount, tax and amount due. Fully local and deterministic — no AI model, no upload.",
        parameters = schema_json()
    ),
)]
impl TimesheetToInvoice {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "timesheet-to-invoice", |a: Args| {
            let opts = Options {
                rate: a.rate,
                currency: a.currency,
                client: a.client,
                business: a.business,
                invoice_number: a.invoice_number,
                issue_date: a.issue_date,
                due_date: a.due_date,
                payment_terms: a.payment_terms as i64,
                tax_label: a.tax_label,
                tax_rate: a.tax_rate,
                discount_percent: a.discount_percent,
                round: a.round as i64,
                group_by: GroupBy::parse(&a.group_by).map_err(SkillError::InvalidArgs)?,
                notes: a.notes,
                format: OutputFormat::parse(&a.format).map_err(SkillError::InvalidArgs)?,
            };
            generate(&a.entries, &opts).map_err(SkillError::InvalidArgs)
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
                    "entries": { "type": "string", "description": "Tracked-hours lines, one billable row each, fields separated by '|' or a tab: 'description | hours', 'description | hours | rate', or with a leading service date 'YYYY-MM-DD | description | hours [| rate]'. Hours accept decimals (3.5), '2h 30m', '2:30', or a clock range such as '09:00-12:30' or '9am-5pm'. Blank lines and lines starting with # or // are ignored. Maximum 1000000 bytes and 500 billable rows." },
                    "rate": { "type": "number", "minimum": 0, "maximum": 1000000, "default": 100.0, "description": "Default hourly rate applied to every row that has no per-row rate, in the chosen currency. Default 100." },
                    "currency": { "type": "string", "default": "$", "description": "Currency symbol or code placed before each amount, such as $, £, € or 'USD '. Default '$'." },
                    "business": { "type": "string", "default": "", "description": "Your business block printed under 'From', one detail per line: name, address, email, tax id. Blank omits the block." },
                    "client": { "type": "string", "default": "", "description": "Client block printed under 'Bill to', one detail per line: company, contact, address. Blank omits the block." },
                    "invoice_number": { "type": "string", "default": "INV-001", "description": "Invoice reference shown in the heading, such as INV-001 or 2026-014. Default 'INV-001'." },
                    "issue_date": { "type": "string", "default": "", "description": "Invoice date in YYYY-MM-DD form, for example 2026-08-14. Blank omits the issue date and disables the automatic due date." },
                    "due_date": { "type": "string", "default": "", "description": "Explicit due date in YYYY-MM-DD form. Blank computes it as the issue date plus payment_terms days." },
                    "payment_terms": { "type": "integer", "minimum": 0, "maximum": 365, "default": 30, "description": "Net payment days used to compute the due date from the issue date (0 = no terms line). Common values are 7, 14, 30 and 60. Default 30." },
                    "tax_label": { "type": "string", "default": "Tax", "description": "Name of the tax line, such as Tax, VAT, GST or Sales tax. Default 'Tax'." },
                    "tax_rate": { "type": "number", "minimum": 0, "maximum": 100, "default": 0.0, "description": "Tax percentage applied to the subtotal after any discount (0 = no tax line). Default 0." },
                    "discount_percent": { "type": "number", "minimum": 0, "maximum": 100, "default": 0.0, "description": "Discount percentage applied to the subtotal before tax (0 = no discount line). Default 0." },
                    "round": { "type": "integer", "minimum": 0, "maximum": 60, "default": 0, "description": "Billing increment in minutes: each row's tracked time is rounded up to the next multiple before pricing (0 = bill exactly what was tracked). Common increments are 6 and 15." },
                    "group_by": { "type": "string", "enum": ["entry", "description", "date"], "default": "entry", "description": "How rows are billed: entry keeps one line per input row, description merges rows with the same description and rate, date merges rows with the same service date and rate. Default entry." },
                    "notes": { "type": "string", "default": "", "description": "Notes or payment instructions printed at the end of the invoice, such as bank details or a late-fee policy. Blank omits the section." },
                    "format": { "type": "string", "enum": ["markdown", "text", "csv", "json"], "default": "markdown", "description": "Output document: markdown (tables and bold totals), text (fixed-width plain text), csv (spreadsheet rows plus total rows), or json (full structured invoice). Default markdown." }
                },
                "required": ["entries"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
