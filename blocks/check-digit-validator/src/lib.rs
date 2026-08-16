//! gizza-ai/check-digit-validator — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the CLI);
//! handle() delegates to block_utils::run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_check_digit_validator_core::{run, Item, Mode, Report};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    value: String,
    #[serde(default = "default_scheme")]
    scheme: String,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default)]
    show_steps: bool,
}

fn default_scheme() -> String {
    "auto".into()
}
fn default_mode() -> String {
    "validate".into()
}

#[derive(Serialize)]
struct ItemOut {
    input: String,
    normalized: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    scheme: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scheme_label: Option<String>,
    valid: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    expected_check: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    actual_check: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    full_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    also_valid_as: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    steps: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

impl From<Item> for ItemOut {
    fn from(i: Item) -> Self {
        ItemOut {
            input: i.input,
            normalized: i.normalized,
            scheme: i.scheme.map(|s| s.value().to_string()),
            scheme_label: i.scheme.map(|s| s.label().to_string()),
            valid: i.valid,
            expected_check: i.expected_check,
            actual_check: i.actual_check,
            full_code: i.full_code,
            detail: i.detail,
            also_valid_as: i
                .also_valid_as
                .into_iter()
                .map(|s| s.value().to_string())
                .collect(),
            steps: i.steps,
            error: i.error,
        }
    }
}

#[derive(Serialize)]
struct Resp {
    report: String,
    mode: String,
    total: usize,
    valid: usize,
    invalid: usize,
    errors: usize,
    items: Vec<ItemOut>,
}

impl From<Report> for Resp {
    fn from(r: Report) -> Self {
        let report = r.to_text();
        Resp {
            report,
            mode: match r.mode {
                Mode::Validate => "validate".into(),
                Mode::Compute => "compute".into(),
            },
            total: r.total,
            valid: r.valid_count,
            invalid: r.invalid_count,
            errors: r.error_count,
            items: r.items.into_iter().map(ItemOut::from).collect(),
        }
    }
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("value")
                .required()
                .describe(
                    "The code(s) to check, e.g. '4539 1488 0343 6467' or '978-0-306-40615-7'. \
                     Spaces, dashes, dots, slashes and colons are ignored. Pass several codes at \
                     once separated by newlines, commas or semicolons (max 5000 per run).",
                ),
        )
        .param(
            Param::enumv(
                "scheme",
                [
                    "auto",
                    "luhn",
                    "credit-card",
                    "imei",
                    "npi",
                    "isbn10",
                    "isbn13",
                    "issn",
                    "ean8",
                    "ean13",
                    "upc-a",
                    "gtin14",
                    "sscc",
                    "iban",
                    "routing",
                    "isin",
                    "vin",
                ],
            )
            .default("auto")
            .describe(
                "Check-digit scheme. Default 'auto' detects every scheme the code's length and \
                 character set allow and reports which ones it passes. Explicit values: luhn \
                 (generic mod-10), credit-card (Luhn + brand), imei (15 digits), npi (10 digits), \
                 isbn10 (mod-11, X allowed), isbn13, issn (mod-11, X allowed), ean8, ean13, upc-a, \
                 gtin14, sscc (18 digits), iban (mod-97-10), routing (9-digit ABA), isin, vin \
                 (17 characters, check character at position 9).",
            ),
        )
        .param(
            Param::enumv("mode", ["validate", "compute"])
                .default("validate")
                .describe(
                    "'validate' (default) checks the check digit already present in the code. \
                     'compute' takes the payload WITHOUT its check digit and returns the digit \
                     plus the completed code; it requires an explicit scheme. For iban pass the \
                     country code + account part (check digits optional); for vin pass the full \
                     17 characters — position 9 is rewritten.",
                ),
        )
        .param(
            Param::boolean("show_steps")
                .default(false)
                .describe(
                    "Set true to append the arithmetic behind each verdict (weighted sums, the \
                     modulo step and the resulting digit). Default false.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct CheckDigitValidator;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/check-digit-validator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Validate or compute check digits for Luhn, ISBN, EAN/UPC, IBAN and more",
    skill(
        description = "Validate a code's check digit — or compute a missing one — across the common schemes: Luhn (payment cards, IMEI, NPI), the GS1 mod-10 family (EAN-8/13, UPC-A, GTIN-14, SSCC-18, ISBN-13), mod-11 (ISBN-10, ISSN, VIN), ISIN, 9-digit ABA routing numbers and IBAN (mod-97-10). With scheme=auto it reports which schemes the code could be and which of them it passes. Accepts up to 5000 codes per run separated by newlines, commas or semicolons; spaces and dashes are ignored. Returns a text report plus per-code fields: matched scheme, valid flag, expected vs actual check digit, card brand or IBAN country, and an optional arithmetic breakdown.",
        parameters = schema_json()
    ),
)]
impl CheckDigitValidator {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "check-digit-validator", |a: Args| {
            let r = run(&a.value, &a.scheme, &a.mode, a.show_steps)
                .map_err(SkillError::InvalidArgs)?;
            Ok(Resp::from(r))
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
                    "value": {
                        "type": "string",
                        "description": "The code(s) to check, e.g. '4539 1488 0343 6467' or '978-0-306-40615-7'. Spaces, dashes, dots, slashes and colons are ignored. Pass several codes at once separated by newlines, commas or semicolons (max 5000 per run)."
                    },
                    "scheme": {
                        "type": "string",
                        "enum": ["auto","luhn","credit-card","imei","npi","isbn10","isbn13","issn","ean8","ean13","upc-a","gtin14","sscc","iban","routing","isin","vin"],
                        "default": "auto",
                        "description": "Check-digit scheme. Default 'auto' detects every scheme the code's length and character set allow and reports which ones it passes. Explicit values: luhn (generic mod-10), credit-card (Luhn + brand), imei (15 digits), npi (10 digits), isbn10 (mod-11, X allowed), isbn13, issn (mod-11, X allowed), ean8, ean13, upc-a, gtin14, sscc (18 digits), iban (mod-97-10), routing (9-digit ABA), isin, vin (17 characters, check character at position 9)."
                    },
                    "mode": {
                        "type": "string",
                        "enum": ["validate", "compute"],
                        "default": "validate",
                        "description": "'validate' (default) checks the check digit already present in the code. 'compute' takes the payload WITHOUT its check digit and returns the digit plus the completed code; it requires an explicit scheme. For iban pass the country code + account part (check digits optional); for vin pass the full 17 characters — position 9 is rewritten."
                    },
                    "show_steps": {
                        "type": "boolean",
                        "default": false,
                        "description": "Set true to append the arithmetic behind each verdict (weighted sums, the modulo step and the resulting digit). Default false."
                    }
                },
                "required": ["value"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
