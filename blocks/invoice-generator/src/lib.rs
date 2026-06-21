//! gizza-ai/invoice-generator — turn line items into a formatted, printable PDF
//! invoice, returned as a downloadable file.
//!
//! Pure Rust (lopdf, base-14 fonts) → runs on ALL backends including the chat
//! Service Worker. Surfaces: chat + CLI. No standalone page (structured input +
//! PDF file output fits neither page shape — like merge-pdf / images-to-pdf).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use gizza_ai_block_utils::{Envelope, ForUi, Input, Param, SkillError, SkillResultExt, ToolDescriptor};
use gizza_ai_invoice_generator_core::{generate, parse_items};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize, Debug)]
struct Args {
    items: String,
    #[serde(default)]
    seller: String,
    #[serde(default)]
    client: String,
    #[serde(default)]
    invoice_number: String,
    #[serde(default)]
    date: String,
    #[serde(default)]
    tax_rate: f64,
    #[serde(default = "default_currency")]
    currency: String,
    #[serde(default)]
    notes: String,
}
fn default_currency() -> String {
    "$".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("items").required().describe(
            "Line items, one per line as 'description | quantity | unit_price' (e.g. 'Design work | 10 | 75').",
        ))
        .param(Param::string("seller").describe("Your name/company and address (the 'From' block); newlines allowed."))
        .param(Param::string("client").describe("The customer's name and address (the 'Bill To' block); newlines allowed."))
        .param(Param::string("invoice_number").describe("Invoice number/reference."))
        .param(Param::string("date").describe("Invoice date (any text, e.g. 2024-01-15)."))
        .param(Param::number("tax_rate").min(0.0).max(100.0).describe("Tax rate as a percentage applied to the subtotal (default 0)."))
        .param(Param::string("currency").default("$").describe("Currency symbol/prefix for amounts (default '$')."))
        .param(Param::string("notes").describe("Optional notes/footer text."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct InvoiceGenerator;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/invoice-generator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate a printable PDF invoice from line items",
    skill(
        description = "Turn line items into a formatted, printable PDF invoice (returned for download). items is one per line: 'description | quantity | unit_price'. Optionally set seller (From), client (Bill To), invoice_number, date, tax_rate (percent), currency, and notes. The subtotal, tax, and total are computed automatically. Returns a one-page PDF. Runs locally.",
        parameters = schema_json()
    ),
)]
impl InvoiceGenerator {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("invoice-generator")?;
    let items = parse_items(&args.items).map_err(SkillError::InvalidArgs)?;
    let pdf = generate(
        &args.seller,
        &args.client,
        &args.invoice_number,
        &args.date,
        &items,
        args.tax_rate,
        &args.currency,
        &args.notes,
    )
    .map_err(SkillError::InvalidArgs)?;

    let filename = if args.invoice_number.trim().is_empty() {
        "invoice.pdf".to_string()
    } else {
        let safe: String = args
            .invoice_number
            .chars()
            .map(|c| if c.is_ascii_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
            .collect();
        format!("invoice-{safe}.pdf")
    };
    let data_url = format!("data:application/pdf;base64,{}", B64.encode(&pdf));
    let env = Envelope {
        for_llm: format!("generated a {}-item PDF invoice ({} bytes)", items.len(), pdf.len()),
        for_ui: ForUi { data_url, mime: "application/pdf".to_string(), filename },
    };
    serde_json::to_vec(&env).map_err(|e| SkillError::Serialize(format!("serialize envelope: {e}")))
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
                    "items":          { "type": "string", "description": "Line items, one per line as 'description | quantity | unit_price' (e.g. 'Design work | 10 | 75')." },
                    "seller":         { "type": "string", "description": "Your name/company and address (the 'From' block); newlines allowed." },
                    "client":         { "type": "string", "description": "The customer's name and address (the 'Bill To' block); newlines allowed." },
                    "invoice_number": { "type": "string", "description": "Invoice number/reference." },
                    "date":           { "type": "string", "description": "Invoice date (any text, e.g. 2024-01-15)." },
                    "tax_rate":       { "type": "number", "minimum": 0, "maximum": 100, "description": "Tax rate as a percentage applied to the subtotal (default 0)." },
                    "currency":       { "type": "string", "default": "$", "description": "Currency symbol/prefix for amounts (default '$')." },
                    "notes":          { "type": "string", "description": "Optional notes/footer text." }
                },
                "required": ["items"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
