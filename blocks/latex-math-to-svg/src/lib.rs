//! gizza-ai/latex-math-to-svg — render a LaTeX math expression into a crisp,
//! standalone SVG with no TeX install. Pure-Rust typesetter, so it runs on every
//! backend (incl. the chat Service Worker). The SVG is returned as an
//! `image/svg+xml` data-URL envelope. Surfaces: chat + CLI (image-bytes output →
//! no page, like the QR / chart tools).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::build_media_envelope;
use gizza_ai_block_utils::{Input, Param, SkillError, SkillResultExt, ToolDescriptor};
use gizza_ai_latex_math_to_svg_core::render_svg;
use serde::Deserialize;
use wafer_sdk::*;

const MAX_OUTPUT_BYTES: usize = 4 * 1024 * 1024;

#[derive(Deserialize, Debug)]
struct Args {
    latex: String,
    #[serde(default)]
    color: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("latex").required().describe(
                "The LaTeX math expression to render, e.g. 'x = \\frac{-b \\pm \\sqrt{b^2-4ac}}{2a}'. Math-mode syntax (no surrounding $). Supports fractions, sub/superscripts, roots, Greek letters, sums/integrals, and \\left...\\right delimiters.",
            ),
        )
        .param(
            Param::string("color")
                .describe("Text colour as a CSS value (e.g. '#1a1a1a', 'black', 'rgb(0,0,0)'). Defaults to dark grey."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/latex-math-to-svg",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Render LaTeX math to SVG",
    skill(
        description = "Render a LaTeX math expression into a crisp, scalable SVG — no TeX install needed. Pass `latex` as math-mode source (no surrounding $), e.g. \\frac{a}{b}, x^2, \\sqrt{x+1}, \\sum_{i=1}^{n} i, \\alpha+\\beta, \\left(\\frac{a}{b}\\right). Supports fractions, super/subscripts, roots, Greek letters, big operators with limits, relations/arrows, and scalable delimiters. Optional `color` sets the text colour (default dark grey). Returns an image/svg+xml image. Runs locally on the device.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("latex-math-to-svg")?;
    let svg = render_svg(&args.latex, &args.color).map_err(SkillError::InvalidArgs)?;
    build_media_envelope(
        svg.as_bytes(),
        "image/svg+xml",
        "math.svg".to_string(),
        format!("Rendered LaTeX math as SVG ({} bytes)", svg.len()),
        MAX_OUTPUT_BYTES,
    )
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
                    "latex": { "type": "string", "description": "The LaTeX math expression to render, e.g. 'x = \\frac{-b \\pm \\sqrt{b^2-4ac}}{2a}'. Math-mode syntax (no surrounding $). Supports fractions, sub/superscripts, roots, Greek letters, sums/integrals, and \\left...\\right delimiters." },
                    "color": { "type": "string", "description": "Text colour as a CSS value (e.g. '#1a1a1a', 'black', 'rgb(0,0,0)'). Defaults to dark grey." }
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
