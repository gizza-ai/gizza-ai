//! Browser-facing wasm-bindgen wrapper for /tools/iban-validator/.
//! Single field: iban. Returns a human-readable multi-line summary.
use gizza_ai_iban_validator_core::validate;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(iban: &str) -> Result<String, JsValue> {
    let r = validate(iban).map_err(|e| JsValue::from_str(&e))?;
    let mut out = if r.valid {
        format!("VALID — passes the mod-97 checksum ({} characters)", r.length)
    } else {
        "INVALID — fails the IBAN check (wrong length or bad checksum)".to_string()
    };
    out.push_str(&format!("\nFormatted: {}", r.formatted));
    match &r.country {
        Some(c) => out.push_str(&format!("\nCountry: {} ({})", c, r.country_code)),
        None => out.push_str(&format!("\nCountry code: {} (not in IBAN registry)", r.country_code)),
    }
    out.push_str(&format!("\nCheck digits: {}", r.check_digits));
    out.push_str(&format!("\nBBAN: {}", r.bban));
    if let Some(b) = &r.bank_code {
        out.push_str(&format!("\nBank code: {b}"));
    }
    if let Some(br) = &r.branch_code {
        out.push_str(&format!("\nBranch code: {br}"));
    }
    if let Some(a) = &r.account_number {
        out.push_str(&format!("\nAccount number: {a}"));
    }
    Ok(out)
}
