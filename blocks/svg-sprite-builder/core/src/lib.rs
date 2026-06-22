//! svg-sprite-builder core — combine several standalone `<svg>` documents into
//! one SVG **symbol sprite sheet**: each input SVG becomes a `<symbol id="…"
//! viewBox="…">…</symbol>`, wrapped in a single hidden `<svg>` you drop once at
//! the top of a page and reference everywhere with `<use href="#id">`.
//!
//! Pure-Rust, no I/O, no XML dependency — a small forgiving scanner that handles
//! the SVG subset that matters (the outer `<svg …>` open tag, its `viewBox` /
//! `width` / `height` / `id` / `<title>`, and the inner markup). Shared by the
//! chat skill block and the web page.

/// How to derive each symbol's `id`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdSource {
    /// `<prefix><n>` (1-based), e.g. `icon-1`, `icon-2`.
    Auto,
    /// The source `<svg>`'s own `id` attribute; falls back to Auto when absent.
    SvgId,
    /// The source `<svg>`'s first `<title>` text; falls back to Auto when absent.
    Title,
}

impl IdSource {
    pub fn parse(s: &str) -> Result<IdSource, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "auto" | "" => Ok(IdSource::Auto),
            "id" | "svg-id" | "svgid" => Ok(IdSource::SvgId),
            "title" => Ok(IdSource::Title),
            other => Err(format!(
                "invalid id_source {other:?}: expected \"auto\", \"id\", or \"title\""
            )),
        }
    }
}

/// One parsed source SVG.
struct Svg {
    /// `viewBox` to put on the `<symbol>` (derived from width/height if absent).
    view_box: Option<String>,
    /// The source `id` attribute, if any.
    id_attr: Option<String>,
    /// First `<title>` text content, if any.
    title: Option<String>,
    /// Everything between `<svg …>` and `</svg>` (verbatim).
    inner: String,
}

/// Build a `<symbol>` sprite sheet from one or more concatenated `<svg>`
/// documents in `svgs`.
///
/// - `prefix` seeds auto ids (`<prefix>-1`, `<prefix>-2`, …) and disambiguates
///   duplicate derived ids.
/// - `id_source` selects how each symbol's id is chosen (see [`IdSource`]).
/// - When `hidden` is true the wrapper `<svg>` carries `aria-hidden="true"` and
///   a zero-size hidden style so dropping it in the DOM renders nothing;
///   otherwise the bare wrapper is emitted.
///
/// Returns the sprite SVG followed by an XML comment listing the `<use>`
/// snippet for every symbol. Errors when `svgs` contains no `<svg>` element.
pub fn build(svgs: &str, prefix: &str, id_source: IdSource, hidden: bool) -> Result<String, String> {
    let parsed = parse_svgs(svgs)?;
    if parsed.is_empty() {
        return Err("no <svg> element found in the input".into());
    }

    let prefix = {
        let p = sanitize_id(prefix);
        if p.is_empty() {
            "icon".to_string()
        } else {
            p
        }
    };

    let mut used_ids: Vec<String> = Vec::new();
    let mut symbols = String::new();
    let mut uses: Vec<String> = Vec::new();

    for (i, svg) in parsed.iter().enumerate() {
        let base = match id_source {
            IdSource::Auto => format!("{prefix}-{}", i + 1),
            IdSource::SvgId => svg
                .id_attr
                .as_deref()
                .map(sanitize_id)
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| format!("{prefix}-{}", i + 1)),
            IdSource::Title => svg
                .title
                .as_deref()
                .map(sanitize_id)
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| format!("{prefix}-{}", i + 1)),
        };
        let id = unique_id(base, &mut used_ids);

        let vb = svg
            .view_box
            .clone()
            .map(|v| format!(" viewBox=\"{}\"", escape_attr(&v)))
            .unwrap_or_default();

        symbols.push_str(&format!(
            "  <symbol id=\"{}\"{}>{}</symbol>\n",
            escape_attr(&id),
            vb,
            svg.inner.trim()
        ));
        uses.push(format!("<use href=\"#{id}\" />"));
    }

    let open = if hidden {
        "<svg xmlns=\"http://www.w3.org/2000/svg\" aria-hidden=\"true\" style=\"position:absolute;width:0;height:0;overflow:hidden\">"
    } else {
        "<svg xmlns=\"http://www.w3.org/2000/svg\">"
    };

    let mut out = String::new();
    out.push_str(open);
    out.push('\n');
    out.push_str(&symbols);
    out.push_str("</svg>\n");
    out.push_str("<!-- Reference a symbol with:\n");
    for u in &uses {
        out.push_str("  <svg>");
        out.push_str(u);
        out.push_str("</svg>\n");
    }
    out.push_str("-->");
    Ok(out)
}

/// Convenience used by the CLI/chat wrappers: parse string args then [`build`].
pub fn run(svgs: &str, prefix: &str, id_source: &str, hidden: bool) -> Result<String, String> {
    let src = IdSource::parse(id_source)?;
    build(svgs, prefix, src, hidden)
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

/// Find every top-level `<svg …> … </svg>` in `text` and parse it.
fn parse_svgs(text: &str) -> Result<Vec<Svg>, String> {
    let mut out = Vec::new();
    let mut i = 0usize;
    while let Some(start) = find_svg_open(text, i) {
        // Locate the end of the opening tag.
        let tag_end = match find_unquoted(text, start, '>') {
            Some(e) => e,
            None => return Err("malformed <svg>: opening tag is never closed".into()),
        };
        let open_tag = &text[start..=tag_end];
        let self_closing = open_tag.trim_end().ends_with("/>");

        let (inner, after) = if self_closing {
            (String::new(), tag_end + 1)
        } else {
            // Find the matching </svg>, honoring nested <svg> elements.
            let close = match find_matching_close(text, tag_end + 1) {
                Some(c) => c,
                None => return Err("malformed <svg>: missing closing </svg>".into()),
            };
            (text[tag_end + 1..close.0].to_string(), close.1)
        };

        out.push(Svg {
            view_box: derive_view_box(open_tag),
            id_attr: get_attr(open_tag, "id"),
            title: first_title(&inner),
            inner,
        });

        i = after;
    }
    Ok(out)
}

/// Index of the next `<svg` whose next char is whitespace, `>`, or `/`.
fn find_svg_open(text: &str, from: usize) -> Option<usize> {
    let lower = text.to_ascii_lowercase();
    let mut search = from;
    while let Some(rel) = lower[search..].find("<svg") {
        let pos = search + rel;
        let after = pos + 4;
        match lower[after..].chars().next() {
            Some(c) if c.is_whitespace() || c == '>' || c == '/' => return Some(pos),
            None => return Some(pos),
            _ => search = after, // <svgfoo> — keep looking
        }
    }
    None
}

/// Position of the matching `</svg>` for an `<svg>` opened before `from`,
/// accounting for nested `<svg>` opens. Returns (close_tag_start, after_close).
fn find_matching_close(text: &str, from: usize) -> Option<(usize, usize)> {
    let lower = text.to_ascii_lowercase();
    let mut depth = 0usize;
    let mut i = from;
    loop {
        let next_open = find_svg_open(text, i);
        let next_close = lower[i..].find("</svg").map(|r| i + r);
        match (next_open, next_close) {
            (Some(o), Some(c)) if o < c => {
                // A nested <svg>: skip past its open tag.
                i = find_unquoted(text, o, '>').map(|e| e + 1)?;
                depth += 1;
            }
            (_, Some(c)) => {
                let end = find_unquoted(text, c, '>')?;
                if depth == 0 {
                    return Some((c, end + 1));
                }
                depth -= 1;
                i = end + 1;
            }
            (_, None) => return None,
        }
    }
}

/// Find `ch` starting at `from`, skipping single/double-quoted spans so an
/// attribute value containing `>` doesn't end the tag early.
fn find_unquoted(text: &str, from: usize, ch: char) -> Option<usize> {
    let mut quote: Option<char> = None;
    for (idx, c) in text[from..].char_indices() {
        match quote {
            Some(q) => {
                if c == q {
                    quote = None;
                }
            }
            None => {
                if c == '"' || c == '\'' {
                    quote = Some(c);
                } else if c == ch {
                    return Some(from + idx);
                }
            }
        }
    }
    None
}

/// Read attribute `name`'s value from an opening tag string (`<svg … >`).
fn get_attr(tag: &str, name: &str) -> Option<String> {
    let lower = tag.to_ascii_lowercase();
    let needle = name.to_ascii_lowercase();
    let mut search = 0;
    while let Some(rel) = lower[search..].find(&needle) {
        let pos = search + rel;
        // Must be preceded by whitespace so `viewBox` doesn't match `box` and
        // `id` doesn't match inside another attr name (e.g. `width`).
        let prev_ok = pos == 0 || tag[..pos].chars().last().map_or(false, |c| c.is_whitespace());
        let after = &tag[pos + name.len()..];
        let rest = after.trim_start();
        if prev_ok && rest.starts_with('=') {
            let rest = rest[1..].trim_start();
            let mut chars = rest.chars();
            return match chars.next() {
                Some('"') => rest[1..].split('"').next().map(|s| s.to_string()),
                Some('\'') => rest[1..].split('\'').next().map(|s| s.to_string()),
                _ => None,
            };
        }
        search = pos + name.len();
    }
    None
}

/// `viewBox`, or one synthesized from `width`/`height` (`0 0 W H`).
fn derive_view_box(tag: &str) -> Option<String> {
    if let Some(vb) = get_attr(tag, "viewBox") {
        let t = vb.trim();
        if !t.is_empty() {
            return Some(t.to_string());
        }
    }
    let w = get_attr(tag, "width").and_then(|s| parse_len(&s));
    let h = get_attr(tag, "height").and_then(|s| parse_len(&s));
    match (w, h) {
        (Some(w), Some(h)) => Some(format!("0 0 {} {}", trim_num(w), trim_num(h))),
        _ => None,
    }
}

/// Parse a length attribute, dropping a trailing `px` unit; rejects `%`/em/etc.
fn parse_len(s: &str) -> Option<f64> {
    let t = s.trim();
    let t = t.strip_suffix("px").unwrap_or(t);
    let v: f64 = t.trim().parse().ok()?;
    if v.is_finite() && v > 0.0 {
        Some(v)
    } else {
        None
    }
}

fn trim_num(v: f64) -> String {
    if v.fract() == 0.0 {
        format!("{}", v as i64)
    } else {
        format!("{v}")
    }
}

/// First `<title>…</title>` text content inside `inner`, if any.
fn first_title(inner: &str) -> Option<String> {
    let lower = inner.to_ascii_lowercase();
    let open = lower.find("<title")?;
    let gt = find_unquoted(inner, open, '>')?;
    let close = lower[gt..].find("</title").map(|r| gt + r)?;
    let raw = inner[gt + 1..close].trim();
    if raw.is_empty() {
        None
    } else {
        Some(raw.to_string())
    }
}

// ---------------------------------------------------------------------------
// Id helpers
// ---------------------------------------------------------------------------

/// Lower-case, replace runs of non-`[a-z0-9-]` with a single `-`, trim `-`.
fn sanitize_id(s: &str) -> String {
    let mut out = String::new();
    let mut prev_dash = true; // suppress leading dashes
    for c in s.trim().chars() {
        let c = c.to_ascii_lowercase();
        if c.is_ascii_alphanumeric() {
            out.push(c);
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    out
}

/// Ensure `base` is unique within `used`, appending `-2`, `-3`, … on collision.
fn unique_id(base: String, used: &mut Vec<String>) -> String {
    if !used.iter().any(|u| u == &base) {
        used.push(base.clone());
        return base;
    }
    let mut n = 2;
    loop {
        let cand = format!("{base}-{n}");
        if !used.iter().any(|u| u == &cand) {
            used.push(cand.clone());
            return cand;
        }
        n += 1;
    }
}

fn escape_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('"', "&quot;")
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combines_two_svgs_with_viewbox_and_auto_ids() {
        let input = r#"
            <svg viewBox="0 0 24 24"><path d="M1 1"/></svg>
            <svg viewBox="0 0 16 16"><circle cx="8" cy="8" r="4"/></svg>
        "#;
        let out = build(input, "icon", IdSource::Auto, true).unwrap();
        assert!(out.contains(r#"<symbol id="icon-1" viewBox="0 0 24 24"><path d="M1 1"/></symbol>"#), "{out}");
        assert!(out.contains(r#"<symbol id="icon-2" viewBox="0 0 16 16"><circle cx="8" cy="8" r="4"/></symbol>"#), "{out}");
        assert!(out.contains("aria-hidden=\"true\""));
        assert!(out.contains("<use href=\"#icon-1\" />"));
    }

    #[test]
    fn id_from_svg_id_attr() {
        let input = r#"<svg id="Home Icon" viewBox="0 0 10 10"><rect/></svg>"#;
        let out = build(input, "icon", IdSource::SvgId, true).unwrap();
        assert!(out.contains(r#"<symbol id="home-icon" viewBox="0 0 10 10">"#), "{out}");
    }

    #[test]
    fn id_from_title_falls_back_to_auto() {
        let input = r#"<svg viewBox="0 0 1 1"><title>Arrow Up</title><path/></svg>
                       <svg viewBox="0 0 2 2"><path/></svg>"#;
        let out = build(input, "ic", IdSource::Title, true).unwrap();
        assert!(out.contains(r#"id="arrow-up""#), "{out}");
        assert!(out.contains(r#"id="ic-2""#), "{out}");
    }

    #[test]
    fn derives_viewbox_from_width_height() {
        let input = r#"<svg width="32" height="48px"><g/></svg>"#;
        let out = build(input, "icon", IdSource::Auto, true).unwrap();
        assert!(out.contains(r#"viewBox="0 0 32 48""#), "{out}");
    }

    #[test]
    fn deduplicates_colliding_ids() {
        let input = r#"<svg id="a"><path/></svg><svg id="a"><path/></svg>"#;
        let out = build(input, "icon", IdSource::SvgId, true).unwrap();
        assert!(out.contains(r#"id="a">"#));
        assert!(out.contains(r#"id="a-2">"#), "{out}");
    }

    #[test]
    fn not_hidden_emits_bare_wrapper() {
        let input = r#"<svg viewBox="0 0 1 1"><path/></svg>"#;
        let out = build(input, "icon", IdSource::Auto, false).unwrap();
        assert!(out.contains(r#"<svg xmlns="http://www.w3.org/2000/svg">"#));
        assert!(!out.contains("aria-hidden"));
    }

    #[test]
    fn handles_self_closing_svg() {
        let input = r#"<svg viewBox="0 0 5 5" />"#;
        let out = build(input, "icon", IdSource::Auto, true).unwrap();
        assert!(out.contains(r#"<symbol id="icon-1" viewBox="0 0 5 5"></symbol>"#), "{out}");
    }

    #[test]
    fn attr_with_gt_in_quotes_does_not_break_parsing() {
        let input = r#"<svg viewBox="0 0 9 9" data-x="a > b"><path d="M0 0 L1"/></svg>"#;
        let out = build(input, "icon", IdSource::Auto, true).unwrap();
        assert!(out.contains(r#"viewBox="0 0 9 9""#), "{out}");
    }

    #[test]
    fn nested_svg_is_kept_in_inner() {
        let input = r#"<svg viewBox="0 0 4 4"><svg viewBox="0 0 2 2"><path/></svg></svg>"#;
        let out = build(input, "icon", IdSource::Auto, true).unwrap();
        // Exactly one symbol (the inner <svg> is part of the outer's content).
        assert_eq!(out.matches("<symbol").count(), 1, "{out}");
        assert!(out.contains(r#"<svg viewBox="0 0 2 2"><path/></svg>"#), "{out}");
    }

    #[test]
    fn empty_or_no_svg_is_error() {
        assert!(build("", "icon", IdSource::Auto, true).is_err());
        assert!(build("just some text", "icon", IdSource::Auto, true).is_err());
    }

    #[test]
    fn run_parses_id_source_string() {
        assert!(run("<svg><path/></svg>", "icon", "title", true).is_ok());
        assert!(run("<svg><path/></svg>", "icon", "bogus", true).is_err());
    }

    #[test]
    fn empty_prefix_defaults_to_icon() {
        let out = build("<svg><path/></svg>", "  ", IdSource::Auto, true).unwrap();
        assert!(out.contains(r#"id="icon-1""#), "{out}");
    }
}
