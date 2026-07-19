//! rtf-to-text core — convert an RTF document to plain Unicode text.
//! Pure compute, no deps; shared by the chat skill block and the web page.
//!
//! RTF (Rich Text Format) is a plain-ASCII markup: text interleaved with
//! **control words** (`\word`, optional signed integer, optional trailing
//! space), **control symbols** (`\` + one non-letter, e.g. `\\`, `\{`, `\~`),
//! and **groups** delimited by `{` … `}`. This is a hand-rolled tokenizer (no
//! regex / crate) that walks the source and keeps only the visible text:
//!
//! - Formatting control words (`\b`, `\f0`, `\fs24`, …) are dropped.
//! - Structural control words become their text: `\par`/`\line`/`\sect`/`\page`
//!   → newline, `\tab` → tab, `\cell` → tab, `\row` → newline; typographic ones
//!   (`\emdash`, `\bullet`, smart quotes, `\~`, `\_`, `\-`) → the right Unicode.
//! - `\'hh` hex escapes are decoded as **Windows-1252** (the RTF `\ansi`
//!   default), including the 0x80–0x9F range (€, smart quotes, en/em dash, …).
//! - `\uN` Unicode escapes decode the (possibly negative) code point and skip
//!   the following `\ucN` ANSI-fallback characters (default 1).
//! - Non-text destinations — font/color tables, stylesheet, `\info`, `\pict`,
//!   `\*` ignorable groups, … — are skipped whole.

/// Convert RTF markup to plain text.
///
/// `line_breaks` selects how paragraph/line breaks render:
/// - `""` | `"preserve"` (default): `\par`/`\line`/`\cell`/… become newlines or
///   tabs, so the paragraph structure survives.
/// - `"collapse"`: every run of whitespace (including those breaks) collapses to
///   a single space, producing one flat line — handy for search or feeding an LLM.
///
/// Returns `Err` if the input is not an RTF document (must begin with `{\rtf`)
/// or on an unknown `line_breaks` value. The output is always valid UTF-8.
pub fn rtf_to_text(rtf: &str, line_breaks: &str) -> Result<String, String> {
    let collapse = match line_breaks {
        "" | "preserve" => false,
        "collapse" => true,
        other => {
            return Err(format!(
                "invalid line_breaks {other:?}: expected \"preserve\" or \"collapse\""
            ))
        }
    };

    if !rtf.trim_start().starts_with("{\\rtf") {
        return Err(
            "not an RTF document: expected the source to begin with \"{\\rtf\"".to_string(),
        );
    }

    let chars: Vec<char> = rtf.chars().collect();
    let n = chars.len();
    let mut i = chars.iter().position(|c| !c.is_whitespace()).unwrap_or(0);
    let mut out = String::with_capacity(rtf.len());

    // Group-scoped state saved/restored on `{`/`}`.
    let mut stack: Vec<(i32, bool)> = Vec::new();
    let mut ucskip: i32 = 1; // chars to skip after a \uN (from \ucN, default 1)
    let mut ignorable = false; // inside a skipped destination group
    let mut curskip: i32 = 0; // remaining \uN fallback chars to skip

    while i < n {
        let c = chars[i];
        match c {
            '{' => {
                stack.push((ucskip, ignorable));
                i += 1;
            }
            '}' => {
                if let Some((u, ig)) = stack.pop() {
                    ucskip = u;
                    ignorable = ig;
                }
                i += 1;
            }
            '\\' => {
                let Some(&next) = chars.get(i + 1) else {
                    // A trailing lone backslash is not a valid escape; drop it.
                    i += 1;
                    continue;
                };
                if next.is_ascii_alphabetic() {
                    // Control word: letters, then an optional signed integer,
                    // then an optional single trailing space (the delimiter).
                    let word_start = i + 1;
                    let mut j = word_start;
                    while j < n && chars[j].is_ascii_alphabetic() {
                        j += 1;
                    }
                    let word: String = chars[word_start..j].iter().collect();
                    let mut arg: Option<i64> = None;
                    let sign_start = j;
                    let mut k = j;
                    if k < n && chars[k] == '-' {
                        k += 1;
                    }
                    let digit_start = k;
                    while k < n && chars[k].is_ascii_digit() {
                        k += 1;
                    }
                    if k > digit_start {
                        let s: String = chars[sign_start..k].iter().collect();
                        arg = s.parse::<i64>().ok();
                        j = k;
                    }
                    if j < n && chars[j] == ' ' {
                        j += 1; // consume the single delimiter space
                    }
                    i = j;
                    handle_word(
                        &word,
                        arg,
                        &mut out,
                        &mut ucskip,
                        &mut ignorable,
                        &mut curskip,
                        collapse,
                    );
                } else if next == '\'' {
                    // \'hh — a hex byte in the current (Windows-1252) code page.
                    let h1 = chars.get(i + 2).copied();
                    let h2 = chars.get(i + 3).copied();
                    i += 4.min(n - i); // advance past \'hh (or to EOF if truncated)
                    let byte = match (h1.and_then(hex_val), h2.and_then(hex_val)) {
                        (Some(a), Some(b)) => Some((a * 16 + b) as u8),
                        _ => None,
                    };
                    if curskip > 0 {
                        curskip -= 1;
                    } else if !ignorable {
                        if let Some(b) = byte {
                            out.push(cp1252_decode(b));
                        }
                    }
                } else {
                    // Control symbol: backslash + one non-letter.
                    i += 2;
                    handle_symbol(next, &mut out, &mut ignorable, &mut curskip, collapse);
                }
            }
            // Raw CR/LF in the source are RTF whitespace, not text.
            '\r' | '\n' => {
                i += 1;
            }
            _ => {
                if curskip > 0 {
                    curskip -= 1;
                } else if !ignorable {
                    out.push(c);
                }
                i += 1;
            }
        }
    }

    Ok(finalize(&out, collapse))
}

/// Control words that introduce a non-text destination whose content should be
/// dropped wholesale (font/color tables, metadata, pictures, list/rev tables, …).
fn is_destination(word: &str) -> bool {
    matches!(
        word,
        "fonttbl"
            | "colortbl"
            | "stylesheet"
            | "listtable"
            | "listoverridetable"
            | "revtbl"
            | "rsidtbl"
            | "info"
            | "author"
            | "operator"
            | "company"
            | "manager"
            | "title"
            | "subject"
            | "keywords"
            | "comment"
            | "doccomm"
            | "generator"
            | "creatim"
            | "revtim"
            | "printim"
            | "buptim"
            | "pict"
            | "shppict"
            | "nonshppict"
            | "object"
            | "objdata"
            | "themedata"
            | "colorschememapping"
            | "datastore"
            | "latentstyles"
            | "filetbl"
            | "fldinst"
            | "bkmkstart"
            | "bkmkend"
            | "header"
            | "headerl"
            | "headerr"
            | "headerf"
            | "footer"
            | "footerl"
            | "footerr"
            | "footerf"
            | "footnote"
            | "annotation"
            | "atnid"
            | "atnauthor"
            | "xmlnstbl"
            | "panose"
            | "falt"
            | "pgptbl"
            | "password"
            | "passwordhash"
    )
}

#[allow(clippy::too_many_arguments)]
fn handle_word(
    word: &str,
    arg: Option<i64>,
    out: &mut String,
    ucskip: &mut i32,
    ignorable: &mut bool,
    curskip: &mut i32,
    collapse: bool,
) {
    // While skipping a \uN fallback, a control word counts as one skipped unit.
    if *curskip > 0 {
        *curskip -= 1;
        return;
    }
    match word {
        // \ucN sets how many fallback chars follow each \uN.
        "uc" => {
            *ucskip = arg.unwrap_or(1).max(0) as i32;
            return;
        }
        // \uN — a Unicode code point (signed 16-bit), then \ucN fallback chars.
        "u" => {
            if let Some(a) = arg {
                let cp = if a < 0 { a + 0x10000 } else { a };
                if !*ignorable {
                    if let Some(ch) = u32::try_from(cp).ok().and_then(char::from_u32) {
                        out.push(ch);
                    }
                }
                *curskip = *ucskip;
            }
            return;
        }
        _ => {}
    }
    if *ignorable {
        return;
    }
    match word {
        "par" | "sect" | "page" | "line" | "softline" => push_break(out, collapse),
        "tab" => out.push('\t'),
        "cell" | "nestcell" => out.push('\t'),
        "row" | "nestrow" => push_break(out, collapse),
        "emdash" => out.push('\u{2014}'),
        "endash" => out.push('\u{2013}'),
        "bullet" => out.push_str("\u{2022} "),
        "lquote" => out.push('\u{2018}'),
        "rquote" => out.push('\u{2019}'),
        "ldblquote" => out.push('\u{201C}'),
        "rdblquote" => out.push('\u{201D}'),
        "emspace" => out.push('\u{2003}'),
        "enspace" => out.push('\u{2002}'),
        "qmspace" => out.push('\u{2005}'),
        _ => {
            // Any other control word is either a non-text destination (skip its
            // group) or a formatting word (no output).
            if is_destination(word) {
                *ignorable = true;
            }
        }
    }
}

fn handle_symbol(sym: char, out: &mut String, ignorable: &mut bool, curskip: &mut i32, collapse: bool) {
    // `\*` marks the current group as an ignorable destination — do this even
    // mid-skip so `{\*\generator …}` is dropped whole.
    if sym == '*' {
        *ignorable = true;
        return;
    }
    if *curskip > 0 {
        *curskip -= 1;
        return;
    }
    if *ignorable {
        return;
    }
    match sym {
        '\\' | '{' | '}' => out.push(sym),
        '~' => out.push('\u{00A0}'), // non-breaking space
        '_' => out.push('-'),        // non-breaking hyphen → plain hyphen
        '-' => {}                    // optional hyphen → dropped
        '\n' | '\r' => push_break(out, collapse), // `\<newline>` == \par
        // Any other escaped punctuation (`\;`, …) → the literal character.
        other => out.push(other),
    }
}

/// A paragraph/line break: a real newline in preserve mode, a space in collapse
/// mode (finalize dedupes the spaces).
fn push_break(out: &mut String, collapse: bool) {
    out.push(if collapse { ' ' } else { '\n' });
}

/// Decode a Windows-1252 byte to its Unicode character. 0x00–0x7F and 0xA0–0xFF
/// map to the same code point (ASCII / Latin-1); 0x80–0x9F use the CP1252 table.
fn cp1252_decode(b: u8) -> char {
    const HIGH: [char; 32] = [
        '\u{20AC}', '\u{0081}', '\u{201A}', '\u{0192}', '\u{201E}', '\u{2026}', '\u{2020}',
        '\u{2021}', '\u{02C6}', '\u{2030}', '\u{0160}', '\u{2039}', '\u{0152}', '\u{008D}',
        '\u{017D}', '\u{008F}', '\u{0090}', '\u{2018}', '\u{2019}', '\u{201C}', '\u{201D}',
        '\u{2022}', '\u{2013}', '\u{2014}', '\u{02DC}', '\u{2122}', '\u{0161}', '\u{203A}',
        '\u{0153}', '\u{009D}', '\u{017E}', '\u{0178}',
    ];
    if (0x80..=0x9F).contains(&b) {
        HIGH[(b - 0x80) as usize]
    } else {
        b as char
    }
}

fn hex_val(c: char) -> Option<u16> {
    c.to_digit(16).map(|d| d as u16)
}

/// Tidy the raw extracted text. Collapse mode → one space-separated line.
/// Preserve mode → right-trim each line, collapse 3+ blank lines to one blank
/// line, and strip leading/trailing blank lines.
fn finalize(s: &str, collapse: bool) -> String {
    if collapse {
        let mut result = String::with_capacity(s.len());
        let mut prev_space = false;
        for ch in s.chars() {
            if ch.is_whitespace() {
                if !prev_space {
                    result.push(' ');
                    prev_space = true;
                }
            } else {
                result.push(ch);
                prev_space = false;
            }
        }
        return result.trim().to_string();
    }

    let mut lines: Vec<String> = Vec::new();
    let mut blank_run = 0usize;
    for line in s.split('\n') {
        let trimmed = line.trim_end();
        if trimmed.is_empty() {
            blank_run += 1;
            if blank_run <= 1 {
                lines.push(String::new());
            }
        } else {
            blank_run = 0;
            lines.push(trimmed.to_string());
        }
    }
    // Drop leading/trailing blank lines.
    while lines.first().is_some_and(|l| l.is_empty()) {
        lines.remove(0);
    }
    while lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_basic_document() {
        let rtf = "{\\rtf1\\ansi\\deff0{\\fonttbl{\\f0 Arial;}}\\pard Hello, \\b world\\b0  fox!\\par}";
        // Note: the single space after \b0 is the control-word delimiter (eaten);
        // the second space survives, so "world fox!".
        assert_eq!(rtf_to_text(rtf, "preserve").unwrap(), "Hello, world fox!");
    }

    #[test]
    fn paragraphs_become_newlines() {
        let rtf = "{\\rtf1\\ansi First line.\\par Second line.\\par}";
        assert_eq!(
            rtf_to_text(rtf, "preserve").unwrap(),
            "First line.\nSecond line."
        );
    }

    #[test]
    fn collapse_flattens_to_one_line() {
        let rtf = "{\\rtf1\\ansi First line.\\par Second line.\\par}";
        assert_eq!(
            rtf_to_text(rtf, "collapse").unwrap(),
            "First line. Second line."
        );
    }

    #[test]
    fn decodes_hex_escapes_cp1252() {
        // \'e9 = é (Latin-1), \'80 = € (CP1252 0x80), \'a0 = non-breaking space.
        let rtf = "{\\rtf1\\ansi Caf\\'e9 costs 5\\'a0\\'80}";
        assert_eq!(rtf_to_text(rtf, "preserve").unwrap(), "Café costs 5\u{a0}€");
    }

    #[test]
    fn decodes_unicode_escape_with_fallback_skip() {
        // 霱 = ☃ (U+2603), followed by one '?' ANSI fallback char to skip.
        let rtf = "{\\rtf1\\ansi Snow \\u9731?man}";
        assert_eq!(rtf_to_text(rtf, "preserve").unwrap(), "Snow ☃man");
    }

    #[test]
    fn positive_unicode_escape_euro() {
        let rtf = "{\\rtf1\\ansi Price \\u8364?100}";
        assert_eq!(rtf_to_text(rtf, "preserve").unwrap(), "Price €100");
    }

    #[test]
    fn negative_unicode_escape() {
        // RTF stores \uN as a signed 16-bit value; negatives wrap by +65536.
        // \u-3 → 65533 = U+FFFD.
        let rtf = "{\\rtf1\\ansi x\\u-3?y}";
        assert_eq!(rtf_to_text(rtf, "preserve").unwrap(), "x\u{FFFD}y");
    }

    #[test]
    fn respects_uc_skip_count() {
        // \uc2 → skip 2 fallback units after each \uN.
        let rtf = "{\\rtf1\\ansi\\uc2 \\u9731?\\'3fend}";
        assert_eq!(rtf_to_text(rtf, "preserve").unwrap(), "☃end");
    }

    #[test]
    fn skips_font_and_color_tables() {
        let rtf = "{\\rtf1{\\fonttbl{\\f0 Times New Roman;}}{\\colortbl;\\red255\\green0\\blue0;}Body}";
        assert_eq!(rtf_to_text(rtf, "preserve").unwrap(), "Body");
    }

    #[test]
    fn skips_ignorable_star_destination() {
        let rtf = "{\\rtf1{\\*\\generator Riched20 10.0;}Visible}";
        assert_eq!(rtf_to_text(rtf, "preserve").unwrap(), "Visible");
    }

    #[test]
    fn control_symbols_and_escaped_braces() {
        let rtf = "{\\rtf1 a\\~b\\-c\\_d \\{x\\} \\\\z}";
        assert_eq!(rtf_to_text(rtf, "preserve").unwrap(), "a\u{a0}bc-d {x} \\z");
    }

    #[test]
    fn typographic_control_words() {
        let rtf = "{\\rtf1 yes\\emdash no \\bullet item \\ldblquote hi\\rdblquote}";
        assert_eq!(
            rtf_to_text(rtf, "preserve").unwrap(),
            "yes\u{2014}no \u{2022} item \u{201C}hi\u{201D}"
        );
    }

    #[test]
    fn table_cells_and_rows() {
        let rtf = "{\\rtf1 A\\cell B\\cell\\row C\\cell\\row}";
        // Trailing cell-tabs before each row break are trimmed by finalize.
        assert_eq!(rtf_to_text(rtf, "preserve").unwrap(), "A\tB\nC");
    }

    #[test]
    fn preserves_unicode_body_text() {
        let rtf = "{\\rtf1\\ansi café ☕ — плайн\\par}";
        assert_eq!(rtf_to_text(rtf, "preserve").unwrap(), "café ☕ — плайн");
    }

    #[test]
    fn rejects_non_rtf_input() {
        let err = rtf_to_text("just plain text", "preserve").unwrap_err();
        assert!(err.contains("not an RTF document"), "got: {err}");
    }

    #[test]
    fn rejects_unknown_line_breaks() {
        let err = rtf_to_text("{\\rtf1 x}", "wrap").unwrap_err();
        assert!(err.contains("invalid line_breaks"), "got: {err}");
    }

    #[test]
    fn handles_truncated_and_empty() {
        // Unterminated groups / trailing backslash must not panic.
        assert_eq!(rtf_to_text("{\\rtf1 hi\\", "preserve").unwrap(), "hi");
        assert_eq!(rtf_to_text("{\\rtf1}", "preserve").unwrap(), "");
        assert_eq!(rtf_to_text("{\\rtf1 a\\'g}", "preserve").unwrap(), "a");
    }

    #[test]
    fn leading_whitespace_before_rtf_ok() {
        assert_eq!(rtf_to_text("  \n{\\rtf1 hi}", "preserve").unwrap(), "hi");
    }
}
