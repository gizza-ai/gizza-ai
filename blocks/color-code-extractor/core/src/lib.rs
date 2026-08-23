//! color-code-extractor core — pure compute, shared by the chat skill block and the web page.
//!
//! Scans arbitrary text (CSS, SCSS/LESS, HTML, JS, JSON, config files, prose) for
//! colour literals — `#rgb`/`#rgba`/`#rrggbb`/`#rrggbbaa`, `rgb()`/`rgba()`,
//! `hsl()`/`hsla()`, `hwb()` and the 148 CSS colour keywords plus `transparent` —
//! normalises every hit to RGBA, deduplicates them into one palette, and renders
//! that palette as a plain list, CSV, JSON, CSS custom properties, SCSS/LESS
//! variables, a Tailwind colour map or an SVG swatch sheet.
//!
//! Deduplication is by COLOUR, not by spelling: `#f00`, `#FF0000`, `red` and
//! `rgb(255, 0, 0)` are one palette entry with a usage count of four. Alpha is part
//! of the identity, so `#ff0000` and `rgba(255,0,0,.5)` stay separate entries.
//!
//! No dependencies, no I/O — the whole scan runs in the sandbox.

/// Largest accepted input, in bytes.
pub const MAX_INPUT_BYTES: usize = 5_000_000;
/// Largest accepted `limit` value.
pub const MAX_LIMIT: i64 = 1000;

/// A colour normalised to 8-bit RGB plus a 0..=1 alpha.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: f64,
}

impl Rgba {
    fn opaque(r: u8, g: u8, b: u8) -> Self {
        Rgba { r, g, b, a: 1.0 }
    }
    /// Identity for deduplication: exact channels plus alpha quantised to 1/1000
    /// so `0.5` and `50%` collapse to one entry.
    fn key(&self) -> (u8, u8, u8, i64) {
        (self.r, self.g, self.b, (self.a * 1000.0).round() as i64)
    }
    fn is_grey(&self) -> bool {
        self.r == self.g && self.g == self.b
    }
    fn is_black_or_white(&self) -> bool {
        (self.r == 0 && self.g == 0 && self.b == 0) || (self.r == 255 && self.g == 255 && self.b == 255)
    }
}

/// One deduplicated palette entry.
#[derive(Clone, Debug)]
pub struct PaletteEntry {
    pub color: Rgba,
    /// The literal exactly as first written in the input.
    pub original: String,
    /// Every distinct spelling seen, in first-seen order.
    pub spellings: Vec<String>,
    /// How many literals in the input resolved to this colour.
    pub count: usize,
    /// Byte offset of the first occurrence — drives the `first_seen` sort.
    pub first_at: usize,
}

// ---------------------------------------------------------------------------
// CSS colour keywords (CSS Color Module Level 4). Kept sorted for binary search;
// `table_is_sorted` guards that invariant. `transparent` is handled separately
// because it is the only keyword with a non-1 alpha.
// ---------------------------------------------------------------------------
const NAMED: &[(&str, u32)] = &[
    ("aliceblue", 0xF0F8FF),
    ("antiquewhite", 0xFAEBD7),
    ("aqua", 0x00FFFF),
    ("aquamarine", 0x7FFFD4),
    ("azure", 0xF0FFFF),
    ("beige", 0xF5F5DC),
    ("bisque", 0xFFE4C4),
    ("black", 0x000000),
    ("blanchedalmond", 0xFFEBCD),
    ("blue", 0x0000FF),
    ("blueviolet", 0x8A2BE2),
    ("brown", 0xA52A2A),
    ("burlywood", 0xDEB887),
    ("cadetblue", 0x5F9EA0),
    ("chartreuse", 0x7FFF00),
    ("chocolate", 0xD2691E),
    ("coral", 0xFF7F50),
    ("cornflowerblue", 0x6495ED),
    ("cornsilk", 0xFFF8DC),
    ("crimson", 0xDC143C),
    ("cyan", 0x00FFFF),
    ("darkblue", 0x00008B),
    ("darkcyan", 0x008B8B),
    ("darkgoldenrod", 0xB8860B),
    ("darkgray", 0xA9A9A9),
    ("darkgreen", 0x006400),
    ("darkgrey", 0xA9A9A9),
    ("darkkhaki", 0xBDB76B),
    ("darkmagenta", 0x8B008B),
    ("darkolivegreen", 0x556B2F),
    ("darkorange", 0xFF8C00),
    ("darkorchid", 0x9932CC),
    ("darkred", 0x8B0000),
    ("darksalmon", 0xE9967A),
    ("darkseagreen", 0x8FBC8F),
    ("darkslateblue", 0x483D8B),
    ("darkslategray", 0x2F4F4F),
    ("darkslategrey", 0x2F4F4F),
    ("darkturquoise", 0x00CED1),
    ("darkviolet", 0x9400D3),
    ("deeppink", 0xFF1493),
    ("deepskyblue", 0x00BFFF),
    ("dimgray", 0x696969),
    ("dimgrey", 0x696969),
    ("dodgerblue", 0x1E90FF),
    ("firebrick", 0xB22222),
    ("floralwhite", 0xFFFAF0),
    ("forestgreen", 0x228B22),
    ("fuchsia", 0xFF00FF),
    ("gainsboro", 0xDCDCDC),
    ("ghostwhite", 0xF8F8FF),
    ("gold", 0xFFD700),
    ("goldenrod", 0xDAA520),
    ("gray", 0x808080),
    ("green", 0x008000),
    ("greenyellow", 0xADFF2F),
    ("grey", 0x808080),
    ("honeydew", 0xF0FFF0),
    ("hotpink", 0xFF69B4),
    ("indianred", 0xCD5C5C),
    ("indigo", 0x4B0082),
    ("ivory", 0xFFFFF0),
    ("khaki", 0xF0E68C),
    ("lavender", 0xE6E6FA),
    ("lavenderblush", 0xFFF0F5),
    ("lawngreen", 0x7CFC00),
    ("lemonchiffon", 0xFFFACD),
    ("lightblue", 0xADD8E6),
    ("lightcoral", 0xF08080),
    ("lightcyan", 0xE0FFFF),
    ("lightgoldenrodyellow", 0xFAFAD2),
    ("lightgray", 0xD3D3D3),
    ("lightgreen", 0x90EE90),
    ("lightgrey", 0xD3D3D3),
    ("lightpink", 0xFFB6C1),
    ("lightsalmon", 0xFFA07A),
    ("lightseagreen", 0x20B2AA),
    ("lightskyblue", 0x87CEFA),
    ("lightslategray", 0x778899),
    ("lightslategrey", 0x778899),
    ("lightsteelblue", 0xB0C4DE),
    ("lightyellow", 0xFFFFE0),
    ("lime", 0x00FF00),
    ("limegreen", 0x32CD32),
    ("linen", 0xFAF0E6),
    ("magenta", 0xFF00FF),
    ("maroon", 0x800000),
    ("mediumaquamarine", 0x66CDAA),
    ("mediumblue", 0x0000CD),
    ("mediumorchid", 0xBA55D3),
    ("mediumpurple", 0x9370DB),
    ("mediumseagreen", 0x3CB371),
    ("mediumslateblue", 0x7B68EE),
    ("mediumspringgreen", 0x00FA9A),
    ("mediumturquoise", 0x48D1CC),
    ("mediumvioletred", 0xC71585),
    ("midnightblue", 0x191970),
    ("mintcream", 0xF5FFFA),
    ("mistyrose", 0xFFE4E1),
    ("moccasin", 0xFFE4B5),
    ("navajowhite", 0xFFDEAD),
    ("navy", 0x000080),
    ("oldlace", 0xFDF5E6),
    ("olive", 0x808000),
    ("olivedrab", 0x6B8E23),
    ("orange", 0xFFA500),
    ("orangered", 0xFF4500),
    ("orchid", 0xDA70D6),
    ("palegoldenrod", 0xEEE8AA),
    ("palegreen", 0x98FB98),
    ("paleturquoise", 0xAFEEEE),
    ("palevioletred", 0xDB7093),
    ("papayawhip", 0xFFEFD5),
    ("peachpuff", 0xFFDAB9),
    ("peru", 0xCD853F),
    ("pink", 0xFFC0CB),
    ("plum", 0xDDA0DD),
    ("powderblue", 0xB0E0E6),
    ("purple", 0x800080),
    ("rebeccapurple", 0x663399),
    ("red", 0xFF0000),
    ("rosybrown", 0xBC8F8F),
    ("royalblue", 0x4169E1),
    ("saddlebrown", 0x8B4513),
    ("salmon", 0xFA8072),
    ("sandybrown", 0xF4A460),
    ("seagreen", 0x2E8B57),
    ("seashell", 0xFFF5EE),
    ("sienna", 0xA0522D),
    ("silver", 0xC0C0C0),
    ("skyblue", 0x87CEEB),
    ("slateblue", 0x6A5ACD),
    ("slategray", 0x708090),
    ("slategrey", 0x708090),
    ("snow", 0xFFFAFA),
    ("springgreen", 0x00FF7F),
    ("steelblue", 0x4682B4),
    ("tan", 0xD2B48C),
    ("teal", 0x008080),
    ("thistle", 0xD8BFD8),
    ("tomato", 0xFF6347),
    ("turquoise", 0x40E0D0),
    ("violet", 0xEE82EE),
    ("wheat", 0xF5DEB3),
    ("white", 0xFFFFFF),
    ("whitesmoke", 0xF5F5F5),
    ("yellow", 0xFFFF00),
    ("yellowgreen", 0x9ACD32),
];

fn lookup_named(lower: &str) -> Option<Rgba> {
    if lower == "transparent" {
        return Some(Rgba { r: 0, g: 0, b: 0, a: 0.0 });
    }
    NAMED
        .binary_search_by(|(n, _)| (*n).cmp(lower))
        .ok()
        .map(|i| {
            let v = NAMED[i].1;
            Rgba::opaque((v >> 16) as u8, (v >> 8) as u8, v as u8)
        })
}

// ---------------------------------------------------------------------------
// Colour-space maths
// ---------------------------------------------------------------------------

fn hsl_to_rgb(h: f64, s: f64, l: f64) -> (u8, u8, u8) {
    let h = ((h % 360.0) + 360.0) % 360.0;
    let s = s.clamp(0.0, 1.0);
    let l = l.clamp(0.0, 1.0);
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = h / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r1, g1, b1) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    let f = |v: f64| ((v + m) * 255.0).round().clamp(0.0, 255.0) as u8;
    (f(r1), f(g1), f(b1))
}

fn hwb_to_rgb(h: f64, w: f64, b: f64) -> (u8, u8, u8) {
    let w = w.clamp(0.0, 1.0);
    let b = b.clamp(0.0, 1.0);
    if w + b >= 1.0 {
        let g = (w / (w + b) * 255.0).round().clamp(0.0, 255.0) as u8;
        return (g, g, g);
    }
    let (r0, g0, b0) = hsl_to_rgb(h, 1.0, 0.5);
    let mix = |c: u8| (((c as f64 / 255.0) * (1.0 - w - b) + w) * 255.0).round().clamp(0.0, 255.0) as u8;
    (mix(r0), mix(g0), mix(b0))
}

/// Returns hue in degrees, saturation and lightness in 0..=1.
pub fn rgb_to_hsl(c: Rgba) -> (f64, f64, f64) {
    let (r, g, b) = (c.r as f64 / 255.0, c.g as f64 / 255.0, c.b as f64 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let l = (max + min) / 2.0;
    if (max - min).abs() < f64::EPSILON {
        return (0.0, 0.0, l);
    }
    let d = max - min;
    let s = d / (1.0 - (2.0 * l - 1.0).abs());
    let h = if max == r {
        60.0 * (((g - b) / d) % 6.0)
    } else if max == g {
        60.0 * ((b - r) / d + 2.0)
    } else {
        60.0 * ((r - g) / d + 4.0)
    };
    (((h % 360.0) + 360.0) % 360.0, s, l)
}

fn rgb_to_hwb(c: Rgba) -> (f64, f64, f64) {
    let (h, _, _) = rgb_to_hsl(c);
    let (r, g, b) = (c.r as f64 / 255.0, c.g as f64 / 255.0, c.b as f64 / 255.0);
    (h, r.min(g).min(b), 1.0 - r.max(g).max(b))
}

// ---------------------------------------------------------------------------
// Formatting
// ---------------------------------------------------------------------------

/// Trim a float to at most `places` decimals with no trailing zeros: 0.5 → "0.5",
/// 1.0 → "1", 0.333333 → "0.333".
fn num(v: f64, places: usize) -> String {
    let mut s = format!("{v:.places$}");
    if s.contains('.') {
        while s.ends_with('0') {
            s.pop();
        }
        if s.ends_with('.') {
            s.pop();
        }
    }
    if s == "-0" {
        s = "0".into();
    }
    s
}

fn fmt_hex(c: Rgba, upper: bool) -> String {
    let s = if c.a >= 1.0 {
        format!("#{:02x}{:02x}{:02x}", c.r, c.g, c.b)
    } else {
        let a = (c.a * 255.0).round().clamp(0.0, 255.0) as u8;
        format!("#{:02x}{:02x}{:02x}{:02x}", c.r, c.g, c.b, a)
    };
    if upper {
        // Keep the leading '#'; only the digits change case.
        format!("#{}", s[1..].to_ascii_uppercase())
    } else {
        s
    }
}

fn fmt_rgb(c: Rgba) -> String {
    if c.a >= 1.0 {
        format!("rgb({}, {}, {})", c.r, c.g, c.b)
    } else {
        format!("rgba({}, {}, {}, {})", c.r, c.g, c.b, num(c.a, 3))
    }
}

fn fmt_hsl(c: Rgba) -> String {
    let (h, s, l) = rgb_to_hsl(c);
    if c.a >= 1.0 {
        format!("hsl({}, {}%, {}%)", num(h, 1), num(s * 100.0, 1), num(l * 100.0, 1))
    } else {
        format!(
            "hsla({}, {}%, {}%, {})",
            num(h, 1),
            num(s * 100.0, 1),
            num(l * 100.0, 1),
            num(c.a, 3)
        )
    }
}

fn fmt_hwb(c: Rgba) -> String {
    let (h, w, b) = rgb_to_hwb(c);
    if c.a >= 1.0 {
        format!("hwb({} {}% {}%)", num(h, 1), num(w * 100.0, 1), num(b * 100.0, 1))
    } else {
        format!(
            "hwb({} {}% {}% / {})",
            num(h, 1),
            num(w * 100.0, 1),
            num(b * 100.0, 1),
            num(c.a, 3)
        )
    }
}

/// The nearest CSS keyword, but only when the colour matches one EXACTLY.
fn fmt_name(c: Rgba) -> Option<String> {
    if c.a <= 0.0 && c.r == 0 && c.g == 0 && c.b == 0 {
        return Some("transparent".into());
    }
    if c.a < 1.0 {
        return None;
    }
    let want = ((c.r as u32) << 16) | ((c.g as u32) << 8) | c.b as u32;
    NAMED.iter().find(|(_, v)| *v == want).map(|(n, _)| (*n).to_string())
}

fn render_value(e: &PaletteEntry, color_format: &str, upper: bool) -> String {
    match color_format {
        "hex" => fmt_hex(e.color, upper),
        "rgb" => fmt_rgb(e.color),
        "hsl" => fmt_hsl(e.color),
        "hwb" => fmt_hwb(e.color),
        "name" => fmt_name(e.color).unwrap_or_else(|| fmt_hex(e.color, upper)),
        _ => e.original.clone(),
    }
}

// ---------------------------------------------------------------------------
// Scanning
// ---------------------------------------------------------------------------

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'-' || b == b'_'
}

/// A `#` or bare keyword directly after one of these is a selector, an entity or a
/// preprocessor variable name — `#header`, `.red`, `@brand`, `$blue`, `&#65;` — not
/// a colour literal.
fn blocks_word_start(prev: u8) -> bool {
    is_ident_byte(prev) || matches!(prev, b'#' | b'.' | b'@' | b'$' | b'&')
}

fn parse_hex_digits(digits: &str) -> Option<Rgba> {
    let d = digits.as_bytes();
    let h = |i: usize| u8::from_str_radix(&digits[i..i + 1], 16).ok();
    let hh = |i: usize| u8::from_str_radix(&digits[i..i + 2], 16).ok();
    match d.len() {
        3 => {
            let (r, g, b) = (h(0)?, h(1)?, h(2)?);
            Some(Rgba::opaque(r * 17, g * 17, b * 17))
        }
        4 => {
            let (r, g, b, a) = (h(0)?, h(1)?, h(2)?, h(3)?);
            Some(Rgba { r: r * 17, g: g * 17, b: b * 17, a: (a * 17) as f64 / 255.0 })
        }
        6 => Some(Rgba::opaque(hh(0)?, hh(2)?, hh(4)?)),
        8 => Some(Rgba { r: hh(0)?, g: hh(2)?, b: hh(4)?, a: hh(6)? as f64 / 255.0 }),
        _ => None,
    }
}

/// A bare number, a percentage, an angle (`deg`/`grad`/`rad`/`turn`) or the CSS-4
/// `none` keyword. Returns the raw number plus whether a `%` was present.
fn parse_token(tok: &str) -> Option<(f64, bool)> {
    let t = tok.trim();
    if t.is_empty() {
        return None;
    }
    if t.eq_ignore_ascii_case("none") {
        return Some((0.0, false));
    }
    if let Some(stripped) = t.strip_suffix('%') {
        return stripped.trim().parse::<f64>().ok().map(|v| (v, true));
    }
    for (suffix, scale) in [
        ("deg", 1.0),
        ("grad", 0.9),
        ("turn", 360.0),
        ("rad", 180.0 / core::f64::consts::PI),
    ] {
        if t.len() > suffix.len() && t[t.len() - suffix.len()..].eq_ignore_ascii_case(suffix) {
            return t[..t.len() - suffix.len()].trim().parse::<f64>().ok().map(|v| (v * scale, false));
        }
    }
    t.parse::<f64>().ok().map(|v| (v, false))
}

fn parse_alpha(tok: &str) -> Option<f64> {
    let (v, pct) = parse_token(tok)?;
    Some((if pct { v / 100.0 } else { v }).clamp(0.0, 1.0))
}

fn parse_rgb_channel(tok: &str) -> Option<u8> {
    let (v, pct) = parse_token(tok)?;
    let v = if pct { v / 100.0 * 255.0 } else { v };
    Some(v.round().clamp(0.0, 255.0) as u8)
}

/// Parse `rgb(...)` / `hsl(...)` / `hwb(...)` starting at the `(` byte offset.
/// Returns the colour plus the byte offset just past the closing `)`.
fn parse_function(text: &str, name: &str, open: usize) -> Option<(Rgba, usize)> {
    let bytes = text.as_bytes();
    let mut close = None;
    for (off, b) in bytes.iter().enumerate().skip(open + 1) {
        match b {
            // A nested call (calc(), var()) is not statically resolvable — skip it.
            b'(' => return None,
            b')' => {
                close = Some(off);
                break;
            }
            _ => {}
        }
    }
    let close = close?;
    let inner = text.get(open + 1..close)?;
    if inner.len() > 200 {
        return None;
    }
    // Legacy `a, b, c` and modern `a b c / alpha` both normalise to whitespace
    // separated tokens plus an optional alpha after a single slash.
    let normalised = inner.replace(',', " ");
    let mut sides = normalised.split('/');
    let main: Vec<&str> = sides.next()?.split_whitespace().collect();
    let slash_alpha = sides.next();
    if sides.next().is_some() {
        return None;
    }
    let (comps, legacy_alpha) = match main.len() {
        3 => (&main[..3], None),
        4 if slash_alpha.is_none() => (&main[..3], Some(main[3])),
        _ => return None,
    };
    let alpha = match slash_alpha.or(legacy_alpha) {
        Some(t) => parse_alpha(t)?,
        None => 1.0,
    };
    let (r, g, b) = match name {
        "rgb" | "rgba" => (
            parse_rgb_channel(comps[0])?,
            parse_rgb_channel(comps[1])?,
            parse_rgb_channel(comps[2])?,
        ),
        "hsl" | "hsla" => {
            let (h, _) = parse_token(comps[0])?;
            let (s, _) = parse_token(comps[1])?;
            let (l, _) = parse_token(comps[2])?;
            hsl_to_rgb(h, s / 100.0, l / 100.0)
        }
        "hwb" => {
            let (h, _) = parse_token(comps[0])?;
            let (w, _) = parse_token(comps[1])?;
            let (bl, _) = parse_token(comps[2])?;
            hwb_to_rgb(h, w / 100.0, bl / 100.0)
        }
        _ => return None,
    };
    Some((Rgba { r, g, b, a: alpha }, close + 1))
}

/// Every colour literal in `text`, in source order, as (byte offset, literal, colour).
pub fn scan(text: &str, include_named: bool) -> Vec<(usize, String, Rgba)> {
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        let prev = if i > 0 { bytes[i - 1] } else { b' ' };
        if b == b'#' {
            if !blocks_word_start(prev) {
                let start = i + 1;
                let mut j = start;
                while j < bytes.len() && bytes[j].is_ascii_hexdigit() {
                    j += 1;
                }
                let n = j - start;
                let clean_end = j >= bytes.len() || !is_ident_byte(bytes[j]);
                // `#abc {` is an id selector, not a three-digit hex colour — a
                // colour literal is never followed by a rule block.
                let mut k = j;
                while k < bytes.len() && bytes[k].is_ascii_whitespace() {
                    k += 1;
                }
                let is_selector = k < bytes.len() && bytes[k] == b'{';
                if clean_end && !is_selector && matches!(n, 3 | 4 | 6 | 8) {
                    if let Some(c) = parse_hex_digits(&text[start..j]) {
                        out.push((i, text[i..j].to_string(), c));
                        i = j;
                        continue;
                    }
                }
            }
            i += 1;
            continue;
        }
        if b.is_ascii_alphabetic() {
            let mut j = i;
            while j < bytes.len() && is_ident_byte(bytes[j]) {
                j += 1;
            }
            if !blocks_word_start(prev) {
                let lower = text[i..j].to_ascii_lowercase();
                let mut k = j;
                while k < bytes.len() && (bytes[k] == b' ' || bytes[k] == b'\t') {
                    k += 1;
                }
                let is_fn = matches!(lower.as_str(), "rgb" | "rgba" | "hsl" | "hsla" | "hwb");
                if is_fn && k < bytes.len() && bytes[k] == b'(' {
                    if let Some((c, end)) = parse_function(text, &lower, k) {
                        out.push((i, text[i..end].to_string(), c));
                        i = end;
                        continue;
                    }
                } else if include_named && !is_fn {
                    if let Some(c) = lookup_named(&lower) {
                        out.push((i, text[i..j].to_string(), c));
                    }
                }
            }
            i = j.max(i + 1);
            continue;
        }
        i += 1;
    }
    out
}

// ---------------------------------------------------------------------------
// Output rendering
// ---------------------------------------------------------------------------

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

fn csv_field(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn slug_name(prefix: &str, idx: usize) -> String {
    format!("{prefix}-{}", idx + 1)
}

fn uses(count: usize) -> String {
    if count == 1 {
        "1 use".into()
    } else {
        format!("{count} uses")
    }
}

fn render_svg(entries: &[PaletteEntry], color_format: &str, upper: bool, counts: bool) -> String {
    const CELL_W: i32 = 160;
    const CELL_H: i32 = 132;
    const SWATCH: i32 = 140;
    const PAD: i32 = 16;
    let cols = entries.len().min(4).max(1) as i32;
    let rows = ((entries.len() as i32) + cols - 1) / cols;
    let w = PAD + cols * CELL_W;
    let h = PAD + rows * CELL_H;
    let mut s = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w}\" height=\"{h}\" viewBox=\"0 0 {w} {h}\" role=\"img\" aria-label=\"Extracted colour palette\">\n"
    );
    s.push_str(&format!("  <rect width=\"{w}\" height=\"{h}\" fill=\"#ffffff\"/>\n"));
    s.push_str("  <g font-family=\"ui-monospace, SFMono-Regular, Menlo, monospace\" font-size=\"12\">\n");
    for (i, e) in entries.iter().enumerate() {
        let col = (i as i32) % cols;
        let row = (i as i32) / cols;
        let x = PAD + col * CELL_W;
        let y = PAD + row * CELL_H;
        let hex = fmt_hex(Rgba { a: 1.0, ..e.color }, false);
        let label = xml_escape(&render_value(e, color_format, upper));
        s.push_str(&format!(
            "    <rect x=\"{x}\" y=\"{y}\" width=\"{SWATCH}\" height=\"88\" rx=\"6\" fill=\"{hex}\" fill-opacity=\"{}\" stroke=\"#d6d6d6\"/>\n",
            num(e.color.a, 3)
        ));
        s.push_str(&format!(
            "    <text x=\"{x}\" y=\"{}\" fill=\"#111111\">{label}</text>\n",
            y + 106
        ));
        if counts {
            s.push_str(&format!(
                "    <text x=\"{x}\" y=\"{}\" fill=\"#666666\">{}</text>\n",
                y + 122,
                uses(e.count)
            ));
        }
    }
    s.push_str("  </g>\n</svg>\n");
    s
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

const OUTPUT_FORMATS: [&str; 8] = ["list", "csv", "json", "css_vars", "scss", "less", "tailwind", "svg"];
const COLOR_FORMATS: [&str; 6] = ["hex", "original", "rgb", "hsl", "hwb", "name"];
const SORTS: [&str; 5] = ["first_seen", "frequency", "hue", "lightness", "alphabetical"];

fn bad_choice(param: &str, got: &str, allowed: &[&str]) -> String {
    format!("unknown {param} '{got}' — expected one of: {}", allowed.join(", "))
}

/// Extract, deduplicate and render every colour literal found in `text`.
#[allow(clippy::too_many_arguments)]
pub fn extract(
    text: &str,
    output_format: &str,
    color_format: &str,
    sort: &str,
    include_counts: bool,
    include_named: bool,
    exclude_grey: bool,
    exclude_monochrome: bool,
    uppercase: bool,
    limit: i64,
    var_prefix: &str,
) -> Result<String, String> {
    if text.trim().is_empty() {
        return Err("input text is empty — paste some CSS, HTML or any text containing colour values".into());
    }
    if text.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "input is {} bytes, over the {MAX_INPUT_BYTES}-byte limit",
            text.len()
        ));
    }
    let output_format = if output_format.trim().is_empty() { "list" } else { output_format.trim() };
    let color_format = if color_format.trim().is_empty() { "hex" } else { color_format.trim() };
    let sort = if sort.trim().is_empty() { "first_seen" } else { sort.trim() };
    if !OUTPUT_FORMATS.contains(&output_format) {
        return Err(bad_choice("output_format", output_format, &OUTPUT_FORMATS));
    }
    if !COLOR_FORMATS.contains(&color_format) {
        return Err(bad_choice("color_format", color_format, &COLOR_FORMATS));
    }
    if !SORTS.contains(&sort) {
        return Err(bad_choice("sort", sort, &SORTS));
    }
    if !(0..=MAX_LIMIT).contains(&limit) {
        return Err(format!(
            "limit must be between 0 (no limit) and {MAX_LIMIT}, got {limit}"
        ));
    }
    let prefix = var_prefix.trim();
    let prefix = if prefix.is_empty() { "color" } else { prefix };
    if !prefix.bytes().all(is_ident_byte) {
        return Err(format!(
            "var_prefix '{prefix}' must contain only letters, digits, hyphens and underscores"
        ));
    }

    let hits = scan(text, include_named);
    if hits.is_empty() {
        return Err("no colour values found — this tool looks for #rgb/#rrggbb hex, rgb()/rgba(), hsl()/hsla(), hwb() and CSS colour keywords".into());
    }

    // Deduplicate by colour, preserving first-seen order and every spelling.
    let mut entries: Vec<PaletteEntry> = Vec::new();
    for (at, literal, color) in hits {
        let key = color.key();
        match entries.iter_mut().find(|e| e.color.key() == key) {
            Some(e) => {
                e.count += 1;
                if !e.spellings.iter().any(|s| s == &literal) {
                    e.spellings.push(literal);
                }
            }
            None => entries.push(PaletteEntry {
                color,
                original: literal.clone(),
                spellings: vec![literal],
                count: 1,
                first_at: at,
            }),
        }
    }
    let total_matches: usize = entries.iter().map(|e| e.count).sum();

    if exclude_monochrome {
        entries.retain(|e| !e.color.is_grey());
    } else if exclude_grey {
        entries.retain(|e| !e.color.is_grey() || e.color.is_black_or_white());
    }
    if entries.is_empty() {
        return Err("every colour found was filtered out — turn off the grey/monochrome filters to see them".into());
    }

    match sort {
        "frequency" => entries.sort_by(|a, b| b.count.cmp(&a.count).then(a.first_at.cmp(&b.first_at))),
        "hue" => entries.sort_by(|a, b| {
            let (ha, sa, la) = rgb_to_hsl(a.color);
            let (hb, sb, lb) = rgb_to_hsl(b.color);
            ha.total_cmp(&hb).then(sb.total_cmp(&sa)).then(la.total_cmp(&lb))
        }),
        "lightness" => entries.sort_by(|a, b| {
            let (_, _, la) = rgb_to_hsl(a.color);
            let (_, _, lb) = rgb_to_hsl(b.color);
            la.total_cmp(&lb).then(a.first_at.cmp(&b.first_at))
        }),
        "alphabetical" => entries.sort_by(|a, b| {
            render_value(a, color_format, uppercase).cmp(&render_value(b, color_format, uppercase))
        }),
        _ => entries.sort_by_key(|e| e.first_at),
    }
    if limit > 0 {
        entries.truncate(limit as usize);
    }

    let values: Vec<String> = entries.iter().map(|e| render_value(e, color_format, uppercase)).collect();

    let out = match output_format {
        "list" => {
            if include_counts {
                let width = values.iter().map(|v| v.chars().count()).max().unwrap_or(0);
                entries
                    .iter()
                    .zip(&values)
                    .map(|(e, v)| {
                        let pad = " ".repeat(width - v.chars().count());
                        format!("{v}{pad}  ×{}", e.count)
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            } else {
                values.join("\n")
            }
        }
        "csv" => {
            let mut s = String::from("color,hex,rgb,hsl,alpha");
            if include_counts {
                s.push_str(",count");
            }
            s.push('\n');
            for (e, v) in entries.iter().zip(&values) {
                s.push_str(&csv_field(v));
                s.push(',');
                s.push_str(&csv_field(&fmt_hex(e.color, uppercase)));
                s.push(',');
                s.push_str(&csv_field(&fmt_rgb(e.color)));
                s.push(',');
                s.push_str(&csv_field(&fmt_hsl(e.color)));
                s.push(',');
                s.push_str(&num(e.color.a, 3));
                if include_counts {
                    s.push_str(&format!(",{}", e.count));
                }
                s.push('\n');
            }
            s.trim_end().to_string()
        }
        "json" => {
            let mut s = String::from("{\n");
            s.push_str(&format!("  \"total_matches\": {total_matches},\n"));
            s.push_str(&format!("  \"unique_colors\": {},\n", entries.len()));
            s.push_str("  \"colors\": [\n");
            for (i, (e, v)) in entries.iter().zip(&values).enumerate() {
                let (h, sat, l) = rgb_to_hsl(e.color);
                let spellings: Vec<String> =
                    e.spellings.iter().map(|s| format!("\"{}\"", json_escape(s))).collect();
                s.push_str("    {\n");
                s.push_str(&format!("      \"value\": \"{}\",\n", json_escape(v)));
                s.push_str(&format!("      \"hex\": \"{}\",\n", fmt_hex(e.color, uppercase)));
                s.push_str(&format!("      \"rgb\": \"{}\",\n", fmt_rgb(e.color)));
                s.push_str(&format!("      \"hsl\": \"{}\",\n", fmt_hsl(e.color)));
                s.push_str(&format!(
                    "      \"r\": {}, \"g\": {}, \"b\": {}, \"alpha\": {},\n",
                    e.color.r,
                    e.color.g,
                    e.color.b,
                    num(e.color.a, 3)
                ));
                s.push_str(&format!(
                    "      \"hue\": {}, \"saturation\": {}, \"lightness\": {},\n",
                    num(h, 1),
                    num(sat * 100.0, 1),
                    num(l * 100.0, 1)
                ));
                if let Some(n) = fmt_name(e.color) {
                    s.push_str(&format!("      \"name\": \"{n}\",\n"));
                }
                s.push_str(&format!("      \"count\": {},\n", e.count));
                s.push_str(&format!("      \"spellings\": [{}]\n", spellings.join(", ")));
                s.push_str(if i + 1 == entries.len() { "    }\n" } else { "    },\n" });
            }
            s.push_str("  ]\n}");
            s
        }
        "css_vars" => {
            let mut s = String::from(":root {\n");
            for (i, (e, v)) in entries.iter().zip(&values).enumerate() {
                let comment = if include_counts { format!(" /* {} */", uses(e.count)) } else { String::new() };
                s.push_str(&format!("  --{}: {v};{comment}\n", slug_name(prefix, i)));
            }
            s.push('}');
            s
        }
        "scss" | "less" => {
            let sigil = if output_format == "scss" { '$' } else { '@' };
            entries
                .iter()
                .zip(&values)
                .enumerate()
                .map(|(i, (e, v))| {
                    let comment = if include_counts { format!(" // {}", uses(e.count)) } else { String::new() };
                    format!("{sigil}{}: {v};{comment}", slug_name(prefix, i))
                })
                .collect::<Vec<_>>()
                .join("\n")
        }
        "tailwind" => {
            let mut s = String::from("module.exports = {\n  theme: {\n    extend: {\n      colors: {\n");
            for (i, v) in values.iter().enumerate() {
                s.push_str(&format!("        '{}': '{v}',\n", slug_name(prefix, i)));
            }
            s.push_str("      },\n    },\n  },\n};");
            s
        }
        _ => render_svg(&entries, color_format, uppercase, include_counts),
    };
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(text: &str, out: &str) -> String {
        extract(text, out, "hex", "first_seen", false, true, false, false, false, 0, "color").unwrap()
    }

    #[test]
    fn named_table_is_sorted_for_binary_search() {
        for pair in NAMED.windows(2) {
            assert!(pair[0].0 < pair[1].0, "{} must sort before {}", pair[0].0, pair[1].0);
        }
        assert_eq!(NAMED.len(), 148);
    }

    #[test]
    fn extracts_and_deduplicates_across_spellings() {
        let css = "a { color: #f00; } b { color: #FF0000; } c { color: red; } d { color: rgb(255, 0, 0); }";
        assert_eq!(run(css, "list"), "#ff0000");
    }

    #[test]
    fn counts_every_occurrence() {
        let css = ".a{color:#f00}.b{color:red}.c{background:#0000ff}";
        let out = extract(css, "list", "hex", "frequency", true, true, false, false, false, 0, "color").unwrap();
        assert_eq!(out, "#ff0000  ×2\n#0000ff  ×1");
    }

    #[test]
    fn parses_every_supported_notation() {
        let text = "#0f0 #00ff00 #0f0f #00ff0080 rgb(0 255 0) rgba(0,255,0,.5) \
                    hsl(120, 100%, 50%) hsla(120deg 100% 50% / 50%) hwb(120 0% 0%) lime";
        let out = run(text, "list");
        // opaque green, half-alpha green (from #0f0f == 0.533 alpha), #00ff0080, rgba .5
        assert!(out.starts_with("#00ff00\n"), "got {out}");
        assert!(out.contains("#00ff0080"), "8-digit alpha hex missing from {out}");
    }

    #[test]
    fn modern_slash_alpha_matches_legacy_rgba() {
        let a = run("rgb(255 0 0 / 0.5)", "list");
        let b = run("rgba(255, 0, 0, 0.5)", "list");
        assert_eq!(a, b);
        assert_eq!(a, "#ff000080");
    }

    #[test]
    fn hsl_round_trips_to_the_expected_hex() {
        assert_eq!(run("hsl(210, 50%, 40%)", "list"), "#336699");
        assert_eq!(run("hwb(60 20% 20%)", "list"), "#cccc33");
    }

    #[test]
    fn skips_selectors_variables_and_entities() {
        // #abc here is an id selector, .red a class, $blue/@blue preprocessor vars.
        let text = "#abc {} .red {} $blue: 1; @blue: 2; --brand-red: 3; &#65;";
        let err = extract(text, "list", "hex", "first_seen", false, true, false, false, false, 0, "color")
            .unwrap_err();
        assert!(err.contains("no colour values found"), "got {err}");
    }

    #[test]
    fn ignores_calc_and_var_inside_color_functions() {
        let text = "rgb(calc(1 + 1), 0, 0) hsl(var(--h), 100%, 50%) #123456";
        assert_eq!(run(text, "list"), "#123456");
    }

    #[test]
    fn named_colors_can_be_switched_off() {
        let text = "color: red; background: #00f;";
        let with = extract(text, "list", "hex", "first_seen", false, true, false, false, false, 0, "c").unwrap();
        let without =
            extract(text, "list", "hex", "first_seen", false, false, false, false, false, 0, "c").unwrap();
        assert_eq!(with, "#ff0000\n#0000ff");
        assert_eq!(without, "#0000ff");
    }

    #[test]
    fn grey_and_monochrome_filters_differ_on_black_and_white() {
        let text = "#000 #fff #888 #ff0000";
        let grey = extract(text, "list", "hex", "first_seen", false, true, true, false, false, 0, "c").unwrap();
        let mono = extract(text, "list", "hex", "first_seen", false, true, false, true, false, 0, "c").unwrap();
        assert_eq!(grey, "#000000\n#ffffff\n#ff0000");
        assert_eq!(mono, "#ff0000");
    }

    #[test]
    fn color_formats_render_the_same_color_four_ways() {
        let t = "#336699";
        let f = |fmt: &str| extract(t, "list", fmt, "first_seen", false, true, false, false, false, 0, "c").unwrap();
        assert_eq!(f("hex"), "#336699");
        assert_eq!(f("rgb"), "rgb(51, 102, 153)");
        assert_eq!(f("hsl"), "hsl(210, 50%, 40%)");
        assert_eq!(f("hwb"), "hwb(210 20% 40%)");
        assert_eq!(f("original"), "#336699");
        assert_eq!(f("name"), "#336699"); // no exact keyword → falls back to hex
        assert_eq!(
            extract("rgb(255,0,0)", "list", "name", "first_seen", false, true, false, false, false, 0, "c").unwrap(),
            "red"
        );
    }

    #[test]
    fn uppercase_only_affects_hex_digits() {
        let out = extract("#aabbcc", "list", "hex", "first_seen", false, true, false, false, true, 0, "c").unwrap();
        assert_eq!(out, "#AABBCC");
    }

    #[test]
    fn sorts_by_hue_lightness_and_alphabetically() {
        let t = "#0000ff #ff0000 #00ff00";
        let s = |k: &str| extract(t, "list", "hex", k, false, true, false, false, false, 0, "c").unwrap();
        assert_eq!(s("first_seen"), "#0000ff\n#ff0000\n#00ff00");
        assert_eq!(s("hue"), "#ff0000\n#00ff00\n#0000ff");
        assert_eq!(s("alphabetical"), "#0000ff\n#00ff00\n#ff0000");
        assert_eq!(
            extract("#000 #fff #888", "list", "hex", "lightness", false, true, false, false, false, 0, "c").unwrap(),
            "#000000\n#888888\n#ffffff"
        );
    }

    #[test]
    fn limit_truncates_after_sorting() {
        let t = ".a{color:#f00}.b{color:#f00}.c{color:#0f0}.d{color:#00f}";
        let out = extract(t, "list", "hex", "frequency", false, true, false, false, false, 2, "c").unwrap();
        assert_eq!(out, "#ff0000\n#00ff00");
    }

    #[test]
    fn renders_css_scss_less_and_tailwind() {
        let t = "#f00 #f00 #0f0";
        let vars = extract(t, "css_vars", "hex", "first_seen", true, true, false, false, false, 0, "brand").unwrap();
        assert_eq!(
            vars,
            ":root {\n  --brand-1: #ff0000; /* 2 uses */\n  --brand-2: #00ff00; /* 1 use */\n}"
        );
        let scss = extract(t, "scss", "hex", "first_seen", false, true, false, false, false, 0, "c").unwrap();
        assert_eq!(scss, "$c-1: #ff0000;\n$c-2: #00ff00;");
        let less = extract(t, "less", "hex", "first_seen", false, true, false, false, false, 0, "c").unwrap();
        assert_eq!(less, "@c-1: #ff0000;\n@c-2: #00ff00;");
        let tw = extract(t, "tailwind", "hex", "first_seen", false, true, false, false, false, 0, "c").unwrap();
        assert!(tw.contains("'c-1': '#ff0000',"), "got {tw}");
    }

    #[test]
    fn renders_csv_json_and_svg() {
        let csv = extract("#f00", "csv", "hex", "first_seen", true, true, false, false, false, 0, "c").unwrap();
        assert_eq!(
            csv,
            "color,hex,rgb,hsl,alpha,count\n#ff0000,#ff0000,\"rgb(255, 0, 0)\",\"hsl(0, 100%, 50%)\",1,1"
        );
        let json = run("#f00 red", "json");
        assert!(json.contains("\"total_matches\": 2"), "got {json}");
        assert!(json.contains("\"unique_colors\": 1"), "got {json}");
        assert!(json.contains("\"name\": \"red\""), "got {json}");
        assert!(json.contains("\"spellings\": [\"#f00\", \"red\"]"), "got {json}");
        let svg = run("#f00", "svg");
        assert!(svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\""), "got {svg}");
        assert!(svg.contains("fill=\"#ff0000\""), "got {svg}");
    }

    #[test]
    fn rejects_empty_input() {
        let err = extract("   ", "list", "hex", "first_seen", false, true, false, false, false, 0, "c").unwrap_err();
        assert!(err.contains("empty"), "got {err}");
    }

    #[test]
    fn rejects_unknown_options_and_bad_limit() {
        let base = "#f00";
        assert!(extract(base, "yaml", "hex", "first_seen", false, true, false, false, false, 0, "c")
            .unwrap_err()
            .contains("unknown output_format"));
        assert!(extract(base, "list", "lab", "first_seen", false, true, false, false, false, 0, "c")
            .unwrap_err()
            .contains("unknown color_format"));
        assert!(extract(base, "list", "hex", "random", false, true, false, false, false, 0, "c")
            .unwrap_err()
            .contains("unknown sort"));
        assert!(extract(base, "list", "hex", "first_seen", false, true, false, false, false, 9999, "c")
            .unwrap_err()
            .contains("limit must be"));
        assert!(extract(base, "css_vars", "hex", "first_seen", false, true, false, false, false, 0, "my prefix")
            .unwrap_err()
            .contains("var_prefix"));
    }

    #[test]
    fn rejects_input_with_no_colors_and_fully_filtered_output() {
        assert!(extract("the quick brown fox", "list", "hex", "first_seen", false, false, false, false, false, 0, "c")
            .unwrap_err()
            .contains("no colour values found"));
        assert!(extract("#888", "list", "hex", "first_seen", false, true, false, true, false, 0, "c")
            .unwrap_err()
            .contains("filtered out"));
    }

    #[test]
    fn rejects_oversized_input() {
        let big = "#f00 ".repeat(MAX_INPUT_BYTES / 5 + 1);
        let err = extract(&big, "list", "hex", "first_seen", false, true, false, false, false, 0, "c").unwrap_err();
        assert!(err.contains("over the"), "got {err}");
    }

    #[test]
    fn transparent_keyword_is_its_own_entry() {
        let out = run("color: transparent; background: #000;", "list");
        assert_eq!(out, "#00000000\n#000000");
    }
}
