//! gizza-ai/regex-match-generator — generate sample strings that match a regular expression.
//! The chat schema is single-sourced from descriptor() (which also drives the CLI); handle()
//! delegates to block_utils::run_skill.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_regex_match_generator_core::{OUTPUTS, STYLES};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    pattern: String,
    #[serde(default = "default_count")]
    count: usize,
    #[serde(default = "default_style")]
    style: String,
    #[serde(default = "default_seed")]
    seed: u64,
    #[serde(default = "default_max_repeat")]
    max_repeat: u32,
    #[serde(default = "default_max_length")]
    max_length: usize,
    #[serde(default = "default_true")]
    unique: bool,
    #[serde(default = "default_output")]
    output: String,
}

fn default_count() -> usize { 5 }
fn default_style() -> String { "random".to_string() }
fn default_seed() -> u64 { 42 }
fn default_max_repeat() -> u32 { 4 }
fn default_max_length() -> usize { 200 }
fn default_true() -> bool { true }
fn default_output() -> String { "lines".to_string() }

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("pattern").required().describe(
            "Regular expression to generate matches for, written without delimiters or flags. \
             Supports literals, escapes, `.`, character classes and ranges, `\\d \\w \\s` and their \
             negations, groups, alternation and the quantifiers ? * + {n} {n,} {n,m}. Anchors are \
             ignored; lookaround, backreferences and `\\b` are rejected. Example: [A-Z]{3}-\\d{4}.",
        ))
        .param(Param::integer("count").default(5).min(1.0).max(200.0).describe(
            "How many sample strings to generate, from 1 to 200. Default 5. With unique on, fewer \
             may come back when the pattern has fewer distinct matches.",
        ))
        .param(Param::enumv("style", STYLES).default("random").describe(
            "How choices are made: random picks seeded pseudo-random branches and repeat counts; \
             sequential walks the pattern's choices in odometer order for systematic coverage; \
             shortest always takes the first branch and the fewest repeats; longest takes the last \
             branch and the most repeats allowed by max_repeat. shortest and longest produce one \
             distinct string, so pair them with count 1.",
        ))
        .param(Param::integer("seed").default(42).min(0.0).max(4294967295.0).describe(
            "Seed for the random style, from 0 to 4294967295. The same pattern, seed and settings \
             always produce the same samples, so fixtures stay stable. Default 42. Ignored by the \
             other styles.",
        ))
        .param(Param::integer("max_repeat").default(4).min(1.0).max(50.0).describe(
            "Cap on unbounded quantifiers `*`, `+` and {n,}, and on oversized {n,m} upper bounds, \
             from 1 to 50. Default 4, so `a+` yields 1 to 4 letters. Never lowers a minimum the \
             pattern itself demands.",
        ))
        .param(Param::integer("max_length").default(200).min(1.0).max(2000.0).describe(
            "Maximum characters per generated sample, from 1 to 2000. Default 200. Repeat counts \
             are reduced to fit; if even the shortest possible match is longer, the run fails and \
             says so.",
        ))
        .param(Param::boolean("unique").default(true).describe(
            "Drop duplicate samples so every returned string is distinct. Turn off to get exactly \
             count samples, duplicates included. Default on.",
        ))
        .param(Param::enumv("output", OUTPUTS).default("lines").describe(
            "Output format: lines (one sample per line, default), json (samples plus the run \
             settings) or csv (index,sample with every sample quoted).",
        ))
}
fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/regex-match-generator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate deterministic sample strings that match a regular expression.",
    skill(
        description = "Generate sample strings that match a regular expression — the reverse of a regex tester, for building fixtures, seeding fuzzers and filling validated form fields. pattern takes literals, escapes, `.`, character classes and ranges, `\\d \\w \\s` plus negations, groups, alternation and the quantifiers ? * + {n} {n,} {n,m}; anchors are ignored and lookaround, backreferences and `\\b` are rejected with a specific message. count sets how many samples, style chooses random, sequential, shortest or longest, seed makes the random style reproducible, max_repeat caps unbounded quantifiers, max_length caps each sample, unique drops duplicates, and output selects lines, JSON or CSV. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "regex-match-generator", |a: Args| {
            gizza_ai_regex_match_generator_core::run(
                &a.pattern,
                a.count,
                &a.style,
                a.seed,
                a.max_repeat,
                a.max_length,
                a.unique,
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
    fn schema_has_expected_parameters() {
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = schema["properties"].as_object().unwrap();
        for key in [
            "pattern",
            "count",
            "style",
            "seed",
            "max_repeat",
            "max_length",
            "unique",
            "output",
        ] {
            assert!(props.contains_key(key), "missing {key}");
            assert!(props[key]["description"].as_str().unwrap_or_default().len() > 20);
        }
        assert_eq!(schema["required"], serde_json::json!(["pattern"]));
        assert_eq!(props["style"]["enum"], serde_json::json!(STYLES));
        assert_eq!(props["style"]["default"], "random");
        assert_eq!(props["output"]["enum"], serde_json::json!(OUTPUTS));
        assert_eq!(props["output"]["default"], "lines");
        assert_eq!(props["count"]["default"], 5);
        assert_eq!(props["seed"]["default"], 42);
        assert_eq!(props["max_repeat"]["default"], 4);
        assert_eq!(props["max_length"]["default"], 200);
        assert_eq!(props["unique"]["default"], true);
    }

    #[test]
    fn manifest_tool_parameters_match_the_descriptor() {
        let manifest: serde_json::Value =
            serde_json::from_str(include_str!("../manifest.json")).unwrap();
        let live: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(manifest["tool"]["parameters"], live);
    }
}
