//! gizza-ai/transcript-clean — deterministic transcript cleaner on the shared tool
//! abstraction. Chat schema is single-sourced from descriptor() (which also drives
//! the CLI); handle() delegates to block_utils::run_skill. Pure → runs on all
//! backends (chat, CLI, web page).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_transcript_clean_core::clean;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_filler_level")]
    filler_level: String,
    #[serde(default)]
    extra_fillers: String,
    #[serde(default = "default_true")]
    remove_timestamps: bool,
    #[serde(default = "default_true")]
    remove_brackets: bool,
    #[serde(default = "default_true")]
    merge_speakers: bool,
    #[serde(default = "default_true")]
    fix_capitalization: bool,
    #[serde(default = "default_true")]
    fix_punctuation: bool,
}

fn default_filler_level() -> String {
    "standard".into()
}
fn default_true() -> bool {
    true
}

/// Single source for the chat schema (and CLI). The cleaner is deterministic:
/// fixed word lists and rules, no LLM, no network.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("The raw transcript to clean — captions (SRT/VTT), a speech-to-text dump, or a pasted call/interview log. Keep one utterance or caption per line; `Name:` / `[Name]:` / `>> Name:` speaker labels are recognized and preserved."),
        )
        .param(
            Param::enumv("filler_level", ["off", "standard", "aggressive"])
                .default("standard")
                .describe("How much filler to strip. 'off' keeps all words. 'standard' removes only unambiguous non-word interjections (um, uh, erm, hmm, mm-hmm). 'aggressive' also removes discourse markers (like, basically, actually, you know, I mean, right) — deterministic, so it can over-strip genuine words."),
        )
        .param(
            Param::string("extra_fillers")
                .default("")
                .describe("Comma-separated extra words/phrases to remove at ANY level, e.g. 'you know, right, so'. Matched whole-word, case-insensitive."),
        )
        .param(
            Param::boolean("remove_timestamps")
                .default(true)
                .describe("Strip clock timestamps ([00:01:23], 0:01), SRT sequence numbers, SRT/VTT '-->' cue lines, and the WEBVTT header."),
        )
        .param(
            Param::boolean("remove_brackets")
                .default(true)
                .describe("Strip bracketed/parenthesized non-verbal cue markers like [laughter], (applause), [inaudible]. Speaker labels are extracted first, so a [Name]: label is kept."),
        )
        .param(
            Param::boolean("merge_speakers")
                .default(true)
                .describe("Merge consecutive turns from the same speaker into one line. Turn off to keep every original turn separate."),
        )
        .param(
            Param::boolean("fix_capitalization")
                .default(true)
                .describe("Capitalize sentence starts and the standalone pronoun 'i' (i'm, i'll, …)."),
        )
        .param(
            Param::boolean("fix_punctuation")
                .default(true)
                .describe("Normalize spacing around punctuation, collapse repeated commas, and ensure each line ends with a terminal mark."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/transcript-clean",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Clean transcripts: strip timestamps, fillers, and cue markers",
    skill(
        description = "Turn a raw transcript into clean, readable prose. Deterministic (no LLM, no network): it drops timestamps and caption scaffolding (SRT/VTT), strips non-verbal cue markers like [laughter], removes filler words and hyphenated stutters (I-I-I), merges consecutive same-speaker turns, collapses duplicate lines, and normalizes capitalization and punctuation. Filler removal has three levels (off/standard/aggressive) plus a custom word list; each cleanup step is an independent switch.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "transcript-clean", |a: Args| {
            clean(
                &a.input,
                &a.filler_level,
                &a.extra_fillers,
                a.remove_timestamps,
                a.remove_brackets,
                a.merge_speakers,
                a.fix_capitalization,
                a.fix_punctuation,
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
                    "input": { "type": "string", "description": "The raw transcript to clean — captions (SRT/VTT), a speech-to-text dump, or a pasted call/interview log. Keep one utterance or caption per line; `Name:` / `[Name]:` / `>> Name:` speaker labels are recognized and preserved." },
                    "filler_level": { "type": "string", "enum": ["off", "standard", "aggressive"], "default": "standard", "description": "How much filler to strip. 'off' keeps all words. 'standard' removes only unambiguous non-word interjections (um, uh, erm, hmm, mm-hmm). 'aggressive' also removes discourse markers (like, basically, actually, you know, I mean, right) — deterministic, so it can over-strip genuine words." },
                    "extra_fillers": { "type": "string", "default": "", "description": "Comma-separated extra words/phrases to remove at ANY level, e.g. 'you know, right, so'. Matched whole-word, case-insensitive." },
                    "remove_timestamps": { "type": "boolean", "default": true, "description": "Strip clock timestamps ([00:01:23], 0:01), SRT sequence numbers, SRT/VTT '-->' cue lines, and the WEBVTT header." },
                    "remove_brackets": { "type": "boolean", "default": true, "description": "Strip bracketed/parenthesized non-verbal cue markers like [laughter], (applause), [inaudible]. Speaker labels are extracted first, so a [Name]: label is kept." },
                    "merge_speakers": { "type": "boolean", "default": true, "description": "Merge consecutive turns from the same speaker into one line. Turn off to keep every original turn separate." },
                    "fix_capitalization": { "type": "boolean", "default": true, "description": "Capitalize sentence starts and the standalone pronoun 'i' (i'm, i'll, …)." },
                    "fix_punctuation": { "type": "boolean", "default": true, "description": "Normalize spacing around punctuation, collapse repeated commas, and ensure each line ends with a terminal mark." }
                },
                "required": ["input"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
