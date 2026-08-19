//! gizza-ai/newsletter-html-builder — chat skill block on the shared tool
//! abstraction. Builds responsive, email-client-safe newsletter HTML from a
//! plain list of sections. The chat schema is single-sourced from descriptor()
//! (which also drives the CLI); handle() delegates to block_utils::run_skill.
//! No host calls.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    sections: String,
    #[serde(default)]
    subject: String,
    #[serde(default)]
    preheader: String,
    #[serde(default)]
    width: f64,
    #[serde(default)]
    background: String,
    #[serde(default)]
    content_background: String,
    #[serde(default)]
    text_color: String,
    #[serde(default)]
    accent: String,
    #[serde(default)]
    font: String,
    /// Emit the `prefers-color-scheme: dark` block + colour-scheme meta tags
    /// (default true).
    #[serde(default = "default_true")]
    dark_mode: bool,
}

fn default_true() -> bool {
    true
}

/// Single-source param descriptor → chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("sections")
                .required()
                .describe(
                    "The newsletter content, ONE section per line, pipe-separated as \
                     'type | content | extra'. Types: 'heading | Big title'; \
                     'subheading | Smaller title'; 'text | A paragraph'; \
                     'button | Read more | https://example.com'; \
                     'image | https://example.com/hero.png | Alt text | https://example.com' \
                     (4th part is an optional click-through link); \
                     'columns | Left copy | Right copy' (stacks on mobile); 'divider'; \
                     'spacer | 24' (height in px); 'footer | Small print'; \
                     'html | <p>raw markup</p>'. Inside any text you can use **bold**, \
                     *italic*, [label](https://example.com) and \\n for a line break; merge \
                     tags such as {{first_name}} pass through untouched. Lines starting with \
                     '#' are comments. Max 200 sections.",
                ),
        )
        .param(
            Param::string("subject")
                .default("")
                .describe(
                    "Text for the document <title> (what a browser tab or an email client's \
                     'view in browser' shows), e.g. 'March newsletter'. Blank (default) uses \
                     'Newsletter'.",
                ),
        )
        .param(
            Param::string("preheader")
                .default("")
                .describe(
                    "Hidden preview line shown after the subject in most inboxes, e.g. \
                     'Three new features and a discount inside'. Rendered as a hidden, \
                     zero-height div padded with zero-width characters so body copy does not \
                     bleed into the preview. Blank (default) omits it.",
                ),
        )
        .param(
            Param::integer("width")
                .default(600)
                .min(320.0)
                .max(900.0)
                .describe(
                    "Content width in pixels (320-900). 600 (default) is the standard \
                     newsletter width that fits every desktop client; the layout drops to \
                     100% width below this size.",
                ),
        )
        .param(
            Param::string("background")
                .default("#f4f4f5")
                .describe(
                    "Page background behind the content card, as a hex colour ('#f4f4f5', \
                     default) or a CSS colour name ('white'). Default '#f4f4f5'.",
                ),
        )
        .param(
            Param::string("content_background")
                .default("#ffffff")
                .describe(
                    "Background of the content card itself, as a hex colour or CSS colour \
                     name. Default '#ffffff'.",
                ),
        )
        .param(
            Param::string("text_color")
                .default("#1f2937")
                .describe(
                    "Body text colour, as a hex colour or CSS colour name. Default '#1f2937' \
                     (near-black).",
                ),
        )
        .param(
            Param::string("accent")
                .default("#2563eb")
                .describe(
                    "Accent colour used for button backgrounds and links, as a hex colour or \
                     CSS colour name. Default '#2563eb' (blue).",
                ),
        )
        .param(
            Param::enumv(
                "font",
                [
                    "system",
                    "arial",
                    "helvetica",
                    "verdana",
                    "tahoma",
                    "trebuchet",
                    "georgia",
                    "times",
                    "courier",
                ],
            )
            .default("system")
            .describe(
                "Which email-safe font stack to inline: 'system' (default, native UI font), \
                 'arial', 'helvetica', 'verdana', 'tahoma', 'trebuchet', 'georgia', 'times' \
                 or 'courier'. Web fonts are deliberately not offered — several major clients \
                 ignore them.",
            ),
        )
        .param(
            Param::boolean("dark_mode")
                .default(true)
                .describe(
                    "When true (default), add the 'color-scheme' meta tags and a \
                     'prefers-color-scheme: dark' block so the newsletter gets readable dark \
                     colours in clients that support it. Set false to always render the light \
                     palette.",
                ),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/newsletter-html-builder",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Build responsive, email-client-safe newsletter HTML from simple sections.",
    skill(
        description = "Build a complete, responsive, email-client-safe newsletter HTML document from a plain list of sections. Write one section per line as 'type | content | extra': heading, subheading, text, button (label + URL), image (URL + alt + optional link), columns (two columns that stack on mobile), divider, spacer (px), footer, or html for raw markup; **bold**, *italic*, [links](url) and \\n work inside text, and merge tags like {{first_name}} pass through. The output uses nested role=presentation tables with every style inlined, an Outlook ghost-table conditional comment, a hidden preheader line, a mobile media query and an optional prefers-color-scheme dark block. Content width, page/card/text/accent colours, the email-safe font stack, the document title and the preheader are all configurable.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "newsletter-html-builder", |a: Args| {
            gizza_ai_newsletter_html_builder_core::build(
                &a.sections,
                &a.subject,
                &a.preheader,
                a.width,
                &a.background,
                &a.content_background,
                &a.text_color,
                &a.accent,
                &a.font,
                a.dark_mode,
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
            r##"{
                "type": "object",
                "properties": {
                    "sections": { "type": "string", "description": "The newsletter content, ONE section per line, pipe-separated as 'type | content | extra'. Types: 'heading | Big title'; 'subheading | Smaller title'; 'text | A paragraph'; 'button | Read more | https://example.com'; 'image | https://example.com/hero.png | Alt text | https://example.com' (4th part is an optional click-through link); 'columns | Left copy | Right copy' (stacks on mobile); 'divider'; 'spacer | 24' (height in px); 'footer | Small print'; 'html | <p>raw markup</p>'. Inside any text you can use **bold**, *italic*, [label](https://example.com) and \\n for a line break; merge tags such as {{first_name}} pass through untouched. Lines starting with '#' are comments. Max 200 sections." },
                    "subject": { "type": "string", "default": "", "description": "Text for the document <title> (what a browser tab or an email client's 'view in browser' shows), e.g. 'March newsletter'. Blank (default) uses 'Newsletter'." },
                    "preheader": { "type": "string", "default": "", "description": "Hidden preview line shown after the subject in most inboxes, e.g. 'Three new features and a discount inside'. Rendered as a hidden, zero-height div padded with zero-width characters so body copy does not bleed into the preview. Blank (default) omits it." },
                    "width": { "type": "integer", "default": 600, "minimum": 320, "maximum": 900, "description": "Content width in pixels (320-900). 600 (default) is the standard newsletter width that fits every desktop client; the layout drops to 100% width below this size." },
                    "background": { "type": "string", "default": "#f4f4f5", "description": "Page background behind the content card, as a hex colour ('#f4f4f5', default) or a CSS colour name ('white'). Default '#f4f4f5'." },
                    "content_background": { "type": "string", "default": "#ffffff", "description": "Background of the content card itself, as a hex colour or CSS colour name. Default '#ffffff'." },
                    "text_color": { "type": "string", "default": "#1f2937", "description": "Body text colour, as a hex colour or CSS colour name. Default '#1f2937' (near-black)." },
                    "accent": { "type": "string", "default": "#2563eb", "description": "Accent colour used for button backgrounds and links, as a hex colour or CSS colour name. Default '#2563eb' (blue)." },
                    "font": { "type": "string", "enum": ["system", "arial", "helvetica", "verdana", "tahoma", "trebuchet", "georgia", "times", "courier"], "default": "system", "description": "Which email-safe font stack to inline: 'system' (default, native UI font), 'arial', 'helvetica', 'verdana', 'tahoma', 'trebuchet', 'georgia', 'times' or 'courier'. Web fonts are deliberately not offered — several major clients ignore them." },
                    "dark_mode": { "type": "boolean", "default": true, "description": "When true (default), add the 'color-scheme' meta tags and a 'prefers-color-scheme: dark' block so the newsletter gets readable dark colours in clients that support it. Set false to always render the light palette." }
                },
                "required": ["sections"],
                "additionalProperties": false
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
