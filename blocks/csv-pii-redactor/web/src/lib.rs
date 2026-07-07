//! Browser-facing wasm-bindgen wrapper for /tools/csv-pii-redactor/.
//! Field order MUST match meta.toml: data, columns, mode, header, delimiter,
//! mask_char, keep_last, salt, hash_length, label.
use gizza_ai_csv_pii_redactor_core::{redact_csv, Mode, Options};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    data: &str,
    columns: &str,
    mode: &str,
    header: &str,
    delimiter: &str,
    mask_char: &str,
    keep_last: &str,
    salt: &str,
    hash_length: &str,
    label: &str,
) -> Result<String, JsValue> {
    let hdr = !matches!(
        header.trim().to_ascii_lowercase().as_str(),
        "false" | "0" | "off" | "no"
    );
    let delim = if delimiter.is_empty() { "," } else { delimiter };
    let keep = keep_last.trim().parse::<usize>().unwrap_or(0);
    let hlen = hash_length.trim().parse::<usize>().unwrap_or(8);
    let opts = Options {
        mode: Mode::parse(mode).map_err(|e| JsValue::from_str(&e))?,
        mask_char: mask_char.chars().next().unwrap_or('*'),
        keep_last: keep,
        salt: salt.to_string(),
        hash_length: hlen,
        label: if label.is_empty() {
            "[REDACTED]".to_string()
        } else {
            label.to_string()
        },
    };
    redact_csv(data, columns, hdr, delim, &opts).map_err(|e| JsValue::from_str(&e))
}
