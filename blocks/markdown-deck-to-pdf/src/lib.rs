//! gizza-ai/markdown-deck-to-pdf — turn a Markdown slide deck into a paginated
//! PDF with one slide per page. The chat schema is single-sourced from
//! `descriptor()` (which also drives the CLI); `handle()` builds a base64
//! download envelope like markdown-to-pptx / csv-to-xlsx (pure compute, binary
//! output — no host calls).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use gizza_ai_block_utils::{Envelope, ForUi, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_markdown_deck_to_pdf_core::{
    to_pdf_with_counts, DeckOptions, SlideSize, SplitLevel, Theme, DEFAULT_FONT_SIZE,
};
use serde::Deserialize;
use wafer_sdk::*;

/// Portable Document Format.
const PDF_MIME: &str = "application/pdf";
/// Cap the produced PDF so a runaway deck can't blow up the chat transport.
const MAX_OUTPUT_BYTES: usize = 8 * 1024 * 1024;

#[derive(Deserialize, Debug)]
#[serde(default)]
struct Args {
    markdown: String,
    title: String,
    split_level: String,
    slide_size: String,
    theme: String,
    font_size: f64,
    header: String,
    footer: String,
    page_numbers: bool,
    outline: bool,
}

impl Default for Args {
    fn default() -> Self {
        Args {
            markdown: String::new(),
            title: String::new(),
            split_level: "h1".to_string(),
            slide_size: "16:9".to_string(),
            theme: "light".to_string(),
            font_size: DEFAULT_FONT_SIZE,
            header: String::new(),
            footer: String::new(),
            page_numbers: true,
            outline: true,
        }
    }
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("markdown")
                .required()
                .describe("The slide deck written in Markdown. A thematic break (`---`) always starts a new slide; headings start slides per `split_level` and become the slide title. Lists, paragraphs, quotes, tables, code blocks and inline `**bold**`/`*italic*`/`` `code` `` all render. Example: `# Intro\\n- First point\\n\\n---\\n\\n# Next`."),
        )
        .param(
            Param::string("title")
                .describe("Optional deck title. When set, a centered cover slide with this text is added as page 1. Example: 'Q3 Business Review'."),
        )
        .param(
            Param::enumv("split_level", ["h1", "h2", "both", "none"])
                .default("h1")
                .describe("Which heading levels start a new slide: 'h1' (default) breaks at each `#`, 'h2' at each `##`, 'both' at every `#` and `##`, 'none' never breaks on headings. A `---` thematic break always starts a new slide regardless."),
        )
        .param(
            Param::enumv("slide_size", ["16:9", "4:3", "a4-landscape", "letter-landscape"])
                .default("16:9")
                .describe("Page geometry for every slide, always landscape: '16:9' (default, 960x540 pt), '4:3' (720x540 pt), 'a4-landscape' (842x595 pt) or 'letter-landscape' (792x612 pt). Use a paper size when the deck will be printed as a handout."),
        )
        .param(
            Param::enumv("theme", ["light", "dark"])
                .default("light")
                .describe("Slide colour theme: 'light' (default) is dark text on a white background; 'dark' is light text on a near-black background."),
        )
        .param(
            Param::number("font_size")
                .default(DEFAULT_FONT_SIZE)
                .min(8.0)
                .max(48.0)
                .describe("Base body text size in points, 8-48 (default 20). Slide titles scale from it. Body text also shrinks automatically, down to half this size, so a busy slide still fits on one page."),
        )
        .param(
            Param::string("header")
                .describe("Optional text repeated in the top-left corner of every slide, e.g. a project or client name. Empty (default) leaves the header off."),
        )
        .param(
            Param::string("footer")
                .describe("Optional text repeated in the bottom-left corner of every slide, e.g. 'Confidential' or a date. Empty (default) leaves the footer off."),
        )
        .param(
            Param::boolean("page_numbers")
                .default(true)
                .describe("Print 'n / total' in the bottom-right corner of every page. Default true; set false for a clean deck with no numbering."),
        )
        .param(
            Param::boolean("outline")
                .default(true)
                .describe("Add a PDF outline (bookmark sidebar) with one entry per slide, titled by its heading. Default true; set false to omit the outline."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct MarkdownDeckToPdf;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/markdown-deck-to-pdf",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert a Markdown slide deck into a PDF with one slide per page",
    skill(
        description = "Convert a Markdown slide deck into a paginated PDF with exactly one slide per page, and return it as a download. Thematic breaks (`---`) always split slides and headings can split at `#`, `##`, both or not at all. Choose a 16:9, 4:3, A4-landscape or Letter-landscape page, a light or dark theme, a base font size, repeated header/footer text, page numbers, and a PDF outline with one bookmark per slide. Body text shrinks automatically to fit each slide. Output is a real PDF that any viewer opens; base-14 fonts only, so text is folded to Latin-1 and images render as their alt text.",
        parameters = schema_json()
    ),
)]
impl MarkdownDeckToPdf {
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
        .map_err(|e| SkillError::InvalidArgs(format!("invalid markdown-deck-to-pdf args: {e}")))?;
    let split = SplitLevel::parse(&args.split_level).map_err(SkillError::InvalidArgs)?;
    let size = SlideSize::parse(&args.slide_size).map_err(SkillError::InvalidArgs)?;
    let theme = Theme::parse(&args.theme).map_err(SkillError::InvalidArgs)?;

    let opts = DeckOptions {
        title: &args.title,
        split,
        size,
        theme,
        font_size: args.font_size,
        header: &args.header,
        footer: &args.footer,
        page_numbers: args.page_numbers,
        outline: args.outline,
    };
    let (bytes, slides, pages) =
        to_pdf_with_counts(&args.markdown, &opts).map_err(SkillError::InvalidArgs)?;

    if bytes.len() > MAX_OUTPUT_BYTES {
        return Err(SkillError::InvalidArgs(format!(
            "output PDF is {} bytes, over the {MAX_OUTPUT_BYTES}-byte cap",
            bytes.len()
        )));
    }

    let filename = "deck.pdf".to_string();
    let out_len = bytes.len();
    let data_url = format!("data:{PDF_MIME};base64,{}", B64.encode(&bytes));
    let env = Envelope {
        for_llm: format!(
            "wrote a {slides}-slide {pages}-page {out_len}-byte PDF deck ({filename})"
        ),
        for_ui: ForUi {
            data_url,
            mime: PDF_MIME.to_string(),
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
    /// LLM-facing schema (or the page controls the manifest renders from it).
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "markdown": { "type": "string", "description": "The slide deck written in Markdown. A thematic break (`---`) always starts a new slide; headings start slides per `split_level` and become the slide title. Lists, paragraphs, quotes, tables, code blocks and inline `**bold**`/`*italic*`/`` `code` `` all render. Example: `# Intro\\n- First point\\n\\n---\\n\\n# Next`." },
                    "title": { "type": "string", "description": "Optional deck title. When set, a centered cover slide with this text is added as page 1. Example: 'Q3 Business Review'." },
                    "split_level": { "type": "string", "enum": ["h1", "h2", "both", "none"], "default": "h1", "description": "Which heading levels start a new slide: 'h1' (default) breaks at each `#`, 'h2' at each `##`, 'both' at every `#` and `##`, 'none' never breaks on headings. A `---` thematic break always starts a new slide regardless." },
                    "slide_size": { "type": "string", "enum": ["16:9", "4:3", "a4-landscape", "letter-landscape"], "default": "16:9", "description": "Page geometry for every slide, always landscape: '16:9' (default, 960x540 pt), '4:3' (720x540 pt), 'a4-landscape' (842x595 pt) or 'letter-landscape' (792x612 pt). Use a paper size when the deck will be printed as a handout." },
                    "theme": { "type": "string", "enum": ["light", "dark"], "default": "light", "description": "Slide colour theme: 'light' (default) is dark text on a white background; 'dark' is light text on a near-black background." },
                    "font_size": { "type": "number", "default": 20.0, "minimum": 8, "maximum": 48, "description": "Base body text size in points, 8-48 (default 20). Slide titles scale from it. Body text also shrinks automatically, down to half this size, so a busy slide still fits on one page." },
                    "header": { "type": "string", "description": "Optional text repeated in the top-left corner of every slide, e.g. a project or client name. Empty (default) leaves the header off." },
                    "footer": { "type": "string", "description": "Optional text repeated in the bottom-left corner of every slide, e.g. 'Confidential' or a date. Empty (default) leaves the footer off." },
                    "page_numbers": { "type": "boolean", "default": true, "description": "Print 'n / total' in the bottom-right corner of every page. Default true; set false for a clean deck with no numbering." },
                    "outline": { "type": "boolean", "default": true, "description": "Add a PDF outline (bookmark sidebar) with one entry per slide, titled by its heading. Default true; set false to omit the outline." }
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
