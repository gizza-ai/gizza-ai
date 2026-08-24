//! Browser-facing wasm-bindgen wrapper for /tools/password-vault-audit/.
//! Compiled with wasm-pack for the standalone /tools/password-vault-audit/ page.
use gizza_ai_password_vault_audit_core::{audit, Options, OutputForm, SourceFormat};
use wasm_bindgen::prelude::*;

/// Audit the vault text in `data`.
///
/// The standalone tool page passes every field value as a string, so the numeric
/// and boolean params arrive as strings and are parsed here:
/// - `format`: `"auto"`/`"list"`/`"csv"`/`"bitwarden-json"` (blank → auto).
/// - `min_length`, `min_score`, `max_age_days`: integers (blank → the descriptor
///   default 12 / 40 / 365), clamped to their documented ranges.
/// - `check_*` / `mask_passwords`: `"true"`/`"1"`/`"yes"`/`"on"` → on. The four
///   checks that default to ON also treat blank as on; `check_missing_2fa`
///   defaults to OFF, so blank is off for that one.
/// - `output`: `"report"`/`"json"`/`"csv"` (blank → report).
///
/// Throws a JS error string on an unknown `format` or `output`, on empty input,
/// on a CSV with no password column, on unparseable JSON, and when the vault is
/// over the 5000-entry cap.
#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    data: &str,
    format: &str,
    min_length: &str,
    min_score: &str,
    max_age_days: &str,
    check_common: &str,
    check_reuse: &str,
    check_similar: &str,
    check_insecure_urls: &str,
    check_missing_2fa: &str,
    mask_passwords: &str,
    output: &str,
) -> Result<String, JsValue> {
    let opts = Options {
        format: SourceFormat::parse(format).map_err(|e| JsValue::from_str(&e))?,
        min_length: num(min_length, 12, 1, 256) as usize,
        min_score: num(min_score, 40, 0, 100),
        max_age_days: num(max_age_days, 365, 0, 3650),
        check_common: on_by_default(check_common),
        check_reuse: on_by_default(check_reuse),
        check_similar: on_by_default(check_similar),
        check_insecure_urls: on_by_default(check_insecure_urls),
        check_missing_2fa: off_by_default(check_missing_2fa),
        mask_passwords: on_by_default(mask_passwords),
        output: OutputForm::parse(output).map_err(|e| JsValue::from_str(&e))?,
    };
    audit(data, &opts, now_unix()).map_err(|e| JsValue::from_str(&e))
}

/// The browser target has no std clock, so "now" comes from JS.
fn now_unix() -> f64 {
    js_sys::Date::now() / 1000.0
}

fn num(v: &str, default: u32, lo: u32, hi: u32) -> u32 {
    match v.trim() {
        "" => default,
        s => s.parse::<f64>().map(|n| n.round()).map_or(default, |n| {
            if n < lo as f64 {
                lo
            } else if n > hi as f64 {
                hi
            } else {
                n as u32
            }
        }),
    }
}

/// Positive-truthy parse for a checkbox whose descriptor default is `true`: a checked box sends
/// `"true"`, an unchecked one `"false"`, and a URL prefill that omits the field sends blank.
fn on_by_default(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "" | "true" | "1" | "yes" | "on"
    )
}

/// Same, for a checkbox whose descriptor default is `false` — blank means off.
fn off_by_default(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    )
}
