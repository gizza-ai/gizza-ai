//! gizza-ai/svg-recolor core — recolor an SVG by remapping its colors,
//! dependency-free. Walks the SVG markup and rewrites the colors that appear in
//! the color-bearing presentation attributes (`fill`, `stroke`, `stop-color`,
//! `color`, `flood-color`, `lighting-color`, `solid-color`, `stop-color`),
//! their `style="…"` inline equivalents, and the values inside `<style>` blocks.
//!
//! Two modes:
//!   * **map** — replace each *source* color with its mapped *target* color.
//!     Matching is format-insensitive: `#fff`, `#FFFFFF`, `#ffffffff` (with full
//!     alpha) and `rgb(255,255,255)` all match the source `#ffffff`. The output
//!     keeps each occurrence's original syntax otherwise untouched, only the
//!     matched colors are swapped.
//!   * **monochrome** — recolor *every* color to a single target color (handy for
//!     turning a multi-color icon into a single-tint glyph). `none`, `transparent`
//!     and `currentColor` keywords are preserved (they aren't real colors).
//!
//! Colors are compared by their canonical RGB(A) value, so any input syntax maps
//! correctly. Path data, geometry and structure are never changed.

use std::collections::HashMap;

/// A parsed RGBA color in 0–255 channels (alpha 255 = opaque).
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

/// The 16 SVG/HTML basic + a few very common extended named colors. Kept small
/// and dependency-free; unknown names are left untouched (treated as non-colors).
fn named_color(name: &str) -> Option<Rgba> {
    let n = name.trim().to_ascii_lowercase();
    let (r, g, b) = match n.as_str() {
        "black" => (0, 0, 0),
        "silver" => (192, 192, 192),
        "gray" | "grey" => (128, 128, 128),
        "white" => (255, 255, 255),
        "maroon" => (128, 0, 0),
        "red" => (255, 0, 0),
        "purple" => (128, 0, 128),
        "fuchsia" | "magenta" => (255, 0, 255),
        "green" => (0, 128, 0),
        "lime" => (0, 255, 0),
        "olive" => (128, 128, 0),
        "yellow" => (255, 255, 0),
        "navy" => (0, 0, 128),
        "blue" => (0, 0, 255),
        "teal" => (0, 128, 128),
        "aqua" | "cyan" => (0, 255, 255),
        "orange" => (255, 165, 0),
        "pink" => (255, 192, 203),
        "brown" => (165, 42, 42),
        "gold" => (255, 215, 0),
        "indigo" => (75, 0, 130),
        "violet" => (238, 130, 238),
        "darkgray" | "darkgrey" => (169, 169, 169),
        "lightgray" | "lightgrey" => (211, 211, 211),
        "dimgray" | "dimgrey" => (105, 105, 105),
        _ => return None,
    };
    Some(Rgba { r, g, b, a: 255 })
}

/// Parse a CSS color string (hex, rgb()/rgba(), or a known named color) into RGBA.
/// Returns None for keywords like `none`/`transparent`/`currentColor`/`inherit`,
/// `url(#…)` references, and anything unrecognized — those are left untouched.
pub fn parse_color(s: &str) -> Option<Rgba> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    let lower = t.to_ascii_lowercase();
    // Non-color keywords / references — never remapped.
    match lower.as_str() {
        "none" | "transparent" | "currentcolor" | "inherit" | "initial" | "unset"
        | "context-fill" | "context-stroke" => return None,
        _ => {}
    }
    if lower.starts_with("url(") {
        return None;
    }
    if let Some(hex) = t.strip_prefix('#') {
        return parse_hex(hex);
    }
    if lower.starts_with("rgb") {
        return parse_rgb_fn(&lower);
    }
    named_color(&lower)
}

fn parse_hex(hex: &str) -> Option<Rgba> {
    let hex = hex.trim();
    if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let h = |s: &str| u8::from_str_radix(s, 16).ok();
    match hex.len() {
        3 => {
            let r = h(&hex[0..1])?;
            let g = h(&hex[1..2])?;
            let b = h(&hex[2..3])?;
            Some(Rgba {
                r: r * 17,
                g: g * 17,
                b: b * 17,
                a: 255,
            })
        }
        4 => {
            let r = h(&hex[0..1])?;
            let g = h(&hex[1..2])?;
            let b = h(&hex[2..3])?;
            let a = h(&hex[3..4])?;
            Some(Rgba {
                r: r * 17,
                g: g * 17,
                b: b * 17,
                a: a * 17,
            })
        }
        6 => Some(Rgba {
            r: h(&hex[0..2])?,
            g: h(&hex[2..4])?,
            b: h(&hex[4..6])?,
            a: 255,
        }),
        8 => Some(Rgba {
            r: h(&hex[0..2])?,
            g: h(&hex[2..4])?,
            b: h(&hex[4..6])?,
            a: h(&hex[6..8])?,
        }),
        _ => None,
    }
}

fn parse_rgb_fn(lower: &str) -> Option<Rgba> {
    let open = lower.find('(')?;
    let close = lower.rfind(')')?;
    if close <= open {
        return None;
    }
    let inner = &lower[open + 1..close];
    // Support both comma- and space-separated, with an optional `/ alpha`.
    let inner = inner.replace('/', " ").replace(',', " ");
    let parts: Vec<&str> = inner.split_whitespace().collect();
    if parts.len() < 3 {
        return None;
    }
    let chan = |p: &str| -> Option<u8> {
        if let Some(pct) = p.strip_suffix('%') {
            let v: f64 = pct.parse().ok()?;
            Some((v / 100.0 * 255.0).round().clamp(0.0, 255.0) as u8)
        } else {
            let v: f64 = p.parse().ok()?;
            Some(v.round().clamp(0.0, 255.0) as u8)
        }
    };
    let r = chan(parts[0])?;
    let g = chan(parts[1])?;
    let b = chan(parts[2])?;
    let a = if parts.len() >= 4 {
        if let Some(pct) = parts[3].strip_suffix('%') {
            let v: f64 = pct.parse().ok()?;
            (v / 100.0 * 255.0).round().clamp(0.0, 255.0) as u8
        } else {
            let v: f64 = parts[3].parse().ok()?;
            (v * 255.0).round().clamp(0.0, 255.0) as u8
        }
    } else {
        255
    };
    Some(Rgba { r, g, b, a })
}

/// Canonical lowercase hex for a color: `#rrggbb`, or `#rrggbbaa` when not opaque.
pub fn to_hex(c: Rgba) -> String {
    if c.a == 255 {
        format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)
    } else {
        format!("#{:02x}{:02x}{:02x}{:02x}", c.r, c.g, c.b, c.a)
    }
}

/// What to do with colors found in the SVG.
pub enum Mode {
    /// Replace each source color with its mapped target (canonical-RGBA keyed).
    Map(HashMap<Rgba, Rgba>),
    /// Recolor every real color to a single target.
    Monochrome(Rgba),
}

/// The presentation properties whose values are colors we remap.
const COLOR_PROPS: &[&str] = &[
    "fill",
    "stroke",
    "stop-color",
    "color",
    "flood-color",
    "lighting-color",
    "solid-color",
];

/// Decide the replacement for a single color token (already known to parse).
fn remap(c: Rgba, mode: &Mode) -> Option<Rgba> {
    match mode {
        Mode::Map(m) => m.get(&c).copied(),
        Mode::Monochrome(t) => Some(*t),
    }
}

/// Rewrite a single color value token, preserving its surrounding whitespace.
/// Returns the replacement string (which may equal the original).
fn rewrite_color_value(value: &str, mode: &Mode) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return value.to_string();
    }
    let lead = &value[..value.len() - value.trim_start().len()];
    let trail = &value[value.trim_end().len()..];
    match parse_color(trimmed) {
        Some(c) => match remap(c, mode) {
            Some(nc) => format!("{lead}{}{trail}", to_hex(nc)),
            None => value.to_string(),
        },
        None => value.to_string(),
    }
}

/// Rewrite a `prop:value;prop:value` style/CSS declaration string, only touching
/// declarations whose property is a color property.
fn rewrite_style_decls(style: &str, mode: &Mode) -> String {
    let mut out = String::with_capacity(style.len());
    for (i, decl) in style.split(';').enumerate() {
        if i > 0 {
            out.push(';');
        }
        if let Some(colon) = decl.find(':') {
            let prop = &decl[..colon];
            let val = &decl[colon + 1..];
            let prop_key = prop.trim().to_ascii_lowercase();
            if COLOR_PROPS.contains(&prop_key.as_str()) {
                out.push_str(prop);
                out.push(':');
                out.push_str(&rewrite_color_value(val, mode));
                continue;
            }
        }
        out.push_str(decl);
    }
    out
}

/// Walk an opening/standalone tag's attributes and rewrite color attributes and
/// inline `style`. `tag` is the full `<…>` token including the angle brackets.
fn rewrite_tag(tag: &str, mode: &Mode) -> String {
    // Comments / declarations / closing tags: leave untouched.
    if tag.starts_with("<!") || tag.starts_with("<?") || tag.starts_with("</") {
        return tag.to_string();
    }
    let mut out = String::with_capacity(tag.len());
    let bytes = tag.as_bytes();
    let mut i = 0;
    // Copy the leading `<name` chunk up to the first whitespace.
    while i < bytes.len() && !bytes[i].is_ascii_whitespace() && bytes[i] != b'>' {
        out.push(bytes[i] as char);
        i += 1;
    }
    while i < bytes.len() {
        let ch = bytes[i] as char;
        if ch == '>' || ch == '/' {
            out.push(ch);
            i += 1;
            continue;
        }
        if ch.is_ascii_whitespace() {
            out.push(ch);
            i += 1;
            continue;
        }
        // Read an attribute name.
        let attr_start = i;
        while i < bytes.len() {
            let c = bytes[i] as char;
            if c == '=' || c.is_ascii_whitespace() || c == '>' || c == '/' {
                break;
            }
            i += 1;
        }
        let attr_name = &tag[attr_start..i];
        // Optional whitespace then '='.
        let mut j = i;
        while j < bytes.len() && (bytes[j] as char).is_ascii_whitespace() {
            j += 1;
        }
        if j >= bytes.len() || bytes[j] != b'=' {
            // Valueless attribute — emit the name as-is.
            out.push_str(attr_name);
            continue;
        }
        // Emit name + the (possibly spaced) '='.
        out.push_str(attr_name);
        out.push_str(&tag[i..j]); // whitespace before '='
        out.push('=');
        j += 1;
        // Whitespace after '='.
        let ws_start = j;
        while j < bytes.len() && (bytes[j] as char).is_ascii_whitespace() {
            j += 1;
        }
        out.push_str(&tag[ws_start..j]);
        // Read the value, quoted or bare.
        if j < bytes.len() && (bytes[j] == b'"' || bytes[j] == b'\'') {
            let quote = bytes[j];
            out.push(quote as char);
            j += 1;
            let val_start = j;
            while j < bytes.len() && bytes[j] != quote {
                j += 1;
            }
            let value = &tag[val_start..j];
            let key = attr_name.trim().to_ascii_lowercase();
            let new_val = if key == "style" {
                rewrite_style_decls(value, mode)
            } else if COLOR_PROPS.contains(&key.as_str()) {
                rewrite_color_value(value, mode)
            } else {
                value.to_string()
            };
            out.push_str(&new_val);
            if j < bytes.len() {
                out.push(quote as char);
                j += 1;
            }
        } else {
            // Bare value up to whitespace or '>'.
            let val_start = j;
            while j < bytes.len() {
                let c = bytes[j] as char;
                if c.is_ascii_whitespace() || c == '>' || c == '/' {
                    break;
                }
                j += 1;
            }
            let value = &tag[val_start..j];
            let key = attr_name.trim().to_ascii_lowercase();
            let new_val = if key == "style" {
                rewrite_style_decls(value, mode)
            } else if COLOR_PROPS.contains(&key.as_str()) {
                rewrite_color_value(value, mode)
            } else {
                value.to_string()
            };
            out.push_str(&new_val);
        }
        i = j;
    }
    out
}

/// Rewrite the CSS inside a `<style>` block: only the values of color properties
/// are remapped. A forgiving scan over `prop:value;` declarations (selectors and
/// braces are passed through unchanged).
fn rewrite_css_block(css: &str, mode: &Mode) -> String {
    let mut out = String::with_capacity(css.len());
    let bytes = css.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        // Copy up to a '{', '}' or ';' boundary, then handle declarations inside braces.
        let start = i;
        while i < bytes.len() && bytes[i] != b'{' {
            i += 1;
        }
        out.push_str(&css[start..i]); // selector(s)
        if i >= bytes.len() {
            break;
        }
        out.push('{');
        i += 1;
        let block_start = i;
        let mut depth = 1;
        while i < bytes.len() && depth > 0 {
            match bytes[i] {
                b'{' => depth += 1,
                b'}' => depth -= 1,
                _ => {}
            }
            if depth == 0 {
                break;
            }
            i += 1;
        }
        let block = &css[block_start..i];
        out.push_str(&rewrite_style_decls(block, mode));
        if i < bytes.len() {
            out.push('}');
            i += 1;
        }
    }
    out
}

/// Recolor an SVG. `mode` selects map vs monochrome. Returns the rewritten SVG.
pub fn recolor(svg: &str, mode: &Mode) -> Result<String, String> {
    if svg.trim().is_empty() {
        return Err("Empty SVG input.".into());
    }
    if !svg.contains('<') {
        return Err("Input does not look like SVG markup (no tags found).".into());
    }
    let mut out = String::with_capacity(svg.len());
    let bytes = svg.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'<' {
            // Determine the kind of node to find its end.
            let rest = &svg[i..];
            if rest.starts_with("<!--") {
                if let Some(end) = rest.find("-->") {
                    out.push_str(&rest[..end + 3]);
                    i += end + 3;
                    continue;
                }
            }
            // Find the matching '>' (quote-aware).
            let mut j = i + 1;
            let mut quote: Option<u8> = None;
            while j < bytes.len() {
                let c = bytes[j];
                match quote {
                    Some(q) => {
                        if c == q {
                            quote = None;
                        }
                    }
                    None => {
                        if c == b'"' || c == b'\'' {
                            quote = Some(c);
                        } else if c == b'>' {
                            break;
                        }
                    }
                }
                j += 1;
            }
            if j >= bytes.len() {
                out.push_str(&svg[i..]);
                break;
            }
            let tag = &svg[i..=j];
            let tag_lower = tag.to_ascii_lowercase();
            out.push_str(&rewrite_tag(tag, mode));
            i = j + 1;
            // If this is a <style> open tag, capture its raw contents and rewrite CSS.
            if (tag_lower.starts_with("<style") )
                && !tag_lower.ends_with("/>")
                && !tag.starts_with("</")
            {
                let after = &svg[i..];
                let lower_after = after.to_ascii_lowercase();
                if let Some(close) = lower_after.find("</style") {
                    let css = &after[..close];
                    out.push_str(&rewrite_css_block(css, mode));
                    i += close;
                }
            }
            continue;
        }
        // Text content — emit verbatim.
        let start = i;
        while i < bytes.len() && bytes[i] != b'<' {
            i += 1;
        }
        out.push_str(&svg[start..i]);
    }
    Ok(out)
}

/// Build a `Mode::Map` from a list of `from=>to` pairs (string color syntaxes).
/// Returns an error describing any unparseable color.
pub fn build_map(pairs: &[(String, String)]) -> Result<HashMap<Rgba, Rgba>, String> {
    if pairs.is_empty() {
        return Err("No color mappings provided.".into());
    }
    let mut map = HashMap::new();
    for (from, to) in pairs {
        let fc = parse_color(from)
            .ok_or_else(|| format!("Could not parse source color '{from}'."))?;
        let tc = parse_color(to)
            .ok_or_else(|| format!("Could not parse target color '{to}'."))?;
        map.insert(fc, tc);
    }
    Ok(map)
}

/// Parse a free-form mapping string like "#000=>#fff, red => #00ff00" or
/// newline-separated pairs into `(from, to)` tuples. Accepts `=>`, `->`, `:`,
/// or `=` as the separator and `,` or newline as the pair separator.
pub fn parse_mapping(s: &str) -> Result<Vec<(String, String)>, String> {
    let mut pairs = Vec::new();
    for raw in s.split(['\n', ',', ';']) {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        let sep = ["=>", "->", ":", "="]
            .iter()
            .filter_map(|sep| line.find(sep).map(|idx| (idx, sep.len())))
            .min_by_key(|(idx, _)| *idx);
        let (idx, seplen) = sep.ok_or_else(|| {
            format!("Mapping entry '{line}' is missing a separator (use 'from=>to').")
        })?;
        let from = line[..idx].trim().to_string();
        let to = line[idx + seplen..].trim().to_string();
        if from.is_empty() || to.is_empty() {
            return Err(format!("Mapping entry '{line}' has an empty side."));
        }
        pairs.push((from, to));
    }
    if pairs.is_empty() {
        return Err("No color mappings found.".into());
    }
    Ok(pairs)
}

/// High-level entry: recolor `svg`. If `to` is non-empty, every color is set to
/// `to` (monochrome). Otherwise `mapping` is parsed as `from=>to` pairs.
pub fn run(svg: &str, mapping: &str, to: &str) -> Result<String, String> {
    let to_t = to.trim();
    if !to_t.is_empty() {
        let target = parse_color(to_t)
            .ok_or_else(|| format!("Could not parse target color '{to_t}'."))?;
        return recolor(svg, &Mode::Monochrome(target));
    }
    let pairs = parse_mapping(mapping)?;
    let map = build_map(&pairs)?;
    recolor(svg, &Mode::Map(map))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hex_forms() {
        assert_eq!(parse_color("#fff"), Some(Rgba { r: 255, g: 255, b: 255, a: 255 }));
        assert_eq!(parse_color("#ffffff"), Some(Rgba { r: 255, g: 255, b: 255, a: 255 }));
        assert_eq!(parse_color("#000"), Some(Rgba { r: 0, g: 0, b: 0, a: 255 }));
        assert_eq!(parse_color("#ff000080").unwrap().a, 0x80);
    }

    #[test]
    fn parse_rgb_and_named() {
        assert_eq!(parse_color("rgb(255, 0, 0)"), Some(Rgba { r: 255, g: 0, b: 0, a: 255 }));
        assert_eq!(parse_color("rgba(0,0,0,0.5)").unwrap().a, 128);
        assert_eq!(parse_color("red"), Some(Rgba { r: 255, g: 0, b: 0, a: 255 }));
        assert_eq!(parse_color("WHITE"), Some(Rgba { r: 255, g: 255, b: 255, a: 255 }));
    }

    #[test]
    fn keywords_not_colors() {
        assert!(parse_color("none").is_none());
        assert!(parse_color("currentColor").is_none());
        assert!(parse_color("url(#grad)").is_none());
        assert!(parse_color("transparent").is_none());
    }

    #[test]
    fn map_remaps_attributes() {
        let svg = r##"<svg><path fill="#000" stroke="red" d="M0 0"/></svg>"##;
        let out = run(svg, "#000 => #ffffff, red=>#00ff00", "").unwrap();
        assert!(out.contains(r##"fill="#ffffff""##), "{out}");
        assert!(out.contains(r##"stroke="#00ff00""##), "{out}");
        // d untouched
        assert!(out.contains(r#"d="M0 0""#));
    }

    #[test]
    fn map_is_format_insensitive() {
        // source written as #ffffff, document uses #FFF and rgb form.
        let svg = r##"<rect fill="#FFF"/><rect fill="rgb(255,255,255)"/>"##;
        let out = run(svg, "#ffffff => #123456", "").unwrap();
        assert_eq!(out.matches("#123456").count(), 2, "{out}");
    }

    #[test]
    fn monochrome_recolors_all() {
        let svg = r##"<path fill="#000"/><path stroke="blue"/><path fill="none"/>"##;
        let out = run(svg, "", "#ff0000").unwrap();
        assert!(out.contains(r##"fill="#ff0000""##));
        assert!(out.contains(r##"stroke="#ff0000""##));
        // none preserved
        assert!(out.contains(r#"fill="none""#));
    }

    #[test]
    fn inline_style_and_css_block() {
        let svg = r##"<svg><style>.a{fill:#000;stroke:#111}</style><rect style="fill:#000;opacity:0.5"/></svg>"##;
        let out = run(svg, "#000=>#fff, #111=>#222", "").unwrap();
        assert!(out.contains("fill:#fff"), "{out}");
        assert!(out.contains("stroke:#222"), "{out}");
        // opacity (non-color) untouched
        assert!(out.contains("opacity:0.5"), "{out}");
    }

    #[test]
    fn errors() {
        assert!(run("", "#000=>#fff", "").is_err());
        assert!(run("<svg/>", "notacolor=>#fff", "").is_err());
        assert!(run("<svg/>", "", "notacolor").is_err());
        assert!(run("<svg/>", "", "").is_err());
        assert!(run("<svg/>", "#000 #fff", "").is_err()); // no separator
    }

    #[test]
    fn unmapped_colors_unchanged() {
        let svg = r##"<path fill="#abcdef"/>"##;
        let out = run(svg, "#000=>#fff", "").unwrap();
        assert!(out.contains("#abcdef"), "{out}");
    }
}
