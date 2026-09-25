//! Browser-facing wasm-bindgen wrapper for /tools/elo-rating-calculator/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
#[allow(clippy::too_many_arguments)]
pub fn run(
    player_a_rating: f64,
    player_b_rating: f64,
    score_a: f64,
    k_factor: f64,
    games: u64,
    k_factor_b: f64,
    max_rating_difference: f64,
    decimals: u64,
    output_format: &str,
    player_a_name: &str,
    player_b_name: &str,
) -> Result<String, JsValue> {
    gizza_ai_elo_rating_calculator_core::run(
        player_a_rating,
        player_b_rating,
        score_a,
        k_factor,
        games,
        k_factor_b,
        max_rating_difference,
        decimals,
        output_format,
        player_a_name,
        player_b_name,
    )
    .map_err(|e| JsValue::from_str(&e))
}
