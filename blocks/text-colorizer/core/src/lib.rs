//! gizza-ai/text-colorizer core — apply user-defined `color: regex` rules to log
//! or command output and export the result as ANSI terminal escapes or a
//! self-contained HTML `<pre>`. Pure-Rust (`regex`), no wafer/wasm-bindgen deps —
//! shared by the chat skill block, the CLI, and the web page.
//!
//! Rules are one per line, `color-spec: regex`. The color spec is the part before
//! the FIRST colon; the rest of the line (trimmed) is the Rust-regex pattern
//! (colons in the pattern are fine — only the first colon splits). Blank lines are
//! skipped. Example:
//!
//! ```text
//! bold red: \bERROR\b
//! yellow: \bWARN(ING)?\b
//! green on black: \bOK\b
//! #3465a4 underline: https?://\S+
//! ```
//!
//! A color spec is whitespace-separated tokens: optional attributes
//! (`bold` `dim` `italic` `underline` `blink` `reverse` `strike`), an optional
//! foreground color (a name or `#rgb`/`#rrggbb`), and an optional `on <color>`
//! background. Named colors: `black red green yellow blue magenta cyan white`,
//! their `bright*` variants, and `gray`/`grey` (= bright black).
//!
//! Matching is done line by line, so `^`/`$` anchor per line. When two rules match
//! overlapping text, the EARLIER rule (higher up in the list) wins for the
//! overlapping characters. With `whole_line`, the first rule that matches anywhere
//! on a line colors the entire line.

use regex::RegexBuilder;

/// Cap on input text size (untrusted from CLI/chat/page).
const MAX_TEXT_BYTES: usize = 512 * 1024; // 512 KiB
/// Cap on the number of rules.
const MAX_RULES: usize = 200;

/// Sentinel owner index meaning "no rule colors this character".
const UNOWNED: usize = usize::MAX;

/// An RGB triple for a resolved color.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Rgb(u8, u8, u8);

/// A color is either one of the 16 basic ANSI colors (with its SGR base) or an
/// arbitrary RGB value (rendered as 24-bit truecolor in ANSI, hex in HTML).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Color {
    /// `code` is the foreground SGR code (30-37 basic, 90-97 bright); `hex` is the
    /// matching palette color used for HTML.
    Named { code: u8, hex: Rgb },
    Rgb(Rgb),
}

/// Text attributes (SGR codes for ANSI, CSS for HTML).
#[derive(Clone, Copy, Default, PartialEq, Eq)]
struct Attrs {
    bold: bool,
    dim: bool,
    italic: bool,
    underline: bool,
    blink: bool,
    reverse: bool,
    strike: bool,
}

/// A fully parsed style: optional fg/bg + attributes.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Style {
    fg: Option<Color>,
    bg: Option<Color>,
    attrs: Attrs,
}

/// Map a named color to its (basic SGR fg code, HTML hex). Uses the Tango palette
/// for HTML so named colors render close to a typical terminal.
fn named_color(name: &str) -> Option<Color> {
    let (code, r, g, b) = match name {
        "black" => (30, 0x00, 0x00, 0x00),
        "red" => (31, 0xcc, 0x00, 0x00),
        "green" => (32, 0x4e, 0x9a, 0x06),
        "yellow" => (33, 0xc4, 0xa0, 0x00),
        "blue" => (34, 0x34, 0x65, 0xa4),
        "magenta" | "purple" => (35, 0x75, 0x50, 0x7b),
        "cyan" => (36, 0x06, 0x98, 0x9a),
        "white" => (37, 0xd3, 0xd7, 0xcf),
        "gray" | "grey" | "brightblack" => (90, 0x55, 0x57, 0x53),
        "brightred" => (91, 0xef, 0x29, 0x29),
        "brightgreen" => (92, 0x8a, 0xe2, 0x34),
        "brightyellow" => (93, 0xfc, 0xe9, 0x4f),
        "brightblue" => (94, 0x72, 0x9f, 0xcf),
        "brightmagenta" | "brightpurple" => (95, 0xad, 0x7f, 0xa8),
        "brightcyan" => (96, 0x34, 0xe2, 0xe2),
        "brightwhite" => (97, 0xee, 0xee, 0xec),
        _ => return None,
    };
    Some(Color::Named { code, hex: Rgb(r, g, b) })
}

/// Parse a `#rgb` or `#rrggbb` hex color.
fn parse_hex(tok: &str) -> Option<Rgb> {
    let h = tok.strip_prefix('#')?;
    let bytes = h.as_bytes();
    let hx = |c: u8| -> Option<u8> {
        match c {
            b'0'..=b'9' => Some(c - b'0'),
            b'a'..=b'f' => Some(c - b'a' + 10),
            b'A'..=b'F' => Some(c - b'A' + 10),
            _ => None,
        }
    };
    match bytes.len() {
        3 => {
            let r = hx(bytes[0])?;
            let g = hx(bytes[1])?;
            let b = hx(bytes[2])?;
            Some(Rgb(r * 17, g * 17, b * 17))
        }
        6 => {
            let r = hx(bytes[0])? * 16 + hx(bytes[1])?;
            let g = hx(bytes[2])? * 16 + hx(bytes[3])?;
            let b = hx(bytes[4])? * 16 + hx(bytes[5])?;
            Some(Rgb(r, g, b))
        }
        _ => None,
    }
}

/// Resolve one color token (name or hex) to a `Color`, or `None` if unrecognized.
fn parse_color_token(tok: &str) -> Option<Color> {
    if tok.starts_with('#') {
        parse_hex(tok).map(Color::Rgb)
    } else {
        named_color(&tok.to_ascii_lowercase())
    }
}

/// Parse a whitespace-separated color spec (the part before the first colon).
fn parse_style(spec: &str, rule_no: usize) -> Result<Style, String> {
    let mut style = Style { fg: None, bg: None, attrs: Attrs::default() };
    let mut toks = spec.split_whitespace();
    while let Some(tok) = toks.next() {
        let lower = tok.to_ascii_lowercase();
        match lower.as_str() {
            "bold" => style.attrs.bold = true,
            "dim" | "faint" => style.attrs.dim = true,
            "italic" => style.attrs.italic = true,
            "underline" => style.attrs.underline = true,
            "blink" => style.attrs.blink = true,
            "reverse" | "inverse" => style.attrs.reverse = true,
            "strike" | "strikethrough" => style.attrs.strike = true,
            "on" => {
                let bg = toks.next().ok_or_else(|| {
                    format!("rule {rule_no}: 'on' must be followed by a background color")
                })?;
                style.bg = Some(parse_color_token(bg).ok_or_else(|| {
                    format!("rule {rule_no}: unknown background color {bg:?}")
                })?);
            }
            _ => {
                let c = parse_color_token(tok).ok_or_else(|| {
                    format!("rule {rule_no}: unknown color or attribute {tok:?}")
                })?;
                if style.fg.is_some() {
                    return Err(format!(
                        "rule {rule_no}: more than one foreground color in {spec:?}"
                    ));
                }
                style.fg = Some(c);
            }
        }
    }
    if style.fg.is_none() && style.bg.is_none() && style.attrs == Attrs::default() {
        return Err(format!("rule {rule_no}: color spec {spec:?} sets no color or style"));
    }
    Ok(style)
}

/// A compiled rule: its regex + the style to apply to matches.
struct Rule {
    re: regex::Regex,
    style: Style,
}

/// Parse the `rules` source into compiled rules (order preserved).
fn parse_rules(src: &str, ignore_case: bool) -> Result<Vec<Rule>, String> {
    let mut rules = Vec::new();
    let mut rule_no = 0usize;
    for raw in src.lines() {
        let line = raw.trim();
        if line.is_empty() {
            continue;
        }
        rule_no += 1;
        if rule_no > MAX_RULES {
            return Err(format!("too many rules (max {MAX_RULES})"));
        }
        let colon = line.find(':').ok_or_else(|| {
            format!("rule {rule_no}: expected 'color: regex' (missing ':') in {line:?}")
        })?;
        let spec = line[..colon].trim();
        let pattern = line[colon + 1..].trim();
        if pattern.is_empty() {
            return Err(format!("rule {rule_no}: empty regex pattern"));
        }
        let style = parse_style(spec, rule_no)?;
        let re = RegexBuilder::new(pattern)
            .case_insensitive(ignore_case)
            .build()
            .map_err(|e| format!("rule {rule_no}: invalid regex {pattern:?}: {e}"))?;
        rules.push(Rule { re, style });
    }
    if rules.is_empty() {
        return Err("no rules provided (add at least one 'color: regex' line)".into());
    }
    Ok(rules)
}

/// The public entry point.
///
/// - `text`: the text to colorize.
/// - `rules_src`: one `color: regex` rule per line (see module docs).
/// - `output`: `"ansi"` (default) emits terminal escape codes; `"html"` emits a
///   self-contained styled `<pre>`.
/// - `theme`: `"dark"` (default) or `"light"` — HTML `<pre>` background/foreground
///   (ignored for ANSI, where the terminal owns the background).
/// - `ignore_case`: match all rules case-insensitively.
/// - `whole_line`: color the whole line matched by the first matching rule instead
///   of just the matched substrings.
///
/// Returns `Err` on an invalid regex, unknown color, malformed rule, unknown
/// `output`/`theme`, or oversized input.
pub fn colorize(
    text: &str,
    rules_src: &str,
    output: &str,
    theme: &str,
    ignore_case: bool,
    whole_line: bool,
) -> Result<String, String> {
    if text.len() > MAX_TEXT_BYTES {
        return Err(format!(
            "input too large ({} bytes; max {})",
            text.len(),
            MAX_TEXT_BYTES
        ));
    }
    let mode = match output {
        "" | "ansi" => OutMode::Ansi,
        "html" => OutMode::Html,
        other => return Err(format!("invalid output {other:?}: expected \"ansi\" or \"html\"")),
    };
    let theme = match theme {
        "" | "dark" => ThemeKind::Dark,
        "light" => ThemeKind::Light,
        other => return Err(format!("invalid theme {other:?}: expected \"dark\" or \"light\"")),
    };
    let rules = parse_rules(rules_src, ignore_case)?;

    let mut body = String::new();
    let lines: Vec<&str> = text.split('\n').collect();
    let last = lines.len().saturating_sub(1);
    for (i, line) in lines.iter().enumerate() {
        render_line(&mut body, line, &rules, mode, whole_line);
        if i != last {
            body.push('\n');
        }
    }

    Ok(match mode {
        OutMode::Ansi => body,
        OutMode::Html => wrap_html(&body, theme),
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum OutMode {
    Ansi,
    Html,
}

#[derive(Clone, Copy)]
enum ThemeKind {
    Dark,
    Light,
}

/// Render a single line into `out`.
fn render_line(out: &mut String, line: &str, rules: &[Rule], mode: OutMode, whole_line: bool) {
    if whole_line {
        // First rule matching anywhere on the line colors the whole line.
        for rule in rules {
            if rule.re.is_match(line) {
                emit_styled(out, line, &rule.style, mode);
                return;
            }
        }
        emit_plain(out, line, mode);
        return;
    }

    let n = line.len();
    if n == 0 {
        return;
    }
    // Per-character ownership: earlier rules win on overlap (UNOWNED == plain).
    let mut owner = vec![UNOWNED; n];
    for (idx, rule) in rules.iter().enumerate() {
        for m in rule.re.find_iter(line) {
            for slot in owner[m.start()..m.end()].iter_mut() {
                if *slot == UNOWNED {
                    *slot = idx;
                }
            }
        }
    }

    // Group consecutive bytes with the same owner into segments. Segment
    // boundaries fall on match edges, which are UTF-8 char boundaries, so slicing
    // is always valid.
    let mut i = 0usize;
    while i < n {
        let cur = owner[i];
        let mut j = i + 1;
        while j < n && owner[j] == cur {
            j += 1;
        }
        let seg = &line[i..j];
        if cur == UNOWNED {
            emit_plain(out, seg, mode);
        } else {
            emit_styled(out, seg, &rules[cur].style, mode);
        }
        i = j;
    }
}

/// Append `text` styled with `style`.
fn emit_styled(out: &mut String, text: &str, style: &Style, mode: OutMode) {
    match mode {
        OutMode::Ansi => {
            out.push_str(&ansi_open(style));
            out.push_str(text);
            out.push_str("\x1b[0m");
        }
        OutMode::Html => {
            out.push_str("<span style=\"");
            out.push_str(&css_style(style));
            out.push_str("\">");
            html_escape_into(out, text);
            out.push_str("</span>");
        }
    }
}

/// Append plain (unstyled) `text`.
fn emit_plain(out: &mut String, text: &str, mode: OutMode) {
    match mode {
        OutMode::Ansi => out.push_str(text),
        OutMode::Html => html_escape_into(out, text),
    }
}

/// Build the opening ANSI SGR sequence for a style.
fn ansi_open(style: &Style) -> String {
    let mut codes: Vec<String> = Vec::new();
    let a = &style.attrs;
    if a.bold {
        codes.push("1".into());
    }
    if a.dim {
        codes.push("2".into());
    }
    if a.italic {
        codes.push("3".into());
    }
    if a.underline {
        codes.push("4".into());
    }
    if a.blink {
        codes.push("5".into());
    }
    if a.reverse {
        codes.push("7".into());
    }
    if a.strike {
        codes.push("9".into());
    }
    if let Some(fg) = style.fg {
        match fg {
            Color::Named { code, .. } => codes.push(code.to_string()),
            Color::Rgb(Rgb(r, g, b)) => codes.push(format!("38;2;{r};{g};{b}")),
        }
    }
    if let Some(bg) = style.bg {
        match bg {
            // Background SGR code = fg code + 10.
            Color::Named { code, .. } => codes.push((code + 10).to_string()),
            Color::Rgb(Rgb(r, g, b)) => codes.push(format!("48;2;{r};{g};{b}")),
        }
    }
    format!("\x1b[{}m", codes.join(";"))
}

/// Build the CSS for a style (used inside an HTML `<span style="...">`).
fn css_style(style: &Style) -> String {
    let mut parts: Vec<String> = Vec::new();
    if let Some(fg) = style.fg {
        parts.push(format!("color:{}", css_color(fg)));
    }
    if let Some(bg) = style.bg {
        parts.push(format!("background:{}", css_color(bg)));
    }
    if style.attrs.bold {
        parts.push("font-weight:bold".into());
    }
    if style.attrs.dim {
        parts.push("opacity:0.6".into());
    }
    if style.attrs.italic {
        parts.push("font-style:italic".into());
    }
    let mut deco: Vec<&str> = Vec::new();
    if style.attrs.underline {
        deco.push("underline");
    }
    if style.attrs.strike {
        deco.push("line-through");
    }
    if !deco.is_empty() {
        parts.push(format!("text-decoration:{}", deco.join(" ")));
    }
    if style.attrs.reverse {
        // Best-effort: reverse swaps fg/bg — approximate by inverting the span.
        parts.push("filter:invert(100%)".into());
    }
    parts.join(";")
}

/// A color rendered as a CSS `#rrggbb` value.
fn css_color(c: Color) -> String {
    let Rgb(r, g, b) = match c {
        Color::Named { hex, .. } => hex,
        Color::Rgb(rgb) => rgb,
    };
    format!("#{r:02x}{g:02x}{b:02x}")
}

/// Escape `&`, `<`, `>` into `out` for safe HTML text content.
fn html_escape_into(out: &mut String, s: &str) {
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(c),
        }
    }
}

/// Wrap the rendered body in a self-contained themed `<pre>`.
fn wrap_html(body: &str, theme: ThemeKind) -> String {
    let (bg, fg) = match theme {
        ThemeKind::Dark => ("#1e1e1e", "#d4d4d4"),
        ThemeKind::Light => ("#ffffff", "#1e1e1e"),
    };
    format!(
        "<pre style=\"background:{bg};color:{fg};padding:1rem;border-radius:6px;\
         font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;\
         line-height:1.4;overflow:auto\">{body}</pre>"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ansi_basic_named() {
        let out = colorize("ERROR here", "red: ERROR", "ansi", "dark", false, false).unwrap();
        assert_eq!(out, "\x1b[31mERROR\x1b[0m here");
    }

    #[test]
    fn ansi_attrs_and_bg_and_hex() {
        let out =
            colorize("OK", "bold #00ff00 on black: OK", "ansi", "dark", false, false).unwrap();
        // bold; fg truecolor 0,255,0; bg black (40).
        assert_eq!(out, "\x1b[1;38;2;0;255;0;40mOK\x1b[0m");
    }

    #[test]
    fn html_escapes_and_wraps() {
        let out = colorize("a<b>", "green: a", "html", "dark", false, false).unwrap();
        assert!(out.starts_with("<pre style=\"background:#1e1e1e"));
        assert!(out.contains("<span style=\"color:#4e9a06\">a</span>"));
        assert!(out.contains("&lt;b&gt;"), "unmatched text is HTML-escaped: {out}");
    }

    #[test]
    fn earlier_rule_wins_on_overlap() {
        // Both rules touch the middle; the first (on "AB") wins the overlap.
        let out = colorize("ABC", "red: A.\nblue: .C", "ansi", "dark", false, false).unwrap();
        assert_eq!(out, "\x1b[31mAB\x1b[0m\x1b[34mC\x1b[0m");
    }

    #[test]
    fn whole_line_colors_entire_line() {
        let out = colorize(
            "info line\nERROR boom",
            "red: ERROR",
            "ansi",
            "dark",
            false,
            true,
        )
        .unwrap();
        assert_eq!(out, "info line\n\x1b[31mERROR boom\x1b[0m");
    }

    #[test]
    fn ignore_case_matches() {
        let out = colorize("error", "red: ERROR", "ansi", "dark", true, false).unwrap();
        assert_eq!(out, "\x1b[31merror\x1b[0m");
    }

    #[test]
    fn multiline_anchors_per_line() {
        let out = colorize("a\nb", "red: ^a$", "ansi", "dark", false, false).unwrap();
        assert_eq!(out, "\x1b[31ma\x1b[0m\nb");
    }

    #[test]
    fn err_no_rules() {
        assert!(colorize("x", "   \n\n", "ansi", "dark", false, false).is_err());
    }

    #[test]
    fn err_missing_colon() {
        let e = colorize("x", "red ERROR", "ansi", "dark", false, false).unwrap_err();
        assert!(e.contains("missing ':'"), "{e}");
    }

    #[test]
    fn err_unknown_color() {
        let e = colorize("x", "chartreuse: x", "ansi", "dark", false, false).unwrap_err();
        assert!(e.contains("unknown color"), "{e}");
    }

    #[test]
    fn err_invalid_regex() {
        let e = colorize("x", "red: (unclosed", "ansi", "dark", false, false).unwrap_err();
        assert!(e.contains("invalid regex"), "{e}");
    }

    #[test]
    fn err_bad_output() {
        assert!(colorize("x", "red: x", "xml", "dark", false, false).is_err());
    }

    #[test]
    fn regex_with_colon_in_pattern() {
        // Only the first colon splits, so "https?://" survives in the pattern.
        let out = colorize(
            "see http://x",
            "blue: https?://\\S+",
            "ansi",
            "dark",
            false,
            false,
        )
        .unwrap();
        assert_eq!(out, "see \x1b[34mhttp://x\x1b[0m");
    }
}
