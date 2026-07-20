//! gizza-ai/diceware-passphrase — generate memorable diceware passphrases from
//! the EFF wordlists. Thin wrapper; chat schema single-sourced from
//! descriptor(); handler delegates to run_skill. Pure (getrandom CSPRNG) → all
//! backends incl. the chat SW.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_diceware_passphrase_core::{format_text, generate, Options};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    #[serde(default = "default_words")]
    words: u32,
    #[serde(default)]
    wordlist: String,
    #[serde(default)]
    separator: String,
    #[serde(default)]
    capitalize: bool,
    #[serde(default)]
    add_number: bool,
    #[serde(default)]
    add_symbol: bool,
    #[serde(default = "default_count")]
    count: u32,
    #[serde(default)]
    show_rolls: bool,
    #[serde(default)]
    rolls: String,
}
fn default_words() -> u32 { 6 }
fn default_count() -> u32 { 1 }

#[derive(Serialize)]
struct Resp {
    passphrases: Vec<String>,
    entropy_bits: f64,
    strength: String,
    crack_time_offline: String,
    detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    dice_rolls: Option<Vec<Vec<String>>>,
    text: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::integer("words").default(6).min(2.0).max(20.0).describe("Number of words in the passphrase (2-20, default 6 ≈ 77.5 bits with the EFF long list; use 8+ for vaults/master keys). Ignored when 'rolls' is provided."))
        .param(Param::enumv("wordlist", ["eff-long", "eff-short"]).default("eff-long").describe("Word list: 'eff-long' (EFF long list, 7,776 words, 5 dice per word, ~12.9 bits/word, default) or 'eff-short' (EFF short list, 1,296 shorter words, 4 dice per word, ~10.3 bits/word)."))
        .param(Param::enumv("separator", ["hyphen", "space", "underscore", "dot", "none", "random-symbol"]).default("hyphen").describe("How words are joined: 'hyphen' (-, default), 'space', 'underscore' (_), 'dot' (.), 'none' (joined directly), or 'random-symbol' (a random symbol between each pair of words, ~3.6 extra bits per gap)."))
        .param(Param::boolean("capitalize").default(false).describe("Capitalize the first letter of every word (e.g. Abacus-Abdomen). Readability only — adds no entropy. Default false."))
        .param(Param::boolean("add_number").default(false).describe("Append one random digit 0-9 to the passphrase (~3.3 extra bits). Default false."))
        .param(Param::boolean("add_symbol").default(false).describe("Append one random symbol from !@#$%^&*-+=? to the passphrase (~3.6 extra bits). Default false."))
        .param(Param::integer("count").default(1).min(1.0).max(20.0).describe("How many passphrases to generate (1-20, default 1; one per line). Must be 1 when 'rolls' is provided."))
        .param(Param::boolean("show_rolls").default(false).describe("Also list the dice roll for each word (e.g. '62315  tiger') so you can verify against a printed wordlist. Default false."))
        .param(Param::string("rolls").default("").describe("Optional physical dice rolls, digits 1-6 (spaces/commas between words allowed): 5 digits per word for eff-long (e.g. '62315 14534'), 4 for eff-short. When set, words are looked up deterministically instead of using the RNG."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct DicewarePassphrase;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/diceware-passphrase",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate a memorable diceware passphrase from the EFF wordlist",
    skill(
        description = "Generate a memorable multi-word diceware passphrase from the EFF wordlists with a cryptographic RNG, entirely locally. Pick the word count (default 6 ≈ 77.5 bits), the wordlist (EFF long or short), and the separator; optionally capitalize words, append a random digit/symbol, generate a batch, show the dice roll for each word, or supply your own physical dice rolls for a deterministic lookup. Returns the passphrase(s) plus entropy bits, a strength label, and an offline crack-time estimate.",
        parameters = schema_json()
    ),
)]
impl DicewarePassphrase {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "diceware-passphrase", |a: Args| {
            let opts = Options {
                words: a.words as usize,
                wordlist: a.wordlist,
                separator: a.separator,
                capitalize: a.capitalize,
                add_number: a.add_number,
                add_symbol: a.add_symbol,
                count: a.count as usize,
                show_rolls: a.show_rolls,
                rolls: a.rolls,
            };
            let out = generate(&opts).map_err(SkillError::InvalidArgs)?;
            let text = format_text(&out, opts.show_rolls);
            Ok(Resp {
                passphrases: out.passphrases.clone(),
                entropy_bits: out.bits,
                strength: out.strength.to_string(),
                crack_time_offline: format!("{} at 10 billion guesses/sec", out.crack_time),
                detail: out.detail.clone(),
                dice_rolls: if opts.show_rolls { Some(out.rolls.clone()) } else { None },
                text,
            })
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
                    "words":      { "type": "integer", "default": 6, "minimum": 2, "maximum": 20, "description": "Number of words in the passphrase (2-20, default 6 ≈ 77.5 bits with the EFF long list; use 8+ for vaults/master keys). Ignored when 'rolls' is provided." },
                    "wordlist":   { "type": "string", "enum": ["eff-long", "eff-short"], "default": "eff-long", "description": "Word list: 'eff-long' (EFF long list, 7,776 words, 5 dice per word, ~12.9 bits/word, default) or 'eff-short' (EFF short list, 1,296 shorter words, 4 dice per word, ~10.3 bits/word)." },
                    "separator":  { "type": "string", "enum": ["hyphen", "space", "underscore", "dot", "none", "random-symbol"], "default": "hyphen", "description": "How words are joined: 'hyphen' (-, default), 'space', 'underscore' (_), 'dot' (.), 'none' (joined directly), or 'random-symbol' (a random symbol between each pair of words, ~3.6 extra bits per gap)." },
                    "capitalize": { "type": "boolean", "default": false, "description": "Capitalize the first letter of every word (e.g. Abacus-Abdomen). Readability only — adds no entropy. Default false." },
                    "add_number": { "type": "boolean", "default": false, "description": "Append one random digit 0-9 to the passphrase (~3.3 extra bits). Default false." },
                    "add_symbol": { "type": "boolean", "default": false, "description": "Append one random symbol from !@#$%^&*-+=? to the passphrase (~3.6 extra bits). Default false." },
                    "count":      { "type": "integer", "default": 1, "minimum": 1, "maximum": 20, "description": "How many passphrases to generate (1-20, default 1; one per line). Must be 1 when 'rolls' is provided." },
                    "show_rolls": { "type": "boolean", "default": false, "description": "Also list the dice roll for each word (e.g. '62315  tiger') so you can verify against a printed wordlist. Default false." },
                    "rolls":      { "type": "string", "default": "", "description": "Optional physical dice rolls, digits 1-6 (spaces/commas between words allowed): 5 digits per word for eff-long (e.g. '62315 14534'), 4 for eff-short. When set, words are looked up deterministically instead of using the RNG." }
                },
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
