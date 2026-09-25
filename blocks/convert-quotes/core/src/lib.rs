//! convert-quotes core — pure compute, shared by the chat skill block and the
//! web page. No wafer/wasm-bindgen deps.
//!
//! Converts the *quote delimiters* around quoted runs in text or code between
//! styles — single `'…'`, double `"…"`, curly double `“…”` and curly single
//! `‘…’` — while leaving everything else byte-identical.
//!
//! What makes this different from a blind find-and-replace:
//!
//!  - **Escapes are respected.** A backslash escape (`\"`, `\'`, `\\`, `\n`) is
//!    never mistaken for a delimiter, and an escaped quote that no longer needs
//!    escaping in the new style is unescaped (`"a \" b"` → `'a " b'`).
//!  - **Inner quotes are re-escaped** so the result is still a valid literal —
//!    backslash (`\'`), doubled (`''`, the SQL/CSV convention), or left bare.
//!  - **Apostrophes survive.** A `'` or `’` sitting between two word characters
//!    (`don't`, `it’s`, `O'Hara`) is an apostrophe, not a delimiter.
//!  - **Unbalanced quotes are not swallowed.** An opening quote with no partner
//!    is either left exactly as it was or reported as an error.

/// Largest input accepted, in bytes. Comfortably covers a long source file or
/// document while keeping a single conversion instant in the browser.
pub const MAX_BYTES: usize = 1_000_000;

// Curly / typographic delimiters.
const LDQUO: char = '\u{201C}'; // “ left double quotation mark
const RDQUO: char = '\u{201D}'; // ” right double quotation mark
const LSQUO: char = '\u{2018}'; // ‘ left single quotation mark
const RSQUO: char = '\u{2019}'; // ’ right single quotation mark (also apostrophe)

/// A style of quote delimiter, as an opening/closing character pair.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum QuoteKind {
    /// `'…'`
    StraightSingle,
    /// `"…"`
    StraightDouble,
    /// `‘…’`
    CurlySingle,
    /// `“…”`
    CurlyDouble,
}

impl QuoteKind {
    fn open(self) -> char {
        match self {
            QuoteKind::StraightSingle => '\'',
            QuoteKind::StraightDouble => '"',
            QuoteKind::CurlySingle => LSQUO,
            QuoteKind::CurlyDouble => LDQUO,
        }
    }

    fn close(self) -> char {
        match self {
            QuoteKind::StraightSingle => '\'',
            QuoteKind::StraightDouble => '"',
            QuoteKind::CurlySingle => RSQUO,
            QuoteKind::CurlyDouble => RDQUO,
        }
    }

    /// Single-quote family — the one that collides with apostrophes.
    fn is_single(self) -> bool {
        matches!(self, QuoteKind::StraightSingle | QuoteKind::CurlySingle)
    }
}

/// Which delimiters to read, and which to write.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    /// `'…'` → `"…"` (the default).
    SingleToDouble,
    /// `"…"` → `'…'`
    DoubleToSingle,
    /// `“…”` / `‘…’` → `"…"`
    SmartToDouble,
    /// `“…”` / `‘…’` → `'…'`
    SmartToSingle,
    /// Any quoted run (straight or curly) → `"…"`
    AutoToDouble,
    /// Any quoted run (straight or curly) → `'…'`
    AutoToSingle,
    /// `'…'` ⇄ `"…"` in one pass.
    Swap,
}

impl Direction {
    /// Parse the option string sent by the chat schema / CLI / page select.
    /// Hyphen and underscore spellings are both accepted; blank means the
    /// default `single-to-double`.
    pub fn parse(s: &str) -> Result<Direction, String> {
        match s.trim().replace('_', "-").as_str() {
            "" | "single-to-double" => Ok(Direction::SingleToDouble),
            "double-to-single" => Ok(Direction::DoubleToSingle),
            "smart-to-double" => Ok(Direction::SmartToDouble),
            "smart-to-single" => Ok(Direction::SmartToSingle),
            "auto-to-double" => Ok(Direction::AutoToDouble),
            "auto-to-single" => Ok(Direction::AutoToSingle),
            "swap" => Ok(Direction::Swap),
            other => Err(format!(
                "unknown direction {other:?} — expected one of: single-to-double, \
                 double-to-single, smart-to-double, smart-to-single, auto-to-double, \
                 auto-to-single, swap"
            )),
        }
    }

    /// The delimiter styles this direction treats as quotes in the input.
    fn sources(self) -> &'static [QuoteKind] {
        use QuoteKind::*;
        match self {
            Direction::SingleToDouble => &[StraightSingle],
            Direction::DoubleToSingle => &[StraightDouble],
            Direction::SmartToDouble | Direction::SmartToSingle => &[CurlyDouble, CurlySingle],
            Direction::AutoToDouble | Direction::AutoToSingle => {
                &[StraightDouble, StraightSingle, CurlyDouble, CurlySingle]
            }
            Direction::Swap => &[StraightDouble, StraightSingle],
        }
    }

    /// The delimiter style to emit for a run that opened with `matched`.
    fn target(self, matched: QuoteKind) -> QuoteKind {
        match self {
            Direction::SingleToDouble
            | Direction::SmartToDouble
            | Direction::AutoToDouble => QuoteKind::StraightDouble,
            Direction::DoubleToSingle
            | Direction::SmartToSingle
            | Direction::AutoToSingle => QuoteKind::StraightSingle,
            Direction::Swap => match matched {
                QuoteKind::StraightSingle => QuoteKind::StraightDouble,
                _ => QuoteKind::StraightSingle,
            },
        }
    }
}

/// How to escape a quote character that appears *inside* the converted run and
/// collides with the new delimiter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EscapeStyle {
    /// `\"` / `\'` — the C, JavaScript, Python, JSON, Rust and Go convention.
    Backslash,
    /// `""` / `''` — the SQL, CSV and Pascal convention.
    Doubled,
    /// Leave the inner quote bare. Fine for prose, unsafe for code.
    Bare,
}

impl EscapeStyle {
    /// Parse the option string; blank means the default `backslash`.
    pub fn parse(s: &str) -> Result<EscapeStyle, String> {
        match s.trim() {
            "" | "backslash" => Ok(EscapeStyle::Backslash),
            "doubled" => Ok(EscapeStyle::Doubled),
            "bare" | "none" => Ok(EscapeStyle::Bare),
            other => Err(format!(
                "unknown escape_style {other:?} — expected one of: backslash, doubled, bare"
            )),
        }
    }
}

/// What to do with an opening quote that has no matching closing quote.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unbalanced {
    /// Leave the lone quote character exactly as it was (the default).
    Keep,
    /// Fail, naming the character position, so a broken file is not half-converted.
    Error,
}

impl Unbalanced {
    /// Parse the option string; blank means the default `keep`.
    pub fn parse(s: &str) -> Result<Unbalanced, String> {
        match s.trim() {
            "" | "keep" => Ok(Unbalanced::Keep),
            "error" => Ok(Unbalanced::Error),
            other => Err(format!(
                "unknown on_unbalanced {other:?} — expected one of: keep, error"
            )),
        }
    }
}

/// Everything the conversion needs beyond the text itself.
#[derive(Clone, Copy, Debug)]
pub struct Options {
    pub direction: Direction,
    pub escape_style: EscapeStyle,
    /// Treat a word-internal `'`/`’` (`don't`, `it’s`) as an apostrophe rather
    /// than a delimiter. Default true.
    pub preserve_apostrophes: bool,
    pub on_unbalanced: Unbalanced,
}

impl Default for Options {
    fn default() -> Options {
        Options {
            direction: Direction::SingleToDouble,
            escape_style: EscapeStyle::Backslash,
            preserve_apostrophes: true,
            on_unbalanced: Unbalanced::Keep,
        }
    }
}

/// Counts describing what the conversion did, for the optional JSON report.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// Quoted runs whose delimiters were converted.
    pub converted: usize,
    /// Inner quote characters escaped (or doubled) to protect the new delimiter.
    pub escaped: usize,
    /// Opening quotes left alone because no closing partner was found.
    pub unbalanced: usize,
}

/// The result of a conversion: the rewritten text plus the counts.
#[derive(Clone, Debug)]
pub struct Converted {
    pub text: String,
    pub report: Report,
}

fn is_word(c: char) -> bool {
    c.is_alphanumeric()
}

fn is_quote_char(c: char) -> bool {
    matches!(c, '\'' | '"' | LSQUO | RSQUO | LDQUO | RDQUO)
}

/// A single quote wedged between two word characters is an apostrophe
/// (`don't`, `it’s`, `O'Hara`), never a delimiter.
fn apostrophe_at(chars: &[char], i: usize) -> bool {
    i > 0 && i + 1 < chars.len() && is_word(chars[i - 1]) && is_word(chars[i + 1])
}

/// Index of the delimiter that closes the run opened at `from - 1`, skipping
/// backslash escape pairs and (optionally) word-internal apostrophes.
fn find_close(chars: &[char], from: usize, kind: QuoteKind, preserve_apostrophes: bool) -> Option<usize> {
    let close = kind.close();
    let mut j = from;
    while j < chars.len() {
        let c = chars[j];
        if c == '\\' && j + 1 < chars.len() {
            j += 2;
            continue;
        }
        if c == close && !(kind.is_single() && preserve_apostrophes && apostrophe_at(chars, j)) {
            return Some(j);
        }
        j += 1;
    }
    None
}

fn push_escaped(out: &mut String, c: char, style: EscapeStyle) {
    match style {
        EscapeStyle::Backslash => {
            out.push('\\');
            out.push(c);
        }
        EscapeStyle::Doubled => {
            out.push(c);
            out.push(c);
        }
        EscapeStyle::Bare => out.push(c),
    }
}

/// Rewrite the body of a run for its new delimiter. Non-quote escape sequences
/// (`\n`, `\t`, `\\`, `\u{1F600}`) pass through untouched; an escaped quote is
/// unescaped unless it collides with the new delimiter; a bare quote that now
/// collides is escaped. Returns how many characters were escaped.
fn rewrite_body(body: &[char], target: QuoteKind, style: EscapeStyle, out: &mut String) -> usize {
    let collides = |c: char| c == target.open() || c == target.close();
    let mut escaped = 0usize;
    let mut i = 0;
    while i < body.len() {
        let c = body[i];
        if c == '\\' && i + 1 < body.len() {
            let x = body[i + 1];
            if is_quote_char(x) {
                // The source escaped this quote; it only needs escaping again
                // if it would terminate the new delimiter.
                if collides(x) {
                    push_escaped(out, x, style);
                    escaped += 1;
                } else {
                    out.push(x);
                }
            } else {
                out.push('\\');
                out.push(x);
            }
            i += 2;
            continue;
        }
        if collides(c) {
            push_escaped(out, c, style);
            escaped += 1;
        } else {
            out.push(c);
        }
        i += 1;
    }
    escaped
}

/// Convert the quote delimiters in `input` per `opts`.
///
/// Returns `Err` when the input exceeds [`MAX_BYTES`], or when
/// `on_unbalanced` is [`Unbalanced::Error`] and an opening quote has no partner.
pub fn convert(input: &str, opts: Options) -> Result<Converted, String> {
    if input.len() > MAX_BYTES {
        return Err(format!(
            "input is {} bytes; the limit is {} bytes ({} KB) — split the text and convert it in parts",
            input.len(),
            MAX_BYTES,
            MAX_BYTES / 1024
        ));
    }

    let chars: Vec<char> = input.chars().collect();
    let sources = opts.direction.sources();
    let mut out = String::with_capacity(input.len() + 16);
    let mut report = Report::default();
    let mut i = 0usize;

    while i < chars.len() {
        let c = chars[i];

        // Outside a run, a backslash escape still hides the next character.
        if c == '\\' && i + 1 < chars.len() {
            out.push(c);
            out.push(chars[i + 1]);
            i += 2;
            continue;
        }

        let opener = sources.iter().copied().find(|k| k.open() == c).filter(|k| {
            !(k.is_single() && opts.preserve_apostrophes && apostrophe_at(&chars, i))
        });
        let Some(kind) = opener else {
            out.push(c);
            i += 1;
            continue;
        };

        match find_close(&chars, i + 1, kind, opts.preserve_apostrophes) {
            Some(j) => {
                let target = opts.direction.target(kind);
                out.push(target.open());
                report.escaped += rewrite_body(&chars[i + 1..j], target, opts.escape_style, &mut out);
                out.push(target.close());
                report.converted += 1;
                i = j + 1;
            }
            None if opts.on_unbalanced == Unbalanced::Error => {
                return Err(format!(
                    "unbalanced quote: the {c} at character {} has no matching closing quote — \
                     set on_unbalanced to \"keep\" to leave lone quotes untouched",
                    i + 1
                ));
            }
            None => {
                report.unbalanced += 1;
                out.push(c);
                i += 1;
            }
        }
    }

    Ok(Converted { text: out, report })
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
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

/// String-in / string-out entry point shared by the chat block, the CLI and the
/// page. Every option arrives as text (that is what the page driver hands over),
/// is validated here, and drives the same [`convert`] core.
pub fn run(
    input: &str,
    direction: &str,
    escape_style: &str,
    preserve_apostrophes: bool,
    on_unbalanced: &str,
    include_report: bool,
) -> Result<String, String> {
    let opts = Options {
        direction: Direction::parse(direction)?,
        escape_style: EscapeStyle::parse(escape_style)?,
        preserve_apostrophes,
        on_unbalanced: Unbalanced::parse(on_unbalanced)?,
    };
    let done = convert(input, opts)?;
    if !include_report {
        return Ok(done.text);
    }
    Ok(format!(
        "{{\n  \"result\": \"{}\",\n  \"converted\": {},\n  \"escaped\": {},\n  \"unbalanced\": {}\n}}",
        json_escape(&done.text),
        done.report.converted,
        done.report.escaped,
        done.report.unbalanced
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn go(input: &str, direction: &str) -> String {
        run(input, direction, "backslash", true, "keep", false).unwrap()
    }

    #[test]
    fn single_to_double_is_the_default() {
        assert_eq!(go("print('hello')", ""), "print(\"hello\")");
        assert_eq!(go("print('hello')", "single-to-double"), "print(\"hello\")");
        // Underscore spelling is accepted too (CLI/chat convenience).
        assert_eq!(go("print('hi')", "single_to_double"), "print(\"hi\")");
    }

    #[test]
    fn double_to_single_escapes_inner_single_quotes() {
        assert_eq!(go("\"it's here\"", "double-to-single"), "'it\\'s here'");
    }

    #[test]
    fn escaped_source_quote_is_unescaped_when_no_longer_needed() {
        assert_eq!(go("\"a \\\" b\"", "double-to-single"), "'a \" b'");
        assert_eq!(go("'a \\' b'", "single-to-double"), "\"a ' b\"");
    }

    #[test]
    fn non_quote_escapes_pass_through_untouched() {
        assert_eq!(go("'line\\nbreak \\\\ tab\\t'", "single-to-double"), "\"line\\nbreak \\\\ tab\\t\"");
    }

    #[test]
    fn apostrophes_are_not_delimiters() {
        assert_eq!(go("It's Sarah's, not O'Hara's.", "single-to-double"), "It's Sarah's, not O'Hara's.");
        assert_eq!(go("'don't stop'", "single-to-double"), "\"don't stop\"");
        // Turned off, the same text reads as delimiters instead.
        assert_eq!(
            run("It's fine's", "single-to-double", "backslash", false, "keep", false).unwrap(),
            "It\"s fine\"s"
        );
    }

    #[test]
    fn smart_quotes_become_straight_delimiters() {
        assert_eq!(go("He said \u{201C}hi\u{201D} and \u{2018}bye\u{2019}.", "smart-to-double"), "He said \"hi\" and \"bye\".");
        assert_eq!(go("\u{201C}quoted\u{201D}", "smart-to-single"), "'quoted'");
        // Curly apostrophes inside a curly run survive.
        assert_eq!(go("\u{201C}it\u{2019}s fine\u{201D}", "smart-to-double"), "\"it\u{2019}s fine\"");
    }

    #[test]
    fn auto_normalizes_every_style_at_once() {
        assert_eq!(
            go("'a' \"b\" \u{201C}c\u{201D} \u{2018}d\u{2019}", "auto-to-double"),
            "\"a\" \"b\" \"c\" \"d\""
        );
        assert_eq!(
            go("'a' \"b\" \u{201C}c\u{201D} \u{2018}d\u{2019}", "auto-to-single"),
            "'a' 'b' 'c' 'd'"
        );
    }

    #[test]
    fn swap_exchanges_both_straight_styles_in_one_pass() {
        assert_eq!(go("a = 'x'; b = \"y\";", "swap"), "a = \"x\"; b = 'y';");
    }

    #[test]
    fn nested_quotes_are_escaped_for_the_new_delimiter() {
        assert_eq!(go("'He said \"hi\"'", "single-to-double"), "\"He said \\\"hi\\\"\"");
    }

    #[test]
    fn escape_styles_change_how_inner_quotes_are_protected() {
        let src = "\"it's here\"";
        assert_eq!(run(src, "double-to-single", "backslash", true, "keep", false).unwrap(), "'it\\'s here'");
        assert_eq!(run(src, "double-to-single", "doubled", true, "keep", false).unwrap(), "'it''s here'");
        assert_eq!(run(src, "double-to-single", "bare", true, "keep", false).unwrap(), "'it's here'");
    }

    #[test]
    fn unbalanced_quote_is_kept_by_default() {
        assert_eq!(go("value = 'oops", "single-to-double"), "value = 'oops");
        assert_eq!(go("'ok' and 'oops", "single-to-double"), "\"ok\" and 'oops");
    }

    #[test]
    fn unbalanced_quote_can_be_an_error() {
        let err = run("value = 'oops", "single-to-double", "backslash", true, "error", false).unwrap_err();
        assert!(err.contains("unbalanced quote"), "{err}");
        assert!(err.contains("character 9"), "{err}");
    }

    #[test]
    fn report_counts_conversions_escapes_and_strays() {
        let out = run("'He said \"hi\"' and 'oops", "single-to-double", "backslash", true, "keep", true).unwrap();
        assert!(out.contains("\"converted\": 1"), "{out}");
        assert!(out.contains("\"escaped\": 2"), "{out}");
        assert!(out.contains("\"unbalanced\": 1"), "{out}");
        assert!(out.contains(r#""result": "\"He said \\\"hi\\\"\" and 'oops""#), "{out}");
    }

    #[test]
    fn unknown_option_values_are_rejected_with_the_valid_list() {
        assert!(run("x", "sideways", "backslash", true, "keep", false)
            .unwrap_err()
            .contains("single-to-double"));
        assert!(run("x", "", "curly", true, "keep", false)
            .unwrap_err()
            .contains("doubled"));
        assert!(run("x", "", "backslash", true, "explode", false)
            .unwrap_err()
            .contains("keep"));
    }

    #[test]
    fn oversized_input_is_rejected_at_the_cap() {
        let at_cap = "a".repeat(MAX_BYTES);
        assert_eq!(go(&at_cap, "single-to-double").len(), MAX_BYTES);
        let over = "a".repeat(MAX_BYTES + 1);
        let err = run(&over, "", "backslash", true, "keep", false).unwrap_err();
        assert!(err.contains("the limit is 1000000 bytes"), "{err}");
    }

    #[test]
    fn text_without_quotes_is_returned_unchanged() {
        assert_eq!(go("", "single-to-double"), "");
        assert_eq!(go("no quotes here — just prose.", "auto-to-double"), "no quotes here — just prose.");
    }

    #[test]
    fn multibyte_text_keeps_its_characters() {
        assert_eq!(go("'café 🎉 日本語'", "single-to-double"), "\"café 🎉 日本語\"");
    }
}
