//! gizza-ai/latex-to-text — strip LaTeX markup into readable plain text.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

fn default_math() -> String {
    "remove".to_string()
}
fn default_citations() -> String {
    "drop".to_string()
}
fn default_line_breaks() -> String {
    "paragraphs".to_string()
}
fn default_true() -> bool {
    true
}

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_math")]
    math: String,
    #[serde(default = "default_citations")]
    citations: String,
    #[serde(default)]
    drop_environments: String,
    #[serde(default)]
    keep_comments: bool,
    #[serde(default = "default_true")]
    unicode: bool,
    #[serde(default = "default_true")]
    body_only: bool,
    #[serde(default = "default_line_breaks")]
    line_breaks: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("input").required().multiline().describe("LaTeX or TeX source to convert. Paste a full .tex document or a fragment; by default the preamble is skipped and only the document body is converted."))
        .param(Param::enumv("math", ["remove", "keep", "placeholder"]).default("remove").describe("How to handle inline and display math: remove it (default), keep the original math source with delimiters, or replace each formula with [math]."))
        .param(Param::enumv("citations", ["drop", "keys"]).default("drop").describe("How to handle citation and reference commands: drop them (default) or keep their raw keys, e.g. smith2024 or fig:one."))
        .param(Param::string("drop_environments").default("").describe("Comma-separated non-math environments whose contents should be discarded. Blank uses the built-in list: array,longtable,picture,tabular,tabularx,verbatim,lstlisting,minted,tikzpicture."))
        .param(Param::boolean("keep_comments").default(false).describe("Keep percent-comment text instead of dropping comments. Escaped percent signs are always kept as literal text."))
        .param(Param::boolean("unicode").default(true).describe("Convert common LaTeX accents and symbol macros to Unicode, such as Cafe\\'e to Café and \\ldots to …. Turn off for ASCII fallbacks."))
        .param(Param::boolean("body_only").default(true).describe("When a document environment is present, convert only the text between \\begin{document} and \\end{document}. Turn off to include title/author metadata from the preamble."))
        .param(Param::enumv("line_breaks", ["paragraphs", "source"]).default("paragraphs").describe("Output line wrapping: paragraphs reflows source line breaks into readable paragraphs (default), while source preserves single newlines from the input."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/latex-to-text",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Strip LaTeX commands and environments into readable plain text.",
    skill(
        description = "Convert LaTeX or TeX source into readable plain text. The tool removes commands while keeping visible argument text, drops comments by default, strips the preamble when a document environment exists, handles common section/list/font commands, converts accents and symbol macros to Unicode, and lets you choose how math, citations, dropped environments, comments, Unicode output, body-only mode, and source line breaks are handled. It does not run a TeX engine, follow input/include files, expand user-defined macros, or resolve BibTeX entries.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "latex-to-text", |a: Args| {
            gizza_ai_latex_to_text_core::to_text(
                &a.input,
                &a.math,
                &a.citations,
                &a.drop_environments,
                a.keep_comments,
                a.unicode,
                a.body_only,
                &a.line_breaks,
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
                "input":{"type":"string","description":"LaTeX or TeX source to convert. Paste a full .tex document or a fragment; by default the preamble is skipped and only the document body is converted."},
                "math":{"type":"string","enum":["remove","keep","placeholder"],"default":"remove","description":"How to handle inline and display math: remove it (default), keep the original math source with delimiters, or replace each formula with [math]."},
                "citations":{"type":"string","enum":["drop","keys"],"default":"drop","description":"How to handle citation and reference commands: drop them (default) or keep their raw keys, e.g. smith2024 or fig:one."},
                "drop_environments":{"type":"string","default":"","description":"Comma-separated non-math environments whose contents should be discarded. Blank uses the built-in list: array,longtable,picture,tabular,tabularx,verbatim,lstlisting,minted,tikzpicture."},
                "keep_comments":{"type":"boolean","default":false,"description":"Keep percent-comment text instead of dropping comments. Escaped percent signs are always kept as literal text."},
                "unicode":{"type":"boolean","default":true,"description":"Convert common LaTeX accents and symbol macros to Unicode, such as Cafe\\'e to Café and \\ldots to …. Turn off for ASCII fallbacks."},
                "body_only":{"type":"boolean","default":true,"description":"When a document environment is present, convert only the text between \\begin{document} and \\end{document}. Turn off to include title/author metadata from the preamble."},
                "line_breaks":{"type":"string","enum":["paragraphs","source"],"default":"paragraphs","description":"Output line wrapping: paragraphs reflows source line breaks into readable paragraphs (default), while source preserves single newlines from the input."}
            },
            "required":["input"],
            "additionalProperties":false
        }"#).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "chat schema drift");
    }
}
