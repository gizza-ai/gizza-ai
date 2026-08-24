//! gizza-ai/svg-to-data-uri core — turn SVG markup into an inline `data:` URI
//! and a ready-to-paste CSS/HTML/JSX snippet. Pure Rust, no I/O.
//!
//! This is deliberately SVG-*aware*, which is what separates it from the
//! generic `data-uri-encode` block:
//!
//! * the percent-encoding set is the MINIMAL one that is safe inside a quoted
//!   CSS `url("…")`, an HTML attribute and a JSX attribute — not RFC 3986's
//!   unreserved-only set. Escaping only the characters that actually break
//!   those contexts is what makes the URL form beat base64 on size for
//!   text-shaped SVG markup (base64 costs a flat ~33%);
//! * the markup can be minified first (XML declaration, DOCTYPE, comments,
//!   redundant whitespace) because those bytes are pure overhead in a URI;
//! * a missing root `xmlns` is injected, because an SVG data URI without it
//!   renders as nothing in `url()` — the single most common failure report.
//!
//! Both encodings are always measured so the caller can report which one wins.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;
use serde::Serialize;

/// The media type every SVG data URI carries.
pub const SVG_MIME: &str = "image/svg+xml";

/// The SVG namespace injected when the root element is missing `xmlns`.
pub const SVG_NS: &str = "http://www.w3.org/2000/svg";

/// Upper bound on the input markup. Data URIs are for icons and small
/// patterns; past ~1 MB of markup an external `.svg` file (cacheable, and not
/// re-parsed on every stylesheet load) is strictly better, and the encoded URI
/// would be unusable in practice.
pub const MAX_SVG_BYTES: usize = 1_000_000;

/// How the payload is encoded into the URI.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Encoding {
    /// Minimal percent-encoding — usually the smaller of the two for SVG.
    Url,
    /// Standard Base64 — larger, but opaque to any tooling that mangles markup.
    Base64,
}

impl Encoding {
    /// Parse the `encoding` param. Blank means the default (`url`).
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "url" | "url-encoded" | "percent" => Ok(Encoding::Url),
            "base64" | "b64" => Ok(Encoding::Base64),
            other => Err(format!(
                "encoding must be 'url' or 'base64' (got {other:?})"
            )),
        }
    }

    /// The canonical name echoed back in the structured result.
    pub fn name(self) -> &'static str {
        match self {
            Encoding::Url => "url",
            Encoding::Base64 => "base64",
        }
    }
}

/// Which snippet shape to emit as the primary result.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output {
    /// The bare `data:image/svg+xml,…` URI.
    Uri,
    /// A CSS `background-image` declaration.
    Css,
    /// CSS `mask-image` + the `-webkit-` prefixed twin.
    Mask,
    /// An HTML `<img>` tag.
    Img,
    /// A small JSX component wrapping an `<img>`.
    Jsx,
    /// A size report: both encodings measured, winner named.
    Compare,
}

impl Output {
    /// Parse the `output` param. Blank means the default (`uri`).
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "uri" | "data-uri" => Ok(Output::Uri),
            "css" | "background" | "background-image" => Ok(Output::Css),
            "mask" | "mask-image" => Ok(Output::Mask),
            "img" | "html" => Ok(Output::Img),
            "jsx" | "react" => Ok(Output::Jsx),
            "compare" | "size" => Ok(Output::Compare),
            other => Err(format!(
                "output must be one of uri, css, mask, img, jsx, compare (got {other:?})"
            )),
        }
    }
}

/// What to do with the double quotes that SVG attributes are written with.
///
/// The URI has to sit inside `url("…")` / `src="…"`, so a raw `"` in the
/// payload would close the wrapper early.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quotes {
    /// Rewrite attribute `"` to `'` — one byte each, and valid XML.
    Single,
    /// Percent-encode `"` as `%22` — leaves the markup byte-identical.
    Encode,
}

impl Quotes {
    /// Parse the `quotes` param. Blank means the default (`single`).
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "single" | "convert" => Ok(Quotes::Single),
            "encode" | "escape" | "percent" => Ok(Quotes::Encode),
            other => Err(format!(
                "quotes must be 'single' or 'encode' (got {other:?})"
            )),
        }
    }

    /// The canonical name echoed back in the structured result.
    pub fn name(self) -> &'static str {
        match self {
            Quotes::Single => "single",
            Quotes::Encode => "encode",
        }
    }
}

/// Everything the caller might want about one conversion.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SvgDataUri {
    /// The primary result — the snippet shape asked for by `output`.
    pub output: String,
    /// The `data:` URI on its own, in the requested encoding.
    pub data_uri: String,
    /// Encoding actually used: `"url"` or `"base64"`.
    pub encoding: String,
    /// Quote handling actually used: `"single"` or `"encode"`.
    pub quotes: String,
    /// Byte length of the markup exactly as it was supplied.
    pub original_bytes: usize,
    /// Byte length of the markup after minify + `xmlns` injection.
    pub encoded_bytes: usize,
    /// Length of the full `data:` URI in its URL-encoded form.
    pub url_length: usize,
    /// Length of the full `data:` URI in its Base64 form.
    pub base64_length: usize,
    /// Which encoding produced the shorter URI: `"url"`, `"base64"` or `"equal"`.
    pub smaller: String,
    /// Whether a root `xmlns` attribute had to be added.
    pub xmlns_added: bool,
}

/// Strip an XML prolog, DOCTYPE and comments, then collapse redundant
/// whitespace. Everything removed here is invisible in a rendered SVG but
/// costs bytes in every copy of the URI.
///
/// Whitespace between tags is dropped entirely and runs of whitespace inside a
/// tag collapse to one space. Text content therefore also gets collapsed —
/// which matters only for `<text>` elements relying on runs of spaces, so the
/// caller can switch minification off.
pub fn minify_svg(src: &str) -> String {
    collapse_whitespace(&strip_regions(src))
}

/// Remove `<?…?>`, `<!--…-->` and `<!…>` regions, preserving every other byte.
fn strip_regions(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut rest = src;
    while !rest.is_empty() {
        let next = rest.find('<');
        let Some(lt) = next else {
            out.push_str(rest);
            break;
        };
        out.push_str(&rest[..lt]);
        let tail = &rest[lt..];
        if let Some(after) = tail.strip_prefix("<?") {
            match after.find("?>") {
                Some(end) => {
                    rest = &after[end + 2..];
                    continue;
                }
                // Unterminated — keep it verbatim rather than eating the rest.
                None => {
                    out.push_str(tail);
                    break;
                }
            }
        }
        if let Some(after) = tail.strip_prefix("<!--") {
            match after.find("-->") {
                Some(end) => {
                    rest = &after[end + 3..];
                    continue;
                }
                None => {
                    out.push_str(tail);
                    break;
                }
            }
        }
        if tail.starts_with("<!") {
            let mut depth = 0usize;
            let mut consumed = None;
            for (off, ch) in tail.char_indices() {
                match ch {
                    '[' => depth += 1,
                    ']' => depth = depth.saturating_sub(1),
                    '>' if depth == 0 && off > 1 => {
                        consumed = Some(off + 1);
                        break;
                    }
                    _ => {}
                }
            }
            match consumed {
                Some(n) => {
                    rest = &tail[n..];
                    continue;
                }
                None => {
                    out.push_str(tail);
                    break;
                }
            }
        }
        // An ordinary tag — copy the `<` and continue scanning after it.
        out.push('<');
        rest = &tail[1..];
    }
    out
}

/// Drop whitespace that sits between tags and squeeze every other run of
/// whitespace down to a single space.
fn collapse_whitespace(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut chars = src.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch.is_whitespace() {
            // Consume the whole run, then decide whether it is load-bearing.
            while chars.peek().is_some_and(|c| c.is_whitespace()) {
                chars.next();
            }
            let between_tags = out.ends_with('>') && chars.peek() == Some(&'<');
            if !between_tags {
                out.push(' ');
            }
            continue;
        }
        out.push(ch);
    }
    out.trim().to_string()
}

/// Locate the root `<svg` start tag and return `(tag_start, tag_end_exclusive)`.
fn find_root_svg(src: &str) -> Option<(usize, usize)> {
    let mut search = 0usize;
    while let Some(rel) = src[search..].find("<svg") {
        let start = search + rel;
        let after = start + 4;
        // `<svg` must be followed by whitespace, `/` or `>` — not `<svgfoo`.
        let next = src[after..].chars().next();
        if matches!(next, Some(c) if c.is_whitespace() || c == '>' || c == '/') {
            let end = src[start..].find('>')? + start + 1;
            return Some((start, end));
        }
        search = after;
    }
    None
}

/// Add `xmlns="http://www.w3.org/2000/svg"` to the root element when absent.
/// Returns the markup and whether anything was inserted.
pub fn ensure_xmlns(src: &str) -> (String, bool) {
    let Some((start, end)) = find_root_svg(src) else {
        return (src.to_string(), false);
    };
    let tag = &src[start..end];
    // Look for an `xmlns` attribute, not `xmlns:xlink` — the default namespace
    // is the one that decides whether the image renders at all.
    let has_default_ns = tag
        .match_indices("xmlns")
        .any(|(off, _)| !tag[off + 5..].starts_with(':'));
    if has_default_ns {
        return (src.to_string(), false);
    }
    let mut out = String::with_capacity(src.len() + SVG_NS.len() + 10);
    out.push_str(&src[..start + 4]);
    out.push_str(&format!(" xmlns=\"{SVG_NS}\""));
    out.push_str(&src[start + 4..]);
    (out, true)
}

/// Percent-encode the characters that would break a quoted CSS `url("…")`, an
/// HTML attribute or a JSX attribute — and nothing else. Everything left
/// untouched (letters, digits, spaces, `/`, `=`, `:`, `.`, `,`, `-`, `+`, `;`,
/// `'`, `!`, `*`, `~`, `$`, `@`) is accepted verbatim by browsers inside a
/// quoted data URI, and keeping it readable is the whole size advantage over
/// base64.
fn percent_encode_svg(src: &str, quotes: Quotes) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut out = String::with_capacity(src.len() + src.len() / 8);
    let mut buf = [0u8; 4];
    for ch in src.chars() {
        let must_escape = matches!(
            ch,
            '%' | '#' | '<' | '>' | '?' | '[' | '\\' | ']' | '^' | '`' | '{' | '|' | '}' | '&'
        ) || ch.is_control()
            || (ch == '"' && quotes == Quotes::Encode);
        if !must_escape {
            out.push(ch);
            continue;
        }
        for &b in ch.encode_utf8(&mut buf).as_bytes() {
            out.push('%');
            out.push(HEX[(b >> 4) as usize] as char);
            out.push(HEX[(b & 0x0f) as usize] as char);
        }
    }
    out
}

/// Rewrite every `"` to `'`. SVG attributes are equally valid single-quoted,
/// and it saves two bytes per quote versus `%22`.
fn quotes_to_single(src: &str) -> String {
    src.replace('"', "'")
}

/// Build the URL-encoded form of the URI.
fn url_uri(markup: &str, quotes: Quotes) -> String {
    let prepared = match quotes {
        Quotes::Single => quotes_to_single(markup),
        Quotes::Encode => markup.to_string(),
    };
    format!("data:{SVG_MIME},{}", percent_encode_svg(&prepared, quotes))
}

/// Build the Base64 form of the URI.
fn base64_uri(markup: &str) -> String {
    format!("data:{SVG_MIME};base64,{}", B64.encode(markup.as_bytes()))
}

/// Render the size-comparison report.
fn compare_report(url_len: usize, b64_len: usize) -> String {
    let (winner, small, large) = match url_len.cmp(&b64_len) {
        std::cmp::Ordering::Less => ("URL-encoded", url_len, b64_len),
        std::cmp::Ordering::Greater => ("Base64", b64_len, url_len),
        std::cmp::Ordering::Equal => ("tie", url_len, b64_len),
    };
    let mut out = String::new();
    out.push_str(&format!("URL-encoded : {url_len} characters\n"));
    out.push_str(&format!("Base64      : {b64_len} characters\n"));
    if winner == "tie" {
        out.push_str("Smaller     : identical length");
    } else {
        let pct = ((large - small) as f64 / large as f64) * 100.0;
        out.push_str(&format!(
            "Smaller     : {winner} by {} characters ({:.1}%)",
            large - small,
            pct
        ));
    }
    out
}

/// Convert SVG markup into a data URI plus the requested snippet.
///
/// `svg` is the raw markup. `minify` strips the prolog/DOCTYPE/comments and
/// collapses whitespace; `add_xmlns` injects the default namespace when the
/// root element lacks it.
pub fn convert(
    svg: &str,
    encoding: Encoding,
    output: Output,
    quotes: Quotes,
    minify: bool,
    add_xmlns: bool,
) -> Result<SvgDataUri, String> {
    let original_bytes = svg.len();
    let trimmed = svg.trim();
    if trimmed.is_empty() {
        return Err(
            "svg is empty — paste SVG markup, e.g. '<svg viewBox=\"0 0 16 16\">…</svg>'"
                .to_string(),
        );
    }
    if original_bytes > MAX_SVG_BYTES {
        return Err(format!(
            "svg is {original_bytes} bytes, over the {MAX_SVG_BYTES}-byte limit — inline data URIs \
             are for icons and small patterns; serve anything larger as a cacheable .svg file"
        ));
    }

    let mut markup = if minify {
        minify_svg(trimmed)
    } else {
        trimmed.to_string()
    };

    if find_root_svg(&markup).is_none() {
        return Err(
            "no root <svg> element found — expected markup containing '<svg …>…</svg>', got \
             something else (if you have a data: URI already, use data-uri-decode instead)"
                .to_string(),
        );
    }

    let mut xmlns_added = false;
    if add_xmlns {
        let (with_ns, added) = ensure_xmlns(&markup);
        markup = with_ns;
        xmlns_added = added;
    }

    let url_form = url_uri(&markup, quotes);
    let base64_form = base64_uri(&markup);
    let url_length = url_form.chars().count();
    let base64_length = base64_form.chars().count();

    let data_uri = match encoding {
        Encoding::Url => url_form.clone(),
        Encoding::Base64 => base64_form.clone(),
    };

    let snippet = match output {
        Output::Uri => data_uri.clone(),
        Output::Css => format!("background-image: url(\"{data_uri}\");"),
        Output::Mask => {
            format!("-webkit-mask-image: url(\"{data_uri}\");\nmask-image: url(\"{data_uri}\");")
        }
        Output::Img => format!("<img src=\"{data_uri}\" alt=\"\" />"),
        Output::Jsx => {
            format!("export const Icon = () => (\n  <img src=\"{data_uri}\" alt=\"\" />\n);")
        }
        Output::Compare => compare_report(url_length, base64_length),
    };

    let smaller = match url_length.cmp(&base64_length) {
        std::cmp::Ordering::Less => "url",
        std::cmp::Ordering::Greater => "base64",
        std::cmp::Ordering::Equal => "equal",
    };

    Ok(SvgDataUri {
        output: snippet,
        data_uri,
        encoding: encoding.name().to_string(),
        quotes: quotes.name().to_string(),
        original_bytes,
        encoded_bytes: markup.len(),
        url_length,
        base64_length,
        smaller: smaller.to_string(),
        xmlns_added,
    })
}

/// String-in / string-out entry point shared by the chat block and the CLI.
/// Blank option strings fall back to the documented defaults.
#[allow(clippy::too_many_arguments)]
pub fn run(
    svg: &str,
    encoding: &str,
    output: &str,
    quotes: &str,
    minify: bool,
    add_xmlns: bool,
) -> Result<SvgDataUri, String> {
    convert(
        svg,
        Encoding::parse(encoding)?,
        Output::parse(output)?,
        Quotes::parse(quotes)?,
        minify,
        add_xmlns,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const ICON: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 16 16"><circle cx="8" cy="8" r="7" fill="#0af"/></svg>"##;

    #[test]
    fn happy_path_url_encoded_uri() {
        let r = run(ICON, "url", "uri", "single", true, true).unwrap();
        assert_eq!(
            r.data_uri,
            "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 16 16'%3E%3Ccircle cx='8' cy='8' r='7' fill='%230af'/%3E%3C/svg%3E"
        );
        assert_eq!(r.encoding, "url");
        assert_eq!(r.smaller, "url");
        assert!(!r.xmlns_added);
        // The URL form must actually beat base64 on a text-shaped icon —
        // that is the whole reason it is the default.
        assert!(r.url_length < r.base64_length);
    }

    #[test]
    fn base64_form_is_standard_and_prefixed() {
        let r = run("<svg/>", "base64", "uri", "single", true, false).unwrap();
        // base64("<svg/>") = PHN2Zy8+
        assert_eq!(r.data_uri, "data:image/svg+xml;base64,PHN2Zy8+");
    }

    #[test]
    fn error_on_empty_input() {
        let err = run("   \n ", "url", "uri", "single", true, true).unwrap_err();
        assert!(err.contains("empty"), "{err}");
    }

    #[test]
    fn error_when_no_root_svg_element() {
        let err = run("<div>hello</div>", "url", "uri", "single", true, true).unwrap_err();
        assert!(err.contains("no root <svg>"), "{err}");
    }

    #[test]
    fn error_on_unknown_enum_values() {
        assert!(run(ICON, "hex", "uri", "single", true, true)
            .unwrap_err()
            .contains("encoding must be"));
        assert!(run(ICON, "url", "pdf", "single", true, true)
            .unwrap_err()
            .contains("output must be"));
        assert!(run(ICON, "url", "uri", "curly", true, true)
            .unwrap_err()
            .contains("quotes must be"));
    }

    #[test]
    fn error_over_size_cap() {
        let big = format!("<svg>{}</svg>", "x".repeat(MAX_SVG_BYTES));
        let err = run(&big, "url", "uri", "single", true, true).unwrap_err();
        assert!(err.contains("over the"), "{err}");
    }

    #[test]
    fn minify_strips_prolog_doctype_and_comments() {
        let src = "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<!DOCTYPE svg PUBLIC \"-//W3C//DTD SVG 1.1//EN\" \"x.dtd\">\n<!-- an icon -->\n<svg viewBox=\"0 0 2 2\">\n  <rect width=\"2\" height=\"2\"/>\n</svg>";
        let out = minify_svg(src);
        assert_eq!(
            out,
            "<svg viewBox=\"0 0 2 2\"><rect width=\"2\" height=\"2\"/></svg>"
        );
    }

    #[test]
    fn minify_off_keeps_the_markup_verbatim() {
        let src = "<svg viewBox=\"0 0 2 2\">\n  <rect/>\n</svg>";
        let r = run(src, "base64", "uri", "single", false, false).unwrap();
        let expected = format!("data:image/svg+xml;base64,{}", B64.encode(src.as_bytes()));
        assert_eq!(r.data_uri, expected);
    }

    #[test]
    fn xmlns_is_injected_when_missing() {
        let r = run(
            "<svg viewBox=\"0 0 1 1\"/>",
            "url",
            "uri",
            "encode",
            true,
            true,
        )
        .unwrap();
        assert!(r.xmlns_added);
        assert!(r
            .data_uri
            .contains("xmlns=%22http://www.w3.org/2000/svg%22"));
    }

    #[test]
    fn xmlns_injection_can_be_disabled() {
        let r = run(
            "<svg viewBox=\"0 0 1 1\"/>",
            "url",
            "uri",
            "single",
            true,
            false,
        )
        .unwrap();
        assert!(!r.xmlns_added);
        assert!(!r.data_uri.contains("xmlns"));
    }

    #[test]
    fn xmlns_xlink_alone_does_not_count_as_the_default_namespace() {
        let (out, added) = ensure_xmlns("<svg xmlns:xlink=\"http://www.w3.org/1999/xlink\"/>");
        assert!(added);
        assert!(out.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\" xmlns:xlink="));
    }

    #[test]
    fn quotes_encode_leaves_double_quotes_as_percent_22() {
        let r = run(
            "<svg viewBox=\"0 0 1 1\"/>",
            "url",
            "uri",
            "encode",
            true,
            false,
        )
        .unwrap();
        assert!(
            r.data_uri.contains("viewBox=%220 0 1 1%22"),
            "{}",
            r.data_uri
        );
        assert!(!r.data_uri.contains('\''));
    }

    #[test]
    fn hash_and_angle_brackets_and_ampersands_are_escaped() {
        let r = run(
            "<svg fill=\"#fff\"><text>a &amp; b</text></svg>",
            "url",
            "uri",
            "single",
            true,
            false,
        )
        .unwrap();
        assert!(r.data_uri.contains("%23fff"));
        assert!(r.data_uri.contains("%26amp;"));
        assert!(!r.data_uri[5..].contains('<'));
        assert!(!r.data_uri.contains('#'));
    }

    #[test]
    fn snippet_shapes() {
        let css = run("<svg/>", "base64", "css", "single", true, false).unwrap();
        assert_eq!(
            css.output,
            "background-image: url(\"data:image/svg+xml;base64,PHN2Zy8+\");"
        );

        let mask = run("<svg/>", "base64", "mask", "single", true, false).unwrap();
        assert_eq!(
            mask.output,
            "-webkit-mask-image: url(\"data:image/svg+xml;base64,PHN2Zy8+\");\nmask-image: url(\"data:image/svg+xml;base64,PHN2Zy8+\");"
        );

        let img = run("<svg/>", "base64", "img", "single", true, false).unwrap();
        assert_eq!(
            img.output,
            "<img src=\"data:image/svg+xml;base64,PHN2Zy8+\" alt=\"\" />"
        );

        let jsx = run("<svg/>", "base64", "jsx", "single", true, false).unwrap();
        assert_eq!(
            jsx.output,
            "export const Icon = () => (\n  <img src=\"data:image/svg+xml;base64,PHN2Zy8+\" alt=\"\" />\n);"
        );
    }

    #[test]
    fn compare_output_names_the_winner() {
        let r = run(ICON, "url", "compare", "single", true, true).unwrap();
        assert!(r.output.starts_with("URL-encoded : "), "{}", r.output);
        assert!(
            r.output.contains("Smaller     : URL-encoded by "),
            "{}",
            r.output
        );
        assert!(r
            .output
            .contains(&format!("{} characters", r.base64_length)));
    }

    #[test]
    fn aliases_are_accepted_for_every_enum() {
        assert_eq!(Encoding::parse("URL-encoded").unwrap(), Encoding::Url);
        assert_eq!(Encoding::parse("B64").unwrap(), Encoding::Base64);
        assert_eq!(Output::parse("background-image").unwrap(), Output::Css);
        assert_eq!(Output::parse("React").unwrap(), Output::Jsx);
        assert_eq!(Quotes::parse("").unwrap(), Quotes::Single);
    }

    #[test]
    fn multibyte_content_survives_both_encodings() {
        let src = "<svg><text>héllo ✓</text></svg>";
        let url = run(src, "url", "uri", "single", true, false).unwrap();
        assert!(url.data_uri.contains("héllo ✓"), "{}", url.data_uri);
        let b64 = run(src, "base64", "uri", "single", true, false).unwrap();
        let expected = format!("data:image/svg+xml;base64,{}", B64.encode(src.as_bytes()));
        assert_eq!(b64.data_uri, expected);
    }

    #[test]
    fn byte_counts_are_reported() {
        let src = "  <svg viewBox=\"0 0 1 1\">\n  <rect/>\n</svg>  ";
        let r = run(src, "url", "uri", "single", true, false).unwrap();
        assert_eq!(r.original_bytes, src.len());
        assert_eq!(
            r.encoded_bytes,
            "<svg viewBox=\"0 0 1 1\"><rect/></svg>".len()
        );
        assert!(r.encoded_bytes < r.original_bytes);
    }
}
