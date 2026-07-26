//! gizza-ai/ocr-text-cleaner — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. Real cleanup logic lives
//! in the `core` crate, shared with the web page.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn t() -> bool {
    true
}
fn line_breaks_default() -> String {
    "keep".to_string()
}

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default = "t")]
    fix_ligatures: bool,
    #[serde(default = "t")]
    join_hyphenated: bool,
    #[serde(default = "line_breaks_default")]
    line_breaks: String,
    #[serde(default = "t")]
    fix_confusables: bool,
    #[serde(default)]
    fix_rn: bool,
    #[serde(default = "t")]
    fix_spacing: bool,
}

/// Single source for the chat schema (and the CLI + page query-params).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("text").required().multiline().describe("OCR-extracted text to clean."))
        .param(Param::boolean("fix_ligatures").default(true).describe("Expand typographic ligatures (\u{FB01} \u{FB02} \u{FB00} \u{FB03} \u{FB04} \u{FB05} \u{FB06}) into plain letters (fi fl ff ffi ffl ft st)."))
        .param(Param::boolean("join_hyphenated").default(true).describe("Rejoin a word split by a hyphen at a line break into one word; an uppercase continuation keeps the hyphen as a real compound."))
        .param(Param::enumv("line_breaks", ["keep", "paragraphs", "all"]).default("keep").describe("Line-break handling: keep them, join soft breaks within each paragraph (reflow), or collapse every break into one line."))
        .param(Param::boolean("fix_confusables").default(true).describe("Fix letter and number confusion using word-versus-number context (HeIIo becomes Hello, l00 becomes 100, a stray | becomes I); alphanumeric codes like COVID19 are preserved."))
        .param(Param::boolean("fix_rn").default(false).describe("Aggressively replace the rn/RN to m/M OCR merge error everywhere. Off by default: with no dictionary it also rewrites real words containing rn such as modern."))
        .param(Param::boolean("fix_spacing").default(true).describe("Normalize spacing: collapse repeated spaces, remove spaces around punctuation, insert a missing space after sentence and clause punctuation, and trim line ends."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/ocr-text-cleaner",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Clean mechanical OCR errors from extracted text",
    skill(
        description = "Deterministically clean the mechanical errors OCR engines leave in extracted text: expand typographic ligatures, rejoin words hyphenated across line breaks, reflow or collapse line breaks, fix letter/number confusables by context, optionally undo the rn->m merge error, and normalize spacing. No dictionary or model — every fix is a fixed rule, so output is reproducible.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "ocr-text-cleaner", |a: Args| {
            gizza_ai_ocr_text_cleaner_core::run(
                &a.text,
                a.fix_ligatures,
                a.join_hyphenated,
                &a.line_breaks,
                a.fix_confusables,
                a.fix_rn,
                a.fix_spacing,
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
                "text":{"type":"string","description":"OCR-extracted text to clean."},
                "fix_ligatures":{"type":"boolean","default":true,"description":"Expand typographic ligatures (ﬁ ﬂ ﬀ ﬃ ﬄ ﬅ ﬆ) into plain letters (fi fl ff ffi ffl ft st)."},
                "join_hyphenated":{"type":"boolean","default":true,"description":"Rejoin a word split by a hyphen at a line break into one word; an uppercase continuation keeps the hyphen as a real compound."},
                "line_breaks":{"type":"string","enum":["keep","paragraphs","all"],"default":"keep","description":"Line-break handling: keep them, join soft breaks within each paragraph (reflow), or collapse every break into one line."},
                "fix_confusables":{"type":"boolean","default":true,"description":"Fix letter and number confusion using word-versus-number context (HeIIo becomes Hello, l00 becomes 100, a stray | becomes I); alphanumeric codes like COVID19 are preserved."},
                "fix_rn":{"type":"boolean","default":false,"description":"Aggressively replace the rn/RN to m/M OCR merge error everywhere. Off by default: with no dictionary it also rewrites real words containing rn such as modern."},
                "fix_spacing":{"type":"boolean","default":true,"description":"Normalize spacing: collapse repeated spaces, remove spaces around punctuation, insert a missing space after sentence and clause punctuation, and trim line ends."}
            },
            "required":["text"],
            "additionalProperties":false
        }"#).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
