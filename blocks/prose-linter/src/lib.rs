//! gizza-ai/prose-linter — flag clichés, weasel words, passive voice,
//! redundancies and other style problems in English prose using an embedded
//! rule set. Thin wrapper; chat schema single-sourced from descriptor();
//! handler delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_prose_linter_core::{analyze, OutputFormat};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default = "default_checks")]
    checks: String,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default)]
    ignore: String,
    #[serde(default = "default_max_issues")]
    max_issues: u32,
    #[serde(default = "default_long_sentence_words")]
    long_sentence_words: u32,
}
fn default_checks() -> String {
    "all".into()
}
fn default_output() -> String {
    "report".into()
}
fn default_max_issues() -> u32 {
    200
}
fn default_long_sentence_words() -> u32 {
    30
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The English prose to lint. Paste plain text, Markdown or a draft — e.g. 'So the cat was stolen at the end of the day.' Maximum 1000000 bytes."),
        )
        .param(
            Param::string("checks")
                .default("all")
                .describe("Which rules to run, comma-separated. 'all' (default) runs adverb, cliche, hedge, illusion, jargon, long-sentence, passive, redundancy, so-start, there-is, weasel and wordy. Prefix a name with '-' to switch it off, e.g. 'all,-passive'. Name rules directly to run only those, e.g. 'cliche,weasel'. 'eprime' (flags every form of 'to be') is opt-in and never part of 'all' — ask for it by name, e.g. 'all,eprime'. 'none' clears the list."),
        )
        .param(
            Param::enumv("output", ["report", "annotated", "json"])
                .default("report")
                .describe("Report shape: report (aligned line:col / rule / issue table plus per-rule counts), annotated (each offending line reprinted with carets under the exact match) or json (the same findings as a machine-readable object with byte-accurate line, col, rule, message, match and suggestion). Default report."),
        )
        .param(
            Param::string("ignore")
                .default("")
                .describe("Comma-separated words or phrases to never flag, for terms your style guide allows — e.g. 'very, touch base'. Matching is case-insensitive and compares the whole matched phrase. Empty by default, which flags everything."),
        )
        .param(
            Param::integer("max_issues")
                .min(0.0)
                .max(10000.0)
                .default(200)
                .describe("Maximum number of issues to list (0 = list them all). The total count is always reported even when the list is truncated. Default 200."),
        )
        .param(
            Param::integer("long_sentence_words")
                .min(0.0)
                .max(200.0)
                .default(30)
                .describe("Flag any sentence longer than this many words with the long-sentence rule (0 = never flag sentence length). Default 30."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ProseLinter;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/prose-linter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Flag clichés, weasel words, passive voice and redundancies in English prose",
    skill(
        description = "Lint English prose against an embedded style rule set and return every finding with its line, column, rule and a plain-English fix. Rules: cliche (tired stock phrases), weasel (vague quantifiers like 'very' or 'a number of'), adverb (filler adverbs such as 'basically' or 'literally'), hedge (backing away from a claim, e.g. 'I think', 'sort of'), jargon (corporate speak such as 'leverage' or 'circle back', with the plain word to use), wordy (long-winded phrases such as 'in order to' → 'to'), redundancy (saying it twice, e.g. 'free gift' → 'gift'), passive (a form of 'to be' plus a past participle, including irregular ones), there-is (sentences opening with 'There is/are'), so-start (sentences opening with 'So'), illusion (a word accidentally repeated twice in a row), long-sentence (sentences over a configurable word count), and the opt-in eprime rule (every form of 'to be'). Options: checks (comma list; 'all', '-name' to disable, 'eprime' to add), output (report, annotated or json), ignore (allow-list of phrases), max_issues and long_sentence_words. Fully local, deterministic and offline — no AI model, no spelling or grammar checking, English only.",
        parameters = schema_json()
    ),
)]
impl ProseLinter {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "prose-linter", |a: Args| {
            let fmt = OutputFormat::parse(&a.output).map_err(SkillError::InvalidArgs)?;
            analyze(
                &a.text,
                &a.checks,
                fmt,
                &a.ignore,
                a.max_issues as usize,
                a.long_sentence_words as usize,
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
                    "text": { "type": "string", "description": "The English prose to lint. Paste plain text, Markdown or a draft — e.g. 'So the cat was stolen at the end of the day.' Maximum 1000000 bytes." },
                    "checks": { "type": "string", "default": "all", "description": "Which rules to run, comma-separated. 'all' (default) runs adverb, cliche, hedge, illusion, jargon, long-sentence, passive, redundancy, so-start, there-is, weasel and wordy. Prefix a name with '-' to switch it off, e.g. 'all,-passive'. Name rules directly to run only those, e.g. 'cliche,weasel'. 'eprime' (flags every form of 'to be') is opt-in and never part of 'all' — ask for it by name, e.g. 'all,eprime'. 'none' clears the list." },
                    "output": { "type": "string", "enum": ["report", "annotated", "json"], "default": "report", "description": "Report shape: report (aligned line:col / rule / issue table plus per-rule counts), annotated (each offending line reprinted with carets under the exact match) or json (the same findings as a machine-readable object with byte-accurate line, col, rule, message, match and suggestion). Default report." },
                    "ignore": { "type": "string", "default": "", "description": "Comma-separated words or phrases to never flag, for terms your style guide allows — e.g. 'very, touch base'. Matching is case-insensitive and compares the whole matched phrase. Empty by default, which flags everything." },
                    "max_issues": { "type": "integer", "minimum": 0, "maximum": 10000, "default": 200, "description": "Maximum number of issues to list (0 = list them all). The total count is always reported even when the list is truncated. Default 200." },
                    "long_sentence_words": { "type": "integer", "minimum": 0, "maximum": 200, "default": 30, "description": "Flag any sentence longer than this many words with the long-sentence rule (0 = never flag sentence length). Default 30." }
                },
                "required": ["text"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
