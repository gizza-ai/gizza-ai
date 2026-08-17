//! gizza-ai/csv-quote-normalizer core — pure compute, shared by the chat skill
//! block and the web page. No wafer/wasm-bindgen deps.
//!
//! Reads a CSV with a deliberately TOLERANT parser (the kind of file a strict
//! reader rejects: backslash-escaped quotes, single-quoted fields, curly “smart”
//! quotes, padding before an opening quote, stray quotes inside a value, an
//! unclosed quote at EOF) and re-emits it with ONE consistent dialect: a chosen
//! quoting policy, a chosen escape convention, a chosen quote character, a chosen
//! delimiter and a chosen line ending.
//!
//! Deliberately NOT in scope (other blocks own these): trimming cell whitespace
//! (csv-whitespace-normalizer), reporting problems without fixing them
//! (csv-structure-validator), padding ragged rows, renaming headers.

use std::collections::BTreeMap;

/// Hard input cap — keeps a pasted file from wedging the browser tab.
pub const MAX_INPUT_BYTES: usize = 5_000_000;

const SMART_DOUBLE: [char; 2] = ['\u{201C}', '\u{201D}'];
const SMART_SINGLE: [char; 2] = ['\u{2018}', '\u{2019}'];

/// A repair the tolerant parser had to make, for the `report` output.
type Repair = (&'static str, usize);

const R_BOM: &str = "byte-order mark (BOM) removed";
const R_BLANK: &str = "blank line removed";
const R_BACKSLASH: &str = "backslash-escaped quote (\\\") rewritten with the chosen escaping";
const R_BACKSLASH_BS: &str = "backslash-escaped backslash (\\\\) unescaped";
const R_PAD_OPEN: &str = "padding before an opening quote removed";
const R_PAD_CLOSE: &str = "padding after a closing quote removed";
const R_STRAY_IN: &str = "stray quote inside a quoted field kept as literal text";
const R_BARE: &str = "bare quote inside an unquoted field escaped on output";
const R_JUNK: &str = "text after a closing quote merged back into the field";
const R_UNCLOSED: &str = "unclosed quote closed at end of input";
const R_SMART: &str = "curly quote (\u{201C} \u{201D} \u{2018} \u{2019}) read as a field quote";

/// Is `c` the active quote character (or, with `smart`, its curly variant)?
fn is_quote(c: char, qc: char, smart: bool) -> bool {
    if c == qc {
        return true;
    }
    if !smart {
        return false;
    }
    match qc {
        '"' => SMART_DOUBLE.contains(&c),
        '\'' => SMART_SINGLE.contains(&c),
        _ => false,
    }
}

/// Resolve a delimiter spec to one character.
fn resolve_delim(spec: &str, which: &str) -> Result<char, String> {
    match spec {
        "," | "comma" => Ok(','),
        "\t" | "tab" | "\\t" => Ok('\t'),
        ";" | "semicolon" => Ok(';'),
        "|" | "pipe" => Ok('|'),
        " " | "space" => Ok(' '),
        other => {
            let mut it = other.chars();
            match (it.next(), it.next()) {
                (Some(c), None) => Ok(c),
                _ => Err(format!(
                    "{which} must be a single character or one of comma/tab/semicolon/pipe/space — got '{other}'"
                )),
            }
        }
    }
}

/// Sniff the quote character: double wins if any straight or curly double quote
/// appears; single only if a `'` actually opens a field (so `it's` in plain text
/// is never mistaken for a quote character).
fn detect_quote(ch: &[char]) -> Option<char> {
    if ch
        .iter()
        .any(|c| *c == '"' || SMART_DOUBLE.contains(c))
    {
        return Some('"');
    }
    let candidates = [',', ';', '\t', '|'];
    let mut prev_starts_field = true;
    for c in ch {
        if (*c == '\'' || SMART_SINGLE.contains(c)) && prev_starts_field {
            return Some('\'');
        }
        prev_starts_field = candidates.contains(c) || *c == '\n' || *c == '\r';
    }
    Some('"')
}

/// Sniff the field separator from the first logical line: the most frequent
/// candidate outside quoted fields wins, ties broken by `, ; tab |` order.
fn detect_delim(ch: &[char], quote: Option<char>, smart: bool) -> char {
    let candidates = [',', ';', '\t', '|'];
    let mut counts = [0usize; 4];
    let mut in_quotes = false;
    for (i, c) in ch.iter().enumerate() {
        if let Some(qc) = quote {
            if is_quote(*c, qc, smart) {
                // A doubled quote is an escape, not a state flip.
                if in_quotes && matches!(ch.get(i + 1), Some(n) if is_quote(*n, qc, smart)) {
                    continue;
                }
                in_quotes = !in_quotes;
                continue;
            }
        }
        if in_quotes {
            continue;
        }
        if *c == '\n' || *c == '\r' {
            break;
        }
        if let Some(k) = candidates.iter().position(|d| d == c) {
            counts[k] += 1;
        }
    }
    let best = counts.iter().copied().max().unwrap_or(0);
    if best == 0 {
        return ',';
    }
    candidates[counts.iter().position(|n| *n == best).unwrap()]
}

/// One parsed field: its text plus whether the SOURCE had it quoted.
type Field = (String, bool);

/// Tolerant parse. Never fails on malformed quoting — every ambiguity is
/// resolved into a repair, recorded with its 1-based physical line.
fn parse(
    ch: &[char],
    delim: char,
    quote: Option<char>,
    smart: bool,
    backslash: bool,
    rep: &mut Vec<Repair>,
) -> Vec<Vec<Field>> {
    let mut records: Vec<Vec<Field>> = Vec::new();
    let mut i = 0usize;
    let mut line = 1usize;

    while i < ch.len() {
        // Blank lines carry no fields — drop them rather than emitting an empty row.
        if ch[i] == '\n' || ch[i] == '\r' {
            if ch[i] == '\r' && matches!(ch.get(i + 1), Some('\n')) {
                i += 1;
            }
            i += 1;
            rep.push((R_BLANK, line));
            line += 1;
            continue;
        }

        let mut fields: Vec<Field> = Vec::new();
        loop {
            let field_line = line;

            // Look ahead past padding for an opening quote; only commit to the
            // quoted path if one is actually there (otherwise keep the padding).
            let mut opened = None;
            if let Some(qc) = quote {
                let mut j = i;
                while j < ch.len() && (ch[j] == ' ' || ch[j] == '\t') && ch[j] != delim {
                    j += 1;
                }
                if j < ch.len() && is_quote(ch[j], qc, smart) {
                    opened = Some((j, j - i));
                }
            }

            if let (Some(qc), Some((j, pad))) = (quote, opened) {
                if ch[j] != qc {
                    rep.push((R_SMART, line));
                }
                if pad > 0 {
                    rep.push((R_PAD_OPEN, line));
                }
                i = j + 1;
                let mut val = String::new();
                let mut closed = false;

                while i < ch.len() {
                    let c = ch[i];

                    if c == '\r' || c == '\n' {
                        // A newline inside a quoted field is legal; normalize it
                        // so the chosen line ending applies everywhere.
                        if c == '\r' && matches!(ch.get(i + 1), Some('\n')) {
                            i += 1;
                        }
                        val.push('\n');
                        i += 1;
                        line += 1;
                        continue;
                    }

                    if backslash && c == '\\' {
                        match ch.get(i + 1) {
                            Some(&n) if is_quote(n, qc, smart) => {
                                // `\"` right before a delimiter/newline/EOF is a
                                // literal trailing backslash (`"C:\dir\",next`),
                                // not an escape — let the quote close instead.
                                let after = ch.get(i + 2).copied();
                                let terminates = match after {
                                    None => true,
                                    Some(a) => a == delim || a == '\n' || a == '\r',
                                };
                                if terminates {
                                    val.push('\\');
                                    i += 1;
                                } else {
                                    val.push(qc);
                                    i += 2;
                                    rep.push((R_BACKSLASH, line));
                                }
                                continue;
                            }
                            Some('\\') => {
                                val.push('\\');
                                i += 2;
                                rep.push((R_BACKSLASH_BS, line));
                                continue;
                            }
                            _ => {}
                        }
                    }

                    if is_quote(c, qc, smart) {
                        // Doubled quote → one literal quote (RFC 4180 escaping).
                        if matches!(ch.get(i + 1), Some(&n) if is_quote(n, qc, smart)) {
                            val.push(qc);
                            i += 2;
                            continue;
                        }
                        // A quote closes the field only when a delimiter, a
                        // newline or EOF follows (padding allowed).
                        let mut k = i + 1;
                        while k < ch.len() && (ch[k] == ' ' || ch[k] == '\t') && ch[k] != delim {
                            k += 1;
                        }
                        let pad_after = k - (i + 1);
                        let terminates =
                            k >= ch.len() || ch[k] == delim || ch[k] == '\n' || ch[k] == '\r';
                        if terminates {
                            if c != qc {
                                rep.push((R_SMART, line));
                            }
                            if pad_after > 0 {
                                rep.push((R_PAD_CLOSE, line));
                            }
                            i = k;
                            closed = true;
                            break;
                        }
                        // Not a terminator. If the rest of this line holds no
                        // further quote, the value simply ran on past its closing
                        // quote (`"abc"def`) — close and merge the tail back in.
                        let mut m = k;
                        let mut rest_has_quote = false;
                        while m < ch.len() && ch[m] != '\n' && ch[m] != '\r' {
                            if is_quote(ch[m], qc, smart) {
                                rest_has_quote = true;
                                break;
                            }
                            m += 1;
                        }
                        if !rest_has_quote {
                            if c != qc {
                                rep.push((R_SMART, line));
                            }
                            i += 1;
                            while i < ch.len()
                                && ch[i] != delim
                                && ch[i] != '\n'
                                && ch[i] != '\r'
                            {
                                val.push(ch[i]);
                                i += 1;
                            }
                            rep.push((R_JUNK, line));
                            closed = true;
                            break;
                        }
                        // Otherwise it is an un-escaped inner quote
                        // (`"He said "hi" to me"`) — keep it as text.
                        val.push(qc);
                        i += 1;
                        rep.push((R_STRAY_IN, line));
                        continue;
                    }

                    val.push(c);
                    i += 1;
                }

                if !closed {
                    rep.push((R_UNCLOSED, field_line));
                }
                fields.push((val, true));
            } else {
                let mut val = String::new();
                while i < ch.len() {
                    let c = ch[i];
                    if c == delim || c == '\n' || c == '\r' {
                        break;
                    }
                    if backslash && c == '\\' {
                        if let Some(qc) = quote {
                            if matches!(ch.get(i + 1), Some(&n) if is_quote(n, qc, smart)) {
                                val.push(qc);
                                i += 2;
                                rep.push((R_BACKSLASH, line));
                                continue;
                            }
                        }
                    }
                    if let Some(qc) = quote {
                        if is_quote(c, qc, smart) {
                            rep.push((R_BARE, line));
                        }
                    }
                    val.push(c);
                    i += 1;
                }
                fields.push((val, false));
            }

            if i < ch.len() && ch[i] == delim {
                i += 1;
                continue;
            }
            break;
        }

        records.push(fields);

        if i < ch.len() {
            if ch[i] == '\r' {
                i += 1;
                if matches!(ch.get(i), Some('\n')) {
                    i += 1;
                }
                line += 1;
            } else if ch[i] == '\n' {
                i += 1;
                line += 1;
            }
        }
    }

    records
}

/// Does the value look like a plain decimal number? Used by `non_numeric`.
/// Deliberately stricter than `f64::from_str`: `inf`/`NaN` are text, not numbers.
fn is_numeric(s: &str) -> bool {
    if s.is_empty() {
        return false;
    }
    if !s
        .chars()
        .all(|c| c.is_ascii_digit() || matches!(c, '+' | '-' | '.' | 'e' | 'E'))
    {
        return false;
    }
    if !s.chars().any(|c| c.is_ascii_digit()) {
        return false;
    }
    s.parse::<f64>().is_ok()
}

/// Serialize one field under the chosen policy.
fn write_field(
    val: &str,
    style: &str,
    out_delim: char,
    out_quote: char,
    backslash_escape: bool,
    eol: &str,
    row: usize,
    col: usize,
) -> Result<(String, bool), String> {
    let special =
        |s: &str| s.chars().any(|c| c == out_delim || c == out_quote || c == '\n' || c == '\r');

    if style == "never" {
        if !backslash_escape {
            if special(val) {
                return Err(format!(
                    "row {row}, field {col}: the value contains the delimiter, the quote character or a line break, and quote_style = 'never' has no way to represent it — use escape = 'backslash', or quote_style = 'minimal'"
                ));
            }
            if val.contains('\\') {
                // Nothing to escape it with, but it also can't be misread.
            }
            return Ok((val.to_string(), false));
        }
        let mut out = String::with_capacity(val.len());
        for c in val.chars() {
            match c {
                '\\' => out.push_str("\\\\"),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                c if c == out_delim => {
                    out.push('\\');
                    out.push(c);
                }
                c if c == out_quote => {
                    out.push('\\');
                    out.push(c);
                }
                c => out.push(c),
            }
        }
        return Ok((out, false));
    }

    let mut quoted = match style {
        "always" => true,
        "non_numeric" => !is_numeric(val),
        "minimal" => false,
        other => {
            return Err(format!(
                "quote_style must be minimal, always, non_numeric, or never — got '{other}'"
            ))
        }
    };
    // Whatever the policy asked for, never emit something that can't be re-read.
    if !quoted && special(val) {
        quoted = true;
    }

    if !quoted {
        return Ok((val.to_string(), false));
    }

    let mut out = String::with_capacity(val.len() + 2);
    out.push(out_quote);
    for c in val.chars() {
        if c == out_quote {
            if backslash_escape {
                out.push('\\');
                out.push(c);
            } else {
                out.push(c);
                out.push(c);
            }
        } else if c == '\\' && backslash_escape {
            out.push_str("\\\\");
        } else if c == '\n' {
            // An embedded line break takes the chosen line ending too.
            out.push_str(eol);
        } else {
            out.push(c);
        }
    }
    out.push(out_quote);
    Ok((out, true))
}

fn describe_char(c: char) -> String {
    match c {
        '\t' => "tab".to_string(),
        ' ' => "space".to_string(),
        c => c.to_string(),
    }
}

/// Re-emit `input` with one consistent quoting/escaping/delimiter dialect.
///
/// * `delimiter` — `auto` (default) or a delimiter spec for the INPUT.
/// * `output_delimiter` — `same` (default) or a delimiter spec for the OUTPUT.
/// * `input_quote` — `auto` | `double` | `single` | `none`.
/// * `quote_style` — `minimal` | `always` | `non_numeric` | `never`.
/// * `output_quote` — `double` | `single`.
/// * `escape` — `doubled` (RFC 4180 `""`) | `backslash` (`\"`).
/// * `backslash_escapes` — read `\"` in the INPUT as an escaped quote.
/// * `smart_quotes` — read curly quotes as field quotes.
/// * `line_ending` — `lf` | `crlf`.
/// * `output` — `csv` (the rewritten file) | `report` (what was changed).
#[allow(clippy::too_many_arguments)]
pub fn normalize(
    input: &str,
    delimiter: &str,
    output_delimiter: &str,
    input_quote: &str,
    quote_style: &str,
    output_quote: &str,
    escape: &str,
    backslash_escapes: bool,
    smart_quotes: bool,
    line_ending: &str,
    output: &str,
) -> Result<String, String> {
    if input.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "input is {} bytes; the limit is {MAX_INPUT_BYTES} bytes",
            input.len()
        ));
    }
    let mut rep: Vec<Repair> = Vec::new();

    let mut text = input;
    if let Some(stripped) = text.strip_prefix('\u{feff}') {
        text = stripped;
        rep.push((R_BOM, 1));
    }
    if text.trim().is_empty() {
        return Err("input is empty — paste some CSV text".into());
    }

    let quote_style = if quote_style.is_empty() { "minimal" } else { quote_style };
    if !matches!(quote_style, "minimal" | "always" | "non_numeric" | "never") {
        return Err(format!(
            "quote_style must be minimal, always, non_numeric, or never — got '{quote_style}'"
        ));
    }
    let out_quote = match if output_quote.is_empty() { "double" } else { output_quote } {
        "double" => '"',
        "single" => '\'',
        other => {
            return Err(format!(
                "output_quote must be double or single — got '{other}'"
            ))
        }
    };
    let backslash_escape_out = match if escape.is_empty() { "doubled" } else { escape } {
        "doubled" => false,
        "backslash" => true,
        other => {
            return Err(format!(
                "escape must be doubled or backslash — got '{other}'"
            ))
        }
    };
    let eol = match if line_ending.is_empty() { "lf" } else { line_ending } {
        "lf" => "\n",
        "crlf" => "\r\n",
        other => return Err(format!("line_ending must be lf or crlf — got '{other}'")),
    };
    let output = if output.is_empty() { "csv" } else { output };
    if !matches!(output, "csv" | "report") {
        return Err(format!("output must be csv or report — got '{output}'"));
    }

    let ch: Vec<char> = text.chars().collect();

    let quote_spec = if input_quote.is_empty() { "auto" } else { input_quote };
    let (in_quote, quote_sniffed) = match quote_spec {
        "auto" => (detect_quote(&ch), true),
        "double" => (Some('"'), false),
        "single" => (Some('\''), false),
        "none" => (None, false),
        other => {
            return Err(format!(
                "input_quote must be auto, double, single, or none — got '{other}'"
            ))
        }
    };

    let delim_spec = if delimiter.is_empty() { "auto" } else { delimiter };
    let (in_delim, delim_sniffed) = if delim_spec == "auto" {
        (detect_delim(&ch, in_quote, smart_quotes), true)
    } else {
        (resolve_delim(delim_spec, "delimiter")?, false)
    };

    let out_delim_spec = if output_delimiter.is_empty() { "same" } else { output_delimiter };
    let out_delim = if out_delim_spec == "same" {
        in_delim
    } else {
        resolve_delim(out_delim_spec, "output_delimiter")?
    };

    if let Some(q) = in_quote {
        if q == in_delim {
            return Err("the input delimiter and quote character must differ".into());
        }
    }
    if out_delim == out_quote && quote_style != "never" {
        return Err("the output delimiter and quote character must differ".into());
    }

    let records = parse(&ch, in_delim, in_quote, smart_quotes, backslash_escapes, &mut rep);
    if records.is_empty() {
        return Err("input is empty — paste some CSV text".into());
    }

    let mut out = String::with_capacity(text.len() + text.len() / 8);
    let mut quoted_out = 0usize;
    let mut quoted_in = 0usize;
    let mut min_cols = usize::MAX;
    let mut max_cols = 0usize;

    for (r, rec) in records.iter().enumerate() {
        min_cols = min_cols.min(rec.len());
        max_cols = max_cols.max(rec.len());
        for (c, (val, was_quoted)) in rec.iter().enumerate() {
            if *was_quoted {
                quoted_in += 1;
            }
            if c > 0 {
                out.push(out_delim);
            }
            let (rendered, q) = write_field(
                val,
                quote_style,
                out_delim,
                out_quote,
                backslash_escape_out,
                eol,
                r + 1,
                c + 1,
            )?;
            if q {
                quoted_out += 1;
            }
            out.push_str(&rendered);
        }
        out.push_str(eol);
    }

    if output == "csv" {
        return Ok(out);
    }

    let total_fields: usize = records.iter().map(|r| r.len()).sum();
    let mut counts: BTreeMap<&'static str, (usize, Vec<usize>)> = BTreeMap::new();
    for (kind, line) in &rep {
        let e = counts.entry(kind).or_insert((0, Vec::new()));
        e.0 += 1;
        if !e.1.contains(line) {
            e.1.push(*line);
        }
    }

    let mut r = String::new();
    r.push_str("CSV quote normalization report\n\n");
    r.push_str(&format!(
        "Input delimiter:  {}{}\n",
        describe_char(in_delim),
        if delim_sniffed { " (auto-detected)" } else { "" }
    ));
    r.push_str(&format!(
        "Input quote:      {}{}\n",
        match in_quote {
            Some(q) => describe_char(q),
            None => "none".to_string(),
        },
        if quote_sniffed { " (auto-detected)" } else { "" }
    ));
    r.push_str(&format!("Output delimiter: {}\n", describe_char(out_delim)));
    r.push_str(&format!(
        "Output quote:     {} (escaped by {})\n",
        describe_char(out_quote),
        if backslash_escape_out { "backslash" } else { "doubling" }
    ));
    r.push_str(&format!("Quote style:      {quote_style}\n"));
    r.push_str(&format!(
        "Line ending:      {}\n\n",
        if eol == "\n" { "LF" } else { "CRLF" }
    ));
    r.push_str(&format!("Rows:   {}\n", records.len()));
    if min_cols == max_cols {
        r.push_str(&format!("Fields: {total_fields} in {min_cols} consistent columns\n"));
    } else {
        r.push_str(&format!(
            "Fields: {total_fields}, but rows are ragged ({min_cols}-{max_cols} columns) — this tool re-quotes, it does not pad rows\n"
        ));
    }
    r.push_str(&format!(
        "Quoted: {quoted_in} of {total_fields} fields in the input, {quoted_out} in the output\n\n"
    ));

    if counts.is_empty() {
        r.push_str("Repairs applied: 0 — the input already parsed cleanly.\n");
    } else {
        r.push_str(&format!("Repairs applied: {}\n", rep.len()));
        for (kind, (n, lines)) in &counts {
            let shown = lines.len().min(5);
            let where_ = lines[..shown]
                .iter()
                .map(|l| l.to_string())
                .collect::<Vec<_>>()
                .join(", ");
            let more = if lines.len() > shown { ", …" } else { "" };
            let label = if shown == 1 { "line" } else { "lines" };
            r.push_str(&format!("  {kind} — {n} ({label} {where_}{more})\n"));
        }
    }
    Ok(r)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Defaults: auto dialect, minimal quoting, doubled escaping, LF, csv out.
    fn norm(input: &str) -> String {
        normalize(
            input, "auto", "same", "auto", "minimal", "double", "doubled", true, true, "lf", "csv",
        )
        .unwrap()
    }

    #[test]
    fn clean_csv_round_trips() {
        assert_eq!(norm("a,b\n1,2\n"), "a,b\n1,2\n");
    }

    #[test]
    fn backslash_escaped_quotes_become_rfc4180() {
        // The exact file csv-change-delimiter mangles.
        assert_eq!(norm("a,b\n\"x \\\"y\\\"\",2\n"), "a,b\n\"x \"\"y\"\"\",2\n");
    }

    #[test]
    fn single_quoted_fields_are_read_and_re_emitted_double() {
        assert_eq!(norm("a,b\n'x,y',2\n"), "a,b\n\"x,y\",2\n");
    }

    #[test]
    fn curly_smart_quotes_are_read_as_field_quotes() {
        assert_eq!(
            norm("a,b\n\u{201C}x,y\u{201D},2\n"),
            "a,b\n\"x,y\",2\n"
        );
    }

    #[test]
    fn padding_before_an_opening_quote_is_dropped() {
        assert_eq!(norm("a,b\n1, \"x,y\"\n"), "a,b\n1,\"x,y\"\n");
    }

    #[test]
    fn stray_inner_quotes_are_kept_as_text() {
        assert_eq!(
            norm("note\n\"He said \"hi\" to me\"\n"),
            "note\n\"He said \"\"hi\"\" to me\"\n"
        );
    }

    #[test]
    fn text_after_a_closing_quote_is_merged_back() {
        assert_eq!(norm("a\n\"abc\"def\n"), "a\nabcdef\n");
    }

    #[test]
    fn bare_quote_in_an_unquoted_field_is_escaped_on_output() {
        assert_eq!(norm("a,b\nsay \"hi\",2\n"), "a,b\n\"say \"\"hi\"\"\",2\n");
    }

    #[test]
    fn unclosed_quote_is_closed_at_eof() {
        assert_eq!(norm("a,b\n\"oops,2\n"), "a,b\n\"oops,2\n\"\n");
    }

    #[test]
    fn trailing_backslash_before_a_closing_quote_stays_literal() {
        assert_eq!(norm("p,n\n\"C:\\dir\\\",next\n"), "p,n\nC:\\dir\\,next\n");
    }

    #[test]
    fn embedded_newline_survives_and_takes_the_chosen_line_ending() {
        let out = normalize(
            "a\n\"one\r\ntwo\"\n", "auto", "same", "auto", "minimal", "double", "doubled", true,
            true, "crlf", "csv",
        )
        .unwrap();
        assert_eq!(out, "a\r\n\"one\r\ntwo\"\r\n");
    }

    #[test]
    fn quote_style_always_quotes_every_field() {
        let out = normalize(
            "a,b\n1,2\n", "auto", "same", "auto", "always", "double", "doubled", true, true, "lf",
            "csv",
        )
        .unwrap();
        assert_eq!(out, "\"a\",\"b\"\n\"1\",\"2\"\n");
    }

    #[test]
    fn quote_style_non_numeric_quotes_only_text() {
        let out = normalize(
            "a,b\n1,x\n-2.5e3,\n", "auto", "same", "auto", "non_numeric", "double", "doubled",
            true, true, "lf", "csv",
        )
        .unwrap();
        assert_eq!(out, "\"a\",\"b\"\n1,\"x\"\n-2.5e3,\"\"\n");
    }

    #[test]
    fn quote_style_never_with_backslash_escapes_the_delimiter() {
        let out = normalize(
            "a,b\n\"x,y\",2\n", "auto", "same", "auto", "never", "double", "backslash", true,
            true, "lf", "csv",
        )
        .unwrap();
        assert_eq!(out, "a,b\nx\\,y,2\n");
    }

    #[test]
    fn quote_style_never_without_an_escape_is_a_clear_error() {
        let err = normalize(
            "a,b\n\"x,y\",2\n", "auto", "same", "auto", "never", "double", "doubled", true, true,
            "lf", "csv",
        )
        .unwrap_err();
        assert!(err.contains("quote_style = 'never'"), "{err}");
        assert!(err.contains("row 2, field 1"), "{err}");
    }

    #[test]
    fn backslash_output_escaping_is_emitted() {
        let out = normalize(
            "a\n\"x \"\"y\"\"\"\n", "auto", "same", "auto", "minimal", "double", "backslash",
            true, true, "lf", "csv",
        )
        .unwrap();
        assert_eq!(out, "a\n\"x \\\"y\\\"\"\n");
    }

    #[test]
    fn output_quote_single_is_honoured() {
        let out = normalize(
            "a\n\"x,y\"\n", "auto", "same", "auto", "minimal", "single", "doubled", true, true,
            "lf", "csv",
        )
        .unwrap();
        assert_eq!(out, "a\n'x,y'\n");
    }

    #[test]
    fn delimiter_can_change_on_the_way_out() {
        let out = normalize(
            "a;b\n\"x;y\";2\n", "auto", "tab", "auto", "minimal", "double", "doubled", true, true,
            "lf", "csv",
        )
        .unwrap();
        assert_eq!(out, "a\tb\nx;y\t2\n");
    }

    #[test]
    fn input_quote_none_treats_quotes_as_literal_text() {
        let out = normalize(
            "a,b\n\"x\",2\n", "auto", "same", "none", "minimal", "double", "doubled", true, true,
            "lf", "csv",
        )
        .unwrap();
        assert_eq!(out, "a,b\n\"\"\"x\"\"\",2\n");
    }

    #[test]
    fn smart_quotes_off_keeps_curly_quotes_as_content() {
        let out = normalize(
            "a\n\u{201C}x\u{201D}\n", "auto", "same", "double", "minimal", "double", "doubled",
            true, false, "lf", "csv",
        )
        .unwrap();
        assert_eq!(out, "a\n\u{201C}x\u{201D}\n");
    }

    #[test]
    fn backslash_escapes_off_keeps_the_backslash() {
        // With backslash reading off, `\` is content and the quote it precedes is
        // an un-escaped inner quote — both survive, properly escaped on output.
        let out = normalize(
            "a,b\n\"x \\\"y\",2\n", "auto", "same", "auto", "minimal", "double", "doubled",
            false, true, "lf", "csv",
        )
        .unwrap();
        assert_eq!(out, "a,b\n\"x \\\"\"y\",2\n");
    }

    #[test]
    fn blank_lines_and_bom_are_dropped() {
        assert_eq!(norm("\u{feff}a,b\n\n1,2\n"), "a,b\n1,2\n");
    }

    #[test]
    fn ragged_rows_keep_their_length() {
        assert_eq!(norm("a,b,c\n1,2\n"), "a,b,c\n1,2\n");
    }

    #[test]
    fn tsv_is_auto_detected() {
        assert_eq!(norm("a\tb\n1\t2\n"), "a\tb\n1\t2\n");
    }

    #[test]
    fn report_lists_every_repair() {
        let out = normalize(
            "a,b\n\"x \\\"y\\\"\",2\n", "auto", "same", "auto", "minimal", "double", "doubled",
            true, true, "lf", "report",
        )
        .unwrap();
        assert!(out.contains("Input delimiter:  , (auto-detected)"), "{out}");
        assert!(out.contains("Repairs applied: 2"), "{out}");
        assert!(out.contains("backslash-escaped quote"), "{out}");
        assert!(out.contains("(line 2)"), "{out}");
    }

    #[test]
    fn report_says_so_when_nothing_needed_fixing() {
        let out = normalize(
            "a,b\n1,2\n", "auto", "same", "auto", "minimal", "double", "doubled", true, true,
            "lf", "report",
        )
        .unwrap();
        assert!(out.contains("Repairs applied: 0"), "{out}");
        assert!(out.contains("2 consistent columns"), "{out}");
    }

    #[test]
    fn report_flags_ragged_rows() {
        let out = normalize(
            "a,b,c\n1,2\n", "auto", "same", "auto", "minimal", "double", "doubled", true, true,
            "lf", "report",
        )
        .unwrap();
        assert!(out.contains("ragged"), "{out}");
    }

    #[test]
    fn empty_input_is_an_error() {
        let err = normalize(
            "   ", "auto", "same", "auto", "minimal", "double", "doubled", true, true, "lf",
            "csv",
        )
        .unwrap_err();
        assert!(err.contains("empty"), "{err}");
    }

    #[test]
    fn unknown_quote_style_is_an_error() {
        let err = normalize(
            "a\n1\n", "auto", "same", "auto", "loose", "double", "doubled", true, true, "lf",
            "csv",
        )
        .unwrap_err();
        assert!(err.contains("quote_style must be"), "{err}");
    }

    #[test]
    fn bad_delimiter_spec_is_an_error() {
        let err = normalize(
            "a\n1\n", "commaa", "same", "auto", "minimal", "double", "doubled", true, true, "lf",
            "csv",
        )
        .unwrap_err();
        assert!(err.contains("delimiter must be"), "{err}");
    }

    #[test]
    fn oversized_input_is_rejected() {
        let big = "a,b\n".repeat(MAX_INPUT_BYTES / 4 + 1);
        let err = normalize(
            &big, "auto", "same", "auto", "minimal", "double", "doubled", true, true, "lf", "csv",
        )
        .unwrap_err();
        assert!(err.contains("the limit is"), "{err}");
    }
}
