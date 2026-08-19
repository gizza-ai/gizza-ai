//! gizza-ai/article-to-epub — package article text or cleaned HTML into a valid
//! EPUB 3 ebook. The chat schema is single-sourced from `descriptor()` (which
//! also drives the CLI); `handle()` returns a base64 download envelope like
//! csv-to-xlsx (pure compute, binary output — no host calls).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use gizza_ai_article_to_epub_core::{convert, InputFormat, Options, SplitLevel};
use gizza_ai_block_utils::{Envelope, ForUi, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

/// EPUB OCF container MIME type.
const EPUB_MIME: &str = "application/epub+zip";
/// Cap the produced book so a runaway article can't blow up the chat transport.
const MAX_OUTPUT_BYTES: usize = 24 * 1024 * 1024;

#[derive(Deserialize, Debug)]
#[serde(default)]
struct Args {
    content: String,
    input_format: String,
    title: String,
    author: String,
    language: String,
    publisher: String,
    split_level: String,
    include_title_page: bool,
    base_font_size: f64,
}

impl Default for Args {
    fn default() -> Self {
        Args {
            content: String::new(),
            input_format: "auto".to_string(),
            title: String::new(),
            author: String::new(),
            language: "en".to_string(),
            publisher: String::new(),
            split_level: "h1".to_string(),
            include_title_page: true,
            base_font_size: 0.0,
        }
    }
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("content")
                .required()
                .describe("The article itself: plain text or cleaned HTML. Plain text uses blank lines for paragraphs and understands Markdown-style '# ' headings, '-' bullets and '1.' lists. HTML is re-emitted as well-formed XHTML; scripts, styles and forms are dropped."),
        )
        .param(
            Param::enumv("input_format", ["auto", "html", "text"])
                .default("auto")
                .describe("How to read the content. 'auto' (default) detects HTML by looking for tags like <p> or <h1>; set 'html' or 'text' to force it (use 'text' to keep angle brackets in prose literal)."),
        )
        .param(
            Param::string("title")
                .describe("Book title (dc:title) and the download filename. Leave empty to derive it from the HTML <title>, the first heading, or the first line of text."),
        )
        .param(
            Param::string("author")
                .describe("Author for dc:creator, e.g. 'Jane Doe'. Empty omits the element."),
        )
        .param(
            Param::string("language")
                .default("en")
                .describe("Book language as a BCP-47 tag for dc:language, e.g. 'en', 'de' or 'pt-BR' (default 'en')."),
        )
        .param(
            Param::string("publisher")
                .describe("Publisher or source name for dc:publisher, e.g. the site the article came from. Empty omits the element."),
        )
        .param(
            Param::enumv("split_level", ["none", "h1", "h2", "h1-h2"])
                .default("h1")
                .describe("Where a new chapter starts: 'h1' (default), 'h2', 'h1-h2' for both, or 'none' for one single chapter. Chapter titles become the table of contents."),
        )
        .param(
            Param::boolean("include_title_page")
                .default(true)
                .describe("Write a title page with the title, author and publisher as the first page of the book (default true)."),
        )
        .param(
            Param::number("base_font_size")
                .default(0.0)
                .min(0.0)
                .max(72.0)
                .describe("Base font size in points for the embedded stylesheet, e.g. 12. 0 (default) leaves the size to the reading app, which is what most e-readers expect."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct ArticleToEpub;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/article-to-epub",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Package article text or cleaned HTML into a valid EPUB ebook",
    skill(
        description = "Package an article — plain text or cleaned HTML — into a valid EPUB 3 ebook and return it as a download. Sets the title, author, language and publisher metadata, splits chapters at headings, builds both an EPUB 3 nav document and an EPUB 2 NCX table of contents, and embeds a reading stylesheet. Messy HTML is re-emitted as well-formed XHTML with scripts, styles and forms removed; images are not packaged (their alt text is kept).",
        parameters = schema_json()
    ),
)]
impl ArticleToEpub {
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
        .map_err(|e| SkillError::InvalidArgs(format!("invalid article-to-epub args: {e}")))?;

    let opts = Options {
        input_format: InputFormat::parse(&args.input_format).map_err(SkillError::InvalidArgs)?,
        title: args.title,
        author: args.author,
        language: args.language,
        publisher: args.publisher,
        split_level: SplitLevel::parse(&args.split_level).map_err(SkillError::InvalidArgs)?,
        include_title_page: args.include_title_page,
        base_font_size: args.base_font_size,
    };
    let out = convert(&args.content, &opts).map_err(SkillError::InvalidArgs)?;

    if out.epub.len() > MAX_OUTPUT_BYTES {
        return Err(SkillError::InvalidArgs(format!(
            "output book is {} bytes, over the {MAX_OUTPUT_BYTES}-byte cap",
            out.epub.len()
        )));
    }

    let images = if out.images_dropped > 0 {
        format!(
            "; {} image(s) dropped (alt text kept)",
            out.images_dropped
        )
    } else {
        String::new()
    };
    let data_url = format!("data:{EPUB_MIME};base64,{}", B64.encode(&out.epub));
    let env = Envelope {
        for_llm: format!(
            "packaged \"{}\" ({} format) into a {}-byte EPUB with {} chapter(s) and {} word(s){images} ({})",
            out.title,
            out.format.name(),
            out.epub.len(),
            out.chapters,
            out.words,
            out.filename,
        ),
        for_ui: ForUi {
            data_url,
            mime: EPUB_MIME.to_string(),
            filename: out.filename,
        },
    };
    serde_json::to_vec(&env).map_err(|e| SkillError::Serialize(format!("serialize envelope: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// copy, so an accidental descriptor edit can't silently change the
    /// LLM-facing schema (and the page controls the manifest renders from it).
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "content": { "type": "string", "description": "The article itself: plain text or cleaned HTML. Plain text uses blank lines for paragraphs and understands Markdown-style '# ' headings, '-' bullets and '1.' lists. HTML is re-emitted as well-formed XHTML; scripts, styles and forms are dropped." },
                    "input_format": { "type": "string", "enum": ["auto", "html", "text"], "default": "auto", "description": "How to read the content. 'auto' (default) detects HTML by looking for tags like <p> or <h1>; set 'html' or 'text' to force it (use 'text' to keep angle brackets in prose literal)." },
                    "title": { "type": "string", "description": "Book title (dc:title) and the download filename. Leave empty to derive it from the HTML <title>, the first heading, or the first line of text." },
                    "author": { "type": "string", "description": "Author for dc:creator, e.g. 'Jane Doe'. Empty omits the element." },
                    "language": { "type": "string", "default": "en", "description": "Book language as a BCP-47 tag for dc:language, e.g. 'en', 'de' or 'pt-BR' (default 'en')." },
                    "publisher": { "type": "string", "description": "Publisher or source name for dc:publisher, e.g. the site the article came from. Empty omits the element." },
                    "split_level": { "type": "string", "enum": ["none", "h1", "h2", "h1-h2"], "default": "h1", "description": "Where a new chapter starts: 'h1' (default), 'h2', 'h1-h2' for both, or 'none' for one single chapter. Chapter titles become the table of contents." },
                    "include_title_page": { "type": "boolean", "default": true, "description": "Write a title page with the title, author and publisher as the first page of the book (default true)." },
                    "base_font_size": { "type": "number", "default": 0.0, "minimum": 0, "maximum": 72, "description": "Base font size in points for the embedded stylesheet, e.g. 12. 0 (default) leaves the size to the reading app, which is what most e-readers expect." }
                },
                "required": ["content"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
