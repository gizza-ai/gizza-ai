//! gizza-ai/elo-rating-calculator — chat skill block on the shared tool abstraction.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    player_a_rating: f64,
    #[serde(default = "default_player_b_rating")]
    player_b_rating: f64,
    #[serde(default = "default_score")]
    score_a: f64,
    #[serde(default = "default_k")]
    k_factor: f64,
    #[serde(default = "default_games")]
    games: u64,
    #[serde(default)]
    k_factor_b: f64,
    #[serde(default)]
    max_rating_difference: f64,
    #[serde(default)]
    decimals: u64,
    #[serde(default = "default_format")]
    output_format: String,
    #[serde(default)]
    player_a_name: String,
    #[serde(default)]
    player_b_name: String,
}

fn default_k() -> f64 {
    32.0
}
fn default_player_b_rating() -> f64 {
    1500.0
}
fn default_score() -> f64 {
    1.0
}
fn default_games() -> u64 {
    1
}
fn default_format() -> String {
    "summary".to_string()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::number("player_a_rating").required().min(0.0).max(5000.0).describe("Player A's starting Elo rating, from 0 to 5000. Example: 1600."))
        .param(Param::number("player_b_rating").default(1500.0).min(0.0).max(5000.0).describe("Player B's starting Elo rating, from 0 to 5000. Default 1500."))
        .param(Param::number("score_a").default(1.0).min(0.0).max(1000.0).describe("Player A's actual score: 1 for a win, 0.5 for a draw, 0 for a loss. For a series, enter total points across games; player B's score is games - score_a. Default 1."))
        .param(Param::number("k_factor").default(32.0).min(0.1).max(200.0).describe("K-factor for Player A. Common values: 10 master/low volatility, 20 standard/FIDE established, 32 online default, 40 new players. Default 32."))
        .param(Param::integer("games").default(1.0).min(1.0).max(1000.0).describe("Number of games represented by score_a. Use 1 for a single match; for a same-opponent series, enter the count and score_a as total points. Default 1."))
        .param(Param::number("k_factor_b").default(0.0).min(0.0).max(200.0).describe("Optional K-factor for Player B. Leave 0 to mirror k_factor; set a value when the players have different development factors."))
        .param(Param::number("max_rating_difference").default(0.0).min(0.0).max(5000.0).describe("Optional cap on the rating difference used in the expected-score formula. 0 disables the cap; 400 matches the common FIDE cap."))
        .param(Param::integer("decimals").default(0.0).min(0.0).max(6.0).describe("Decimal places for rating changes and final ratings. Default 0 for whole rating points; use 1-6 to inspect unrounded values."))
        .param(Param::enumv("output_format", ["summary", "scenarios", "json", "csv", "delta"]).default("summary").describe("Output shape. summary gives both players, expected scores, formula substitutions and scenarios; scenarios prints win/draw/loss what-ifs; json and csv are machine-readable; delta returns Player A's signed rating change only."))
        .param(Param::string("player_a_name").default("Player A").describe("Optional display name for Player A, up to 60 characters. Used in summaries, JSON and CSV."))
        .param(Param::string("player_b_name").default("Player B").describe("Optional display name for Player B, up to 60 characters. Used in summaries, JSON and CSV."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn run_args(a: Args) -> Result<String, SkillError> {
    gizza_ai_elo_rating_calculator_core::run(
        a.player_a_rating,
        a.player_b_rating,
        a.score_a,
        a.k_factor,
        a.games,
        a.k_factor_b,
        a.max_rating_difference,
        a.decimals,
        &a.output_format,
        &a.player_a_name,
        &a.player_b_name,
    )
    .map_err(SkillError::InvalidArgs)
}

#[cfg(target_arch = "wasm32")]
struct EloRatingCalculator;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/elo-rating-calculator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Compute Elo expected scores, rating deltas and new ratings for a two-player match or same-opponent series.",
    skill(
        description = "Elo rating calculator for a two-player match or a same-opponent series. Provide player_a_rating, player_b_rating, score_a (1 win, 0.5 draw, 0 loss, or total points across games), and optionally k_factor (default 32), games, player-specific k_factor_b, max_rating_difference such as 400 for FIDE-style caps, decimals and output_format. It returns both players' expected scores, signed rating changes, new ratings, formula substitution, win/draw/loss scenarios, and JSON/CSV/delta outputs. Pure arithmetic, no network.",
        parameters = schema_json()
    ),
)]
impl EloRatingCalculator {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "elo-rating-calculator", run_args) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_args_solves_the_classic_case() {
        let a: Args = serde_json::from_str(r#"{"player_a_rating":1600,"player_b_rating":1500,"score_a":1,"output_format":"delta"}"#).unwrap();
        assert_eq!(run_args(a).unwrap(), "+12");
    }

    #[test]
    fn descriptor_schema_contains_required_fields() {
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(schema["required"], serde_json::json!(["player_a_rating"]));
        assert_eq!(schema["properties"]["output_format"]["enum"][0], "summary");
    }
}
