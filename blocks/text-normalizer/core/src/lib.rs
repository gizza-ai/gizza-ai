//! text-normalizer core — clean and normalize text in one configurable pass.
//! No wafer/wasm-bindgen deps; shared by the chat skill block and the web page.
//!
//! The pipeline runs in this fixed order (each step is optional):
//!   1. **punctuation** — fold fancy Unicode punctuation (curly quotes, en/em
//!      dashes, ellipsis, prime marks, guillemets) to plain ASCII.
//!   2. **form** — apply a Unicode normalization form (NFC / NFD / NFKC / NFKD).
//!   3. **strip_accents** — decompose and drop combining marks (é → e).
//!   4. **case** — lower / UPPER / fold (caseless, ß → ss).
//!   5. **whitespace** — trim / collapse runs / collapse per line.
//!
//! Whitespace runs last so it tidies anything the earlier steps introduce.

use unicode_normalization::char::is_combining_mark;
use unicode_normalization::UnicodeNormalization;

/// Unicode normalization form to apply.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Form {
    /// Leave the code points as-is.
    None,
    /// Canonical composition (NFC) — the web-recommended default.
    Nfc,
    /// Canonical decomposition (NFD).
    Nfd,
    /// Compatibility composition (NFKC) — folds ligatures (ﬁ → fi), full-width,
    /// circled/styled letters, fractions, etc.
    Nfkc,
    /// Compatibility decomposition (NFKD).
    Nfkd,
}

impl Form {
    /// Parse a form name (case-insensitive; blank → `Nfc`). Unknown → `Err`.
    pub fn parse(s: &str) -> Result<Form, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "nfc" => Ok(Form::Nfc),
            "none" | "off" => Ok(Form::None),
            "nfd" => Ok(Form::Nfd),
            "nfkc" => Ok(Form::Nfkc),
            "nfkd" => Ok(Form::Nfkd),
            other => Err(format!(
                "invalid form {other:?}: expected one of none, nfc, nfd, nfkc, nfkd"
            )),
        }
    }
}

/// Case transform to apply after normalization.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Case {
    /// Leave letter case untouched.
    None,
    /// Lowercase (full Unicode mapping).
    Lower,
    /// UPPERCASE (full Unicode mapping).
    Upper,
    /// Case fold for caseless matching — like lowercase but also maps ß → ss so
    /// "STRASSE" and "straße" compare equal.
    Fold,
}

impl Case {
    /// Parse a case name (case-insensitive; blank → `None`). Unknown → `Err`.
    pub fn parse(s: &str) -> Result<Case, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "none" | "keep" => Ok(Case::None),
            "lower" | "lowercase" => Ok(Case::Lower),
            "upper" | "uppercase" => Ok(Case::Upper),
            "fold" | "casefold" | "case-fold" => Ok(Case::Fold),
            other => Err(format!(
                "invalid case {other:?}: expected one of none, lower, upper, fold"
            )),
        }
    }
}

/// How to normalize whitespace.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Whitespace {
    /// Leave all whitespace untouched.
    Keep,
    /// Trim leading/trailing whitespace only; keep interior spacing.
    Trim,
    /// Collapse every run of whitespace (spaces, tabs, newlines, Unicode
    /// spaces) to a single ASCII space, and trim the ends.
    Collapse,
    /// Collapse spaces/tabs within each line and trim each line, but keep line
    /// breaks (blank lines are removed; `\r\n` and `\r` become `\n`).
    CollapseLines,
}

impl Whitespace {
    /// Parse a whitespace mode (case-insensitive; blank → `Collapse`). Unknown → `Err`.
    pub fn parse(s: &str) -> Result<Whitespace, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "collapse" => Ok(Whitespace::Collapse),
            "keep" | "none" => Ok(Whitespace::Keep),
            "trim" => Ok(Whitespace::Trim),
            "collapse-lines" | "collapse_lines" | "lines" => Ok(Whitespace::CollapseLines),
            other => Err(format!(
                "invalid whitespace {other:?}: expected one of keep, trim, collapse, collapse-lines"
            )),
        }
    }
}

/// All normalization options. Build with [`Options::default`] then override.
#[derive(Clone, Copy)]
pub struct Options {
    pub form: Form,
    pub case: Case,
    pub strip_accents: bool,
    pub whitespace: Whitespace,
    pub punctuation: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            form: Form::Nfc,
            case: Case::None,
            strip_accents: false,
            whitespace: Whitespace::Collapse,
            punctuation: false,
        }
    }
}

/// Normalize `text` per `opts`. Never fails (option parsing is done upstream);
/// returns the cleaned string.
pub fn normalize(text: &str, opts: Options) -> String {
    let mut s = text.to_string();

    if opts.punctuation {
        s = normalize_punctuation(&s);
    }

    s = match opts.form {
        Form::None => s,
        Form::Nfc => s.nfc().collect(),
        Form::Nfd => s.nfd().collect(),
        Form::Nfkc => s.nfkc().collect(),
        Form::Nfkd => s.nfkd().collect(),
    };

    if opts.strip_accents {
        // Decompose, drop combining marks, then recompose to NFC so downstream
        // steps see clean composed base letters (unless the caller explicitly
        // asked for a decomposed form).
        s = s.nfd().filter(|c| !is_combining_mark(*c)).collect::<String>();
        if opts.form != Form::Nfd && opts.form != Form::Nfkd {
            s = s.nfc().collect();
        }
    }

    s = match opts.case {
        Case::None => s,
        Case::Lower => s.to_lowercase(),
        Case::Upper => s.to_uppercase(),
        Case::Fold => case_fold(&s),
    };

    match opts.whitespace {
        Whitespace::Keep => s,
        Whitespace::Trim => s.trim().to_string(),
        Whitespace::Collapse => collapse_whitespace(&s),
        Whitespace::CollapseLines => collapse_lines(&s),
    }
}

/// Fold fancy Unicode punctuation to plain ASCII: curly quotes and guillemets →
/// straight quotes, en/em dashes and the horizontal bar → `-`, the ellipsis
/// glyph → `...`, and prime marks → `'`/`"`.
fn normalize_punctuation(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            // double quotes / guillemets / double prime
            '\u{201C}' | '\u{201D}' | '\u{201E}' | '\u{201F}' | '\u{00AB}' | '\u{00BB}'
            | '\u{2033}' | '\u{301D}' | '\u{301E}' => out.push('"'),
            // single quotes / prime / single guillemets
            '\u{2018}' | '\u{2019}' | '\u{201A}' | '\u{201B}' | '\u{2039}' | '\u{203A}'
            | '\u{2032}' | '\u{2035}' => out.push('\''),
            // dashes / hyphens / minus
            '\u{2010}' | '\u{2011}' | '\u{2012}' | '\u{2013}' | '\u{2014}' | '\u{2015}'
            | '\u{2212}' => out.push('-'),
            // ellipsis
            '\u{2026}' => out.push_str("..."),
            other => out.push(other),
        }
    }
    out
}

/// Case fold for caseless matching: lowercase everything, plus the common
/// full-fold expansion Rust's `to_lowercase` doesn't perform (ß → ss).
fn case_fold(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            'ß' => out.push_str("ss"),
            _ => {
                for l in c.to_lowercase() {
                    out.push(l);
                }
            }
        }
    }
    out
}

/// Collapse every run of Unicode whitespace to a single ASCII space and trim ends.
fn collapse_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Collapse spaces/tabs within each line, trim each line, keep non-blank lines
/// separated by a single `\n`. `\r\n` and `\r` line endings are normalized to `\n`.
fn collapse_lines(text: &str) -> String {
    text.replace("\r\n", "\n")
        .replace('\r', "\n")
        .lines()
        .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts() -> Options {
        // A neutral base for tests: no-op except the pieces each test toggles.
        Options {
            form: Form::None,
            case: Case::None,
            strip_accents: false,
            whitespace: Whitespace::Keep,
            punctuation: false,
        }
    }

    #[test]
    fn nfkc_folds_ligatures_and_fullwidth() {
        let o = Options {
            form: Form::Nfkc,
            ..opts()
        };
        // ﬁ ligature (U+FB01) → "fi"; full-width A (U+FF21) → "A".
        assert_eq!(normalize("\u{FB01}le \u{FF21}BC", o), "file ABC");
    }

    #[test]
    fn strip_accents_removes_diacritics() {
        let o = Options {
            strip_accents: true,
            ..opts()
        };
        assert_eq!(normalize("café résumé naïve", o), "cafe resume naive");
        // Also works on precomposed input.
        assert_eq!(normalize("Å ñ", o), "A n");
    }

    #[test]
    fn case_transforms() {
        assert_eq!(
            normalize(
                "Straße",
                Options {
                    case: Case::Lower,
                    ..opts()
                }
            ),
            "straße"
        );
        assert_eq!(
            normalize(
                "Straße",
                Options {
                    case: Case::Fold,
                    ..opts()
                }
            ),
            "strasse"
        );
        assert_eq!(
            normalize(
                "café",
                Options {
                    case: Case::Upper,
                    ..opts()
                }
            ),
            "CAFÉ"
        );
    }

    #[test]
    fn whitespace_collapse_and_trim() {
        let collapse = Options {
            whitespace: Whitespace::Collapse,
            ..opts()
        };
        assert_eq!(normalize("  a\t b\n\n c  ", collapse), "a b c");
        // Unicode non-breaking / thin spaces collapse too.
        assert_eq!(normalize("a\u{00A0}\u{2009}b", collapse), "a b");

        let trim = Options {
            whitespace: Whitespace::Trim,
            ..opts()
        };
        assert_eq!(normalize("  a  b  ", trim), "a  b");

        let lines = Options {
            whitespace: Whitespace::CollapseLines,
            ..opts()
        };
        assert_eq!(
            normalize("  first  line \r\n\r\n second   line  ", lines),
            "first line\nsecond line"
        );
    }

    #[test]
    fn punctuation_folds_to_ascii() {
        let o = Options {
            punctuation: true,
            ..opts()
        };
        assert_eq!(
            normalize("\u{201C}Hi\u{201D}\u{2014}it\u{2019}s\u{2026}", o),
            "\"Hi\"-it's..."
        );
    }

    #[test]
    fn full_pipeline_nlp_preset() {
        // punctuation + nfkc + strip accents + fold + collapse.
        let o = Options {
            form: Form::Nfkc,
            case: Case::Fold,
            strip_accents: true,
            whitespace: Whitespace::Collapse,
            punctuation: true,
        };
        assert_eq!(
            normalize("  Café  \u{201C}RÉSUMÉ\u{201D}\u{2026}  ", o),
            "cafe \"resume\"..."
        );
    }

    #[test]
    fn default_options_collapse_and_nfc() {
        // Default: NFC + collapse whitespace, nothing else.
        assert_eq!(normalize("  hello   world  ", Options::default()), "hello world");
    }

    #[test]
    fn parsers_accept_aliases_and_reject_unknown() {
        assert!(Form::parse("NFKC").is_ok());
        assert!(Form::parse("").is_ok());
        assert!(Form::parse("nfkx").is_err());
        assert!(Case::parse("Fold").is_ok());
        assert!(Case::parse("bogus").is_err());
        assert!(Whitespace::parse("collapse-lines").is_ok());
        assert!(Whitespace::parse("nope").is_err());
    }

    #[test]
    fn empty_input_is_ok() {
        assert_eq!(normalize("", Options::default()), "");
    }
}
