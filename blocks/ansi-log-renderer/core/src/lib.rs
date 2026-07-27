//! ansi-log-renderer core — turn raw terminal output (ANSI escape codes) into
//! clean colored HTML, or strip the codes to plain text. Pure compute, no regex.
//!
//! A hand-rolled ECMA-48 scanner walks the bytes. In `html` mode it INTERPRETS
//! the SGR (Select Graphic Rendition) sequences — 16 basic + bright colors, the
//! 256-color xterm cube, 24-bit RGB truecolor, and bold/dim/italic/underline/
//! inverse/strikethrough/conceal — and reproduces them as styled `<span>`s
//! inside a themed `<pre>`. Non-SGR control (cursor moves, screen erase, OSC
//! titles/hyperlinks) is dropped. In `text` mode it delegates to the shared
//! strip-ansi-codes core to return plain readable text.
//!
//! All escape bytes are ASCII (`< 0x80`), so scanning bytes never splits a
//! multi-byte UTF-8 character — surrounding text (accents, emoji) is preserved.
//! The rendered inner content is built as a byte buffer (only ASCII markup +
//! the input's own bytes appended in order), so it round-trips to valid UTF-8.

const ESC: u8 = 0x1B; // \e — start of every 7-bit escape sequence
const BEL: u8 = 0x07; // \a — an alternate OSC string terminator (xterm)

/// Render `text` (raw terminal output with ANSI escape codes).
///
/// - `output`: `"html"` (default) renders colored HTML; `"text"` strips every
///   escape sequence and returns plain text.
/// - `theme`: `"dark"` (default) or `"light"` — the default foreground/background
///   colors and the wrapping `<pre>` background used when text sets no color.
/// - `styles`: `"inline"` (default) emits self-contained `style="..."` spans;
///   `"classes"` emits `class="ansi-..."` spans plus a `<style>` block defining
///   the palette (basic colors become classes; 256/RGB colors stay inline).
///
/// Returns `Err` on an unknown `output`, `theme`, or `styles`. Empty strings for
/// `output`/`theme`/`styles` fall back to their defaults. The result is UTF-8.
pub fn render(text: &str, output: &str, theme: &str, styles: &str) -> Result<String, String> {
    match output {
        "" | "html" => {}
        "text" => {
            // Plain-text mode: reuse the audited strip-ansi-codes scanner.
            return gizza_ai_strip_ansi_codes_core::strip(text, "all");
        }
        other => {
            return Err(format!(
                "invalid output {other:?}: expected \"html\" or \"text\""
            ))
        }
    }

    let theme = Theme::parse(theme)?;
    let use_classes = match styles {
        "" | "inline" => false,
        "classes" => true,
        other => {
            return Err(format!(
                "invalid styles {other:?}: expected \"inline\" or \"classes\""
            ))
        }
    };

    render_html(text, theme, use_classes)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Theme {
    Dark,
    Light,
}

impl Theme {
    fn parse(s: &str) -> Result<Theme, String> {
        match s {
            "" | "dark" => Ok(Theme::Dark),
            "light" => Ok(Theme::Light),
            other => Err(format!(
                "invalid theme {other:?}: expected \"dark\" or \"light\""
            )),
        }
    }
    /// Default (background, foreground) for the `<pre>` container.
    fn default_bg_fg(self) -> (&'static str, &'static str) {
        match self {
            Theme::Dark => ("#0c0c0c", "#cccccc"),
            Theme::Light => ("#ffffff", "#000000"),
        }
    }
    fn class(self) -> &'static str {
        match self {
            Theme::Dark => "ansi ansi--dark",
            Theme::Light => "ansi ansi--light",
        }
    }
}

/// A resolved SGR color: a basic 0–15 palette slot (nameable as a class), an
/// xterm 256-index, or a 24-bit RGB triple.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Color {
    Basic(u8),   // 0..=15
    Indexed(u8), // 0..=255 (via 38;5;n / 48;5;n)
    Rgb(u8, u8, u8),
}

impl Color {
    fn to_hex(self) -> String {
        let (r, g, b) = match self {
            Color::Basic(i) => BASIC_PALETTE[i as usize],
            Color::Indexed(i) => xterm256(i),
            Color::Rgb(r, g, b) => (r, g, b),
        };
        format!("#{r:02x}{g:02x}{b:02x}")
    }
    /// Class name for a basic 0–15 color (`red`, `bright-red`, …); `None` for
    /// indexed/RGB colors, which must fall back to an inline style.
    fn basic_name(self) -> Option<&'static str> {
        match self {
            Color::Basic(i) => Some(BASIC_NAMES[i as usize]),
            _ => None,
        }
    }
}

/// Classic xterm 16-color palette (dark-terminal defaults). Bright variants 8–15.
const BASIC_PALETTE: [(u8, u8, u8); 16] = [
    (0x00, 0x00, 0x00), // 0 black
    (0xcd, 0x00, 0x00), // 1 red
    (0x00, 0xcd, 0x00), // 2 green
    (0xcd, 0xcd, 0x00), // 3 yellow
    (0x00, 0x00, 0xee), // 4 blue
    (0xcd, 0x00, 0xcd), // 5 magenta
    (0x00, 0xcd, 0xcd), // 6 cyan
    (0xe5, 0xe5, 0xe5), // 7 white
    (0x7f, 0x7f, 0x7f), // 8 bright black (gray)
    (0xff, 0x00, 0x00), // 9 bright red
    (0x00, 0xff, 0x00), // 10 bright green
    (0xff, 0xff, 0x00), // 11 bright yellow
    (0x5c, 0x5c, 0xff), // 12 bright blue
    (0xff, 0x00, 0xff), // 13 bright magenta
    (0x00, 0xff, 0xff), // 14 bright cyan
    (0xff, 0xff, 0xff), // 15 bright white
];

const BASIC_NAMES: [&str; 16] = [
    "black",
    "red",
    "green",
    "yellow",
    "blue",
    "magenta",
    "cyan",
    "white",
    "bright-black",
    "bright-red",
    "bright-green",
    "bright-yellow",
    "bright-blue",
    "bright-magenta",
    "bright-cyan",
    "bright-white",
];

/// Resolve an xterm 256-color index to RGB: 0–15 palette, 16–231 the 6×6×6 cube,
/// 232–255 the 24-step grayscale ramp.
fn xterm256(i: u8) -> (u8, u8, u8) {
    match i {
        0..=15 => BASIC_PALETTE[i as usize],
        16..=231 => {
            let n = i - 16;
            let steps = [0u8, 95, 135, 175, 215, 255];
            let r = steps[(n / 36) as usize];
            let g = steps[((n / 6) % 6) as usize];
            let b = steps[(n % 6) as usize];
            (r, g, b)
        }
        232..=255 => {
            let v = 8 + 10 * (i - 232);
            (v, v, v)
        }
    }
}

/// The active SGR state as we walk the stream.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Style {
    fg: Option<Color>,
    bg: Option<Color>,
    bold: bool,
    dim: bool,
    italic: bool,
    underline: bool,
    inverse: bool,
    strike: bool,
    conceal: bool,
}

impl Style {
    fn default() -> Style {
        Style {
            fg: None,
            bg: None,
            bold: false,
            dim: false,
            italic: false,
            underline: false,
            inverse: false,
            strike: false,
            conceal: false,
        }
    }
    fn is_default(&self) -> bool {
        *self == Style::default()
    }
}

fn render_html(text: &str, theme: Theme, use_classes: bool) -> Result<String, String> {
    let (bg, fg) = theme.default_bg_fg();
    // Build the inner content as bytes: only ASCII markup and the input's own
    // bytes are appended, in order, so the buffer stays valid UTF-8.
    let mut inner: Vec<u8> = Vec::with_capacity(text.len() * 2);
    let mut style = Style::default();
    let mut open = false; // is a <span> currently open for `style`?

    let bytes = text.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == ESC && i + 1 < bytes.len() {
            match bytes[i + 1] {
                b'[' => {
                    let (end, params, final_byte) = scan_csi(bytes, i + 2);
                    if final_byte == Some(b'm') {
                        if open {
                            inner.extend_from_slice(b"</span>");
                            open = false;
                        }
                        apply_sgr(&mut style, &params);
                    }
                    // Non-SGR CSI (cursor/erase) is dropped.
                    i = end;
                    continue;
                }
                // OSC / DCS / SOS / PM / APC strings: dropped.
                b']' | b'P' | b'X' | b'^' | b'_' => {
                    i = scan_string(bytes, i + 2);
                    continue;
                }
                // Any other ESC Fe/nF sequence: dropped.
                _ => {
                    i = scan_esc(bytes, i + 1);
                    continue;
                }
            }
        }
        if b == ESC {
            // Trailing lone ESC — drop it.
            i += 1;
            continue;
        }

        // Ordinary text byte. Open/close a span to reflect the current style.
        if style.is_default() {
            if open {
                inner.extend_from_slice(b"</span>");
                open = false;
            }
        } else if !open {
            inner.extend_from_slice(open_span(&style, use_classes).as_bytes());
            open = true;
        }
        push_escaped(&mut inner, b);
        i += 1;
    }
    if open {
        inner.extend_from_slice(b"</span>");
    }

    let inner = String::from_utf8(inner)
        .map_err(|e| format!("internal: produced invalid UTF-8 ({e})"))?;

    let mut out = String::new();
    if use_classes {
        out.push_str(&class_stylesheet());
        out.push_str(&format!("<pre class=\"{}\">", theme.class()));
    } else {
        out.push_str(&format!(
            "<pre style=\"background-color:{bg};color:{fg};padding:1rem;overflow:auto;font-family:ui-monospace,monospace;line-height:1.5\">"
        ));
    }
    out.push_str(&inner);
    out.push_str("</pre>");
    Ok(out)
}

/// Push one text byte, HTML-escaping the markup-significant ASCII characters.
/// Non-ASCII UTF-8 continuation/lead bytes pass through verbatim (they can never
/// equal an escaped ASCII char), so the byte run reconstructs the original text.
fn push_escaped(out: &mut Vec<u8>, b: u8) {
    match b {
        b'&' => out.extend_from_slice(b"&amp;"),
        b'<' => out.extend_from_slice(b"&lt;"),
        b'>' => out.extend_from_slice(b"&gt;"),
        b'"' => out.extend_from_slice(b"&quot;"),
        _ => out.push(b),
    }
}

/// Build the opening `<span>` for a non-default style.
fn open_span(style: &Style, use_classes: bool) -> String {
    // Resolve effective fg/bg, honoring inverse (swap) and conceal (hide fg).
    let (mut fg, mut bg) = (style.fg, style.bg);
    if style.inverse {
        std::mem::swap(&mut fg, &mut bg);
    }
    if use_classes {
        let mut classes: Vec<String> = Vec::new();
        let mut inline: Vec<String> = Vec::new();
        if style.bold {
            classes.push("ansi-bold".into());
        }
        if style.dim {
            classes.push("ansi-dim".into());
        }
        if style.italic {
            classes.push("ansi-italic".into());
        }
        if style.underline {
            classes.push("ansi-underline".into());
        }
        if style.strike {
            classes.push("ansi-strike".into());
        }
        if style.conceal {
            classes.push("ansi-conceal".into());
        } else if let Some(c) = fg {
            match c.basic_name() {
                Some(name) => classes.push(format!("ansi-fg-{name}")),
                None => inline.push(format!("color:{}", c.to_hex())),
            }
        }
        if let Some(c) = bg {
            match c.basic_name() {
                Some(name) => classes.push(format!("ansi-bg-{name}")),
                None => inline.push(format!("background-color:{}", c.to_hex())),
            }
        }
        let mut s = String::from("<span");
        if !classes.is_empty() {
            s.push_str(&format!(" class=\"{}\"", classes.join(" ")));
        }
        if !inline.is_empty() {
            s.push_str(&format!(" style=\"{}\"", inline.join(";")));
        }
        s.push('>');
        s
    } else {
        let mut decls: Vec<String> = Vec::new();
        if style.conceal {
            decls.push("color:transparent".into());
        } else if let Some(c) = fg {
            decls.push(format!("color:{}", c.to_hex()));
        }
        if let Some(c) = bg {
            decls.push(format!("background-color:{}", c.to_hex()));
        }
        if style.bold {
            decls.push("font-weight:bold".into());
        }
        if style.dim {
            decls.push("opacity:0.7".into());
        }
        if style.italic {
            decls.push("font-style:italic".into());
        }
        let mut deco: Vec<&str> = Vec::new();
        if style.underline {
            deco.push("underline");
        }
        if style.strike {
            deco.push("line-through");
        }
        if !deco.is_empty() {
            decls.push(format!("text-decoration:{}", deco.join(" ")));
        }
        format!("<span style=\"{}\">", decls.join(";"))
    }
}

/// The `<style>` block emitted ahead of a classes-mode `<pre>`, so the output is
/// self-contained. Defines the container, both theme backgrounds, all 16 fg/bg
/// colors, and the text styles. The `<pre>`'s theme class selects the background.
fn class_stylesheet() -> String {
    let (dbg, dfg) = Theme::Dark.default_bg_fg();
    let (lbg, lfg) = Theme::Light.default_bg_fg();
    let mut s = String::from("<style>\n");
    s.push_str(".ansi{padding:1rem;overflow:auto;font-family:ui-monospace,monospace;line-height:1.5}\n");
    s.push_str(&format!(".ansi--dark{{background-color:{dbg};color:{dfg}}}\n"));
    s.push_str(&format!(".ansi--light{{background-color:{lbg};color:{lfg}}}\n"));
    s.push_str(".ansi-bold{font-weight:bold}\n");
    s.push_str(".ansi-dim{opacity:0.7}\n");
    s.push_str(".ansi-italic{font-style:italic}\n");
    s.push_str(".ansi-underline{text-decoration:underline}\n");
    s.push_str(".ansi-strike{text-decoration:line-through}\n");
    s.push_str(".ansi-underline.ansi-strike{text-decoration:underline line-through}\n");
    s.push_str(".ansi-conceal{color:transparent}\n");
    for idx in 0..16u8 {
        let name = BASIC_NAMES[idx as usize];
        let hex = Color::Basic(idx).to_hex();
        s.push_str(&format!(".ansi-fg-{name}{{color:{hex}}}\n"));
        s.push_str(&format!(".ansi-bg-{name}{{background-color:{hex}}}\n"));
    }
    s.push_str("</style>\n");
    s
}

/// Apply a parsed SGR parameter list to `style`. An empty list means reset.
fn apply_sgr(style: &mut Style, params: &[u16]) {
    if params.is_empty() {
        *style = Style::default();
        return;
    }
    let mut i = 0;
    while i < params.len() {
        let p = params[i];
        match p {
            0 => *style = Style::default(),
            1 => style.bold = true,
            2 => style.dim = true,
            3 => style.italic = true,
            4 => style.underline = true,
            7 => style.inverse = true,
            8 => style.conceal = true,
            9 => style.strike = true,
            21 | 22 => {
                style.bold = false;
                style.dim = false;
            }
            23 => style.italic = false,
            24 => style.underline = false,
            27 => style.inverse = false,
            28 => style.conceal = false,
            29 => style.strike = false,
            30..=37 => style.fg = Some(Color::Basic((p - 30) as u8)),
            39 => style.fg = None,
            40..=47 => style.bg = Some(Color::Basic((p - 40) as u8)),
            49 => style.bg = None,
            90..=97 => style.fg = Some(Color::Basic((p - 90 + 8) as u8)),
            100..=107 => style.bg = Some(Color::Basic((p - 100 + 8) as u8)),
            38 | 48 => {
                let is_fg = p == 38;
                // 38;5;n (indexed) or 38;2;r;g;b (truecolor).
                if let Some(&mode) = params.get(i + 1) {
                    if mode == 5 {
                        if let Some(&n) = params.get(i + 2) {
                            let c = Color::Indexed(n as u8);
                            if is_fg {
                                style.fg = Some(c);
                            } else {
                                style.bg = Some(c);
                            }
                            i += 2;
                        }
                    } else if mode == 2 {
                        if let (Some(&r), Some(&g), Some(&bl)) =
                            (params.get(i + 2), params.get(i + 3), params.get(i + 4))
                        {
                            let c = Color::Rgb(r as u8, g as u8, bl as u8);
                            if is_fg {
                                style.fg = Some(c);
                            } else {
                                style.bg = Some(c);
                            }
                            i += 4;
                        }
                    }
                }
            }
            _ => {} // 5 blink, 6, etc. — no visual mapping, ignored
        }
        i += 1;
    }
}

/// Scan a CSI body from `start` (just after `ESC [`). Returns
/// `(index past the sequence, parsed numeric params, final byte if present)`.
/// Parameters are `;`-separated decimals; an empty field is 0 (SGR convention).
fn scan_csi(bytes: &[u8], start: usize) -> (usize, Vec<u16>, Option<u8>) {
    let mut i = start;
    let mut params: Vec<u16> = Vec::new();
    let mut cur: Option<u16> = None;
    while i < bytes.len() && (0x30..=0x3F).contains(&bytes[i]) {
        let c = bytes[i];
        if c.is_ascii_digit() {
            let d = (c - b'0') as u16;
            cur = Some(cur.unwrap_or(0).saturating_mul(10).saturating_add(d));
        } else if c == b';' {
            params.push(cur.take().unwrap_or(0));
        }
        // Other private-parameter bytes (`:<=>?`) are consumed but ignored.
        i += 1;
    }
    // A pending value, or a value after a trailing ';', closes the list.
    if let Some(v) = cur.take() {
        params.push(v);
    }
    // Skip intermediate bytes 0x20..=0x2F.
    while i < bytes.len() && (0x20..=0x2F).contains(&bytes[i]) {
        i += 1;
    }
    if i < bytes.len() && (0x40..=0x7E).contains(&bytes[i]) {
        (i + 1, params, Some(bytes[i]))
    } else {
        (i, params, None)
    }
}

/// Scan a string-type sequence (OSC/DCS/SOS/PM/APC) to its terminator — BEL or
/// ST (`ESC \`) — returning the index past it. Unterminated runs to end.
fn scan_string(bytes: &[u8], start: usize) -> usize {
    let mut i = start;
    while i < bytes.len() {
        if bytes[i] == BEL {
            return i + 1;
        }
        if bytes[i] == ESC && i + 1 < bytes.len() && bytes[i + 1] == b'\\' {
            return i + 2;
        }
        i += 1;
    }
    i
}

/// Scan a generic `ESC <Fe/nF>` sequence: intermediate bytes then a final byte.
fn scan_esc(bytes: &[u8], start: usize) -> usize {
    let mut i = start;
    while i < bytes.len() && (0x20..=0x2F).contains(&bytes[i]) {
        i += 1;
    }
    if i < bytes.len() {
        i + 1
    } else {
        i
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_renders_basic_color() {
        let out = render("\x1b[31mERROR\x1b[0m ok", "html", "dark", "inline").unwrap();
        assert!(out.starts_with("<pre style=\"background-color:#0c0c0c;color:#cccccc"));
        assert!(out.contains("<span style=\"color:#cd0000\">ERROR</span>"));
        assert!(out.ends_with(" ok</pre>"));
    }

    #[test]
    fn html_escapes_entities() {
        let out = render("<b> & \"x\"", "html", "dark", "inline").unwrap();
        assert!(out.contains("&lt;b&gt; &amp; &quot;x&quot;"));
    }

    #[test]
    fn html_preserves_unicode() {
        let out = render("\x1b[32m✓ café 🚀\x1b[0m", "html", "dark", "inline").unwrap();
        assert!(out.contains("✓ café 🚀"));
    }

    #[test]
    fn html_bold_and_styles() {
        let out = render("\x1b[1;4;3mx\x1b[0m", "html", "dark", "inline").unwrap();
        assert!(out.contains("font-weight:bold"));
        assert!(out.contains("font-style:italic"));
        assert!(out.contains("text-decoration:underline"));
    }

    #[test]
    fn html_truecolor_and_256() {
        let out = render(
            "\x1b[38;2;255;128;0mA\x1b[0m\x1b[38;5;196mB\x1b[0m",
            "html",
            "dark",
            "inline",
        )
        .unwrap();
        assert!(out.contains("color:#ff8000")); // truecolor
        assert!(out.contains("color:#ff0000")); // xterm 196 == pure red
    }

    #[test]
    fn html_bright_and_background() {
        let out = render("\x1b[91;42mx\x1b[0m", "html", "dark", "inline").unwrap();
        assert!(out.contains("color:#ff0000")); // 91 -> bright red
        assert!(out.contains("background-color:#00cd00")); // 42 -> green bg
    }

    #[test]
    fn html_inverse_swaps() {
        let out = render("\x1b[31;7mx\x1b[0m", "html", "dark", "inline").unwrap();
        // fg red becomes background; nothing sets the other, so only bg emitted.
        assert!(out.contains("background-color:#cd0000"));
        assert!(!out.contains("color:#cd0000;")); // red is no longer the fg
    }

    #[test]
    fn html_classes_mode_has_stylesheet() {
        let out = render("\x1b[31mx\x1b[0m", "html", "dark", "classes").unwrap();
        assert!(out.contains("<style>"));
        assert!(out.contains(".ansi-fg-red{color:#cd0000}"));
        assert!(out.contains("<pre class=\"ansi ansi--dark\">"));
        assert!(out.contains("<span class=\"ansi-fg-red\">x</span>"));
    }

    #[test]
    fn html_classes_mode_256_falls_back_inline() {
        let out = render("\x1b[38;5;196mx\x1b[0m", "html", "dark", "classes").unwrap();
        assert!(out.contains("<span style=\"color:#ff0000\">x</span>"));
    }

    #[test]
    fn html_light_theme_default_colors() {
        let out = render("x", "html", "light", "inline").unwrap();
        assert!(out.contains("background-color:#ffffff;color:#000000"));
    }

    #[test]
    fn html_drops_cursor_and_osc() {
        let out = render(
            "\x1b[2J\x1b[H\x1b]0;title\x07\x1b[33mwarn\x1b[0m",
            "html",
            "dark",
            "inline",
        )
        .unwrap();
        assert!(!out.contains("title"));
        assert!(!out.contains("2J"));
        assert!(out.contains(">warn</span>"));
    }

    #[test]
    fn text_mode_strips_everything() {
        let out = render("\x1b[1;32m✓ build passed\x1b[0m", "text", "dark", "inline").unwrap();
        assert_eq!(out, "✓ build passed");
    }

    #[test]
    fn defaults_apply_on_empty_strings() {
        let a = render("\x1b[31mx\x1b[0m", "", "", "").unwrap();
        let b = render("\x1b[31mx\x1b[0m", "html", "dark", "inline").unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn invalid_output_errors() {
        assert!(render("x", "json", "dark", "inline").is_err());
    }

    #[test]
    fn invalid_theme_errors() {
        assert!(render("x", "html", "blue", "inline").is_err());
    }

    #[test]
    fn invalid_styles_errors() {
        assert!(render("x", "html", "dark", "fancy").is_err());
    }

    #[test]
    fn xterm256_cube_and_gray() {
        assert_eq!(xterm256(16), (0, 0, 0));
        assert_eq!(xterm256(196), (255, 0, 0));
        assert_eq!(xterm256(231), (255, 255, 255));
        assert_eq!(xterm256(232), (8, 8, 8));
        assert_eq!(xterm256(255), (238, 238, 238));
    }
}
