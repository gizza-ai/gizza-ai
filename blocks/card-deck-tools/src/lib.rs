//! gizza-ai/card-deck-tools — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Pure.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_card_deck_tools_core::run;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default = "default_players")]
    players: u64,
    #[serde(default = "default_cards_per_player")]
    cards_per_player: u64,
    #[serde(default = "default_count")]
    count: u64,
    #[serde(default = "default_decks")]
    decks: u64,
    #[serde(default)]
    jokers: u64,
    #[serde(default = "default_seed")]
    seed: String,
    #[serde(default = "default_notation")]
    notation: String,
    #[serde(default)]
    replacement: bool,
    #[serde(default)]
    sort_hands: bool,
    #[serde(default)]
    evaluate: bool,
}
fn default_mode() -> String {
    "deal".into()
}
fn default_players() -> u64 {
    4
}
fn default_cards_per_player() -> u64 {
    5
}
fn default_count() -> u64 {
    5
}
fn default_decks() -> u64 {
    1
}
fn default_seed() -> String {
    "42".into()
}
fn default_notation() -> String {
    "short".into()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::enumv("mode", ["shuffle", "deal", "draw"]).default("deal").describe("What to do with the shuffled deck: shuffle (list the whole deck in its new order), deal (hand out players × cards_per_player round-robin), or draw (take count cards off the top). Default deal."))
        .param(Param::integer("players").default(4).min(1.0).max(52.0).describe("deal only: how many players to deal to, 1–52. players × cards_per_player must not exceed the deck size. Default 4."))
        .param(Param::integer("cards_per_player").default(5).min(1.0).max(52.0).describe("deal only: cards dealt to each player, 1–52 (5 for poker, 2 for Texas hold'em, 13 for bridge). Default 5."))
        .param(Param::integer("count").default(5).min(1.0).max(432.0).describe("draw only: how many cards to take off the top, 1–432. Without replacement it cannot exceed the deck size. Default 5."))
        .param(Param::integer("decks").default(1).min(1.0).max(8.0).describe("How many 52-card decks to stack and shuffle together, 1–8 (a casino shoe is typically 6 or 8). Default 1."))
        .param(Param::integer("jokers").default(0).min(0.0).max(2.0).describe("Jokers added per deck, 0–2 (so 1 deck + 2 jokers = 54 cards). Jokers render as JK and are never given a poker ranking. Default 0."))
        .param(Param::string("seed").default("42").describe("Seed for the reproducible shuffle — the same seed and settings always give the same deck. A whole number is used directly; any other text (e.g. \"table-3\") is hashed. Change it to reshuffle. Default 42."))
        .param(Param::enumv("notation", ["short", "symbol", "long"]).default("short").describe("How each card is written: short (AS, TD, 7H), symbol (A♠, 10♦, 7♥), or long (Ace of Spades). Default short."))
        .param(Param::boolean("replacement").default(false).describe("draw only: put each card back before the next draw, so the same card can appear more than once and count may exceed the deck size. Default false."))
        .param(Param::boolean("sort_hands").default(false).describe("Sort each hand (or the drawn cards) high to low by rank instead of showing them in dealt order. Default false."))
        .param(Param::boolean("evaluate").default(false).describe("Append the best five-card poker ranking after each hand (e.g. \"Pair of 7s\", \"Royal flush\"). Works on hands of 5–7 cards without jokers; other hands are marked not ranked. Default false."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct CardDeckTools;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/card-deck-tools",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Shuffle, deal, and draw from a standard card deck with a seeded, reproducible shuffle",
    skill(
        description = "Shuffle a playing-card deck with a seeded, reproducible Fisher–Yates shuffle, then either list the shuffled order (`mode=shuffle`), deal `players` × `cards_per_player` hands round-robin (`mode=deal`), or take `count` cards off the top (`mode=draw`, optionally with `replacement`). The deck is `decks` stacked 52-card decks (1–8) plus `jokers` jokers each (0–2). The same `seed` always reproduces the same deck; any text works as a seed. Cards render as `short` (AS), `symbol` (A♠) or `long` (Ace of Spades); `sort_hands` orders each hand high to low and `evaluate` labels each 5–7 card hand with its best poker ranking. Not cryptographically secure — it is built for reproducibility, not secrecy.",
        parameters = schema_json()
    ),
)]
impl CardDeckTools {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "card-deck-tools", |a: Args| {
            run(
                &a.mode,
                a.players as usize,
                a.cards_per_player as usize,
                a.count as usize,
                a.decks as usize,
                a.jokers as usize,
                &a.seed,
                &a.notation,
                a.replacement,
                a.sort_hands,
                a.evaluate,
            )
            .map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "mode":             { "type": "string", "enum": ["shuffle", "deal", "draw"], "default": "deal", "description": "What to do with the shuffled deck: shuffle (list the whole deck in its new order), deal (hand out players × cards_per_player round-robin), or draw (take count cards off the top). Default deal." },
                    "players":          { "type": "integer", "default": 4, "minimum": 1, "maximum": 52, "description": "deal only: how many players to deal to, 1–52. players × cards_per_player must not exceed the deck size. Default 4." },
                    "cards_per_player": { "type": "integer", "default": 5, "minimum": 1, "maximum": 52, "description": "deal only: cards dealt to each player, 1–52 (5 for poker, 2 for Texas hold'em, 13 for bridge). Default 5." },
                    "count":            { "type": "integer", "default": 5, "minimum": 1, "maximum": 432, "description": "draw only: how many cards to take off the top, 1–432. Without replacement it cannot exceed the deck size. Default 5." },
                    "decks":            { "type": "integer", "default": 1, "minimum": 1, "maximum": 8, "description": "How many 52-card decks to stack and shuffle together, 1–8 (a casino shoe is typically 6 or 8). Default 1." },
                    "jokers":           { "type": "integer", "default": 0, "minimum": 0, "maximum": 2, "description": "Jokers added per deck, 0–2 (so 1 deck + 2 jokers = 54 cards). Jokers render as JK and are never given a poker ranking. Default 0." },
                    "seed":             { "type": "string", "default": "42", "description": "Seed for the reproducible shuffle — the same seed and settings always give the same deck. A whole number is used directly; any other text (e.g. \"table-3\") is hashed. Change it to reshuffle. Default 42." },
                    "notation":         { "type": "string", "enum": ["short", "symbol", "long"], "default": "short", "description": "How each card is written: short (AS, TD, 7H), symbol (A♠, 10♦, 7♥), or long (Ace of Spades). Default short." },
                    "replacement":      { "type": "boolean", "default": false, "description": "draw only: put each card back before the next draw, so the same card can appear more than once and count may exceed the deck size. Default false." },
                    "sort_hands":       { "type": "boolean", "default": false, "description": "Sort each hand (or the drawn cards) high to low by rank instead of showing them in dealt order. Default false." },
                    "evaluate":         { "type": "boolean", "default": false, "description": "Append the best five-card poker ranking after each hand (e.g. \"Pair of 7s\", \"Royal flush\"). Works on hands of 5–7 cards without jokers; other hands are marked not ranked. Default false." }
                },
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
