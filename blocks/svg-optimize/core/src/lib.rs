//! gizza-ai/svg-optimize core — minify and clean up SVG markup (SVGO-style),
//! dependency-free. A forgiving, quote-aware tokenizer that walks the SVG and:
//!   * strips comments `<!-- … -->`, the `<?xml … ?>` declaration and `<!DOCTYPE …>`
//!   * drops editor/metadata elements (`<metadata>`, `<title>`, `<desc>`, `<defs>`
//!     only when empty) and namespaced editor cruft (`<sodipodi:*>`, `<inkscape:*>`)
//!   * removes editor attributes (`xmlns:inkscape`, `xmlns:sodipodi`, `inkscape:*`,
//!     `sodipodi:*`) and the (optional) `id`/`class` attributes when requested
//!   * collapses indentation/whitespace between tags and inside tags
//!   * preserves the verbatim contents of `<script>`, `<style>` and `<text>`/`<tspan>`
//!     (whitespace there can be visually significant)
//!
//! It is intentionally conservative — it never rewrites path data or numbers, so
//! the rendered image is unchanged; it only removes safe-to-remove cruft.

/// Elements whose *contents* must be preserved verbatim.
const RAW: &[&str] = &["script", "style"];
/// Elements whose text content can be visually significant (don't trim text runs).
const TEXTUAL: &[&str] = &["text", "tspan", "textpath", "textPath", "title", "desc"];
/// Editor-only elements that are always safe to drop (with their subtree).
const DROP_EL: &[&str] = &["metadata", "sodipodi:namedview", "rdf:rdf", "rdf:RDF"];
/// Editor-only namespace prefixes whose attributes are dropped.
const EDITOR_ATTR_PREFIX: &[&str] = &["inkscape:", "sodipodi:"];
/// Editor-only xmlns declarations dropped from the root.
const EDITOR_XMLNS: &[&str] = &[
    "xmlns:inkscape",
    "xmlns:sodipodi",
    "xmlns:rdf",
    "xmlns:cc",
    "xmlns:dc",
];

/// Options controlling how aggressive the optimization is.
#[derive(Clone, Copy)]
pub struct Options {
    /// Strip `<!-- … -->` comments (default true).
    pub remove_comments: bool,
    /// Strip editor metadata elements + attributes (default true).
    pub remove_metadata: bool,
    /// Strip `id` and `class` attributes from every element (default false —
    /// they may be referenced by CSS/JS/`<use>`).
    pub remove_ids: bool,
    /// Remove `width`/`height` from the root `<svg>` when a `viewBox` is present
    /// so the SVG scales responsively (default false).
    pub remove_dimensions: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            remove_comments: true,
            remove_metadata: true,
            remove_ids: false,
            remove_dimensions: false,
        }
    }
}

fn tag_name(raw: &str) -> String {
    raw.trim_start_matches('<')
        .trim_start_matches('/')
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == ':' || *c == '_')
        .collect::<String>()
        .to_ascii_lowercase()
}

fn is_raw(name: &str) -> bool {
    RAW.contains(&name)
}
fn is_textual(name: &str) -> bool {
    TEXTUAL.iter().any(|t| t.eq_ignore_ascii_case(name))
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

/// Split a raw tag (`<svg a="1" b='2' />`) into (name, attrs, self_closing).
/// `attrs` is the text between the name and the trailing `/?>`.
fn split_tag(raw: &str) -> (String, &str, bool) {
    let inner = raw
        .trim_start_matches('<')
        .trim_end_matches('>')
        .trim_end();
    let self_closing = inner.ends_with('/');
    let inner = inner.trim_end_matches('/').trim_end();
    // Skip the name (and a leading '/').
    let mut idx = 0;
    let bytes = inner.as_bytes();
    if idx < bytes.len() && bytes[idx] == b'/' {
        idx += 1;
    }
    while idx < bytes.len() {
        let c = bytes[idx];
        if c.is_ascii_alphanumeric() || c == b'-' || c == b':' || c == b'_' {
            idx += 1;
        } else {
            break;
        }
    }
    let attrs = inner[idx..].trim_start();
    (tag_name(raw), attrs, self_closing)
}

/// Parse `attrs` text into (name, value-with-quotes) pairs, quote-aware.
fn parse_attrs(attrs: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let b = attrs.as_bytes();
    let mut i = 0;
    while i < b.len() {
        // skip whitespace
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if i >= b.len() {
            break;
        }
        let name_start = i;
        while i < b.len() && b[i] != b'=' && !b[i].is_ascii_whitespace() {
            i += 1;
        }
        let name = attrs[name_start..i].to_string();
        // skip ws
        while i < b.len() && b[i].is_ascii_whitespace() {
            i += 1;
        }
        if i < b.len() && b[i] == b'=' {
            i += 1;
            while i < b.len() && b[i].is_ascii_whitespace() {
                i += 1;
            }
            let val_start = i;
            if i < b.len() && (b[i] == b'"' || b[i] == b'\'') {
                let q = b[i];
                i += 1;
                while i < b.len() && b[i] != q {
                    i += 1;
                }
                if i < b.len() {
                    i += 1;
                } // closing quote
            } else {
                while i < b.len() && !b[i].is_ascii_whitespace() {
                    i += 1;
                }
            }
            out.push((name, attrs[val_start..i].to_string()));
        } else {
            // boolean attribute
            out.push((name, String::new()));
        }
    }
    out
}

fn attr_dropped(name: &str, opts: &Options, is_root: bool) -> bool {
    let lname = name.to_ascii_lowercase();
    if opts.remove_metadata {
        if EDITOR_ATTR_PREFIX.iter().any(|p| lname.starts_with(p)) {
            return true;
        }
        if is_root && EDITOR_XMLNS.iter().any(|x| lname == *x) {
            return true;
        }
    }
    if opts.remove_ids && (lname == "id" || lname == "class") {
        return true;
    }
    // Note: width/height removal is handled in rebuild_tag (guarded on viewBox),
    // not here.
    false
}

/// Rebuild a tag from its parts, dropping editor attrs and normalizing spacing.
/// Returns None if the whole tag should be dropped (currently never).
fn rebuild_tag(raw: &str, opts: &Options, is_root: bool, has_viewbox: bool) -> String {
    let (name, attrs, self_closing) = split_tag(raw);
    let is_closing = raw.starts_with("</");
    if is_closing {
        return format!("</{}>", name);
    }
    let parsed = parse_attrs(attrs);
    // Only strip dimensions when a viewBox exists (else the image would lose its size).
    let allow_dims = opts.remove_dimensions && is_root && has_viewbox;
    let mut kept: Vec<String> = Vec::new();
    for (an, av) in parsed {
        let lname = an.to_ascii_lowercase();
        let drop = (allow_dims && is_root && (lname == "width" || lname == "height"))
            || attr_dropped(&an, opts, is_root);
        if drop {
            continue;
        }
        if av.is_empty() {
            kept.push(an);
        } else {
            kept.push(format!("{}={}", an, av));
        }
    }
    // Render name with its original case for the *name portion*; we lowercased
    // tag_name for matching but SVG element names are conventionally lowercase
    // (and case matters for some, e.g. textPath/clipPath). Recover original.
    let orig_name = orig_tag_name(raw);
    let mut s = String::with_capacity(raw.len());
    s.push('<');
    s.push_str(&orig_name);
    for a in &kept {
        s.push(' ');
        s.push_str(a);
    }
    if self_closing {
        s.push_str("/>");
    } else {
        s.push('>');
    }
    let _ = name;
    s
}

/// Original-case tag name (preserves camelCase like clipPath/linearGradient).
fn orig_tag_name(raw: &str) -> String {
    raw.trim_start_matches('<')
        .trim_start_matches('/')
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-' || *c == ':' || *c == '_')
        .collect()
}

/// Collapse runs of whitespace in a text run to single spaces; fully trim if the
/// run is only whitespace.
fn collapse_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_ws = false;
    for c in s.chars() {
        if c.is_ascii_whitespace() {
            if !prev_ws {
                out.push(' ');
            }
            prev_ws = true;
        } else {
            out.push(c);
            prev_ws = false;
        }
    }
    out
}

/// Optimize an SVG document. Returns an error on empty input or input that does
/// not look like SVG (no `<svg` element found).
pub fn optimize(input: &str, opts: &Options) -> Result<String, String> {
    if input.trim().is_empty() {
        return Err("input is empty".into());
    }
    let lower = input.to_ascii_lowercase();
    if !lower.contains("<svg") {
        return Err("input does not look like SVG (no <svg> element found)".into());
    }
    // Does the root carry a viewBox? (cheap pre-scan, case-insensitive).
    let has_viewbox = lower.contains("viewbox");

    let b = input.as_bytes();
    let mut out = String::with_capacity(input.len());
    let mut i = 0;
    // Stack of currently-open element names (lowercased) for raw/textual context
    // and for dropping editor subtrees.
    let mut stack: Vec<String> = Vec::new();
    // When >0, we're inside a dropped editor subtree — emit nothing.
    let mut drop_depth: usize = 0;
    let mut seen_root = false;

    while i < b.len() {
        if b[i] == b'<' {
            // Comment?
            if input[i..].starts_with("<!--") {
                if let Some(end) = input[i..].find("-->") {
                    let full = i + end + 3;
                    if drop_depth == 0 && !opts.remove_comments {
                        out.push_str(&input[i..full]);
                    }
                    i = full;
                    continue;
                } else {
                    i = b.len();
                    continue;
                }
            }
            // XML declaration <?xml ... ?> — always dropped.
            if input[i..].starts_with("<?") {
                if let Some(end) = input[i..].find("?>") {
                    i += end + 2;
                } else {
                    i = b.len();
                }
                continue;
            }
            // DOCTYPE / CDATA-ish <!... > — drop DOCTYPE, keep CDATA verbatim.
            if input[i..].starts_with("<![CDATA[") {
                if let Some(end) = input[i..].find("]]>") {
                    let full = i + end + 3;
                    if drop_depth == 0 {
                        out.push_str(&input[i..full]);
                    }
                    i = full;
                    continue;
                } else {
                    i = b.len();
                    continue;
                }
            }
            if input[i..].starts_with("<!") {
                let end = scan_tag(b, i);
                i = end; // drop DOCTYPE and other declarations
                continue;
            }
            // Regular tag.
            let end = scan_tag(b, i);
            let raw = &input[i..end];
            let name = tag_name(raw);
            let is_closing = raw.starts_with("</");
            let self_closing = raw.trim_end().ends_with("/>");

            if is_closing {
                // Are we closing a dropped subtree?
                if drop_depth > 0 {
                    // Pop matching dropped element.
                    if let Some(top) = stack.last() {
                        if *top == name {
                            stack.pop();
                            drop_depth -= 1;
                        }
                    }
                    i = end;
                    continue;
                }
                if let Some(pos) = stack.iter().rposition(|n| *n == name) {
                    stack.truncate(pos);
                }
                out.push_str(&format!("</{}>", orig_tag_name(raw)));
                i = end;
                continue;
            }

            // Opening (or self-closing) tag.
            if drop_depth > 0 {
                if !self_closing {
                    stack.push(name);
                    drop_depth += 1;
                }
                i = end;
                continue;
            }

            // Should this whole element (subtree) be dropped?
            let drop_this = opts.remove_metadata && DROP_EL.contains(&name.as_str());
            if drop_this {
                if !self_closing {
                    stack.push(name);
                    drop_depth += 1;
                }
                i = end;
                continue;
            }

            let is_root = name == "svg" && !seen_root;
            if is_root {
                seen_root = true;
            }
            let rebuilt = rebuild_tag(raw, opts, is_root, has_viewbox);
            out.push_str(&rebuilt);
            if !self_closing {
                stack.push(name);
            }
            i = end;
            continue;
        }

        // Text run up to the next '<'.
        let next = input[i..].find('<').map(|p| i + p).unwrap_or(b.len());
        let text = &input[i..next];
        i = next;
        if drop_depth > 0 {
            continue;
        }
        let in_raw = stack.last().map(|n| is_raw(n)).unwrap_or(false);
        let in_textual = stack.last().map(|n| is_textual(n)).unwrap_or(false);
        if in_raw {
            out.push_str(text);
        } else if in_textual {
            // Preserve internal spacing but drop leading/trailing pure-ws runs
            // only if entirely whitespace (keep significant spaces).
            if text.trim().is_empty() {
                // Inside <text>, collapse to nothing between child tags.
                continue;
            }
            out.push_str(text);
        } else {
            let collapsed = collapse_text(text);
            // Drop a run that's purely whitespace between tags.
            if collapsed.trim().is_empty() {
                continue;
            }
            out.push_str(collapsed.trim());
        }
    }

    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn collapses_whitespace_and_strips_comments() {
        let svg = "<svg xmlns=\"http://www.w3.org/2000/svg\">\n  <!-- a comment -->\n  <rect x=\"0\" y=\"0\"/>\n</svg>";
        let out = optimize(svg, &Options::default()).unwrap();
        assert!(!out.contains("<!--"), "comment removed: {out}");
        assert!(!out.contains('\n'), "newlines collapsed: {out}");
        assert!(out.contains("<rect x=\"0\" y=\"0\"/>"), "rect kept: {out}");
    }

    #[test]
    fn strips_xml_decl_and_doctype() {
        let svg = "<?xml version=\"1.0\"?>\n<!DOCTYPE svg PUBLIC \"-//W3C//DTD SVG 1.1//EN\" \"x.dtd\">\n<svg xmlns=\"http://www.w3.org/2000/svg\"><rect/></svg>";
        let out = optimize(svg, &Options::default()).unwrap();
        assert!(!out.contains("<?xml"), "xml decl removed: {out}");
        assert!(!out.contains("DOCTYPE"), "doctype removed: {out}");
        assert!(out.starts_with("<svg"), "starts with svg: {out}");
    }

    #[test]
    fn drops_editor_metadata_and_attrs() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:inkscape="http://www.inkscape.org/namespaces/inkscape" inkscape:version="1.0"><metadata><foo>x</foo></metadata><sodipodi:namedview/><rect sodipodi:type="rect"/></svg>"#;
        let out = optimize(svg, &Options::default()).unwrap();
        assert!(!out.contains("metadata"), "metadata dropped: {out}");
        assert!(!out.contains("inkscape"), "inkscape attrs dropped: {out}");
        assert!(!out.contains("sodipodi"), "sodipodi dropped: {out}");
        assert!(out.contains("<rect/>"), "clean rect kept: {out}");
    }

    #[test]
    fn remove_ids_option() {
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg"><rect id="r1" class="box" x="0"/></svg>"#;
        let mut opts = Options::default();
        opts.remove_ids = true;
        let out = optimize(svg, &opts).unwrap();
        assert!(!out.contains("id="), "id removed: {out}");
        assert!(!out.contains("class="), "class removed: {out}");
        assert!(out.contains("x=\"0\""), "x kept: {out}");
    }

    #[test]
    fn remove_dimensions_only_with_viewbox() {
        let with_vb = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100" viewBox="0 0 100 100"><rect/></svg>"#;
        let mut opts = Options::default();
        opts.remove_dimensions = true;
        let out = optimize(with_vb, &opts).unwrap();
        assert!(!out.contains("width="), "width removed when viewBox present: {out}");
        assert!(out.contains("viewBox"), "viewBox kept: {out}");

        let no_vb = r#"<svg xmlns="http://www.w3.org/2000/svg" width="100" height="100"><rect/></svg>"#;
        let out2 = optimize(no_vb, &opts).unwrap();
        assert!(out2.contains("width="), "width KEPT when no viewBox: {out2}");
    }

    #[test]
    fn preserves_style_and_script_verbatim() {
        let svg = "<svg xmlns=\"http://www.w3.org/2000/svg\"><style>\n  .a {\n    fill: red;\n  }\n</style><rect class=\"a\"/></svg>";
        let out = optimize(svg, &Options::default()).unwrap();
        assert!(out.contains("fill: red;"), "style content kept: {out}");
        assert!(out.contains("\n    fill"), "style whitespace verbatim: {out}");
    }

    #[test]
    fn keep_comments_when_disabled() {
        let svg = "<svg xmlns=\"http://www.w3.org/2000/svg\"><!-- keep me --><rect/></svg>";
        let mut opts = Options::default();
        opts.remove_comments = false;
        let out = optimize(svg, &opts).unwrap();
        assert!(out.contains("<!-- keep me -->"), "comment kept: {out}");
    }

    #[test]
    fn errors_on_empty() {
        assert!(optimize("   ", &Options::default()).is_err());
    }

    #[test]
    fn errors_on_non_svg() {
        assert!(optimize("<html><body>no svg here</body></html>", &Options::default()).is_err());
    }

    #[test]
    fn preserves_text_content() {
        let svg = "<svg xmlns=\"http://www.w3.org/2000/svg\"><text x=\"0\" y=\"0\">Hello  World</text></svg>";
        let out = optimize(svg, &Options::default()).unwrap();
        assert!(out.contains("Hello  World"), "text spacing preserved: {out}");
    }
}
