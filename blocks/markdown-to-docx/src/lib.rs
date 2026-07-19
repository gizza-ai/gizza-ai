//! gizza-ai/markdown-to-docx — turn Markdown text into a real binary Microsoft
//! Word `.docx` document (an OOXML ZIP of XML parts). The chat schema is
//! single-sourced from `descriptor()` (which also drives the CLI); `handle()`
//! builds a base64 download envelope like markdown-to-pptx / csv-to-xlsx (pure
//! compute, binary output — no host calls).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use gizza_ai_block_utils::{Envelope, ForUi, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_markdown_to_docx_core::{
    to_docx_with_count, FontFamily, PageSize, MAX_FONT_SIZE, MIN_FONT_SIZE,
};
use serde::Deserialize;
use wafer_sdk::*;

/// OOXML Word document (`.docx`) MIME type.
const DOCX_MIME: &str = "application/vnd.openxmlformats-officedocument.wordprocessingml.document";
/// Cap the produced document so a runaway input can't blow up the chat transport.
const MAX_OUTPUT_BYTES: usize = 24 * 1024 * 1024;

#[derive(Deserialize, Debug)]
#[serde(default)]
struct Args {
    markdown: String,
    title: String,
    page_size: String,
    font_family: String,
    font_size: f64,
    page_break: bool,
}

impl Default for Args {
    fn default() -> Self {
        Args {
            markdown: String::new(),
            title: String::new(),
            page_size: "letter".to_string(),
            font_family: "calibri".to_string(),
            font_size: 11.0,
            page_break: false,
        }
    }
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("markdown")
                .required()
                .describe("The document body in Markdown. Headings (`#`…`######`) become Word heading styles; paragraphs carry inline **bold**, *italic*, `code` and ~~strikethrough~~; `-`/`1.` become bullet/numbered lists; `> ` becomes a block quote; fenced ``` blocks become monospace code; and `---` becomes a horizontal rule (or page break). Example: `# Report\\n\\nRevenue is **up**.`."),
        )
        .param(
            Param::string("title")
                .describe("Optional document title. When set, it is added as a large title heading at the top and stored as the document's title metadata."),
        )
        .param(
            Param::enumv("page_size", ["letter", "a4"])
                .default("letter")
                .describe("Printed page size: 'letter' (default, 8.5×11in) or 'a4' (210×297mm)."),
        )
        .param(
            Param::enumv("font_family", ["calibri", "aptos", "times_new_roman", "arial"])
                .default("calibri")
                .describe("Body font family for the document: 'calibri' (default), 'aptos', 'times_new_roman', or 'arial'."),
        )
        .param(
            Param::number("font_size")
                .default(11)
                .min(MIN_FONT_SIZE as f64)
                .max(MAX_FONT_SIZE as f64)
                .describe("Body font size in points (8–24). Defaults to 11. Headings scale relative to this."),
        )
        .param(
            Param::boolean("page_break")
                .default(false)
                .describe("When true, a `---` thematic break inserts a page break instead of a horizontal rule. Defaults to false."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct MarkdownToDocx;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/markdown-to-docx",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert Markdown text into a real Microsoft Word .docx document",
    skill(
        description = "Convert Markdown text into a real binary Microsoft Word .docx document and return it as a download. Headings become Word heading styles, paragraphs keep bold/italic/inline-code/strikethrough, `-`/`1.` become bullet and numbered lists, `> ` becomes a block quote, fenced code blocks become monospace, and `---` becomes a rule or page break. Choose the page size (Letter/A4), body font (Calibri/Aptos/Times New Roman/Arial), font size (8–24pt), and whether `---` starts a new page. The output is a genuine OOXML .docx that Word, Google Docs, Pages and LibreOffice Writer open natively.",
        parameters = schema_json()
    ),
)]
impl MarkdownToDocx {
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
        .map_err(|e| SkillError::InvalidArgs(format!("invalid markdown-to-docx args: {e}")))?;
    let page_size = PageSize::parse(&args.page_size).map_err(SkillError::InvalidArgs)?;
    let font_family = FontFamily::parse(&args.font_family).map_err(SkillError::InvalidArgs)?;
    if !args.font_size.is_finite()
        || args.font_size < MIN_FONT_SIZE as f64
        || args.font_size > MAX_FONT_SIZE as f64
    {
        return Err(SkillError::InvalidArgs(format!(
            "font_size must be between {MIN_FONT_SIZE} and {MAX_FONT_SIZE}"
        )));
    }
    let font_size = args.font_size.round() as u32;

    let (bytes, blocks) = to_docx_with_count(
        &args.markdown,
        &args.title,
        page_size,
        font_family,
        font_size,
        args.page_break,
    )
    .map_err(SkillError::InvalidArgs)?;

    if bytes.len() > MAX_OUTPUT_BYTES {
        return Err(SkillError::InvalidArgs(format!(
            "output document is {} bytes, over the {MAX_OUTPUT_BYTES}-byte cap",
            bytes.len()
        )));
    }

    let filename = "document.docx".to_string();
    let out_len = bytes.len();
    let data_url = format!("data:{DOCX_MIME};base64,{}", B64.encode(&bytes));
    let env = Envelope {
        for_llm: format!("wrote a {out_len}-byte .docx document with {blocks} blocks ({filename})"),
        for_ui: ForUi {
            data_url,
            mime: DOCX_MIME.to_string(),
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
                    "markdown": { "type": "string", "description": "The document body in Markdown. Headings (`#`…`######`) become Word heading styles; paragraphs carry inline **bold**, *italic*, `code` and ~~strikethrough~~; `-`/`1.` become bullet/numbered lists; `> ` becomes a block quote; fenced ``` blocks become monospace code; and `---` becomes a horizontal rule (or page break). Example: `# Report\\n\\nRevenue is **up**.`." },
                    "title": { "type": "string", "description": "Optional document title. When set, it is added as a large title heading at the top and stored as the document's title metadata." },
                    "page_size": { "type": "string", "enum": ["letter", "a4"], "default": "letter", "description": "Printed page size: 'letter' (default, 8.5×11in) or 'a4' (210×297mm)." },
                    "font_family": { "type": "string", "enum": ["calibri", "aptos", "times_new_roman", "arial"], "default": "calibri", "description": "Body font family for the document: 'calibri' (default), 'aptos', 'times_new_roman', or 'arial'." },
                    "font_size": { "type": "number", "minimum": 8, "maximum": 24, "default": 11, "description": "Body font size in points (8–24). Defaults to 11. Headings scale relative to this." },
                    "page_break": { "type": "boolean", "default": false, "description": "When true, a `---` thematic break inserts a page break instead of a horizontal rule. Defaults to false." }
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
