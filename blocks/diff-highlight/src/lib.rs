//! gizza-ai/diff-highlight — render a unified diff or a two-text comparison
//! into a shareable, syntax-highlighted PNG.
//!
//! Text params are single-sourced through `descriptor()` for chat and CLI. The
//! pure renderer lives in the core crate (`syntect` + `fontdue` + `png`, all
//! wasm-safe). Like `code-screenshot`, this is chat + CLI only: image bytes from
//! text input do not have a standalone generated page shape in this repo.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{
    build_media_envelope, Input, Param, SkillError, SkillResultExt, ToolDescriptor,
};
use gizza_ai_diff_highlight_core::{render_png, DiffInput, Layout, Options, RenderError};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_OUTPUT_BYTES: usize = 8 * 1024 * 1024;

#[derive(Deserialize)]
struct Args {
    #[serde(default)]
    diff: String,
    #[serde(default)]
    left: String,
    #[serde(default)]
    right: String,
    #[serde(default = "default_layout")]
    layout: String,
    #[serde(default)]
    language: String,
    #[serde(default)]
    theme: String,
    #[serde(default = "default_true")]
    line_numbers: bool,
    #[serde(default = "default_true")]
    word_highlight: bool,
    #[serde(default)]
    ignore_whitespace: bool,
    #[serde(default = "default_context")]
    context: u64,
}

fn default_layout() -> String {
    "side-by-side".into()
}
fn default_true() -> bool {
    true
}
fn default_context() -> u64 {
    3
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("diff")
                .default("")
                .describe("Unified diff / patch text to render. Use this OR provide both `left` and `right`; leave blank when comparing two raw texts."),
        )
        .param(
            Param::string("left")
                .default("")
                .describe("Original text for two-text comparison mode. Provide with `right`; leave blank when using `diff`."),
        )
        .param(
            Param::string("right")
                .default("")
                .describe("Modified text for two-text comparison mode. Provide with `left`; leave blank when using `diff`."),
        )
        .param(
            Param::enumv("layout", ["side-by-side", "unified"])
                .default("side-by-side")
                .describe("Image layout: `side-by-side` (default, old left / new right) or `unified` (classic inline -/+ rows)."),
        )
        .param(
            Param::string("language")
                .default("")
                .describe("Optional syntax language hint such as `rust`, `python`, `javascript`, `typescript`, `bash`, `go`, `yaml` or `markdown`. Unknown or blank renders as plaintext."),
        )
        .param(
            Param::string("theme")
                .default("")
                .describe("Optional syntect theme name such as `base16-ocean.dark`, `Solarized (dark)`, `Solarized (light)` or `InspiredGitHub`. Blank or unknown falls back to a dark theme."),
        )
        .param(
            Param::boolean("line_numbers")
                .default(true)
                .describe("Show old/new line number gutters. Default true."),
        )
        .param(
            Param::boolean("word_highlight")
                .default(true)
                .describe("Highlight the changed character span inside paired changed lines. Default true."),
        )
        .param(
            Param::boolean("ignore_whitespace")
                .default(false)
                .describe("Treat lines that differ only in whitespace as unchanged. Default false."),
        )
        .param(
            Param::integer("context")
                .min(0.0)
                .max(50.0)
                .default(3)
                .describe("Unchanged context lines kept around each change before long runs collapse into a note. Default 3, maximum 50."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn map_render_err(e: RenderError) -> SkillError {
    SkillError::InvalidArgs(format!("invalid diff-highlight args: {e}"))
}

fn opt_string(s: &str) -> Option<String> {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("diff-highlight")?;
    let input = gizza_ai_diff_highlight_core::pick_input(
        Some(args.diff.as_str()),
        Some(args.left.as_str()),
        Some(args.right.as_str()),
    )
    .map_err(map_render_err)?;
    let layout = Layout::parse(&args.layout).map_err(map_render_err)?;
    let options = Options {
        layout,
        language: opt_string(&args.language),
        theme: opt_string(&args.theme),
        line_numbers: args.line_numbers,
        word_highlight: args.word_highlight,
        ignore_whitespace: args.ignore_whitespace,
        context: (args.context as usize).min(gizza_ai_diff_highlight_core::MAX_CONTEXT),
    };
    let (png, width, height) = render_png(input, &options).map_err(map_render_err)?;
    let mode = match input {
        DiffInput::Patch(_) => "unified diff",
        DiffInput::Pair { .. } => "two-text comparison",
    };
    let for_llm = format!(
        "rendered a {width}x{height} PNG ({bytes} bytes) from {mode} using {layout} layout",
        bytes = png.len(),
        layout = args.layout.trim()
    );
    build_media_envelope(
        &png,
        "image/png",
        "diff-highlight.png".to_string(),
        for_llm,
        MAX_OUTPUT_BYTES,
    )
}

#[cfg(target_arch = "wasm32")]
struct DiffHighlight;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/diff-highlight",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Render a diff as a syntax-highlighted PNG",
    skill(
        description = "Render a unified diff or two raw texts into a shareable PNG image with syntax highlighting, add/remove tinting, optional side-by-side or unified layout, line numbers, intra-line change highlighting, whitespace-ignore comparison and configurable context. Use this when the user wants a picture of a diff for PRs, chats or release notes rather than text output. Returns an image, not text.",
        parameters = schema_json()
    )
)]
impl DiffHighlight {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
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
                    "diff": { "type": "string", "default": "", "description": "Unified diff / patch text to render. Use this OR provide both `left` and `right`; leave blank when comparing two raw texts." },
                    "left": { "type": "string", "default": "", "description": "Original text for two-text comparison mode. Provide with `right`; leave blank when using `diff`." },
                    "right": { "type": "string", "default": "", "description": "Modified text for two-text comparison mode. Provide with `left`; leave blank when using `diff`." },
                    "layout": { "type": "string", "enum": ["side-by-side", "unified"], "default": "side-by-side", "description": "Image layout: `side-by-side` (default, old left / new right) or `unified` (classic inline -/+ rows)." },
                    "language": { "type": "string", "default": "", "description": "Optional syntax language hint such as `rust`, `python`, `javascript`, `typescript`, `bash`, `go`, `yaml` or `markdown`. Unknown or blank renders as plaintext." },
                    "theme": { "type": "string", "default": "", "description": "Optional syntect theme name such as `base16-ocean.dark`, `Solarized (dark)`, `Solarized (light)` or `InspiredGitHub`. Blank or unknown falls back to a dark theme." },
                    "line_numbers": { "type": "boolean", "default": true, "description": "Show old/new line number gutters. Default true." },
                    "word_highlight": { "type": "boolean", "default": true, "description": "Highlight the changed character span inside paired changed lines. Default true." },
                    "ignore_whitespace": { "type": "boolean", "default": false, "description": "Treat lines that differ only in whitespace as unchanged. Default false." },
                    "context": { "type": "integer", "minimum": 0, "maximum": 50, "default": 3, "description": "Unchanged context lines kept around each change before long runs collapse into a note. Default 3, maximum 50." }
                },
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn args_defaults_match_descriptor() {
        let a: Args = serde_json::from_str(r#"{"diff":"@@\n-a\n+b\n"}"#).unwrap();
        assert_eq!(a.layout, "side-by-side");
        assert!(a.line_numbers);
        assert!(a.word_highlight);
        assert!(!a.ignore_whitespace);
        assert_eq!(a.context, 3);
    }

    #[test]
    fn bad_layout_maps_to_invalid_args() {
        let err = map_render_err(RenderError::BadLayout("grid".into()));
        assert!(matches!(err, SkillError::InvalidArgs(ref s) if s.contains("side-by-side")));
    }

    #[test]
    fn run_returns_a_png_media_envelope() {
        let out = run(br#"{"diff":"@@\n-old\n+new\n","language":"rust"}"#.to_vec()).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&out).unwrap();
        assert!(v["_for_llm"].as_str().unwrap().contains("PNG"));
        assert!(v["_for_ui"]["data_url"]
            .as_str()
            .unwrap()
            .starts_with("data:image/png;base64,"));
    }
}
