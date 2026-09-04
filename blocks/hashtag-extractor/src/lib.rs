//! gizza-ai/hashtag-extractor — chat skill block on the shared tool abstraction.
//!
//! Turns pasted prose into ranked, paste-ready hashtags in one pass: it scores
//! the text's keywords (and optional multi-word keyphrases) AND keeps the
//! `#tags` the author already wrote. The chat schema is single-sourced from
//! `descriptor()` (which also drives the CLI); `handle()` delegates to
//! `block_utils::run_skill`. Pure compute — no host calls.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default = "ten")]
    max_tags: i64,
    #[serde(default)]
    platform: String,
    #[serde(default)]
    style: String,
    #[serde(default = "one")]
    phrase_words: i64,
    #[serde(default = "three")]
    min_word_length: i64,
    #[serde(default = "yes")]
    include_existing: bool,
    #[serde(default)]
    separator: String,
}

fn ten() -> i64 {
    10
}
fn one() -> i64 {
    1
}
fn three() -> i64 {
    3
}
fn yes() -> bool {
    true
}

/// Single source for the chat schema (and CLI). `text` is required; every option
/// falls back to the documented default, so a bare paragraph returns the ten
/// best lowercase hashtags.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text").required().describe(
                "The text to pull hashtags from — a caption, post, article, product blurb or \
                 keyword list. Any `#tags` already written in it are kept verbatim (see \
                 include_existing); everything else is scored as keywords. Unicode-aware, so \
                 non-Latin scripts tokenize, but the built-in stop-word list is English only.",
            ),
        )
        .param(
            Param::integer("max_tags")
                .min(0.0)
                .max(100.0)
                .default(10)
                .describe(
                    "Maximum hashtags to return, highest score first. 0 = no limit of your own \
                     (the platform's recommended count still applies if you set one). When both \
                     are set the tighter number wins. Default 10.",
                ),
        )
        .param(
            Param::enumv(
                "platform",
                ["none", "instagram", "tiktok", "x", "linkedin", "facebook"],
            )
            .default("none")
            .describe(
                "Cap the output at the count that currently performs best on one network: \
                 instagram 5, tiktok 5, linkedin 5, facebook 3, x 2. none applies no platform \
                 cap. These are 2026 usage recommendations, not hard platform maxima. \
                 Default none.",
            ),
        )
        .param(
            Param::enumv("style", ["lowercase", "camel", "pascal", "preserve"])
                .default("lowercase")
                .describe(
                    "Casing for the generated hashtags: lowercase = #contentmarketing; camel = \
                     #contentMarketing; pascal = #ContentMarketing (the accessible choice for \
                     multi-word tags — screen readers announce the words separately); preserve \
                     keeps the spelling used in the text. Hashtags already written in the text \
                     are always emitted verbatim. Default lowercase.",
                ),
        )
        .param(
            Param::integer("phrase_words")
                .min(1.0)
                .max(4.0)
                .default(1)
                .describe(
                    "Maximum words joined into one hashtag. 1 = single-word tags only; 2-4 also \
                     scores consecutive keyword runs, so \"content marketing\" becomes \
                     #contentmarketing. A longer phrase suppresses a shorter tag it fully \
                     contains when both occur equally often. Default 1.",
                ),
        )
        .param(
            Param::integer("min_word_length")
                .min(1.0)
                .max(20.0)
                .default(3)
                .describe(
                    "Shortest word that may become (part of) a hashtag, in characters. Raise it \
                     to drop filler like \"new\" or \"top\"; lower it to keep short brand words. \
                     English stop words and digits-only tokens are always dropped. Default 3.",
                ),
        )
        .param(
            Param::boolean("include_existing").default(true).describe(
                "Keep the hashtags already written in the text and list them first, verbatim, \
                 before the generated ones. false generates from the keywords only. Duplicates \
                 are removed case-insensitively either way. Default true.",
            ),
        )
        .param(
            Param::enumv("separator", ["space", "comma", "newline"])
                .default("space")
                .describe(
                    "How the paste-ready line joins the hashtags: space = \"#a #b\"; comma = \
                     \"#a, #b\"; newline = one tag per line. Default space.",
                ),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

impl Args {
    fn extract(&self) -> Result<String, String> {
        gizza_ai_hashtag_extractor_core::run(
            &self.text,
            self.max_tags,
            &self.platform,
            &self.style,
            self.phrase_words,
            self.min_word_length,
            self.include_existing,
            &self.separator,
        )
    }
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/hashtag-extractor",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Turn text into ranked, paste-ready hashtags and keep the ones already written in it.",
    skill(
        description = "Turn a caption, post or article into ranked, paste-ready hashtags. Two jobs in one pass: it scores the text's keywords and formats the best ones as hashtags, and it keeps any #tags already written in the text, listed first and verbatim (include_existing, default on). Relevance = how often a word occurs, weighted by how early it appears and how many words the phrase has, so the ordering is explainable rather than alphabetical. Returns the joined hashtag line plus a summary of the tag count, character count and how many candidates were found. Options: max_tags (0-100, default 10), platform to cap the output at the count that performs best on instagram/tiktok/x/linkedin/facebook, style (lowercase/camel/pascal/preserve), phrase_words 1-4 for multi-word tags like #contentmarketing, min_word_length, and separator (space/comma/newline). English stop words and digits-only tokens are dropped; duplicates are removed case-insensitively. Runs offline with no network access.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "hashtag-extractor", |a: Args| {
            a.extract().map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match the authored
    /// schema, so the LLM sees no drift.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        // `r##` (not `r#`): a description contains the literal sequence `"#`.
        let authored: serde_json::Value = serde_json::from_str(
            r##"{
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "The text to pull hashtags from — a caption, post, article, product blurb or keyword list. Any `#tags` already written in it are kept verbatim (see include_existing); everything else is scored as keywords. Unicode-aware, so non-Latin scripts tokenize, but the built-in stop-word list is English only." },
                    "max_tags": { "type": "integer", "minimum": 0, "maximum": 100, "default": 10, "description": "Maximum hashtags to return, highest score first. 0 = no limit of your own (the platform's recommended count still applies if you set one). When both are set the tighter number wins. Default 10." },
                    "platform": { "type": "string", "enum": ["none","instagram","tiktok","x","linkedin","facebook"], "default": "none", "description": "Cap the output at the count that currently performs best on one network: instagram 5, tiktok 5, linkedin 5, facebook 3, x 2. none applies no platform cap. These are 2026 usage recommendations, not hard platform maxima. Default none." },
                    "style": { "type": "string", "enum": ["lowercase","camel","pascal","preserve"], "default": "lowercase", "description": "Casing for the generated hashtags: lowercase = #contentmarketing; camel = #contentMarketing; pascal = #ContentMarketing (the accessible choice for multi-word tags — screen readers announce the words separately); preserve keeps the spelling used in the text. Hashtags already written in the text are always emitted verbatim. Default lowercase." },
                    "phrase_words": { "type": "integer", "minimum": 1, "maximum": 4, "default": 1, "description": "Maximum words joined into one hashtag. 1 = single-word tags only; 2-4 also scores consecutive keyword runs, so \"content marketing\" becomes #contentmarketing. A longer phrase suppresses a shorter tag it fully contains when both occur equally often. Default 1." },
                    "min_word_length": { "type": "integer", "minimum": 1, "maximum": 20, "default": 3, "description": "Shortest word that may become (part of) a hashtag, in characters. Raise it to drop filler like \"new\" or \"top\"; lower it to keep short brand words. English stop words and digits-only tokens are always dropped. Default 3." },
                    "include_existing": { "type": "boolean", "default": true, "description": "Keep the hashtags already written in the text and list them first, verbatim, before the generated ones. false generates from the keywords only. Duplicates are removed case-insensitively either way. Default true." },
                    "separator": { "type": "string", "enum": ["space","comma","newline"], "default": "space", "description": "How the paste-ready line joins the hashtags: space = \"#a #b\"; comma = \"#a, #b\"; newline = one tag per line. Default space." }
                },
                "required": ["text"],
                "additionalProperties": false
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    /// The defaults serde applies to a text-only payload must equal the defaults
    /// the schema advertises.
    #[test]
    fn serde_defaults_match_the_schema_defaults() {
        let a: Args = serde_json::from_str(
            r#"{"text":"Content marketing builds brand trust. Great content marketing wins."}"#,
        )
        .unwrap();
        assert_eq!(a.max_tags, 10);
        assert_eq!(a.phrase_words, 1);
        assert_eq!(a.min_word_length, 3);
        assert!(a.include_existing);
        let out = a.extract().unwrap();
        assert_eq!(
            out,
            "#content #marketing #builds #brand #trust #great #wins\n\n7 hashtags · 54 characters"
        );
    }

    /// A bad argument surfaces as an invalid-args error, not a panic.
    #[test]
    fn unknown_platform_is_an_error() {
        let a: Args =
            serde_json::from_str(r#"{"text":"hello world","platform":"myspace"}"#).unwrap();
        let e = a.extract().unwrap_err();
        assert!(e.contains("platform must be one of"), "{e}");
    }
}
