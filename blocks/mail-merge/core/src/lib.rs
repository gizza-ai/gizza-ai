//! mail-merge core — pure compute, shared by the chat skill block and the web
//! page. No wafer/wasm-bindgen deps.
//!
//! Fills a text/markdown template once per CSV row (classic "form letter" mail
//! merge): each `{{Column}}` placeholder is replaced by that row's value for the
//! matching CSV header column, and the per-row outputs are joined by a chosen
//! separator. Placeholder substitution is plain named lookup — no expressions,
//! loops or conditionals (that is the sibling `render-template` tool).

/// Maximum number of data rows a single merge may render. Keeps output bounded
/// for the in-browser wasm sandbox. At the cap it succeeds; one over errors.
pub const MAX_ROWS: usize = 1000;

/// Placeholder delimiter pair, selected by the `syntax` param.
#[derive(Clone, Copy)]
struct Delims {
    open: &'static str,
    close: &'static str,
}

fn delims(syntax: &str) -> Result<Delims, String> {
    Ok(match syntax {
        "" | "double_curly" => Delims { open: "{{", close: "}}" },
        "single_curly" => Delims { open: "{", close: "}" },
        "double_angle" => Delims { open: "<<", close: ">>" },
        other => {
            return Err(format!(
                "invalid syntax {other:?}: expected \"double_curly\", \"single_curly\", or \"double_angle\""
            ))
        }
    })
}

fn delim_byte(delimiter: &str) -> Result<u8, String> {
    Ok(match delimiter {
        "" | "," | "comma" => b',',
        ";" | "semicolon" => b';',
        "\t" | "tab" | "\\t" => b'\t',
        other => {
            return Err(format!(
                "invalid delimiter {other:?}: expected \"comma\", \"semicolon\", or \"tab\""
            ))
        }
    })
}

/// What to do when a `{{placeholder}}` names a column that is not in the CSV
/// header. (A column that exists but is empty for a given row always renders
/// as an empty string regardless of this setting.)
#[derive(Clone, Copy)]
enum OnMissing {
    /// Replace the placeholder with an empty string (the mail-merge default).
    Empty,
    /// Leave the placeholder text verbatim (useful for spotting typos).
    Keep,
    /// Fail the whole merge, naming the unknown column.
    Error,
}

fn on_missing(v: &str) -> Result<OnMissing, String> {
    Ok(match v {
        "" | "empty" => OnMissing::Empty,
        "keep" => OnMissing::Keep,
        "error" => OnMissing::Error,
        other => {
            return Err(format!(
                "invalid on_missing {other:?}: expected \"empty\", \"keep\", or \"error\""
            ))
        }
    })
}

fn separator_text(separator: &str) -> Result<&'static str, String> {
    Ok(match separator {
        "" | "divider" => "\n\n---\n\n",
        "blank_line" => "\n\n",
        "newline" => "\n",
        "form_feed" => "\u{000C}",
        "none" => "",
        other => {
            return Err(format!(
                "invalid separator {other:?}: expected \"divider\", \"blank_line\", \"newline\", \"form_feed\", or \"none\""
            ))
        }
    })
}

/// Fill `template` once per data row in `csv`.
///
/// - `csv`: first row is the header (column names); the remaining rows are data.
/// - `syntax`: placeholder style — `double_curly` (`{{col}}`, default),
///   `single_curly` (`{col}`), or `double_angle` (`<<col>>`).
/// - `delimiter`: CSV field delimiter — `comma` (default), `semicolon`, or `tab`.
/// - `on_missing`: how to handle a placeholder whose column is absent from the
///   header — `empty` (default), `keep`, or `error`.
/// - `case_insensitive`: when true (default), `{{First Name}}` matches a header
///   named `first name`.
/// - `separator`: text inserted between the rendered rows — `divider` (a `---`
///   rule, default), `blank_line`, `newline`, `form_feed`, or `none`.
///
/// Returns `Err` on an invalid option, empty/invalid CSV, more than
/// [`MAX_ROWS`] data rows, or (with `on_missing = error`) an unknown column.
pub fn merge(
    template: &str,
    csv: &str,
    syntax: &str,
    delimiter: &str,
    on_missing_v: &str,
    case_insensitive: bool,
    separator: &str,
) -> Result<String, String> {
    let d = delims(syntax)?;
    let delim = delim_byte(delimiter)?;
    let miss = on_missing(on_missing_v)?;
    let sep = separator_text(separator)?;

    if csv.trim().is_empty() {
        return Err("CSV data is empty — provide a header row plus at least one data row".into());
    }

    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .has_headers(false)
        .flexible(true)
        .from_reader(csv.as_bytes());
    let records: Vec<csv::StringRecord> = rdr
        .records()
        .collect::<Result<_, _>>()
        .map_err(|e| format!("CSV parse error: {e}"))?;

    let header = records
        .first()
        .ok_or_else(|| "CSV data is empty — provide a header row plus at least one data row".to_string())?;
    if records.len() < 2 {
        return Err("CSV has a header row but no data rows — add at least one row below the header".into());
    }
    let n_rows = records.len() - 1;
    if n_rows > MAX_ROWS {
        return Err(format!(
            "too many data rows: {n_rows} (max {MAX_ROWS}) — split the CSV into smaller batches"
        ));
    }

    // Map header name -> column index. First occurrence wins on a duplicate.
    // With case-insensitive matching the keys are lowercased.
    let key = |s: &str| {
        if case_insensitive {
            s.to_lowercase()
        } else {
            s.to_string()
        }
    };
    let mut cols: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for (i, h) in header.iter().enumerate() {
        cols.entry(key(h.trim())).or_insert(i);
    }

    let mut out = String::new();
    for (ri, rec) in records.iter().enumerate().skip(1) {
        let resolve = |name: &str| -> Option<String> {
            cols.get(&key(name))
                .map(|&idx| rec.get(idx).unwrap_or("").to_string())
        };
        let rendered = substitute(template, d, &resolve, miss)?;
        if ri > 1 {
            out.push_str(sep);
        }
        out.push_str(&rendered);
    }
    Ok(out)
}

/// Substitute every `open…close` placeholder in `template` using `resolve`.
/// An empty placeholder name, or an `open` with no matching `close`, is copied
/// through verbatim.
fn substitute(
    template: &str,
    d: Delims,
    resolve: &dyn Fn(&str) -> Option<String>,
    miss: OnMissing,
) -> Result<String, String> {
    let mut out = String::with_capacity(template.len());
    let mut i = 0;
    while i < template.len() {
        let rest = &template[i..];
        if rest.starts_with(d.open) {
            let after_open = i + d.open.len();
            if let Some(rel) = template[after_open..].find(d.close) {
                let raw = &template[after_open..after_open + rel];
                let name = raw.trim();
                if name.is_empty() {
                    // Not a real field (e.g. `{{}}`) — emit the open delim and
                    // continue scanning just past it.
                    out.push_str(d.open);
                    i = after_open;
                    continue;
                }
                match resolve(name) {
                    Some(v) => out.push_str(&v),
                    None => match miss {
                        OnMissing::Empty => {}
                        OnMissing::Keep => {
                            out.push_str(d.open);
                            out.push_str(raw);
                            out.push_str(d.close);
                        }
                        OnMissing::Error => {
                            return Err(format!(
                                "template references column {name:?}, which is not in the CSV header"
                            ));
                        }
                    },
                }
                i = after_open + rel + d.close.len();
                continue;
            }
            // Unterminated open delimiter — emit it literally and move on.
            out.push_str(d.open);
            i = after_open;
            continue;
        }
        // Copy one UTF-8 char.
        let ch = rest.chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn happy_basic_double_curly() {
        let out = merge(
            "Hi {{name}}, you owe ${{amount}}.",
            "name,amount\nAlice,10\nBob,20",
            "double_curly",
            "comma",
            "empty",
            true,
            "divider",
        )
        .unwrap();
        assert_eq!(out, "Hi Alice, you owe $10.\n\n---\n\nHi Bob, you owe $20.");
    }

    #[test]
    fn case_insensitive_header_match() {
        // Placeholder {{First Name}} matches header `first name`.
        let out = merge(
            "Dear {{First Name}},",
            "first name\nAda\nGrace",
            "double_curly",
            "comma",
            "empty",
            true,
            "newline",
        )
        .unwrap();
        assert_eq!(out, "Dear Ada,\nDear Grace,");
    }

    #[test]
    fn case_sensitive_off_leaves_missing_empty() {
        // With case-insensitive OFF, {{Name}} does not match header `name`.
        let out = merge(
            "Hello {{Name}}!",
            "name\nAda",
            "double_curly",
            "comma",
            "empty",
            false,
            "none",
        )
        .unwrap();
        assert_eq!(out, "Hello !");
    }

    #[test]
    fn single_curly_and_semicolon() {
        let out = merge(
            "{greeting} {who}",
            "greeting;who\nHej;Ada",
            "single_curly",
            "semicolon",
            "empty",
            true,
            "none",
        )
        .unwrap();
        assert_eq!(out, "Hej Ada");
    }

    #[test]
    fn double_angle_and_tab() {
        let out = merge(
            "<<a>>/<<b>>",
            "a\tb\n1\t2",
            "double_angle",
            "tab",
            "empty",
            true,
            "none",
        )
        .unwrap();
        assert_eq!(out, "1/2");
    }

    #[test]
    fn on_missing_keep_leaves_placeholder() {
        let out = merge(
            "{{name}} <{{email}}>",
            "name\nAda",
            "double_curly",
            "comma",
            "keep",
            true,
            "none",
        )
        .unwrap();
        assert_eq!(out, "Ada <{{email}}>");
    }

    #[test]
    fn on_missing_error_names_column() {
        let err = merge(
            "{{name}} {{email}}",
            "name\nAda",
            "double_curly",
            "comma",
            "error",
            true,
            "none",
        )
        .unwrap_err();
        assert!(err.contains("email"), "got: {err}");
    }

    #[test]
    fn quoted_csv_field_with_comma_and_newline() {
        // csv crate handles quoted fields containing the delimiter and newlines.
        let out = merge(
            "{{note}}",
            "note\n\"a,b\nc\"",
            "double_curly",
            "comma",
            "empty",
            true,
            "none",
        )
        .unwrap();
        assert_eq!(out, "a,b\nc");
    }

    #[test]
    fn form_feed_separator() {
        let out = merge(
            "{{x}}",
            "x\n1\n2",
            "double_curly",
            "comma",
            "empty",
            true,
            "form_feed",
        )
        .unwrap();
        assert_eq!(out, "1\u{000C}2");
    }

    #[test]
    fn short_row_yields_empty_cell() {
        // Second data row is missing the `city` value -> empty, not an error.
        let out = merge(
            "{{name}}: {{city}}",
            "name,city\nAda,Paris\nBob",
            "double_curly",
            "comma",
            "empty",
            true,
            "newline",
        )
        .unwrap();
        assert_eq!(out, "Ada: Paris\nBob: ");
    }

    #[test]
    fn empty_placeholder_and_stray_open_are_literal() {
        let out = merge(
            "a {{}} b {{ c",
            "z\n1",
            "double_curly",
            "comma",
            "empty",
            true,
            "none",
        )
        .unwrap();
        assert_eq!(out, "a {{}} b {{ c");
    }

    #[test]
    fn error_on_empty_csv() {
        assert!(merge("{{x}}", "   ", "double_curly", "comma", "empty", true, "none").is_err());
    }

    #[test]
    fn error_on_header_only() {
        let err = merge("{{x}}", "x", "double_curly", "comma", "empty", true, "none").unwrap_err();
        assert!(err.contains("no data rows"), "got: {err}");
    }

    #[test]
    fn error_on_bad_syntax_and_delimiter() {
        assert!(merge("{{x}}", "x\n1", "curly", "comma", "empty", true, "none").is_err());
        assert!(merge("{{x}}", "x\n1", "double_curly", "pipe", "empty", true, "none").is_err());
        assert!(merge("{{x}}", "x\n1", "double_curly", "comma", "skip", true, "none").is_err());
        assert!(merge("{{x}}", "x\n1", "double_curly", "comma", "empty", true, "hr").is_err());
    }

    #[test]
    fn cap_boundary_at_max_ok_over_errors() {
        let mut at = String::from("v");
        for i in 0..MAX_ROWS {
            at.push('\n');
            at.push_str(&i.to_string());
        }
        assert!(merge("{{v}}", &at, "double_curly", "comma", "empty", true, "newline").is_ok());
        // one more data row -> MAX_ROWS + 1 -> error
        let over = format!("{at}\n{MAX_ROWS}");
        let err = merge("{{v}}", &over, "double_curly", "comma", "empty", true, "newline")
            .unwrap_err();
        assert!(err.contains("too many data rows"), "got: {err}");
    }
}
