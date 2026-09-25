//! ris-bibtex-converter core — convert bibliographic records between RIS
//! (`TY  - JOUR` … `ER  - `) and BibTeX (`@article{key, …}`) in both directions.
//!
//! Pure Rust. The RIS tokenizer, the type/field maps, the cite-key generator and
//! the two writers are hand-rolled here; the BibTeX *reader* (tokenizer,
//! `@string` expander, LaTeX accent decoder, name splitter) is reused from the
//! sibling `bibtex-to-csv` core rather than duplicated, so both tools agree on
//! what a `.bib` file means. Shared by the chat skill block, the CLI and the page.

use gizza_ai_bibtex_to_csv_core as bib;

/// Largest input accepted, in bytes. Above this a browser tab stalls long enough
/// to look hung, so it is a hard, documented error instead.
pub const MAX_INPUT_BYTES: usize = 1_000_000;

/// Largest BibTeX field indentation accepted.
pub const MAX_INDENT: u32 = 16;

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

/// Which way to convert.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Direction {
    /// Sniff the input and pick the other format as the output.
    Auto,
    RisToBibtex,
    BibtexToRis,
}

impl Direction {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Ok(Direction::Auto),
            "ris-to-bibtex" | "ris2bib" => Ok(Direction::RisToBibtex),
            "bibtex-to-ris" | "bib2ris" => Ok(Direction::BibtexToRis),
            other => Err(format!(
                "invalid direction '{other}': expected one of auto, ris-to-bibtex, bibtex-to-ris"
            )),
        }
    }
}

/// How a BibTeX cite key is invented for a RIS record (RIS carries no cite key).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum KeyStyle {
    AuthorYearWord,
    AuthorYear,
    RisId,
    Numeric,
}

impl KeyStyle {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "author-year-word" => Ok(KeyStyle::AuthorYearWord),
            "author-year" => Ok(KeyStyle::AuthorYear),
            "ris-id" => Ok(KeyStyle::RisId),
            "numeric" => Ok(KeyStyle::Numeric),
            other => Err(format!(
                "invalid key_style '{other}': expected one of author-year-word, author-year, ris-id, numeric"
            )),
        }
    }
}

/// Order of the emitted records.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Sort {
    Source,
    Key,
    Year,
    Type,
}

impl Sort {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "source" => Ok(Sort::Source),
            "key" => Ok(Sort::Key),
            "year" => Ok(Sort::Year),
            "type" => Ok(Sort::Type),
            other => Err(format!(
                "invalid sort '{other}': expected one of source, key, year, type"
            )),
        }
    }
}

/// Parse the BibTeX field indentation. Blank means the default of 2.
pub fn parse_indent(s: &str) -> Result<u32, String> {
    let t = s.trim();
    if t.is_empty() {
        return Ok(2);
    }
    let n: u32 = t.parse().map_err(|_| {
        format!("invalid indent '{t}': expected a whole number of spaces, 0-{MAX_INDENT}")
    })?;
    if n > MAX_INDENT {
        return Err(format!(
            "invalid indent '{t}': expected a whole number of spaces, 0-{MAX_INDENT}"
        ));
    }
    Ok(n)
}

// ---------------------------------------------------------------------------
// Public façade
// ---------------------------------------------------------------------------

/// One parsed RIS record: its tags in source order, values already unfolded.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct RisRecord {
    pub tags: Vec<(String, String)>,
}

impl RisRecord {
    fn first(&self, tag: &str) -> Option<&str> {
        self.tags
            .iter()
            .find(|(t, v)| t == tag && !v.is_empty())
            .map(|(_, v)| v.as_str())
    }
    fn first_of(&self, tags: &[&str]) -> Option<&str> {
        tags.iter().find_map(|t| self.first(t))
    }
    fn all(&self, tag: &str) -> Vec<&str> {
        self.tags
            .iter()
            .filter(|(t, v)| t == tag && !v.is_empty())
            .map(|(_, v)| v.as_str())
            .collect()
    }
    fn all_of(&self, tags: &[&str]) -> Vec<&str> {
        self.tags
            .iter()
            .filter(|(t, v)| tags.contains(&t.as_str()) && !v.is_empty())
            .map(|(_, v)| v.as_str())
            .collect()
    }
}

/// Convert between RIS and BibTeX. String-typed façade shared by every surface.
///
/// - `input`: the RIS or BibTeX source. At most [`MAX_INPUT_BYTES`] bytes.
/// - `direction`: `auto` (default) | `ris-to-bibtex` | `bibtex-to-ris`.
/// - `key_style`: `author-year-word` (default) | `author-year` | `ris-id` |
///   `numeric` — only used when writing BibTeX.
/// - `include_abstract`: carry the abstract across (`AB`/`N2` ↔ `abstract`).
/// - `include_keywords`: carry keywords across (`KW` ↔ `keywords`).
/// - `translate_latex`: BibTeX→RIS decodes LaTeX accents/braces to UTF-8;
///   RIS→BibTeX escapes LaTeX-special characters so the `.bib` compiles.
/// - `indent`: spaces before each BibTeX field line, 0-16 (default 2).
/// - `sort`: `source` (default) | `key` | `year` | `type`.
///
/// Returns `Err` with a human-readable message on an unknown option value, an
/// over-cap input, an unrecognisable input format, or a record-less input.
#[allow(clippy::too_many_arguments)]
pub fn convert_str(
    input: &str,
    direction: &str,
    key_style: &str,
    include_abstract: bool,
    include_keywords: bool,
    translate_latex: bool,
    indent: &str,
    sort: &str,
) -> Result<String, String> {
    convert(
        input,
        Direction::parse(direction)?,
        KeyStyle::parse(key_style)?,
        include_abstract,
        include_keywords,
        translate_latex,
        parse_indent(indent)?,
        Sort::parse(sort)?,
    )
}

/// Typed conversion entry point.
#[allow(clippy::too_many_arguments)]
pub fn convert(
    input: &str,
    direction: Direction,
    key_style: KeyStyle,
    include_abstract: bool,
    include_keywords: bool,
    translate_latex: bool,
    indent: u32,
    sort: Sort,
) -> Result<String, String> {
    if input.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "input is {} bytes, which is over the {MAX_INPUT_BYTES}-byte limit",
            input.len()
        ));
    }
    if input.trim().is_empty() {
        return Err("input is empty: paste RIS records (a 'TY  - JOUR' line through 'ER  - ') or BibTeX entries (@article{key, ...})".into());
    }
    let resolved = match direction {
        Direction::Auto => detect(input)?,
        d => d,
    };
    match resolved {
        Direction::RisToBibtex => ris_to_bibtex(
            input,
            key_style,
            include_abstract,
            include_keywords,
            translate_latex,
            indent,
            sort,
        ),
        Direction::BibtexToRis => {
            bibtex_to_ris(input, include_abstract, include_keywords, translate_latex, sort)
        }
        Direction::Auto => unreachable!("auto is resolved above"),
    }
}

/// Sniff whether `input` is RIS or BibTeX and return the conversion that turns
/// it into the other format.
pub fn detect(input: &str) -> Result<Direction, String> {
    let ris_at = input
        .lines()
        .scan(0usize, |off, line| {
            let at = *off;
            *off += line.len() + 1;
            Some((at, line))
        })
        .find(|(_, line)| matches!(parse_tag_line(line), Some((ref t, _)) if t == "TY"))
        .map(|(at, _)| at);
    let bib_at = find_bibtex_entry(input);
    match (ris_at, bib_at) {
        (Some(r), Some(b)) => Ok(if r <= b {
            Direction::RisToBibtex
        } else {
            Direction::BibtexToRis
        }),
        (Some(_), None) => Ok(Direction::RisToBibtex),
        (None, Some(_)) => Ok(Direction::BibtexToRis),
        (None, None) => Err(
            "could not tell whether the input is RIS or BibTeX: expected a RIS type line such as 'TY  - JOUR' or a BibTeX entry such as '@article{key,'. Set direction explicitly if the input is unusual."
                .into(),
        ),
    }
}

/// Byte offset of the first line that starts a BibTeX item (`@type{` / `@type(`).
fn find_bibtex_entry(input: &str) -> Option<usize> {
    let mut off = 0usize;
    for line in input.lines() {
        let t = line.trim_start();
        if let Some(rest) = t.strip_prefix('@') {
            let name: String = rest.chars().take_while(|c| c.is_ascii_alphabetic()).collect();
            let after = rest[name.len()..].trim_start();
            if !name.is_empty() && (after.starts_with('{') || after.starts_with('(')) {
                return Some(off);
            }
        }
        off += line.len() + 1;
    }
    None
}

// ---------------------------------------------------------------------------
// RIS reading
// ---------------------------------------------------------------------------

/// Every tag the RIS 1.0 specification defines, plus the widely-exported extras.
/// A line is only treated as a tag line when it is spelled canonically
/// (`XX  - value`, two spaces) or its tag is one of these — otherwise
/// `In - depth analysis` on a wrapped line would masquerade as an `IN` tag.
const KNOWN_TAGS: [&str; 76] = [
    "TY", "A1", "A2", "A3", "A4", "AB", "AD", "AN", "AU", "AV", "BT", "C1", "C2", "C3", "C4", "C5",
    "C6", "C7", "C8", "CA", "CN", "CP", "CT", "CY", "DA", "DB", "DO", "DP", "ED", "EP", "ET", "ID",
    "IS", "J1", "J2", "JA", "JF", "JO", "KW", "L1", "L2", "L3", "L4", "LA", "LB", "LK", "M1", "M2",
    "M3", "N1", "N2", "NV", "OP", "PB", "PP", "PY", "RI", "RN", "RP", "SE", "SN", "SP", "ST", "T1",
    "T2", "T3", "TA", "TI", "TT", "U1", "U2", "U3", "U4", "U5", "UR", "VL",
];

/// Recognise a `XX  - value` RIS tag line. Leading whitespace marks a wrapped
/// continuation line, never a tag.
fn parse_tag_line(line: &str) -> Option<(String, String)> {
    let line = line.trim_end_matches(['\r', '\n']);
    if line.starts_with(char::is_whitespace) {
        return None;
    }
    let c: Vec<char> = line.chars().collect();
    if c.len() < 3 || !c[0].is_ascii_alphabetic() || !c[1].is_ascii_alphanumeric() {
        return None;
    }
    let mut i = 2usize;
    let mut spaces = 0usize;
    while i < c.len() && c[i] == ' ' {
        i += 1;
        spaces += 1;
    }
    if spaces > 3 || i >= c.len() || c[i] != '-' {
        return None;
    }
    let tag: String = c[..2].iter().collect::<String>().to_ascii_uppercase();
    if tag != "ER" && spaces != 2 && !KNOWN_TAGS.contains(&tag.as_str()) {
        return None;
    }
    i += 1;
    if i < c.len() && c[i] == ' ' {
        i += 1;
    }
    let value: String = c[i..].iter().collect::<String>().trim().to_string();
    Some((tag, value))
}

/// Parse RIS source into records. Wrapped continuation lines are folded onto the
/// preceding tag with a single space; `ER` closes a record and a fresh `TY`
/// implicitly closes an unterminated one.
pub fn parse_ris(src: &str) -> Result<Vec<RisRecord>, String> {
    let mut out: Vec<RisRecord> = Vec::new();
    let mut cur: Option<RisRecord> = None;
    let mut saw_tag = false;

    for raw in src.lines() {
        match parse_tag_line(raw) {
            Some((tag, value)) => {
                saw_tag = true;
                match tag.as_str() {
                    "TY" => {
                        if let Some(rec) = cur.take() {
                            if !rec.tags.is_empty() {
                                out.push(rec);
                            }
                        }
                        let mut rec = RisRecord::default();
                        rec.tags.push((
                            "TY".to_string(),
                            if value.is_empty() {
                                "GEN".to_string()
                            } else {
                                value.to_ascii_uppercase()
                            },
                        ));
                        cur = Some(rec);
                    }
                    "ER" => {
                        if let Some(rec) = cur.take() {
                            if !rec.tags.is_empty() {
                                out.push(rec);
                            }
                        }
                    }
                    _ => {
                        let rec = cur.get_or_insert_with(|| RisRecord {
                            tags: vec![("TY".to_string(), "GEN".to_string())],
                        });
                        rec.tags.push((tag, value));
                    }
                }
            }
            None => {
                if raw.trim().is_empty() {
                    continue;
                }
                // A wrapped continuation of the previous tag's value.
                if let Some(rec) = cur.as_mut() {
                    if let Some((_, v)) = rec.tags.last_mut() {
                        if !v.is_empty() {
                            v.push(' ');
                        }
                        v.push_str(raw.trim());
                    }
                }
            }
        }
    }
    if let Some(rec) = cur.take() {
        if !rec.tags.is_empty() {
            out.push(rec);
        }
    }
    if out.is_empty() {
        return Err(if saw_tag {
            "no RIS records found: every record needs a 'TY  - <type>' line (for example 'TY  - JOUR')".into()
        } else {
            "no RIS records found: expected lines shaped 'TY  - JOUR', 'AU  - Shannon, C. E.', ... 'ER  - '".into()
        });
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// Type maps
// ---------------------------------------------------------------------------

/// RIS reference type → BibTeX entry type.
fn ris_type_to_bibtex(ty: &str, rec: &RisRecord) -> &'static str {
    match ty {
        "JOUR" | "JFULL" | "MGZN" | "NEWS" | "ABST" | "INPR" => "article",
        "BOOK" | "EBOOK" | "SER" | "EDBOOK" => "book",
        "CHAP" | "ECHAP" => "incollection",
        "CONF" | "CPAPER" => "inproceedings",
        "THES" => {
            let m3 = rec.first("M3").unwrap_or("").to_ascii_lowercase();
            if m3.contains("master") || m3.contains("msc") || m3.contains("m.s.") {
                "mastersthesis"
            } else {
                "phdthesis"
            }
        }
        "RPRT" | "GOVDOC" | "STAND" => "techreport",
        "UNPB" | "MANSCPT" => "unpublished",
        "COMP" => "software",
        "DATA" | "DBASE" => "misc",
        "ELEC" | "ICOMM" | "BLOG" | "WEB" => "misc",
        _ => "misc",
    }
}

/// BibTeX entry type → RIS reference type.
fn bibtex_type_to_ris(kind: &str) -> &'static str {
    match kind {
        "article" => "JOUR",
        "book" | "booklet" | "collection" | "mvbook" => "BOOK",
        "inbook" | "incollection" | "bookinbook" | "suppbook" => "CHAP",
        "inproceedings" | "conference" | "proceedings" | "inproceeding" => "CONF",
        "phdthesis" | "mastersthesis" | "thesis" => "THES",
        "techreport" | "report" => "RPRT",
        "unpublished" => "UNPB",
        "patent" => "PAT",
        "dataset" => "DATA",
        "software" => "COMP",
        "online" | "electronic" | "webpage" | "www" => "ELEC",
        "manual" => "GEN",
        _ => "GEN",
    }
}

/// True for the BibTeX types whose container title is a `booktitle`, not a journal.
fn is_book_like(kind: &str) -> bool {
    matches!(
        kind,
        "book" | "incollection" | "inbook" | "inproceedings" | "proceedings" | "booklet"
    )
}

// ---------------------------------------------------------------------------
// RIS → BibTeX
// ---------------------------------------------------------------------------

fn ris_to_bibtex(
    src: &str,
    key_style: KeyStyle,
    include_abstract: bool,
    include_keywords: bool,
    escape_latex: bool,
    indent: u32,
    sort: Sort,
) -> Result<String, String> {
    let mut records = parse_ris(src)?;

    // Sort BEFORE keys are assigned so `numeric` keys run 1..n down the output.
    match sort {
        Sort::Source => {}
        Sort::Key => records.sort_by_key(|r| provisional_key(r).to_ascii_lowercase()),
        Sort::Year => records.sort_by_key(|r| ris_year(r).unwrap_or(u32::MAX)),
        Sort::Type => records.sort_by(|a, b| {
            let ka = ris_type_to_bibtex(a.first("TY").unwrap_or("GEN"), a);
            let kb = ris_type_to_bibtex(b.first("TY").unwrap_or("GEN"), b);
            ka.cmp(kb)
                .then_with(|| provisional_key(a).cmp(&provisional_key(b)))
        }),
    }

    let mut used: Vec<String> = Vec::new();
    let mut out = String::new();
    for (i, rec) in records.iter().enumerate() {
        let ty = rec.first("TY").unwrap_or("GEN");
        let kind = ris_type_to_bibtex(ty, rec);
        let key = unique_key(cite_key(rec, key_style, i + 1), &mut used);
        let fields = ris_fields_to_bibtex(rec, kind, include_abstract, include_keywords);

        if !out.is_empty() {
            out.push('\n');
        }
        out.push('@');
        out.push_str(kind);
        out.push('{');
        out.push_str(&key);
        out.push_str(",\n");
        let pad = " ".repeat(indent as usize);
        for (n, (name, value)) in fields.iter().enumerate() {
            let v = if escape_latex && !matches!(name.as_str(), "url" | "doi") {
                latex_escape(value)
            } else {
                value.clone()
            };
            out.push_str(&pad);
            out.push_str(name);
            out.push_str(" = {");
            out.push_str(&v);
            out.push('}');
            if n + 1 < fields.len() {
                out.push(',');
            }
            out.push('\n');
        }
        out.push_str("}\n");
    }
    Ok(out.trim_end().to_string())
}

/// Map one RIS record's tags onto ordered BibTeX fields.
fn ris_fields_to_bibtex(
    rec: &RisRecord,
    kind: &str,
    include_abstract: bool,
    include_keywords: bool,
) -> Vec<(String, String)> {
    let mut f: Vec<(String, String)> = Vec::new();
    let mut push = |name: &str, value: String| {
        let v = collapse_ws(&value);
        if !v.is_empty() {
            f.push((name.to_string(), v));
        }
    };

    let authors = rec.all_of(&["AU", "A1"]);
    if !authors.is_empty() {
        push("author", authors.join(" and "));
    }
    let editors = rec.all_of(&["A2", "A3", "A4", "ED"]);
    // For a chapter/paper the secondary author is the editor of the container;
    // for a plain book with no primary author it is the editor of the book.
    if !editors.is_empty() {
        push("editor", editors.join(" and "));
    }

    let title = rec
        .first_of(&["TI", "T1", "CT", "ST"])
        .or_else(|| rec.first("BT"))
        .unwrap_or_default();
    push("title", title.to_string());

    // Container title: `booktitle` for book-like entries, `journal` otherwise.
    let container = if is_book_like(kind) {
        rec.first_of(&["T2", "BT"])
            .filter(|c| *c != title)
            .map(str::to_string)
    } else {
        rec.first_of(&["JO", "JF", "JA", "J1", "J2", "T2"])
            .map(str::to_string)
    };
    // Tracked rather than re-scanned out of `f`, which the `push` closure has
    // mutably borrowed for the rest of this function.
    let mut has_series = false;
    if let Some(c) = container {
        if kind == "book" {
            has_series = !c.trim().is_empty();
            push("series", c);
        } else if is_book_like(kind) {
            push("booktitle", c);
        } else {
            push("journal", c);
        }
    }
    if let Some(s) = rec.first("T3") {
        if !has_series {
            push("series", s.to_string());
        }
    }

    push("volume", rec.first("VL").unwrap_or_default().to_string());
    push(
        "number",
        rec.first_of(&["IS", "M1"]).unwrap_or_default().to_string(),
    );
    push("pages", join_pages(rec.first("SP"), rec.first("EP")));
    if let Some(y) = ris_year(rec) {
        push("year", y.to_string());
    }
    if let Some(m) = ris_month(rec) {
        push("month", m.to_string());
    }

    let publisher = rec.first_of(&["PB"]).unwrap_or_default().to_string();
    match kind {
        "phdthesis" | "mastersthesis" => push("school", publisher),
        "techreport" => push("institution", publisher),
        _ => push("publisher", publisher),
    }
    push(
        "address",
        rec.first_of(&["CY", "PP"]).unwrap_or_default().to_string(),
    );
    push("edition", rec.first("ET").unwrap_or_default().to_string());
    if matches!(kind, "phdthesis" | "mastersthesis" | "techreport") {
        push("type", rec.first("M3").unwrap_or_default().to_string());
    }
    push("doi", rec.first("DO").unwrap_or_default().to_string());
    push(
        "url",
        rec.first_of(&["UR", "L1", "LK"]).unwrap_or_default().to_string(),
    );
    push("urldate", normalize_date(rec.first("Y2").unwrap_or_default()));
    if let Some(sn) = rec.first("SN") {
        if is_book_like(kind) || kind == "phdthesis" || kind == "mastersthesis" {
            push("isbn", sn.to_string());
        } else {
            push("issn", sn.to_string());
        }
    }
    push("language", rec.first("LA").unwrap_or_default().to_string());
    if include_keywords {
        let kws = rec.all("KW");
        if !kws.is_empty() {
            push("keywords", kws.join(", "));
        }
    }
    if include_abstract {
        push(
            "abstract",
            rec.first_of(&["AB", "N2"]).unwrap_or_default().to_string(),
        );
    }
    push("note", rec.first("N1").unwrap_or_default().to_string());
    f
}

/// `SP`/`EP` → a BibTeX `pages` value with an en-dash range.
fn join_pages(sp: Option<&str>, ep: Option<&str>) -> String {
    let sp = sp.unwrap_or("").trim();
    let ep = ep.unwrap_or("").trim();
    match (sp.is_empty(), ep.is_empty()) {
        (true, true) => String::new(),
        (false, true) => {
            // `379-423` already in SP: normalise the single hyphen to `--`.
            if sp.matches('-').count() == 1 && !sp.contains("--") {
                sp.replacen('-', "--", 1)
            } else {
                sp.to_string()
            }
        }
        (true, false) => ep.to_string(),
        (false, false) => {
            if sp == ep {
                sp.to_string()
            } else {
                format!("{sp}--{ep}")
            }
        }
    }
}

/// First four-digit year in the RIS date tags.
fn ris_year(rec: &RisRecord) -> Option<u32> {
    for tag in ["PY", "Y1", "DA", "C1"] {
        if let Some(v) = rec.first(tag) {
            if let Some(y) = first_year(v) {
                return Some(y);
            }
        }
    }
    None
}

/// Month abbreviation (`jan`..`dec`) from a `YYYY/MM/DD/other` RIS date.
fn ris_month(rec: &RisRecord) -> Option<&'static str> {
    for tag in ["PY", "Y1", "DA"] {
        if let Some(v) = rec.first(tag) {
            let parts: Vec<&str> = v.split('/').collect();
            if parts.len() >= 2 {
                if let Some(m) = month_name(parts[1]) {
                    return Some(m);
                }
            }
        }
    }
    None
}

fn first_year(s: &str) -> Option<u32> {
    let c: Vec<char> = s.chars().collect();
    let mut i = 0usize;
    while i + 4 <= c.len() {
        if c[i..i + 4].iter().all(|ch| ch.is_ascii_digit()) {
            let before_ok = i == 0 || !c[i - 1].is_ascii_digit();
            let after_ok = i + 4 == c.len() || !c[i + 4].is_ascii_digit();
            if before_ok && after_ok {
                let y: u32 = c[i..i + 4].iter().collect::<String>().parse().ok()?;
                return Some(y);
            }
        }
        i += 1;
    }
    None
}

const MONTHS: [&str; 12] = [
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];

/// `07`, `7`, `jul`, `July` → `jul`.
fn month_name(s: &str) -> Option<&'static str> {
    let t = s.trim().trim_matches(['{', '}', '"']).to_ascii_lowercase();
    if t.is_empty() {
        return None;
    }
    if let Ok(n) = t.parse::<usize>() {
        return MONTHS.get(n.checked_sub(1)?).copied();
    }
    MONTHS.iter().find(|m| t.starts_with(**m)).copied()
}

/// `2024/03/09/` or `2024-03-09` → `2024-03-09` (what BibLaTeX's `urldate` wants).
fn normalize_date(s: &str) -> String {
    let t = s.trim();
    if t.is_empty() {
        return String::new();
    }
    let parts: Vec<&str> = t
        .split(['/', '-'])
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    let numeric: Vec<&&str> = parts.iter().take(3).filter(|p| p.chars().all(|c| c.is_ascii_digit())).collect();
    match numeric.len() {
        3 => format!("{}-{:0>2}-{:0>2}", numeric[0], numeric[1], numeric[2]),
        2 => format!("{}-{:0>2}", numeric[0], numeric[1]),
        1 => numeric[0].to_string(),
        _ => t.to_string(),
    }
}

fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Escape the ten characters that are special to LaTeX so the emitted `.bib`
/// compiles. `url`/`doi` values are exempted by the caller.
fn latex_escape(s: &str) -> String {
    let mut o = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '\\' => o.push_str("\\textbackslash{}"),
            '{' => o.push_str("\\{"),
            '}' => o.push_str("\\}"),
            '&' | '%' | '$' | '#' | '_' => {
                o.push('\\');
                o.push(c);
            }
            '~' => o.push_str("\\textasciitilde{}"),
            '^' => o.push_str("\\textasciicircum{}"),
            c => o.push(c),
        }
    }
    o
}

// ---------------------------------------------------------------------------
// Cite keys
// ---------------------------------------------------------------------------

const TITLE_STOPWORDS: [&str; 24] = [
    "a", "an", "the", "on", "of", "for", "in", "and", "to", "with", "at", "by", "from", "into",
    "over", "under", "is", "are", "as", "its", "new", "using", "use", "towards",
];

fn provisional_key(rec: &RisRecord) -> String {
    cite_key(rec, KeyStyle::AuthorYearWord, 1)
}

fn cite_key(rec: &RisRecord, style: KeyStyle, ordinal: usize) -> String {
    if style == KeyStyle::Numeric {
        return format!("ref{ordinal}");
    }
    if style == KeyStyle::RisId {
        if let Some(id) = rec.first("ID") {
            let cleaned = sanitize_key(id);
            if !cleaned.is_empty() {
                return cleaned;
            }
        }
    }
    let surname = rec
        .first_of(&["AU", "A1", "A2", "ED"])
        .map(family_name)
        .map(|s| fold_ascii(&s))
        .unwrap_or_default();
    let year = ris_year(rec).map(|y| y.to_string()).unwrap_or_default();
    let word = if style == KeyStyle::AuthorYear {
        String::new()
    } else {
        rec.first_of(&["TI", "T1", "CT", "BT"])
            .map(title_word)
            .unwrap_or_default()
    };
    let key = format!("{surname}{year}{word}");
    if key.is_empty() {
        format!("ref{ordinal}")
    } else {
        key
    }
}

/// The family name of a RIS `AU` value (`Family, Given` or `Given Family`).
fn family_name(name: &str) -> String {
    let n = name.trim().trim_matches(['{', '}']);
    if let Some((last, _)) = n.split_once(',') {
        return last.trim().to_string();
    }
    n.split_whitespace()
        .last()
        .unwrap_or(n)
        .to_string()
}

/// First title word worth putting in a cite key.
fn title_word(title: &str) -> String {
    let mut fallback = String::new();
    for raw in title.split_whitespace() {
        let w = fold_ascii(raw);
        if w.is_empty() {
            continue;
        }
        if fallback.is_empty() {
            fallback = w.clone();
        }
        if !TITLE_STOPWORDS.contains(&w.as_str()) && w.len() >= 3 {
            return w;
        }
    }
    fallback
}

/// Lowercase, strip accents, keep `[a-z0-9]`.
fn fold_ascii(s: &str) -> String {
    let mut out = String::new();
    for ch in s.chars() {
        let c = ch.to_lowercase().next().unwrap_or(ch);
        let rep = match c {
            'á' | 'à' | 'â' | 'ä' | 'ã' | 'å' | 'ā' | 'ă' | 'ą' => "a",
            'ç' | 'ć' | 'č' => "c",
            'ď' | 'đ' | 'ð' => "d",
            'é' | 'è' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => "e",
            'ğ' => "g",
            'í' | 'ì' | 'î' | 'ï' | 'ī' | 'į' | 'ı' => "i",
            'ł' => "l",
            'ñ' | 'ń' | 'ň' => "n",
            'ó' | 'ò' | 'ô' | 'ö' | 'õ' | 'ø' | 'ō' | 'ő' => "o",
            'ř' => "r",
            'ś' | 'š' | 'ş' => "s",
            'ť' | 'ţ' => "t",
            'ú' | 'ù' | 'û' | 'ü' | 'ū' | 'ů' | 'ű' => "u",
            'ý' | 'ÿ' => "y",
            'ź' | 'ż' | 'ž' => "z",
            'ß' => "ss",
            'æ' => "ae",
            'œ' => "oe",
            'þ' => "th",
            c if c.is_ascii_alphanumeric() => {
                out.push(c);
                continue;
            }
            _ => "",
        };
        out.push_str(rep);
    }
    out
}

/// Keep a supplied RIS `ID` usable as a BibTeX cite key.
fn sanitize_key(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | ':' | '.' | '+' | '/'))
        .collect()
}

/// Disambiguate a repeated cite key with a trailing `a`, `b`, `c`, …
fn unique_key(base: String, used: &mut Vec<String>) -> String {
    if !used.contains(&base) {
        used.push(base.clone());
        return base;
    }
    for n in 0u32..1000 {
        let suffix: String = if n < 26 {
            ((b'a' + n as u8) as char).to_string()
        } else {
            format!("{}", n + 1)
        };
        let candidate = format!("{base}{suffix}");
        if !used.contains(&candidate) {
            used.push(candidate.clone());
            return candidate;
        }
    }
    used.push(base.clone());
    base
}

// ---------------------------------------------------------------------------
// BibTeX → RIS
// ---------------------------------------------------------------------------

fn bibtex_to_ris(
    src: &str,
    include_abstract: bool,
    include_keywords: bool,
    decode_latex: bool,
    sort: Sort,
) -> Result<String, String> {
    let mut entries = bib::parse(src, true)?;
    if entries.is_empty() {
        return Err("no BibTeX entries found: expected at least one entry such as '@article{key, title = {...}, year = {2024}}'".into());
    }
    match sort {
        Sort::Source => {}
        Sort::Key => entries.sort_by_key(|e| e.key.to_ascii_lowercase()),
        Sort::Year => entries.sort_by_key(|e| {
            e.fields
                .iter()
                .find(|(n, _)| n == "year")
                .and_then(|(_, v)| first_year(v))
                .unwrap_or(u32::MAX)
        }),
        Sort::Type => entries.sort_by(|a, b| {
            a.kind
                .cmp(&b.kind)
                .then_with(|| a.key.to_ascii_lowercase().cmp(&b.key.to_ascii_lowercase()))
        }),
    }

    let mut out = String::new();
    for e in &entries {
        if !out.is_empty() {
            out.push('\n');
        }
        for (tag, value) in bibtex_entry_to_ris(e, include_abstract, include_keywords, decode_latex)
        {
            out.push_str(&tag);
            out.push_str("  - ");
            out.push_str(&value);
            out.push('\n');
        }
        out.push_str("ER  - \n");
    }
    Ok(out.trim_end_matches('\n').to_string())
}

fn bibtex_entry_to_ris(
    e: &bib::Entry,
    include_abstract: bool,
    include_keywords: bool,
    decode_latex: bool,
) -> Vec<(String, String)> {
    let get = |name: &str| -> String {
        let raw = e
            .fields
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
            .unwrap_or("");
        let s = if decode_latex {
            bib::decode_latex_text(raw)
        } else {
            raw.to_string()
        };
        collapse_ws(&s)
    };
    let names = |name: &str| -> Vec<String> {
        let raw = e
            .fields
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
            .unwrap_or("");
        if raw.trim().is_empty() {
            return Vec::new();
        }
        let joined = bib::format_names(raw, bib::AuthorFormat::LastFirst, bib::AuthorSeparator::Pipe);
        joined
            .split(" | ")
            .map(|n| {
                let s = if decode_latex {
                    bib::decode_latex_text(n)
                } else {
                    n.to_string()
                };
                collapse_ws(&s)
            })
            .filter(|n| !n.is_empty())
            .collect()
    };

    let kind = e.kind.to_ascii_lowercase();
    let ty = bibtex_type_to_ris(&kind);
    let mut out: Vec<(String, String)> = vec![("TY".into(), ty.into())];
    let mut push = |tag: &str, value: String| {
        if !value.trim().is_empty() {
            out.push((tag.to_string(), value));
        }
    };

    if !e.key.trim().is_empty() {
        push("ID", e.key.trim().to_string());
    }
    for a in names("author") {
        push("AU", a);
    }
    for a in names("editor") {
        push("A2", a);
    }
    push("TI", get("title"));
    let booktitle = get("booktitle");
    if !booktitle.is_empty() {
        push("T2", booktitle);
    } else {
        let journal = get("journal");
        if !journal.is_empty() {
            push("JO", journal);
        }
    }
    push("T3", get("series"));
    let year = get("year");
    if !year.is_empty() {
        push("PY", first_year(&year).map(|y| y.to_string()).unwrap_or(year));
    }
    if let Some(m) = month_name(&get("month")) {
        let idx = MONTHS.iter().position(|x| *x == m).unwrap_or(0) + 1;
        let y = first_year(&get("year"))
            .map(|y| y.to_string())
            .unwrap_or_default();
        push("DA", format!("{y}/{idx:02}//"));
    }
    push("VL", get("volume"));
    push("IS", get("number"));
    let (sp, ep) = split_pages(&get("pages"));
    push("SP", sp);
    push("EP", ep);
    push("ET", get("edition"));
    let publisher = [get("publisher"), get("school"), get("institution"), get("organization")]
        .into_iter()
        .find(|v| !v.is_empty())
        .unwrap_or_default();
    push("PB", publisher);
    push("CY", get("address"));
    let sn = [get("isbn"), get("issn")]
        .into_iter()
        .find(|v| !v.is_empty())
        .unwrap_or_default();
    push("SN", sn);
    push("DO", get("doi"));
    push("UR", get("url"));
    push("Y2", normalize_date(&get("urldate")));
    push("LA", get("language"));
    let m3 = if !get("type").is_empty() {
        get("type")
    } else if kind == "phdthesis" {
        "PhD thesis".to_string()
    } else if kind == "mastersthesis" {
        "Master's thesis".to_string()
    } else {
        String::new()
    };
    push("M3", m3);
    if include_abstract {
        push("AB", get("abstract"));
    }
    if include_keywords {
        for kw in get("keywords")
            .split([',', ';'])
            .map(str::trim)
            .filter(|k| !k.is_empty())
        {
            push("KW", kw.to_string());
        }
    }
    push("N1", get("note"));
    out
}

/// A BibTeX `pages` value → `(SP, EP)`.
fn split_pages(pages: &str) -> (String, String) {
    let p = pages.trim();
    if p.is_empty() {
        return (String::new(), String::new());
    }
    let normalized = p.replace('\u{2013}', "-").replace('\u{2014}', "-");
    let collapsed = {
        let mut s = normalized;
        while s.contains("--") {
            s = s.replace("--", "-");
        }
        s
    };
    match collapsed.split_once('-') {
        Some((a, b)) if !a.trim().is_empty() && !b.trim().is_empty() => {
            (a.trim().to_string(), b.trim().to_string())
        }
        _ => (p.to_string(), String::new()),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const SHANNON_RIS: &str = "TY  - JOUR\nAU  - Shannon, C. E.\nTI  - A Mathematical Theory of Communication\nJO  - Bell System Technical Journal\nPY  - 1948\nVL  - 27\nIS  - 3\nSP  - 379\nEP  - 423\nDO  - 10.1002/j.1538-7305.1948.tb01338.x\nER  - \n";

    fn ris2bib(src: &str) -> String {
        convert_str(src, "auto", "author-year-word", true, true, true, "2", "source").unwrap()
    }
    fn bib2ris(src: &str) -> String {
        convert_str(src, "auto", "author-year-word", true, true, true, "2", "source").unwrap()
    }

    #[test]
    fn ris_to_bibtex_happy_path() {
        assert_eq!(
            ris2bib(SHANNON_RIS),
            "@article{shannon1948mathematical,\n  \
             author = {Shannon, C. E.},\n  \
             title = {A Mathematical Theory of Communication},\n  \
             journal = {Bell System Technical Journal},\n  \
             volume = {27},\n  \
             number = {3},\n  \
             pages = {379--423},\n  \
             year = {1948},\n  \
             doi = {10.1002/j.1538-7305.1948.tb01338.x}\n\
             }"
        );
    }

    #[test]
    fn bibtex_to_ris_happy_path() {
        let bib = "@article{shannon1948,\n  author = {Shannon, C. E.},\n  title = {A Mathematical Theory of Communication},\n  journal = {Bell System Technical Journal},\n  year = {1948},\n  volume = {27},\n  pages = {379--423},\n  doi = {10.1002/j.1538-7305.1948.tb01338.x}\n}";
        assert_eq!(
            bib2ris(bib),
            "TY  - JOUR\nID  - shannon1948\nAU  - Shannon, C. E.\nTI  - A Mathematical Theory of Communication\nJO  - Bell System Technical Journal\nPY  - 1948\nVL  - 27\nSP  - 379\nEP  - 423\nDO  - 10.1002/j.1538-7305.1948.tb01338.x\nER  - "
        );
    }

    #[test]
    fn round_trip_keeps_the_core_fields() {
        let back = ris2bib(&bib2ris(&ris2bib(SHANNON_RIS)));
        assert!(back.contains("journal = {Bell System Technical Journal}"), "{back}");
        assert!(back.contains("pages = {379--423}"), "{back}");
        assert!(back.contains("volume = {27}"), "{back}");
    }

    #[test]
    fn detects_both_directions() {
        assert_eq!(detect(SHANNON_RIS).unwrap(), Direction::RisToBibtex);
        assert_eq!(detect("@book{k, title = {T}}").unwrap(), Direction::BibtexToRis);
        assert!(detect("just some prose").is_err());
    }

    #[test]
    fn explicit_direction_overrides_detection() {
        // Forcing the wrong direction surfaces a parse error rather than silently
        // echoing the input back.
        let err = convert_str(SHANNON_RIS, "bibtex-to-ris", "author-year-word", true, true, true, "2", "source")
            .unwrap_err();
        assert!(err.contains("no BibTeX entries found"), "{err}");
    }

    #[test]
    fn error_on_unrecognisable_input() {
        let err = convert_str("hello world", "auto", "author-year-word", true, true, true, "2", "source")
            .unwrap_err();
        assert!(err.contains("could not tell whether the input is RIS or BibTeX"), "{err}");
    }

    #[test]
    fn error_on_empty_input() {
        let err = convert_str("   \n ", "auto", "author-year-word", true, true, true, "2", "source")
            .unwrap_err();
        assert!(err.contains("input is empty"), "{err}");
    }

    #[test]
    fn error_on_bad_option_values() {
        assert!(convert_str(SHANNON_RIS, "sideways", "author-year-word", true, true, true, "2", "source")
            .unwrap_err()
            .contains("invalid direction"));
        assert!(convert_str(SHANNON_RIS, "auto", "rainbow", true, true, true, "2", "source")
            .unwrap_err()
            .contains("invalid key_style"));
        assert!(convert_str(SHANNON_RIS, "auto", "author-year-word", true, true, true, "99", "source")
            .unwrap_err()
            .contains("invalid indent"));
        assert!(convert_str(SHANNON_RIS, "auto", "author-year-word", true, true, true, "2", "backwards")
            .unwrap_err()
            .contains("invalid sort"));
    }

    #[test]
    fn error_on_oversized_input() {
        let big = "TY  - JOUR\n".repeat(MAX_INPUT_BYTES / 5);
        let err = convert_str(&big, "auto", "author-year-word", true, true, true, "2", "source")
            .unwrap_err();
        assert!(err.contains("over the"), "{err}");
    }

    #[test]
    fn key_styles() {
        let src = "TY  - JOUR\nID  - Zotero:AB12\nAU  - van Beethoven, Ludwig\nTI  - The Ninth Symphony\nPY  - 1824\nER  - \n";
        let k = |style: &str| {
            let out = convert_str(src, "auto", style, true, true, true, "2", "source").unwrap();
            out.lines().next().unwrap().to_string()
        };
        assert_eq!(k("author-year-word"), "@article{vanbeethoven1824ninth,");
        assert_eq!(k("author-year"), "@article{vanbeethoven1824,");
        assert_eq!(k("ris-id"), "@article{Zotero:AB12,");
        assert_eq!(k("numeric"), "@article{ref1,");
    }

    #[test]
    fn duplicate_keys_get_a_letter_suffix() {
        let src = format!("{SHANNON_RIS}{SHANNON_RIS}");
        let out = ris2bib(&src);
        assert!(out.contains("@article{shannon1948mathematical,"), "{out}");
        assert!(out.contains("@article{shannon1948mathematicala,"), "{out}");
    }

    #[test]
    fn accented_names_fold_into_ascii_keys() {
        let src = "TY  - JOUR\nAU  - Erdős, Pál\nTI  - Über Fehlerabschätzungen\nPY  - 1959\nER  - \n";
        assert!(ris2bib(src).starts_with("@article{erdos1959uber,"), "{}", ris2bib(src));
    }

    #[test]
    fn wrapped_continuation_lines_are_folded() {
        let src = "TY  - JOUR\nTI  - A very long title that the exporter\n      wrapped onto a second line\nPY  - 2001\nER  - \n";
        let out = ris2bib(src);
        assert!(
            out.contains("title = {A very long title that the exporter wrapped onto a second line}"),
            "{out}"
        );
    }

    #[test]
    fn thesis_book_and_report_types_map_both_ways() {
        let src = "TY  - THES\nAU  - Doe, Jane\nTI  - On Widgets\nPY  - 2010\nPB  - MIT\nM3  - Master's thesis\nER  - \nTY  - RPRT\nTI  - Annual Report\nPY  - 2011\nPB  - ACME\nER  - \nTY  - CHAP\nTI  - A Chapter\nT2  - The Big Book\nPY  - 2012\nER  - \n";
        let out = ris2bib(src);
        assert!(out.contains("@mastersthesis{"), "{out}");
        assert!(out.contains("school = {MIT}"), "{out}");
        assert!(out.contains("@techreport{"), "{out}");
        assert!(out.contains("institution = {ACME}"), "{out}");
        assert!(out.contains("@incollection{"), "{out}");
        assert!(out.contains("booktitle = {The Big Book}"), "{out}");

        let back = bib2ris(&out);
        assert!(back.contains("TY  - THES"), "{back}");
        assert!(back.contains("TY  - RPRT"), "{back}");
        assert!(back.contains("TY  - CHAP"), "{back}");
        assert!(back.contains("T2  - The Big Book"), "{back}");
    }

    #[test]
    fn abstract_and_keywords_can_be_dropped() {
        let src = "TY  - JOUR\nTI  - T\nPY  - 2020\nAB  - Some abstract.\nKW  - alpha\nKW  - beta\nER  - \n";
        let with = convert_str(src, "auto", "numeric", true, true, true, "2", "source").unwrap();
        assert!(with.contains("keywords = {alpha, beta}"), "{with}");
        assert!(with.contains("abstract = {Some abstract.}"), "{with}");
        let without = convert_str(src, "auto", "numeric", false, false, true, "2", "source").unwrap();
        assert!(!without.contains("abstract"), "{without}");
        assert!(!without.contains("keywords"), "{without}");
    }

    #[test]
    fn latex_is_escaped_going_out_and_decoded_coming_in() {
        let src = "TY  - JOUR\nTI  - Cost & Benefit: 50% of R_2\nPY  - 2020\nUR  - https://example.org/a_b\nER  - \n";
        let escaped = convert_str(src, "auto", "numeric", true, true, true, "2", "source").unwrap();
        assert!(escaped.contains("title = {Cost \\& Benefit: 50\\% of R\\_2}"), "{escaped}");
        // URLs are left alone so they stay clickable.
        assert!(escaped.contains("url = {https://example.org/a_b}"), "{escaped}");
        let verbatim = convert_str(src, "auto", "numeric", true, true, false, "2", "source").unwrap();
        assert!(verbatim.contains("title = {Cost & Benefit: 50% of R_2}"), "{verbatim}");

        let bib = "@article{x, title = {Erd{\\H o}s and {DNA}}, author = {M\\\"uller, Anna}, year = {1999}}";
        let ris = convert_str(bib, "auto", "numeric", true, true, true, "2", "source").unwrap();
        assert!(ris.contains("TI  - Erdős and DNA"), "{ris}");
        assert!(ris.contains("AU  - Müller, Anna"), "{ris}");
    }

    #[test]
    fn indent_controls_bibtex_field_padding() {
        let flat = convert_str(SHANNON_RIS, "auto", "numeric", true, true, true, "0", "source").unwrap();
        assert!(flat.contains("\nauthor = {Shannon, C. E.},\n"), "{flat}");
        let wide = convert_str(SHANNON_RIS, "auto", "numeric", true, true, true, "4", "source").unwrap();
        assert!(wide.contains("\n    author = {Shannon, C. E.},\n"), "{wide}");
    }

    #[test]
    fn sort_orders_the_output() {
        let src = "TY  - JOUR\nAU  - Zeta, A.\nTI  - Zulu\nPY  - 2001\nER  - \nTY  - BOOK\nAU  - Alpha, B.\nTI  - Able\nPY  - 1999\nER  - \n";
        let by_year = convert_str(src, "auto", "numeric", true, true, true, "2", "year").unwrap();
        assert!(by_year.starts_with("@book{ref1,"), "{by_year}");
        let by_key = convert_str(src, "auto", "author-year-word", true, true, true, "2", "key").unwrap();
        assert!(by_key.starts_with("@book{alpha1999able,"), "{by_key}");
        let by_type = convert_str(src, "auto", "numeric", true, true, true, "2", "type").unwrap();
        assert!(by_type.starts_with("@article{ref1,"), "{by_type}");
        let by_source = convert_str(src, "auto", "numeric", true, true, true, "2", "source").unwrap();
        assert!(by_source.starts_with("@article{ref1,"), "{by_source}");
    }

    #[test]
    fn multiple_authors_and_editors_survive_both_ways() {
        let bib = "@incollection{k, author = {Curie, Marie and Pierre Curie}, editor = {Smith, John}, title = {T}, booktitle = {B}, year = {1900}, pages = {1--9}}";
        let ris = bib2ris(bib);
        assert!(ris.contains("AU  - Curie, Marie\nAU  - Curie, Pierre\n"), "{ris}");
        assert!(ris.contains("A2  - Smith, John"), "{ris}");
        assert!(ris.contains("SP  - 1\nEP  - 9"), "{ris}");
        let bib2 = ris2bib(&ris);
        assert!(bib2.contains("author = {Curie, Marie and Curie, Pierre}"), "{bib2}");
        assert!(bib2.contains("editor = {Smith, John}"), "{bib2}");
        assert!(bib2.contains("pages = {1--9}"), "{bib2}");
    }

    #[test]
    fn month_and_access_date_cross_over() {
        let src = "TY  - ELEC\nTI  - A Page\nPY  - 2024/03/09/\nY2  - 2025/01/02/\nUR  - https://example.org/\nER  - \n";
        let out = convert_str(src, "auto", "numeric", true, true, true, "2", "source").unwrap();
        assert!(out.contains("year = {2024}"), "{out}");
        assert!(out.contains("month = {mar}"), "{out}");
        assert!(out.contains("urldate = {2025-01-02}"), "{out}");
        let back = bib2ris(&out);
        assert!(back.contains("DA  - 2024/03//"), "{back}");
        assert!(back.contains("Y2  - 2025-01-02"), "{back}");
    }

    #[test]
    fn single_page_and_open_range_values() {
        assert_eq!(join_pages(Some("379"), Some("423")), "379--423");
        assert_eq!(join_pages(Some("379-423"), None), "379--423");
        assert_eq!(join_pages(Some("e1234"), None), "e1234");
        assert_eq!(join_pages(Some("7"), Some("7")), "7");
        assert_eq!(join_pages(None, Some("423")), "423");
        assert_eq!(split_pages("379--423"), ("379".into(), "423".into()));
        assert_eq!(split_pages("e1234"), ("e1234".into(), String::new()));
    }

    #[test]
    fn record_without_er_still_parses() {
        let recs = parse_ris("TY  - JOUR\nTI  - One\nTY  - BOOK\nTI  - Two\n").unwrap();
        assert_eq!(recs.len(), 2);
        assert_eq!(recs[1].first("TY"), Some("BOOK"));
    }

    #[test]
    fn prose_that_looks_like_a_tag_is_treated_as_continuation() {
        let src = "TY  - JOUR\nAB  - We ran an\nIn - depth analysis of it\nPY  - 2020\nER  - \n";
        let out = convert_str(src, "auto", "numeric", true, true, true, "2", "source").unwrap();
        assert!(out.contains("abstract = {We ran an In - depth analysis of it}"), "{out}");
    }
}
