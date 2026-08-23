//! gizza-ai/xml-diff — structural (semantic) diff of two XML documents. Chat
//! schema single-sourced from descriptor() (which also drives the CLI); handle()
//! delegates to run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_xml_diff_core::diff_raw;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    left: String,
    right: String,
    #[serde(default = "default_strategy")]
    strategy: String,
    #[serde(default = "yes")]
    ignore_whitespace: bool,
    #[serde(default = "yes")]
    ignore_comments: bool,
    #[serde(default)]
    ignore_namespaces: bool,
    #[serde(default)]
    numeric_text: bool,
    #[serde(default = "default_format")]
    format: String,
    #[serde(default = "default_indent")]
    indent: u64,
}

fn default_strategy() -> String {
    "lcs".to_string()
}
fn default_format() -> String {
    "json".to_string()
}
fn yes() -> bool {
    true
}
fn default_indent() -> u64 {
    2
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("left")
                .required()
                .describe("The first (left/old) XML document."),
        )
        .param(
            Param::string("right")
                .required()
                .describe("The second (right/new) XML document to compare against the first."),
        )
        .param(
            Param::enumv("strategy", ["lcs", "index", "unordered"])
                .default("lcs")
                .describe(
                    "How sibling elements are paired up. 'lcs' (default) aligns identical \
                     subtrees so a pure insertion or deletion is reported as one added/removed \
                     element; 'index' compares siblings position by position; 'unordered' \
                     treats siblings as a set, so sibling order is ignored entirely.",
                ),
        )
        .param(
            Param::boolean("ignore_whitespace")
                .default(true)
                .describe(
                    "Ignore insignificant whitespace: collapse whitespace runs in text and \
                     attribute values and drop whitespace-only text nodes, so indentation and \
                     line breaks never count as differences. Default true.",
                ),
        )
        .param(
            Param::boolean("ignore_comments").default(true).describe(
                "Drop XML comments before comparing. Set false to report comment changes as \
                 comment() nodes. Default true.",
            ),
        )
        .param(
            Param::boolean("ignore_namespaces").default(false).describe(
                "Compare local names only: namespace prefixes and xmlns declarations are \
                 ignored, so ns:book and p:book match. Default false.",
            ),
        )
        .param(
            Param::boolean("numeric_text").default(false).describe(
                "Compare text and attribute values numerically when both sides parse as \
                 numbers, so 1 equals 1.0 and 2.50 equals 2.5. Default false.",
            ),
        )
        .param(
            Param::enumv("format", ["json", "text"])
                .default("json")
                .describe(
                    "Report rendering: 'json' (default) returns the machine-readable report \
                     object; 'text' returns a compact one-line-per-change summary.",
                ),
        )
        .param(
            Param::integer("indent")
                .min(0.0)
                .max(8.0)
                .default(2)
                .describe(
                    "JSON output indentation in spaces (1-8). Use 0 to minify. Ignored when \
                     format=text. Default 2.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct XmlDiff;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/xml-diff",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Structural diff of two XML documents",
    skill(
        description = "Compare two XML documents structurally (not as text lines) and report what was added, removed or changed. Both documents are parsed into element trees: attributes are compared as a sorted map so attribute order never matters, insignificant whitespace and comments are ignored by default, and CDATA is folded into element text. Every difference carries an XPath-style path such as /catalog/book[2], /catalog/book[2]/@id or /catalog/book[2]/title/text(). Returns { equal, added, removed, changed, changes:[{ path, kind: added|removed|changed, old?, new? }] } as JSON, or a compact text summary with format=text. Sibling matching is configurable (lcs | index | unordered), as are namespace-insensitive and numeric value comparison. Limits: 1 MB and 500 nesting levels per document. Runs locally.",
        parameters = schema_json()
    ),
)]
impl XmlDiff {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "xml-diff", |a: Args| {
            diff_raw(
                &a.left,
                &a.right,
                &a.strategy,
                a.ignore_whitespace,
                a.ignore_comments,
                a.ignore_namespaces,
                a.numeric_text,
                &a.format,
                a.indent as usize,
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
                    "left":  { "type": "string", "description": "The first (left/old) XML document." },
                    "right": { "type": "string", "description": "The second (right/new) XML document to compare against the first." },
                    "strategy": {
                        "type": "string",
                        "enum": ["lcs", "index", "unordered"],
                        "default": "lcs",
                        "description": "How sibling elements are paired up. 'lcs' (default) aligns identical subtrees so a pure insertion or deletion is reported as one added/removed element; 'index' compares siblings position by position; 'unordered' treats siblings as a set, so sibling order is ignored entirely."
                    },
                    "ignore_whitespace": {
                        "type": "boolean",
                        "default": true,
                        "description": "Ignore insignificant whitespace: collapse whitespace runs in text and attribute values and drop whitespace-only text nodes, so indentation and line breaks never count as differences. Default true."
                    },
                    "ignore_comments": {
                        "type": "boolean",
                        "default": true,
                        "description": "Drop XML comments before comparing. Set false to report comment changes as comment() nodes. Default true."
                    },
                    "ignore_namespaces": {
                        "type": "boolean",
                        "default": false,
                        "description": "Compare local names only: namespace prefixes and xmlns declarations are ignored, so ns:book and p:book match. Default false."
                    },
                    "numeric_text": {
                        "type": "boolean",
                        "default": false,
                        "description": "Compare text and attribute values numerically when both sides parse as numbers, so 1 equals 1.0 and 2.50 equals 2.5. Default false."
                    },
                    "format": {
                        "type": "string",
                        "enum": ["json", "text"],
                        "default": "json",
                        "description": "Report rendering: 'json' (default) returns the machine-readable report object; 'text' returns a compact one-line-per-change summary."
                    },
                    "indent": {
                        "type": "integer",
                        "minimum": 0,
                        "maximum": 8,
                        "default": 2,
                        "description": "JSON output indentation in spaces (1-8). Use 0 to minify. Ignored when format=text. Default 2."
                    }
                },
                "required": ["left", "right"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
