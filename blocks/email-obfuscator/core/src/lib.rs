//! email-obfuscator core — encode an email address into scraper-resistant
//! HTML you can paste into a page. Pure compute, no wafer/wasm-bindgen deps;
//! shared by the chat skill block and the web page.
//!
//! Address-harvesting bots scrape `mailto:` links and plain `foo@bar.com`
//! strings out of raw HTML. The transforms here keep the address readable +
//! clickable in a real browser while hiding it from naive regex scrapers:
//!
//! - `entities` (default): replace every character with an HTML numeric
//!   character reference. `decimal` → `&#106;…`, `hex` → `&#x6a;…`. The
//!   browser renders the original text; a regex over the source sees only
//!   entities. Optionally wrapped in a real `mailto:` anchor (also entity-
//!   encoded).
//! - `js`: emit a `<script>` that assembles the address from character codes
//!   at runtime via `document.write`, so the address never appears as a literal
//!   in the served HTML at all. Leaves an entity-encoded `<noscript>` fallback.
//! - `css`: render the address reversed in the source and flip it back with
//!   `unicode-bidi:bidi-override; direction:rtl`. A scraper reads
//!   "moc.rab@oof"; the user sees "foo@bar.com".
//! - `rot13`: a `mailto:` link whose href is ROT13-scrambled, decoded by a tiny
//!   inline script on click (the classic WordPress-style obfuscation). Letters
//!   are rotated 13, so the literal href reads "znvygb:sbb@one.pbz".

/// Which obfuscation strategy to apply.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Mode {
    /// HTML numeric character references.
    Entities,
    /// `<script>` that builds the address from char codes.
    Js,
    /// CSS bidi-override reversal.
    Css,
    /// ROT13 `mailto:` decoded by inline JS.
    Rot13,
}

impl Mode {
    /// Parse a mode name; blank defaults to `entities`.
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "entities" | "entity" => Ok(Mode::Entities),
            "js" | "javascript" => Ok(Mode::Js),
            "css" => Ok(Mode::Css),
            "rot13" => Ok(Mode::Rot13),
            other => Err(format!(
                "invalid mode {other:?}: expected \"entities\", \"js\", \"css\", or \"rot13\""
            )),
        }
    }
}

/// Numeric-entity radix for entity output.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EntityStyle {
    Decimal,
    Hex,
}

impl EntityStyle {
    fn parse(s: &str) -> Result<Self, String> {
        match s.trim() {
            "" | "decimal" | "dec" => Ok(EntityStyle::Decimal),
            "hex" | "hexadecimal" => Ok(EntityStyle::Hex),
            other => Err(format!(
                "invalid entity_style {other:?}: expected \"decimal\" or \"hex\""
            )),
        }
    }
}

/// Validate that `email` looks like a real address (`local@domain` with a dot in
/// the domain) so we don't silently obfuscate garbage. Returns the trimmed
/// address on success.
fn validate(email: &str) -> Result<String, String> {
    let e = email.trim();
    if e.is_empty() {
        return Err("email is empty".into());
    }
    if e.chars().any(|c| c.is_whitespace()) {
        return Err("email must not contain whitespace".into());
    }
    let mut parts = e.splitn(2, '@');
    let local = parts.next().unwrap_or("");
    let domain = match parts.next() {
        Some(d) => d,
        None => return Err("email must contain exactly one '@'".into()),
    };
    if domain.contains('@') {
        return Err("email must contain exactly one '@'".into());
    }
    if local.is_empty() {
        return Err("email is missing the local part before '@'".into());
    }
    if domain.is_empty() || !domain.contains('.') {
        return Err("email domain must contain a dot, e.g. example.com".into());
    }
    if domain.starts_with('.') || domain.ends_with('.') {
        return Err("email domain must not start or end with a dot".into());
    }
    Ok(e.to_string())
}

/// Encode a `&str` as a string of HTML numeric character references.
fn to_entities(s: &str, style: EntityStyle) -> String {
    let mut out = String::new();
    for c in s.chars() {
        let cp = c as u32;
        match style {
            EntityStyle::Decimal => out.push_str(&format!("&#{cp};")),
            EntityStyle::Hex => out.push_str(&format!("&#x{cp:x};")),
        }
    }
    out
}

/// JS array of decimal char codes, e.g. `[102,111,111]`.
fn char_code_array(s: &str) -> String {
    let codes: Vec<String> = s.chars().map(|c| (c as u32).to_string()).collect();
    format!("[{}]", codes.join(","))
}

/// ROT13 every ASCII letter; leave everything else (digits, `@`, `.`, `:`)
/// untouched.
fn rot13(s: &str) -> String {
    s.chars()
        .map(|c| match c {
            'a'..='z' => (((c as u8 - b'a' + 13) % 26) + b'a') as char,
            'A'..='Z' => (((c as u8 - b'A' + 13) % 26) + b'A') as char,
            other => other,
        })
        .collect()
}

/// Escape a string for inclusion in a double-quoted JS string literal.
fn js_escape(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            other => out.push(other),
        }
    }
    out
}

/// Minimal HTML text escaping for visible link text.
fn html_escape(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            other => out.push(other),
        }
    }
    out
}

/// Options controlling the obfuscated output.
pub struct Options<'a> {
    pub mode: &'a str,
    pub entity_style: &'a str,
    /// Wrap the address in a clickable `mailto:` anchor (where the mode allows).
    pub link: bool,
    /// Optional visible link text (e.g. "Email us"). Empty → show the address.
    pub link_text: &'a str,
}

impl Default for Options<'_> {
    fn default() -> Self {
        Options {
            mode: "entities",
            entity_style: "decimal",
            link: true,
            link_text: "",
        }
    }
}

/// Obfuscate `email` into a paste-ready HTML snippet.
///
/// Returns `Err` on an invalid email or an unknown `mode`/`entity_style`.
pub fn obfuscate(email: &str, opts: &Options) -> Result<String, String> {
    let email = validate(email)?;
    let mode = Mode::parse(opts.mode)?;
    let style = EntityStyle::parse(opts.entity_style)?;
    let link_text = opts.link_text.trim();

    let out = match mode {
        Mode::Entities => {
            let addr = to_entities(&email, style);
            if opts.link {
                let href = to_entities(&format!("mailto:{email}"), style);
                let text = if link_text.is_empty() {
                    addr
                } else {
                    to_entities(link_text, style)
                };
                format!("<a href=\"{href}\">{text}</a>")
            } else {
                addr
            }
        }
        Mode::Css => {
            // Reverse the char order in the source; the browser flips it back
            // with bidi-override. (chars(), not bytes — keeps multi-byte intact,
            // though emails are ASCII in practice.) Link mode is not meaningful
            // for a reversed anchor href, so css always emits display text.
            let reversed: String = email.chars().rev().collect();
            format!(
                "<span style=\"unicode-bidi:bidi-override;direction:rtl;\">{}</span>",
                to_entities(&reversed, style)
            )
        }
        Mode::Js => {
            let user_codes = char_code_array(email.split('@').next().unwrap_or(""));
            let domain_codes = char_code_array(email.split('@').nth(1).unwrap_or(""));
            let text_expr = if link_text.is_empty() {
                String::from("addr")
            } else {
                format!("\"{}\"", js_escape(link_text))
            };
            let noscript = to_entities(&email, style);
            let (anchor_open, anchor_close) = if opts.link {
                ("'<a href=\"mailto:'+addr+'\">'+", "+'</a>'")
            } else {
                ("''+", "")
            };
            format!(
                "<script>(function(){{var u={user},d={domain};\
var addr=String.fromCharCode.apply(null,u)+'@'+String.fromCharCode.apply(null,d);\
document.write({open}{text}{close});}})();</script>\
<noscript>{ns}</noscript>",
                user = user_codes,
                domain = domain_codes,
                open = anchor_open,
                text = text_expr,
                close = anchor_close,
                ns = noscript,
            )
        }
        Mode::Rot13 => {
            // The literal href is ROT13-scrambled; an onclick handler decodes it
            // back to a real mailto: just before navigation.
            let scrambled = rot13(&format!("mailto:{email}"));
            let text = if link_text.is_empty() {
                to_entities(&email, style)
            } else {
                html_escape(link_text)
            };
            let noscript = to_entities(&email, style);
            format!(
                "<a href=\"{scrambled}\" onclick=\"this.href=this.href.replace(/[a-zA-Z]/g,function(c){{return String.fromCharCode((c<='Z'?90:122)>=(c=c.charCodeAt(0)+13)?c:c-26);}});\">{text}</a>\
<noscript> ({noscript})</noscript>"
            )
        }
    };
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts<'a>(mode: &'a str, style: &'a str, link: bool, text: &'a str) -> Options<'a> {
        Options {
            mode,
            entity_style: style,
            link,
            link_text: text,
        }
    }

    #[test]
    fn entities_decimal_link() {
        let out = obfuscate("foo@bar.com", &opts("entities", "decimal", true, "")).unwrap();
        // No literal address anywhere in the source.
        assert!(!out.contains("foo@bar.com"));
        assert!(!out.contains("mailto:foo"));
        // Decimal entity for 'f' is &#102;.
        assert!(out.contains("&#102;"));
        assert!(out.starts_with("<a href="));
    }

    #[test]
    fn entities_hex_no_link() {
        let out = obfuscate("a@b.io", &opts("entities", "hex", false, "")).unwrap();
        assert!(!out.contains("<a"));
        // hex for 'a' (0x61).
        assert!(out.contains("&#x61;"));
        assert!(!out.contains("a@b.io"));
    }

    #[test]
    fn css_reverses() {
        let out = obfuscate("ab@c.de", &opts("css", "decimal", false, "")).unwrap();
        assert!(out.contains("bidi-override"));
        assert!(out.contains("direction:rtl"));
        // Reversed of "ab@c.de" is "ed.c@ba"; entity-encoded so 'e' (101) leads.
        assert!(out.contains("&#101;"));
        assert!(!out.contains("ab@c.de"));
    }

    #[test]
    fn js_builds_from_codes() {
        let out = obfuscate("joe@x.org", &opts("js", "decimal", true, "")).unwrap();
        assert!(out.contains("<script>"));
        assert!(out.contains("String.fromCharCode"));
        assert!(out.contains("<noscript>"));
        // 'j' = 106, 'o' = 111, 'e' = 101 in the user array.
        assert!(out.contains("106,111,101"));
        assert!(!out.contains("joe@x.org"));
    }

    #[test]
    fn rot13_scrambles_href() {
        let out = obfuscate("bob@one.com", &opts("rot13", "decimal", true, "Email")).unwrap();
        // rot13("mailto:bob@one.com") = "znvygb:obo@bar.pbz".
        assert!(out.contains("znvygb:obo@bar.pbz"));
        assert!(out.contains("onclick="));
        // Plaintext address must not appear in the visible href.
        assert!(!out.contains("href=\"mailto:bob@one.com"));
    }

    #[test]
    fn link_text_used() {
        let out = obfuscate("c@d.net", &opts("entities", "decimal", true, "Contact")).unwrap();
        // "Contact" entity-encoded: 'C' = 67.
        assert!(out.contains("&#67;"));
    }

    #[test]
    fn rejects_no_at() {
        assert!(obfuscate("not-an-email", &Options::default()).is_err());
    }

    #[test]
    fn rejects_no_dot_domain() {
        assert!(obfuscate("a@localhost", &Options::default()).is_err());
    }

    #[test]
    fn rejects_whitespace() {
        assert!(obfuscate("a b@c.com", &Options::default()).is_err());
    }

    #[test]
    fn rejects_empty() {
        assert!(obfuscate("   ", &Options::default()).is_err());
    }

    #[test]
    fn rejects_bad_mode() {
        assert!(obfuscate("a@b.com", &opts("wat", "decimal", true, "")).is_err());
    }

    #[test]
    fn rejects_bad_style() {
        assert!(obfuscate("a@b.com", &opts("entities", "octal", true, "")).is_err());
    }

    #[test]
    fn rot13_roundtrip() {
        assert_eq!(rot13(&rot13("mailto:foo@bar.com")), "mailto:foo@bar.com");
    }
}
