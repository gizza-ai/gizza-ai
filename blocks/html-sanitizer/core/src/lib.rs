//! gizza-ai/html-sanitizer core — pure compute, shared by the chat skill block
//! and the web page. No wafer/wasm-bindgen deps.
//!
//! Strips scripts, styles, event handlers, unsafe URI schemes, and disallowed
//! tags from pasted HTML using an ALLOWLIST model (only known-safe tags and
//! attributes survive) and returns either safe HTML markup or clean plain text.
//! A forgiving, dependency-free tokenizer (same shape as html-minifier) drives
//! it; plain-text mode runs the sanitized HTML through `nanohtml2text`.

/// Tags kept in `safe-html` output (open + close emitted, contents recursed).
/// Everything not here is either dropped-with-contents (dangerous) or unwrapped
/// (unknown/other — tag removed, inner content kept).
const ALLOWED_TAGS: &[&str] = &[
    "a", "abbr", "address", "article", "aside", "b", "bdi", "bdo", "blockquote", "br",
    "caption", "cite", "code", "col", "colgroup", "dd", "del", "details", "dfn", "div",
    "dl", "dt", "em", "figcaption", "figure", "footer", "h1", "h2", "h3", "h4", "h5",
    "h6", "header", "hgroup", "hr", "i", "img", "ins", "kbd", "li", "main", "mark",
    "nav", "ol", "p", "pre", "q", "rp", "rt", "ruby", "s", "samp", "section", "small",
    "span", "strong", "sub", "summary", "sup", "table", "tbody", "td", "tfoot", "th",
    "thead", "time", "tr", "u", "ul", "var", "wbr",
];

/// Tags dropped ALONG WITH their inner content — active or head-only elements
/// whose contents must never reach the output. A tag here that is void (see
/// `VOID`) or self-closing just drops the single tag.
const DANGEROUS_TAGS: &[&str] = &[
    "script", "style", "iframe", "object", "embed", "applet", "noscript", "template",
    "link", "meta", "base", "form", "input", "button", "select", "option", "optgroup",
    "textarea", "fieldset", "legend", "label", "output", "datalist", "keygen", "svg",
    "math", "canvas", "audio", "video", "source", "track", "param", "map", "area",
    "frame", "frameset", "noframes", "head", "title", "xml", "dialog", "marquee",
    "blink", "portal", "slot",
];

/// Void elements (no closing tag). A dangerous void element drops just its tag;
/// an allowed void element emits a single self-closing tag.
const VOID: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta",
    "param", "source", "track", "wbr", "keygen",
];

/// Attributes kept on allowed tags (plus the `aria-*` / `data-*` prefixes and,
/// conditionally, the URL and `style` attributes handled specially below).
const SAFE_ATTRS: &[&str] = &[
    "alt", "title", "class", "id", "name", "target", "rel", "width", "height",
    "colspan", "rowspan", "span", "start", "reversed", "type", "value", "datetime",
    "dir", "lang", "align", "valign", "scope", "headers", "abbr", "open", "download",
    "loading", "decoding", "sizes", "role", "color", "face", "border", "cellpadding",
    "cellspacing", "bgcolor", "nowrap", "char", "charoff", "axis", "summary", "label",
    "coords", "shape", "hreflang", "media", "sizes",
];

/// URL-bearing attributes whose value must pass the scheme allowlist.
const URL_ATTRS: &[&str] = &["href", "src", "srcset", "cite", "longdesc", "poster", "background"];

fn is_void(name: &str) -> bool {
    VOID.contains(&name)
}

/// Lowercased tag name from a raw tag string like `</Div ...` or `<img ...`.
fn tag_name(raw: &str) -> String {
    raw.trim_start_matches('<')
        .trim_start_matches('/')
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == ':')
        .collect::<String>()
        .to_ascii_lowercase()
}

/// Index just past the tag's closing `>`, respecting quoted attribute values.
fn scan_tag(b: &[u8], start: usize) -> usize {
    let mut j = start + 1;
    let mut quote = 0u8;
    while j < b.len() {
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
    b.len()
}

/// Decode the HTML character references that matter for defeating scheme
/// obfuscation (`&#106;avascript:`, `&#x6a;…`, `&colon;`, `&Tab;`). Not a full
/// entity table — text nodes are left encoded; this only feeds URL/scheme checks.
fn decode_refs(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let bytes = s.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == b'&' {
            // Numeric: &#DD; or &#xHH;
            if i + 2 < bytes.len() && bytes[i + 1] == b'#' {
                let hex = bytes[i + 2] == b'x' || bytes[i + 2] == b'X';
                let start = if hex { i + 3 } else { i + 2 };
                let mut j = start;
                while j < bytes.len() && bytes[j] != b';'
                    && ((hex && bytes[j].is_ascii_hexdigit())
                        || (!hex && bytes[j].is_ascii_digit()))
                {
                    j += 1;
                }
                if j > start {
                    let radix = if hex { 16 } else { 10 };
                    if let Ok(n) = u32::from_str_radix(&s[start..j], radix) {
                        if let Some(ch) = char::from_u32(n) {
                            out.push(ch);
                            i = if j < bytes.len() && bytes[j] == b';' { j + 1 } else { j };
                            continue;
                        }
                    }
                }
            } else if let Some((name, ch)) = [
                ("&colon;", ':'),
                ("&Tab;", '\t'),
                ("&NewLine;", '\n'),
                ("&amp;", '&'),
                ("&sol;", '/'),
            ]
            .into_iter()
            .find(|(name, _)| s[i..].starts_with(name))
            {
                out.push(ch);
                i += name.len();
                continue;
            }
        }
        // Default: copy one char.
        let ch = s[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// True if a URL value uses a safe scheme (or is relative / anchor). Entities
/// and embedded control/whitespace chars are stripped first so obfuscated
/// `javascript:` / `data:text/html` schemes are still caught.
fn url_is_safe(raw: &str, allow_data_image: bool) -> bool {
    let decoded = decode_refs(raw);
    let cleaned: String = decoded
        .chars()
        .filter(|c| !c.is_control() && !c.is_whitespace())
        .collect();
    let lower = cleaned.to_ascii_lowercase();
    if lower.is_empty() {
        return true;
    }
    let mut scheme = String::new();
    let mut has_scheme = false;
    for ch in lower.chars() {
        if ch == ':' {
            has_scheme = true;
            break;
        }
        if ch == '/' || ch == '?' || ch == '#' || ch == '\\' {
            break; // relative path / query / fragment — no scheme
        }
        if ch.is_ascii_alphanumeric() || ch == '+' || ch == '-' || ch == '.' {
            scheme.push(ch);
        } else {
            break; // an invalid scheme char means this isn't a scheme
        }
    }
    if !has_scheme || scheme.is_empty() {
        return true; // relative URL
    }
    match scheme.as_str() {
        "http" | "https" | "mailto" | "tel" | "ftp" | "ftps" | "sms" | "geo" | "bitcoin" => true,
        "data" => allow_data_image && lower.starts_with("data:image/"),
        _ => false,
    }
}

/// True if an inline `style` value is free of the obvious script vectors.
fn style_is_safe(value: &str) -> bool {
    let lower = decode_refs(value).to_ascii_lowercase().replace(char::is_whitespace, "");
    !(lower.contains("javascript:")
        || lower.contains("expression(")
        || lower.contains("vbscript:")
        || lower.contains("url(javascript")
        || lower.contains("url(data:text")
        || lower.contains("-moz-binding"))
}

struct Attr {
    name: String,
    value: Option<String>,
    quote: char,
}

/// Parse the attributes out of a raw open-tag string (`<name attr=... >`).
fn parse_attrs(raw: &str) -> Vec<Attr> {
    // Drop the leading `<name` and the trailing `>` / `/>`.
    let inner = raw
        .trim_start_matches('<')
        .trim_end_matches('>')
        .trim_end();
    let inner = inner.strip_suffix('/').unwrap_or(inner);
    // Skip the tag name.
    let after_name = inner
        .char_indices()
        .find(|(_, c)| c.is_ascii_whitespace())
        .map(|(idx, _)| &inner[idx..])
        .unwrap_or("");
    let b: Vec<char> = after_name.chars().collect();
    let mut attrs = Vec::new();
    let mut i = 0usize;
    let n = b.len();
    while i < n {
        while i < n && (b[i].is_ascii_whitespace() || b[i] == '/') {
            i += 1;
        }
        if i >= n {
            break;
        }
        // Attribute name.
        let start = i;
        while i < n && !b[i].is_ascii_whitespace() && b[i] != '=' && b[i] != '/' && b[i] != '>' {
            i += 1;
        }
        let name: String = b[start..i].iter().collect::<String>().to_ascii_lowercase();
        if name.is_empty() {
            i += 1;
            continue;
        }
        // Optional = value.
        while i < n && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if i < n && b[i] == '=' {
            i += 1;
            while i < n && b[i].is_ascii_whitespace() {
                i += 1;
            }
            if i < n && (b[i] == '"' || b[i] == '\'') {
                let q = b[i];
                i += 1;
                let vstart = i;
                while i < n && b[i] != q {
                    i += 1;
                }
                let value: String = b[vstart..i].iter().collect();
                if i < n {
                    i += 1; // consume closing quote
                }
                attrs.push(Attr { name, value: Some(value), quote: q });
            } else {
                let vstart = i;
                while i < n && !b[i].is_ascii_whitespace() && b[i] != '>' {
                    i += 1;
                }
                let value: String = b[vstart..i].iter().collect();
                attrs.push(Attr { name, value: Some(value), quote: '"' });
            }
        } else {
            attrs.push(Attr { name, value: None, quote: '"' });
        }
    }
    attrs
}

/// HTML-escape an attribute value for re-emission inside double quotes.
fn escape_attr(v: &str) -> String {
    v.replace('&', "&amp;").replace('"', "&quot;").replace('<', "&lt;")
}

struct Opts {
    allow_links: bool,
    allow_images: bool,
    allow_styles: bool,
    keep_classes: bool,
}

/// Build a sanitized open tag for an allowed element, keeping only safe
/// attributes with safe values.
fn sanitize_open_tag(name: &str, raw: &str, self_closing: bool, opts: &Opts) -> String {
    let mut out = String::with_capacity(raw.len());
    out.push('<');
    out.push_str(name);
    for attr in parse_attrs(raw) {
        let an = attr.name.as_str();
        // Event handlers and script-ish namespaced attrs never survive.
        if an.starts_with("on") || an == "xmlns" || an == "formaction" || an == "srcdoc" {
            continue;
        }
        let is_url = URL_ATTRS.contains(&an) || an == "xlink:href";
        let is_style = an == "style";
        let allowed = SAFE_ATTRS.contains(&an)
            || an.starts_with("aria-")
            || an.starts_with("data-")
            || is_url
            || is_style;
        if !allowed {
            continue;
        }
        if is_style && !opts.allow_styles {
            continue;
        }
        if (an == "class" || an == "id") && !opts.keep_classes {
            continue;
        }
        if is_url && !opts.allow_links {
            continue;
        }
        match &attr.value {
            Some(v) => {
                if is_url && !url_is_safe(v, opts.allow_images) {
                    continue;
                }
                if is_style && !style_is_safe(v) {
                    continue;
                }
                let q = if attr.quote == '\'' { '\'' } else { '"' };
                if q == '"' {
                    out.push_str(&format!(" {}=\"{}\"", an, escape_attr(v)));
                } else {
                    out.push_str(&format!(" {}='{}'", an, v.replace('\'', "&#39;")));
                }
            }
            None => {
                out.push(' ');
                out.push_str(an);
            }
        }
    }
    if self_closing || is_void(name) {
        out.push_str(if self_closing { " />" } else { ">" });
    } else {
        out.push('>');
    }
    out
}

/// Remove all dangerous/disallowed content from `html`, keeping only allowlisted
/// tags with safe attributes. Returns clean, safe HTML markup.
fn sanitize_html(html: &str, opts: &Opts, keep_comments: bool) -> String {
    let b = html.as_bytes();
    let lower = html.to_ascii_lowercase();
    let n = b.len();
    let mut i = 0usize;
    let mut out = String::with_capacity(n);

    while i < n {
        if b[i] == b'<' {
            // Comment.
            if html[i..].starts_with("<!--") {
                let end = html[i..].find("-->").map(|p| i + p + 3).unwrap_or(n);
                if keep_comments {
                    out.push_str(&html[i..end]);
                }
                i = end;
                continue;
            }
            // Doctype / declaration / processing instruction — drop.
            if i + 1 < n && (b[i + 1] == b'!' || b[i + 1] == b'?') {
                i = scan_tag(b, i);
                continue;
            }
            // A `<` not starting a real tag name — emit as escaped text.
            let next_is_name = i + 1 < n
                && (b[i + 1].is_ascii_alphabetic() || b[i + 1] == b'/');
            if !next_is_name {
                out.push_str("&lt;");
                i += 1;
                continue;
            }
            let end = scan_tag(b, i);
            let raw = &html[i..end];
            let name = tag_name(raw);
            let is_close = b.get(i + 1) == Some(&b'/');
            let self_closing = raw.trim_end().ends_with("/>");

            if name.is_empty() {
                i = end;
                continue;
            }

            // Images gated by allow_images: treat as disallowed when off.
            let image_off = name == "img" && !opts.allow_images;

            if is_close {
                // Emit a close only for allowed tags we actually opened.
                if ALLOWED_TAGS.contains(&name.as_str()) && !image_off {
                    out.push_str(&format!("</{name}>"));
                }
                i = end;
                continue;
            }

            if DANGEROUS_TAGS.contains(&name.as_str()) {
                if self_closing || is_void(&name) {
                    i = end; // drop the single tag
                } else {
                    // Drop the element AND its contents up to the matching close.
                    let close_pat = format!("</{name}");
                    let close_lt = lower[end..].find(&close_pat).map(|p| end + p).unwrap_or(n);
                    if close_lt < n {
                        i = scan_tag(b, close_lt);
                    } else {
                        i = n;
                    }
                }
                continue;
            }

            if ALLOWED_TAGS.contains(&name.as_str()) && !image_off {
                out.push_str(&sanitize_open_tag(&name, raw, self_closing, opts));
                i = end;
                continue;
            }

            // Unknown / disallowed-but-inert tag: drop the tag, keep content.
            i = end;
        } else {
            let start = i;
            while i < n && b[i] != b'<' {
                i += 1;
            }
            out.push_str(&html[start..i]);
        }
    }
    out
}

/// Collapse 3+ blank lines to a single blank line and trim, for plain-text mode.
fn tidy_text(raw: &str) -> String {
    let lf = raw.replace("\r\n", "\n").replace('\r', "\n");
    let mut out = String::with_capacity(lf.len());
    let mut newline_run = 0usize;
    for ch in lf.chars() {
        if ch == '\n' {
            newline_run += 1;
            if newline_run <= 2 {
                out.push(ch);
            }
        } else {
            newline_run = 0;
            out.push(ch);
        }
    }
    out.trim().to_string()
}

/// Sanitize `html`.
///
/// * `mode` — `"safe-html"` (default) returns cleaned HTML markup; `"plain-text"`
///   returns the visible text with all markup removed.
/// * `allow_links` — keep `href`/`src` on safe schemes (default true); when false,
///   URL attributes are dropped.
/// * `allow_images` — keep `<img>` with a safe `src` (default true); when false,
///   images are removed entirely.
/// * `allow_styles` — keep inline `style` attributes free of script vectors
///   (default false; `<style>` blocks are always removed).
/// * `keep_classes` — keep `class` and `id` attributes (default true); when false,
///   they are dropped for lean, CMS-ready markup (useful for pasted Word/Docs HTML).
/// * `keep_comments` — keep `<!-- … -->` comments (default false).
#[allow(clippy::too_many_arguments)]
pub fn sanitize(
    html: &str,
    mode: &str,
    allow_links: bool,
    allow_images: bool,
    allow_styles: bool,
    keep_classes: bool,
    keep_comments: bool,
) -> Result<String, String> {
    if html.trim().is_empty() {
        return Err("no HTML input: paste the HTML to sanitize".into());
    }
    let mode = if mode.trim().is_empty() { "safe-html" } else { mode.trim() };
    if mode != "safe-html" && mode != "plain-text" {
        return Err(format!(
            "invalid mode {mode:?}: expected \"safe-html\" or \"plain-text\""
        ));
    }
    let opts = Opts { allow_links, allow_images, allow_styles, keep_classes };
    // Always strip dangerous content first.
    let cleaned = sanitize_html(html, &opts, keep_comments && mode == "safe-html");
    if mode == "plain-text" {
        Ok(tidy_text(&nanohtml2text::html2text(&cleaned)))
    } else {
        Ok(cleaned.trim().to_string())
    }
}

/// Chat / CLI surface: sanitize `html` and return the cleaned output (safe HTML
/// markup or plain text) as a string. Thin wrapper over [`sanitize`] so the
/// block's `handle` and the page's `render` share one implementation.
#[allow(clippy::too_many_arguments)]
pub fn run(
    html: &str,
    mode: &str,
    allow_links: bool,
    allow_images: bool,
    allow_styles: bool,
    keep_classes: bool,
    keep_comments: bool,
) -> Result<String, String> {
    sanitize(
        html,
        mode,
        allow_links,
        allow_images,
        allow_styles,
        keep_classes,
        keep_comments,
    )
}

/// Page surface: identical to [`run`] — the sanitized output is already the
/// final text the page displays, so there is no separate human-readable form.
#[allow(clippy::too_many_arguments)]
pub fn render(
    html: &str,
    mode: &str,
    allow_links: bool,
    allow_images: bool,
    allow_styles: bool,
    keep_classes: bool,
    keep_comments: bool,
) -> Result<String, String> {
    run(
        html,
        mode,
        allow_links,
        allow_images,
        allow_styles,
        keep_classes,
        keep_comments,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn safe(html: &str) -> String {
        sanitize(html, "safe-html", true, true, false, true, false).unwrap()
    }

    #[test]
    fn strips_script_and_content() {
        let got = safe("<p>ok</p><script>alert('xss')</script><p>bye</p>");
        assert_eq!(got, "<p>ok</p><p>bye</p>");
        assert!(!got.contains("alert"), "script body dropped: {got:?}");
    }

    #[test]
    fn strips_style_block() {
        let got = safe("<style>p{color:red}</style><p>hi</p>");
        assert_eq!(got, "<p>hi</p>");
    }

    #[test]
    fn drops_event_handlers() {
        let got = safe(r#"<a href="https://x.test" onclick="steal()">go</a>"#);
        assert_eq!(got, r#"<a href="https://x.test">go</a>"#);
        assert!(!got.contains("onclick"), "no handler: {got:?}");
    }

    #[test]
    fn drops_javascript_uri() {
        let got = safe(r#"<a href="javascript:alert(1)">x</a>"#);
        assert_eq!(got, "<a>x</a>");
    }

    #[test]
    fn catches_entity_obfuscated_javascript_uri() {
        let got = safe(r#"<a href="&#106;avascript:alert(1)">x</a>"#);
        assert_eq!(got, "<a>x</a>", "obfuscated scheme dropped: {got:?}");
    }

    #[test]
    fn keeps_relative_and_anchor_links() {
        let got = safe(r##"<a href="/page?q=1">a</a><a href="#top">b</a>"##);
        assert_eq!(got, r##"<a href="/page?q=1">a</a><a href="#top">b</a>"##);
    }

    #[test]
    fn unwraps_unknown_tag_keeps_text() {
        let got = safe("<font size=7>big</font>");
        assert_eq!(got, "big");
    }

    #[test]
    fn removes_iframe_with_contents() {
        let got = safe(r#"<div><iframe src="//evil"></iframe>text</div>"#);
        assert_eq!(got, "<div>text</div>");
    }

    #[test]
    fn drops_inline_style_by_default_keeps_when_allowed() {
        let off = sanitize(r#"<p style="color:red">x</p>"#, "safe-html", true, true, false, true, false).unwrap();
        assert_eq!(off, "<p>x</p>");
        let on = sanitize(r#"<p style="color:red">x</p>"#, "safe-html", true, true, true, true, false).unwrap();
        assert_eq!(on, r#"<p style="color:red">x</p>"#);
    }

    #[test]
    fn allowed_style_still_strips_script_vector() {
        let on = sanitize(
            r#"<p style="background:url(javascript:alert(1))">x</p>"#,
            "safe-html", true, true, true, true, false,
        )
        .unwrap();
        assert_eq!(on, "<p>x</p>");
    }

    #[test]
    fn allow_images_false_removes_img() {
        let off = sanitize(r#"<p><img src="https://x.test/a.png" alt="a">t</p>"#, "safe-html", true, false, false, true, false).unwrap();
        assert_eq!(off, "<p>t</p>");
    }

    #[test]
    fn keep_classes_false_strips_class_and_id() {
        let on = safe(r#"<p class="MsoNormal" id="x">hi</p>"#);
        assert_eq!(on, r#"<p class="MsoNormal" id="x">hi</p>"#);
        let off = sanitize(r#"<p class="MsoNormal" id="x">hi</p>"#, "safe-html", true, true, false, false, false).unwrap();
        assert_eq!(off, "<p>hi</p>");
    }

    #[test]
    fn data_image_uri_kept_only_for_images() {
        let img = safe(r#"<img src="data:image/png;base64,AAAA" alt="x">"#);
        assert!(img.contains("data:image/png"), "data image kept: {img:?}");
        let a = safe(r#"<a href="data:text/html,<script>">x</a>"#);
        assert_eq!(a, "<a>x</a>");
    }

    #[test]
    fn comments_dropped_by_default_kept_when_asked() {
        let drop = safe("<p>a</p><!-- secret --><p>b</p>");
        assert_eq!(drop, "<p>a</p><p>b</p>");
        let keep = sanitize("<p>a</p><!-- note --><p>b</p>", "safe-html", true, true, false, true, true).unwrap();
        assert_eq!(keep, "<p>a</p><!-- note --><p>b</p>");
    }

    #[test]
    fn plain_text_mode_strips_all_markup_and_scripts() {
        let got = sanitize(
            "<h1>Title</h1><script>bad()</script><p>Hello <b>world</b>.</p>",
            "plain-text", true, true, false, false, false,
        )
        .unwrap();
        assert!(got.contains("Title"), "got: {got:?}");
        assert!(got.contains("Hello world."), "got: {got:?}");
        assert!(!got.contains('<'), "no tags: {got:?}");
        assert!(!got.contains("bad"), "no script body: {got:?}");
    }

    #[test]
    fn keeps_safe_tables_and_formatting() {
        let got = safe("<table><tr><td>1</td><td>2</td></tr></table><strong>x</strong>");
        assert_eq!(got, "<table><tr><td>1</td><td>2</td></tr></table><strong>x</strong>");
    }

    #[test]
    fn strips_svg_payload() {
        let got = safe(r#"<svg><script>alert(1)</script></svg><p>ok</p>"#);
        assert_eq!(got, "<p>ok</p>");
    }

    #[test]
    fn error_on_empty_input() {
        assert!(sanitize("   ", "safe-html", true, true, false, false, false).is_err());
    }

    #[test]
    fn error_on_bad_mode() {
        let e = sanitize("<p>x</p>", "markdown", true, true, false, false, false).unwrap_err();
        assert!(e.contains("invalid mode"), "got: {e}");
    }

    #[test]
    fn stray_lt_is_escaped() {
        let got = safe("a < b and c");
        assert_eq!(got, "a &lt; b and c");
    }
}
