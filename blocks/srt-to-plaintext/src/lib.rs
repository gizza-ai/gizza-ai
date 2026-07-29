//! gizza-ai/srt-to-plaintext — chat skill block on the shared tool abstraction.
//! Converts SubRip/WebVTT subtitle cues into plain transcript text.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default)]
    layout: String,
    #[serde(default = "default_true")]
    strip_tags: bool,
    #[serde(default)]
    remove_sound_effects: bool,
    #[serde(default)]
    remove_speaker_labels: bool,
    #[serde(default)]
    dedupe: bool,
}

fn default_true() -> bool {
    true
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("input").required().describe("SubRip (.srt) or WebVTT subtitle text. Cue numbers, timing lines like 00:00:01,000 --> 00:00:04,000, and blank separators are removed; only caption text is returned."))
        .param(Param::enumv("layout", ["lines", "blocks", "paragraph"]).default("lines").describe("Output layout: lines = one cleaned caption per line (default), blocks = preserve cue line breaks with blank lines between cues, paragraph = one flowing paragraph."))
        .param(Param::boolean("strip_tags").default(true).describe("Remove inline formatting tags such as <i>, <b>, <font ...>, and ASS/SSA override blocks like {\\an8}. Default true."))
        .param(Param::boolean("remove_sound_effects").default(false).describe("Remove bracketed non-speech cues such as [applause] or (door slams), plus musical-note markers. Default false."))
        .param(Param::boolean("remove_speaker_labels").default(false).describe("Remove leading speaker labels such as NARRATOR: or - JOHN:. Heuristic and off by default to avoid clipping genuine dialogue."))
        .param(Param::boolean("dedupe").default(false).describe("Collapse consecutive duplicate captions, useful for rolling auto-caption exports. Comparison is case-insensitive. Default false."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/srt-to-plaintext",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Strip SRT/WebVTT cue numbers and timestamps into plain transcript text.",
    skill(
        description = "Convert SubRip (.srt) or WebVTT subtitle text into a plain transcript. It removes cue numbers, timestamp ranges and blank separators, then returns only caption text. Options choose one line per cue, preserved cue blocks or one paragraph; strip formatting tags; remove bracketed sound-effect/music cues; remove leading speaker labels; and collapse consecutive duplicate captions. The parser is deterministic and local: it never executes anything and accepts common SRT/WebVTT timing lines with comma or dot milliseconds.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "srt-to-plaintext", |a: Args| {
            gizza_ai_srt_to_plaintext_core::convert(
                &a.input,
                &a.layout,
                a.strip_tags,
                a.remove_sound_effects,
                a.remove_speaker_labels,
                a.dedupe,
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

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type":"object",
                "properties":{
                    "input":{"type":"string","description":"SubRip (.srt) or WebVTT subtitle text. Cue numbers, timing lines like 00:00:01,000 --> 00:00:04,000, and blank separators are removed; only caption text is returned."},
                    "layout":{"type":"string","enum":["lines","blocks","paragraph"],"default":"lines","description":"Output layout: lines = one cleaned caption per line (default), blocks = preserve cue line breaks with blank lines between cues, paragraph = one flowing paragraph."},
                    "strip_tags":{"type":"boolean","default":true,"description":"Remove inline formatting tags such as <i>, <b>, <font ...>, and ASS/SSA override blocks like {\\an8}. Default true."},
                    "remove_sound_effects":{"type":"boolean","default":false,"description":"Remove bracketed non-speech cues such as [applause] or (door slams), plus musical-note markers. Default false."},
                    "remove_speaker_labels":{"type":"boolean","default":false,"description":"Remove leading speaker labels such as NARRATOR: or - JOHN:. Heuristic and off by default to avoid clipping genuine dialogue."},
                    "dedupe":{"type":"boolean","default":false,"description":"Collapse consecutive duplicate captions, useful for rolling auto-caption exports. Comparison is case-insensitive. Default false."}
                },
                "required":["input"],
                "additionalProperties":false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
