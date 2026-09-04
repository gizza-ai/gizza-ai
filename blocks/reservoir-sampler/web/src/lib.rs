//! Browser-facing wasm-bindgen wrapper for /tools/reservoir-sampler/.
//! Compiled with wasm-pack for the standalone /tools/reservoir-sampler/ page.
use wasm_bindgen::prelude::*;

fn truthy(v: &str) -> bool {
    matches!(
        v.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "yes" | "on"
    )
}

/// Draw a uniform random sample of `k` lines from `data` in one pass.
///
/// The standalone tool page passes every field value as a string, so the
/// integer/boolean params arrive as strings and are parsed here:
/// - `k`: sample size (blank/unparseable → 0, which the core maps to its
///   default of 10; the core also enforces the upper bound).
/// - `seed`: PRNG seed (blank/unparseable → 42, the descriptor default).
/// - `skip_empty` / `header` / `stats`: `"true"`/`"1"`/`"on"`/`"yes"` → on,
///   anything else (including blank) → off. `skip_empty` defaults to CHECKED on
///   the page, so a blank value only reaches here via a URL deep-link.
/// - `algorithm` / `order` / `format`: validated by the core, which reports the
///   accepted values in its error message.
#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    data: &str,
    k: &str,
    algorithm: &str,
    seed: &str,
    skip_empty: &str,
    header: &str,
    order: &str,
    format: &str,
    stats: &str,
) -> Result<String, JsValue> {
    let k = k.trim().parse::<u32>().unwrap_or(0);
    let seed = seed.trim().parse::<u64>().unwrap_or(42);
    gizza_ai_reservoir_sampler_core::sample(
        data,
        k,
        algorithm,
        seed,
        truthy(skip_empty),
        truthy(header),
        order,
        format,
        truthy(stats),
    )
    .map_err(|e| JsValue::from_str(&e))
}
