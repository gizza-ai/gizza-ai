//! gizza-ai/html-to-jsx — convert raw HTML into React JSX.
//!
//! Thin chat-skill wrapper around `gizza-ai-html-to-jsx-core`. The chat schema
//! is derived from `descriptor()` (single source — shared shape across chat +
//! CLI); the handler delegates to `block_utils::run_skill`.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    html: String,
    #[serde(default)]
    indent: String,
    #[serde(default)]
    component: String,
    #[serde(default)]
    comments: String,
    #[serde(default)]
    boolean_attrs: String,
    #[serde(default)]
    value_attrs: String,
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("html")
                .required()
                .describe("The HTML markup to convert to JSX. Paste a snippet such as <div class=\"card\"><img src=\"a.png\"></div>; a full page works too (the doctype is dropped)."),
        )
        .param(
            Param::enumv("indent", ["2", "4", "tab"])
                .default("2")
                .describe("Indentation of the generated JSX: '2' spaces (default), '4' spaces, or 'tab'."),
        )
        .param(
            Param::string("component")
                .default("")
                .describe("Optional React component name, e.g. 'Card'. When set, the JSX is wrapped in 'export default function Card() { return (...); }'. Leave empty (default) to get a bare JSX snippet."),
        )
        .param(
            Param::enumv("comments", ["jsx", "strip"])
                .default("jsx")
                .describe("How to handle HTML comments: 'jsx' (default) rewrites them as {/* ... */} expression containers; 'strip' removes them."),
        )
        .param(
            Param::enumv("boolean_attrs", ["explicit", "shorthand"])
                .default("explicit")
                .describe("How to render valueless attributes such as disabled or required: 'explicit' (default) writes disabled={true}; 'shorthand' leaves the bare attribute."),
        )
        .param(
            Param::enumv("value_attrs", ["default", "keep"])
                .default("default")
                .describe("How to render value/checked on input, textarea and select: 'default' (default) rewrites them to defaultValue/defaultChecked so React does not warn about an uncontrolled field; 'keep' leaves the original names."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/html-to-jsx",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert raw HTML into React JSX.",
    skill(
        description = "Convert an HTML snippet into React JSX. Renames attributes to their React props (class to className, for to htmlFor, tabindex to tabIndex, readonly to readOnly, maxlength to maxLength, http-equiv to httpEquiv, and the SVG set such as stroke-width to strokeWidth), turns an inline style=\"...\" string into a style={{ ... }} object with camelCased properties, renders valueless boolean attributes as {true}, self-closes void tags like <br> and <img>, rewrites HTML comments as {/* ... */}, converts inline on* handlers into arrow functions, escapes braces in text, and wraps multiple roots in a fragment. Options control indentation, an optional component wrapper, comment handling, boolean-attribute style, and value/checked rewriting.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "html-to-jsx", |a: Args| {
            gizza_ai_html_to_jsx_core::html_to_jsx(
                &a.html,
                &a.indent,
                &a.component,
                &a.comments,
                &a.boolean_attrs,
                &a.value_attrs,
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
                    "html": { "type": "string", "description": "The HTML markup to convert to JSX. Paste a snippet such as <div class=\"card\"><img src=\"a.png\"></div>; a full page works too (the doctype is dropped)." },
                    "indent": { "type": "string", "enum": ["2", "4", "tab"], "default": "2", "description": "Indentation of the generated JSX: '2' spaces (default), '4' spaces, or 'tab'." },
                    "component": { "type": "string", "default": "", "description": "Optional React component name, e.g. 'Card'. When set, the JSX is wrapped in 'export default function Card() { return (...); }'. Leave empty (default) to get a bare JSX snippet." },
                    "comments": { "type": "string", "enum": ["jsx", "strip"], "default": "jsx", "description": "How to handle HTML comments: 'jsx' (default) rewrites them as {/* ... */} expression containers; 'strip' removes them." },
                    "boolean_attrs": { "type": "string", "enum": ["explicit", "shorthand"], "default": "explicit", "description": "How to render valueless attributes such as disabled or required: 'explicit' (default) writes disabled={true}; 'shorthand' leaves the bare attribute." },
                    "value_attrs": { "type": "string", "enum": ["default", "keep"], "default": "default", "description": "How to render value/checked on input, textarea and select: 'default' (default) rewrites them to defaultValue/defaultChecked so React does not warn about an uncontrolled field; 'keep' leaves the original names." }
                },
                "required": ["html"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
