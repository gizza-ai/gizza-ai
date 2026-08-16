//! gizza-ai/markdown-link-flattener — chat skill block on the shared tool abstraction.
//! Flattens Markdown inline links to visible text or text plus URLs.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    markdown: String,
    #[serde(default = "default_link_mode")]
    link_mode: String,
    #[serde(default = "default_image_mode")]
    image_mode: String,
    #[serde(default = "default_reference_definitions")]
    reference_definitions: String,
    #[serde(default = "default_true")]
    preserve_code: bool,
}

fn default_link_mode() -> String {
    "text".to_string()
}
fn default_image_mode() -> String {
    "alt_text".to_string()
}
fn default_reference_definitions() -> String {
    "drop".to_string()
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("markdown")
                .required()
                .describe("Markdown text whose inline links should be flattened. Inline links like [visible text](https://example.com), image links like ![alt](image.png), and reference definition lines like [id]: https://example.com are handled. Code spans and fenced code blocks are preserved by default. Up to 1000000 bytes per run."),
        )
        .param(
            Param::enumv("link_mode", ["text", "text_url", "url"])
                .default("text")
                .describe("How normal inline links are rewritten. text keeps only the visible label, so [docs](https://example.com) becomes docs. text_url keeps the label and appends the destination in parentheses, producing docs (https://example.com). url keeps only the destination."),
        )
        .param(
            Param::enumv("image_mode", ["alt_text", "alt_url", "drop", "keep_markdown"])
                .default("alt_text")
                .describe("How image syntax is treated. alt_text keeps the image alt text, alt_url writes alt text plus the image URL, drop removes the image entirely, and keep_markdown leaves image syntax untouched while still flattening normal links."),
        )
        .param(
            Param::enumv("reference_definitions", ["drop", "keep"])
                .default("drop")
                .describe("Whether reference definition lines such as [docs]: https://example.com are removed. drop is the default because the inline link syntax has been flattened and the definitions usually become dead footnotes. keep leaves those lines as written."),
        )
        .param(
            Param::boolean("preserve_code")
                .default(true)
                .describe("Leave backtick code spans and fenced code blocks unchanged. On by default so README snippets like `[x](y)` keep showing Markdown syntax. Turn it off only when the whole document should be flattened even inside examples."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/markdown-link-flattener",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Remove Markdown link syntax while keeping visible text or appending URLs",
    skill(
        description = "Flatten Markdown links in pasted Markdown. Pass `markdown` and choose `link_mode`: `text` keeps only visible link text, `text_url` writes `text (url)`, and `url` keeps only destinations. Images are controlled separately with `image_mode`: keep alt text, alt text plus URL, drop them, or preserve image Markdown. Reference definition lines can be dropped or kept with `reference_definitions`; dropping is useful after inline link syntax has been removed. `preserve_code` leaves backtick code spans and fenced code blocks unchanged by default so examples of Markdown syntax are not altered. Up to 1000000 bytes, deterministic and browser-local.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "markdown-link-flattener", |a: Args| {
            gizza_ai_markdown_link_flattener_core::run(
                &a.markdown,
                &a.link_mode,
                &a.image_mode,
                &a.reference_definitions,
                a.preserve_code,
            )
            .map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}
