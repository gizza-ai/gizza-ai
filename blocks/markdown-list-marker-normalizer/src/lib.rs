//! gizza-ai/markdown-list-marker-normalizer — make every bullet in a Markdown
//! document use one marker character and one indentation ladder.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    markdown: String,
    #[serde(default = "default_marker")]
    marker: String,
    #[serde(default = "default_indent")]
    indent: i64,
    #[serde(default = "default_true")]
    normalize_indent: bool,
    #[serde(default = "default_ordered")]
    ordered: String,
    #[serde(default = "default_marker_space")]
    marker_space: i64,
}

fn default_marker() -> String {
    "dash".to_string()
}
fn default_ordered() -> String {
    "keep".to_string()
}
fn default_indent() -> i64 {
    2
}
fn default_marker_space() -> i64 {
    1
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("markdown").required().describe(
            "The Markdown document to normalize. Only list scaffolding is rewritten: bullet \
             characters, nesting indentation, ordered numbers, and the gap after each marker. \
             Item text, headings, tables, blockquotes, thematic breaks such as --- or ***, \
             emphasis at the start of a line, and every line inside a fenced code block are \
             preserved exactly. Maximum 500000 characters.",
        ))
        .param(
            Param::enumv(
                "marker",
                ["dash", "asterisk", "plus", "consistent", "sublist"],
            )
            .default("dash")
            .describe(
                "Which character every unordered bullet becomes. dash uses -, asterisk uses *, \
                 plus uses +. consistent reuses whichever character the document used first, so \
                 an already-uniform file is left alone. sublist alternates by depth: - at the top \
                 level, * one level in, + two levels in, then repeating. Default dash, the marker \
                 Prettier and most style guides emit.",
            ),
        )
        .param(
            Param::integer("indent")
                .default(2)
                .min(1.0)
                .max(8.0)
                .describe(
                    "Spaces of indentation per nesting level, 1 to 8. Sloppy input such as 1, 3, \
                     or 5 spaces is snapped onto this exact ladder, and tabs are expanded at 4 \
                     columns then written back as spaces. Use 2 for Prettier and CommonMark \
                     defaults, 3 for Kramdown-safe ordered sublists, or 4 to match code blocks. \
                     Ignored when normalize_indent is false.",
                ),
        )
        .param(Param::boolean("normalize_indent").default(true).describe(
            "Rewrite each item's indentation onto the indent ladder. Default true. Set false to \
             change only the marker characters, numbering, and marker spacing while leaving every \
             line's original leading whitespace untouched.",
        ))
        .param(
            Param::enumv("ordered", ["keep", "ordered", "one", "zero"])
                .default("keep")
                .describe(
                    "What to do with numbered list items. keep leaves the numbers as written. \
                     ordered renumbers each list sequentially from its own first number, so \
                     3. 7. 9. becomes 3. 4. 5. one writes every item as 1. and zero writes every \
                     item as 0. — both make reordering diff-free. The . or ) delimiter is always \
                     preserved.",
                ),
        )
        .param(
            Param::integer("marker_space")
                .default(1)
                .min(1.0)
                .max(4.0)
                .describe(
                    "Spaces between a list marker and the item text, 1 to 4. 1 is the common \
                     default (- item); 2 or 3 aligns wrapped text under the first character for \
                     ordered lists. Nested items are indented by the indent setting, not by this \
                     value.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn run_args(a: Args) -> Result<String, String> {
    gizza_ai_markdown_list_marker_normalizer_core::normalize(
        &a.markdown,
        &a.marker,
        a.indent,
        a.normalize_indent,
        &a.ordered,
        a.marker_space,
    )
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/markdown-list-marker-normalizer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Normalize Markdown bullets to one marker character and one indentation ladder.",
    skill(
        description = "Normalize the unordered list markers in a Markdown document to one consistent character (-, *, or +, or whichever the file used first, or alternating per nesting depth) and snap sloppy nesting onto an exact spaces-per-level ladder, expanding tabs. Optionally renumber ordered lists sequentially or as all-1. / all-0., and set the number of spaces between each marker and its text. Item text, headings, tables, blockquotes, thematic breaks, line-start emphasis, and fenced code blocks are preserved exactly. Returns the rewritten Markdown. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "markdown-list-marker-normalizer", |a: Args| {
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
                    "markdown": {
                        "type": "string",
                        "description": "The Markdown document to normalize. Only list scaffolding is rewritten: bullet characters, nesting indentation, ordered numbers, and the gap after each marker. Item text, headings, tables, blockquotes, thematic breaks such as --- or ***, emphasis at the start of a line, and every line inside a fenced code block are preserved exactly. Maximum 500000 characters."
                    },
                    "marker": {
                        "type": "string",
                        "enum": ["dash", "asterisk", "plus", "consistent", "sublist"],
                        "default": "dash",
                        "description": "Which character every unordered bullet becomes. dash uses -, asterisk uses *, plus uses +. consistent reuses whichever character the document used first, so an already-uniform file is left alone. sublist alternates by depth: - at the top level, * one level in, + two levels in, then repeating. Default dash, the marker Prettier and most style guides emit."
                    },
                    "indent": {
                        "type": "integer",
                        "default": 2,
                        "minimum": 1,
                        "maximum": 8,
                        "description": "Spaces of indentation per nesting level, 1 to 8. Sloppy input such as 1, 3, or 5 spaces is snapped onto this exact ladder, and tabs are expanded at 4 columns then written back as spaces. Use 2 for Prettier and CommonMark defaults, 3 for Kramdown-safe ordered sublists, or 4 to match code blocks. Ignored when normalize_indent is false."
                    },
                    "normalize_indent": {
                        "type": "boolean",
                        "default": true,
                        "description": "Rewrite each item's indentation onto the indent ladder. Default true. Set false to change only the marker characters, numbering, and marker spacing while leaving every line's original leading whitespace untouched."
                    },
                    "ordered": {
                        "type": "string",
                        "enum": ["keep", "ordered", "one", "zero"],
                        "default": "keep",
                        "description": "What to do with numbered list items. keep leaves the numbers as written. ordered renumbers each list sequentially from its own first number, so 3. 7. 9. becomes 3. 4. 5. one writes every item as 1. and zero writes every item as 0. — both make reordering diff-free. The . or ) delimiter is always preserved."
                    },
                    "marker_space": {
                        "type": "integer",
                        "default": 1,
                        "minimum": 1,
                        "maximum": 4,
                        "description": "Spaces between a list marker and the item text, 1 to 4. 1 is the common default (- item); 2 or 3 aligns wrapped text under the first character for ordered lists. Nested items are indented by the indent setting, not by this value."
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

    #[test]
    fn defaults_normalize_to_dashes_and_two_space_steps() {
        let out = run_args(Args {
            markdown: "* alpha\n   + beta\n".into(),
            marker: default_marker(),
            indent: default_indent(),
            normalize_indent: true,
            ordered: default_ordered(),
            marker_space: default_marker_space(),
        })
        .unwrap();
        assert_eq!(out, "- alpha\n  - beta\n");
    }

    #[test]
    fn empty_markdown_is_rejected() {
        let err = run_args(Args {
            markdown: "  ".into(),
            marker: default_marker(),
            indent: default_indent(),
            normalize_indent: true,
            ordered: default_ordered(),
            marker_space: default_marker_space(),
        })
        .unwrap_err();
        assert!(err.contains("empty"), "{err}");
    }
}
