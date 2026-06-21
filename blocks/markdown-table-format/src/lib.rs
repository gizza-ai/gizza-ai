//! gizza-ai/markdown-table-format — chat skill block on the shared tool abstraction.
//! Aligns and pretty-prints GitHub-flavored Markdown pipe tables. The chat schema
//! is single-sourced from descriptor() (which also drives the CLI); handle()
//! delegates to block_utils::run_skill. Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_markdown_table_format_core::format_tables_styled;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    markdown: String,
    #[serde(default = "default_align")]
    align: String,
    #[serde(default = "default_style")]
    style: String,
}

fn default_align() -> String {
    "keep".into()
}

fn default_style() -> String {
    "pretty".into()
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("markdown")
                .required()
                .describe("The Markdown text containing one or more pipe tables to reformat."),
        )
        .param(
            Param::enumv("align", ["keep", "left", "center", "right"])
                .default("keep")
                .describe(
                    "Column alignment for the output: keep = preserve each column's \
                     alignment from the source delimiter row; left/center/right = force \
                     every column to that alignment.",
                ),
        )
        .param(
            Param::enumv("style", ["pretty", "compact"])
                .default("pretty")
                .describe(
                    "Output layout: pretty = pad every column to its widest cell so the \
                     grid lines up in a monospace font; compact = single-space padding (no \
                     width alignment) for the smallest, diff-friendly output.",
                ),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/markdown-table-format",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Align and pretty-print GitHub-flavored Markdown tables",
    skill(
        description = "Align and pretty-print GitHub-flavored Markdown (GFM) pipe tables. \
            Finds every table in the input (a header row, a |---|---| delimiter row, then body \
            rows), recomputes each column's width from its widest cell, and re-renders all rows \
            with evenly padded cells and a normalized delimiter row. Column alignment markers \
            (:--- left, ---: right, :---: center) are honored and re-emitted; with align='keep' \
            the source alignment is preserved, while left/center/right force every column. Ragged \
            rows are padded to the full column count, escaped \\| pipes stay inside their cell, \
            and East-Asian wide characters are width-counted so columns line up in a monospace \
            font. Tables inside fenced code blocks (``` / ~~~) are left untouched, and text \
            outside tables is passed through unchanged, so a whole document can be tidied in one \
            pass. style='pretty' produces a width-aligned grid; style='compact' produces minimal \
            single-space padding for the smallest, diff-friendly output. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "markdown-table-format", |a: Args| {
            format_tables_styled(&a.markdown, &a.align, &a.style).map_err(SkillError::InvalidArgs)
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
                    "markdown": { "type": "string", "description": "The Markdown text containing one or more pipe tables to reformat." },
                    "align": {
                        "type": "string",
                        "enum": ["keep", "left", "center", "right"],
                        "default": "keep",
                        "description": "Column alignment for the output: keep = preserve each column's alignment from the source delimiter row; left/center/right = force every column to that alignment."
                    },
                    "style": {
                        "type": "string",
                        "enum": ["pretty", "compact"],
                        "default": "pretty",
                        "description": "Output layout: pretty = pad every column to its widest cell so the grid lines up in a monospace font; compact = single-space padding (no width alignment) for the smallest, diff-friendly output."
                    }
                },
                "required": ["markdown"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
