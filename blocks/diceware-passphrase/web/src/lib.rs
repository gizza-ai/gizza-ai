//! Browser-facing wasm-bindgen wrapper for /tools/diceware-passphrase/.
//! Field order MUST match meta.toml: words, wordlist, separator, capitalize,
//! add_number, add_symbol, count, show_rolls, rolls.
use gizza_ai_diceware_passphrase_core::{format_text, generate, Options};
use wasm_bindgen::prelude::*;

fn truthy(s: &str) -> bool {
    matches!(s.trim().to_ascii_lowercase().as_str(), "true" | "1" | "on" | "yes")
}

#[wasm_bindgen]
pub fn run(
    words: f64,
    wordlist: &str,
    separator: &str,
    capitalize: &str,
    add_number: &str,
    add_symbol: &str,
    count: f64,
    show_rolls: &str,
    rolls: &str,
) -> Result<String, JsValue> {
    let opts = Options {
        // empty field arrives as 0/NaN → descriptor default; out-of-range
        // values pass through so the core's validation error reaches the page.
        words: if words >= 1.0 { words.round() as usize } else { 6 },
        wordlist: wordlist.trim().to_string(),
        separator: separator.trim().to_string(),
        capitalize: truthy(capitalize),
        add_number: truthy(add_number),
        add_symbol: truthy(add_symbol),
        count: if count >= 1.0 { count.round() as usize } else { 1 },
        show_rolls: truthy(show_rolls),
        rolls: rolls.to_string(),
    };
    let out = generate(&opts).map_err(|e| JsValue::from_str(&e))?;
    Ok(format_text(&out, opts.show_rolls))
}
