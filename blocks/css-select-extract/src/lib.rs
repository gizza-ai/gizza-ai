//! gizza-ai/css-select-extract — fetch a page via wafer-run/network and
//! extract content matching a CSS selector (text / inner HTML / attribute).

// The #[wafer_block] macro emits wasm-only registration; supporting imports
// and the Args type are only used inside that impl.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use std::collections::HashMap;

use gizza_ai_block_utils::{Input, Param, SkillError, SkillResultExt, ToolDescriptor};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

/// Default cap on the number of matches returned when `limit` is omitted.
const DEFAULT_LIMIT: usize = 100;

/// Maximum bytes of HTML we'll fetch and parse. The runtime grants this block
/// a 64 MiB linear-memory cap (`GIZZA_MAX_WASM_MEMORY_PAGES`), so a generous
/// page-size cap is safe; this just guards against pathologically huge bodies.
const MAX_HTML_BYTES: usize = 8 << 20; // 8 MiB

/// What to pull out of each matched element.
pub mod core {
    use scraper::{Html, Selector};

    /// The three extraction modes the tool supports.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum ExtractKind {
        /// Concatenated text content of the element (and its descendants).
        Text,
        /// The element's inner HTML.
        Html,
        /// The value of a named attribute (elements lacking it are skipped).
        Attr,
    }

    impl ExtractKind {
        /// Parse the user-facing string form. `None` defaults to `Text`.
        pub fn parse(s: Option<&str>) -> Result<Self, String> {
            match s {
                None | Some("text") => Ok(Self::Text),
                Some("html") => Ok(Self::Html),
                Some("attr") => Ok(Self::Attr),
                Some(other) => Err(format!(
                    "invalid extract `{other}`: expected one of text, html, attr"
                )),
            }
        }
    }

    /// Run `selector` against `html` and extract a value from each match.
    ///
    /// - `Text` → the element's concatenated text content (descendant text
    ///   nodes joined with no separator, matching the document's whitespace).
    /// - `Html` → the element's inner HTML.
    /// - `Attr` → the value of the attribute named by `attr`; elements that
    ///   lack the attribute are skipped (not emitted as empty strings).
    ///
    /// At most `limit` matches are returned. An invalid CSS selector, or
    /// `Attr` without an attribute name, is a clear `Err`.
    pub fn extract(
        html: &str,
        selector: &str,
        kind: ExtractKind,
        attr: Option<&str>,
        limit: usize,
    ) -> Result<Vec<String>, String> {
        let sel = Selector::parse(selector)
            .map_err(|e| format!("invalid CSS selector `{selector}`: {e}"))?;

        // For attr extraction, resolve the (trimmed, non-empty) attribute name
        // up front so a bad `attr` arg is reported even on an empty page.
        let attr_name = if kind == ExtractKind::Attr {
            Some(
                attr.map(str::trim)
                    .filter(|s| !s.is_empty())
                    .ok_or_else(|| "`attr` is required when extract=\"attr\"".to_string())?,
            )
        } else {
            None
        };

        let doc = Html::parse_document(html);
        let mut out = Vec::new();
        for el in doc.select(&sel) {
            if out.len() >= limit {
                break;
            }
            match kind {
                ExtractKind::Text => {
                    out.push(el.text().collect::<String>());
                }
                ExtractKind::Html => {
                    out.push(el.inner_html());
                }
                ExtractKind::Attr => {
                    // attr_name is Some for ExtractKind::Attr (validated above).
                    let name = attr_name.expect("attr_name set for ExtractKind::Attr");
                    if let Some(v) = el.value().attr(name) {
                        out.push(v.to_string());
                    }
                    // Elements lacking the attribute are skipped.
                }
            }
        }
        Ok(out)
    }
}

#[derive(Deserialize)]
struct Args {
    url: String,
    selector: String,
    #[serde(default)]
    extract: Option<String>,
    #[serde(default)]
    attr: Option<String>,
    #[serde(default)]
    limit: Option<usize>,
}

#[derive(Serialize)]
struct ToolResp {
    url: String,
    selector: String,
    extract: String,
    count: usize,
    matches: Vec<String>,
}

/// Single-source param descriptor → chat schema (and CLI). See
/// docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md.
/// css-select-extract is `Input::None` — `url` is a normal required string param
/// (no `ref`), so there is no `url`⊕`ref` `oneOf`. Returning the flat `ToolResp`,
/// the handler stays thin (like web-fetch) rather than going through `run_skill`.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("url")
                .required()
                .describe("Absolute http or https URL of the page to fetch."),
        )
        .param(
            Param::string("selector")
                .required()
                .describe("CSS selector to match elements (e.g. \"h2.title\", \"a\", \"div#main p\")."),
        )
        .param(
            Param::enumv("extract", ["text", "html", "attr"]).describe(
                "What to extract from each matched element: \"text\" (default) = concatenated text content, \"html\" = inner HTML, \"attr\" = the value of the attribute named by \"attr\".",
            ),
        )
        .param(
            Param::string("attr").describe(
                "Attribute name to read when extract=\"attr\" (e.g. \"href\"). Required when extract=\"attr\"; elements lacking it are skipped.",
            ),
        )
        .param(
            Param::integer("limit")
                .min(1.0)
                .describe("Maximum number of matches to return (default 100)."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

/// Extract a `Content-Type` header value (case-insensitive name match,
/// first value in the multi-value list). Returns `None` if absent.
fn extract_content_type(headers: &HashMap<String, Vec<String>>) -> Option<String> {
    headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("content-type"))
        .and_then(|(_, vs)| vs.first().cloned())
}

#[cfg(target_arch = "wasm32")]
struct CssSelectExtract;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/css-select-extract",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Fetch a page and extract content matching a CSS selector",
    requires = ["wafer-run/network"],
    skill(
        description = "Fetch an http(s) page and extract content from every element matching a CSS selector. Returns a list of matches as their text content, inner HTML, or a named attribute's value.",
        parameters = schema_json()
    ),
)]
impl CssSelectExtract {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("css-select-extract")?;

    let kind =
        core::ExtractKind::parse(args.extract.as_deref()).map_err(SkillError::InvalidArgs)?;
    let limit = args.limit.unwrap_or(DEFAULT_LIMIT).max(1);

    let resp = wafer_sdk::clients::network::do_request("GET", &args.url, &HashMap::new(), None)?;
    if resp.status_code >= 400 {
        return Err(SkillError::HttpStatus {
            status: resp.status_code,
            url: args.url,
        });
    }
    if resp.body.len() > MAX_HTML_BYTES {
        return Err(SkillError::TooLarge {
            kind: "input page",
            bytes: resp.body.len(),
            cap: MAX_HTML_BYTES,
        });
    }
    // Surface the content-type only to keep parity with web-fetch's diagnostics;
    // we parse whatever we get as HTML regardless (html5ever is lenient).
    let _content_type = extract_content_type(&resp.headers);
    let html = String::from_utf8_lossy(&resp.body);

    let matches = core::extract(&html, &args.selector, kind, args.attr.as_deref(), limit)
        .map_err(SkillError::InvalidArgs)?;

    let tool = ToolResp {
        url: args.url,
        selector: args.selector,
        extract: match kind {
            core::ExtractKind::Text => "text",
            core::ExtractKind::Html => "html",
            core::ExtractKind::Attr => "attr",
        }
        .to_string(),
        count: matches.len(),
        matches,
    };
    serde_json::to_vec(&tool)
        .map_err(|e| SkillError::Serialize(format!("serialize tool response: {e}")))
}

#[cfg(test)]
mod tests {
    use super::{
        core::{extract, ExtractKind},
        schema_json,
    };

    /// Migration safety: the descriptor-derived chat schema must match the
    /// pre-retrofit authored schema, so the LLM sees no drift. (to_schema_json
    /// now emits `additionalProperties: false`, which css-select-extract's
    /// authored schema already had; the descriptor renders `limit`'s minimum as
    /// the JSON number `1`, same as the authored schema.)
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "url":      { "type": "string", "description": "Absolute http or https URL of the page to fetch." },
                    "selector": { "type": "string", "description": "CSS selector to match elements (e.g. \"h2.title\", \"a\", \"div#main p\")." },
                    "extract":  { "type": "string", "enum": ["text", "html", "attr"], "description": "What to extract from each matched element: \"text\" (default) = concatenated text content, \"html\" = inner HTML, \"attr\" = the value of the attribute named by \"attr\"." },
                    "attr":     { "type": "string", "description": "Attribute name to read when extract=\"attr\" (e.g. \"href\"). Required when extract=\"attr\"; elements lacking it are skipped." },
                    "limit":    { "type": "integer", "minimum": 1, "description": "Maximum number of matches to return (default 100)." }
                },
                "required": ["url", "selector"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    const FIXTURE: &str = r#"
        <html><body>
          <h2 class="title">First Heading</h2>
          <h2 class="title">Second <em>Heading</em></h2>
          <p>not a title</p>
          <a href="https://example.com/a">Link A</a>
          <a>no href here</a>
          <a href="https://example.com/b">Link B</a>
        </body></html>
    "#;

    #[test]
    fn extracts_text_from_fixture() {
        let got = extract(FIXTURE, "h2.title", ExtractKind::Text, None, 100).unwrap();
        assert_eq!(got, vec!["First Heading", "Second Heading"]);
    }

    #[test]
    fn extracts_inner_html() {
        let got = extract(FIXTURE, "h2.title", ExtractKind::Html, None, 100).unwrap();
        assert_eq!(got[0], "First Heading");
        assert_eq!(got[1], "Second <em>Heading</em>");
    }

    #[test]
    fn extracts_attr_and_skips_missing() {
        // Three <a> elements but only two carry href → the bare <a> is skipped.
        let got = extract(FIXTURE, "a", ExtractKind::Attr, Some("href"), 100).unwrap();
        assert_eq!(got, vec!["https://example.com/a", "https://example.com/b"]);
    }

    #[test]
    fn invalid_selector_is_an_error() {
        let err = extract(FIXTURE, "h2..bad[", ExtractKind::Text, None, 100).unwrap_err();
        assert!(err.contains("invalid CSS selector"), "got: {err}");
    }

    #[test]
    fn attr_without_name_is_an_error() {
        let err = extract(FIXTURE, "a", ExtractKind::Attr, None, 100).unwrap_err();
        assert!(err.contains("`attr` is required"), "got: {err}");
        let err = extract(FIXTURE, "a", ExtractKind::Attr, Some("  "), 100).unwrap_err();
        assert!(err.contains("`attr` is required"), "got: {err}");
    }

    #[test]
    fn limit_caps_matches() {
        let got = extract(FIXTURE, "a", ExtractKind::Attr, Some("href"), 1).unwrap();
        assert_eq!(got, vec!["https://example.com/a"]);
    }

    #[test]
    fn no_matches_returns_empty() {
        let got = extract(FIXTURE, "h3", ExtractKind::Text, None, 100).unwrap();
        assert!(got.is_empty());
    }

    #[test]
    fn extract_kind_parse_defaults_and_rejects() {
        assert_eq!(ExtractKind::parse(None).unwrap(), ExtractKind::Text);
        assert_eq!(ExtractKind::parse(Some("text")).unwrap(), ExtractKind::Text);
        assert_eq!(ExtractKind::parse(Some("html")).unwrap(), ExtractKind::Html);
        assert_eq!(ExtractKind::parse(Some("attr")).unwrap(), ExtractKind::Attr);
        assert!(ExtractKind::parse(Some("bogus")).is_err());
    }
}
