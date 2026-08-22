//! gizza-ai/repeated-word-remover — chat skill block on the shared tool abstraction.
//! Deletes accidentally doubled words (`the the`, `on on on`) while protecting the
//! English repeats that are meant to be there (`had had`, `that that`).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_repeated_word_remover_core::{
    analyze, default_keep_words, parse_keep_words, OutputFormat, Options, MAX_INPUT_BYTES,
};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default = "default_keep_words")]
    keep_words: String,
    #[serde(default)]
    case_sensitive: bool,
    #[serde(default = "default_true")]
    across_line_breaks: bool,
    #[serde(default)]
    ignore_punctuation: bool,
    #[serde(default)]
    include_numbers: bool,
    #[serde(default = "default_min_length")]
    min_length: usize,
}

fn default_output() -> String {
    "clean".into()
}
fn default_true() -> bool {
    true
}
fn default_min_length() -> usize {
    1
}

fn run_tool(a: Args) -> Result<String, String> {
    let opts = Options {
        case_sensitive: a.case_sensitive,
        across_line_breaks: a.across_line_breaks,
        ignore_punctuation: a.ignore_punctuation,
        include_numbers: a.include_numbers,
        min_length: a.min_length,
        keep_words: parse_keep_words(&a.keep_words),
        format: OutputFormat::parse(&a.output)?,
    };
    analyze(&a.input, &opts).map(|r| r.output)
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .multiline()
                .describe("Text to clean. Paste prose, notes, chat transcripts or OCR output. Only ADJACENT repeats are considered — a word that reappears later in the sentence is never touched. Max 200,000 bytes."),
        )
        .param(
            Param::enumv("output", ["clean", "marked", "report"])
                .default("clean")
                .describe("Which rendering to return. 'clean' (default) is your text with every accidental repeat deleted. 'marked' returns the ORIGINAL text with each deleted copy wrapped in markdown strikethrough (~~the~~) so you can review the change before applying it. 'report' returns an audit: how many spots were found, word counts before and after, the percent saved, and one line/column entry per doubled-word spot."),
        )
        .param(
            Param::string("keep_words")
                .default(default_keep_words())
                .describe("Words that are legitimately doubled in English and must never be collapsed, compared case-insensitively. Separate with commas, semicolons, spaces or newlines. Defaults to had, that, is, do, no, very, long, many, far, ha, blah, bye, night, so, chop, tut, yum — so 'He had had enough' and 'the fact that that happened' survive. Clear it to collapse every repeat."),
        )
        .param(
            Param::boolean("case_sensitive")
                .default(false)
                .describe("Require the two words to match exactly, including case. Default false, so 'The the cat' is caught and the first spelling ('The') is the one kept. Set true when a capitalised word starting a sentence must not merge with the lower-case word before it."),
        )
        .param(
            Param::boolean("across_line_breaks")
                .default(true)
                .describe("Treat a repeat split by a single hard line break as a repeat, catching the wrap-typo shape where a line ends with 'the' and the next line starts with 'the'. Default true — this is the commonest real doubled word in OCR and hard-wrapped text. A blank line is a paragraph break and never bridges a repeat."),
        )
        .param(
            Param::boolean("ignore_punctuation")
                .default(false)
                .describe("Let commas, semicolons, colons, brackets, quotes, slashes and pipes sit between the two words, so 'well, well now' collapses to 'well now'. Default false, which keeps punctuated repetition intact. Sentence-enders . ! ? and dashes never bridge a repeat in either setting."),
        )
        .param(
            Param::boolean("include_numbers")
                .default(false)
                .describe("Also collapse repeated tokens that contain no letters, such as '2024 2024' in a pasted table row. Default false, which protects numeric columns, IDs and version strings from being mangled."),
        )
        .param(
            Param::integer("min_length")
                .default(1)
                .min(1.0)
                .max(20.0)
                .describe("Ignore repeats of words shorter than this many characters. A floor of 3 leaves 'I I' and 'a a' alone while still fixing 'the the'. 1 (the default) applies no floor. Must be between 1 and 20."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/repeated-word-remover",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Remove accidentally doubled words from text.",
    skill(
        description = "Find and delete accidentally repeated words in text — 'the the', 'is is on on', a word doubled across a line wrap — while leaving the English repeats that are meant to be there ('had had', 'that that', 'a long long time') alone via an editable keep list. Only adjacent repeats are collapsed, and the first occurrence always wins, so capitalisation, indentation and punctuation survive. Return the cleaned text, a strikethrough-marked diff of what would be deleted, or an audit report with per-spot line/column positions and before/after word counts. Options cover case sensitivity, bridging line breaks or punctuation, repeated numbers, and a minimum word length. Runs locally in the sandbox; nothing is uploaded.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "repeated-word-remover", |a: Args| {
            run_tool(a).map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(input: &str) -> Args {
        Args {
            input: input.into(),
            output: default_output(),
            keep_words: default_keep_words(),
            case_sensitive: false,
            across_line_breaks: default_true(),
            ignore_punctuation: false,
            include_numbers: false,
            min_length: default_min_length(),
        }
    }

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "Text to clean. Paste prose, notes, chat transcripts or OCR output. Only ADJACENT repeats are considered — a word that reappears later in the sentence is never touched. Max 200,000 bytes." },
                    "output": { "type": "string", "enum": ["clean", "marked", "report"], "default": "clean", "description": "Which rendering to return. 'clean' (default) is your text with every accidental repeat deleted. 'marked' returns the ORIGINAL text with each deleted copy wrapped in markdown strikethrough (~~the~~) so you can review the change before applying it. 'report' returns an audit: how many spots were found, word counts before and after, the percent saved, and one line/column entry per doubled-word spot." },
                    "keep_words": { "type": "string", "default": "had, that, is, do, no, very, long, many, far, ha, blah, bye, night, so, chop, tut, yum", "description": "Words that are legitimately doubled in English and must never be collapsed, compared case-insensitively. Separate with commas, semicolons, spaces or newlines. Defaults to had, that, is, do, no, very, long, many, far, ha, blah, bye, night, so, chop, tut, yum — so 'He had had enough' and 'the fact that that happened' survive. Clear it to collapse every repeat." },
                    "case_sensitive": { "type": "boolean", "default": false, "description": "Require the two words to match exactly, including case. Default false, so 'The the cat' is caught and the first spelling ('The') is the one kept. Set true when a capitalised word starting a sentence must not merge with the lower-case word before it." },
                    "across_line_breaks": { "type": "boolean", "default": true, "description": "Treat a repeat split by a single hard line break as a repeat, catching the wrap-typo shape where a line ends with 'the' and the next line starts with 'the'. Default true — this is the commonest real doubled word in OCR and hard-wrapped text. A blank line is a paragraph break and never bridges a repeat." },
                    "ignore_punctuation": { "type": "boolean", "default": false, "description": "Let commas, semicolons, colons, brackets, quotes, slashes and pipes sit between the two words, so 'well, well now' collapses to 'well now'. Default false, which keeps punctuated repetition intact. Sentence-enders . ! ? and dashes never bridge a repeat in either setting." },
                    "include_numbers": { "type": "boolean", "default": false, "description": "Also collapse repeated tokens that contain no letters, such as '2024 2024' in a pasted table row. Default false, which protects numeric columns, IDs and version strings from being mangled." },
                    "min_length": { "type": "integer", "default": 1, "minimum": 1, "maximum": 20, "description": "Ignore repeats of words shorter than this many characters. A floor of 3 leaves 'I I' and 'a a' alone while still fixing 'the the'. 1 (the default) applies no floor. Must be between 1 and 20." }
                },
                "required": ["input"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored);
    }

    #[test]
    fn descriptor_describes_every_param() {
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = derived["properties"].as_object().unwrap();
        assert_eq!(props.len(), 8);
        for (name, prop) in props {
            let d = prop["description"].as_str().unwrap_or("");
            assert!(d.len() > 20, "param '{name}' needs a real .describe()");
        }
    }

    #[test]
    fn run_tool_cleans_by_default() {
        assert_eq!(
            run_tool(args("I think the the cat sat down.")).unwrap(),
            "I think the cat sat down."
        );
        // the default keep list is honoured through the string spec
        assert_eq!(
            run_tool(args("He had had enough.")).unwrap(),
            "He had had enough."
        );
    }

    #[test]
    fn run_tool_renders_the_marked_and_report_views() {
        let mut a = args("the the cat");
        a.output = "marked".into();
        assert_eq!(run_tool(a).unwrap(), "the ~~the~~ cat");

        let mut a = args("the the cat");
        a.output = "report".into();
        let out = run_tool(a).unwrap();
        assert!(out.starts_with("Found 1 doubled-word spot;"), "{out}");
        assert!(out.contains("line 1, col 1"), "{out}");
    }

    #[test]
    fn run_tool_rejects_a_bad_output_and_an_oversized_input() {
        let mut a = args("the the");
        a.output = "diff".into();
        assert!(run_tool(a).unwrap_err().contains("output must be"));

        let big = "a".repeat(MAX_INPUT_BYTES + 1);
        assert!(run_tool(args(&big)).unwrap_err().contains("200000 bytes"));
    }

    #[test]
    fn an_empty_keep_list_collapses_everything() {
        let mut a = args("He had had enough.");
        a.keep_words = String::new();
        assert_eq!(run_tool(a).unwrap(), "He had enough.");
    }
}
