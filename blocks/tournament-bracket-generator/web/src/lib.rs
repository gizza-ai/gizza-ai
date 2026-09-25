//! Browser-facing wasm-bindgen wrapper for /tools/tournament-bracket-generator/.
use gizza_ai_tournament_bracket_generator_core::{
    generate, BracketType, Options, OutputFormat, Seeding,
};
use wasm_bindgen::prelude::*;

/// Page checkboxes arrive as "true"/"false"; be liberal about what counts as on.
fn flag(s: &str, fallback: bool) -> bool {
    match s.trim().to_ascii_lowercase().as_str() {
        "" => fallback,
        "true" | "1" | "on" | "yes" => true,
        _ => false,
    }
}

fn or_default(s: &str, fallback: &str) -> String {
    if s.trim().is_empty() {
        fallback.to_string()
    } else {
        s.trim().to_string()
    }
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    participants: &str,
    bracket_type: &str,
    seeding: &str,
    output_format: &str,
    third_place_match: &str,
    grand_final_reset: &str,
    tournament_name: &str,
    include_summary: &str,
    seed: &str,
) -> Result<String, JsValue> {
    let err = |e: String| JsValue::from_str(&e);
    let seed_text = seed.trim().replace([',', '_'], "");
    let seed = if seed_text.is_empty() {
        0u64
    } else {
        seed_text.parse::<i64>().map(i64::unsigned_abs).map_err(|_| {
            JsValue::from_str(&format!(
                "seed must be a whole number, got `{}`",
                seed.trim()
            ))
        })?
    };
    let opts = Options {
        bracket_type: BracketType::parse(&or_default(bracket_type, "single")).map_err(err)?,
        seeding: Seeding::parse(&or_default(seeding, "standard")).map_err(err)?,
        format: OutputFormat::parse(&or_default(output_format, "text")).map_err(err)?,
        third_place_match: flag(third_place_match, false),
        grand_final_reset: flag(grand_final_reset, true),
        tournament_name: tournament_name.to_string(),
        include_summary: flag(include_summary, true),
        seed,
    };
    generate(participants, &opts).map_err(|e| JsValue::from_str(&e))
}
