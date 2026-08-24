//! gizza-ai/bibtex-to-csv core — turn a BibTeX bibliography into a CSV table,
//! one row per `@type{key, ...}` entry. Pure Rust, no dependencies: the BibTeX
//! tokenizer, the `@string` macro expander, the LaTeX accent decoder, the author
//! name splitter and the RFC-4180 writer are all hand-rolled (BibTeX is not
//! valid XML/JSON and has no maintained wasm-safe parsing crate worth the risk).
//! Shared by the chat skill block, the CLI and the page.

/// Largest input accepted, in bytes. Above this the browser tab would stall long
/// enough to look hung, so it is a hard, documented error instead.
pub const MAX_INPUT_BYTES: usize = 1_000_000;

/// Largest number of columns accepted in `columns = "custom"` mode.
pub const MAX_COLUMNS: usize = 200;

/// The default column set: the fields a bibliography actually gets consumed by.
/// `journal` and `booktitle` stay SEPARATE (an `@inproceedings` can carry both).
pub const STANDARD_COLUMNS: [&str; 15] = [
    "type",
    "key",
    "title",
    "author",
    "year",
    "journal",
    "booktitle",
    "volume",
    "number",
    "pages",
    "publisher",
    "doi",
    "isbn",
    "issn",
    "url",
];

/// Output field separator.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Delimiter {
    Comma,
    Semicolon,
    Tab,
    Pipe,
}

impl Delimiter {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "comma" | "," => Ok(Delimiter::Comma),
            "semicolon" | ";" => Ok(Delimiter::Semicolon),
            "tab" | "\t" => Ok(Delimiter::Tab),
            "pipe" | "|" => Ok(Delimiter::Pipe),
            other => Err(format!(
                "invalid delimiter '{other}': expected one of comma, semicolon, tab, pipe"
            )),
        }
    }
    fn ch(self) -> char {
        match self {
            Delimiter::Comma => ',',
            Delimiter::Semicolon => ';',
            Delimiter::Tab => '\t',
            Delimiter::Pipe => '|',
        }
    }
}

/// Which columns the CSV carries.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Columns {
    /// [`STANDARD_COLUMNS`] — the default.
    Standard,
    /// `type`, `key`, then every field name found anywhere, alphabetically.
    All,
    /// Exactly the columns named in `custom_columns`, in that order.
    Custom,
}

impl Columns {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "standard" => Ok(Columns::Standard),
            "all" => Ok(Columns::All),
            "custom" => Ok(Columns::Custom),
            other => Err(format!(
                "invalid columns '{other}': expected one of standard, all, custom"
            )),
        }
    }
}

/// How each author name is written in the `author`/`editor` columns.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AuthorFormat {
    /// Keep the source spelling (after LaTeX decoding). The default.
    Bibtex,
    /// `Curie, Marie`
    LastFirst,
    /// `Marie Curie`
    FirstLast,
}

impl AuthorFormat {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "bibtex" => Ok(AuthorFormat::Bibtex),
            "last-first" => Ok(AuthorFormat::LastFirst),
            "first-last" => Ok(AuthorFormat::FirstLast),
            other => Err(format!(
                "invalid author_format '{other}': expected one of bibtex, last-first, first-last"
            )),
        }
    }
}

/// What joins two author names inside one cell.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AuthorSeparator {
    /// ` and ` — the BibTeX spelling. The default.
    And,
    /// `; `
    Semicolon,
    /// `, `
    Comma,
    /// ` | `
    Pipe,
}

impl AuthorSeparator {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "and" => Ok(AuthorSeparator::And),
            "semicolon" => Ok(AuthorSeparator::Semicolon),
            "comma" => Ok(AuthorSeparator::Comma),
            "pipe" => Ok(AuthorSeparator::Pipe),
            other => Err(format!(
                "invalid author_separator '{other}': expected one of and, semicolon, comma, pipe"
            )),
        }
    }
    fn text(self) -> &'static str {
        match self {
            AuthorSeparator::And => " and ",
            AuthorSeparator::Semicolon => "; ",
            AuthorSeparator::Comma => ", ",
            AuthorSeparator::Pipe => " | ",
        }
    }
}

/// Row order.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Sort {
    /// Source order. The default.
    Source,
    /// Case-insensitive by cite key.
    Key,
    /// Ascending by `year`; entries with no parseable year go last.
    Year,
    /// By entry type, then cite key.
    Type,
}

impl Sort {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "source" | "none" => Ok(Sort::Source),
            "key" => Ok(Sort::Key),
            "year" => Ok(Sort::Year),
            "type" => Ok(Sort::Type),
            other => Err(format!(
                "invalid sort '{other}': expected one of source, key, year, type"
            )),
        }
    }
}

/// One parsed `@type{key, field = value, ...}` entry.
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub kind: String,
    pub key: String,
    /// Field name (lowercased) → value, in source order.
    pub fields: Vec<(String, String)>,
}

impl Entry {
    fn get(&self, name: &str) -> Option<&str> {
        self.fields
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }
}

/// Convert BibTeX source to CSV. String-typed façade shared by every surface.
///
/// - `bibtex`: the `.bib` source. At most [`MAX_INPUT_BYTES`] bytes.
/// - `columns`: `standard` (default) | `all` | `custom`.
/// - `custom_columns`: comma-separated column names, required when
///   `columns = "custom"`, ignored otherwise. `type` and `key` are accepted as
///   virtual columns alongside real field names.
/// - `delimiter`: `comma` (default) | `semicolon` | `tab` | `pipe`.
/// - `header`: emit the column-name header row.
/// - `decode_latex`: decode LaTeX accent/symbol macros and drop protective braces.
/// - `author_format`: `bibtex` (default) | `last-first` | `first-last`.
/// - `author_separator`: `and` (default) | `semicolon` | `comma` | `pipe`.
/// - `expand_strings`: resolve `@string` macros and `#` concatenation.
/// - `sort`: `source` (default) | `key` | `year` | `type`.
/// - `bom`: prepend a UTF-8 byte-order mark so Excel opens it as UTF-8.
///
/// Returns `Err` with a human-readable message on a parse error, an unknown
/// option value, an over-cap input, or an empty/entry-less bibliography.
#[allow(clippy::too_many_arguments)]
pub fn convert_str(
    bibtex: &str,
    columns: &str,
    custom_columns: &str,
    delimiter: &str,
    header: bool,
    decode_latex: bool,
    author_format: &str,
    author_separator: &str,
    expand_strings: bool,
    sort: &str,
    bom: bool,
) -> Result<String, String> {
    convert(
        bibtex,
        Columns::parse(columns)?,
        custom_columns,
        Delimiter::parse(delimiter)?,
        header,
        decode_latex,
        AuthorFormat::parse(author_format)?,
        AuthorSeparator::parse(author_separator)?,
        expand_strings,
        Sort::parse(sort)?,
        bom,
    )
}

/// Typed conversion. See [`convert_str`] for the parameter meanings.
#[allow(clippy::too_many_arguments)]
pub fn convert(
    bibtex: &str,
    columns: Columns,
    custom_columns: &str,
    delimiter: Delimiter,
    header: bool,
    decode_latex: bool,
    author_format: AuthorFormat,
    author_separator: AuthorSeparator,
    expand_strings: bool,
    sort: Sort,
    bom: bool,
) -> Result<String, String> {
    if bibtex.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "input is {} bytes, over the {MAX_INPUT_BYTES}-byte limit: split the .bib file and convert it in parts",
            bibtex.len()
        ));
    }
    if bibtex.trim().is_empty() {
        return Err("no BibTeX to convert: paste the contents of a .bib file, e.g. @article{key, title = {…}, author = {…}, year = {2024}}".into());
    }

    let mut entries = parse(bibtex, expand_strings)?;
    if entries.is_empty() {
        return Err("no BibTeX entries found: the input has no @type{key, ...} entry (only @string/@comment/@preamble items or free text)".into());
    }

    // Author-ish fields get name-splitting before LaTeX brace removal, so a
    // brace-protected corporate name such as `{The MIT Press}` stays whole.
    if author_format != AuthorFormat::Bibtex || author_separator != AuthorSeparator::And {
        for e in &mut entries {
            for (name, v) in &mut e.fields {
                if matches!(name.as_str(), "author" | "editor" | "translator") {
                    *v = format_names(v, author_format, author_separator);
                }
            }
        }
    }

    if decode_latex {
        for e in &mut entries {
            for (_, v) in &mut e.fields {
                *v = decode_latex_text(v);
            }
        }
    }

    sort_entries(&mut entries, sort);

    let cols = resolve_columns(columns, custom_columns, &entries)?;

    let sep = delimiter.ch();
    let mut out = String::new();
    if bom {
        out.push('\u{feff}');
    }
    if header {
        push_row(&mut out, cols.iter().map(|c| c.as_str()), sep);
    }
    for e in &entries {
        push_row(
            &mut out,
            cols.iter().map(|c| match c.as_str() {
                "type" => e.kind.as_str(),
                "key" => e.key.as_str(),
                other => e.get(other).unwrap_or(""),
            }),
            sep,
        );
    }
    // Trim the trailing newline: every surface (page, CLI, chat) renders the CSV
    // as text, and a dangling blank line reads as an empty final record.
    if out.ends_with('\n') {
        out.pop();
    }
    Ok(out)
}

fn push_row<'a>(out: &mut String, cells: impl Iterator<Item = &'a str>, sep: char) {
    let mut first = true;
    for cell in cells {
        if !first {
            out.push(sep);
        }
        first = false;
        out.push_str(&csv_escape(cell, sep));
    }
    out.push('\n');
}

/// RFC 4180: quote only when the cell contains the separator, a quote, CR or LF.
fn csv_escape(s: &str, sep: char) -> String {
    if s.contains(sep) || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn sort_entries(entries: &mut [Entry], sort: Sort) {
    match sort {
        Sort::Source => {}
        Sort::Key => entries.sort_by_key(|e| e.key.to_lowercase()),
        Sort::Year => entries.sort_by_key(|e| {
            // Missing/unparseable years sort last, then by key for stability.
            let y = e
                .get("year")
                .and_then(|v| {
                    let digits: String = v.chars().filter(|c| c.is_ascii_digit()).collect();
                    digits.parse::<i64>().ok()
                })
                .unwrap_or(i64::MAX);
            (y, e.key.to_lowercase())
        }),
        Sort::Type => entries.sort_by_key(|e| (e.kind.to_lowercase(), e.key.to_lowercase())),
    }
}

fn resolve_columns(
    columns: Columns,
    custom_columns: &str,
    entries: &[Entry],
) -> Result<Vec<String>, String> {
    match columns {
        Columns::Standard => Ok(STANDARD_COLUMNS.iter().map(|s| s.to_string()).collect()),
        Columns::All => {
            let mut names: Vec<String> = Vec::new();
            for e in entries {
                for (n, _) in &e.fields {
                    if !names.iter().any(|x| x == n) {
                        names.push(n.clone());
                    }
                }
            }
            names.sort();
            let mut cols = vec!["type".to_string(), "key".to_string()];
            cols.extend(names.into_iter().filter(|n| n != "type" && n != "key"));
            Ok(cols)
        }
        Columns::Custom => {
            let cols: Vec<String> = custom_columns
                .split(',')
                .map(|s| s.trim().to_lowercase())
                .filter(|s| !s.is_empty())
                .collect();
            if cols.is_empty() {
                return Err("columns is 'custom' but custom_columns is empty: list the columns you want, e.g. key,title,year".into());
            }
            if cols.len() > MAX_COLUMNS {
                return Err(format!(
                    "custom_columns lists {} columns, over the {MAX_COLUMNS}-column limit",
                    cols.len()
                ));
            }
            Ok(cols)
        }
    }
}

// ---------------------------------------------------------------------------
// BibTeX parsing
// ---------------------------------------------------------------------------

/// Parse `.bib` source into entries, expanding `@string` macros when asked.
///
/// `@comment` and `@preamble` items and any free text between items are dropped
/// (standard BibTeX behaviour).
pub fn parse(src: &str, expand_strings: bool) -> Result<Vec<Entry>, String> {
    let b: Vec<char> = src.chars().collect();
    let mut i = 0usize;
    let mut entries: Vec<Entry> = Vec::new();
    let mut macros: Vec<(String, String)> = Vec::new();

    while i < b.len() {
        // Skip to the next '@'; everything in between is an implicit comment.
        while i < b.len() && b[i] != '@' {
            i += 1;
        }
        if i >= b.len() {
            break;
        }
        let at = i;
        // A '@' that is not the first thing on its line is prose (an email
        // address, a stray note) — BibTeX ignores text between entries, so a
        // malformed header only becomes an error when it looks like a real
        // entry, i.e. it starts a line.
        let starts_line = b[..at]
            .iter()
            .rev()
            .take_while(|c| **c != '\n')
            .all(|c| c.is_whitespace());
        i += 1; // '@'
        skip_ws(&b, &mut i);
        let kind_start = i;
        while i < b.len() && (b[i].is_alphanumeric() || b[i] == '_') {
            i += 1;
        }
        let kind: String = b[kind_start..i].iter().collect::<String>().to_lowercase();
        skip_ws(&b, &mut i);
        let open = match b.get(i) {
            _ if kind.is_empty() => None,
            Some('{') => Some('}'),
            Some('(') => Some(')'),
            _ => None,
        };
        let Some(open) = open else {
            if starts_line {
                return Err(format!(
                    "expected an entry after '@' at character {}, e.g. @article{{key, title = {{…}}}}",
                    at + 1
                ));
            }
            i = at + 1;
            continue;
        };
        i += 1;

        match kind.as_str() {
            "comment" | "preamble" => {
                skip_balanced(&b, &mut i, open, &kind)?;
            }
            "string" => {
                let (name, value) = parse_string_def(&b, &mut i, open, &macros, expand_strings)?;
                macros.retain(|(n, _)| *n != name);
                macros.push((name, value));
            }
            _ => {
                let entry = parse_entry(&b, &mut i, open, kind, &macros, expand_strings)?;
                entries.push(entry);
            }
        }
    }
    Ok(entries)
}

fn skip_ws(b: &[char], i: &mut usize) {
    while *i < b.len() && b[*i].is_whitespace() {
        *i += 1;
    }
}

/// Consume through the matching close delimiter, discarding the content.
fn skip_balanced(b: &[char], i: &mut usize, close: char, kind: &str) -> Result<(), String> {
    let mut depth = 1usize;
    while *i < b.len() {
        let c = b[*i];
        *i += 1;
        if c == '{' {
            depth += 1;
        } else if c == '}' || (c == close && close == ')' && depth == 1) {
            depth -= 1;
            if depth == 0 {
                return Ok(());
            }
        }
    }
    Err(format!(
        "unterminated @{kind} block: the closing '{close}' is missing"
    ))
}

fn parse_string_def(
    b: &[char],
    i: &mut usize,
    close: char,
    macros: &[(String, String)],
    expand: bool,
) -> Result<(String, String), String> {
    skip_ws(b, i);
    let name = read_name(b, i);
    if name.is_empty() {
        return Err(format!(
            "expected a macro name after '@string{{' at character {}, e.g. @string{{jcp = \"J. Chem. Phys.\"}}",
            *i + 1
        ));
    }
    skip_ws(b, i);
    if b.get(*i) != Some(&'=') {
        return Err(format!(
            "expected '=' after the @string macro '{name}' at character {}",
            *i + 1
        ));
    }
    *i += 1;
    let value = read_value(b, i, macros, expand, &format!("@string macro '{name}'"))?;
    skip_ws(b, i);
    match b.get(*i) {
        Some(c) if *c == close => {
            *i += 1;
            Ok((name.to_lowercase(), value))
        }
        _ => Err(format!(
            "unterminated @string block for '{name}': the closing '{close}' is missing"
        )),
    }
}

fn parse_entry(
    b: &[char],
    i: &mut usize,
    close: char,
    kind: String,
    macros: &[(String, String)],
    expand: bool,
) -> Result<Entry, String> {
    skip_ws(b, i);
    // Cite key: everything up to the first ',' or the closing delimiter.
    let key_start = *i;
    while *i < b.len() && b[*i] != ',' && b[*i] != close && b[*i] != '\n' {
        *i += 1;
    }
    let key: String = b[key_start..*i].iter().collect::<String>().trim().to_string();
    let mut fields: Vec<(String, String)> = Vec::new();

    loop {
        skip_ws(b, i);
        match b.get(*i) {
            None => {
                return Err(format!(
                    "unterminated @{kind} entry '{key}': the closing '{close}' is missing"
                ))
            }
            Some(c) if *c == close => {
                *i += 1;
                break;
            }
            Some(',') => {
                *i += 1;
                continue;
            }
            _ => {}
        }
        let name = read_name(b, i);
        if name.is_empty() {
            return Err(format!(
                "expected a field name in @{kind} entry '{key}' at character {}, e.g. title = {{…}}",
                *i + 1
            ));
        }
        skip_ws(b, i);
        if b.get(*i) != Some(&'=') {
            return Err(format!(
                "expected '=' after field '{name}' in @{kind} entry '{key}' at character {}",
                *i + 1
            ));
        }
        *i += 1;
        let where_ = format!("field '{name}' of @{kind} entry '{key}'");
        let value = read_value(b, i, macros, expand, &where_)?;
        let lname = name.to_lowercase();
        fields.retain(|(n, _)| *n != lname);
        fields.push((lname, value));
    }
    Ok(Entry { kind, key, fields })
}

fn read_name(b: &[char], i: &mut usize) -> String {
    let start = *i;
    while *i < b.len() {
        let c = b[*i];
        if c.is_whitespace() || c == '=' || c == ',' || c == '}' || c == ')' || c == '#' {
            break;
        }
        *i += 1;
    }
    b[start..*i].iter().collect()
}

/// Read one field value: `{…}` / `"…"` / a bare number / a macro name, plus any
/// `#`-concatenated continuation.
fn read_value(
    b: &[char],
    i: &mut usize,
    macros: &[(String, String)],
    expand: bool,
    where_: &str,
) -> Result<String, String> {
    let mut out = String::new();
    loop {
        skip_ws(b, i);
        match b.get(*i) {
            Some('{') => {
                *i += 1;
                out.push_str(&read_braced(b, i, where_)?);
            }
            Some('"') => {
                *i += 1;
                out.push_str(&read_quoted(b, i, where_)?);
            }
            Some(c) if c.is_ascii_digit() => {
                let start = *i;
                while *i < b.len() && b[*i].is_ascii_digit() {
                    *i += 1;
                }
                out.extend(b[start..*i].iter());
            }
            Some(c) if c.is_alphabetic() || *c == '_' => {
                let name = read_name(b, i);
                let lname = name.to_lowercase();
                match macros.iter().find(|(n, _)| *n == lname) {
                    Some((_, v)) if expand => out.push_str(v),
                    _ => out.push_str(&name),
                }
            }
            _ => {
                return Err(format!(
                    "expected a value for {where_} at character {}: use {{braces}}, \"quotes\", a number, or a @string macro name",
                    *i + 1
                ))
            }
        }
        // `#` concatenation.
        let save = *i;
        skip_ws(b, i);
        if b.get(*i) == Some(&'#') {
            *i += 1;
            continue;
        }
        *i = save;
        return Ok(normalize_ws(&out));
    }
}

/// Read a `{…}` group's contents, keeping nested braces (they are protection
/// markers the LaTeX decoder deals with later).
fn read_braced(b: &[char], i: &mut usize, where_: &str) -> Result<String, String> {
    let mut depth = 1usize;
    let mut out = String::new();
    while *i < b.len() {
        let c = b[*i];
        *i += 1;
        match c {
            '\\' if *i < b.len() => {
                out.push('\\');
                out.push(b[*i]);
                *i += 1;
            }
            '{' => {
                depth += 1;
                out.push('{');
            }
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Ok(out);
                }
                out.push('}');
            }
            _ => out.push(c),
        }
    }
    Err(format!(
        "unterminated {{ … }} value in {where_}: the closing '}}' is missing"
    ))
}

fn read_quoted(b: &[char], i: &mut usize, where_: &str) -> Result<String, String> {
    let mut depth = 0usize;
    let mut out = String::new();
    while *i < b.len() {
        let c = b[*i];
        *i += 1;
        match c {
            '\\' if *i < b.len() => {
                out.push('\\');
                out.push(b[*i]);
                *i += 1;
            }
            '{' => {
                depth += 1;
                out.push('{');
            }
            '}' => {
                depth = depth.saturating_sub(1);
                out.push('}');
            }
            '"' if depth == 0 => return Ok(out),
            _ => out.push(c),
        }
    }
    Err(format!(
        "unterminated \" … \" value in {where_}: the closing '\"' is missing"
    ))
}

/// Collapse the newlines + runs of spaces BibTeX allows inside a value.
fn normalize_ws(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut pending_space = false;
    for c in s.chars() {
        if c.is_whitespace() {
            pending_space = !out.is_empty();
        } else {
            if pending_space {
                out.push(' ');
            }
            pending_space = false;
            out.push(c);
        }
    }
    out
}

// ---------------------------------------------------------------------------
// LaTeX decoding
// ---------------------------------------------------------------------------

/// `\<accent><letter>` and `\<accent>{<letter>}` combining rules, as a flat
/// (accent, letter, replacement) table — small enough that a scan beats a map.
const ACCENTS: &[(char, char, char)] = &[
    ('\'', 'a', 'á'), ('\'', 'e', 'é'), ('\'', 'i', 'í'), ('\'', 'o', 'ó'), ('\'', 'u', 'ú'),
    ('\'', 'y', 'ý'), ('\'', 'c', 'ć'), ('\'', 'n', 'ń'), ('\'', 's', 'ś'), ('\'', 'z', 'ź'),
    ('\'', 'A', 'Á'), ('\'', 'E', 'É'), ('\'', 'I', 'Í'), ('\'', 'O', 'Ó'), ('\'', 'U', 'Ú'),
    ('\'', 'C', 'Ć'), ('\'', 'N', 'Ń'), ('\'', 'S', 'Ś'), ('\'', 'Z', 'Ź'),
    ('`', 'a', 'à'), ('`', 'e', 'è'), ('`', 'i', 'ì'), ('`', 'o', 'ò'), ('`', 'u', 'ù'),
    ('`', 'A', 'À'), ('`', 'E', 'È'), ('`', 'I', 'Ì'), ('`', 'O', 'Ò'), ('`', 'U', 'Ù'),
    ('"', 'a', 'ä'), ('"', 'e', 'ë'), ('"', 'i', 'ï'), ('"', 'o', 'ö'), ('"', 'u', 'ü'),
    ('"', 'y', 'ÿ'), ('"', 'A', 'Ä'), ('"', 'E', 'Ë'), ('"', 'I', 'Ï'), ('"', 'O', 'Ö'),
    ('"', 'U', 'Ü'),
    ('^', 'a', 'â'), ('^', 'e', 'ê'), ('^', 'i', 'î'), ('^', 'o', 'ô'), ('^', 'u', 'û'),
    ('^', 'A', 'Â'), ('^', 'E', 'Ê'), ('^', 'I', 'Î'), ('^', 'O', 'Ô'), ('^', 'U', 'Û'),
    ('~', 'a', 'ã'), ('~', 'n', 'ñ'), ('~', 'o', 'õ'), ('~', 'A', 'Ã'), ('~', 'N', 'Ñ'),
    ('~', 'O', 'Õ'),
    ('c', 'c', 'ç'), ('c', 'C', 'Ç'), ('c', 's', 'ş'), ('c', 'S', 'Ş'),
    ('v', 'c', 'č'), ('v', 'C', 'Č'), ('v', 's', 'š'), ('v', 'S', 'Š'), ('v', 'z', 'ž'),
    ('v', 'Z', 'Ž'), ('v', 'r', 'ř'), ('v', 'e', 'ě'), ('v', 'n', 'ň'),
    ('=', 'a', 'ā'), ('=', 'e', 'ē'), ('=', 'i', 'ī'), ('=', 'o', 'ō'), ('=', 'u', 'ū'),
    ('.', 'z', 'ż'), ('.', 'e', 'ė'), ('.', 'c', 'ċ'),
    ('u', 'a', 'ă'), ('u', 'g', 'ğ'), ('u', 'G', 'Ğ'),
    ('r', 'a', 'å'), ('r', 'A', 'Å'), ('r', 'u', 'ů'),
    ('H', 'o', 'ő'), ('H', 'u', 'ű'),
    ('k', 'a', 'ą'), ('k', 'e', 'ę'),
];

/// Standalone `\<name>` macros (longest first when they share a prefix).
const SYMBOLS: &[(&str, &str)] = &[
    ("textquotedblleft", "\u{201c}"),
    ("textquotedblright", "\u{201d}"),
    ("textemdash", "—"),
    ("textendash", "–"),
    ("textbullet", "•"),
    ("textdegree", "°"),
    ("textperiodcentered", "·"),
    ("textquoteleft", "\u{2018}"),
    ("textquoteright", "\u{2019}"),
    ("textbackslash", "\\"),
    ("textasciitilde", "~"),
    ("copyright", "©"),
    ("pounds", "£"),
    ("ldots", "…"),
    ("dots", "…"),
    ("AE", "Æ"),
    ("ae", "æ"),
    ("OE", "Œ"),
    ("oe", "œ"),
    ("aa", "å"),
    ("AA", "Å"),
    ("ss", "ß"),
    ("dh", "ð"),
    ("th", "þ"),
    ("DH", "Ð"),
    ("TH", "Þ"),
    ("dj", "đ"),
    ("DJ", "Đ"),
    ("ng", "ŋ"),
    ("i", "ı"),
    ("j", "ȷ"),
    ("l", "ł"),
    ("L", "Ł"),
    ("o", "ø"),
    ("O", "Ø"),
];

/// Decode LaTeX accent/symbol macros to UTF-8 and drop protective braces.
///
/// `{\"a}` → `ä`, `\'{e}` → `é`, `{DNA}` → `DNA`, `\&` → `&`, `---` → `—`,
/// ` `` `/`''` → curly quotes. Unknown macros keep their name (`\relax` →
/// `relax`) rather than vanishing silently.
pub fn decode_latex_text(s: &str) -> String {
    let b: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0usize;
    while i < b.len() {
        match b[i] {
            '\\' => {
                i += 1;
                let Some(&c) = b.get(i) else {
                    out.push('\\');
                    break;
                };
                // \& \% \$ \# \_ \{ \} — escaped literals.
                if matches!(c, '&' | '%' | '$' | '#' | '_' | '{' | '}') {
                    out.push(c);
                    i += 1;
                    continue;
                }
                // \<accent><letter> or \<accent>{<letter>} — non-alphabetic accents.
                if !c.is_alphabetic() {
                    let acc = c;
                    i += 1;
                    if let Some(ch) = take_accent_arg(&b, &mut i) {
                        match ACCENTS.iter().find(|(a, l, _)| *a == acc && *l == ch) {
                            Some((_, _, r)) => out.push(*r),
                            None => out.push(ch),
                        }
                    }
                    continue;
                }
                // \<word> — a named macro; letter-accents (\c \v \u \H \k \r) are
                // single letters that take an argument.
                let start = i;
                while i < b.len() && b[i].is_alphabetic() {
                    i += 1;
                }
                let name: String = b[start..i].iter().collect();
                if name.len() == 1 && "cvuHkr".contains(&name) {
                    let acc = name.chars().next().unwrap();
                    let save = i;
                    if let Some(ch) = take_accent_arg(&b, &mut i) {
                        match ACCENTS.iter().find(|(a, l, _)| *a == acc && *l == ch) {
                            Some((_, _, r)) => out.push(*r),
                            None => out.push(ch),
                        }
                        continue;
                    }
                    i = save;
                }
                match SYMBOLS.iter().find(|(n, _)| *n == name) {
                    Some((_, r)) => {
                        out.push_str(r);
                        // `\ss{}` / `\o{}` — swallow an empty argument group.
                        if b.get(i) == Some(&'{') && b.get(i + 1) == Some(&'}') {
                            i += 2;
                        }
                    }
                    None => out.push_str(&name),
                }
            }
            // Protective braces: `{DNA}` → `DNA`.
            '{' | '}' => i += 1,
            '-' if b.get(i + 1) == Some(&'-') => {
                if b.get(i + 2) == Some(&'-') {
                    out.push('—');
                    i += 3;
                } else {
                    out.push('–');
                    i += 2;
                }
            }
            '`' if b.get(i + 1) == Some(&'`') => {
                out.push('\u{201c}');
                i += 2;
            }
            '\'' if b.get(i + 1) == Some(&'\'') => {
                out.push('\u{201d}');
                i += 2;
            }
            c => {
                out.push(c);
                i += 1;
            }
        }
    }
    out
}

/// After an accent macro, take its letter: `a`, `{a}`, or `\i` (dotless i).
fn take_accent_arg(b: &[char], i: &mut usize) -> Option<char> {
    while b.get(*i).is_some_and(|c| c.is_whitespace()) {
        *i += 1;
    }
    if b.get(*i) == Some(&'{') {
        *i += 1;
        let ch = if b.get(*i) == Some(&'\\') {
            // `\'{\i}` → the accent applies to a dotless i; treat it as 'i'.
            *i += 1;
            while *i < b.len() && b[*i].is_alphabetic() {
                *i += 1;
            }
            'i'
        } else {
            let c = *b.get(*i)?;
            *i += 1;
            c
        };
        if b.get(*i) == Some(&'}') {
            *i += 1;
        }
        Some(ch)
    } else {
        let c = *b.get(*i)?;
        *i += 1;
        Some(c)
    }
}

// ---------------------------------------------------------------------------
// Author names
// ---------------------------------------------------------------------------

/// Re-spell a BibTeX ` and `-separated name list.
pub fn format_names(value: &str, fmt: AuthorFormat, sep: AuthorSeparator) -> String {
    let names = split_names(value);
    let rendered: Vec<String> = names
        .iter()
        .map(|n| match fmt {
            AuthorFormat::Bibtex => n.trim().to_string(),
            AuthorFormat::LastFirst => {
                let (first, last) = split_name(n);
                if first.is_empty() {
                    last
                } else {
                    format!("{last}, {first}")
                }
            }
            AuthorFormat::FirstLast => {
                let (first, last) = split_name(n);
                if first.is_empty() {
                    last
                } else {
                    format!("{first} {last}")
                }
            }
        })
        .collect();
    rendered.join(sep.text())
}

/// Split on the BibTeX ` and ` separator (case-insensitive, whitespace-delimited,
/// not inside braces — `{Smith and Sons Ltd}` stays one name).
fn split_names(value: &str) -> Vec<String> {
    let b: Vec<char> = value.chars().collect();
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut depth = 0usize;
    let mut i = 0usize;
    while i < b.len() {
        match b[i] {
            '{' => {
                depth += 1;
                cur.push('{');
                i += 1;
            }
            '}' => {
                depth = depth.saturating_sub(1);
                cur.push('}');
                i += 1;
            }
            c if depth == 0
                && c.is_whitespace()
                && b.get(i + 1).is_some_and(|x| *x == 'a' || *x == 'A')
                && b.get(i + 2).is_some_and(|x| *x == 'n' || *x == 'N')
                && b.get(i + 3).is_some_and(|x| *x == 'd' || *x == 'D')
                && b.get(i + 4).is_some_and(|x| x.is_whitespace()) =>
            {
                out.push(cur.trim().to_string());
                cur = String::new();
                i += 5;
            }
            c => {
                cur.push(c);
                i += 1;
            }
        }
    }
    if !cur.trim().is_empty() {
        out.push(cur.trim().to_string());
    }
    out
}

/// Split one name into `(first, last)`.
///
/// `von Last, First` / `von Last, Jr, First` (comma form) or `First von Last`
/// (natural form). A single-token name is all "last".
fn split_name(name: &str) -> (String, String) {
    let name = name.trim();
    if name.starts_with('{') && name.ends_with('}') {
        return (String::new(), name.to_string());
    }
    if let Some(idx) = name.find(',') {
        let last = name[..idx].trim().to_string();
        let rest = name[idx + 1..].trim();
        // `Last, Jr, First` — the suffix stays glued to the last name.
        let (first, last) = match rest.split_once(',') {
            Some((jr, first)) => (
                first.trim().to_string(),
                format!("{last} {}", jr.trim()).trim().to_string(),
            ),
            None => (rest.to_string(), last),
        };
        return (first, last);
    }
    let parts: Vec<&str> = name.split_whitespace().collect();
    if parts.len() < 2 {
        return (String::new(), name.to_string());
    }
    // The last name starts at the first lowercase particle (von, van, de, …)
    // that is not the final token; otherwise it is just the final token.
    let start = (0..parts.len() - 1)
        .find(|&i| {
            i > 0
                && parts[i]
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_lowercase() && c.is_alphabetic())
        })
        .unwrap_or(parts.len() - 1);
    (parts[..start].join(" "), parts[start..].join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"@article{curie1898,
  title  = {Sur une substance nouvelle radio-active},
  author = {Curie, Marie and Curie, Pierre},
  year   = {1898},
  journal = {Comptes Rendus},
  volume = {127},
  pages  = {175--178}
}"#;

    fn defaults(src: &str) -> Result<String, String> {
        convert_str(
            src, "standard", "", "comma", true, true, "bibtex", "and", true, "source", false,
        )
    }

    #[test]
    fn happy_path_standard_columns() {
        let csv = defaults(SAMPLE).unwrap();
        let expected = "type,key,title,author,year,journal,booktitle,volume,number,pages,publisher,doi,isbn,issn,url\n\
article,curie1898,Sur une substance nouvelle radio-active,\"Curie, Marie and Curie, Pierre\",1898,Comptes Rendus,,127,,175–178,,,,,";
        assert_eq!(csv, expected);
    }

    #[test]
    fn error_on_unterminated_entry() {
        let err = defaults("@article{smith2020, title = {Half an entry}").unwrap_err();
        assert!(
            err.contains("unterminated") && err.contains("smith2020"),
            "unhelpful error: {err}"
        );
    }

    #[test]
    fn error_on_empty_input() {
        let err = defaults("   \n  ").unwrap_err();
        assert!(err.contains("no BibTeX to convert"), "{err}");
    }

    #[test]
    fn error_when_no_entries_present() {
        let err = defaults("@comment{just a note}\n@string{jcp = \"J. Chem. Phys.\"}").unwrap_err();
        assert!(err.contains("no BibTeX entries found"), "{err}");
    }

    #[test]
    fn error_on_missing_equals() {
        let err = defaults("@book{b1, title {No equals}}").unwrap_err();
        assert!(
            err.contains("expected '='") && err.contains("title"),
            "{err}"
        );
    }

    #[test]
    fn error_on_bad_option_values() {
        let bad = convert_str(
            SAMPLE, "standard", "", "colon", true, true, "bibtex", "and", true, "source", false,
        )
        .unwrap_err();
        assert!(bad.contains("invalid delimiter 'colon'"), "{bad}");
        let bad = convert_str(
            SAMPLE, "sideways", "", "comma", true, true, "bibtex", "and", true, "source", false,
        )
        .unwrap_err();
        assert!(bad.contains("invalid columns 'sideways'"), "{bad}");
    }

    #[test]
    fn input_cap_boundary() {
        // Exactly at the cap parses; one byte over is refused.
        let filler = "x".repeat(MAX_INPUT_BYTES - SAMPLE.len() - 1);
        let at_cap = format!("{SAMPLE}\n{filler}");
        assert_eq!(at_cap.len(), MAX_INPUT_BYTES);
        assert!(defaults(&at_cap).is_ok());
        let over = format!("{at_cap}x");
        assert_eq!(over.len(), MAX_INPUT_BYTES + 1);
        let err = defaults(&over).unwrap_err();
        assert!(err.contains("over the 1000000-byte limit"), "{err}");
    }

    #[test]
    fn all_columns_is_the_union_of_fields() {
        let src = "@book{a, title={A}, isbn={1}}\n@misc{b, note={N}, title={B}}";
        let csv = convert_str(
            src, "all", "", "comma", true, true, "bibtex", "and", true, "source", false,
        )
        .unwrap();
        assert_eq!(
            csv,
            "type,key,isbn,note,title\nbook,a,1,,A\nmisc,b,,N,B"
        );
    }

    #[test]
    fn custom_columns_pick_and_order() {
        let csv = convert_str(
            SAMPLE, "custom", "year, Title ,key", "comma", true, true, "bibtex", "and", true,
            "source", false,
        )
        .unwrap();
        assert_eq!(
            csv,
            "year,title,key\n1898,Sur une substance nouvelle radio-active,curie1898"
        );
    }

    #[test]
    fn custom_columns_must_not_be_empty() {
        let err = convert_str(
            SAMPLE, "custom", "  ", "comma", true, true, "bibtex", "and", true, "source", false,
        )
        .unwrap_err();
        assert!(err.contains("custom_columns is empty"), "{err}");
    }

    #[test]
    fn custom_columns_cap_boundary() {
        let at_cap = (0..MAX_COLUMNS)
            .map(|i| format!("c{i}"))
            .collect::<Vec<_>>()
            .join(",");
        assert!(convert_str(
            SAMPLE, "custom", &at_cap, "comma", false, true, "bibtex", "and", true, "source", false,
        )
        .is_ok());
        let over = format!("{at_cap},c200");
        let err = convert_str(
            SAMPLE, "custom", &over, "comma", false, true, "bibtex", "and", true, "source", false,
        )
        .unwrap_err();
        assert!(err.contains("over the 200-column limit"), "{err}");
    }

    #[test]
    fn delimiters_and_header_toggle() {
        let src = "@misc{m, title={A, B}}";
        let csv = convert_str(
            src, "custom", "key,title", "semicolon", true, true, "bibtex", "and", true, "source",
            false,
        )
        .unwrap();
        assert_eq!(csv, "key;title\nm;A, B");
        let csv = convert_str(
            src, "custom", "key,title", "tab", false, true, "bibtex", "and", true, "source", false,
        )
        .unwrap();
        assert_eq!(csv, "m\tA, B");
        let csv = convert_str(
            src, "custom", "key,title", "pipe", false, true, "bibtex", "and", true, "source", false,
        )
        .unwrap();
        assert_eq!(csv, "m|A, B");
        // The comma delimiter quotes the comma-bearing title.
        let csv = convert_str(
            src, "custom", "key,title", "comma", false, true, "bibtex", "and", true, "source",
            false,
        )
        .unwrap();
        assert_eq!(csv, "m,\"A, B\"");
    }

    #[test]
    fn latex_decoding_and_brace_stripping() {
        let src = r#"@article{k, title = {Structure of {DNA}: \"Uber die \'{e}tude --- part \#2}, author = {Erd{\H o}s, P\'al}}"#;
        let csv = convert_str(
            src, "custom", "title,author", "comma", false, true, "bibtex", "and", true, "source",
            false,
        )
        .unwrap();
        assert_eq!(csv, "Structure of DNA: Über die étude — part #2,\"Erdős, Pál\"");
        // decode_latex = false keeps the source spelling verbatim.
        let raw = convert_str(
            src, "custom", "title", "comma", false, false, "bibtex", "and", true, "source", false,
        )
        .unwrap();
        assert!(raw.contains("{DNA}") && raw.contains(r#"\'{e}"#), "{raw}");
    }

    #[test]
    fn author_formats_and_separators() {
        let src = "@misc{m, author = {Curie, Marie and Ludwig van Beethoven and {The MIT Press}}}";
        let last_first = convert_str(
            src, "custom", "author", "comma", false, true, "last-first", "semicolon", true,
            "source", false,
        )
        .unwrap();
        assert_eq!(
            last_first,
            "\"Curie, Marie; van Beethoven, Ludwig; The MIT Press\""
        );
        let first_last = convert_str(
            src, "custom", "author", "comma", false, true, "first-last", "pipe", true, "source",
            false,
        )
        .unwrap();
        assert_eq!(first_last, "Marie Curie | Ludwig van Beethoven | The MIT Press");
    }

    #[test]
    fn string_macros_expand_and_concatenate() {
        let src = r#"@string{jcp = "J. Chem. Phys."}
@article{a, journal = jcp # " (Letters)", title = "T"}"#;
        let csv = convert_str(
            src, "custom", "journal", "comma", false, true, "bibtex", "and", true, "source", false,
        )
        .unwrap();
        assert_eq!(csv, "J. Chem. Phys. (Letters)");
        // expand_strings = false leaves the macro name in place.
        let raw = convert_str(
            src, "custom", "journal", "comma", false, true, "bibtex", "and", false, "source", false,
        )
        .unwrap();
        assert_eq!(raw, "jcp (Letters)");
    }

    #[test]
    fn sorting_modes() {
        let src = "@misc{zeta, year={2001}}\n@article{alpha, year={1999}}\n@misc{mid}";
        let by_key = convert_str(
            src, "custom", "key", "comma", false, true, "bibtex", "and", true, "key", false,
        )
        .unwrap();
        assert_eq!(by_key, "alpha\nmid\nzeta");
        let by_year = convert_str(
            src, "custom", "key", "comma", false, true, "bibtex", "and", true, "year", false,
        )
        .unwrap();
        assert_eq!(by_year, "alpha\nzeta\nmid");
        let by_type = convert_str(
            src, "custom", "type,key", "comma", false, true, "bibtex", "and", true, "type", false,
        )
        .unwrap();
        assert_eq!(by_type, "article,alpha\nmisc,mid\nmisc,zeta");
        let source_order = convert_str(
            src, "custom", "key", "comma", false, true, "bibtex", "and", true, "source", false,
        )
        .unwrap();
        assert_eq!(source_order, "zeta\nalpha\nmid");
    }

    #[test]
    fn bom_prefixes_the_output() {
        let csv = convert_str(
            "@misc{m, title={T}}", "custom", "title", "comma", false, true, "bibtex", "and", true,
            "source", true,
        )
        .unwrap();
        assert_eq!(csv, "\u{feff}T");
    }

    #[test]
    fn quotes_inside_cells_are_doubled() {
        let src = r#"@misc{m, title = {He said ``no'' twice}}"#;
        let csv = convert_str(
            src, "custom", "title", "comma", false, true, "bibtex", "and", true, "source", false,
        )
        .unwrap();
        assert_eq!(csv, "He said \u{201c}no\u{201d} twice");
        let src = r#"@misc{m, title = {A "quoted" word}}"#;
        let csv = convert_str(
            src, "custom", "title", "comma", false, true, "bibtex", "and", true, "source", false,
        )
        .unwrap();
        assert_eq!(csv, "\"A \"\"quoted\"\" word\"");
    }

    #[test]
    fn quoted_values_parens_and_multiline_are_parsed() {
        let src = "@Article(a,\n  title = \"A very\n     long title\",\n  year = 1999,\n)";
        let csv = convert_str(
            src, "custom", "type,title,year", "comma", false, true, "bibtex", "and", true,
            "source", false,
        )
        .unwrap();
        assert_eq!(csv, "article,A very long title,1999");
    }

    #[test]
    fn free_text_between_entries_is_ignored() {
        let src = "% a stray comment line\n@misc{m, title={T}}\ntrailing junk";
        let csv = convert_str(
            src, "custom", "key,title", "comma", false, true, "bibtex", "and", true, "source",
            false,
        )
        .unwrap();
        assert_eq!(csv, "m,T");
    }
}
