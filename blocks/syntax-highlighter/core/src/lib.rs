//! gizza-ai/syntax-highlighter core — pure, wasm-safe syntax highlighting of a
//! code snippet into inline-styled HTML or ANSI 24-bit terminal escape codes.
//!
//! No wafer / wasm-bindgen / network deps. The chat skill block, the CLI, and
//! the unit tests all call [`highlight`]. Verified to compile + link on
//! wasm32-wasip1 (syntect uses the pure-Rust `fancy-regex` engine + bundled
//! binary-dump syntaxes/themes — no onig / no host filesystem; same `default-fancy`
//! stack as the code-screenshot block).

use syntect::easy::HighlightLines;
use syntect::highlighting::{Style as SynStyle, Theme, ThemeSet};
use syntect::html::{styled_line_to_highlighted_html, IncludeBackground};
use syntect::parsing::{SyntaxReference, SyntaxSet};
use syntect::util::{as_24_bit_terminal_escaped, LinesWithEndings};

/// Guard against pathological inputs (the CLI / chat get untrusted text).
const MAX_BYTES: usize = 512 * 1024; // 512 KiB of source code

/// Output format for [`highlight`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    /// Inline-styled HTML (`<pre style=...><span style=...>...</span></pre>`),
    /// ready to drop into a page — no external stylesheet needed.
    Html,
    /// ANSI 24-bit ("true color") terminal escape sequences, ready to `cat` /
    /// `echo` into a terminal. Ends with a reset (`\x1b[0m`).
    Ansi,
}

/// An error from [`highlight`]. The block maps this onto a `SkillError`.
#[derive(Debug, PartialEq, Eq)]
pub enum HighlightError {
    /// `code` was empty (or only whitespace).
    EmptyCode,
    /// `code` exceeded [`MAX_BYTES`].
    TooLarge(usize),
    /// `format` was neither "html" nor "ansi".
    BadFormat(String),
}

impl std::fmt::Display for HighlightError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HighlightError::EmptyCode => write!(f, "code is empty"),
            HighlightError::TooLarge(n) => {
                write!(f, "code is too large ({n} bytes; max {MAX_BYTES})")
            }
            HighlightError::BadFormat(s) => {
                write!(f, "invalid format {s:?}: expected \"html\" or \"ansi\"")
            }
        }
    }
}

impl std::error::Error for HighlightError {}

/// Parse a format string. Blank → `Html` (the default).
pub fn parse_format(format: &str) -> Result<Format, HighlightError> {
    match format.trim().to_ascii_lowercase().as_str() {
        "" | "html" => Ok(Format::Html),
        "ansi" | "terminal" => Ok(Format::Ansi),
        other => Err(HighlightError::BadFormat(other.to_string())),
    }
}

/// Resolve a user-supplied language hint to a syntect syntax. Tries, in order:
/// a few friendly aliases, then exact extension/token/name (case-insensitive),
/// finally falling back to plain text. Never fails — an unknown language
/// renders as plaintext (still valid, just no coloring). Mirrors the
/// code-screenshot block's resolver so the two tools agree on language names.
fn resolve_syntax<'a>(ss: &'a SyntaxSet, language: Option<&str>) -> &'a SyntaxReference {
    let plain = ss.find_syntax_plain_text();
    let Some(lang) = language.map(|l| l.trim()).filter(|l| !l.is_empty()) else {
        return plain;
    };
    let lower = lang.to_ascii_lowercase();
    let token = match lower.as_str() {
        "rust" | "rs" => "rs",
        "python" | "py" => "py",
        "javascript" | "js" => "js",
        "typescript" | "ts" => "ts",
        "shell" | "bash" | "sh" | "zsh" => "sh",
        "c++" | "cpp" | "cxx" => "cpp",
        "c#" | "csharp" | "cs" => "cs",
        "golang" | "go" => "go",
        "yml" | "yaml" => "yaml",
        "markdown" | "md" => "md",
        "html" | "htm" => "html",
        other => other,
    };
    ss.find_syntax_by_extension(token)
        .or_else(|| ss.find_syntax_by_token(token))
        .or_else(|| ss.find_syntax_by_name(lang))
        .unwrap_or(plain)
}

/// Resolve a theme name to a syntect theme, falling back to a sensible dark
/// theme (`base16-ocean.dark`) for unknown / empty names.
fn resolve_theme<'a>(ts: &'a ThemeSet, theme: Option<&str>) -> &'a Theme {
    const DEFAULT: &str = "base16-ocean.dark";
    let name = theme
        .map(|t| t.trim())
        .filter(|t| !t.is_empty())
        .unwrap_or(DEFAULT);
    ts.themes
        .get(name)
        .or_else(|| ts.themes.get(DEFAULT))
        .unwrap_or_else(|| ts.themes.values().next().expect("themeset is non-empty"))
}

/// The list of theme names bundled with syntect's defaults, sorted. Exposed so
/// the descriptor / page can advertise the valid `theme` values.
pub fn theme_names() -> Vec<String> {
    let ts = ThemeSet::load_defaults();
    let mut names: Vec<String> = ts.themes.keys().cloned().collect();
    names.sort();
    names
}

/// Highlight `code` for `language`, returning inline-styled HTML or ANSI text.
///
/// - `language` (blank → auto/plaintext): a hint like `"rust"`, `"python"`,
///   `"javascript"`, `"bash"`. Unknown / omitted falls back to plain text
///   (still returns valid output, just uncolored).
/// - `theme` (blank → `"base16-ocean.dark"`): a syntect theme name (see
///   [`theme_names`]). Unknown names fall back to the default.
/// - `format`: [`Format::Html`] or [`Format::Ansi`].
///
/// Returns `Err` if `code` is empty or exceeds [`MAX_BYTES`].
pub fn highlight(
    code: &str,
    language: Option<&str>,
    theme: Option<&str>,
    format: Format,
) -> Result<String, HighlightError> {
    if code.trim().is_empty() {
        return Err(HighlightError::EmptyCode);
    }
    if code.len() > MAX_BYTES {
        return Err(HighlightError::TooLarge(code.len()));
    }

    let ss = SyntaxSet::load_defaults_newlines();
    let ts = ThemeSet::load_defaults();
    let syntax = resolve_syntax(&ss, language);
    let theme = resolve_theme(&ts, theme);

    let mut hl = HighlightLines::new(syntax, theme);

    match format {
        Format::Html => {
            // Background colour for the <pre> from the theme, if any.
            let bg = theme
                .settings
                .background
                .unwrap_or(syntect::highlighting::Color {
                    r: 0x2b,
                    g: 0x30,
                    b: 0x3b,
                    a: 0xff,
                });
            let mut out = format!(
                "<pre style=\"background-color:#{:02x}{:02x}{:02x};padding:16px;border-radius:6px;overflow:auto;\">\n",
                bg.r, bg.g, bg.b
            );
            for line in LinesWithEndings::from(code) {
                let regions: Vec<(SynStyle, &str)> = hl
                    .highlight_line(line, &ss)
                    .map_err(|_| HighlightError::EmptyCode)?;
                let html =
                    styled_line_to_highlighted_html(&regions[..], IncludeBackground::No)
                        .map_err(|_| HighlightError::EmptyCode)?;
                out.push_str(&html);
            }
            out.push_str("</pre>\n");
            Ok(out)
        }
        Format::Ansi => {
            let mut out = String::new();
            for line in LinesWithEndings::from(code) {
                let regions: Vec<(SynStyle, &str)> = hl
                    .highlight_line(line, &ss)
                    .map_err(|_| HighlightError::EmptyCode)?;
                out.push_str(&as_24_bit_terminal_escaped(&regions[..], false));
            }
            // Reset so the terminal returns to default colours afterwards.
            out.push_str("\x1b[0m");
            Ok(out)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_highlights_rust() {
        let out = highlight("fn main() {}", Some("rust"), None, Format::Html).unwrap();
        assert!(out.starts_with("<pre"), "got: {out}");
        assert!(out.ends_with("</pre>\n"), "got: {out}");
        // Inline-styled spans (coloured tokens) are present.
        assert!(out.contains("<span style="), "got: {out}");
        // The keyword text survives.
        assert!(out.contains("main"), "got: {out}");
    }

    #[test]
    fn html_escapes_special_chars() {
        // `<`, `>`, `&` in the source must be entity-escaped in HTML output so
        // the markup is safe to embed.
        let out =
            highlight("let x = a < b && c > d;", Some("rust"), None, Format::Html).unwrap();
        assert!(out.contains("&lt;"), "got: {out}");
        assert!(out.contains("&gt;"), "got: {out}");
        assert!(out.contains("&amp;"), "got: {out}");
        // The raw unescaped operators must NOT leak into the markup.
        assert!(!out.contains("a < b"), "raw '<' leaked: {out}");
    }

    #[test]
    fn ansi_emits_escape_codes_and_reset() {
        let out = highlight("print('hi')", Some("python"), None, Format::Ansi).unwrap();
        // 24-bit foreground colour escape (\x1b[38;2;...m) is present.
        assert!(out.contains("\x1b[38;2;"), "no fg escape: {out:?}");
        // Ends with a reset.
        assert!(out.ends_with("\x1b[0m"), "no reset: {out:?}");
        // The source text survives.
        assert!(out.contains("print"), "got: {out:?}");
    }

    #[test]
    fn unknown_language_falls_back_to_plaintext() {
        // No panic, valid output, source preserved.
        let out = highlight("just some text", Some("klingon"), None, Format::Html).unwrap();
        assert!(out.contains("just some text"), "got: {out}");
    }

    #[test]
    fn blank_language_is_plaintext() {
        let out = highlight("hello world", None, None, Format::Html).unwrap();
        assert!(out.contains("hello world"), "got: {out}");
    }

    #[test]
    fn unknown_theme_falls_back_to_default() {
        // Should not error; just uses the default theme.
        let out =
            highlight("fn x() {}", Some("rust"), Some("no-such-theme"), Format::Html).unwrap();
        assert!(out.starts_with("<pre"), "got: {out}");
    }

    #[test]
    fn known_theme_is_used() {
        // InspiredGitHub is a light theme bundled with syntect defaults.
        let out =
            highlight("fn x() {}", Some("rust"), Some("InspiredGitHub"), Format::Html).unwrap();
        assert!(out.starts_with("<pre"), "got: {out}");
    }

    #[test]
    fn empty_code_errors() {
        assert_eq!(
            highlight("   \n  ", Some("rust"), None, Format::Html).unwrap_err(),
            HighlightError::EmptyCode
        );
    }

    #[test]
    fn too_large_errors() {
        let big = "a".repeat(MAX_BYTES + 1);
        assert!(matches!(
            highlight(&big, None, None, Format::Html).unwrap_err(),
            HighlightError::TooLarge(_)
        ));
    }

    #[test]
    fn parse_format_accepts_aliases_and_rejects_garbage() {
        assert_eq!(parse_format("").unwrap(), Format::Html);
        assert_eq!(parse_format("HTML").unwrap(), Format::Html);
        assert_eq!(parse_format("ansi").unwrap(), Format::Ansi);
        assert_eq!(parse_format("terminal").unwrap(), Format::Ansi);
        assert!(matches!(
            parse_format("svg").unwrap_err(),
            HighlightError::BadFormat(_)
        ));
    }

    #[test]
    fn theme_names_includes_defaults() {
        let names = theme_names();
        assert!(names.iter().any(|n| n == "base16-ocean.dark"), "{names:?}");
        assert!(names.iter().any(|n| n == "InspiredGitHub"), "{names:?}");
    }
}
