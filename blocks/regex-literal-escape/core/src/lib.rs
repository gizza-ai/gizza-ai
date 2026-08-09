//! regex-literal-escape core — turn arbitrary text into a regex-safe literal for a
//! chosen regex flavor. Pure compute, shared by the chat skill block and the web page.
//! No wafer/wasm-bindgen deps.
//!
//! Every flavor reproduces the escaping its own standard-library helper performs, so the
//! output is byte-identical to what `preg_quote` / `RegExp.escape` / `re.escape` /
//! `regexp.QuoteMeta` / `Regex.Escape` / `Pattern.quote` / `Regexp.escape` /
//! `regex::escape` would produce. The metacharacter sets genuinely differ between
//! flavors, which is why picking the right one matters.

/// Maximum accepted input length, in characters.
pub const MAX_TEXT_CHARS: usize = 100_000;

/// Regex dialect whose escaping rules to reproduce.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flavor {
    /// PHP `preg_quote()` / PCRE.
    Pcre,
    /// The classic JavaScript `escapeRegExp` idiom used with `new RegExp(...)`.
    JavaScript,
    /// ECMAScript `RegExp.escape()` (ES2025) — safe in every position, `\xNN`-heavy.
    JavaScriptStrict,
    /// Python `re.escape()` (3.7+).
    Python,
    /// Go `regexp.QuoteMeta()` / RE2.
    Re2,
    /// .NET `Regex.Escape()`.
    Dotnet,
    /// Java `Pattern.quote()` — `\Q…\E` quoting.
    Java,
    /// Ruby `Regexp.escape()`.
    Ruby,
    /// Rust `regex::escape()`.
    Rust,
}

impl Flavor {
    /// Parse a flavor name. Common language aliases (`php`, `js`, `go`, `csharp`, …) are accepted.
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "pcre" | "php" | "preg_quote" | "perl" => Ok(Flavor::Pcre),
            "javascript" | "js" => Ok(Flavor::JavaScript),
            "javascript-strict" | "js-strict" | "regexp.escape" => Ok(Flavor::JavaScriptStrict),
            "python" | "py" => Ok(Flavor::Python),
            "re2" | "go" | "golang" => Ok(Flavor::Re2),
            "dotnet" | ".net" | "csharp" | "c#" => Ok(Flavor::Dotnet),
            "java" => Ok(Flavor::Java),
            "ruby" | "rb" => Ok(Flavor::Ruby),
            "rust" => Ok(Flavor::Rust),
            other => Err(format!(
                "unknown flavor '{other}' (use pcre, javascript, javascript-strict, python, re2, dotnet, java, ruby, or rust)"
            )),
        }
    }

    /// Characters this flavor escapes with a plain backslash prefix.
    fn metachars(self) -> &'static str {
        match self {
            // preg_quote: . \ + * ? [ ^ ] $ ( ) { } = ! < > | : - #
            Flavor::Pcre => r".\+*?[^]$(){}=!<>|:-#",
            // The MDN escapeRegExp idiom: [.*+?^${}()|[\]\\]
            Flavor::JavaScript => r"^$\.*+?()[]{}|",
            // CPython _special_chars_map (whitespace handled separately)
            Flavor::Python => r"()[]{}?*+-|^$\.&~#",
            // Go specialBytes
            Flavor::Re2 => r"\.+*?()|[]{}^$",
            // .NET RegexParser metachars (whitespace handled separately)
            Flavor::Dotnet => r"\*+?|{[()^$.#",
            // Ruby rb_reg_quote (whitespace handled separately)
            Flavor::Ruby => r".*?+^$|()[]{}\-#",
            // regex-syntax is_meta_character
            Flavor::Rust => r"\.+*?()|[]{}^$#&-~",
            // Handled by their own routines.
            Flavor::JavaScriptStrict | Flavor::Java => "",
        }
    }

    /// How this flavor natively escapes whitespace, when `escape_whitespace` is off.
    fn native_whitespace(self, c: char) -> Option<&'static str> {
        match self {
            // re.escape maps each whitespace byte to backslash + the character itself.
            Flavor::Python => match c {
                ' ' => Some("\\ "),
                '\t' => Some("\\\t"),
                '\n' => Some("\\\n"),
                '\r' => Some("\\\r"),
                '\u{b}' => Some("\\\u{b}"),
                '\u{c}' => Some("\\\u{c}"),
                _ => None,
            },
            Flavor::Dotnet => match c {
                ' ' => Some("\\ "),
                '\t' => Some("\\t"),
                '\n' => Some("\\n"),
                '\r' => Some("\\r"),
                '\u{c}' => Some("\\f"),
                _ => None,
            },
            Flavor::Ruby => match c {
                ' ' => Some("\\ "),
                '\t' => Some("\\t"),
                '\n' => Some("\\n"),
                '\r' => Some("\\r"),
                '\u{b}' => Some("\\v"),
                '\u{c}' => Some("\\f"),
                _ => None,
            },
            _ => None,
        }
    }
}

/// Portable whitespace escapes used when `escape_whitespace` is on. `\xNN` for the
/// space keeps the result valid under PCRE `/x`, RE2 and Rust, which all reject `\ `.
fn forced_whitespace(c: char) -> Option<&'static str> {
    match c {
        ' ' => Some("\\x20"),
        '\t' => Some("\\t"),
        '\n' => Some("\\n"),
        '\r' => Some("\\r"),
        '\u{b}' => Some("\\v"),
        '\u{c}' => Some("\\f"),
        _ => None,
    }
}

/// Whitespace / line terminators `RegExp.escape()` emits as `\uXXXX`.
fn is_js_other_whitespace(c: char) -> bool {
    matches!(c,
        '\u{a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}'
        | '\u{2028}' | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}' | '\u{feff}')
}

/// ECMAScript `RegExp.escape()` (ES2025).
fn js_regexp_escape(text: &str, extra: &str) -> String {
    const SYNTAX: &str = r"^$\.*+?()[]{}|/";
    const OTHER_PUNCTUATORS: &str = ",-=<>#&!%:;@~'`\" ";
    let mut out = String::with_capacity(text.len() + 8);
    for (i, c) in text.chars().enumerate() {
        // The first code point is hex-escaped when it is a decimal digit or an ASCII
        // letter, so the literal can never be spliced into \1 / \x0 / \u000 sequences.
        if i == 0 && c.is_ascii_alphanumeric() {
            out.push_str(&format!("\\x{:02x}", c as u32));
            continue;
        }
        match c {
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\u{b}' => out.push_str("\\v"),
            '\u{c}' => out.push_str("\\f"),
            '\r' => out.push_str("\\r"),
            c if SYNTAX.contains(c) => {
                out.push('\\');
                out.push(c);
            }
            c if extra.contains(c) && c.is_ascii() => {
                out.push_str(&format!("\\x{:02x}", c as u32));
            }
            c if OTHER_PUNCTUATORS.contains(c) => {
                out.push_str(&format!("\\x{:02x}", c as u32));
            }
            c if is_js_other_whitespace(c) => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// Java `Pattern.quote()` — wrap in `\Q…\E`, splicing any embedded `\E`.
fn java_quote(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 4);
    out.push_str("\\Q");
    let mut rest = text;
    while let Some(pos) = rest.find("\\E") {
        out.push_str(&rest[..pos]);
        out.push_str("\\E\\\\E\\Q");
        rest = &rest[pos + 2..];
    }
    out.push_str(rest);
    out.push_str("\\E");
    out
}

fn escape_flavor(text: &str, flavor: Flavor, extra: &str, escape_whitespace: bool) -> String {
    match flavor {
        Flavor::Java => java_quote(text),
        Flavor::JavaScriptStrict => js_regexp_escape(text, extra),
        _ => {
            let metas = flavor.metachars();
            let mut out = String::with_capacity(text.len() + 8);
            for c in text.chars() {
                if escape_whitespace {
                    if let Some(s) = forced_whitespace(c) {
                        out.push_str(s);
                        continue;
                    }
                }
                if flavor == Flavor::Pcre && c == '\0' {
                    // preg_quote() writes the NUL byte as its octal escape.
                    out.push_str("\\000");
                    continue;
                }
                if metas.contains(c) || extra.contains(c) {
                    out.push('\\');
                    out.push(c);
                    continue;
                }
                if let Some(s) = flavor.native_whitespace(c) {
                    out.push_str(s);
                    continue;
                }
                out.push(c);
            }
            out
        }
    }
}

/// Escape the result again so it can be pasted between the quotes of a source-code
/// string literal (Java/C#/Go/JS): backslashes double, quotes and real newlines escape.
fn to_string_literal(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out
}

fn validate_delimiter(delimiter: &str) -> Result<String, String> {
    let mut seen = String::new();
    for c in delimiter.chars() {
        if c.is_alphanumeric() || c == '_' {
            return Err(format!(
                "delimiter character '{c}' is a letter, digit, or underscore — escaping it would create an escape sequence like \\d or \\w; use punctuation such as / # ~ or %"
            ));
        }
        if c.is_whitespace() {
            return Err(
                "delimiter must not contain whitespace — use escape_whitespace instead".to_string(),
            );
        }
        if !seen.contains(c) {
            seen.push(c);
        }
    }
    Ok(seen)
}

/// Escape `text` into a regex-safe literal for `flavor`.
///
/// * `delimiter` — extra punctuation characters to escape as well (the second argument
///   of `preg_quote`); typically the pattern delimiter such as `/` or `#`.
/// * `escape_whitespace` — emit portable `\t \n \r \f \v \x20` escapes for whitespace
///   instead of the flavor's native handling (required under PCRE/Ruby `/x` mode).
/// * `string_literal` — additionally escape the result for pasting inside a source-code
///   string literal.
pub fn escape(
    text: &str,
    flavor: Flavor,
    delimiter: &str,
    escape_whitespace: bool,
    string_literal: bool,
) -> Result<String, String> {
    let len = text.chars().count();
    if len > MAX_TEXT_CHARS {
        return Err(format!(
            "text is too long ({len} characters); the limit is {MAX_TEXT_CHARS}"
        ));
    }
    let extra = validate_delimiter(delimiter)?;
    let escaped = escape_flavor(text, flavor, &extra, escape_whitespace);
    Ok(if string_literal {
        to_string_literal(&escaped)
    } else {
        escaped
    })
}

/// String-keyed entry point used by the chat/web/CLI wrappers.
pub fn run(
    text: &str,
    flavor: &str,
    delimiter: &str,
    escape_whitespace: bool,
    string_literal: bool,
) -> Result<String, String> {
    let flavor = Flavor::parse(flavor)?;
    escape(text, flavor, delimiter, escape_whitespace, string_literal)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn esc(text: &str, flavor: &str) -> String {
        run(text, flavor, "", false, false).unwrap()
    }

    #[test]
    fn pcre_escapes_preg_quote_set() {
        // preg_quote('$40 for a g3/400') — '/' is NOT escaped without a delimiter.
        assert_eq!(esc("$40 for a g3/400", "pcre"), "\\$40 for a g3/400");
        assert_eq!(esc("a.b*c+(d)", "pcre"), "a\\.b\\*c\\+\\(d\\)");
        assert_eq!(esc("a-b=c!d<e>f:g#h", "pcre"), "a\\-b\\=c\\!d\\<e\\>f\\:g\\#h");
        // NUL is written as its octal escape.
        assert_eq!(esc("a\0b", "pcre"), "a\\000b");
    }

    #[test]
    fn pcre_delimiter_is_escaped_too() {
        assert_eq!(
            run("$40 for a g3/400", "pcre", "/", false, false).unwrap(),
            "\\$40 for a g3\\/400"
        );
        assert_eq!(run("a#b", "pcre", "~", false, false).unwrap(), "a\\#b");
        assert_eq!(run("a~b", "re2", "~", false, false).unwrap(), "a\\~b");
    }

    #[test]
    fn javascript_uses_the_escape_regexp_idiom() {
        assert_eq!(esc("a.b*c+(d)", "javascript"), "a\\.b\\*c\\+\\(d\\)");
        // The idiom deliberately leaves '-' '#' '/' alone.
        assert_eq!(esc("a-b#c/d", "javascript"), "a-b#c/d");
        assert_eq!(esc("1 + 1 = 2", "js"), "1 \\+ 1 = 2");
    }

    #[test]
    fn javascript_strict_matches_regexp_escape() {
        // Leading ASCII letter/digit is hex-escaped; '.' takes a backslash.
        assert_eq!(esc("foo.bar", "javascript-strict"), "\\x66oo\\.bar");
        assert_eq!(esc("5bar", "javascript-strict"), "\\x35bar");
        assert_eq!(esc("foo-bar", "javascript-strict"), "\\x66oo\\x2dbar");
        assert_eq!(esc("(foo)", "javascript-strict"), "\\(foo\\)");
        assert_eq!(esc("foo\nbar", "javascript-strict"), "\\x66oo\\nbar");
        assert_eq!(
            esc("Buy it. use it.", "javascript-strict"),
            "\\x42uy\\x20it\\.\\x20use\\x20it\\."
        );
        assert_eq!(esc("foo\u{2028}bar", "javascript-strict"), "\\x66oo\\u2028bar");
    }

    #[test]
    fn python_escapes_only_specials_and_whitespace() {
        assert_eq!(esc("https://www.python.org", "python"), "https://www\\.python\\.org");
        assert_eq!(esc("a-b~c&d#e", "python"), "a\\-b\\~c\\&d\\#e");
        // ! " % ' , / : ; < = > @ ` are not escaped since Python 3.7.
        assert_eq!(esc("!\"%',/:;<=>@`", "python"), "!\"%',/:;<=>@`");
        // Whitespace becomes backslash + the character itself.
        assert_eq!(esc("a b", "python"), "a\\ b");
        assert_eq!(esc("a\tb", "python"), "a\\\tb");
    }

    #[test]
    fn re2_quotemeta_is_the_narrowest_set() {
        assert_eq!(esc("a.b*c+(d)", "re2"), "a\\.b\\*c\\+\\(d\\)");
        // Go leaves '-', '#', '/', '~', '&' and whitespace untouched.
        assert_eq!(esc("a-b#c/d~e&f g", "go"), "a-b#c/d~e&f g");
        assert_eq!(esc("^$|[]{}", "re2"), "\\^\\$\\|\\[\\]\\{\\}");
    }

    #[test]
    fn dotnet_escapes_whitespace_but_not_closers() {
        // Regex.Escape leaves ']' and '}' unescaped and turns a space into "\ ".
        assert_eq!(esc("a]b}c", "dotnet"), "a]b}c");
        assert_eq!(esc("a b", "dotnet"), "a\\ b");
        assert_eq!(esc("a\nb\tc", "dotnet"), "a\\nb\\tc");
        assert_eq!(esc("a.b*c+(d)", "csharp"), "a\\.b\\*c\\+\\(d\\)");
    }

    #[test]
    fn java_wraps_in_quote_block_and_splices_embedded_end() {
        assert_eq!(esc("a.b*c+(d)", "java"), "\\Qa.b*c+(d)\\E");
        assert_eq!(esc("a\\Eb", "java"), "\\Qa\\E\\\\E\\Qb\\E");
    }

    #[test]
    fn ruby_and_rust_have_their_own_sets() {
        assert_eq!(esc("a b\nc", "ruby"), "a\\ b\\nc");
        assert_eq!(esc("a-b#c", "ruby"), "a\\-b\\#c");
        // Rust's regex::escape covers '#', '&', '-', '~' but not whitespace.
        assert_eq!(esc("a-b#c&d~e f", "rust"), "a\\-b\\#c\\&d\\~e f");
    }

    #[test]
    fn escape_whitespace_is_portable_across_flavors() {
        assert_eq!(run("a b\tc\nd", "pcre", "", true, false).unwrap(), "a\\x20b\\tc\\nd");
        assert_eq!(run("a b", "re2", "", true, false).unwrap(), "a\\x20b");
        // It overrides Python's native backslash-plus-literal form.
        assert_eq!(run("a b\nc", "python", "", true, false).unwrap(), "a\\x20b\\nc");
    }

    #[test]
    fn string_literal_doubles_backslashes() {
        assert_eq!(
            run("a.b\"c", "pcre", "", false, true).unwrap(),
            "a\\\\.b\\\"c"
        );
        assert_eq!(run("a.b", "java", "", false, true).unwrap(), "\\\\Qa.b\\\\E");
    }

    #[test]
    fn empty_input_is_allowed() {
        assert_eq!(esc("", "pcre"), "");
        assert_eq!(esc("", "java"), "\\Q\\E");
    }

    #[test]
    fn unicode_passes_through_unchanged() {
        assert_eq!(esc("café ☕ 😀", "pcre"), "café ☕ 😀");
    }

    #[test]
    fn unknown_flavor_is_an_error() {
        let err = run("x", "perl6", "", false, false).unwrap_err();
        assert!(err.contains("unknown flavor 'perl6'"), "{err}");
    }

    #[test]
    fn alphanumeric_delimiter_is_an_error() {
        let err = run("x", "pcre", "d", false, false).unwrap_err();
        assert!(err.contains("letter, digit, or underscore"), "{err}");
        let err = run("x", "pcre", " ", false, false).unwrap_err();
        assert!(err.contains("whitespace"), "{err}");
    }

    #[test]
    fn over_long_text_is_an_error() {
        let big = "a".repeat(MAX_TEXT_CHARS + 1);
        let err = run(&big, "pcre", "", false, false).unwrap_err();
        assert!(err.contains("too long"), "{err}");
        // The exact boundary is accepted.
        let ok = "a".repeat(MAX_TEXT_CHARS);
        assert_eq!(run(&ok, "pcre", "", false, false).unwrap().len(), MAX_TEXT_CHARS);
    }
}
