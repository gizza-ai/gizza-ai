//! gizza-ai/latex-to-mathml — chat skill block on the shared tool abstraction.
//! Converts a LaTeX math expression into a MathML `<math>` element. The chat
//! schema is single-sourced from descriptor() (which also drives the CLI);
//! handle() delegates to block_utils::run_skill, which runs the core converter.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    latex: String,
    #[serde(default)]
    display: String,
    /// Pretty-print the MathML with one element per indented line.
    #[serde(default)]
    pretty: bool,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("latex")
                .required()
                .describe("The LaTeX math expression to convert, in math mode and with no surrounding $ … $ (e.g. \\frac{a}{b}, x^2, \\sqrt{x+1}, \\sum_{i=1}^{n} i, \\alpha + \\beta)."),
        )
        .param(
            Param::enumv("display", ["block", "inline"])
                .default("block")
                .describe("MathML display mode. 'block' (default) renders a centred standalone equation (display=\"block\"); 'inline' flows the equation within a line of text (display=\"inline\")."),
        )
        .param(
            Param::boolean("pretty")
                .default(false)
                .describe("When true, indent the MathML with one element per line for readability. Default false (compact single line)."),
        )
}
fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/latex-to-mathml",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert LaTeX math to MathML",
    skill(
        description = "Convert a LaTeX math expression into MathML markup (a <math xmlns=\"http://www.w3.org/1998/Math/MathML\"> element). MathML is the W3C standard for math in HTML — it's accessible to screen readers, selectable/searchable as text, and reusable in EPUB, DocBook and Office documents, unlike a rendered image. Pass `latex` as math-mode source with no surrounding $ (e.g. \\frac{a}{b}, x^2, \\sqrt{x+1}, \\sum_{i=1}^{n} i, \\alpha+\\beta, \\left(\\frac{a}{b}\\right)). Covers fractions, super/subscripts, roots, Greek letters, big operators with limits, relations/arrows, scalable delimiters, font styles (\\mathbb/\\mathbf), and matrix/align environments. Set display='inline' for an inline equation (default 'block' is a centred standalone equation), or pretty=true to indent the output. Runs locally on the device.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "latex-to-mathml", |a: Args| {
            gizza_ai_latex_to_mathml_core::run(&a.latex, &a.display, a.pretty)
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
    /// schema, so any future change to the LLM-facing API is intentional and
    /// reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "latex": { "type": "string", "description": "The LaTeX math expression to convert, in math mode and with no surrounding $ … $ (e.g. \\frac{a}{b}, x^2, \\sqrt{x+1}, \\sum_{i=1}^{n} i, \\alpha + \\beta)." },
                    "display": { "type": "string", "enum": ["block", "inline"], "default": "block", "description": "MathML display mode. 'block' (default) renders a centred standalone equation (display=\"block\"); 'inline' flows the equation within a line of text (display=\"inline\")." },
                    "pretty": { "type": "boolean", "default": false, "description": "When true, indent the MathML with one element per line for readability. Default false (compact single line)." }
                },
                "required": ["latex"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
