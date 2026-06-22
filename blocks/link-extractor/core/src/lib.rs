//! gizza-ai/link-extractor core — pull every hyperlink and in-page anchor
//! (jump target) out of HTML or Markdown. No wafer/wasm-bindgen deps.
//!
//! - **HTML** is parsed with `scraper` (html5ever, wasm32-safe): `<a href>`,
//!   plus `<area href>`, give the hyperlinks (text = the element's text); any
//!   element carrying an `id` (or an `<a name>`) is an anchor / jump target.
//! - **Markdown** is parsed with `pulldown-cmark` (CommonMark, wasm32-safe):
//!   inline + reference links and autolinks give the hyperlinks (text = the
//!   link's text); explicit HTML `id=`/`name=` anchors inside the Markdown are
//!   also collected. A heading produces a GitHub-style slug anchor.
//!
//! Output is a `Report` (counts + `links` + `anchors`) serialized to JSON, or a
//! human-readable text rendering for the page. A `base_url` resolves relative
//! links to absolute, and `dedup` collapses repeated destinations.

use scraper::{Html, Selector};
use serde::Serialize;
use url::Url;

/// Which parser to use for the input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Auto,
    Html,
    Markdown,
}

impl Source {
    pub fn parse(s: &str) -> Result<Source, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Ok(Source::Auto),
            "html" => Ok(Source::Html),
            "markdown" | "md" => Ok(Source::Markdown),
            other => Err(format!("source {other:?} not supported (auto|html|markdown)")),
        }
    }
}

/// Output rendering format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output {
    Text,
    Json,
}

impl Output {
    pub fn parse(s: &str) -> Result<Output, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "text" => Ok(Output::Text),
            "json" => Ok(Output::Json),
            other => Err(format!("output {other:?} not supported (text|json)")),
        }
    }
}

/// Options controlling extraction.
#[derive(Debug, Clone, Default)]
pub struct Options {
    /// Absolute base URL to resolve relative links against (e.g.
    /// "https://example.com/docs/"). When set, each relative link's `url` is
    /// rewritten to the resolved absolute form (and its `relative` flag cleared).
    pub base_url: Option<String>,
    /// When true, collapse repeated links (same final `url`) keeping first-seen.
    pub dedup: bool,
}

/// One extracted hyperlink.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Link {
    /// The href / destination URL. Verbatim as written, unless a `base_url` was
    /// supplied and the link was relative — then the resolved absolute URL.
    pub url: String,
    /// The visible link text (whitespace-collapsed). May be empty.
    pub text: String,
    /// True when `url` is a relative reference (no scheme, not protocol-relative,
    /// not a fragment). Cleared once resolved against a `base_url`.
    pub relative: bool,
    /// The `rel` attribute value (e.g. "nofollow", "noopener sponsored"), present
    /// only for HTML `<a rel="…">`. Empty/absent → omitted from JSON.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub rel: String,
}

/// One in-page anchor (a jump target the input defines).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Anchor {
    /// The anchor name (value of `id`/`name`, or a heading slug). A
    /// same-document link `#name` jumps here.
    pub name: String,
}

/// The full extraction result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Report {
    /// Which parser was actually used (`auto` is resolved to html or markdown).
    pub source: String,
    /// Number of hyperlinks found.
    pub link_count: usize,
    /// Number of in-page anchors found.
    pub anchor_count: usize,
    pub links: Vec<Link>,
    pub anchors: Vec<Anchor>,
}

/// Collapse runs of whitespace to single spaces and trim.
fn clean(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Classify a destination as relative or absolute. A fragment-only (`#x`),
/// protocol-relative (`//host`), or scheme-bearing (`https:`, `mailto:`) URL is
/// NOT relative; everything else is.
fn is_relative(url: &str) -> bool {
    let u = url.trim();
    if u.is_empty() || u.starts_with('#') || u.starts_with("//") {
        return false;
    }
    // A scheme is letters/digits/+/-/. then ':' before any '/', '?'.
    let has_scheme = u
        .find(':')
        .map(|colon| {
            let before = &u[..colon];
            !before.is_empty()
                && before.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
                && before
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
                && !before.contains(['/', '?'])
        })
        .unwrap_or(false);
    !has_scheme
}

/// GitHub-style heading slug: lowercase, drop punctuation, spaces/_ → '-',
/// collapse consecutive dashes, trim leading/trailing dashes.
fn slugify(text: &str) -> String {
    let mut out = String::new();
    for c in text.chars() {
        if c.is_alphanumeric() {
            out.extend(c.to_lowercase());
        } else if c == ' ' || c == '-' || c == '_' {
            out.push('-');
        }
        // other punctuation is dropped
    }
    let mut collapsed = String::new();
    let mut prev_dash = false;
    for c in out.chars() {
        if c == '-' {
            if !prev_dash {
                collapsed.push('-');
            }
            prev_dash = true;
        } else {
            collapsed.push(c);
            prev_dash = false;
        }
    }
    collapsed.trim_matches('-').to_string()
}

/// Heuristic: does this look like HTML rather than Markdown?
fn looks_like_html(s: &str) -> bool {
    let lower = s.to_ascii_lowercase();
    const NEEDLES: [&str; 9] = [
        "<a ", "<a>", "<html", "<body", "<div", "<p>", "<p ", "<area", "href=",
    ];
    NEEDLES.iter().any(|n| lower.contains(n))
}

fn extract_html(input: &str) -> Report {
    let doc = Html::parse_fragment(input);
    let a_sel = Selector::parse("a[href], area[href]").unwrap();
    let id_sel = Selector::parse("[id]").unwrap();
    let named_a_sel = Selector::parse("a[name]").unwrap();

    let mut links = Vec::new();
    for el in doc.select(&a_sel) {
        if let Some(href) = el.value().attr("href") {
            let url = href.trim().to_string();
            let text = clean(&el.text().collect::<String>());
            let relative = is_relative(&url);
            let rel = el.value().attr("rel").map(|r| clean(r)).unwrap_or_default();
            links.push(Link { url, text, relative, rel });
        }
    }

    let mut anchors = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for el in doc.select(&id_sel) {
        if let Some(id) = el.value().attr("id") {
            let name = id.trim().to_string();
            if !name.is_empty() && seen.insert(name.clone()) {
                anchors.push(Anchor { name });
            }
        }
    }
    for el in doc.select(&named_a_sel) {
        if let Some(n) = el.value().attr("name") {
            let name = n.trim().to_string();
            if !name.is_empty() && seen.insert(name.clone()) {
                anchors.push(Anchor { name });
            }
        }
    }

    Report {
        source: "html".into(),
        link_count: links.len(),
        anchor_count: anchors.len(),
        links,
        anchors,
    }
}

fn extract_markdown(input: &str) -> Report {
    use pulldown_cmark::{Event, Parser, Tag, TagEnd};

    let parser = Parser::new(input);
    let mut links: Vec<Link> = Vec::new();
    let mut anchors: Vec<Anchor> = Vec::new();
    let mut seen_anchor = std::collections::HashSet::new();

    // (url, text-accumulator) while inside a link.
    let mut link_stack: Vec<(String, String)> = Vec::new();
    // heading text accumulator while inside a heading.
    let mut heading_text: Option<String> = None;

    for ev in parser {
        match ev {
            Event::Start(Tag::Link { dest_url, .. }) => {
                link_stack.push((dest_url.to_string(), String::new()));
            }
            Event::End(TagEnd::Link) => {
                if let Some((url, text)) = link_stack.pop() {
                    let url = url.trim().to_string();
                    let relative = is_relative(&url);
                    links.push(Link {
                        text: clean(&text),
                        relative,
                        rel: String::new(),
                        url,
                    });
                }
            }
            Event::Start(Tag::Heading { id, .. }) => {
                if let Some(id) = id {
                    let name = id.to_string();
                    if !name.is_empty() && seen_anchor.insert(name.clone()) {
                        anchors.push(Anchor { name });
                    }
                }
                heading_text = Some(String::new());
            }
            Event::End(TagEnd::Heading(_)) => {
                if let Some(t) = heading_text.take() {
                    let slug = slugify(&t);
                    if !slug.is_empty() && seen_anchor.insert(slug.clone()) {
                        anchors.push(Anchor { name: slug });
                    }
                }
            }
            Event::Text(t) | Event::Code(t) => {
                if let Some((_, acc)) = link_stack.last_mut() {
                    acc.push_str(&t);
                }
                if let Some(h) = heading_text.as_mut() {
                    h.push_str(&t);
                }
            }
            Event::Html(html) | Event::InlineHtml(html) => {
                for name in scan_html_anchor_names(&html) {
                    if !name.is_empty() && seen_anchor.insert(name.clone()) {
                        anchors.push(Anchor { name });
                    }
                }
            }
            _ => {}
        }
    }

    Report {
        source: "markdown".into(),
        link_count: links.len(),
        anchor_count: anchors.len(),
        links,
        anchors,
    }
}

/// Find `id="…"` / `name="…"` (single- or double-quoted) inside a raw HTML
/// snippet embedded in Markdown — good enough for the common
/// `<a name="x"></a>` / `<span id="y">` anchor idioms.
fn scan_html_anchor_names(html: &str) -> Vec<String> {
    let mut out = Vec::new();
    let lower = html.to_ascii_lowercase();
    for attr in ["id", "name"] {
        let mut from = 0;
        while let Some(pos) = lower[from..].find(attr) {
            let abs = from + pos;
            let ok_before = abs == 0
                || matches!(lower.as_bytes()[abs - 1], b' ' | b'\t' | b'<' | b'\n' | b'"' | b'\'');
            let after = abs + attr.len();
            let rest = &html[after..];
            let rest_trim = rest.trim_start();
            if ok_before && rest_trim.starts_with('=') {
                let val_part = rest_trim[1..].trim_start();
                let bytes = val_part.as_bytes();
                if let Some(&q) = bytes.first() {
                    if q == b'"' || q == b'\'' {
                        let qc = q as char;
                        if let Some(end) = val_part[1..].find(qc) {
                            let v = &val_part[1..1 + end];
                            let trimmed = v.trim();
                            if !trimmed.is_empty() {
                                out.push(trimmed.to_string());
                            }
                        }
                    }
                }
            }
            from = abs + attr.len();
        }
    }
    out
}

/// Resolve a relative link against a parsed base URL. Returns `Some(absolute)`
/// when it resolved; `None` when it couldn't (left verbatim).
fn resolve(base: &Url, link: &str) -> Option<String> {
    base.join(link).ok().map(|u| u.to_string())
}

/// Apply `base_url` resolution and `dedup` to a freshly parsed report.
fn apply_options(mut report: Report, opts: &Options) -> Result<Report, String> {
    if let Some(b) = opts.base_url.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        let base = Url::parse(b).map_err(|e| format!("invalid base_url {b:?}: {e}"))?;
        for l in &mut report.links {
            if l.relative {
                if let Some(abs) = resolve(&base, &l.url) {
                    l.url = abs;
                    l.relative = false;
                }
            }
        }
    }
    if opts.dedup {
        let mut seen = std::collections::HashSet::new();
        report.links.retain(|l| seen.insert(l.url.clone()));
    }
    report.link_count = report.links.len();
    Ok(report)
}

/// Extract links + anchors from `input`, choosing the parser per `source` and
/// applying `opts` (base-URL resolution + dedup).
pub fn extract_with(input: &str, source: Source, opts: &Options) -> Result<Report, String> {
    if input.trim().is_empty() {
        return Err("input is empty".into());
    }
    let resolved = match source {
        Source::Html => Source::Html,
        Source::Markdown => Source::Markdown,
        Source::Auto => {
            if looks_like_html(input) {
                Source::Html
            } else {
                Source::Markdown
            }
        }
    };
    let report = match resolved {
        Source::Html => extract_html(input),
        Source::Markdown => extract_markdown(input),
        Source::Auto => unreachable!(),
    };
    apply_options(report, opts)
}

/// Extract with default options (no base-URL, no dedup).
pub fn extract(input: &str, source: Source) -> Result<Report, String> {
    extract_with(input, source, &Options::default())
}

/// JSON rendering (chat + page json mode).
pub fn extract_json(input: &str, source: Source, opts: &Options) -> Result<String, String> {
    let report = extract_with(input, source, opts)?;
    serde_json::to_string_pretty(&report).map_err(|e| format!("serialize: {e}"))
}

/// Human-readable rendering (page text mode).
pub fn render(input: &str, source: Source, output: Output, opts: &Options) -> Result<String, String> {
    let report = extract_with(input, source, opts)?;
    if output == Output::Json {
        return serde_json::to_string_pretty(&report).map_err(|e| format!("serialize: {e}"));
    }
    let mut out = String::new();
    out.push_str(&format!(
        "Parsed as {}. {} link(s), {} anchor(s).\n",
        report.source, report.link_count, report.anchor_count
    ));
    if !report.links.is_empty() {
        out.push_str("\nLinks:\n");
        for l in &report.links {
            let mut tag = String::new();
            if l.relative {
                tag.push_str(" [relative]");
            }
            if !l.rel.is_empty() {
                tag.push_str(&format!(" [rel: {}]", l.rel));
            }
            if l.text.is_empty() {
                out.push_str(&format!("  {}{}\n", l.url, tag));
            } else {
                out.push_str(&format!("  {} — {}{}\n", l.url, l.text, tag));
            }
        }
    }
    if !report.anchors.is_empty() {
        out.push_str("\nAnchors (jump targets):\n");
        for a in &report.anchors {
            out.push_str(&format!("  #{}\n", a.name));
        }
    }
    if report.links.is_empty() && report.anchors.is_empty() {
        out.push_str("\nNo links or anchors found.");
    }
    Ok(out.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const HTML: &str = r##"
        <h2 id="intro">Intro</h2>
        <p>See <a href="https://example.com/a">Example A</a> and
        <a href="/local/page">a local page</a>.</p>
        <a name="legacy"></a>
        <a href="#intro">jump up</a>
        <area href="mailto:hi@example.com">
        <a>no href</a>
    "##;

    const MD: &str = "# Getting Started\n\nSee [Example A](https://example.com/a) and \
        [relative](./docs/x.md). Also an autolink <https://rust-lang.org>.\n\n\
        <a name=\"manual\"></a>\n\n## Big Section";

    #[test]
    fn html_links_and_anchors() {
        let r = extract(HTML, Source::Html).unwrap();
        assert_eq!(r.source, "html");
        // 4 hrefs: example.com, /local/page, #intro, mailto:
        assert_eq!(r.link_count, 4);
        assert_eq!(r.links[0].url, "https://example.com/a");
        assert_eq!(r.links[0].text, "Example A");
        assert!(!r.links[0].relative);
        assert!(r.links[1].relative); // /local/page
        assert!(!r.links[2].relative); // #intro (fragment, not relative)
        assert!(!r.links[3].relative); // mailto:
        let names: Vec<&str> = r.anchors.iter().map(|a| a.name.as_str()).collect();
        assert!(names.contains(&"intro"));
        assert!(names.contains(&"legacy"));
    }

    #[test]
    fn markdown_links_and_slugs() {
        let r = extract(MD, Source::Markdown).unwrap();
        assert_eq!(r.source, "markdown");
        // inline x2 + autolink x1
        assert_eq!(r.link_count, 3);
        assert_eq!(r.links[0].url, "https://example.com/a");
        assert_eq!(r.links[0].text, "Example A");
        assert!(r.links[1].relative); // ./docs/x.md
        assert_eq!(r.links[2].url, "https://rust-lang.org"); // autolink
        let names: Vec<&str> = r.anchors.iter().map(|a| a.name.as_str()).collect();
        assert!(names.contains(&"getting-started"));
        assert!(names.contains(&"big-section"));
        assert!(names.contains(&"manual"));
    }

    #[test]
    fn auto_detects_html_vs_markdown() {
        let h = extract("<a href=\"https://a.com\">x</a>", Source::Auto).unwrap();
        assert_eq!(h.source, "html");
        let m = extract("[x](https://a.com)", Source::Auto).unwrap();
        assert_eq!(m.source, "markdown");
    }

    #[test]
    fn empty_input_is_error() {
        assert!(extract("   ", Source::Auto).is_err());
    }

    #[test]
    fn is_relative_classifies() {
        assert!(is_relative("/x"));
        assert!(is_relative("docs/x.md"));
        assert!(is_relative("./a"));
        assert!(!is_relative("https://x.com"));
        assert!(!is_relative("mailto:a@b.com"));
        assert!(!is_relative("//cdn.example.com/x"));
        assert!(!is_relative("#frag"));
    }

    #[test]
    fn slugify_github_style() {
        assert_eq!(slugify("Getting Started"), "getting-started");
        assert_eq!(slugify("API & Usage!"), "api-usage");
        assert_eq!(slugify("Hello, World"), "hello-world");
    }

    #[test]
    fn source_and_output_parse() {
        assert_eq!(Source::parse("AUTO").unwrap(), Source::Auto);
        assert_eq!(Source::parse("md").unwrap(), Source::Markdown);
        assert_eq!(Source::parse("html").unwrap(), Source::Html);
        assert!(Source::parse("xml").is_err());
        assert_eq!(Output::parse("").unwrap(), Output::Text);
        assert_eq!(Output::parse("json").unwrap(), Output::Json);
        assert!(Output::parse("yaml").is_err());
    }

    #[test]
    fn renders_text_and_json() {
        let o = Options::default();
        let t = render(HTML, Source::Html, Output::Text, &o).unwrap();
        assert!(t.contains("Links:"));
        assert!(t.contains("[relative]"));
        let j = render(HTML, Source::Html, Output::Json, &o).unwrap();
        assert!(j.contains("\"link_count\""));
    }

    #[test]
    fn captures_rel_attribute() {
        let r = extract(
            r#"<a href="https://x.com" rel="nofollow sponsored">x</a>"#,
            Source::Html,
        )
        .unwrap();
        assert_eq!(r.links[0].rel, "nofollow sponsored");
        // markdown links carry no rel
        let m = extract("[x](https://x.com)", Source::Markdown).unwrap();
        assert_eq!(m.links[0].rel, "");
    }

    #[test]
    fn base_url_resolves_relative_links() {
        let opts = Options {
            base_url: Some("https://example.com/docs/".to_string()),
            dedup: false,
        };
        let r = extract_with(HTML, Source::Html, &opts).unwrap();
        // /local/page resolves against the origin; absolute + fragment untouched.
        assert_eq!(r.links[0].url, "https://example.com/a");
        assert_eq!(r.links[1].url, "https://example.com/local/page");
        assert!(!r.links[1].relative);
        // mailto: stays as-is.
        assert_eq!(r.links[3].url, "mailto:hi@example.com");
    }

    #[test]
    fn invalid_base_url_is_error() {
        let opts = Options {
            base_url: Some("not a url".to_string()),
            dedup: false,
        };
        assert!(extract_with(HTML, Source::Html, &opts).is_err());
    }

    #[test]
    fn dedup_collapses_repeated_links() {
        let html = r#"<a href="https://x.com">one</a><a href="https://x.com">two</a><a href="https://y.com">y</a>"#;
        let plain = extract(html, Source::Html).unwrap();
        assert_eq!(plain.link_count, 3);
        let opts = Options { base_url: None, dedup: true };
        let d = extract_with(html, Source::Html, &opts).unwrap();
        assert_eq!(d.link_count, 2);
        assert_eq!(d.links[0].text, "one"); // first-seen kept
    }
}
