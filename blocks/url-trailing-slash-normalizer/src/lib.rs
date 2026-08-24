//! gizza-ai/url-trailing-slash-normalizer — chat skill block on the shared tool
//! abstraction. The chat schema is single-sourced from descriptor() (which also
//! drives the CLI + the page query-params); handle() delegates to
//! block_utils::run_skill. No host calls — runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_url_trailing_slash_normalizer_core::normalize;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    urls: String,
    #[serde(default)]
    mode: String,
    #[serde(default = "yes")]
    skip_file_paths: bool,
    #[serde(default = "yes")]
    normalize_root: bool,
    #[serde(default)]
    dedupe: bool,
    #[serde(default)]
    on_invalid: String,
    #[serde(default)]
    output: String,
}

fn yes() -> bool {
    true
}

/// Single-source param descriptor → chat schema (and CLI + page query-params).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("urls")
                .required()
                .describe("The URLs to normalize, one per line — e.g. 'https://example.com/blog'. Absolute URLs ('https://host/path', any scheme with an authority), scheme-relative URLs ('//cdn.example.com/a'), bare hosts ('example.com/blog', 'example.com:8080/blog') and path-only lines ('/blog/post') are all accepted. Blank lines are ignored. Max 20,000 URLs and 1,000,000 bytes per run."),
        )
        .param(
            Param::enumv("mode", ["add", "remove"])
                .default("add")
                .describe("Which style to enforce: 'add' (default) puts a trailing slash on every directory-style URL, 'remove' takes it off. Only the path changes — the scheme, host, port, query string and fragment are copied through byte-for-byte and nothing is re-encoded."),
        )
        .param(
            Param::boolean("skip_file_paths")
                .default(true)
                .describe("Leave file-like URLs alone in both directions — a last path segment with a real extension such as '/sitemap.xml', '/report.pdf' or '/style.css'. On by default because '/sitemap.xml/' is a different resource on almost every server. An extension must be 1-10 alphanumeric characters containing at least one letter, so '/api/v1.2' counts as a directory. Turn it off to force every URL into the chosen style."),
        )
        .param(
            Param::boolean("normalize_root")
                .default(true)
                .describe("Always render a site root as a single '/' — 'https://example.com' and 'https://example.com//' both become 'https://example.com/', and remove mode never strips the root slash (a bare 'https://example.com' is not a shorter URL, it is an incomplete one). Turn it off to leave root URLs exactly as written."),
        )
        .param(
            Param::boolean("dedupe")
                .default(false)
                .describe("Drop URLs that normalize to something an earlier line already produced, keeping the first occurrence and the original order. Useful when the same page appears in a list both with and without its slash. Default false."),
        )
        .param(
            Param::enumv("on_invalid", ["keep", "drop", "error"])
                .default("keep")
                .describe("What to do with a line that is not a URL or path — a note, a 'mailto:'/'tel:' address, a stray word. 'keep' (default) passes it through untouched so an annotated list survives a round trip, 'drop' leaves it out of the result, 'error' fails the run and names the line."),
        )
        .param(
            Param::enumv("output", ["urls", "changed", "report", "summary"])
                .default("urls")
                .describe("What to return: 'urls' (default) is every URL normalized, one per line; 'changed' is only the URLs whose trailing slash actually moved (the redirect list to hand to your server config); 'report' is a line,original,normalized,action CSV covering every input line, where action is added, removed, unchanged, root, skipped-file, invalid or duplicate; 'summary' is a metric,value CSV of the run totals."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/url-trailing-slash-normalizer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Add or remove trailing slashes across a batch of URLs, leaving file-like paths alone.",
    skill(
        description = "Make the trailing slashes in a list of URLs consistent: add one to every directory-style URL, or strip it from all of them. Takes one URL per line (absolute, scheme-relative, bare host or path-only) and rewrites only the path — scheme, host, port, query string and fragment are copied byte-for-byte, nothing is re-encoded, and repeated trailing slashes collapse to the chosen style. File-like paths such as /sitemap.xml or /report.pdf are left alone by default, the site root always stays a single '/', and unrecognized lines are passed through. Optional dedupe, and four outputs: the normalized list, only the URLs that changed, a per-line CSV report, or a CSV summary of the totals.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "url-trailing-slash-normalizer", |a: Args| {
            normalize(
                &a.urls,
                &a.mode,
                a.skip_file_paths,
                a.normalize_root,
                a.dedupe,
                &a.on_invalid,
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
    /// reviewed.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "urls": { "type": "string", "description": "The URLs to normalize, one per line — e.g. 'https://example.com/blog'. Absolute URLs ('https://host/path', any scheme with an authority), scheme-relative URLs ('//cdn.example.com/a'), bare hosts ('example.com/blog', 'example.com:8080/blog') and path-only lines ('/blog/post') are all accepted. Blank lines are ignored. Max 20,000 URLs and 1,000,000 bytes per run." },
                    "mode": { "type": "string", "enum": ["add", "remove"], "default": "add", "description": "Which style to enforce: 'add' (default) puts a trailing slash on every directory-style URL, 'remove' takes it off. Only the path changes — the scheme, host, port, query string and fragment are copied through byte-for-byte and nothing is re-encoded." },
                    "skip_file_paths": { "type": "boolean", "default": true, "description": "Leave file-like URLs alone in both directions — a last path segment with a real extension such as '/sitemap.xml', '/report.pdf' or '/style.css'. On by default because '/sitemap.xml/' is a different resource on almost every server. An extension must be 1-10 alphanumeric characters containing at least one letter, so '/api/v1.2' counts as a directory. Turn it off to force every URL into the chosen style." },
                    "normalize_root": { "type": "boolean", "default": true, "description": "Always render a site root as a single '/' — 'https://example.com' and 'https://example.com//' both become 'https://example.com/', and remove mode never strips the root slash (a bare 'https://example.com' is not a shorter URL, it is an incomplete one). Turn it off to leave root URLs exactly as written." },
                    "dedupe": { "type": "boolean", "default": false, "description": "Drop URLs that normalize to something an earlier line already produced, keeping the first occurrence and the original order. Useful when the same page appears in a list both with and without its slash. Default false." },
                    "on_invalid": { "type": "string", "enum": ["keep", "drop", "error"], "default": "keep", "description": "What to do with a line that is not a URL or path — a note, a 'mailto:'/'tel:' address, a stray word. 'keep' (default) passes it through untouched so an annotated list survives a round trip, 'drop' leaves it out of the result, 'error' fails the run and names the line." },
                    "output": { "type": "string", "enum": ["urls", "changed", "report", "summary"], "default": "urls", "description": "What to return: 'urls' (default) is every URL normalized, one per line; 'changed' is only the URLs whose trailing slash actually moved (the redirect list to hand to your server config); 'report' is a line,original,normalized,action CSV covering every input line, where action is added, removed, unchanged, root, skipped-file, invalid or duplicate; 'summary' is a metric,value CSV of the run totals." }
                },
                "required": ["urls"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
