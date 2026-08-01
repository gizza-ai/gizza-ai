//! field-extractor core — pure compute, shared by the chat skill block and the
//! web page. No wafer/wasm-bindgen deps.
//!
//! A friendly, browser-local `cut`/`awk` replacement: extract specific fields
//! (columns) or character ranges from every line of text.
//!
//! Two modes:
//! - `fields`: split each line on `delimiter` (blank = collapse runs of
//!   whitespace, like awk `$1..$n`) and emit the selected columns.
//! - `chars`: treat each line as a sequence of Unicode code points and emit the
//!   selected character positions (like `cut -c`), never splitting a code point.
//!
//! Selectors are 1-based and comma-separated. Each term is one of:
//! - a single positive index (`1`, `3`) counted from the start,
//! - a single negative index (`-1` = last, `-2` = second-to-last),
//! - a range `A-B` (`2-4`), which may run either direction (`4-2` reverses),
//! - an open-ended range `A-` (`3-` = field 3 to the end).
//! Endpoints may be negative (`-3--1`, `-2-`).
//!
//! Behaviour on missing columns matches `cut`: an explicitly numbered single
//! field that is out of range emits an empty string; a range simply stops at the
//! last available field. Selectors emit in the order given, so `3,1,2` reorders.

/// Resolve a delimiter/output-delimiter spec: honour keyword names and common
/// backslash escapes so `\t`, `tab`, `newline`, `|`, etc. all work.
fn resolve_delim(spec: &str) -> String {
    match spec.trim().to_ascii_lowercase().as_str() {
        "tab" => return "\t".to_string(),
        "newline" | "nl" => return "\n".to_string(),
        "space" => return " ".to_string(),
        "comma" => return ",".to_string(),
        "pipe" => return "|".to_string(),
        "semicolon" => return ";".to_string(),
        "colon" => return ":".to_string(),
        _ => {}
    }
    unescape(spec)
}

/// Turn textual escapes (`\t`, `\n`, `\r`, `\0`, `\\`) into the real bytes.
/// Unknown escapes are left verbatim (`\x` stays `\x`).
fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('t') => out.push('\t'),
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some('0') => out.push('\0'),
                Some('\\') => out.push('\\'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Parse a signed, non-zero, 1-based index. Rejects `0` and non-numeric text.
fn parse_index(tok: &str) -> Result<isize, String> {
    let t = tok.trim();
    let ok = {
        let body = t.strip_prefix('-').unwrap_or(t);
        !body.is_empty() && body.bytes().all(|b| b.is_ascii_digit())
    };
    if !ok {
        return Err(format!(
            "invalid selector '{tok}': use numbers like 1, -1, 2-4, or 3-"
        ));
    }
    let n: isize = t
        .parse()
        .map_err(|_| format!("invalid selector '{tok}': number is too large"))?;
    if n == 0 {
        return Err(
            "selector index cannot be 0; fields and characters are numbered from 1".to_string(),
        );
    }
    Ok(n)
}

/// Map a 1-based signed index to a 0-based position against a record of `n`
/// units. Positive `p` → `p-1`; negative `-q` → `n-q`. May fall outside `0..n`.
fn to_pos(idx: isize, n: usize) -> isize {
    if idx > 0 {
        idx - 1
    } else {
        n as isize + idx
    }
}

/// A resolved pick: `Some(i)` selects unit `i`; `None` emits an empty string
/// (an explicitly numbered single field that is out of range, matching `cut`).
type Pick = Option<usize>;

/// Turn a selector spec into an ordered list of picks against an `n`-unit line.
fn resolve_selectors(spec: &str, n: usize) -> Result<Vec<Pick>, String> {
    if spec.trim().is_empty() {
        return Err(
            "provide at least one selector, e.g. \"1\", \"1,3\", \"2-4\", or \"-1\"".to_string(),
        );
    }
    let mut picks: Vec<Pick> = Vec::new();
    for raw in spec.split(',') {
        let tok = raw.trim();
        if tok.is_empty() {
            return Err(
                "empty selector between commas; write selectors like \"1,3-5,-1\"".to_string(),
            );
        }
        match find_range_split(tok) {
            Some((left, right)) => {
                let a = parse_index(left)?;
                let a_pos = to_pos(a, n);
                let b_pos = if right.is_empty() {
                    n as isize - 1
                } else {
                    to_pos(parse_index(right)?, n)
                };
                // Clamp the range to the record's valid window so a huge
                // endpoint (e.g. `999999999-`) costs O(n), not O(endpoint).
                let ascending = a_pos <= b_pos;
                let lo = a_pos.min(b_pos).max(0);
                let hi = a_pos.max(b_pos).min(n as isize - 1);
                if lo <= hi {
                    if ascending {
                        for pos in lo..=hi {
                            picks.push(Some(pos as usize));
                        }
                    } else {
                        for pos in (lo..=hi).rev() {
                            picks.push(Some(pos as usize));
                        }
                    }
                }
            }
            None => {
                let idx = parse_index(tok)?;
                let pos = to_pos(idx, n);
                if pos >= 0 && (pos as usize) < n {
                    picks.push(Some(pos as usize));
                } else {
                    picks.push(None);
                }
            }
        }
    }
    Ok(picks)
}

/// If `tok` is a range (`A-B` or open-ended `A-`), return the `(left, right)`
/// endpoint strings; otherwise `None` (a plain single index). The separator is
/// the first `-` at position > 0 whose left side parses as a signed integer and
/// whose right side is empty or a signed integer — this keeps leading `-` (a
/// negative start) distinct from the range separator.
fn find_range_split(tok: &str) -> Option<(&str, &str)> {
    let bytes = tok.as_bytes();
    for i in 1..bytes.len() {
        if bytes[i] != b'-' {
            continue;
        }
        let left = &tok[..i];
        let right = &tok[i + 1..];
        let left_ok = is_signed_int(left);
        let right_ok = right.is_empty() || is_signed_int(right);
        if left_ok && right_ok {
            return Some((left, right));
        }
    }
    None
}

fn is_signed_int(s: &str) -> bool {
    let body = s.strip_prefix('-').unwrap_or(s);
    !body.is_empty() && body.bytes().all(|b| b.is_ascii_digit())
}

/// Extract fields or characters from each line of `text`.
///
/// - `mode`: `"fields"` (default) or `"chars"`.
/// - `selectors`: 1-based selector spec (`"1,3-5,-1"`).
/// - `delimiter`: field separator; blank collapses runs of whitespace. Ignored
///   in `chars` mode. Honours `\t`/`\n` escapes and keyword names.
/// - `output_delimiter`: blank = same as the input delimiter in `fields` mode
///   (a single space when whitespace-splitting), or concatenate in `chars` mode.
/// - `trim`: trim whitespace around each emitted field (`fields` mode).
/// - `skip_empty_lines`: drop blank/whitespace-only lines.
/// - `skip_header`: drop the first line of input.
#[allow(clippy::too_many_arguments)]
pub fn extract(
    text: &str,
    mode: &str,
    selectors: &str,
    delimiter: &str,
    output_delimiter: &str,
    trim: bool,
    skip_empty_lines: bool,
    skip_header: bool,
) -> Result<String, String> {
    let chars_mode = match mode.trim().to_ascii_lowercase().as_str() {
        "" | "fields" | "field" => false,
        "chars" | "char" | "characters" => true,
        other => {
            return Err(format!("mode must be 'fields' or 'chars', got '{other}'"));
        }
    };

    // Validate the selector spec once up front so a bad spec is a clear error
    // rather than silently producing empty output. `n = 0` is a safe probe: it
    // only checks the grammar, not the record widths.
    resolve_selectors(selectors, 0)?;

    // Input delimiter (fields mode).
    let ws_split = delimiter.is_empty() || resolve_delim(delimiter).is_empty();
    let in_delim = if ws_split {
        " ".to_string()
    } else {
        resolve_delim(delimiter)
    };

    // Output delimiter.
    let out_delim = if output_delimiter.is_empty() {
        if chars_mode {
            String::new()
        } else {
            in_delim.clone()
        }
    } else {
        resolve_delim(output_delimiter)
    };

    let mut lines: Vec<&str> = text.split('\n').collect();
    // A trailing newline yields a final empty element; drop it so "a\n" is one
    // record, not two. Genuine trailing blank lines are still present when the
    // text does not end in a newline.
    if text.ends_with('\n') {
        lines.pop();
    }

    let mut iter = lines.into_iter();
    if skip_header {
        iter.next();
    }

    let mut out_lines: Vec<String> = Vec::new();
    for raw in iter {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        if skip_empty_lines && line.trim().is_empty() {
            continue;
        }

        // Split the line into units (fields or characters).
        let units: Vec<String> = if chars_mode {
            line.chars().map(|c| c.to_string()).collect()
        } else if ws_split {
            line.split_whitespace().map(|s| s.to_string()).collect()
        } else {
            line.split(in_delim.as_str()).map(|s| s.to_string()).collect()
        };

        let picks = resolve_selectors(selectors, units.len())?;
        let pieces: Vec<String> = picks
            .into_iter()
            .map(|p| match p {
                Some(i) => {
                    let u = &units[i];
                    if trim && !chars_mode {
                        u.trim().to_string()
                    } else {
                        u.clone()
                    }
                }
                None => String::new(),
            })
            .collect();

        out_lines.push(pieces.join(&out_delim));
    }

    Ok(out_lines.join("\n"))
}

/// Convenience wrapper with default options, kept for the simplest callers.
pub fn run(text: &str) -> Result<String, String> {
    extract(text, "fields", "1", "", "", false, false, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ex(text: &str, mode: &str, sel: &str, delim: &str, out: &str) -> String {
        extract(text, mode, sel, delim, out, false, false, false).unwrap()
    }

    #[test]
    fn whitespace_default_single_field() {
        assert_eq!(ex("alpha beta gamma", "fields", "1", "", ""), "alpha");
        assert_eq!(ex("alpha beta gamma", "fields", "2", "", ""), "beta");
    }

    #[test]
    fn whitespace_collapses_runs() {
        assert_eq!(ex("a    b\tc", "fields", "1,2,3", "", ""), "a b c");
    }

    #[test]
    fn multiple_fields_keep_input_delimiter() {
        assert_eq!(ex("a,b,c,d", "fields", "1,3", ",", ""), "a,c");
    }

    #[test]
    fn negative_indices() {
        assert_eq!(ex("a,b,c,d", "fields", "-1", ",", ""), "d");
        assert_eq!(ex("a,b,c,d", "fields", "-2", ",", ""), "c");
        assert_eq!(ex("a,b,c,d", "fields", "1,-1", ",", ""), "a,d");
    }

    #[test]
    fn reorder_fields() {
        assert_eq!(ex("a,b,c", "fields", "3,1,2", ",", ""), "c,a,b");
    }

    #[test]
    fn closed_range() {
        assert_eq!(ex("a,b,c,d,e", "fields", "2-4", ",", ""), "b,c,d");
    }

    #[test]
    fn open_ended_range() {
        assert_eq!(ex("a,b,c,d,e", "fields", "3-", ",", ""), "c,d,e");
    }

    #[test]
    fn descending_range_reverses() {
        assert_eq!(ex("a,b,c,d", "fields", "4-2", ",", ""), "d,c,b");
    }

    #[test]
    fn negative_range_endpoints() {
        assert_eq!(ex("a,b,c,d,e", "fields", "-3--1", ",", ""), "c,d,e");
        assert_eq!(ex("a,b,c,d,e", "fields", "-2-", ",", ""), "d,e");
    }

    #[test]
    fn combined_selectors() {
        assert_eq!(ex("a,b,c,d,e,f,g", "fields", "1,3-5,7", ",", ""), "a,c,d,e,g");
    }

    #[test]
    fn range_stops_at_last_field() {
        // A range past the end simply stops; it does not emit empties.
        assert_eq!(ex("a,b,c", "fields", "2-9", ",", ""), "b,c");
    }

    #[test]
    fn explicit_out_of_range_field_is_empty() {
        // An explicitly numbered single field out of range emits "" (like cut).
        assert_eq!(ex("a,b,c", "fields", "5", ",", ""), "");
        assert_eq!(ex("a,b,c", "fields", "1,5", ",", ""), "a,");
    }

    #[test]
    fn output_delimiter_override() {
        assert_eq!(
            extract("a,b,c,d", "fields", "1,2", ",", " | ", false, false, false).unwrap(),
            "a | b"
        );
    }

    #[test]
    fn output_delimiter_newline_keyword_and_escape() {
        assert_eq!(
            extract("a,b,c", "fields", "1,2,3", ",", "newline", false, false, false).unwrap(),
            "a\nb\nc"
        );
        assert_eq!(
            extract("a,b,c", "fields", "1,2,3", ",", "\\t", false, false, false).unwrap(),
            "a\tb\tc"
        );
    }

    #[test]
    fn tab_delimiter_via_escape_and_keyword() {
        assert_eq!(ex("a\tb\tc", "fields", "2", "\\t", ""), "b");
        assert_eq!(ex("a\tb\tc", "fields", "2", "tab", ""), "b");
    }

    #[test]
    fn multi_char_delimiter() {
        assert_eq!(ex("a::b::c", "fields", "2,3", "::", ""), "b::c");
    }

    #[test]
    fn trim_fields() {
        assert_eq!(
            extract("a , b , c", "fields", "1,2", ",", "", true, false, false).unwrap(),
            "a,b"
        );
        // Without trim the surrounding spaces stay.
        assert_eq!(ex("a , b , c", "fields", "1,2", ",", ""), "a , b ");
    }

    #[test]
    fn multi_line_input() {
        let input = "a,b,c\nd,e,f\ng,h,i";
        assert_eq!(ex(input, "fields", "1,3", ",", ""), "a,c\nd,f\ng,i");
    }

    #[test]
    fn trailing_newline_is_one_record() {
        assert_eq!(ex("a,b\n", "fields", "2", ",", ""), "b");
    }

    #[test]
    fn skip_empty_lines_option() {
        let input = "a,b\n\n  \nc,d";
        assert_eq!(
            extract(input, "fields", "1", ",", "", false, true, false).unwrap(),
            "a\nc"
        );
    }

    #[test]
    fn skip_header_option() {
        let input = "id,name\n1,Ada\n2,Alan";
        assert_eq!(
            extract(input, "fields", "2", ",", "", false, false, true).unwrap(),
            "Ada\nAlan"
        );
    }

    #[test]
    fn skip_header_then_skip_empty() {
        let input = "hdr\n\na\nb";
        assert_eq!(
            extract(input, "fields", "1", "", "", false, true, true).unwrap(),
            "a\nb"
        );
    }

    #[test]
    fn chars_mode_basic() {
        assert_eq!(ex("abcdef", "chars", "1-3", "", ""), "abc");
        assert_eq!(ex("abcdef", "chars", "-1", "", ""), "f");
        assert_eq!(ex("abcdef", "chars", "2,4,6", "", ""), "bdf");
    }

    #[test]
    fn chars_mode_open_range() {
        assert_eq!(ex("abcdef", "chars", "4-", "", ""), "def");
    }

    #[test]
    fn chars_mode_unicode_safe() {
        // Multi-byte code points must not be split; count by code point.
        assert_eq!(ex("héllo", "chars", "1-2", "", ""), "hé");
        assert_eq!(ex("héllo", "chars", "2", "", ""), "é");
        assert_eq!(ex("a😀b😀c", "chars", "1,2,3", "", ""), "a😀b");
        assert_eq!(ex("a😀b😀c", "chars", "-1", "", ""), "c");
    }

    #[test]
    fn chars_mode_output_delimiter() {
        assert_eq!(
            extract("abcdef", "chars", "1,3,5", "", "-", false, false, false).unwrap(),
            "a-c-e"
        );
    }

    #[test]
    fn chars_mode_out_of_range_is_empty() {
        assert_eq!(ex("abc", "chars", "9", "", ""), "");
    }

    #[test]
    fn error_empty_selectors() {
        let e = extract("a,b", "fields", "", ",", "", false, false, false).unwrap_err();
        assert!(e.contains("at least one selector"), "{e}");
    }

    #[test]
    fn error_zero_index() {
        let e = extract("a,b", "fields", "0", ",", "", false, false, false).unwrap_err();
        assert!(e.contains("cannot be 0"), "{e}");
    }

    #[test]
    fn error_bad_token() {
        let e = extract("a,b", "fields", "x", ",", "", false, false, false).unwrap_err();
        assert!(e.contains("invalid selector 'x'"), "{e}");
    }

    #[test]
    fn error_ambiguous_range() {
        let e = extract("a,b", "fields", "1-3-5", ",", "", false, false, false).unwrap_err();
        assert!(e.contains("invalid selector"), "{e}");
    }

    #[test]
    fn error_empty_between_commas() {
        let e = extract("a,b", "fields", "1,,2", ",", "", false, false, false).unwrap_err();
        assert!(e.contains("empty selector"), "{e}");
    }

    #[test]
    fn error_bad_mode() {
        let e = extract("a,b", "bogus", "1", ",", "", false, false, false).unwrap_err();
        assert!(e.contains("mode must be"), "{e}");
    }

    #[test]
    fn empty_input_is_empty_output() {
        assert_eq!(ex("", "fields", "1", ",", ""), "");
    }

    #[test]
    fn consecutive_delimiters_make_empty_fields() {
        assert_eq!(ex("a,,c", "fields", "1,2,3", ",", ""), "a,,c");
        assert_eq!(ex("a,,c", "fields", "2", ",", ""), "");
    }
}
