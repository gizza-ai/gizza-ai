//! gizza-ai/ipynb-to-markdown — chat skill block on the shared tool abstraction.
//! Renders a Jupyter `.ipynb` notebook into a clean Markdown document: markdown
//! cells verbatim, code cells as fenced blocks, and cell outputs as output
//! sections (images embedded inline as base64 data URIs). The chat schema is
//! single-sourced from descriptor() (which also drives the CLI); handle()
//! delegates to run_skill. Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_ipynb_to_markdown_core::{convert, ImageMode, Options};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    notebook: String,
    #[serde(default = "default_true")]
    include_code: bool,
    #[serde(default = "default_true")]
    include_outputs: bool,
    #[serde(default = "default_true")]
    include_markdown: bool,
    #[serde(default)]
    show_prompts: bool,
    #[serde(default = "default_image_mode")]
    image_mode: String,
}
fn default_true() -> bool {
    true
}
fn default_image_mode() -> String {
    "embed".to_string()
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("notebook")
                .required()
                .describe("The full contents of a Jupyter `.ipynb` file (its JSON). Paste the file's text."),
        )
        .param(
            Param::boolean("include_code")
                .default(true)
                .describe("Include code cells as fenced blocks. When false, code is dropped but outputs are still rendered (like nbconvert --no-input). Default true."),
        )
        .param(
            Param::boolean("include_outputs")
                .default(true)
                .describe("Render each code cell's stored outputs (stream text, results, and error tracebacks) as output sections. When false, outputs are dropped. Default true."),
        )
        .param(
            Param::boolean("include_markdown")
                .default(true)
                .describe("Include markdown and raw cells (verbatim). When false, only code and outputs remain. Default true."),
        )
        .param(
            Param::boolean("show_prompts")
                .default(false)
                .describe("Prefix code cells with `In [n]:` and their outputs with `Out [n]:` execution-count prompts. Default false."),
        )
        .param(
            Param::enumv("image_mode", ["embed", "placeholder", "omit"])
                .default("embed")
                .describe("How image outputs and markdown-cell image attachments are handled: 'embed' inlines them as base64 data URIs (single-file Markdown), 'placeholder' writes a short *[image output]* note, 'omit' drops them. Default 'embed'."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn build_options(a: &Args) -> Result<Options, String> {
    Ok(Options {
        include_code: a.include_code,
        include_outputs: a.include_outputs,
        include_markdown: a.include_markdown,
        show_prompts: a.show_prompts,
        image_mode: ImageMode::parse(&a.image_mode)?,
    })
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/ipynb-to-markdown",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert a Jupyter .ipynb notebook into clean Markdown with outputs",
    skill(
        description = "Convert a Jupyter `.ipynb` notebook (paste its JSON) into a clean Markdown document. Markdown cells are emitted verbatim, code cells become fenced blocks tagged with the kernel language, and cell outputs are rendered as output sections — stream text, results, and error tracebacks (ANSI stripped). Cell order is preserved. Image outputs (PNG/JPEG/GIF/SVG) and markdown-cell attachments are embedded inline as base64 data URIs by default (image_mode='embed'; 'placeholder' or 'omit' instead). The richest representation of each output is chosen (Markdown, then image, then HTML tables, then LaTeX, then plain text). include_code=false drops code but keeps outputs (nbconvert --no-input); include_outputs=false drops outputs; include_markdown=false leaves code only; show_prompts=true adds `In [n]:`/`Out [n]:` labels. Returns the Markdown text. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "ipynb-to-markdown", |a: Args| {
            let opts = build_options(&a).map_err(SkillError::InvalidArgs)?;
            convert(&a.notebook, opts).map_err(SkillError::InvalidArgs)
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
                "type": "object",
                "properties": {
                    "notebook": { "type": "string", "description": "The full contents of a Jupyter `.ipynb` file (its JSON). Paste the file's text." },
                    "include_code": { "type": "boolean", "default": true, "description": "Include code cells as fenced blocks. When false, code is dropped but outputs are still rendered (like nbconvert --no-input). Default true." },
                    "include_outputs": { "type": "boolean", "default": true, "description": "Render each code cell's stored outputs (stream text, results, and error tracebacks) as output sections. When false, outputs are dropped. Default true." },
                    "include_markdown": { "type": "boolean", "default": true, "description": "Include markdown and raw cells (verbatim). When false, only code and outputs remain. Default true." },
                    "show_prompts": { "type": "boolean", "default": false, "description": "Prefix code cells with `In [n]:` and their outputs with `Out [n]:` execution-count prompts. Default false." },
                    "image_mode": { "type": "string", "enum": ["embed", "placeholder", "omit"], "default": "embed", "description": "How image outputs and markdown-cell image attachments are handled: 'embed' inlines them as base64 data URIs (single-file Markdown), 'placeholder' writes a short *[image output]* note, 'omit' drops them. Default 'embed'." }
                },
                "required": ["notebook"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
