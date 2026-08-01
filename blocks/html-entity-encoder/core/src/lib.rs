//! html-entity-encoder core — pure compute, shared by the chat skill block and
//! the web page. No wafer/wasm-bindgen deps.
//!
//! Encodes literal characters INTO HTML character references. Two orthogonal
//! axes control the result:
//!
//!   * `scope` — WHICH characters get encoded:
//!       - `minimal`   : only the five HTML/XML-sensitive characters `& < > " '`,
//!       - `non-ascii` : the five above plus every character above U+007F,
//!       - `named`     : the five above plus every character that has a named
//!                       entity in the HTML5 set.
//!   * `format` — HOW each encoded character is represented:
//!       - `named`   : the HTML5 named entity where one exists (`&amp;`,
//!                     `&copy;`, `&mdash;`), falling back to a decimal numeric
//!                     reference when the character has no name,
//!       - `decimal` : always a decimal numeric reference (`&#38;`, `&#169;`),
//!       - `hex`     : always a hexadecimal numeric reference (`&#x26;`,
//!                     `&#xA9;`).
//!
//! The reverse operation lives in the `html-entity-decoder` tool.

use std::collections::HashMap;
use std::sync::OnceLock;

/// WHICH characters are selected for encoding.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    /// Only `& < > " '` — safe for HTML/XML text and attribute values.
    Minimal,
    /// The five above plus every non-ASCII character (code point > 0x7F).
    NonAscii,
    /// The five above plus every character that has a named HTML5 entity.
    Named,
}

impl Scope {
    pub fn parse(s: &str) -> Result<Scope, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "minimal" | "min" => Ok(Scope::Minimal),
            "non-ascii" | "nonascii" | "non_ascii" | "all-non-ascii" => Ok(Scope::NonAscii),
            "named" | "all-named" | "named-set" => Ok(Scope::Named),
            other => Err(format!(
                "unknown value '{other}' for 'scope' (use 'minimal', 'non-ascii', or 'named')"
            )),
        }
    }
}

/// HOW a selected character is written out.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// Named entity where available, else a decimal numeric reference.
    Named,
    /// Always a decimal numeric reference `&#NNN;`.
    Decimal,
    /// Always a hexadecimal numeric reference `&#xHH;`.
    Hex,
}

impl Format {
    pub fn parse(s: &str) -> Result<Format, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "named" | "name" | "entity" => Ok(Format::Named),
            "decimal" | "dec" | "numeric" => Ok(Format::Decimal),
            "hex" | "hexadecimal" => Ok(Format::Hex),
            other => Err(format!(
                "unknown value '{other}' for 'format' (use 'named', 'decimal', or 'hex')"
            )),
        }
    }
}

/// The five HTML/XML-mandatory characters, always encoded in every scope.
fn is_mandatory(c: char) -> bool {
    matches!(c, '&' | '<' | '>' | '"' | '\'')
}

/// Map from a single character to its preferred HTML5 named entity (WITH the
/// leading `&` and trailing `;`, e.g. `"&amp;"`). Built once from the
/// `entities` crate's WHATWG table.
///
/// A character can have several aliases (`&amp;` and `&AMP;`); we keep the
/// **shortest**, tie-breaking toward the form with more lowercase letters (so
/// `&amp;` wins over `&AMP;`) and then lexicographically for determinism. Only
/// well-formed names (trailing `;`) that stand for exactly one character are
/// considered.
fn named_map() -> &'static HashMap<char, &'static str> {
    static MAP: OnceLock<HashMap<char, &'static str>> = OnceLock::new();
    MAP.get_or_init(|| {
        let mut m: HashMap<char, &'static str> = HashMap::new();
        for e in entities::ENTITIES.iter() {
            if !e.entity.ends_with(';') {
                continue;
            }
            let mut chars = e.characters.chars();
            let (Some(c), None) = (chars.next(), chars.next()) else {
                continue; // multi-char expansion (e.g. combining sequences) — skip
            };
            let better = match m.get(&c) {
                None => true,
                Some(&cur) => is_preferred(e.entity, cur),
            };
            if better {
                m.insert(c, e.entity);
            }
        }
        m
    })
}

/// Sort key for canonical-name selection: (len asc, lowercase count desc via
/// negation, bytes asc). Smaller is more preferred.
fn name_key(s: &str) -> (usize, usize, &str) {
    let lower = s.chars().filter(|c| c.is_ascii_lowercase()).count();
    (s.len(), usize::MAX - lower, s)
}

/// Is candidate `a` a better canonical name than the current best `b`?
/// Shorter wins; then more lowercase letters; then lexicographically smaller.
fn is_preferred(a: &str, b: &str) -> bool {
    name_key(a) < name_key(b)
}

/// Should this character be encoded, given the scope?
fn in_scope(c: char, scope: Scope, map: &HashMap<char, &'static str>) -> bool {
    if is_mandatory(c) {
        return true;
    }
    match scope {
        Scope::Minimal => false,
        Scope::NonAscii => c as u32 > 0x7F,
        Scope::Named => map.contains_key(&c),
    }
}

/// Write a single selected character in the requested format into `out`.
fn write_entity(c: char, format: Format, map: &HashMap<char, &'static str>, out: &mut String) {
    let code = c as u32;
    match format {
        Format::Named => match map.get(&c) {
            Some(&name) => out.push_str(name),
            None => {
                out.push_str("&#");
                out.push_str(&code.to_string());
                out.push(';');
            }
        },
        Format::Decimal => {
            out.push_str("&#");
            out.push_str(&code.to_string());
            out.push(';');
        }
        Format::Hex => {
            out.push_str(&format!("&#x{code:X};"));
        }
    }
}

/// Encode `text` into HTML character references.
///
/// `scope` selects which characters to encode (`"minimal"`, `"non-ascii"`,
/// `"named"`); `format` selects how each is written (`"named"`, `"decimal"`,
/// `"hex"`). Returns the encoded string, or an error string for an unknown
/// `scope`/`format` value.
pub fn encode(text: &str, scope: &str, format: &str) -> Result<String, String> {
    let scope = Scope::parse(scope)?;
    let format = Format::parse(format)?;
    let map = named_map();
    let mut out = String::with_capacity(text.len() + text.len() / 4);

    for c in text.chars() {
        if in_scope(c, scope, map) {
            write_entity(c, format, map, &mut out);
        } else {
            out.push(c);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimal_named_encodes_the_five() {
        assert_eq!(
            encode(r#"<a href="x">Tom & Jerry's</a>"#, "minimal", "named").unwrap(),
            "&lt;a href=&quot;x&quot;&gt;Tom &amp; Jerry&apos;s&lt;/a&gt;"
        );
    }

    #[test]
    fn minimal_leaves_non_ascii_alone() {
        // Accents and symbols are untouched in minimal scope.
        assert_eq!(
            encode("Café © €5 — ok & <b>", "minimal", "named").unwrap(),
            "Café © €5 — ok &amp; &lt;b&gt;"
        );
    }

    #[test]
    fn non_ascii_scope_named_with_numeric_fallback() {
        // © and — have names; an emoji has none → decimal fallback. ASCII text
        // other than the five stays literal.
        assert_eq!(
            encode("A © — 😀 &", "non-ascii", "named").unwrap(),
            "A &copy; &mdash; &#128512; &amp;"
        );
    }

    #[test]
    fn named_scope_encodes_every_named_character() {
        // é has a name; plain letters/spaces do not, so they stay literal.
        assert_eq!(encode("é and z", "named", "named").unwrap(), "&eacute; and z");
    }

    #[test]
    fn decimal_format_always_numeric() {
        assert_eq!(
            encode("© & <", "non-ascii", "decimal").unwrap(),
            "&#169; &#38; &#60;"
        );
    }

    #[test]
    fn hex_format_uppercase_digits() {
        assert_eq!(
            encode("© & <", "non-ascii", "hex").unwrap(),
            "&#xA9; &#x26; &#x3C;"
        );
    }

    #[test]
    fn amp_prefers_lowercase_canonical_name() {
        // The table has both &amp; and &AMP;; the canonical lowercase form wins.
        assert_eq!(encode("&", "minimal", "named").unwrap(), "&amp;");
        assert_eq!(encode("©", "named", "named").unwrap(), "&copy;");
    }

    #[test]
    fn empty_input_is_empty() {
        assert_eq!(encode("", "minimal", "named").unwrap(), "");
    }

    #[test]
    fn rejects_unknown_scope() {
        let err = encode("x", "sideways", "named").unwrap_err();
        assert!(err.contains("scope"), "got: {err}");
    }

    #[test]
    fn rejects_unknown_format() {
        let err = encode("x", "minimal", "octal").unwrap_err();
        assert!(err.contains("format"), "got: {err}");
    }

    #[test]
    fn defaults_via_blank_strings() {
        // blank scope → minimal, blank format → named
        assert_eq!(encode("a & b", "", "").unwrap(), "a &amp; b");
    }
}
