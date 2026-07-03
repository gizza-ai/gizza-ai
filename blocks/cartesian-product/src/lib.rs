//! gizza-ai/cartesian-product — generate every combination (tuple) across two
//! or more input lists (sizes × colors × materials). Thin wrapper; chat schema
//! single-sourced from descriptor(); handler delegates to run_skill. Pure.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_cartesian_product_core::{
    generate, parse_item_separator, parse_join_separator, parse_output_format,
};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    list1: String,
    list2: String,
    #[serde(default)]
    list3: String,
    #[serde(default)]
    list4: String,
    #[serde(default)]
    item_separator: String,
    #[serde(default)]
    dedupe: bool,
    #[serde(default)]
    output_format: String,
    #[serde(default)]
    join_separator: String,
    #[serde(default)]
    custom_join_separator: String,
    #[serde(default)]
    prefix: String,
    #[serde(default)]
    suffix: String,
    #[serde(default = "default_max")]
    max_combinations: u64,
}
fn default_max() -> u64 {
    10_000
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("list1").required().describe("First list. Items separated by newline, comma, semicolon or pipe (auto-detected unless item_separator says otherwise), e.g. 'red, blue, green'."))
        .param(Param::string("list2").required().describe("Second list, same format as list1, e.g. 'S, M, L'."))
        .param(Param::string("list3").default("").describe("Optional third list, e.g. 'cotton, linen'. Leave empty to combine only two lists."))
        .param(Param::string("list4").default("").describe("Optional fourth list. Leave empty if unused."))
        .param(Param::enumv("item_separator", ["auto", "comma", "newline", "semicolon", "pipe"]).default("auto").describe("How items inside each list are split. 'auto' (default) tries newline, then comma, semicolon, pipe. Items are trimmed and blanks dropped."))
        .param(Param::boolean("dedupe").default(false).describe("Remove duplicate items within each list before combining (keeps first occurrence)."))
        .param(Param::enumv("output_format", ["lines", "csv", "json"]).default("lines").describe("'lines' (default): one combination per line, items joined by join_separator. 'csv': one quoted CSV row per combination. 'json': JSON array of per-combination arrays."))
        .param(Param::enumv("join_separator", ["space", "none", "comma", "dash", "underscore", "pipe", "tab", "custom"]).default("space").describe("What joins the items of each combination in 'lines' output: space ' ', none '', comma ', ', dash '-', underscore '_', pipe '|', tab, or 'custom' (see custom_join_separator). Ignored for csv/json."))
        .param(Param::string("custom_join_separator").default("").describe("Join string used when join_separator is 'custom', e.g. ' :: '."))
        .param(Param::string("prefix").default("").describe("Text prepended to every combination line ('lines' output only), e.g. 'sku-'."))
        .param(Param::string("suffix").default("").describe("Text appended to every combination line ('lines' output only), e.g. '-2026'."))
        .param(Param::integer("max_combinations").default(10000).min(1.0).max(100000.0).describe("Safety cap on the number of generated combinations (list sizes multiplied). Exceeding it is an error that reports the exact count. Default 10000, hard cap 100000."))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn run_args(a: Args) -> Result<String, SkillError> {
    let item_sep = parse_item_separator(&a.item_separator).map_err(SkillError::InvalidArgs)?;
    let out_format = parse_output_format(&a.output_format).map_err(SkillError::InvalidArgs)?;
    let join_sep = parse_join_separator(&a.join_separator).map_err(SkillError::InvalidArgs)?;
    generate(
        &[&a.list1, &a.list2, &a.list3, &a.list4],
        item_sep,
        a.dedupe,
        out_format,
        join_sep,
        &a.custom_join_separator,
        &a.prefix,
        &a.suffix,
        a.max_combinations,
    )
    .map_err(SkillError::InvalidArgs)
}

#[cfg(target_arch = "wasm32")]
struct CartesianProduct;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/cartesian-product",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate every combination (tuple) across two to four input lists.",
    skill(
        description = "Generate the cartesian product of two to four lists — every combination that takes one item from each list (e.g. sizes x colors x materials). Items inside each list are split by newline/comma/semicolon/pipe (auto-detected via item_separator), trimmed, blanks dropped, optionally deduped. Output one combination per line joined by join_separator (space/none/comma/dash/underscore/pipe/tab/custom) with optional prefix/suffix, or as CSV rows or a JSON array of arrays. The rightmost list varies fastest. max_combinations (default 10000, cap 100000) guards against exploding output.",
        parameters = schema_json()
    ),
)]
impl CartesianProduct {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "cartesian-product", run_args) {
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
                    "list1":                 { "type": "string", "description": "First list. Items separated by newline, comma, semicolon or pipe (auto-detected unless item_separator says otherwise), e.g. 'red, blue, green'." },
                    "list2":                 { "type": "string", "description": "Second list, same format as list1, e.g. 'S, M, L'." },
                    "list3":                 { "type": "string", "default": "", "description": "Optional third list, e.g. 'cotton, linen'. Leave empty to combine only two lists." },
                    "list4":                 { "type": "string", "default": "", "description": "Optional fourth list. Leave empty if unused." },
                    "item_separator":        { "type": "string", "enum": ["auto", "comma", "newline", "semicolon", "pipe"], "default": "auto", "description": "How items inside each list are split. 'auto' (default) tries newline, then comma, semicolon, pipe. Items are trimmed and blanks dropped." },
                    "dedupe":                { "type": "boolean", "default": false, "description": "Remove duplicate items within each list before combining (keeps first occurrence)." },
                    "output_format":         { "type": "string", "enum": ["lines", "csv", "json"], "default": "lines", "description": "'lines' (default): one combination per line, items joined by join_separator. 'csv': one quoted CSV row per combination. 'json': JSON array of per-combination arrays." },
                    "join_separator":        { "type": "string", "enum": ["space", "none", "comma", "dash", "underscore", "pipe", "tab", "custom"], "default": "space", "description": "What joins the items of each combination in 'lines' output: space ' ', none '', comma ', ', dash '-', underscore '_', pipe '|', tab, or 'custom' (see custom_join_separator). Ignored for csv/json." },
                    "custom_join_separator": { "type": "string", "default": "", "description": "Join string used when join_separator is 'custom', e.g. ' :: '." },
                    "prefix":                { "type": "string", "default": "", "description": "Text prepended to every combination line ('lines' output only), e.g. 'sku-'." },
                    "suffix":                { "type": "string", "default": "", "description": "Text appended to every combination line ('lines' output only), e.g. '-2026'." },
                    "max_combinations":      { "type": "integer", "default": 10000, "minimum": 1, "maximum": 100000, "description": "Safety cap on the number of generated combinations (list sizes multiplied). Exceeding it is an error that reports the exact count. Default 10000, hard cap 100000." }
                },
                "required": ["list1", "list2"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn run_args_happy_path() {
        let a: Args = serde_json::from_str(
            r#"{"list1": "red, blue", "list2": "S, M"}"#,
        )
        .unwrap();
        assert_eq!(run_args(a).unwrap(), "red S\nred M\nblue S\nblue M");
    }

    #[test]
    fn run_args_rejects_bad_enum() {
        let a: Args = serde_json::from_str(
            r#"{"list1": "a", "list2": "b", "output_format": "yaml"}"#,
        )
        .unwrap();
        assert!(run_args(a).is_err());
    }
}
