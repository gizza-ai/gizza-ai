//! gizza-ai/relative-to-absolute-urls — rewrite the relative URLs in a chunk of
//! HTML to absolute ones against a base URL.
//!
//! The contract: **the output is the input with only the URL attribute values
//! changed**. No whitespace collapsing, no tag normalization, no re-indentation,
//! no attribute reordering, no quote-style changes — run it on a template and
//! the diff shows the URLs and nothing else.
//!
//! HTML is not well-formed XML, so this is a forgiving hand-rolled scanner
//! rather than a parser (the same conclusion `html-formatter`, `html-minifier`
//! and `html-comment-stripper` reached). What it gets right that a
//! `href="([^"]*)"` search-and-replace cannot:
//!
//! * **Comments and raw-text elements.** `<!-- <a href="x"> -->` and a `href`
//!   written inside `<script>`, `<textarea>` or `<title>` are text, not markup,
//!   and are copied through untouched. `<style>` is only touched when the CSS
//!   pass is enabled.
//! * **Which attributes are URLs.** `src`/`href` are only the beginning:
//!   `srcset` is a candidate LIST with width/density descriptors, `ping` is a
//!   space-separated list, and `<meta http-equiv="refresh" content="5; url=…">`
//!   hides a URL inside a directive. Each is parsed on its own terms.
//! * **What is already absolute.** Values carrying a scheme (`https:`,
//!   `mailto:`, `tel:`, `data:`, `javascript:`), bare fragments (`#top`) and
//!   template placeholders (`{{ url }}`, `<% … %>`) are left exactly as written.
//! * **`<base href>`.** A document that carries one resolves its own relative
//!   URLs against it, not against the page's own address — so we honour it by
//!   default, exactly like a browser.
//!
//! Resolution itself is the WHATWG algorithm via the `url` crate, so `../`
//! segments, root-relative paths, queries and fragments behave the way they do
//! in a browser's address bar.

use url::Url;

/// Largest accepted input, in bytes.
pub const MAX_INPUT_BYTES: usize = 5_000_000;

/// Raw-text / escapable-raw-text elements: markup inside them is text.
const RAW_TEXT: [&str; 4] = ["script", "style", "textarea", "title"];

/// Prefixes that mark a templating placeholder rather than a URL. Resolving one
/// would bake the base into a string the template engine still has to fill in.
const TEMPLATE_PREFIXES: [&str; 6] = ["{{", "{%", "{#", "<%", "${", "[["];

/// How wide a net to cast over URL-bearing attributes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// `href` and `src` only.
    HrefSrc,
    /// The everyday set: adds `srcset`, `poster`, `action`, `formaction`,
    /// `data`, `background`, `ping` and `<meta http-equiv=refresh>`.
    Common,
    /// Everything HTML defines as a URL attribute, including the rarities.
    All,
}

impl Scope {
    pub fn parse(s: &str) -> Result<Scope, String> {
        match s.trim() {
            "" | "common" => Ok(Scope::Common),
            "href-src" => Ok(Scope::HrefSrc),
            "all" => Ok(Scope::All),
            other => Err(format!(
                "unknown attributes '{other}' — use 'href-src', 'common' or 'all'"
            )),
        }
    }
}

/// How a URL attribute's value is structured.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ValueKind {
    /// One URL.
    Single,
    /// `img`/`source` `srcset`: comma-separated candidates, each an optional
    /// URL + width/density descriptor.
    SrcSet,
    /// `a`/`area` `ping`: whitespace-separated URLs.
    SpaceList,
    /// `<meta http-equiv="refresh" content="5; url=…">`.
    MetaRefresh,
    /// A CSS fragment (`style="…"`) containing `url(…)` / `@import`.
    Css,
}

/// One URL the scanner looked at, and what it decided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Found {
    /// 1-based line of the tag the URL sat in.
    pub line: usize,
    /// Lowercased element name (`""` for a `<style>` block's own contents).
    pub tag: String,
    /// Lowercased attribute name (`"style-block"` inside `<style>`).
    pub attr: String,
    /// The value as written, trimmed.
    pub original: String,
    /// The absolute URL written in its place (empty when nothing changed).
    pub resolved: String,
    /// `rewritten`, or `kept:<reason>`.
    pub action: &'static str,
}

/// Is this attribute a URL attribute on this element, and how is it shaped?
fn url_attr_kind(scope: Scope, tag: &str, attr: &str, meta_refresh: bool) -> Option<ValueKind> {
    // `href` and `src` are URL attributes wherever they appear.
    if attr == "href" || attr == "src" {
        return Some(ValueKind::Single);
    }
    if scope == Scope::HrefSrc {
        return None;
    }
    let common = match (tag, attr) {
        ("img" | "source", "srcset") => Some(ValueKind::SrcSet),
        ("video", "poster") => Some(ValueKind::Single),
        ("form", "action") => Some(ValueKind::Single),
        ("button" | "input", "formaction") => Some(ValueKind::Single),
        ("object", "data") => Some(ValueKind::Single),
        (
            "body" | "table" | "td" | "th" | "tr" | "thead" | "tbody" | "tfoot",
            "background",
        ) => Some(ValueKind::Single),
        ("a" | "area", "ping") => Some(ValueKind::SpaceList),
        ("meta", "content") if meta_refresh => Some(ValueKind::MetaRefresh),
        _ => None,
    };
    if common.is_some() || scope == Scope::Common {
        return common;
    }
    match (tag, attr) {
        ("blockquote" | "q" | "del" | "ins", "cite") => Some(ValueKind::Single),
        ("img" | "iframe" | "frame", "longdesc") => Some(ValueKind::Single),
        ("html", "manifest") => Some(ValueKind::Single),
        ("head", "profile") => Some(ValueKind::Single),
        ("menuitem", "icon") => Some(ValueKind::Single),
        ("object" | "applet", "codebase" | "archive") => Some(ValueKind::Single),
        ("applet", "code" | "object") => Some(ValueKind::Single),
        (_, "itemtype") => Some(ValueKind::Single),
        _ => None,
    }
}

/// The scheme of an absolute URL value (`"https"` for `https://a/b`), if it has
/// one. A colon that appears after a `/` is part of a path, not a scheme.
fn scheme_of(v: &str) -> Option<&str> {
    let b = v.as_bytes();
    if b.is_empty() || !b[0].is_ascii_alphabetic() {
        return None;
    }
    let mut i = 1;
    while i < b.len() {
        let c = b[i];
        if c == b':' {
            return Some(&v[..i]);
        }
        if !(c.is_ascii_alphanumeric() || c == b'+' || c == b'-' || c == b'.') {
            return None;
        }
        i += 1;
    }
    None
}

fn is_template(v: &str) -> bool {
    TEMPLATE_PREFIXES.iter().any(|p| v.starts_with(p))
}

/// Advance past one whole UTF-8 character starting at `p`.
fn next_char(b: &[u8], p: usize) -> usize {
    let mut j = p + 1;
    while j < b.len() && (b[j] & 0xC0) == 0x80 {
        j += 1;
    }
    j
}

/// Byte index just past the `>` that closes the tag starting at `i`, skipping
/// quoted attribute values so a `>` inside one doesn't end the tag early.
fn scan_tag(b: &[u8], i: usize) -> usize {
    let n = b.len();
    let mut j = i + 1;
    let mut quote = 0u8;
    while j < n {
        let c = b[j];
        if quote != 0 {
            if c == quote {
                quote = 0;
            }
        } else if c == b'"' || c == b'\'' {
            quote = c;
        } else if c == b'>' {
            return j + 1;
        }
        j += 1;
    }
    n
}

/// Lowercased element name of a tag starting at `<`, or `""` if it isn't one.
fn tag_name(s: &str) -> String {
    let rest = s
        .strip_prefix("</")
        .or_else(|| s.strip_prefix('<'))
        .unwrap_or(s);
    rest.chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == '_')
        .collect::<String>()
        .to_ascii_lowercase()
}

/// One attribute located inside a tag's source text.
struct Attr {
    name: String,
    /// Byte range of the VALUE inside the tag text, quotes excluded.
    vstart: usize,
    vend: usize,
    quoted: bool,
}

/// Locate every `name=value` attribute in a tag's source text.
fn parse_attrs(tag: &str) -> Vec<Attr> {
    let b = tag.as_bytes();
    let n = b.len();
    let mut i = if tag.starts_with("</") { 2 } else { 1 };
    while i < n && !b[i].is_ascii_whitespace() && b[i] != b'>' && b[i] != b'/' {
        i += 1;
    }
    let mut out: Vec<Attr> = Vec::new();
    while i < n {
        while i < n && (b[i].is_ascii_whitespace() || b[i] == b'/') {
            i += 1;
        }
        if i >= n || b[i] == b'>' {
            break;
        }
        let ns = i;
        while i < n
            && !b[i].is_ascii_whitespace()
            && b[i] != b'='
            && b[i] != b'>'
            && b[i] != b'/'
        {
            i += 1;
        }
        if i == ns {
            i += 1; // never stall on an unexpected byte
            continue;
        }
        let name = tag[ns..i].to_ascii_lowercase();
        let mut j = i;
        while j < n && b[j].is_ascii_whitespace() {
            j += 1;
        }
        if j < n && b[j] == b'=' {
            j += 1;
            while j < n && b[j].is_ascii_whitespace() {
                j += 1;
            }
            if j < n && (b[j] == b'"' || b[j] == b'\'') {
                let q = b[j];
                let vs = j + 1;
                let mut k = vs;
                while k < n && b[k] != q {
                    k += 1;
                }
                out.push(Attr { name, vstart: vs, vend: k, quoted: true });
                i = if k < n { k + 1 } else { k };
            } else {
                let vs = j;
                let mut k = vs;
                while k < n && !b[k].is_ascii_whitespace() && b[k] != b'>' {
                    k += 1;
                }
                out.push(Attr { name, vstart: vs, vend: k, quoted: false });
                i = k;
            }
        } else {
            i = j;
        }
    }
    out
}

/// The (trimmed) value of `name` in a tag's source text, if present.
fn attr_value<'a>(tag: &'a str, attrs: &[Attr], name: &str) -> Option<&'a str> {
    attrs
        .iter()
        .find(|a| a.name == name)
        .map(|a| tag[a.vstart..a.vend].trim())
}

/// Config + running state for one run: resolves values and records decisions.
struct Resolver {
    /// The base the caller supplied.
    user_base: Url,
    /// The base relative URLs actually resolve against (a `<base href>` wins).
    effective: Url,
    keep_protocol_relative: bool,
    resolve_fragments: bool,
    found: Vec<Found>,
}

impl Resolver {
    /// Resolve one URL value. Returns `Some(absolute)` when the value changed;
    /// `None` (with the reason recorded) when it is left exactly as written.
    fn resolve(&mut self, line: usize, tag: &str, attr: &str, raw: &str) -> Option<String> {
        let v = raw.trim();
        let mut resolved = String::new();
        let action: &'static str = if v.is_empty() {
            "kept:empty"
        } else if is_template(v) {
            "kept:template"
        } else if v.starts_with("//") {
            if self.keep_protocol_relative {
                "kept:protocol-relative"
            } else {
                self.join(tag, attr, v, &mut resolved)
            }
        } else if v.starts_with('#') {
            if self.resolve_fragments {
                self.join(tag, attr, v, &mut resolved)
            } else {
                "kept:fragment"
            }
        } else if let Some(scheme) = scheme_of(v) {
            if v[scheme.len() + 1..].starts_with("//") {
                "kept:absolute"
            } else {
                "kept:scheme"
            }
        } else {
            self.join(tag, attr, v, &mut resolved)
        };
        self.found.push(Found {
            line,
            tag: tag.to_string(),
            attr: attr.to_string(),
            original: v.to_string(),
            resolved: resolved.clone(),
            action,
        });
        if action == "rewritten" {
            Some(resolved)
        } else {
            None
        }
    }

    /// WHATWG-resolve `v`; a `<base href>` resolves against the caller's base,
    /// everything else against the effective base.
    fn join(&self, tag: &str, attr: &str, v: &str, out: &mut String) -> &'static str {
        let base = if tag == "base" && attr == "href" {
            &self.user_base
        } else {
            &self.effective
        };
        match base.join(v) {
            Ok(u) => {
                let s = u.to_string();
                if s == v {
                    "kept:absolute"
                } else {
                    *out = s;
                    "rewritten"
                }
            }
            Err(_) => "kept:unresolvable",
        }
    }
}

/// Rewrite an `img`/`source` `srcset`: `url [descriptor], url [descriptor], …`.
/// A candidate's URL runs to the next whitespace or the comma that ends it.
fn rewrite_srcset(v: &str, line: usize, tag: &str, r: &mut Resolver) -> String {
    let b = v.as_bytes();
    let n = b.len();
    let mut out = String::with_capacity(n);
    let mut i = 0usize;
    while i < n {
        // Leading whitespace and stray commas between candidates.
        let ws = i;
        while i < n && (b[i].is_ascii_whitespace() || b[i] == b',') {
            i += 1;
        }
        out.push_str(&v[ws..i]);
        if i >= n {
            break;
        }
        let us = i;
        while i < n && !b[i].is_ascii_whitespace() && b[i] != b',' {
            i += 1;
        }
        let url = &v[us..i];
        match r.resolve(line, tag, "srcset", url) {
            Some(abs) => out.push_str(&abs),
            None => out.push_str(url),
        }
        // Descriptor (`2x`, `640w`) up to the comma that ends the candidate.
        let ds = i;
        while i < n && b[i] != b',' {
            i += 1;
        }
        out.push_str(&v[ds..i]);
    }
    out
}

/// Rewrite a whitespace-separated URL list (`a`/`area` `ping`).
fn rewrite_space_list(v: &str, line: usize, tag: &str, attr: &str, r: &mut Resolver) -> String {
    let b = v.as_bytes();
    let n = b.len();
    let mut out = String::with_capacity(n);
    let mut i = 0usize;
    while i < n {
        let ws = i;
        while i < n && b[i].is_ascii_whitespace() {
            i += 1;
        }
        out.push_str(&v[ws..i]);
        if i >= n {
            break;
        }
        let us = i;
        while i < n && !b[i].is_ascii_whitespace() {
            i += 1;
        }
        let url = &v[us..i];
        match r.resolve(line, tag, attr, url) {
            Some(abs) => out.push_str(&abs),
            None => out.push_str(url),
        }
    }
    out
}

/// Rewrite the URL hidden in `<meta http-equiv="refresh" content="5; url=…">`.
fn rewrite_meta_refresh(v: &str, line: usize, r: &mut Resolver) -> String {
    let lower = v.to_ascii_lowercase();
    let Some(p) = lower.find("url") else {
        return v.to_string();
    };
    let b = v.as_bytes();
    let mut i = p + 3;
    while i < b.len() && b[i].is_ascii_whitespace() {
        i += 1;
    }
    if i >= b.len() || b[i] != b'=' {
        return v.to_string();
    }
    i += 1;
    while i < b.len() && b[i].is_ascii_whitespace() {
        i += 1;
    }
    let quote = if i < b.len() && (b[i] == b'"' || b[i] == b'\'') {
        let q = b[i];
        i += 1;
        Some(q)
    } else {
        None
    };
    let us = i;
    let ue = match quote {
        Some(q) => {
            let mut k = i;
            while k < b.len() && b[k] != q {
                k += 1;
            }
            k
        }
        None => v.len(),
    };
    let url = v[us..ue].trim();
    let mut out = String::with_capacity(v.len());
    out.push_str(&v[..us]);
    match r.resolve(line, "meta", "content", url) {
        Some(abs) => out.push_str(&abs),
        None => out.push_str(&v[us..ue]),
    }
    out.push_str(&v[ue..]);
    out
}

/// Rewrite `url(…)` and `@import "…"` inside a CSS fragment, leaving every
/// other byte alone.
fn rewrite_css(css: &str, line: usize, tag: &str, attr: &str, r: &mut Resolver) -> String {
    let b = css.as_bytes();
    let n = b.len();
    let mut out = String::with_capacity(n);
    let mut i = 0usize;
    while i < n {
        let word_boundary =
            i == 0 || !(b[i - 1].is_ascii_alphanumeric() || b[i - 1] == b'-' || b[i - 1] == b'_');
        if word_boundary && n - i >= 4 && css[i..i + 4].eq_ignore_ascii_case("url(") {
            let mut j = i + 4;
            while j < n && b[j].is_ascii_whitespace() {
                j += 1;
            }
            let quote = if j < n && (b[j] == b'"' || b[j] == b'\'') {
                let q = b[j];
                j += 1;
                Some(q)
            } else {
                None
            };
            let vs = j;
            let terminator = quote.unwrap_or(b')');
            let mut k = vs;
            while k < n && b[k] != terminator {
                k += 1;
            }
            if k >= n {
                out.push_str(&css[i..]); // unterminated url( — copy the rest verbatim
                return out;
            }
            let inner = &css[vs..k];
            let lead = inner.len() - inner.trim_start().len();
            let trail = inner.len() - inner.trim_end().len();
            out.push_str(&css[i..vs + lead]);
            match r.resolve(line, tag, attr, inner.trim()) {
                Some(abs) => out.push_str(&abs),
                None => out.push_str(&css[vs + lead..k - trail]),
            }
            out.push_str(&css[k - trail..k]);
            i = k;
            continue;
        }
        if word_boundary && n - i >= 7 && css[i..i + 7].eq_ignore_ascii_case("@import") {
            let mut j = i + 7;
            while j < n && b[j].is_ascii_whitespace() {
                j += 1;
            }
            if j < n && (b[j] == b'"' || b[j] == b'\'') {
                let q = b[j];
                let vs = j + 1;
                let mut k = vs;
                while k < n && b[k] != q {
                    k += 1;
                }
                if k < n {
                    out.push_str(&css[i..vs]);
                    let inner = &css[vs..k];
                    match r.resolve(line, tag, attr, inner.trim()) {
                        Some(abs) => out.push_str(&abs),
                        None => out.push_str(inner),
                    }
                    i = k;
                    continue;
                }
            }
            out.push_str(&css[i..j]);
            i = j;
            continue;
        }
        let j = next_char(b, i);
        out.push_str(&css[i..j]);
        i = j;
    }
    out
}

/// Rewrite every URL attribute inside one tag's source text.
fn rewrite_tag(
    tag_src: &str,
    name: &str,
    line: usize,
    scope: Scope,
    style_urls: bool,
    r: &mut Resolver,
) -> String {
    let attrs = parse_attrs(tag_src);
    if attrs.is_empty() {
        return tag_src.to_string();
    }
    let meta_refresh = name == "meta"
        && attr_value(tag_src, &attrs, "http-equiv")
            .map(|v| v.eq_ignore_ascii_case("refresh"))
            .unwrap_or(false);
    let mut out = String::with_capacity(tag_src.len());
    let mut pos = 0usize;
    for a in &attrs {
        let kind = if style_urls && a.name == "style" {
            Some(ValueKind::Css)
        } else {
            url_attr_kind(scope, name, &a.name, meta_refresh)
        };
        let Some(kind) = kind else { continue };
        let raw = &tag_src[a.vstart..a.vend];
        let replacement: Option<String> = match kind {
            ValueKind::Single => r.resolve(line, name, &a.name, raw),
            ValueKind::SrcSet => {
                let s = rewrite_srcset(raw, line, name, r);
                (s != raw).then_some(s)
            }
            ValueKind::SpaceList => {
                let s = rewrite_space_list(raw, line, name, &a.name, r);
                (s != raw).then_some(s)
            }
            ValueKind::MetaRefresh => {
                let s = rewrite_meta_refresh(raw, line, r);
                (s != raw).then_some(s)
            }
            ValueKind::Css => {
                let s = rewrite_css(raw, line, name, "style", r);
                (s != raw).then_some(s)
            }
        };
        let Some(mut new) = replacement else { continue };
        // An unquoted value that would become ambiguous gets quoted.
        if !a.quoted && new.chars().any(|c| c.is_whitespace() || "\"'=<>`".contains(c)) {
            new = format!("\"{}\"", new.replace('"', "&quot;"));
        }
        out.push_str(&tag_src[pos..a.vstart]);
        out.push_str(&new);
        pos = a.vend;
    }
    out.push_str(&tag_src[pos..]);
    out
}

/// The first `<base href>` in the document, if any (what a browser honours).
fn find_base_href(html: &str) -> Option<String> {
    let b = html.as_bytes();
    let n = b.len();
    let mut i = 0usize;
    while i < n {
        if b[i] != b'<' {
            i += 1;
            continue;
        }
        if html[i..].starts_with("<!--") {
            i = match html[i + 4..].find("-->") {
                Some(p) => i + 4 + p + 3,
                None => n,
            };
            continue;
        }
        let end = scan_tag(b, i);
        let raw = &html[i..end];
        if tag_name(raw) == "base" {
            let attrs = parse_attrs(raw);
            if let Some(v) = attr_value(raw, &attrs, "href") {
                if !v.is_empty() {
                    return Some(v.to_string());
                }
            }
        }
        i = end;
    }
    None
}

fn escape_field(s: &str) -> String {
    let flat = s.replace('\r', " ").replace('\n', "\\n");
    if flat.contains(',') || flat.contains('"') {
        format!("\"{}\"", flat.replace('"', "\"\""))
    } else {
        flat
    }
}

/// Rewrite the relative URLs in `html` to absolute ones against `base`.
///
/// * `base` — an absolute URL the relative values are relative to.
/// * `attributes` — `"href-src"` | `"common"` (default) | `"all"`.
/// * `use_base_tag` — honour a `<base href>` in the document.
/// * `protocol_relative` — `"resolve"` (give `//host/x` the base's scheme) or
///   `"keep"`.
/// * `resolve_fragments` — also make bare `#anchor` links absolute.
/// * `style_urls` — also rewrite `url(…)` / `@import` in `style` attributes and
///   `<style>` blocks.
/// * `output` — `"html"` | `"report"` | `"urls"`.
#[allow(clippy::too_many_arguments)]
pub fn absolutize(
    html: &str,
    base: &str,
    attributes: &str,
    use_base_tag: bool,
    protocol_relative: &str,
    resolve_fragments: bool,
    style_urls: bool,
    output: &str,
) -> Result<String, String> {
    if html.trim().is_empty() {
        return Err("no HTML input".into());
    }
    if html.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "input is {} bytes, over the {MAX_INPUT_BYTES}-byte limit",
            html.len()
        ));
    }
    let base_raw = base.trim();
    if base_raw.is_empty() {
        return Err(
            "no base URL — pass the absolute address the relative URLs are relative to, e.g. 'https://example.com/blog/post.html'".into(),
        );
    }
    let scope = Scope::parse(attributes)?;
    let keep_protocol_relative = match protocol_relative.trim() {
        "" | "resolve" => false,
        "keep" => true,
        other => {
            return Err(format!(
                "unknown protocol_relative '{other}' — use 'resolve' or 'keep'"
            ))
        }
    };
    if !matches!(output.trim(), "" | "html" | "report" | "urls") {
        return Err(format!(
            "unknown output '{}' — use 'html', 'report' or 'urls'",
            output.trim()
        ));
    }
    let user_base = Url::parse(base_raw).map_err(|e| {
        format!("invalid base URL '{base_raw}': {e} — expected an absolute URL like 'https://example.com/blog/post.html'")
    })?;
    if user_base.cannot_be_a_base() {
        return Err(format!(
            "base URL '{base_raw}' cannot be a base — expected a hierarchical absolute URL like 'https://example.com/blog/post.html', not a '{}:' address",
            user_base.scheme()
        ));
    }

    let doc_base = if use_base_tag { find_base_href(html) } else { None };
    let effective = match doc_base.as_deref() {
        Some(v) => user_base.join(v).map_err(|e| {
            format!("the document's <base href=\"{v}\"> is not a usable base URL: {e}")
        })?,
        None => user_base.clone(),
    };
    let mut r = Resolver {
        user_base: user_base.clone(),
        effective,
        keep_protocol_relative,
        resolve_fragments,
        found: Vec::new(),
    };

    let b = html.as_bytes();
    let n = b.len();
    let lower = html.to_ascii_lowercase();
    let mut out = String::with_capacity(n);
    let mut i = 0usize;
    let mut line = 1usize;

    while i < n {
        if b[i] == b'<' && html[i..].starts_with("<!--") {
            // Comments are text: copied through untouched.
            let end = match html[i + 4..].find("-->") {
                Some(p) => i + 4 + p + 3,
                None => n,
            };
            let raw = &html[i..end];
            out.push_str(raw);
            line += raw.matches('\n').count();
            i = end;
            continue;
        }
        if b[i] == b'<' {
            let end = scan_tag(b, i);
            let raw = &html[i..end];
            let name = tag_name(raw);
            let is_close = b.get(i + 1) == Some(&b'/');
            let self_closing = raw.trim_end().ends_with("/>");
            if is_close {
                out.push_str(raw);
            } else {
                out.push_str(&rewrite_tag(raw, &name, line, scope, style_urls, &mut r));
            }
            line += raw.matches('\n').count();
            i = end;
            // Raw-text element: its content is text, not markup.
            if !is_close && !self_closing && RAW_TEXT.contains(&name.as_str()) {
                let close_pat = format!("</{name}");
                let close_lt = lower[i..].find(&close_pat).map(|p| i + p).unwrap_or(n);
                let body = &html[i..close_lt];
                if style_urls && name == "style" {
                    out.push_str(&rewrite_css(body, line, "style", "style-block", &mut r));
                } else {
                    out.push_str(body);
                }
                line += body.matches('\n').count();
                i = close_lt;
            }
            continue;
        }
        let start = i;
        while i < n && b[i] != b'<' {
            i += 1;
        }
        let t = &html[start..i];
        out.push_str(t);
        line += t.matches('\n').count();
    }

    match output.trim() {
        "urls" => {
            if r.found.is_empty() {
                return Ok("no URL attributes found\n".into());
            }
            let mut s = String::from("line,tag,attribute,original,resolved,action\n");
            for f in &r.found {
                s.push_str(&format!(
                    "{},{},{},{},{},{}\n",
                    f.line,
                    f.tag,
                    f.attr,
                    escape_field(&f.original),
                    escape_field(&f.resolved),
                    f.action
                ));
            }
            Ok(s)
        }
        "report" => {
            let count = |a: &str| r.found.iter().filter(|f| f.action == a).count();
            let mut s = String::from("metric,value\n");
            s.push_str(&format!("base,{}\n", escape_field(r.user_base.as_str())));
            s.push_str(&format!(
                "effective_base,{}\n",
                escape_field(r.effective.as_str())
            ));
            s.push_str(&format!(
                "base_tag_used,{}\n",
                if doc_base.is_some() { "yes" } else { "no" }
            ));
            s.push_str(&format!("urls_found,{}\n", r.found.len()));
            s.push_str(&format!("urls_rewritten,{}\n", count("rewritten")));
            s.push_str(&format!(
                "urls_kept,{}\n",
                r.found.len() - count("rewritten")
            ));
            s.push_str(&format!("kept_absolute,{}\n", count("kept:absolute")));
            s.push_str(&format!("kept_scheme,{}\n", count("kept:scheme")));
            s.push_str(&format!("kept_fragment,{}\n", count("kept:fragment")));
            s.push_str(&format!(
                "kept_protocol_relative,{}\n",
                count("kept:protocol-relative")
            ));
            s.push_str(&format!("kept_template,{}\n", count("kept:template")));
            s.push_str(&format!("kept_empty,{}\n", count("kept:empty")));
            s.push_str(&format!(
                "kept_unresolvable,{}\n",
                count("kept:unresolvable")
            ));
            s.push_str(&format!("bytes_before,{}\n", html.len()));
            s.push_str(&format!("bytes_after,{}\n", out.len()));
            Ok(s)
        }
        _ => Ok(out),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(html: &str, base: &str) -> String {
        absolutize(html, base, "common", true, "resolve", false, false, "html").unwrap()
    }

    #[test]
    fn rewrites_href_and_src_and_leaves_everything_else_byte_identical() {
        assert_eq!(
            run(
                "<p class='x'>  <a href=\"../about.html\">about</a>\n<img src='logo.png' alt=\"Logo, big\">\n",
                "https://example.com/blog/post.html"
            ),
            "<p class='x'>  <a href=\"https://example.com/about.html\">about</a>\n<img src='https://example.com/blog/logo.png' alt=\"Logo, big\">\n"
        );
    }

    #[test]
    fn root_relative_query_and_dot_segments_resolve_like_a_browser() {
        assert_eq!(
            run(
                r#"<a href="/a"></a><a href="?p=2"></a><a href="./b"></a><a href="../../c/"></a>"#,
                "https://example.com/x/y/z.html"
            ),
            r#"<a href="https://example.com/a"></a><a href="https://example.com/x/y/z.html?p=2"></a><a href="https://example.com/x/y/b"></a><a href="https://example.com/c/"></a>"#
        );
    }

    #[test]
    fn leaves_absolute_scheme_fragment_and_template_values_alone() {
        let html = r##"<a href="https://other.test/x">a</a><a href="mailto:hi@example.com">b</a><a href="tel:+15550100">c</a><a href="#top">d</a><a href="javascript:void(0)">e</a><img src="data:image/gif;base64,R0lGOD"><a href="{{ post.url }}">f</a>"##;
        assert_eq!(run(html, "https://example.com/i.html"), html);
    }

    #[test]
    fn protocol_relative_takes_the_base_scheme_or_is_kept() {
        assert_eq!(
            run(r#"<img src="//cdn.test/a.png">"#, "https://example.com/i.html"),
            r#"<img src="https://cdn.test/a.png">"#
        );
        assert_eq!(
            absolutize(
                r#"<img src="//cdn.test/a.png">"#,
                "https://example.com/i.html",
                "common",
                true,
                "keep",
                false,
                false,
                "html"
            )
            .unwrap(),
            r#"<img src="//cdn.test/a.png">"#
        );
    }

    #[test]
    fn fragments_resolve_only_when_asked() {
        assert_eq!(
            absolutize(
                r##"<a href="#top">t</a>"##,
                "https://example.com/i.html",
                "common",
                true,
                "resolve",
                true,
                false,
                "html"
            )
            .unwrap(),
            r##"<a href="https://example.com/i.html#top">t</a>"##
        );
    }

    #[test]
    fn comments_and_raw_text_elements_are_never_touched() {
        let html = "<!-- <a href=\"a.html\"> -->\n<script>var s = '<img src=\"b.png\">';</script>\n<textarea><a href=\"c.html\"></textarea>\n<title>see d.html</title>\n";
        assert_eq!(run(html, "https://example.com/"), html);
    }

    #[test]
    fn common_scope_covers_srcset_poster_action_ping_and_meta_refresh() {
        let out = run(
            concat!(
                r#"<img srcset="a.png 1x, sub/b.png 2x" src="a.png">"#,
                r#"<video poster="p.jpg"></video>"#,
                r#"<form action="submit.php"></form>"#,
                r#"<a href="x" ping="t1.php t2.php">x</a>"#,
                r#"<meta http-equiv="refresh" content="5; url=next.html">"#,
            ),
            "https://example.com/dir/page.html",
        );
        assert_eq!(
            out,
            concat!(
                r#"<img srcset="https://example.com/dir/a.png 1x, https://example.com/dir/sub/b.png 2x" src="https://example.com/dir/a.png">"#,
                r#"<video poster="https://example.com/dir/p.jpg"></video>"#,
                r#"<form action="https://example.com/dir/submit.php"></form>"#,
                r#"<a href="https://example.com/dir/x" ping="https://example.com/dir/t1.php https://example.com/dir/t2.php">x</a>"#,
                r#"<meta http-equiv="refresh" content="5; url=https://example.com/dir/next.html">"#,
            )
        );
    }

    #[test]
    fn href_src_scope_ignores_the_extra_attributes() {
        assert_eq!(
            absolutize(
                r#"<video poster="p.jpg" src="v.mp4"></video>"#,
                "https://example.com/d/",
                "href-src",
                true,
                "resolve",
                false,
                false,
                "html"
            )
            .unwrap(),
            r#"<video poster="p.jpg" src="https://example.com/d/v.mp4"></video>"#
        );
    }

    #[test]
    fn all_scope_adds_cite_longdesc_and_manifest() {
        assert_eq!(
            absolutize(
                r#"<blockquote cite="src.html"><img longdesc="d.html" src="i.png"></blockquote>"#,
                "https://example.com/d/",
                "all",
                true,
                "resolve",
                false,
                false,
                "html"
            )
            .unwrap(),
            r#"<blockquote cite="https://example.com/d/src.html"><img longdesc="https://example.com/d/d.html" src="https://example.com/d/i.png"></blockquote>"#
        );
    }

    #[test]
    fn a_base_tag_wins_and_is_itself_made_absolute() {
        assert_eq!(
            run(
                r#"<base href="/assets/"><img src="logo.png">"#,
                "https://example.com/blog/post.html"
            ),
            r#"<base href="https://example.com/assets/"><img src="https://example.com/assets/logo.png">"#
        );
        assert_eq!(
            absolutize(
                r#"<base href="/assets/"><img src="logo.png">"#,
                "https://example.com/blog/post.html",
                "common",
                false,
                "resolve",
                false,
                false,
                "html"
            )
            .unwrap(),
            r#"<base href="https://example.com/assets/"><img src="https://example.com/blog/logo.png">"#
        );
    }

    #[test]
    fn css_urls_are_opt_in_and_string_aware() {
        let html = r#"<style>.a{background:url(bg.png)}@import "theme.css";</style><div style="background:url('hero.jpg')"></div>"#;
        assert_eq!(run(html, "https://example.com/d/"), html);
        assert_eq!(
            absolutize(
                html,
                "https://example.com/d/",
                "common",
                true,
                "resolve",
                false,
                true,
                "html"
            )
            .unwrap(),
            r#"<style>.a{background:url(https://example.com/d/bg.png)}@import "https://example.com/d/theme.css";</style><div style="background:url('https://example.com/d/hero.jpg')"></div>"#
        );
    }

    #[test]
    fn unquoted_values_are_rewritten_in_place() {
        assert_eq!(
            run("<a href=about.html>a</a>", "https://example.com/d/"),
            "<a href=https://example.com/d/about.html>a</a>"
        );
    }

    #[test]
    fn urls_listing_reports_every_decision() {
        let out = absolutize(
            "<a href=\"a.html\">a</a>\n<a href=\"#top\">t</a>\n<a href=\"https://o.test/\">o</a>\n",
            "https://example.com/d/",
            "common",
            true,
            "resolve",
            false,
            false,
            "urls",
        )
        .unwrap();
        assert_eq!(
            out,
            concat!(
                "line,tag,attribute,original,resolved,action\n",
                "1,a,href,a.html,https://example.com/d/a.html,rewritten\n",
                "2,a,href,#top,,kept:fragment\n",
                "3,a,href,https://o.test/,,kept:absolute\n",
            )
        );
    }

    #[test]
    fn report_counts_the_outcomes() {
        let out = absolutize(
            r##"<a href="a.html">a</a><a href="#top">t</a>"##,
            "https://example.com/d/",
            "common",
            true,
            "resolve",
            false,
            false,
            "report",
        )
        .unwrap();
        assert!(out.contains("urls_found,2\n"), "{out}");
        assert!(out.contains("urls_rewritten,1\n"), "{out}");
        assert!(out.contains("kept_fragment,1\n"), "{out}");
        assert!(
            out.contains("effective_base,https://example.com/d/\n"),
            "{out}"
        );
    }

    #[test]
    fn empty_input_is_an_error() {
        assert_eq!(
            absolutize("  ", "https://example.com/", "common", true, "resolve", false, false, "html")
                .unwrap_err(),
            "no HTML input"
        );
    }

    #[test]
    fn a_relative_or_unusable_base_is_an_error_that_says_what_was_expected() {
        let e = absolutize("<a href=\"a\"></a>", "example.com/x", "common", true, "resolve", false, false, "html")
            .unwrap_err();
        assert!(e.starts_with("invalid base URL 'example.com/x'"), "{e}");
        assert!(e.contains("expected an absolute URL"), "{e}");
        let e = absolutize("<a href=\"a\"></a>", "mailto:hi@example.com", "common", true, "resolve", false, false, "html")
            .unwrap_err();
        assert!(e.contains("cannot be a base"), "{e}");
        let e = absolutize("<a href=\"a\"></a>", "  ", "common", true, "resolve", false, false, "html")
            .unwrap_err();
        assert!(e.starts_with("no base URL"), "{e}");
    }

    #[test]
    fn unknown_option_values_name_the_valid_ones() {
        let e = absolutize("<a href=\"a\"></a>", "https://e.test/", "some", true, "resolve", false, false, "html")
            .unwrap_err();
        assert_eq!(
            e,
            "unknown attributes 'some' — use 'href-src', 'common' or 'all'"
        );
        let e = absolutize("<a href=\"a\"></a>", "https://e.test/", "common", true, "nope", false, false, "html")
            .unwrap_err();
        assert_eq!(
            e,
            "unknown protocol_relative 'nope' — use 'resolve' or 'keep'"
        );
        let e = absolutize("<a href=\"a\"></a>", "https://e.test/", "common", true, "resolve", false, false, "csv")
            .unwrap_err();
        assert_eq!(e, "unknown output 'csv' — use 'html', 'report' or 'urls'");
    }

    #[test]
    fn over_cap_input_is_refused() {
        let big = format!("<p>{}</p>", "x".repeat(MAX_INPUT_BYTES));
        let e = absolutize(&big, "https://e.test/", "common", true, "resolve", false, false, "html")
            .unwrap_err();
        assert!(e.contains("over the 5000000-byte limit"), "{e}");
    }

    #[test]
    fn a_greater_than_inside_an_attribute_does_not_end_the_tag() {
        assert_eq!(
            run(
                r#"<a title="a > b" href="x.html">x</a>"#,
                "https://example.com/d/"
            ),
            r#"<a title="a > b" href="https://example.com/d/x.html">x</a>"#
        );
    }

    #[test]
    fn entities_and_spaces_in_values_survive_resolution() {
        assert_eq!(
            run(
                r#"<a href="p.html?a=1&amp;b=2">x</a><a href="my file.html">y</a>"#,
                "https://example.com/d/"
            ),
            r#"<a href="https://example.com/d/p.html?a=1&amp;b=2">x</a><a href="https://example.com/d/my%20file.html">y</a>"#
        );
    }
}
