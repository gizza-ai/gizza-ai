//! gizza-ai/markdown-to-pptx — turn a Markdown outline into a real binary
//! PowerPoint `.pptx` presentation (an OOXML ZIP of slide XML parts). The chat
//! schema is single-sourced from `descriptor()` (which also drives the CLI);
//! `handle()` builds a base64 download envelope like csv-to-xlsx / create-zip
//! (pure compute, binary output — no host calls).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use gizza_ai_block_utils::{Envelope, ForUi, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_markdown_to_pptx_core::{to_pptx_with_count, AspectRatio, SplitLevel, Theme};
use serde::Deserialize;
use wafer_sdk::*;

/// OOXML presentation (`.pptx`) MIME type.
const PPTX_MIME: &str = "application/vnd.openxmlformats-officedocument.presentationml.presentation";
/// Cap the produced deck so a runaway outline can't blow up the chat transport.
const MAX_OUTPUT_BYTES: usize = 24 * 1024 * 1024;

#[derive(Deserialize, Debug)]
#[serde(default)]
struct Args {
    markdown: String,
    title: String,
    split_level: String,
    theme: String,
    aspect_ratio: String,
}

impl Default for Args {
    fn default() -> Self {
        Args {
            markdown: String::new(),
            title: String::new(),
            split_level: "h1".to_string(),
            theme: "light".to_string(),
            aspect_ratio: "16:9".to_string(),
        }
    }
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("markdown")
                .required()
                .describe("The presentation outline in Markdown. Headings become slide titles, list items and paragraphs become bullets, and a thematic break (`---`) starts a new slide. Example: `# Intro\\n- First point\\n- Second point`."),
        )
        .param(
            Param::string("title")
                .describe("Optional deck title. When set, it is added as a centered title slide at the front and stored as the document's title."),
        )
        .param(
            Param::enumv("split_level", ["h1", "h2", "both"])
                .default("h1")
                .describe("Where headings cut the outline into slides: 'h1' (default) starts a slide at each `#`, 'h2' at each `##`, 'both' at every `#` and `##`. Thematic breaks (`---`) always start a new slide."),
        )
        .param(
            Param::enumv("theme", ["light", "dark"])
                .default("light")
                .describe("Slide color theme: 'light' (default) is dark text on a white background; 'dark' is light text on a near-black background."),
        )
        .param(
            Param::enumv("aspect_ratio", ["16:9", "4:3"])
                .default("16:9")
                .describe("Slide aspect ratio: '16:9' (default) widescreen or '4:3' standard."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct MarkdownToPptx;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/markdown-to-pptx",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert a Markdown outline into a real PowerPoint .pptx presentation",
    skill(
        description = "Convert a Markdown outline into a real binary PowerPoint .pptx presentation and return it as a download. Headings become slide titles, list items and paragraphs become bullets, and thematic breaks (`---`) split slides. Choose where headings split slides (h1/h2/both), a light or dark theme, and a 16:9 or 4:3 aspect ratio. The output is a genuine OOXML .pptx package that PowerPoint, Keynote, Google Slides and LibreOffice Impress open natively.",
        parameters = schema_json()
    ),
)]
impl MarkdownToPptx {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body)
        .map_err(|e| SkillError::InvalidArgs(format!("invalid markdown-to-pptx args: {e}")))?;
    let split = SplitLevel::parse(&args.split_level).map_err(SkillError::InvalidArgs)?;
    let theme = Theme::parse(&args.theme).map_err(SkillError::InvalidArgs)?;
    let aspect = AspectRatio::parse(&args.aspect_ratio).map_err(SkillError::InvalidArgs)?;

    let (bytes, slides) = to_pptx_with_count(&args.markdown, &args.title, split, theme, aspect)
        .map_err(SkillError::InvalidArgs)?;

    if bytes.len() > MAX_OUTPUT_BYTES {
        return Err(SkillError::InvalidArgs(format!(
            "output presentation is {} bytes, over the {MAX_OUTPUT_BYTES}-byte cap",
            bytes.len()
        )));
    }

    let filename = "presentation.pptx".to_string();
    let out_len = bytes.len();
    let data_url = format!("data:{PPTX_MIME};base64,{}", B64.encode(&bytes));
    let env = Envelope {
        for_llm: format!("wrote a {slides}-slide {out_len}-byte .pptx presentation ({filename})"),
        for_ui: ForUi {
            data_url,
            mime: PPTX_MIME.to_string(),
            filename,
        },
    };
    serde_json::to_vec(&env).map_err(|e| SkillError::Serialize(format!("serialize envelope: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// copy, so an accidental descriptor edit can't silently change the
    /// LLM-facing schema (and the page control the manifest renders from it).
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "markdown": { "type": "string", "description": "The presentation outline in Markdown. Headings become slide titles, list items and paragraphs become bullets, and a thematic break (`---`) starts a new slide. Example: `# Intro\\n- First point\\n- Second point`." },
                    "title": { "type": "string", "description": "Optional deck title. When set, it is added as a centered title slide at the front and stored as the document's title." },
                    "split_level": { "type": "string", "enum": ["h1", "h2", "both"], "default": "h1", "description": "Where headings cut the outline into slides: 'h1' (default) starts a slide at each `#`, 'h2' at each `##`, 'both' at every `#` and `##`. Thematic breaks (`---`) always start a new slide." },
                    "theme": { "type": "string", "enum": ["light", "dark"], "default": "light", "description": "Slide color theme: 'light' (default) is dark text on a white background; 'dark' is light text on a near-black background." },
                    "aspect_ratio": { "type": "string", "enum": ["16:9", "4:3"], "default": "16:9", "description": "Slide aspect ratio: '16:9' (default) widescreen or '4:3' standard." }
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
