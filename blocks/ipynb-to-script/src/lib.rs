//! gizza-ai/ipynb-to-script — chat skill block on the shared tool abstraction.
//! Extracts the code cells of a Jupyter `.ipynb` notebook into a clean Python
//! script (or Markdown), dropping outputs and execution counts. The chat schema
//! is single-sourced from descriptor() (which also drives the CLI); handle()
//! delegates to run_skill. Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_ipynb_to_script_core::{convert, Output};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    notebook: String,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default = "default_true")]
    include_markdown: bool,
    #[serde(default)]
    include_outputs: bool,
    #[serde(default)]
    cell_markers: bool,
}
fn default_output() -> String {
    "script".to_string()
}
fn default_true() -> bool {
    true
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
            Param::enumv("output", ["script", "markdown"])
                .default("script")
                .describe("Output format: 'script' for a Python .py file (code verbatim, markdown as # comments), or 'markdown' for a .md document (markdown verbatim, code in fenced blocks). Default 'script'."),
        )
        .param(
            Param::boolean("include_markdown")
                .default(true)
                .describe("Keep markdown/raw cells (as # comments in script mode, verbatim in markdown mode). When false they are dropped, leaving code only. Default true."),
        )
        .param(
            Param::boolean("include_outputs")
                .default(false)
                .describe("Append each code cell's stored text outputs (as # comments in script mode, fenced blocks in markdown mode). Default false — outputs and execution counts are dropped."),
        )
        .param(
            Param::boolean("cell_markers")
                .default(false)
                .describe("Emit `# %%` markers before each cell (VS Code / Jupytext 'percent' format) in script mode. Ignored in markdown mode. Default false."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/ipynb-to-script",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Extract the code cells of a Jupyter .ipynb into a clean script",
    skill(
        description = "Extract the code cells of a Jupyter `.ipynb` notebook (paste its JSON) into a clean Python script, dropping outputs and execution counts. Cell order is preserved. output='script' (default) gives a .py file with code verbatim and markdown cells as # comments; output='markdown' gives a .md file with markdown verbatim and code in fenced blocks. include_markdown=false drops markdown/raw cells (code only). include_outputs=true appends stored text outputs as comments. cell_markers=true adds `# %%` VS Code/Jupytext markers before each cell. Returns the converted text. Runs locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "ipynb-to-script", |a: Args| {
            let output = Output::parse(&a.output).map_err(SkillError::InvalidArgs)?;
            convert(
                &a.notebook,
                output,
                a.include_markdown,
                a.include_outputs,
                a.cell_markers,
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

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "notebook": { "type": "string", "description": "The full contents of a Jupyter `.ipynb` file (its JSON). Paste the file's text." },
                    "output": { "type": "string", "enum": ["script", "markdown"], "default": "script", "description": "Output format: 'script' for a Python .py file (code verbatim, markdown as # comments), or 'markdown' for a .md document (markdown verbatim, code in fenced blocks). Default 'script'." },
                    "include_markdown": { "type": "boolean", "default": true, "description": "Keep markdown/raw cells (as # comments in script mode, verbatim in markdown mode). When false they are dropped, leaving code only. Default true." },
                    "include_outputs": { "type": "boolean", "default": false, "description": "Append each code cell's stored text outputs (as # comments in script mode, fenced blocks in markdown mode). Default false — outputs and execution counts are dropped." },
                    "cell_markers": { "type": "boolean", "default": false, "description": "Emit `# %%` markers before each cell (VS Code / Jupytext 'percent' format) in script mode. Ignored in markdown mode. Default false." }
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
