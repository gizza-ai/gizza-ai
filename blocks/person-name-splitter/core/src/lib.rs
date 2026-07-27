//! person-name-splitter core — pure compute, shared by the chat skill block and the web page.
//!
//! Split a CSV column of full person names into structured components — title,
//! first, middle, last, and suffix — using order-independent heuristics that
//! understand honorific titles (Mr, Ms, Mrs, Dr, Prof), generational/credential
//! suffixes (Jr, Sr, II, III, IV, PhD, MD), surname particles (van, von, de,
//! del, da, di, la, le, du, den, der), hyphenated and apostrophe names, and the
//! `Last, First Middle` comma form. Pure-Rust (`csv` + `serde`); no wafer or
//! wasm-bindgen deps.

use serde::Serialize;

/// Map a delimiter name to its byte.
fn delim_byte(d: &str) -> Result<u8, String> {
    Ok(match d.trim() {
        "" | "comma" | "," => b',',
        "tab" | "\t" | "\\t" => b'\t',
        "semicolon" | ";" => b';',
        "pipe" | "|" => b'|',
        other => {
            return Err(format!(
                "delimiter must be comma/tab/semicolon/pipe, got '{other}'"
            ))
        }
    })
}

/// The five components a full name is split into (plus an ambiguity flag used by
/// the summary report; it is never emitted as a CSV column).
#[derive(Debug, Clone, Default, PartialEq, Serialize)]
pub struct ParsedName {
    pub title: String,
    pub first: String,
    pub middle: String,
    pub last: String,
    pub suffix: String,
    /// True when a non-blank name could not be split into a first + last pair
    /// (e.g. a single mononym, or a title/suffix with no core name).
    #[serde(skip)]
    pub ambiguous: bool,
}

/// Honorific titles, matched case-insensitively after stripping a trailing dot.
const TITLES: &[&str] = &[
    "mr",
    "mrs",
    "ms",
    "miss",
    "mx",
    "dr",
    "prof",
    "professor",
    "sir",
    "madam",
    "rev",
    "fr",
    "capt",
    "col",
    "gen",
    "lt",
    "sgt",
    "hon",
];

/// Generational / credential suffixes. Roman numerals are matched as-is; the
/// alphabetic ones case-insensitively after stripping a trailing dot.
const SUFFIXES: &[&str] = &[
    "jr", "sr", "ii", "iii", "iv", "v", "phd", "md", "dds", "esq", "jd", "mba", "rn", "do",
];

/// Surname particles (nobiliary / tussenvoegsel). Lowercase, matched
/// case-insensitively; they attach to the following last-name token(s).
const PARTICLES: &[&str] = &[
    "van", "von", "de", "del", "della", "der", "den", "da", "di", "la", "le", "du", "dos", "das",
    "st", "mac", "mc", "bin", "al",
];

fn norm(tok: &str) -> String {
    tok.trim_end_matches('.').to_ascii_lowercase()
}

fn is_title(tok: &str) -> bool {
    TITLES.contains(&norm(tok).as_str())
}

fn is_suffix(tok: &str) -> bool {
    SUFFIXES.contains(&norm(tok).as_str())
}

fn is_particle(tok: &str) -> bool {
    PARTICLES.contains(&norm(tok).as_str())
}

/// Split a single full-name string into its components. Order-independent:
/// leading titles and trailing suffixes are peeled off first, then the core
/// name is assigned to first/middle/last with particle-aware surname grouping.
/// Also understands the `Last, First Middle [, Suffix]` comma form.
pub fn parse_name(raw: &str) -> ParsedName {
    let name = raw.trim();
    let mut out = ParsedName::default();
    if name.is_empty() {
        return out;
    }

    // ---- comma form: "Last, First Middle" or "Last, First, Suffix" ----
    if name.contains(',') {
        let mut segs = name.splitn(3, ',');
        let last_seg = segs.next().unwrap_or("").trim();
        let given_seg = segs.next().unwrap_or("").trim();
        let extra_seg = segs.next().unwrap_or("").trim();

        // The last name comes from the first segment (particles kept together).
        let last_toks: Vec<&str> = last_seg.split_whitespace().collect();
        // Given names + any trailing suffix segment.
        let mut given: Vec<&str> = given_seg.split_whitespace().collect();
        let extra: Vec<&str> = extra_seg.split_whitespace().collect();
        given.extend(extra.iter().copied());

        // Peel a leading title off the given names.
        let mut gi = 0;
        let mut titles: Vec<&str> = Vec::new();
        while gi < given.len() && is_title(given[gi]) {
            titles.push(given[gi]);
            gi += 1;
        }
        // Peel trailing suffixes.
        let mut gj = given.len();
        let mut suffixes: Vec<&str> = Vec::new();
        while gj > gi && is_suffix(given[gj - 1]) {
            suffixes.insert(0, given[gj - 1]);
            gj -= 1;
        }
        let core = &given[gi..gj];

        out.title = titles.join(" ");
        out.suffix = suffixes.join(" ");
        out.last = last_toks.join(" ");
        if !core.is_empty() {
            out.first = core[0].to_string();
            out.middle = core[1..].join(" ");
        }
        out.ambiguous = out.last.is_empty() != out.first.is_empty();
        return out;
    }

    // ---- natural order: "Title First Middle Last Suffix" ----
    let toks: Vec<&str> = name.split_whitespace().collect();
    let mut i = 0;
    let mut titles: Vec<&str> = Vec::new();
    while i < toks.len() && is_title(toks[i]) {
        titles.push(toks[i]);
        i += 1;
    }
    let mut j = toks.len();
    let mut suffixes: Vec<&str> = Vec::new();
    while j > i && is_suffix(toks[j - 1]) {
        suffixes.insert(0, toks[j - 1]);
        j -= 1;
    }
    out.title = titles.join(" ");
    out.suffix = suffixes.join(" ");

    let core = &toks[i..j];
    match core.len() {
        0 => {
            // Only a title and/or suffix — nothing to name.
            out.ambiguous = true;
        }
        1 => {
            // A single (mononym) token: treat as the first name, no surname.
            out.first = core[0].to_string();
            out.ambiguous = true;
        }
        _ => {
            // Find where the surname starts: the earliest particle at index >= 1
            // that is followed by another token begins a particle-led surname
            // (e.g. "Ludwig van Beethoven", "Juan Ponce de Leon"). Otherwise the
            // final token is the surname.
            let surname_start = (1..core.len())
                .find(|&k| is_particle(core[k]) && k + 1 < core.len())
                .unwrap_or(core.len() - 1);
            out.first = core[0].to_string();
            out.middle = core[1..surname_start].join(" ");
            out.last = core[surname_start..].join(" ");
        }
    }
    out
}

/// Resolve the target column: a header name (when `header`), a 1-based index, or
/// blank (the first column). A numeric `spec` is always a 1-based index.
fn resolve_column(spec: &str, columns: &[String], header: bool) -> Result<usize, String> {
    let spec = spec.trim();
    if spec.is_empty() {
        return Ok(0);
    }
    if let Ok(n) = spec.parse::<usize>() {
        if n < 1 {
            return Err("name_column index is 1-based; use 1 for the first column".into());
        }
        let idx = n - 1;
        if idx >= columns.len() {
            return Err(format!(
                "name_column index {n} out of range (1..={})",
                columns.len()
            ));
        }
        return Ok(idx);
    }
    if !header {
        return Err(format!(
            "with header off, name_column must be a 1-based index, got '{spec}'"
        ));
    }
    columns.iter().position(|c| c == spec).ok_or_else(|| {
        format!(
            "name_column '{spec}' not found in header ({})",
            columns.join(", ")
        )
    })
}

/// One ambiguously-parsed row, for the summary report.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AmbiguousRow {
    /// 1-based line number in the original CSV (header is line 1 when present).
    pub line: usize,
    /// 1-based index among DATA rows only (header excluded).
    pub row: usize,
    /// The original name value.
    pub value: String,
    /// Why it was flagged ambiguous.
    pub reason: String,
}

/// The summary report (JSON output mode).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Summary {
    pub column: String,
    pub column_index: usize,
    pub total_rows: usize,
    pub blank: usize,
    pub with_title: usize,
    pub with_middle: usize,
    pub with_last: usize,
    pub with_suffix: usize,
    pub ambiguous: usize,
    pub ambiguous_rows: Vec<AmbiguousRow>,
}

/// Split the chosen name column. `output` is `append`, `replace`, or `summary`.
pub fn run(
    data: &str,
    name_column: &str,
    output: &str,
    header: bool,
    delimiter: &str,
    trim: bool,
) -> Result<String, String> {
    if data.trim().is_empty() {
        return Err("input is empty".into());
    }
    let mode = match output.trim() {
        "" | "append" => "append",
        "replace" => "replace",
        "summary" => "summary",
        other => {
            return Err(format!(
                "unknown output '{other}'; expected append, replace, or summary"
            ))
        }
    };
    let delim = delim_byte(delimiter)?;

    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .has_headers(false)
        .flexible(true)
        .from_reader(data.as_bytes());

    let mut records: Vec<(usize, Vec<String>)> = Vec::new();
    let mut rec = csv::StringRecord::new();
    loop {
        match rdr.read_record(&mut rec) {
            Ok(true) => {
                let line = rec.position().map(|p| p.line() as usize).unwrap_or(0);
                let fields: Vec<String> = rec
                    .iter()
                    .map(|s| {
                        if trim {
                            s.trim().to_string()
                        } else {
                            s.to_string()
                        }
                    })
                    .collect();
                records.push((line, fields));
            }
            Ok(false) => break,
            Err(e) => return Err(format!("CSV parse error: {e}")),
        }
    }
    if records.is_empty() {
        return Err("no rows found".into());
    }

    let width = records.iter().map(|(_, r)| r.len()).max().unwrap_or(0);
    let columns: Vec<String> = if header {
        let first = &records[0].1;
        (0..width)
            .map(|i| {
                let n = first.get(i).map(|s| s.trim()).unwrap_or("");
                if n.is_empty() {
                    format!("col{}", i + 1)
                } else {
                    n.to_string()
                }
            })
            .collect()
    } else {
        (0..width).map(|i| format!("col{}", i + 1)).collect()
    };

    let col_idx = resolve_column(name_column, &columns, header)?;
    let col_name = &columns[col_idx];

    let data_rows: &[(usize, Vec<String>)] = if header { &records[1..] } else { &records[..] };

    if mode == "summary" {
        let mut s = Summary {
            column: col_name.clone(),
            column_index: col_idx,
            total_rows: data_rows.len(),
            blank: 0,
            with_title: 0,
            with_middle: 0,
            with_last: 0,
            with_suffix: 0,
            ambiguous: 0,
            ambiguous_rows: Vec::new(),
        };
        for (di, (line, row)) in data_rows.iter().enumerate() {
            let raw = row.get(col_idx).map(|s| s.as_str()).unwrap_or("");
            if raw.trim().is_empty() {
                s.blank += 1;
                continue;
            }
            let parsed = parse_name(raw);
            if !parsed.title.is_empty() {
                s.with_title += 1;
            }
            if !parsed.middle.is_empty() {
                s.with_middle += 1;
            }
            if !parsed.last.is_empty() {
                s.with_last += 1;
            }
            if !parsed.suffix.is_empty() {
                s.with_suffix += 1;
            }
            if parsed.ambiguous {
                s.ambiguous += 1;
                if s.ambiguous_rows.len() < 50 {
                    let reason = if parsed.last.is_empty() && !parsed.first.is_empty() {
                        "no surname could be identified (single name)".to_string()
                    } else if parsed.first.is_empty() {
                        "no given name could be identified".to_string()
                    } else {
                        "uncertain split".to_string()
                    };
                    s.ambiguous_rows.push(AmbiguousRow {
                        line: *line,
                        row: di + 1,
                        value: raw.to_string(),
                        reason,
                    });
                }
            }
        }
        return serde_json::to_string_pretty(&s).map_err(|e| e.to_string());
    }

    // append / replace: build a CSV with the five component columns.
    let comp_headers = [
        format!("{col_name}_title"),
        format!("{col_name}_first"),
        format!("{col_name}_middle"),
        format!("{col_name}_last"),
        format!("{col_name}_suffix"),
    ];

    let mut wtr = csv::WriterBuilder::new()
        .delimiter(delim)
        .flexible(true)
        .from_writer(vec![]);

    // Header row (only when the input has headers).
    if header {
        let mut hdr: Vec<String> = records[0].1.clone();
        // Pad a short header row to the full width so component columns line up.
        while hdr.len() < width {
            hdr.push(String::new());
        }
        let out_hdr = assemble_row(&hdr, col_idx, &comp_headers, mode);
        wtr.write_record(&out_hdr)
            .map_err(|e| format!("CSV write error: {e}"))?;
    }

    for (_, row) in data_rows {
        let raw = row.get(col_idx).map(|s| s.as_str()).unwrap_or("");
        let parsed = parse_name(raw);
        let comps = [
            parsed.title.clone(),
            parsed.first.clone(),
            parsed.middle.clone(),
            parsed.last.clone(),
            parsed.suffix.clone(),
        ];
        let out_row = assemble_row(row, col_idx, &comps, mode);
        wtr.write_record(&out_row)
            .map_err(|e| format!("CSV write error: {e}"))?;
    }

    let bytes = wtr
        .into_inner()
        .map_err(|e| format!("CSV write error: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("UTF-8 error: {e}"))
}

/// Build one output row. In `append` mode every original cell is kept in place
/// and the five component cells are added at the end. In `replace` mode the name
/// cell is swapped in place for the five component cells.
fn assemble_row(row: &[String], col_idx: usize, comps: &[String; 5], mode: &str) -> Vec<String> {
    if mode == "append" {
        let mut out: Vec<String> = row.to_vec();
        out.extend(comps.iter().cloned());
        return out;
    }
    // replace: substitute the name column in place for the components.
    let mut out: Vec<String> = Vec::with_capacity(row.len() + 5);
    let mut placed = false;
    for (ci, cell) in row.iter().enumerate() {
        if ci == col_idx {
            out.extend(comps.iter().cloned());
            placed = true;
        } else {
            out.push(cell.clone());
        }
    }
    if !placed {
        out.extend(comps.iter().cloned());
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(name: &str) -> ParsedName {
        parse_name(name)
    }

    // ---------- parse_name unit coverage ----------

    #[test]
    fn simple_first_last() {
        let n = p("John Smith");
        assert_eq!(n.first, "John");
        assert_eq!(n.last, "Smith");
        assert_eq!(n.middle, "");
        assert!(!n.ambiguous);
    }

    #[test]
    fn first_middle_last() {
        let n = p("John Michael Smith");
        assert_eq!(n.first, "John");
        assert_eq!(n.middle, "Michael");
        assert_eq!(n.last, "Smith");
    }

    #[test]
    fn title_and_suffix() {
        let n = p("Dr. John Smith Jr.");
        assert_eq!(n.title, "Dr.");
        assert_eq!(n.first, "John");
        assert_eq!(n.last, "Smith");
        assert_eq!(n.suffix, "Jr.");
    }

    #[test]
    fn multiple_suffixes() {
        let n = p("John Smith Jr PhD");
        assert_eq!(n.suffix, "Jr PhD");
        assert_eq!(n.last, "Smith");
    }

    #[test]
    fn roman_numeral_suffix() {
        let n = p("Henry Ford III");
        assert_eq!(n.last, "Ford");
        assert_eq!(n.suffix, "III");
    }

    #[test]
    fn particle_surname() {
        let n = p("Ludwig van Beethoven");
        assert_eq!(n.first, "Ludwig");
        assert_eq!(n.middle, "");
        assert_eq!(n.last, "van Beethoven");
    }

    #[test]
    fn particle_with_middle() {
        let n = p("Juan Ponce de Leon");
        assert_eq!(n.first, "Juan");
        assert_eq!(n.middle, "Ponce");
        assert_eq!(n.last, "de Leon");
    }

    #[test]
    fn comma_form() {
        let n = p("Smith, John Michael");
        assert_eq!(n.first, "John");
        assert_eq!(n.middle, "Michael");
        assert_eq!(n.last, "Smith");
    }

    #[test]
    fn comma_form_with_suffix_segment() {
        let n = p("Smith, John, Jr");
        assert_eq!(n.first, "John");
        assert_eq!(n.last, "Smith");
        assert_eq!(n.suffix, "Jr");
    }

    #[test]
    fn comma_form_particle_surname() {
        let n = p("van Beethoven, Ludwig");
        assert_eq!(n.first, "Ludwig");
        assert_eq!(n.last, "van Beethoven");
    }

    #[test]
    fn comma_form_title() {
        let n = p("Smith, Dr. John");
        assert_eq!(n.title, "Dr.");
        assert_eq!(n.first, "John");
        assert_eq!(n.last, "Smith");
    }

    #[test]
    fn hyphenated_last_name() {
        let n = p("Mary Smith-Jones");
        assert_eq!(n.first, "Mary");
        assert_eq!(n.last, "Smith-Jones");
    }

    #[test]
    fn apostrophe_name() {
        let n = p("Conan O'Brien");
        assert_eq!(n.first, "Conan");
        assert_eq!(n.last, "O'Brien");
    }

    #[test]
    fn mononym_is_ambiguous() {
        let n = p("Cher");
        assert_eq!(n.first, "Cher");
        assert_eq!(n.last, "");
        assert!(n.ambiguous);
    }

    #[test]
    fn blank_name_is_empty() {
        let n = p("   ");
        assert_eq!(n, ParsedName::default());
    }

    // ---------- run() integration ----------

    const CSV: &str = "name,age\nJohn Michael Smith,42\nCher,30\nDr. Jane Doe MD,55";

    #[test]
    fn append_keeps_original_and_adds_components() {
        let out = run(CSV, "name", "append", true, "comma", true).unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(
            lines[0],
            "name,age,name_title,name_first,name_middle,name_last,name_suffix"
        );
        assert_eq!(lines[1], "John Michael Smith,42,,John,Michael,Smith,");
        assert_eq!(lines[3], "Dr. Jane Doe MD,55,Dr.,Jane,,Doe,MD");
    }

    #[test]
    fn replace_swaps_the_name_column() {
        let out = run(CSV, "name", "replace", true, "comma", true).unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(
            lines[0],
            "name_title,name_first,name_middle,name_last,name_suffix,age"
        );
        assert_eq!(lines[1], ",John,Michael,Smith,,42");
    }

    #[test]
    fn summary_counts_and_ambiguous() {
        let out = run(CSV, "name", "summary", true, "comma", true).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["total_rows"], 3);
        assert_eq!(v["with_title"], 1);
        assert_eq!(v["with_suffix"], 1);
        assert_eq!(v["with_middle"], 1);
        assert_eq!(v["ambiguous"], 1);
        assert_eq!(v["ambiguous_rows"][0]["value"], "Cher");
    }

    #[test]
    fn blank_name_column_defaults_to_first() {
        let out = run(CSV, "", "append", true, "comma", true).unwrap();
        assert!(out
            .lines()
            .next()
            .unwrap()
            .starts_with("name,age,name_title"));
    }

    #[test]
    fn one_based_index_selects_column() {
        let data = "id,full\n1,John Smith";
        let out = run(data, "2", "append", true, "comma", true).unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(
            lines[0],
            "id,full,full_title,full_first,full_middle,full_last,full_suffix"
        );
        assert_eq!(lines[1], "1,John Smith,,John,,Smith,");
    }

    #[test]
    fn headerless_uses_index() {
        let data = "John Smith,42\nJane de Vries,30";
        let out = run(data, "1", "append", false, "comma", true).unwrap();
        let lines: Vec<&str> = out.lines().collect();
        // No header row emitted; components appended.
        assert_eq!(lines[0], "John Smith,42,,John,,Smith,");
        assert_eq!(lines[1], "Jane de Vries,30,,Jane,,de Vries,");
    }

    #[test]
    fn tab_delimiter_roundtrips() {
        let data = "name\tage\nJohn Smith\t42";
        let out = run(data, "name", "append", true, "tab", true).unwrap();
        assert!(out.contains("John Smith\t42\t\tJohn\t\tSmith\t"));
    }

    #[test]
    fn semicolon_delimiter() {
        let data = "name;age\nMary Smith-Jones;40";
        let out = run(data, "name", "append", true, "semicolon", true).unwrap();
        assert!(out.contains("Mary Smith-Jones;40;;Mary;;Smith-Jones;"));
    }

    #[test]
    fn pipe_delimiter() {
        let data = "name|age\nJohn Smith|1";
        let out = run(data, "name", "append", true, "pipe", true).unwrap();
        assert!(out.contains("John Smith|1||John||Smith|"));
    }

    #[test]
    fn trim_off_preserves_padding_in_other_columns() {
        // csv keeps the leading spaces in the note cell when trim is off.
        let data = "name,note\nJohn Smith,  hi";
        let out = run(data, "name", "append", true, "comma", false).unwrap();
        assert!(out.contains(",  hi,"), "{out}");
        let on = run(data, "name", "append", true, "comma", true).unwrap();
        assert!(on.contains(",hi,"), "{on}");
    }

    #[test]
    fn blank_name_row_yields_empty_components() {
        let data = "name,age\n,42\nJohn Smith,30";
        let out = run(data, "name", "append", true, "comma", true).unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[1], ",42,,,,,");
    }

    // ---------- error paths ----------

    #[test]
    fn empty_input_errors() {
        assert!(run("", "name", "append", true, "comma", true).is_err());
    }

    #[test]
    fn unknown_output_errors() {
        let err = run(CSV, "name", "xml", true, "comma", true).unwrap_err();
        assert!(err.contains("unknown output"), "{err}");
    }

    #[test]
    fn unknown_delimiter_errors() {
        let err = run(CSV, "name", "append", true, "colon", true).unwrap_err();
        assert!(err.contains("delimiter must be"), "{err}");
    }

    #[test]
    fn unknown_column_errors() {
        let err = run(CSV, "nope", "append", true, "comma", true).unwrap_err();
        assert!(err.contains("not found in header"), "{err}");
    }

    #[test]
    fn zero_index_errors() {
        let err = run(CSV, "0", "append", true, "comma", true).unwrap_err();
        assert!(err.contains("1-based"), "{err}");
    }

    #[test]
    fn index_out_of_range_errors() {
        let err = run("a,b\n1,2", "9", "append", true, "comma", true).unwrap_err();
        assert!(err.contains("out of range"), "{err}");
    }

    #[test]
    fn header_off_requires_index() {
        let err = run("John Smith,1", "name", "append", false, "comma", true).unwrap_err();
        assert!(err.contains("must be a 1-based index"), "{err}");
    }
}
