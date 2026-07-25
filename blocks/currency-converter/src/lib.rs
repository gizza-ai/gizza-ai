//! gizza-ai/currency-converter — converts an amount between currencies using a
//! rate table you supply, following multi-leg paths when no direct rate exists.
//!
//! Thin chat-skill wrapper around `gizza-ai-currency-converter-core`. The chat
//! schema is derived from `descriptor()` (single source — shared shape across
//! chat + CLI); the handler delegates to `block_utils::run_skill`. No host calls
//! — runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn default_amount() -> f64 {
    1.0
}
fn default_true() -> bool {
    true
}
fn default_precision() -> u32 {
    2
}

#[derive(Deserialize)]
struct Args {
    #[serde(default = "default_amount")]
    amount: f64,
    from: String,
    to: String,
    rates: String,
    /// Also use each rate in reverse (1/rate). Default true.
    #[serde(default = "default_true")]
    bidirectional: bool,
    /// Decimal places for the converted amount; core clamps to 0..=10.
    #[serde(default = "default_precision")]
    precision: u32,
}

/// Single-source param descriptor → chat schema (and CLI). See
/// docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::number("amount")
                .default(1.0)
                .describe("The amount to convert, in the 'from' currency. Default 1."),
        )
        .param(
            Param::string("from")
                .required()
                .describe("Source currency code, e.g. USD (case-insensitive, 2-10 letters/digits)."),
        )
        .param(
            Param::string("to")
                .required()
                .describe("Target currency code, e.g. EUR (case-insensitive, 2-10 letters/digits)."),
        )
        .param(
            Param::string("rates")
                .required()
                .describe("Exchange-rate table, one rate per line as 'FROM TO RATE' meaning 1 FROM = RATE TO (e.g. 'USD EUR 0.92'). Codes and the number may be separated by space, '/', ':', '=', or ','; blank lines and '#' comments are ignored. Rates chain across lines for multi-leg conversions."),
        )
        .param(
            Param::boolean("bidirectional")
                .default(true)
                .describe("When true (default), each rate is also usable in reverse (1/rate), so one 'USD EUR 0.92' line converts both directions. Set false to only follow rates in the direction given."),
        )
        .param(
            // Bounds reference the core clamp (MAX_PRECISION) so the LLM-facing
            // schema can't drift from what `convert` actually enforces.
            Param::integer("precision")
                .default(2)
                .min(0.0)
                .max(gizza_ai_currency_converter_core::MAX_PRECISION as f64)
                .describe("Decimal places for the converted amount, 0-10. Default 2 (typical money precision); use more for crypto or high-precision rates."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct CurrencyConverter;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/currency-converter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert amounts between currencies from a rate table you supply.",
    skill(
        description = "Convert an amount from one currency to another using an exchange-rate table you supply (no live/online rates). Paste the rates as one 'FROM TO RATE' line per rate, meaning 1 FROM = RATE TO (e.g. 'USD EUR 0.92'); codes and the number may be separated by a space, '/', ':', '=', or ','. Rates chain automatically, so USD->EUR and EUR->JPY lines let you convert USD->JPY across multiple legs (the fewest-legs path is used). By default each rate also works in reverse (1/rate); set bidirectional=false to disable that. Returns the converted amount, the effective rate, and the conversion path.",
        parameters = schema_json()
    )
)]
impl CurrencyConverter {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": … } and routes
        // errors through GuestResult::error.
        match run_skill(&body, "currency-converter", |a: Args| {
            gizza_ai_currency_converter_core::run(
                a.amount,
                &a.from,
                &a.to,
                &a.rates,
                a.bidirectional,
                a.precision,
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
    /// reviewed. Authored 2026-07-25.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "amount": { "type": "number", "default": 1.0, "description": "The amount to convert, in the 'from' currency. Default 1." },
                    "from": { "type": "string", "description": "Source currency code, e.g. USD (case-insensitive, 2-10 letters/digits)." },
                    "to": { "type": "string", "description": "Target currency code, e.g. EUR (case-insensitive, 2-10 letters/digits)." },
                    "rates": { "type": "string", "description": "Exchange-rate table, one rate per line as 'FROM TO RATE' meaning 1 FROM = RATE TO (e.g. 'USD EUR 0.92'). Codes and the number may be separated by space, '/', ':', '=', or ','; blank lines and '#' comments are ignored. Rates chain across lines for multi-leg conversions." },
                    "bidirectional": { "type": "boolean", "default": true, "description": "When true (default), each rate is also usable in reverse (1/rate), so one 'USD EUR 0.92' line converts both directions. Set false to only follow rates in the direction given." },
                    "precision": { "type": "integer", "minimum": 0, "maximum": 10, "default": 2, "description": "Decimal places for the converted amount, 0-10. Default 2 (typical money precision); use more for crypto or high-precision rates." }
                },
                "required": ["from", "to", "rates"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
