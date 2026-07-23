//! gizza-ai/csv-anti-join — return the rows of CSV A whose key has no match in
//! CSV B (the SQL LEFT ANTI JOIN), and optionally the reverse. Thin wrapper; chat
//! schema single-sourced from descriptor(); handler delegates to run_skill.
//! Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_csv_anti_join_core::anti_join;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    a: String,
    b: String,
    key: String,
    #[serde(default)]
    key_b: String,
    #[serde(default = "default_a_only")]
    direction: String,
    #[serde(default)]
    delimiter: String,
    #[serde(default = "default_true")]
    case_sensitive: bool,
    #[serde(default)]
    trim_keys: bool,
}
fn default_true() -> bool {
    true
}
fn default_a_only() -> String {
    "a-only".into()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("a").required().describe("CSV A (the first/left table) text; the first row is its header."))
        .param(Param::string("b").required().describe("CSV B (the second/right table) text; the first row is its header."))
        .param(Param::string("key").required().describe(
            "Key column(s) in A — a header name or 1-based column index. Use a comma-separated list for a composite key (e.g. 'first,last').",
        ))
        .param(Param::string("key_b").default("").describe(
            "Key column(s) in B — header names or 1-based indices. Matched on VALUES, so they may be named differently than in A. Blank reuses `key`. Must have the same number of columns as `key`.",
        ))
        .param(
            Param::enumv("direction", ["a-only", "b-only", "both"])
                .default("a-only")
                .describe("Which unmatched rows to return: 'a-only' rows in A not in B; 'b-only' the reverse (rows in B not in A); 'both' the symmetric difference with a leading '_source' A/B column. Default 'a-only'."),
        )
        .param(
            Param::string("delimiter")
                .default(",")
                .describe("Field separator: a single char or 'comma'/'tab'/'semicolon'/'pipe'. Default ','."),
        )
        .param(
            Param::boolean("case_sensitive")
                .default(true)
                .describe("Match key VALUES case-sensitively. false makes 'A1' and 'a1' match. Default true."),
        )
        .param(
            Param::boolean("trim_keys")
                .default(false)
                .describe("Trim surrounding whitespace on key values before matching, so ' 1 ' matches '1'. Default false."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct CsvAntiJoin;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/csv-anti-join",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Return rows in CSV A with no key match in CSV B (anti-join)",
    skill(
        description = "Return the rows of CSV A whose key has no match in CSV B — the SQL LEFT ANTI JOIN / NOT IN primitive — and optionally the reverse. `key`/`key_b` are header names or 1-based indices (comma-separated for a composite key) matched on VALUES, so the key columns may be named differently in A and B (blank key_b reuses key). `direction` is a-only (rows in A not in B), b-only (the reverse), or both (symmetric difference with a leading '_source' A/B column). B is treated as a key SET, so duplicate keys never multiply output rows and every unmatched A row is kept. delimiter is a single char or comma/tab/semicolon/pipe; case_sensitive and trim_keys control key matching. Runs locally.",
        parameters = schema_json()
    ),
)]
impl CsvAntiJoin {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "csv-anti-join", |a: Args| {
            let delim = if a.delimiter.is_empty() { ",".to_string() } else { a.delimiter };
            anti_join(
                &a.a,
                &a.b,
                &a.key,
                &a.key_b,
                &a.direction,
                &delim,
                a.case_sensitive,
                a.trim_keys,
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
                    "a": { "type": "string", "description": "CSV A (the first/left table) text; the first row is its header." },
                    "b": { "type": "string", "description": "CSV B (the second/right table) text; the first row is its header." },
                    "key": { "type": "string", "description": "Key column(s) in A — a header name or 1-based column index. Use a comma-separated list for a composite key (e.g. 'first,last')." },
                    "key_b": { "type": "string", "default": "", "description": "Key column(s) in B — header names or 1-based indices. Matched on VALUES, so they may be named differently than in A. Blank reuses `key`. Must have the same number of columns as `key`." },
                    "direction": { "type": "string", "enum": ["a-only", "b-only", "both"], "default": "a-only", "description": "Which unmatched rows to return: 'a-only' rows in A not in B; 'b-only' the reverse (rows in B not in A); 'both' the symmetric difference with a leading '_source' A/B column. Default 'a-only'." },
                    "delimiter": { "type": "string", "default": ",", "description": "Field separator: a single char or 'comma'/'tab'/'semicolon'/'pipe'. Default ','." },
                    "case_sensitive": { "type": "boolean", "default": true, "description": "Match key VALUES case-sensitively. false makes 'A1' and 'a1' match. Default true." },
                    "trim_keys": { "type": "boolean", "default": false, "description": "Trim surrounding whitespace on key values before matching, so ' 1 ' matches '1'. Default false." }
                },
                "required": ["a", "b", "key"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
