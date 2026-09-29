//! Browser-facing wasm-bindgen wrapper for /tools/card-deck-tools/.
//! The generic page driver passes every pure-tool field as a string (checkboxes
//! included), so parse numbers/booleans here rather than using JS number/boolean
//! signatures.
use wasm_bindgen::prelude::*;

/// Parse an optional whole-number field, falling back to the descriptor default
/// when the box is empty.
fn num(value: &str, field: &str, fallback: usize) -> Result<usize, JsValue> {
    let v = value.trim();
    if v.is_empty() {
        return Ok(fallback);
    }
    v.parse::<usize>()
        .map_err(|_| JsValue::from_str(&format!("{field} must be a whole number — got \"{v}\"")))
}

/// Checkboxes arrive as "true"/"false"; match positively so an unset field is off.
fn flag(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "true" | "1" | "on" | "yes"
    )
}

fn or_default<'a>(value: &'a str, fallback: &'a str) -> &'a str {
    if value.trim().is_empty() {
        fallback
    } else {
        value
    }
}

#[allow(clippy::too_many_arguments)]
#[wasm_bindgen]
pub fn run(
    mode: &str,
    players: &str,
    cards_per_player: &str,
    count: &str,
    decks: &str,
    jokers: &str,
    seed: &str,
    notation: &str,
    replacement: &str,
    sort_hands: &str,
    evaluate: &str,
) -> Result<String, JsValue> {
    gizza_ai_card_deck_tools_core::run(
        or_default(mode, "deal"),
        num(players, "players", 4)?,
        num(cards_per_player, "cards_per_player", 5)?,
        num(count, "count", 5)?,
        num(decks, "decks", 1)?,
        num(jokers, "jokers", 0)?,
        or_default(seed, "42"),
        or_default(notation, "short"),
        flag(replacement),
        flag(sort_hands),
        flag(evaluate),
    )
    .map_err(|e| JsValue::from_str(&e))
}
