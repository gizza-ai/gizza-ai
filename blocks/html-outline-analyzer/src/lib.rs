//! gizza-ai/html-outline-analyzer — chat skill block on the shared tool abstraction.
//!
//! Parses pasted HTML into a heading outline plus literal element/tag counts.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    html: String,
    #[serde(default)]
    format: String,
    #[serde(default = "default_min_level")]
    min_level: u8,
    #[serde(default = "default_max_level")]
    max_level: u8,
    #[serde(default = "default_true")]
    include_issues: bool,
    #[serde(default = "default_true")]
    include_counts: bool,
    #[serde(default = "default_top_tags")]
    top_tags: u32,
}

fn default_min_level() -> u8 {
    1
}
fn default_max_level() -> u8 {
    6
}
fn default_top_tags() -> u32 {
    20
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("html")
                .required()
                .describe("The HTML document or fragment to analyze. Paste the literal markup; the scanner reports headings from h1 through h6 plus counts of the element tags written in the source."),
        )
        .param(
            Param::enumv("format", ["tree", "markdown", "json", "csv"])
                .default("tree")
                .describe("Output format. 'tree' is a readable outline with issues and tag counts; 'markdown' is a Markdown report; 'json' returns structured headings/issues/counts; 'csv' returns one row per heading."),
        )
        .param(
            Param::integer("min_level")
                .default(1)
                .min(1.0)
                .max(6.0)
                .describe("Lowest heading level to include in the rendered outline, from 1 to 6. Use with max_level to focus on part of a large document."),
        )
        .param(
            Param::integer("max_level")
                .default(6)
                .min(1.0)
                .max(6.0)
                .describe("Highest heading level to include in the rendered outline, from 1 to 6. Must be greater than or equal to min_level."),
        )
        .param(
            Param::boolean("include_issues")
                .default(true)
                .describe("When true, include outline-quality issues such as no h1, multiple h1s, skipped heading levels, empty headings, duplicate heading text, and hidden headings."),
        )
        .param(
            Param::boolean("include_counts")
                .default(true)
                .describe("When true, include element/tag counts: total element tags, distinct tags, and the most common tag names."),
        )
        .param(
            Param::integer("top_tags")
                .default(20)
                .min(1.0)
                .max(500.0)
                .describe("How many distinct tag names to list in the tag-count section, from 1 to 500. Counts are sorted by frequency, then tag name."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn run_args(a: Args) -> Result<String, String> {
    let format = gizza_ai_html_outline_analyzer_core::parse_format(&a.format)?;
    let opts = gizza_ai_html_outline_analyzer_core::Options {
        min_level: a.min_level,
        max_level: a.max_level,
        include_issues: a.include_issues,
        include_counts: a.include_counts,
        top_tags: a.top_tags as usize,
    };
    gizza_ai_html_outline_analyzer_core::analyze_to_string(&a.html, format, &opts)
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/html-outline-analyzer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract an HTML heading outline and element tag counts from pasted markup.",
    skill(
        description = "Analyze pasted HTML markup. Extracts an h1-h6 heading outline with text, id, line/column and hidden-state metadata; flags structure issues such as missing h1, multiple h1s, skipped levels, empty or duplicate headings, and hidden headings; and reports literal element/tag counts from the source. format='tree' (default) is a readable report; 'markdown', 'json', and 'csv' are available for docs and automation. min_level/max_level filter the rendered outline; include_issues/include_counts toggle report sections; top_tags caps the tag-count list.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "html-outline-analyzer", |a: Args| {
            run_args(a).map_err(SkillError::InvalidArgs)
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
                    "html": { "type": "string", "description": "The HTML document or fragment to analyze. Paste the literal markup; the scanner reports headings from h1 through h6 plus counts of the element tags written in the source." },
                    "format": { "type": "string", "enum": ["tree", "markdown", "json", "csv"], "default": "tree", "description": "Output format. 'tree' is a readable outline with issues and tag counts; 'markdown' is a Markdown report; 'json' returns structured headings/issues/counts; 'csv' returns one row per heading." },
                    "min_level": { "type": "integer", "minimum": 1, "maximum": 6, "default": 1, "description": "Lowest heading level to include in the rendered outline, from 1 to 6. Use with max_level to focus on part of a large document." },
                    "max_level": { "type": "integer", "minimum": 1, "maximum": 6, "default": 6, "description": "Highest heading level to include in the rendered outline, from 1 to 6. Must be greater than or equal to min_level." },
                    "include_issues": { "type": "boolean", "default": true, "description": "When true, include outline-quality issues such as no h1, multiple h1s, skipped heading levels, empty headings, duplicate heading text, and hidden headings." },
                    "include_counts": { "type": "boolean", "default": true, "description": "When true, include element/tag counts: total element tags, distinct tags, and the most common tag names." },
                    "top_tags": { "type": "integer", "minimum": 1, "maximum": 500, "default": 20, "description": "How many distinct tag names to list in the tag-count section, from 1 to 500. Counts are sorted by frequency, then tag name." }
                },
                "required": ["html"],
                "additionalProperties": false
            }"#,
        ).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
