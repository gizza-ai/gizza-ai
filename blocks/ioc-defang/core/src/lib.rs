//! ioc-defang core — defang or refang indicators of compromise (IOCs). No
//! wafer/wasm-bindgen deps; shared by the chat skill block and the web page.
//!
//! **Defanging** rewrites URLs, IP addresses, domains and email addresses so a
//! mail client, chat app or terminal will NOT auto-link or auto-execute them —
//! the standard safety practice when sharing malicious indicators in a report,
//! ticket or email. It does this by neutralizing three things:
//!
//! * the scheme: `http`/`https`/`ftp` → `hxxp`/`hxxps`/`fxp`,
//! * every dot `.` between labels → `[.]` (configurable bracket style), and
//! * the `@` in an email → `[at]`.
//!
//! **Refanging** is the exact inverse — it restores a defanged blob back to the
//! real, clickable indicator so an analyst can copy it into a sandbox/tool.
//!
//! The transform is text-wide: it leaves surrounding prose untouched and only
//! rewrites the indicator-ish substrings, so you can paste a whole paragraph.

/// What to do with the text.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Defang,
    Refang,
}

/// Which bracket style to wrap neutralized separators in when defanging.
/// Refanging accepts ALL of these regardless of the chosen style.
#[derive(Clone, Copy)]
enum Style {
    /// `[.]`, `[at]`, `[://]` — the most common CTI convention.
    Square,
    /// `(.)`, `(at)`, `(://)`.
    Round,
    /// `{.}`, `{at}`, `{://}`.
    Curly,
    /// `.` → `[dot]`, `@` → `[at]` — spelled-out words instead of the symbol.
    Dot,
}

/// Defang or refang the IOCs in `text`.
///
/// * `mode` — `"defang"` (default) neutralizes; `"refang"` restores.
/// * `style` — bracket style for defanging: `"square"` (default `[.]`),
///   `"round"` `(.)`, `"curly"` `{.}`, or `"dot"` (`[dot]`/`[at]`). Ignored when
///   refanging (all styles are recognized on the way back).
///
/// Returns `Err` only for an unknown `mode` or `style`.
pub fn defang(text: &str, mode: &str, style: &str) -> Result<String, String> {
    let mode = match mode.trim().to_ascii_lowercase().as_str() {
        "" | "defang" | "fang" | "neutralize" => Mode::Defang,
        "refang" | "unfang" | "restore" => Mode::Refang,
        other => {
            return Err(format!(
                "invalid mode {other:?}: expected 'defang' or 'refang'"
            ))
        }
    };
    let style = match style.trim().to_ascii_lowercase().as_str() {
        "" | "square" | "bracket" | "brackets" => Style::Square,
        "round" | "paren" | "parens" | "parentheses" => Style::Round,
        "curly" | "brace" | "braces" => Style::Curly,
        "dot" | "word" | "words" => Style::Dot,
        other => {
            return Err(format!(
                "invalid style {other:?}: expected 'square', 'round', 'curly' or 'dot'"
            ))
        }
    };
    Ok(match mode {
        Mode::Defang => defang_text(text, style),
        Mode::Refang => refang_text(text),
    })
}

/// Neutralize indicators in `text` using the chosen bracket `style`.
fn defang_text(text: &str, style: Style) -> String {
    let (l, r) = brackets(style);
    let dot_token = format!("{l}{}{r}", dot_word(style));
    let at_token = format!("{l}at{r}");
    // scheme rewrites first (http -> hxxp etc.).
    let mut out = rewrite_schemes(text);
    // `://` -> `[://]` so the separator after the scheme is also broken; do this
    // before dot replacement so we don't touch the dots inside it.
    out = out.replace("://", &format!("{l}://{r}"));
    // `@` -> `[at]` (emails).
    out = out.replace('@', &at_token);
    // every remaining `.` -> `[.]` / `[dot]`.
    out = out.replace('.', &dot_token);
    out
}

/// Restore a defanged blob: strip every supported bracket style and rebuild the
/// real scheme/separators.
fn refang_text(text: &str) -> String {
    let mut out = text.to_string();
    // Order matters: handle the multi-char tokens (`[://]`, `[at]`, `[dot]`)
    // before the bare-dot tokens so we don't half-rewrite them.
    for (l, r) in [("[", "]"), ("(", ")"), ("{", "}")] {
        out = out.replace(&format!("{l}://{r}"), "://");
        out = out.replace(&format!("{l}at{r}"), "@");
        out = out.replace(&format!("{l}dot{r}"), ".");
        out = out.replace(&format!("{l}.{r}"), ".");
    }
    // restore neutralized schemes (after separators so hxxp[://] is whole).
    out = restore_schemes(&out);
    out
}

/// Map a `Style` to its opening/closing bracket pair.
fn brackets(style: Style) -> (&'static str, &'static str) {
    match style {
        Style::Square | Style::Dot => ("[", "]"),
        Style::Round => ("(", ")"),
        Style::Curly => ("{", "}"),
    }
}

/// The token used between the brackets for a dot.
fn dot_word(style: Style) -> &'static str {
    match style {
        Style::Dot => "dot",
        _ => ".",
    }
}

/// Defang URL schemes (`http`→`hxxp`, etc.). Matches only when the scheme is
/// immediately followed by `://`, so the word "http" in prose is left alone.
fn rewrite_schemes(text: &str) -> String {
    let mut out = text.to_string();
    for (from, to) in [
        ("https://", "hxxps://"),
        ("http://", "hxxp://"),
        ("ftp://", "fxp://"),
        ("HTTPS://", "hxxps://"),
        ("HTTP://", "hxxp://"),
        ("FTP://", "fxp://"),
        ("Https://", "hxxps://"),
        ("Http://", "hxxp://"),
        ("Ftp://", "fxp://"),
    ] {
        out = out.replace(from, to);
    }
    out
}

/// Inverse of `rewrite_schemes`; also normalizes the `meow://` convention.
fn restore_schemes(text: &str) -> String {
    let mut out = text.to_string();
    for (from, to) in [
        ("hxxps://", "https://"),
        ("hxxp://", "http://"),
        ("fxp://", "ftp://"),
        ("meow://", "http://"),
    ] {
        out = out.replace(from, to);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defang_url_square() {
        assert_eq!(
            defang("http://evil.example.com/path", "defang", "square").unwrap(),
            "hxxp[://]evil[.]example[.]com/path"
        );
    }

    #[test]
    fn defang_https_and_ip() {
        assert_eq!(
            defang("https://10.0.0.1", "defang", "square").unwrap(),
            "hxxps[://]10[.]0[.]0[.]1"
        );
    }

    #[test]
    fn defang_email() {
        assert_eq!(
            defang("bad.actor@evil.com", "defang", "square").unwrap(),
            "bad[.]actor[at]evil[.]com"
        );
    }

    #[test]
    fn defang_styles() {
        assert_eq!(defang("a.b", "defang", "round").unwrap(), "a(.)b");
        assert_eq!(defang("a.b", "defang", "curly").unwrap(), "a{.}b");
        assert_eq!(
            defang("a.b@c.d", "defang", "dot").unwrap(),
            "a[dot]b[at]c[dot]d"
        );
    }

    #[test]
    fn defang_bare_domain() {
        assert_eq!(
            defang("evil.example.com", "defang", "square").unwrap(),
            "evil[.]example[.]com"
        );
    }

    #[test]
    fn refang_is_inverse_of_defang_square() {
        let original = "http://evil.example.com/a.b and bad@evil.com";
        let fanged = defang(original, "defang", "square").unwrap();
        assert_eq!(defang(&fanged, "refang", "").unwrap(), original);
    }

    #[test]
    fn refang_accepts_all_styles() {
        assert_eq!(
            defang("hxxps(://)10(.)0(.)0(.)1", "refang", "").unwrap(),
            "https://10.0.0.1"
        );
        assert_eq!(
            defang("a[dot]b[at]c[dot]d", "refang", "").unwrap(),
            "a.b@c.d"
        );
        assert_eq!(
            defang("evil{.}example{.}com", "refang", "").unwrap(),
            "evil.example.com"
        );
    }

    #[test]
    fn refang_meow_scheme() {
        assert_eq!(
            defang("meow://1.2.3.4", "refang", "").unwrap(),
            "http://1.2.3.4"
        );
    }

    #[test]
    fn prose_is_preserved_around_iocs() {
        let input = "See http://bad.com for the payload.";
        assert_eq!(
            defang(input, "defang", "square").unwrap(),
            "See hxxp[://]bad[.]com for the payload[.]"
        );
    }

    #[test]
    fn uppercase_scheme_defangs() {
        assert_eq!(
            defang("HTTP://EVIL.COM", "defang", "square").unwrap(),
            "hxxp[://]EVIL[.]COM"
        );
    }

    #[test]
    fn empty_input_ok() {
        assert_eq!(defang("", "defang", "square").unwrap(), "");
        assert_eq!(defang("", "refang", "").unwrap(), "");
    }

    #[test]
    fn rejects_unknown_mode() {
        let e = defang("x", "scramble", "square").unwrap_err();
        assert!(e.contains("invalid mode"), "got {e}");
    }

    #[test]
    fn rejects_unknown_style() {
        let e = defang("x", "defang", "triangle").unwrap_err();
        assert!(e.contains("invalid style"), "got {e}");
    }

    #[test]
    fn mode_and_style_case_insensitive_and_aliases() {
        assert_eq!(defang("a.b", "DEFANG", "SQUARE").unwrap(), "a[.]b");
        assert_eq!(defang("a[.]b", "Refang", "").unwrap(), "a.b");
        assert_eq!(defang("a.b", "neutralize", "brackets").unwrap(), "a[.]b");
    }
}
