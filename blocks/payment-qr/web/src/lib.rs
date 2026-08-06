//! Browser-facing wasm-bindgen wrapper for /tools/payment-qr/.
use gizza_ai_payment_qr_core::Options;
use wasm_bindgen::prelude::*;

fn flag(s: &str, default: bool) -> bool {
    match s.trim().to_ascii_lowercase().as_str() {
        "" => default,
        v => matches!(v, "true" | "1" | "on" | "yes"),
    }
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    address: &str,
    scheme: &str,
    amount: &str,
    label: &str,
    message: &str,
    error_correction: &str,
    size: &str,
    foreground: &str,
    background: &str,
    show_uri: &str,
) -> Result<String, JsValue> {
    let opts = Options {
        address,
        scheme: if scheme.trim().is_empty() {
            "bitcoin"
        } else {
            scheme
        },
        amount,
        label,
        message,
        error_correction: if error_correction.trim().is_empty() {
            "M"
        } else {
            error_correction
        },
        size: size.trim().parse::<u32>().unwrap_or(512),
        foreground: if foreground.trim().is_empty() {
            "#000000"
        } else {
            foreground
        },
        background: if background.trim().is_empty() {
            "#ffffff"
        } else {
            background
        },
        show_uri: flag(show_uri, true),
    };
    gizza_ai_payment_qr_core::run(&opts).map_err(|e| JsValue::from_str(&e))
}
