//! gizza-ai/fixed-width-to-csv core — split fixed-width / column-positional text
//! records into CSV, using either an explicit column spec or boundaries inferred
//! from the whitespace that lines up across every row.
//!
//! Pure-Rust (`csv` for RFC-4180 quoting). No wafer/wasm-bindgen deps.
//!
//! Positions and widths are counted in **characters** (not bytes), so accented
//! and CJK text keeps working — one `char` is one column position.

/// Hard cap on the number of data lines processed in one run.
pub const MAX_LINES: usize = 50_000;
/// Hard cap on the number of columns a spec (or auto-detect) may produce.
pub const MAX_FIELDS: usize = 512;

fn delim_byte(d: &str) -> Result<u8, String> {
    Ok(match d {
        "" | "," | "comma" => b',',
        "\t" | "tab" | "\\t" => b'\t',
        ";" | "semicolon" => b';',
        "|" | "pipe" => b'|',
        " " | "space" => b' ',
        ":" | "colon" => b':',
        other => {
            let b = other.as_bytes();
            if b.len() == 1 {
                b[0]
            } else {
                return Err(format!(
                    "delimiter must be a single character or one of comma/tab/semicolon/pipe/space/colon, got '{other}'"
                ));
            }
        }
    })
}

/// CSV quoting policy for the output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Quote {
    /// Quote a field only when it contains the delimiter, a quote, or a newline.
    Minimal,
    /// Wrap every field in double quotes.
    All,
    /// Never quote. Fast but lossy — a field containing the delimiter will
    /// re-import as two fields.
    Never,
}

impl Quote {
    pub fn parse(s: &str) -> Result<Quote, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "minimal" | "necessary" | "auto" => Ok(Quote::Minimal),
            "all" | "always" => Ok(Quote::All),
            "never" | "none" => Ok(Quote::Never),
            other => Err(format!(
                "unknown quote style '{other}' (use minimal, all, or never)"
            )),
        }
    }
    fn style(self) -> csv::QuoteStyle {
        match self {
            Quote::Minimal => csv::QuoteStyle::Necessary,
            Quote::All => csv::QuoteStyle::Always,
            Quote::Never => csv::QuoteStyle::Never,
        }
    }
}

/// Where the CSV header row comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HeaderMode {
    /// The first data line holds the column titles (default).
    FirstRow,
    /// Take the titles from the names in the column spec (`name:10,age:3`).
    SpecNames,
    /// Emit `col1,col2,…`.
    Generate,
    /// No header row — data rows only.
    NoHeader,
}

impl HeaderMode {
    pub fn parse(s: &str) -> Result<HeaderMode, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "first-row" | "first_row" | "firstrow" | "first" => Ok(HeaderMode::FirstRow),
            "names" | "spec" | "spec-names" => Ok(HeaderMode::SpecNames),
            "generate" | "generated" | "auto" => Ok(HeaderMode::Generate),
            "none" | "no" | "off" => Ok(HeaderMode::NoHeader),
            other => Err(format!(
                "unknown header mode '{other}' (use first-row, names, generate, or none)"
            )),
        }
    }
}

/// One output column: a half-open character range `[start, end)` of each line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    /// Column title from the spec, if one was given.
    pub name: Option<String>,
    /// 0-based, inclusive.
    pub start: usize,
    /// 0-based, exclusive. `None` = "to the end of the line".
    pub end: Option<usize>,
}

/// Everything the converter can be told to do besides the input text itself.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    /// Column spec; empty = auto-detect from aligned whitespace. See [`parse_spec`].
    pub spec: String,
    /// Auto-detect only: how many aligned padding characters in a row make a
    /// column separator. 1 (default) splits on any single aligned space; 2+
    /// keeps values with single internal spaces together. Ignored when `spec`
    /// is given.
    pub min_gap: usize,
    pub header: HeaderMode,
    /// Strip leading/trailing whitespace from every extracted field (default true).
    pub trim: bool,
    /// Output field separator (single char or comma/tab/semicolon/pipe/space/colon).
    pub delimiter: String,
    pub quote: Quote,
    /// Use `\r\n` instead of `\n` between output rows.
    pub crlf: bool,
    /// Prepend a UTF-8 byte-order mark.
    pub bom: bool,
    /// Drop this many lines from the top of the input before anything else.
    pub skip_lines: usize,
    /// Lines whose first non-space characters are this prefix are ignored (empty = off).
    pub comment: String,
    /// Drop blank / whitespace-only lines (default true).
    pub skip_blank: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            spec: String::new(),
            min_gap: 1,
            header: HeaderMode::FirstRow,
            trim: true,
            delimiter: ",".to_string(),
            quote: Quote::Minimal,
            crlf: false,
            bom: false,
            skip_lines: 0,
            comment: String::new(),
            skip_blank: true,
        }
    }
}

fn parse_pos(tok: &str, what: &str, entry: &str) -> Result<usize, String> {
    let n: usize = tok.trim().parse().map_err(|_| {
        format!(
            "spec column '{entry}': {what} must be a whole number, got '{}'",
            tok.trim()
        )
    })?;
    if n == 0 {
        return Err(format!(
            "spec column '{entry}': {what} is 1-based, so it must be 1 or more, got 0"
        ));
    }
    Ok(n)
}

/// `pos,len[,name]` — the pipe-separated form (`1,10,name|11,8,dob`).
/// A length of `*` or `0` means "to the end of the line".
fn parse_triple(entry: &str) -> Result<Field, String> {
    let parts: Vec<&str> = entry.split(',').collect();
    if parts.len() < 2 || parts.len() > 3 {
        return Err(format!(
            "spec column '{entry}': expected 'position,length' or 'position,length,name' when the spec is pipe-separated"
        ));
    }
    let start = parse_pos(parts[0], "position", entry)? - 1;
    let len_tok = parts[1].trim();
    let end = if len_tok == "*" || len_tok == "0" {
        None
    } else {
        let len: usize = len_tok.parse().map_err(|_| {
            format!("spec column '{entry}': length must be a whole number or '*', got '{len_tok}'")
        })?;
        Some(start + len)
    };
    let name = parts
        .get(2)
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty());
    Ok(Field { name, start, end })
}

/// `N`, `*`, `A-B`, or any of those prefixed with `name:`.
fn parse_simple(entry: &str, cursor: usize) -> Result<Field, String> {
    let (name, body) = match entry.split_once(':') {
        Some((n, rest)) => {
            let n = n.trim();
            if n.is_empty() {
                return Err(format!(
                    "spec column '{entry}': the name before ':' is empty (write 'name:width')"
                ));
            }
            (Some(n.to_string()), rest.trim())
        }
        None => (None, entry),
    };
    if body == "*" {
        return Ok(Field {
            name,
            start: cursor,
            end: None,
        });
    }
    if let Some((a, b)) = body.split_once('-') {
        let start1 = parse_pos(a, "range start", entry)?;
        let end1 = parse_pos(b, "range end", entry)?;
        if end1 < start1 {
            return Err(format!(
                "spec column '{entry}': range end {end1} is before range start {start1}"
            ));
        }
        return Ok(Field {
            name,
            start: start1 - 1,
            end: Some(end1),
        });
    }
    let width: usize = body.trim().parse().map_err(|_| {
        format!(
            "spec column '{entry}': expected a width (10), a range (1-10), '*', or 'name:width', got '{body}'"
        )
    })?;
    if width == 0 {
        return Err(format!(
            "spec column '{entry}': width must be 1 or more (use '*' for the rest of the line)"
        ));
    }
    Ok(Field {
        name,
        start: cursor,
        end: Some(cursor + width),
    })
}

/// Parse a column spec into fields.
///
/// Two forms are accepted:
/// - **Pipe-separated** (used as soon as the spec contains a `|`): each entry is
///   `position,length[,name]` with a **1-based** position, e.g.
///   `1,10,name|11,8,dob|19,*,notes`.
/// - **Comma-separated** otherwise: each entry is a width (`10`), a 1-based
///   inclusive range (`1-10`), `*` for the rest of the line, or any of those with
///   a `name:` prefix (`name:10`, `dob:11-18`, `notes:*`). Bare widths run
///   consecutively from position 1, so `10,8,12` means columns 1-10, 11-18, 19-30.
pub fn parse_spec(spec: &str) -> Result<Vec<Field>, String> {
    let s = spec.trim();
    if s.is_empty() {
        return Err("column spec is empty (leave it blank to auto-detect columns)".into());
    }
    let piped = s.contains('|');
    let parts: Vec<&str> = if piped {
        s.split('|').collect()
    } else {
        s.split(',').collect()
    };
    let mut fields: Vec<Field> = Vec::new();
    let mut cursor = 0usize;
    for raw in parts {
        let entry = raw.trim();
        if entry.is_empty() {
            continue;
        }
        if let Some(last) = fields.last() {
            if last.end.is_none() {
                return Err(format!(
                    "spec column '{entry}': '*' (rest of the line) must be the last column"
                ));
            }
        }
        if fields.len() >= MAX_FIELDS {
            return Err(format!("column spec has more than {MAX_FIELDS} columns"));
        }
        let f = if piped {
            parse_triple(entry)?
        } else {
            parse_simple(entry, cursor)?
        };
        cursor = f.end.unwrap_or(cursor);
        fields.push(f);
    }
    if fields.is_empty() {
        return Err("column spec has no columns".into());
    }
    Ok(fields)
}

/// True when this character acts as column padding for boundary detection.
fn is_filler(c: char) -> bool {
    c == ' ' || c == '\t'
}

/// Infer the columns: every character position where EVERY line has padding (or
/// has already ended) is a candidate boundary. Only a *run* of at least
/// `min_gap` such positions actually separates two columns — a narrower run is
/// treated as ordinary field content, which is what keeps a value with a single
/// internal space (`Grace Hop`) in one piece when it happens to line up with
/// padding on every other line. The runs in between are the columns, and the
/// last column is left open-ended so nothing is clipped off a long line.
fn auto_detect(lines: &[Vec<char>], min_gap: usize) -> Result<Vec<Field>, String> {
    let width = lines.iter().map(Vec::len).max().unwrap_or(0);
    let min_gap = min_gap.max(1);
    // Candidate boundary columns: padding (or past end-of-line) on every line.
    let candidate: Vec<bool> = (0..width)
        .map(|c| lines.iter().all(|l| c >= l.len() || is_filler(l[c])))
        .collect();
    // Promote a candidate to a real separator only if its maximal run is wide
    // enough. A run that reaches the end of the widest line always counts: it is
    // trailing padding, never content.
    let mut separator = vec![false; width];
    let mut c = 0;
    while c < width {
        if !candidate[c] {
            c += 1;
            continue;
        }
        let run_start = c;
        while c < width && candidate[c] {
            c += 1;
        }
        if c - run_start >= min_gap || c == width {
            separator[run_start..c].fill(true);
        }
    }
    let mut fields: Vec<Field> = Vec::new();
    let mut start: Option<usize> = None;
    for c in 0..width {
        if separator[c] {
            if let Some(s) = start.take() {
                fields.push(Field {
                    name: None,
                    start: s,
                    end: Some(c),
                });
            }
        } else if start.is_none() {
            start = Some(c);
        }
    }
    if let Some(s) = start.take() {
        fields.push(Field {
            name: None,
            start: s,
            end: None,
        });
    }
    if fields.is_empty() {
        return Err(
            "could not detect any columns — every line looks blank. Paste aligned text or give an explicit column spec."
                .into(),
        );
    }
    if fields.len() > MAX_FIELDS {
        return Err(format!(
            "detected {} columns, which is over the cap of {MAX_FIELDS}",
            fields.len()
        ));
    }
    // Never clip the right-hand column: let it run to the end of each line.
    if let Some(last) = fields.last_mut() {
        last.end = None;
    }
    Ok(fields)
}

fn extract(line: &[char], f: &Field, trim: bool) -> String {
    let start = f.start.min(line.len());
    let end = f.end.unwrap_or(line.len()).min(line.len());
    let s: String = if end > start {
        line[start..end].iter().collect()
    } else {
        String::new()
    };
    if trim {
        s.trim().to_string()
    } else {
        s
    }
}

/// Convert fixed-width text to CSV with the default options (auto-detected
/// columns, first line as the header, trimmed fields, comma delimiter).
pub fn to_csv(input: &str) -> Result<String, String> {
    convert(input, &Options::default())
}

/// Convert fixed-width / column-positional text into CSV.
pub fn convert(input: &str, o: &Options) -> Result<String, String> {
    if input.trim().is_empty() {
        return Err("input is empty — paste some fixed-width text first".into());
    }
    let delim = delim_byte(&o.delimiter)?;
    let comment = o.comment.trim();

    let mut lines: Vec<Vec<char>> = Vec::new();
    for raw in input.lines().skip(o.skip_lines) {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        if !comment.is_empty() && line.trim_start().starts_with(comment) {
            continue;
        }
        if o.skip_blank && line.trim().is_empty() {
            continue;
        }
        lines.push(line.chars().collect());
        if lines.len() > MAX_LINES {
            return Err(format!(
                "too many lines: this tool converts at most {MAX_LINES} data lines per run"
            ));
        }
    }
    if lines.is_empty() {
        return Err(
            "no data lines left — everything was skipped by skip_lines, the comment prefix, or the blank-line filter"
                .into(),
        );
    }

    let fields = if o.spec.trim().is_empty() {
        auto_detect(&lines, o.min_gap)?
    } else {
        parse_spec(&o.spec)?
    };

    let mut rows: Vec<Vec<String>> = lines
        .iter()
        .map(|l| fields.iter().map(|f| extract(l, f, o.trim)).collect())
        .collect();

    let header: Option<Vec<String>> = match o.header {
        HeaderMode::FirstRow => {
            let first = rows.remove(0);
            if rows.is_empty() {
                return Err(
                    "only one line was found and it was used as the header — nothing left to convert. Set the header option to 'generate' or 'none' to treat it as data."
                        .into(),
                );
            }
            Some(
                first
                    .iter()
                    .enumerate()
                    .map(|(i, v)| {
                        let t = v.trim();
                        if t.is_empty() {
                            format!("col{}", i + 1)
                        } else {
                            t.to_string()
                        }
                    })
                    .collect(),
            )
        }
        HeaderMode::SpecNames => {
            if fields.iter().all(|f| f.name.is_none()) {
                return Err(
                    "header 'names' needs a column spec that carries names, e.g. 'name:10,age:3' or '1,10,name|11,3,age' — the current spec has none"
                        .into(),
                );
            }
            Some(
                fields
                    .iter()
                    .enumerate()
                    .map(|(i, f)| f.name.clone().unwrap_or_else(|| format!("col{}", i + 1)))
                    .collect(),
            )
        }
        HeaderMode::Generate => Some((1..=fields.len()).map(|i| format!("col{i}")).collect()),
        HeaderMode::NoHeader => None,
    };

    let terminator = if o.crlf {
        csv::Terminator::CRLF
    } else {
        csv::Terminator::Any(b'\n')
    };
    let mut wtr = csv::WriterBuilder::new()
        .delimiter(delim)
        .quote_style(o.quote.style())
        .terminator(terminator)
        .flexible(false)
        .from_writer(vec![]);
    if let Some(h) = &header {
        wtr.write_record(h)
            .map_err(|e| format!("CSV write error: {e}"))?;
    }
    for row in &rows {
        wtr.write_record(row)
            .map_err(|e| format!("CSV write error: {e}"))?;
    }
    let bytes = wtr
        .into_inner()
        .map_err(|e| format!("CSV finalize error: {e}"))?;
    let out = String::from_utf8(bytes).map_err(|e| format!("CSV utf8 error: {e}"))?;
    let out = out.trim_end_matches(['\r', '\n']).to_string();
    Ok(if o.bom { format!("\u{feff}{out}") } else { out })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "name      age city\nAda        36 London\nBo          7 Oslo";

    #[test]
    fn auto_detect_happy_path() {
        let out = to_csv(SAMPLE).unwrap();
        assert_eq!(out, "name,age,city\nAda,36,London\nBo,7,Oslo");
    }

    #[test]
    fn explicit_widths_are_consecutive() {
        // 10 + 4 + rest => cols 1-10, 11-14, 15-end
        let o = Options {
            spec: "10,4,*".into(),
            ..Default::default()
        };
        let out = convert(SAMPLE, &o).unwrap();
        assert_eq!(out, "name,age,city\nAda,36,London\nBo,7,Oslo");
    }

    #[test]
    fn range_spec_is_one_based_inclusive() {
        let o = Options {
            spec: "1-10,11-14,15-25".into(),
            ..Default::default()
        };
        let out = convert(SAMPLE, &o).unwrap();
        assert_eq!(out, "name,age,city\nAda,36,London\nBo,7,Oslo");
    }

    #[test]
    fn named_spec_with_names_header() {
        let o = Options {
            spec: "who:10,years:4,town:*".into(),
            header: HeaderMode::SpecNames,
            ..Default::default()
        };
        let out = convert("Ada        36 London\nBo          7 Oslo", &o).unwrap();
        assert_eq!(out, "who,years,town\nAda,36,London\nBo,7,Oslo");
    }

    #[test]
    fn pipe_triple_spec_position_length_name() {
        let o = Options {
            spec: "1,10,who|11,4,years|15,*,town".into(),
            header: HeaderMode::SpecNames,
            ..Default::default()
        };
        let out = convert("Ada        36 London", &o).unwrap();
        assert_eq!(out, "who,years,town\nAda,36,London");
    }

    #[test]
    fn generated_and_no_header() {
        let o = Options {
            header: HeaderMode::Generate,
            ..Default::default()
        };
        let out = convert("Ada  36\nBo    7", &o).unwrap();
        assert_eq!(out, "col1,col2\nAda,36\nBo,7");

        let o = Options {
            header: HeaderMode::NoHeader,
            ..Default::default()
        };
        let out = convert("Ada  36\nBo    7", &o).unwrap();
        assert_eq!(out, "Ada,36\nBo,7");
    }

    #[test]
    fn trim_off_keeps_the_original_padding() {
        let o = Options {
            spec: "5,*".into(),
            trim: false,
            header: HeaderMode::NoHeader,
            ..Default::default()
        };
        let out = convert("Ada  36", &o).unwrap();
        assert_eq!(out, "Ada  ,36");
    }

    #[test]
    fn short_lines_pad_with_empty_fields() {
        let o = Options {
            spec: "4,4,4".into(),
            header: HeaderMode::NoHeader,
            ..Default::default()
        };
        let out = convert("ab", &o).unwrap();
        assert_eq!(out, "ab,,");
    }

    #[test]
    fn skip_lines_and_comment_prefix() {
        let input = "REPORT 2026\n# generated\nAda  36\nBo    7";
        let o = Options {
            skip_lines: 1,
            comment: "#".into(),
            header: HeaderMode::NoHeader,
            ..Default::default()
        };
        let out = convert(input, &o).unwrap();
        assert_eq!(out, "Ada,36\nBo,7");
    }

    #[test]
    fn blank_lines_kept_when_skip_blank_off() {
        let o = Options {
            spec: "3,*".into(),
            skip_blank: false,
            header: HeaderMode::NoHeader,
            ..Default::default()
        };
        let out = convert("Ada 36\n\nBo   7", &o).unwrap();
        assert_eq!(out, "Ada,36\n,\nBo,7");
    }

    #[test]
    fn delimiters_quoting_crlf_and_bom() {
        let o = Options {
            delimiter: "tab".into(),
            ..Default::default()
        };
        assert_eq!(
            convert(SAMPLE, &o).unwrap(),
            "name\tage\tcity\nAda\t36\tLondon\nBo\t7\tOslo"
        );

        let o = Options {
            quote: Quote::All,
            header: HeaderMode::NoHeader,
            ..Default::default()
        };
        assert_eq!(convert("Ada  36", &o).unwrap(), "\"Ada\",\"36\"");

        let o = Options {
            crlf: true,
            header: HeaderMode::NoHeader,
            ..Default::default()
        };
        assert_eq!(convert("Ada  36\nBo    7", &o).unwrap(), "Ada,36\r\nBo,7");

        let o = Options {
            bom: true,
            header: HeaderMode::NoHeader,
            ..Default::default()
        };
        assert!(convert("Ada  36", &o).unwrap().starts_with('\u{feff}'));
    }

    #[test]
    fn minimal_quoting_protects_embedded_delimiter_and_never_does_not() {
        let o = Options {
            spec: "12,*".into(),
            header: HeaderMode::NoHeader,
            ..Default::default()
        };
        assert_eq!(
            convert("Paris, FR   capital", &o).unwrap(),
            "\"Paris, FR\",capital"
        );

        let o = Options {
            spec: "12,*".into(),
            header: HeaderMode::NoHeader,
            quote: Quote::Never,
            ..Default::default()
        };
        assert_eq!(
            convert("Paris, FR   capital", &o).unwrap(),
            "Paris, FR,capital"
        );
    }

    #[test]
    fn multibyte_positions_count_characters_not_bytes() {
        // "José" is 5 bytes but 4 chars; a width of 6 must still split cleanly.
        let o = Options {
            spec: "6,*".into(),
            header: HeaderMode::NoHeader,
            ..Default::default()
        };
        assert_eq!(convert("José  Paris", &o).unwrap(), "José,Paris");
    }

    #[test]
    fn line_cap_boundary() {
        let one_line = "ab  cd\n";
        let at_cap: String = one_line.repeat(MAX_LINES);
        let o = Options {
            header: HeaderMode::NoHeader,
            ..Default::default()
        };
        let out = convert(&at_cap, &o).unwrap();
        assert_eq!(out.lines().count(), MAX_LINES);

        let over_cap: String = one_line.repeat(MAX_LINES + 1);
        let err = convert(&over_cap, &o).unwrap_err();
        assert!(err.contains("too many lines"), "got: {err}");
    }

    #[test]
    fn errors() {
        // Empty input.
        assert!(to_csv("   ").is_err());
        // One line consumed as the header leaves nothing.
        assert!(to_csv("Ada  36").unwrap_err().contains("only one line"));
        // Bad spec entries.
        assert!(parse_spec("10,abc")
            .unwrap_err()
            .contains("expected a width"));
        assert!(parse_spec("10-2")
            .unwrap_err()
            .contains("before range start"));
        assert!(parse_spec("0-4").unwrap_err().contains("1-based"));
        assert!(parse_spec("*,4")
            .unwrap_err()
            .contains("must be the last column"));
        assert!(parse_spec(":10")
            .unwrap_err()
            .contains("name before ':' is empty"));
        assert!(parse_spec("1,10,a,b|3,4")
            .unwrap_err()
            .contains("expected 'position,length'"));
        assert!(parse_spec("   ").is_err());
        // Unknown option values.
        assert!(Quote::parse("fancy").is_err());
        assert!(HeaderMode::parse("maybe").is_err());
        assert!(delim_byte("nope").is_err());
        // 'names' header without any named column.
        let o = Options {
            spec: "3,*".into(),
            header: HeaderMode::SpecNames,
            ..Default::default()
        };
        assert!(convert("Ada 36", &o)
            .unwrap_err()
            .contains("needs a column spec"));
        // Everything filtered out.
        let o = Options {
            skip_lines: 5,
            ..Default::default()
        };
        assert!(convert("a  b\nc  d", &o)
            .unwrap_err()
            .contains("no data lines left"));
    }
}
