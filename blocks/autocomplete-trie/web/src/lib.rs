//! Browser-facing wasm-bindgen wrapper for /tools/autocomplete-trie/.
//! Field order MUST match page/meta.toml. Fields arrive as strings from the
//! generic form runtime.
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

fn parse_usize(v: &str, fallback: usize) -> usize {
    v.trim().parse::<usize>().unwrap_or(fallback)
}

#[wasm_bindgen]
pub fn run(
    wordlist: &str,
    prefix: &str,
    limit: &str,
    rank: &str,
    max_typos: &str,
    output: &str,
    case_sensitive: &str,
) -> Result<String, JsValue> {
    gizza_ai_autocomplete_trie_core::run(
        wordlist,
        prefix,
        parse_usize(limit, 10),
        rank,
        parse_usize(max_typos, 0),
        truthy(case_sensitive),
        output,
    )
    .map_err(|e| JsValue::from_str(&e))
}
