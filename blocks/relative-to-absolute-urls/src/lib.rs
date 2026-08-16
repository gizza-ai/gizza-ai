//! gizza-ai/relative-to-absolute-urls — chat skill block on the shared tool
//! abstraction. Rewrites the relative URL attribute values in a chunk of HTML to
//! absolute URLs against a base, using a comment- and raw-text-aware scanner so
//! only the URLs change. The chat schema is single-sourced from `descriptor()`
//! (which also drives the CLI); `handle()` delegates to `block_utils::run_skill`.
//! Pure compute — the markup is scanned in the sandbox, nothing is uploaded.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    html: String,
    base: String,
    #[serde(default)]
    attributes: String,
    #[serde(default = "default_true")]
    use_base_tag: bool,
    #[serde(default)]
    protocol_relative: String,
    #[serde(default)]
    resolve_fragments: bool,
    #[serde(default)]
    style_urls: bool,
    #[serde(default)]
    output: String,
}

fn default_true() -> bool {
    true
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("html")
                .required()
                .describe("The markup to rewrite, as text. Only URL attribute VALUES change — no whitespace collapsing, no tag or attribute reordering, no re-indentation, no quote-style changes, so the diff shows the URLs and nothing else. The scanner is comment- and raw-text-aware: a link written inside <!-- … -->, <script>, <textarea> or <title> is text and is left alone, and a '>' inside a quoted attribute value does not end a tag. Max 5,000,000 bytes."),
        )
        .param(
            Param::string("base")
                .required()
                .describe("The absolute URL the relative values are relative to — normally the address of the page the markup came from, e.g. 'https://example.com/blog/post.html'. Resolution follows the WHATWG rules the browser address bar uses, so '../x', '/x', './x', '?q=1' and a trailing-slash directory base ('https://example.com/blog/') all behave exactly as they would in that page. Must be an absolute hierarchical URL; a relative value or a 'mailto:'-style address is rejected."),
        )
        .param(
            Param::enumv("attributes", ["href-src", "common", "all"])
                .default("common")
                .describe("Which attributes count as URLs. 'href-src' is the conservative pass — href and src only. 'common' (default) adds the everyday rest: srcset (each candidate, descriptors preserved), poster, form action, formaction, object data, the background attributes, ping and the URL inside <meta http-equiv=\"refresh\" content=\"5; url=…\">. 'all' adds the rarities HTML also defines as URLs: cite, longdesc, manifest, profile, itemtype, icon and the applet/object archive, code, codebase and object attributes."),
        )
        .param(
            Param::boolean("use_base_tag")
                .default(true)
                .describe("Honour a <base href> in the document. On by default because that is what a browser does: when a page carries one, its relative URLs resolve against the <base>, not against the page's own address, so ignoring it produces URLs that point somewhere else. The <base href> itself is still resolved against the base you supplied. Turn it off to resolve everything against your base regardless."),
        )
        .param(
            Param::enumv("protocol_relative", ["resolve", "keep"])
                .default("resolve")
                .describe("What to do with protocol-relative values such as '//cdn.example.com/a.png'. 'resolve' (default) gives them the base URL's scheme, which is what you want when the markup is heading somewhere with no scheme of its own — an email, a feed, a PDF. 'keep' leaves them exactly as written, which is right when the output is still going to be served over both http and https."),
        )
        .param(
            Param::boolean("resolve_fragments")
                .default(false)
                .describe("Also make bare in-page anchors such as '#top' absolute ('https://example.com/blog/post.html#top'). Off by default, because inside the original page a bare fragment is a jump link and absolutizing it turns a scroll into a page load. Turn it on when the markup is being lifted OUT of its page — into an email, an RSS item or an embed — where a bare '#top' would resolve against the wrong document."),
        )
        .param(
            Param::boolean("style_urls")
                .default(false)
                .describe("Also rewrite CSS URLs — 'url(…)' and '@import \"…\"' inside style attributes and <style> blocks. Off by default so a run is purely an attribute operation. The CSS pass keeps the original quoting and spacing and only touches the value inside the parentheses; external stylesheets are not fetched, so URLs in a linked .css file are out of reach."),
        )
        .param(
            Param::enumv("output", ["html", "report", "urls"])
                .default("html")
                .describe("What to return: 'html' (default) is the rewritten markup; 'report' is a metric,value CSV with the base actually used, whether a <base> tag was honoured, and counts of what was rewritten versus kept (absolute, other scheme, fragment, protocol-relative, template, empty, unresolvable) plus bytes before/after; 'urls' is a line,tag,attribute,original,resolved,action CSV listing every URL the scanner looked at and what it decided — the dry run for checking a base before trusting it on a whole document."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/relative-to-absolute-urls",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Rewrite relative href, src and other URL attributes in HTML to absolute URLs against a base.",
    skill(
        description = "Rewrite the relative URLs in a chunk of HTML to absolute ones against a base URL, changing only the URL attribute values — no whitespace collapsing, no tag or attribute reordering, no re-indentation, so the diff shows the URLs and nothing else. Resolution is the WHATWG algorithm a browser applies to a relative href, so '../x', '/x', './x', '?q=1' and dot segments behave the same way. The scanner is comment- and raw-text-aware: links inside <!-- … -->, <script>, <textarea> and <title> are text and are left alone, and a '>' inside a quoted attribute cannot end a tag early. attributes selects the net: 'href-src' (href and src only), 'common' (default — adds srcset with its descriptors, poster, action, formaction, object data, background, ping and <meta http-equiv=refresh> content) or 'all' (adds cite, longdesc, manifest, profile, itemtype, icon and the applet/object archive/code/codebase attributes). Values that are already absolute, carry another scheme (mailto:, tel:, data:, javascript:), are bare '#fragments' or are template placeholders ({{ … }}, <% … %>) are left exactly as written. use_base_tag honours a document's own <base href> the way a browser does (default on); protocol_relative decides whether '//cdn.example.com/a.png' takes the base's scheme; resolve_fragments absolutizes bare anchors for markup being lifted into an email or feed; style_urls extends the pass to url(…) and @import in style attributes and <style> blocks. output is 'html' (the rewritten markup), 'report' (a metric,value CSV of counts and the base actually used) or 'urls' (a line,tag,attribute,original,resolved,action CSV dry run). Max 5,000,000 bytes. Runs entirely in the sandbox; nothing is fetched or uploaded.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "relative-to-absolute-urls", |a: Args| {
            gizza_ai_relative_to_absolute_urls_core::absolutize(
                &a.html,
                &a.base,
                &a.attributes,
                a.use_base_tag,
                &a.protocol_relative,
                a.resolve_fragments,
                a.style_urls,
                &a.output,
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
    /// schema, so any future change to the LLM-facing API is intentional and
    /// reviewed. Authored 2026-08-16 for the initial relative-to-absolute-urls release.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "html": { "type": "string", "description": "The markup to rewrite, as text. Only URL attribute VALUES change — no whitespace collapsing, no tag or attribute reordering, no re-indentation, no quote-style changes, so the diff shows the URLs and nothing else. The scanner is comment- and raw-text-aware: a link written inside <!-- … -->, <script>, <textarea> or <title> is text and is left alone, and a '>' inside a quoted attribute value does not end a tag. Max 5,000,000 bytes." },
                    "base": { "type": "string", "description": "The absolute URL the relative values are relative to — normally the address of the page the markup came from, e.g. 'https://example.com/blog/post.html'. Resolution follows the WHATWG rules the browser address bar uses, so '../x', '/x', './x', '?q=1' and a trailing-slash directory base ('https://example.com/blog/') all behave exactly as they would in that page. Must be an absolute hierarchical URL; a relative value or a 'mailto:'-style address is rejected." },
                    "attributes": { "type": "string", "enum": ["href-src", "common", "all"], "default": "common", "description": "Which attributes count as URLs. 'href-src' is the conservative pass — href and src only. 'common' (default) adds the everyday rest: srcset (each candidate, descriptors preserved), poster, form action, formaction, object data, the background attributes, ping and the URL inside <meta http-equiv=\"refresh\" content=\"5; url=…\">. 'all' adds the rarities HTML also defines as URLs: cite, longdesc, manifest, profile, itemtype, icon and the applet/object archive, code, codebase and object attributes." },
                    "use_base_tag": { "type": "boolean", "default": true, "description": "Honour a <base href> in the document. On by default because that is what a browser does: when a page carries one, its relative URLs resolve against the <base>, not against the page's own address, so ignoring it produces URLs that point somewhere else. The <base href> itself is still resolved against the base you supplied. Turn it off to resolve everything against your base regardless." },
                    "protocol_relative": { "type": "string", "enum": ["resolve", "keep"], "default": "resolve", "description": "What to do with protocol-relative values such as '//cdn.example.com/a.png'. 'resolve' (default) gives them the base URL's scheme, which is what you want when the markup is heading somewhere with no scheme of its own — an email, a feed, a PDF. 'keep' leaves them exactly as written, which is right when the output is still going to be served over both http and https." },
                    "resolve_fragments": { "type": "boolean", "default": false, "description": "Also make bare in-page anchors such as '#top' absolute ('https://example.com/blog/post.html#top'). Off by default, because inside the original page a bare fragment is a jump link and absolutizing it turns a scroll into a page load. Turn it on when the markup is being lifted OUT of its page — into an email, an RSS item or an embed — where a bare '#top' would resolve against the wrong document." },
                    "style_urls": { "type": "boolean", "default": false, "description": "Also rewrite CSS URLs — 'url(…)' and '@import \"…\"' inside style attributes and <style> blocks. Off by default so a run is purely an attribute operation. The CSS pass keeps the original quoting and spacing and only touches the value inside the parentheses; external stylesheets are not fetched, so URLs in a linked .css file are out of reach." },
                    "output": { "type": "string", "enum": ["html", "report", "urls"], "default": "html", "description": "What to return: 'html' (default) is the rewritten markup; 'report' is a metric,value CSV with the base actually used, whether a <base> tag was honoured, and counts of what was rewritten versus kept (absolute, other scheme, fragment, protocol-relative, template, empty, unresolvable) plus bytes before/after; 'urls' is a line,tag,attribute,original,resolved,action CSV listing every URL the scanner looked at and what it decided — the dry run for checking a base before trusting it on a whole document." }
                },
                "required": ["html", "base"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
