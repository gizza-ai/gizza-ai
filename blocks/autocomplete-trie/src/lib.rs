//! gizza-ai/autocomplete-trie — build a prefix trie from a wordlist and rank autocomplete
//! suggestions for a typed prefix.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn limit_default() -> usize {
    10
}
fn rank_default() -> String {
    "weight".to_string()
}
fn output_default() -> String {
    "text".to_string()
}

#[derive(Deserialize)]
struct Args {
    wordlist: String,
    #[serde(default)]
    prefix: String,
    #[serde(default = "limit_default")]
    limit: usize,
    #[serde(default = "rank_default")]
    rank: String,
    #[serde(default)]
    max_typos: usize,
    #[serde(default)]
    case_sensitive: bool,
    #[serde(default = "output_default")]
    output: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("wordlist").required().multiline().describe("The terms to index, one per line. A line may carry an optional weight after a tab, pipe, or comma (apple,12) or before one (12|apple). Lines starting with # are ignored, and a term repeated on several lines has its weights added, so a plain list works as a frequency count."))
        .param(Param::string("prefix").default("").describe("The typed prefix to autocomplete, for example 'app'. Leave empty to return the highest-ranked terms in the whole wordlist."))
        .param(Param::integer("limit").default(10).min(1.0).max(100.0).describe("Maximum number of suggestions to return, 1-100. The total number of matching terms is reported either way. Default 10."))
        .param(Param::enumv("rank", ["weight", "alphabetical", "shortest"]).default("weight").describe("Suggestion order: weight = highest weight first (ties alphabetical), alphabetical = A to Z, shortest = fewest characters first. Default weight."))
        .param(Param::integer("max_typos").default(0).min(0.0).max(2.0).describe("Typo tolerance for the prefix, 0-2 edits. 0 (default) matches the prefix exactly; 1 also finds terms one insertion, deletion, or substitution away. Exact-prefix matches always rank above near matches."))
        .param(Param::boolean("case_sensitive").default(false).describe("Match the prefix case-sensitively and keep differently-cased spellings as separate terms. Off by default, so Apple and apple are one term and the first spelling seen is shown."))
        .param(Param::enumv("output", ["text", "json", "trie"]).default("text").describe("Output format: text = ranked suggestion list with trie statistics, json = the full structured result, trie = an ASCII drawing of the trie branch under the prefix with terminal nodes marked."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/autocomplete-trie",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Build a prefix trie from a wordlist and return ranked autocomplete suggestions for a prefix",
    skill(
        description = "Build a character prefix trie from a pasted wordlist and return ranked autocomplete suggestions for a typed prefix. Terms may carry weights (a tab, pipe, or comma separated number, before or after the term) and repeated terms sum into a frequency weight. Suggestions can be ranked by weight, alphabetically, or shortest-first, capped with a limit, matched case-sensitively, and widened with a 0-2 edit typo budget where exact-prefix hits always rank first. Reports the total match count plus trie statistics (terms, nodes, depth, and how many characters the shared prefixes save), and can output a ranked text list, JSON, or an ASCII drawing of the trie branch under the prefix.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "autocomplete-trie", |a: Args| {
            gizza_ai_autocomplete_trie_core::run(
                &a.wordlist,
                &a.prefix,
                a.limit,
                &a.rank,
                a.max_typos,
                a.case_sensitive,
                &a.output,
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
        let authored: serde_json::Value = serde_json::from_str(r#"{
            "type":"object",
            "properties":{
                "wordlist":{"type":"string","description":"The terms to index, one per line. A line may carry an optional weight after a tab, pipe, or comma (apple,12) or before one (12|apple). Lines starting with # are ignored, and a term repeated on several lines has its weights added, so a plain list works as a frequency count."},
                "prefix":{"type":"string","default":"","description":"The typed prefix to autocomplete, for example 'app'. Leave empty to return the highest-ranked terms in the whole wordlist."},
                "limit":{"type":"integer","minimum":1,"maximum":100,"default":10,"description":"Maximum number of suggestions to return, 1-100. The total number of matching terms is reported either way. Default 10."},
                "rank":{"type":"string","enum":["weight","alphabetical","shortest"],"default":"weight","description":"Suggestion order: weight = highest weight first (ties alphabetical), alphabetical = A to Z, shortest = fewest characters first. Default weight."},
                "max_typos":{"type":"integer","minimum":0,"maximum":2,"default":0,"description":"Typo tolerance for the prefix, 0-2 edits. 0 (default) matches the prefix exactly; 1 also finds terms one insertion, deletion, or substitution away. Exact-prefix matches always rank above near matches."},
                "case_sensitive":{"type":"boolean","default":false,"description":"Match the prefix case-sensitively and keep differently-cased spellings as separate terms. Off by default, so Apple and apple are one term and the first spelling seen is shown."},
                "output":{"type":"string","enum":["text","json","trie"],"default":"text","description":"Output format: text = ranked suggestion list with trie statistics, json = the full structured result, trie = an ASCII drawing of the trie branch under the prefix with terminal nodes marked."}
            },
            "required":["wordlist"],
            "additionalProperties":false
        }"#).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
