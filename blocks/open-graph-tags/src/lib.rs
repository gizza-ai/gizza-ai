//! gizza-ai/open-graph-tags — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. All work happens inside
//! the WASM sandbox — no host calls, no network.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_open_graph_tags_core as core;
use serde::Deserialize;
use wafer_sdk::*;

/// Booleans that default to ON must deserialize to `true` when the caller omits
/// them, so each gets its own `#[serde(default = …)]` seed.
fn yes() -> bool {
    true
}

#[derive(Deserialize)]
struct Args {
    title: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    image: String,
    #[serde(default)]
    image_alt: String,
    #[serde(default)]
    image_width: u32,
    #[serde(default)]
    image_height: u32,
    #[serde(default)]
    site_name: String,
    #[serde(default)]
    og_type: String,
    #[serde(default)]
    twitter_card: String,
    #[serde(default)]
    twitter_site: String,
    #[serde(default)]
    twitter_creator: String,
    #[serde(default)]
    locale: String,
    #[serde(default)]
    author: String,
    #[serde(default = "yes")]
    include_basic: bool,
    #[serde(default = "yes")]
    include_twitter: bool,
    #[serde(default)]
    include_schema: bool,
    #[serde(default = "yes")]
    group_comments: bool,
    #[serde(default = "yes")]
    warnings: bool,
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("title")
                .required()
                .describe("The page title, used for <title>, og:title and twitter:title. Keep it to about 60 characters so previews don't truncate it. Example: 'How to bake sourdough'."),
        )
        .param(
            Param::string("description")
                .describe("The one- or two-sentence page summary, used for the meta description, og:description and twitter:description. About 50-160 characters reads best. Omit to leave those tags out."),
        )
        .param(
            Param::string("url")
                .describe("The page's absolute canonical URL, used for og:url and <link rel=\"canonical\">, e.g. 'https://example.com/sourdough'. Relative paths are not resolved by crawlers. Omit to leave those tags out."),
        )
        .param(
            Param::string("image")
                .describe("Absolute URL of the preview image, used for og:image and twitter:image, e.g. 'https://example.com/og/sourdough.png'. 1200x630 pixels suits the 1.91:1 crop Facebook and LinkedIn use. Omit for a text-only card."),
        )
        .param(
            Param::string("image_alt")
                .describe("Alt text describing the preview image, used for og:image:alt and twitter:image:alt, e.g. 'A sliced sourdough loaf'. Only emitted when image is set."),
        )
        .param(
            Param::integer("image_width")
                .default(0)
                .min(0.0)
                .max(core::MAX_IMAGE_DIMENSION as f64)
                .describe("Preview image width in pixels for og:image:width, e.g. 1200. Set it together with image_height so crawlers can reserve layout space before fetching. 0 (default) omits the tag."),
        )
        .param(
            Param::integer("image_height")
                .default(0)
                .min(0.0)
                .max(core::MAX_IMAGE_DIMENSION as f64)
                .describe("Preview image height in pixels for og:image:height, e.g. 630. Set it together with image_width. 0 (default) omits the tag."),
        )
        .param(
            Param::string("site_name")
                .describe("The site's name for og:site_name, shown above the card as the source, e.g. 'Example Bakery'. Omit to leave the tag out."),
        )
        .param(
            Param::enumv("og_type", core::OG_TYPES)
                .default("website")
                .describe("The og:type object type. 'website' (default) for ordinary pages; 'article' for posts (also emits article:author when author is set); 'product', 'profile' (also emits profile:username), 'video.other', 'music.song' or 'book'."),
        )
        .param(
            Param::enumv("twitter_card", core::TWITTER_CARDS)
                .default("summary_large_image")
                .describe("The twitter:card layout on X. 'summary_large_image' (default) is the wide image card; 'summary' is a small square thumbnail; 'player' embeds a media player; 'app' promotes a mobile app."),
        )
        .param(
            Param::string("twitter_site")
                .describe("The site's X handle for twitter:site, e.g. '@examplebakery'. A bare name or a profile URL is accepted and normalized to the @ form."),
        )
        .param(
            Param::string("twitter_creator")
                .describe("The author's X handle for twitter:creator, e.g. '@some_baker'. A bare name or a profile URL is accepted and normalized to the @ form."),
        )
        .param(
            Param::string("locale")
                .default("en_US")
                .describe("The content's locale for og:locale in language_TERRITORY form, e.g. 'en_US' (default), 'en_GB', 'de_DE', 'ja_JP', 'pt_BR'. Set it empty to omit the tag."),
        )
        .param(
            Param::string("author")
                .describe("The author's name, used for <meta name=\"author\">, plus article:author when og_type='article' and profile:username when og_type='profile'. Example: 'Dana Ruiz'."),
        )
        .param(
            Param::boolean("include_basic")
                .default(true)
                .describe("Include the standard <title>, meta description, meta author and <link rel=\"canonical\"> tags (default true). Set false when the page already has them and you only want the social tags."),
        )
        .param(
            Param::boolean("include_twitter")
                .default(true)
                .describe("Include the twitter:* Twitter Card tags for X (default true). Set false to emit Open Graph only — X does fall back to og:title/og:description/og:image."),
        )
        .param(
            Param::boolean("include_schema")
                .default(false)
                .describe("Also emit schema.org itemprop tags (name, description, image) alongside the others. Default false."),
        )
        .param(
            Param::boolean("group_comments")
                .default(true)
                .describe("Label each block with an HTML comment header such as '<!-- Open Graph -->' (default true). Set false for bare tags with no comments."),
        )
        .param(
            Param::boolean("warnings")
                .default(true)
                .describe("Append a trailing '<!-- Checks -->' HTML comment listing advisory issues — title/description length, relative or missing URLs, a large-image card with no image (default true). Checks never block generation; set false to omit the block."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/open-graph-tags",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate Open Graph and Twitter Card meta tags for rich link previews.",
    skill(
        description = "Generate the <head> meta tags that make a page render as a rich preview card when it is shared on Facebook, X, LinkedIn, Slack or Discord. Pass at least `title`; add `description`, `url` (absolute), `image` (absolute, 1200x630 suits the 1.91:1 crop), `image_alt`, `site_name`, `author`, `locale` and the X handles `twitter_site`/`twitter_creator` to fill out the card. `og_type` picks the Open Graph object type (website/article/product/profile/video.other/music.song/book) and `twitter_card` picks the X layout (summary_large_image/summary/player/app). Toggle the emitted blocks with include_basic, include_twitter, include_schema and group_comments. With warnings=true (default) a trailing '<!-- Checks -->' comment lists advisory issues such as an over-long title or a relative image URL. Values are HTML-escaped; the result is a copy-pasteable block of markup for the page's <head>.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "open-graph-tags", |a: Args| {
            core::generate(&core::Options {
                title: a.title,
                description: a.description,
                url: a.url,
                image: a.image,
                image_alt: a.image_alt,
                image_width: a.image_width,
                image_height: a.image_height,
                site_name: a.site_name,
                og_type: a.og_type,
                twitter_card: a.twitter_card,
                twitter_site: a.twitter_site,
                twitter_creator: a.twitter_creator,
                locale: a.locale,
                author: a.author,
                include_basic: a.include_basic,
                include_twitter: a.include_twitter,
                include_schema: a.include_schema,
                group_comments: a.group_comments,
                warnings: a.warnings,
            })
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
    /// schema, so any future change to the LLM-facing API is intentional and
    /// reviewed. Authored 2026-09-04 with the tool.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "title": { "type": "string", "description": "The page title, used for <title>, og:title and twitter:title. Keep it to about 60 characters so previews don't truncate it. Example: 'How to bake sourdough'." },
                    "description": { "type": "string", "description": "The one- or two-sentence page summary, used for the meta description, og:description and twitter:description. About 50-160 characters reads best. Omit to leave those tags out." },
                    "url": { "type": "string", "description": "The page's absolute canonical URL, used for og:url and <link rel=\"canonical\">, e.g. 'https://example.com/sourdough'. Relative paths are not resolved by crawlers. Omit to leave those tags out." },
                    "image": { "type": "string", "description": "Absolute URL of the preview image, used for og:image and twitter:image, e.g. 'https://example.com/og/sourdough.png'. 1200x630 pixels suits the 1.91:1 crop Facebook and LinkedIn use. Omit for a text-only card." },
                    "image_alt": { "type": "string", "description": "Alt text describing the preview image, used for og:image:alt and twitter:image:alt, e.g. 'A sliced sourdough loaf'. Only emitted when image is set." },
                    "image_width": { "type": "integer", "minimum": 0, "maximum": 10000, "default": 0, "description": "Preview image width in pixels for og:image:width, e.g. 1200. Set it together with image_height so crawlers can reserve layout space before fetching. 0 (default) omits the tag." },
                    "image_height": { "type": "integer", "minimum": 0, "maximum": 10000, "default": 0, "description": "Preview image height in pixels for og:image:height, e.g. 630. Set it together with image_width. 0 (default) omits the tag." },
                    "site_name": { "type": "string", "description": "The site's name for og:site_name, shown above the card as the source, e.g. 'Example Bakery'. Omit to leave the tag out." },
                    "og_type": { "type": "string", "enum": ["website", "article", "product", "profile", "video.other", "music.song", "book"], "default": "website", "description": "The og:type object type. 'website' (default) for ordinary pages; 'article' for posts (also emits article:author when author is set); 'product', 'profile' (also emits profile:username), 'video.other', 'music.song' or 'book'." },
                    "twitter_card": { "type": "string", "enum": ["summary_large_image", "summary", "player", "app"], "default": "summary_large_image", "description": "The twitter:card layout on X. 'summary_large_image' (default) is the wide image card; 'summary' is a small square thumbnail; 'player' embeds a media player; 'app' promotes a mobile app." },
                    "twitter_site": { "type": "string", "description": "The site's X handle for twitter:site, e.g. '@examplebakery'. A bare name or a profile URL is accepted and normalized to the @ form." },
                    "twitter_creator": { "type": "string", "description": "The author's X handle for twitter:creator, e.g. '@some_baker'. A bare name or a profile URL is accepted and normalized to the @ form." },
                    "locale": { "type": "string", "default": "en_US", "description": "The content's locale for og:locale in language_TERRITORY form, e.g. 'en_US' (default), 'en_GB', 'de_DE', 'ja_JP', 'pt_BR'. Set it empty to omit the tag." },
                    "author": { "type": "string", "description": "The author's name, used for <meta name=\"author\">, plus article:author when og_type='article' and profile:username when og_type='profile'. Example: 'Dana Ruiz'." },
                    "include_basic": { "type": "boolean", "default": true, "description": "Include the standard <title>, meta description, meta author and <link rel=\"canonical\"> tags (default true). Set false when the page already has them and you only want the social tags." },
                    "include_twitter": { "type": "boolean", "default": true, "description": "Include the twitter:* Twitter Card tags for X (default true). Set false to emit Open Graph only — X does fall back to og:title/og:description/og:image." },
                    "include_schema": { "type": "boolean", "default": false, "description": "Also emit schema.org itemprop tags (name, description, image) alongside the others. Default false." },
                    "group_comments": { "type": "boolean", "default": true, "description": "Label each block with an HTML comment header such as '<!-- Open Graph -->' (default true). Set false for bare tags with no comments." },
                    "warnings": { "type": "boolean", "default": true, "description": "Append a trailing '<!-- Checks -->' HTML comment listing advisory issues — title/description length, relative or missing URLs, a large-image card with no image (default true). Checks never block generation; set false to omit the block." }
                },
                "required": ["title"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
