//! csv-header-sanitizer core — pure compute, shared by the chat skill block and
//! the web page. No wafer/wasm-bindgen deps. Rewrites the HEADER ROW of a
//! CSV/delimited table into valid, consistent identifiers — `First Name` →
//! `first_name`, `Total ($)` → `total`, `2024 Revenue` → `_2024_revenue` — and
//! deduplicates collisions so two source columns can never clean to the same
//! label and silently overwrite one another downstream.
//!
//! Only row 1 is touched. Data rows are passed through, the field separator
//! round-trips unchanged, and nothing is inferred about the values.

use deunicode::deunicode;
use std::collections::HashSet;

/// Hard cap on the pasted table, so a runaway paste can't wedge the tab.
pub const MAX_INPUT_BYTES: usize = 5_000_000;

/// Upper bound for the `max_length` cap. BigQuery's column-name ceiling is 300
/// characters; PostgreSQL truncates identifiers at 63 bytes, so 63 is the other
/// value worth knowing.
pub const MAX_NAME_LENGTH: u32 = 300;

/// Fallback base name used when the `blank_name` option is itself empty.
const FALLBACK_BLANK_NAME: &str = "column";

/// Target identifier casing.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Style {
    /// `first_name` (default).
    Snake,
    /// `firstName`.
    Camel,
    /// `FirstName`.
    Pascal,
    /// `first-name`.
    Kebab,
    /// `FIRST_NAME`.
    ScreamingSnake,
    /// `firstname` — lowercase everything, but do NOT split CamelCase runs.
    Lower,
    /// `First_Name` — keep the original case, only fix the characters.
    Preserve,
}

impl Style {
    /// The separator written between words (empty for the concatenating styles).
    fn sep(self) -> &'static str {
        match self {
            Style::Kebab => "-",
            Style::Camel | Style::Pascal => "",
            _ => "_",
        }
    }

    /// Whether `FirstName` is read as two words. The `lower`/`preserve` styles
    /// deliberately do not split, matching the "just fix the characters"
    /// behavior people expect from those two.
    fn splits_case(self) -> bool {
        !matches!(self, Style::Lower | Style::Preserve)
    }
}

fn parse_style(s: &str) -> Result<Style, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "snake" => Ok(Style::Snake),
        "camel" => Ok(Style::Camel),
        "pascal" => Ok(Style::Pascal),
        "kebab" => Ok(Style::Kebab),
        "screaming_snake" => Ok(Style::ScreamingSnake),
        "lower" => Ok(Style::Lower),
        "preserve" => Ok(Style::Preserve),
        other => Err(format!(
            "style must be one of snake, camel, pascal, kebab, screaming_snake, lower, preserve, got '{other}'"
        )),
    }
}

/// What to do when a name would start with a digit (invalid for an unquoted SQL
/// identifier and for most programming-language identifiers).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum LeadingDigit {
    /// `2024 Revenue` → `_2024_revenue` (default).
    Underscore,
    /// `2024 Revenue` → `col_2024_revenue`.
    Col,
    /// Leave it alone.
    Keep,
}

fn parse_leading_digit(s: &str) -> Result<LeadingDigit, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "underscore" => Ok(LeadingDigit::Underscore),
        "col" => Ok(LeadingDigit::Col),
        "keep" => Ok(LeadingDigit::Keep),
        other => Err(format!(
            "leading_digit must be 'underscore', 'col', or 'keep', got '{other}'"
        )),
    }
}

/// Collision policy for names that clean to the same string.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Dedupe {
    /// `total`, `total_2`, `total_3` (default).
    Suffix,
    /// Suffix the column's own 1-based position: `total`, `total_4`.
    Index,
    /// Leave collisions in place.
    Allow,
}

fn parse_dedupe(s: &str) -> Result<Dedupe, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "suffix" => Ok(Dedupe::Suffix),
        "index" => Ok(Dedupe::Index),
        "allow" => Ok(Dedupe::Allow),
        other => Err(format!(
            "dedupe must be 'suffix', 'index', or 'allow', got '{other}'"
        )),
    }
}

/// What the tool returns.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Output {
    /// The whole table with a rewritten header row (default).
    Csv,
    /// Just the cleaned header row.
    Header,
    /// A two-column `original,sanitized` audit trail.
    Mapping,
}

fn parse_output(s: &str) -> Result<Output, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "csv" => Ok(Output::Csv),
        "header" => Ok(Output::Header),
        "mapping" => Ok(Output::Mapping),
        other => Err(format!(
            "output must be 'csv', 'header', or 'mapping', got '{other}'"
        )),
    }
}

/// Resolve a delimiter spec to its byte. `auto` sniffs the header line.
fn delim_byte(spec: &str, data: &str) -> Result<u8, String> {
    let s = spec.trim();
    Ok(match s {
        "auto" => sniff_delimiter(data),
        "" | "," | "comma" => b',',
        "\t" | "tab" | "\\t" => b'\t',
        ";" | "semicolon" => b';',
        "|" | "pipe" => b'|',
        other => {
            let b = other.as_bytes();
            if b.len() == 1 {
                b[0]
            } else {
                return Err(format!(
                    "delimiter must be 'auto', a single character, or comma/tab/semicolon/pipe, got '{other}'"
                ));
            }
        }
    })
}

/// Pick the delimiter that occurs most often outside quotes on the first
/// non-empty line. Ties (and a line with none of them) fall back to a comma.
fn sniff_delimiter(data: &str) -> u8 {
    let line = data.lines().find(|l| !l.trim().is_empty()).unwrap_or("");
    let mut in_quote = false;
    let (mut comma, mut tab, mut semi, mut pipe) = (0usize, 0usize, 0usize, 0usize);
    for ch in line.chars() {
        match ch {
            '"' => in_quote = !in_quote,
            _ if in_quote => {}
            ',' => comma += 1,
            '\t' => tab += 1,
            ';' => semi += 1,
            '|' => pipe += 1,
            _ => {}
        }
    }
    // Comma first so it wins every tie.
    [(b',', comma), (b'\t', tab), (b';', semi), (b'|', pipe)]
        .into_iter()
        .filter(|(_, n)| *n > 0)
        .max_by_key(|(_, n)| *n)
        .map(|(b, _)| b)
        .unwrap_or(b',')
}

/// Split a raw header label into words. Every non-alphanumeric run is a word
/// boundary; when `split_case` is on, `firstName`/`HTTPStatus`/`2024Revenue`
/// also break at the case (and digit→capital) transitions.
fn words(s: &str, split_case: bool) -> Vec<String> {
    let chars: Vec<char> = s.chars().collect();
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    for (i, &c) in chars.iter().enumerate() {
        if !c.is_alphanumeric() {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            continue;
        }
        if split_case && !cur.is_empty() {
            let prev = chars[i - 1];
            let next_is_lower = chars.get(i + 1).is_some_and(|n| n.is_lowercase());
            // `aB` / `2B` start a new word; so does the last capital of an
            // acronym run that is followed by a lowercase letter (`HTTPStatus`).
            let boundary = c.is_uppercase()
                && ((prev.is_lowercase() || prev.is_numeric())
                    || (prev.is_uppercase() && next_is_lower));
            if boundary {
                out.push(std::mem::take(&mut cur));
            }
        }
        cur.push(c);
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

/// `status` → `Status` (the rest is lowercased, so `HTTP` → `Http`).
fn title(word: &str) -> String {
    let mut cs = word.chars();
    match cs.next() {
        Some(f) => f.to_uppercase().collect::<String>() + &cs.as_str().to_lowercase(),
        None => String::new(),
    }
}

/// Join the word list in the target casing.
fn join_words(ws: &[String], style: Style) -> String {
    match style {
        Style::Snake | Style::Lower => {
            ws.iter().map(|w| w.to_lowercase()).collect::<Vec<_>>().join("_")
        }
        Style::Kebab => ws.iter().map(|w| w.to_lowercase()).collect::<Vec<_>>().join("-"),
        Style::ScreamingSnake => {
            ws.iter().map(|w| w.to_uppercase()).collect::<Vec<_>>().join("_")
        }
        Style::Preserve => ws.join("_"),
        Style::Pascal => ws.iter().map(|w| title(w)).collect::<Vec<_>>().join(""),
        Style::Camel => ws
            .iter()
            .enumerate()
            .map(|(i, w)| if i == 0 { w.to_lowercase() } else { title(w) })
            .collect::<Vec<_>>()
            .join(""),
    }
}

/// Cut `name` to `max` characters, then drop any separator left dangling at the
/// end. `max == 0` means "no limit".
fn truncate(name: &str, max: usize) -> String {
    if max == 0 || name.chars().count() <= max {
        return name.to_string();
    }
    let cut: String = name.chars().take(max).collect();
    let trimmed = cut.trim_end_matches(['_', '-']);
    if trimmed.is_empty() {
        cut
    } else {
        trimmed.to_string()
    }
}

/// Build `base + suffix` so the result still honours the length cap: the base
/// is shortened by however much the suffix needs.
fn with_suffix(base: &str, suffix: &str, max: usize) -> String {
    if max == 0 {
        return format!("{base}{suffix}");
    }
    let room = max.saturating_sub(suffix.chars().count()).max(1);
    format!("{}{}", truncate(base, room), suffix)
}

struct Opts {
    style: Style,
    ascii: bool,
    leading_digit: LeadingDigit,
    max_length: usize,
    blank_name: String,
}

/// Clean ONE header label. `pos` is the column's 1-based position, used to name
/// blank headers (`column_3`).
fn sanitize_one(raw: &str, pos: usize, o: &Opts) -> String {
    let source = if o.ascii {
        deunicode(raw)
    } else {
        raw.to_string()
    };
    let mut ws = words(&source, o.style.splits_case());

    // A header that is blank, or that is nothing but punctuation, gets a
    // positional name so the column is still addressable.
    if ws.is_empty() {
        let base = if o.ascii {
            deunicode(&o.blank_name)
        } else {
            o.blank_name.clone()
        };
        ws = words(&base, o.style.splits_case());
        if ws.is_empty() {
            ws = vec![FALLBACK_BLANK_NAME.to_string()];
        }
        ws.push(pos.to_string());
    }

    let starts_with_digit = ws
        .first()
        .and_then(|w| w.chars().next())
        .is_some_and(|c| c.is_numeric());
    if starts_with_digit && o.leading_digit == LeadingDigit::Col {
        ws.insert(0, "col".to_string());
    }

    let mut name = join_words(&ws, o.style);
    if starts_with_digit && o.leading_digit == LeadingDigit::Underscore {
        name.insert(0, '_');
    }
    truncate(&name, o.max_length)
}

/// Clean a whole header row, applying the collision policy across it.
fn sanitize_row(header: &[String], o: &Opts, dedupe: Dedupe) -> Vec<String> {
    let cleaned: Vec<String> = header
        .iter()
        .enumerate()
        .map(|(i, h)| sanitize_one(h, i + 1, o))
        .collect();
    if dedupe == Dedupe::Allow {
        return cleaned;
    }

    let sep = o.style.sep();
    let mut used: HashSet<String> = HashSet::new();
    let mut out: Vec<String> = Vec::with_capacity(cleaned.len());
    for (i, base) in cleaned.iter().enumerate() {
        if used.insert(base.clone()) {
            out.push(base.clone());
            continue;
        }
        // `index` tries the column's own 1-based position first; both policies
        // then fall back to counting up until the name is free.
        let mut candidate = if dedupe == Dedupe::Index {
            with_suffix(base, &format!("{sep}{}", i + 1), o.max_length)
        } else {
            String::new()
        };
        if candidate.is_empty() || used.contains(&candidate) {
            let mut n = 2usize;
            loop {
                candidate = with_suffix(base, &format!("{sep}{n}"), o.max_length);
                if !used.contains(&candidate) {
                    break;
                }
                n += 1;
            }
        }
        used.insert(candidate.clone());
        out.push(candidate);
    }
    out
}

/// Sanitize the header row of a CSV/delimited table.
///
/// * `data` — the table text; row 1 is the header.
/// * `delimiter` — `auto`, a single character, or `comma`/`tab`/`semicolon`/`pipe`.
/// * `style` — `snake` (default), `camel`, `pascal`, `kebab`, `screaming_snake`,
///   `lower`, or `preserve`.
/// * `ascii` — transliterate Unicode to ASCII before cleaning.
/// * `leading_digit` — `underscore` (default), `col`, or `keep`.
/// * `max_length` — truncate names to this many characters (0 = no limit).
/// * `blank_name` — base name for blank headers, suffixed with the column position.
/// * `dedupe` — `suffix` (default), `index`, or `allow`.
/// * `output` — `csv` (default), `header`, or `mapping`.
#[allow(clippy::too_many_arguments)]
pub fn sanitize(
    data: &str,
    delimiter: &str,
    style: &str,
    ascii: bool,
    leading_digit: &str,
    max_length: u32,
    blank_name: &str,
    dedupe: &str,
    output: &str,
) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err(
            "input is empty — paste a CSV table whose first row is the header".to_string(),
        );
    }
    if data.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "input is {} bytes, which exceeds the {MAX_INPUT_BYTES}-byte limit",
            data.len()
        ));
    }
    if max_length > MAX_NAME_LENGTH {
        return Err(format!(
            "max_length must be between 0 (no limit) and {MAX_NAME_LENGTH}, got {max_length}"
        ));
    }

    let delim = delim_byte(delimiter, data)?;
    let opts = Opts {
        style: parse_style(style)?,
        ascii,
        leading_digit: parse_leading_digit(leading_digit)?,
        max_length: max_length as usize,
        blank_name: if blank_name.trim().is_empty() {
            FALLBACK_BLANK_NAME.to_string()
        } else {
            blank_name.trim().to_string()
        },
    };
    let dedupe = parse_dedupe(dedupe)?;
    let output = parse_output(output)?;

    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .flexible(true)
        .has_headers(false)
        .from_reader(data.as_bytes());

    let mut rows: Vec<Vec<String>> = Vec::new();
    for rec in rdr.records() {
        let rec = rec.map_err(|e| format!("CSV parse error: {e}"))?;
        rows.push(rec.iter().map(|f| f.to_string()).collect());
    }
    let Some(header) = rows.first().cloned() else {
        return Err(
            "input is empty — paste a CSV table whose first row is the header".to_string(),
        );
    };

    let cleaned = sanitize_row(&header, &opts, dedupe);

    let mut wtr = csv::WriterBuilder::new()
        .delimiter(delim)
        .flexible(true)
        .terminator(csv::Terminator::Any(b'\n'))
        .from_writer(Vec::new());

    match output {
        Output::Header => {
            wtr.write_record(&cleaned)
                .map_err(|e| format!("CSV write error: {e}"))?;
        }
        Output::Mapping => {
            wtr.write_record(["original", "sanitized"])
                .map_err(|e| format!("CSV write error: {e}"))?;
            for (raw, name) in header.iter().zip(cleaned.iter()) {
                wtr.write_record([raw.as_str(), name.as_str()])
                    .map_err(|e| format!("CSV write error: {e}"))?;
            }
        }
        Output::Csv => {
            wtr.write_record(&cleaned)
                .map_err(|e| format!("CSV write error: {e}"))?;
            for row in rows.iter().skip(1) {
                wtr.write_record(row)
                    .map_err(|e| format!("CSV write error: {e}"))?;
            }
        }
    }

    let bytes = wtr
        .into_inner()
        .map_err(|e| format!("CSV write error: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("output is not valid UTF-8: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(data: &str) -> Result<String, String> {
        sanitize(data, ",", "snake", true, "underscore", 0, "column", "suffix", "csv")
    }

    fn header_of(data: &str, style: &str) -> String {
        sanitize(data, ",", style, true, "underscore", 0, "column", "suffix", "header")
            .unwrap()
            .trim_end()
            .to_string()
    }

    #[test]
    fn cleans_messy_headers_to_snake_case() {
        let got = run("First Name, Total ($) ,E-Mail Address\nAda,10,a@example.com\n").unwrap();
        assert_eq!(
            got,
            "first_name,total,e_mail_address\nAda,10,a@example.com\n"
        );
    }

    #[test]
    fn deduplicates_collisions_with_numeric_suffixes() {
        // "Total", "TOTAL" and "total " all clean to `total`.
        let got = header_of("Total,TOTAL,total ,Other", "snake");
        assert_eq!(got, "total,total_2,total_3,other");
    }

    #[test]
    fn dedupe_index_uses_the_column_position() {
        // Column 3 is the duplicate, so `index` names it after its position
        // while the default `suffix` policy would call it `total_2`.
        let src = "Total,Notes,TOTAL";
        let by_index = sanitize(
            src, ",", "snake", true, "underscore", 0, "column", "index", "header",
        )
        .unwrap();
        assert_eq!(by_index, "total,notes,total_3\n");
        assert_eq!(header_of(src, "snake"), "total,notes,total_2");
    }

    #[test]
    fn dedupe_allow_keeps_collisions() {
        let got = sanitize(
            "Total,TOTAL", ",", "snake", true, "underscore", 0, "column", "allow", "header",
        )
        .unwrap();
        assert_eq!(got, "total,total\n");
    }

    #[test]
    fn blank_headers_are_named_from_their_position() {
        let got = header_of("id,,   ,!!!", "snake");
        assert_eq!(got, "id,column_2,column_3,column_4");
    }

    #[test]
    fn leading_digits_are_repaired() {
        assert_eq!(header_of("2024 Revenue", "snake"), "_2024_revenue");
        let col = sanitize(
            "2024 Revenue", ",", "snake", true, "col", 0, "column", "suffix", "header",
        )
        .unwrap();
        assert_eq!(col, "col_2024_revenue\n");
        let keep = sanitize(
            "2024 Revenue", ",", "snake", true, "keep", 0, "column", "suffix", "header",
        )
        .unwrap();
        assert_eq!(keep, "2024_revenue\n");
    }

    #[test]
    fn transliterates_unicode_when_ascii_is_on() {
        assert_eq!(header_of("Año,Größe,Ünit Price", "snake"), "ano,grosse,unit_price");
        let kept = sanitize(
            "Año", ",", "snake", false, "underscore", 0, "column", "suffix", "header",
        )
        .unwrap();
        assert_eq!(kept, "año\n");
    }

    #[test]
    fn every_style_renders_its_own_shape() {
        let src = "First Name,HTTPStatusCode";
        assert_eq!(header_of(src, "snake"), "first_name,http_status_code");
        assert_eq!(header_of(src, "camel"), "firstName,httpStatusCode");
        assert_eq!(header_of(src, "pascal"), "FirstName,HttpStatusCode");
        assert_eq!(header_of(src, "kebab"), "first-name,http-status-code");
        assert_eq!(
            header_of(src, "screaming_snake"),
            "FIRST_NAME,HTTP_STATUS_CODE"
        );
        // `lower`/`preserve` fix the characters but never split a CamelCase run.
        assert_eq!(header_of(src, "lower"), "first_name,httpstatuscode");
        assert_eq!(header_of(src, "preserve"), "First_Name,HTTPStatusCode");
    }

    #[test]
    fn max_length_truncates_and_still_fits_the_dedupe_suffix() {
        let got = sanitize(
            "Customer Lifetime Value,Customer Lifetime Value",
            ",",
            "snake",
            true,
            "underscore",
            12,
            "column",
            "suffix",
            "header",
        )
        .unwrap();
        // Truncated to 12 chars, and the duplicate gives up two more so
        // `_2` still fits inside the cap.
        assert_eq!(got, "customer_lif,customer_l_2\n");
    }

    #[test]
    fn mapping_output_is_an_audit_trail() {
        let got = sanitize(
            "First Name,Total ($),Total ($)",
            ",",
            "snake",
            true,
            "underscore",
            0,
            "column",
            "suffix",
            "mapping",
        )
        .unwrap();
        assert_eq!(
            got,
            "original,sanitized\nFirst Name,first_name\nTotal ($),total\nTotal ($),total_2\n"
        );
    }

    #[test]
    fn tab_delimited_input_round_trips_its_separator() {
        let got = sanitize(
            "First Name\tTotal ($)\nAda\t10\n",
            "auto",
            "snake",
            true,
            "underscore",
            0,
            "column",
            "suffix",
            "csv",
        )
        .unwrap();
        assert_eq!(got, "first_name\ttotal\nAda\t10\n");
    }

    #[test]
    fn data_rows_and_quoting_are_preserved() {
        let got = run("A B,C D\n\"x, y\",2\n").unwrap();
        assert_eq!(got, "a_b,c_d\n\"x, y\",2\n");
    }

    #[test]
    fn rejects_empty_input() {
        let err = run("   \n").unwrap_err();
        assert!(err.contains("input is empty"), "got: {err}");
    }

    #[test]
    fn rejects_an_unknown_style() {
        let err = sanitize(
            "a,b", ",", "shouty", true, "underscore", 0, "column", "suffix", "header",
        )
        .unwrap_err();
        assert!(err.contains("style must be one of"), "got: {err}");
    }

    #[test]
    fn rejects_an_out_of_range_max_length() {
        let err = sanitize(
            "a,b", ",", "snake", true, "underscore", 999, "column", "suffix", "header",
        )
        .unwrap_err();
        assert!(err.contains("max_length must be between"), "got: {err}");
    }

    #[test]
    fn rejects_a_multi_character_delimiter() {
        let err = sanitize(
            "a,b", "::", "snake", true, "underscore", 0, "column", "suffix", "header",
        )
        .unwrap_err();
        assert!(err.contains("delimiter must be"), "got: {err}");
    }

    #[test]
    fn rejects_an_oversized_input() {
        let big = "a,b\n".to_string() + &"1,2\n".repeat(MAX_INPUT_BYTES / 4);
        let err = run(&big).unwrap_err();
        assert!(err.contains("exceeds the"), "got: {err}");
    }
}
