//! Browser-facing wasm-bindgen wrapper for /tools/timesheet-to-invoice/.
//! The page passes every field as a string in meta.toml order; this parses the
//! options and delegates to the deterministic core.
use gizza_ai_timesheet_to_invoice_core::{generate, GroupBy, Options, OutputFormat};
use wasm_bindgen::prelude::*;

fn num(v: &str, fallback: f64) -> f64 {
    let t = v.trim();
    if t.is_empty() {
        fallback
    } else {
        t.replace([',', '$', '£', '€', '¥'], "")
            .trim()
            .parse::<f64>()
            .unwrap_or(fallback)
    }
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    entries: &str,
    rate: &str,
    currency: &str,
    business: &str,
    client: &str,
    invoice_number: &str,
    issue_date: &str,
    due_date: &str,
    payment_terms: &str,
    tax_label: &str,
    tax_rate: &str,
    discount_percent: &str,
    round: &str,
    group_by: &str,
    notes: &str,
    format: &str,
) -> Result<String, JsValue> {
    let opts = Options {
        rate: num(rate, 100.0),
        currency: if currency.trim().is_empty() { "$".into() } else { currency.to_string() },
        client: client.to_string(),
        business: business.to_string(),
        invoice_number: if invoice_number.trim().is_empty() { "INV-001".into() } else { invoice_number.to_string() },
        issue_date: issue_date.to_string(),
        due_date: due_date.to_string(),
        payment_terms: num(payment_terms, 30.0).round() as i64,
        tax_label: if tax_label.trim().is_empty() { "Tax".into() } else { tax_label.to_string() },
        tax_rate: num(tax_rate, 0.0),
        discount_percent: num(discount_percent, 0.0),
        round: num(round, 0.0).round() as i64,
        group_by: GroupBy::parse(if group_by.trim().is_empty() { "entry" } else { group_by }).map_err(|e| JsValue::from_str(&e))?,
        notes: notes.to_string(),
        format: OutputFormat::parse(if format.trim().is_empty() { "markdown" } else { format }).map_err(|e| JsValue::from_str(&e))?,
    };
    generate(entries, &opts).map_err(|e| JsValue::from_str(&e))
}
