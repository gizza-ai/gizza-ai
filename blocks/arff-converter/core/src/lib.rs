//! arff-converter core — convert Weka ARFF datasets to CSV and back, preserving
//! attribute types. Pure compute, shared by the chat skill block and the web page.
//!
//! Handles the parts of the ARFF specification that trip up naive line splitting:
//! `%` comments, quoted values with `\n`/`\t`/`\\` escapes, nominal label sets,
//! `date` attributes with a format pattern, sparse `{index value, ...}` rows,
//! trailing `{weight}` instance weights and `?` missing values.

/// Largest input accepted, in characters. Keeps peak memory inside the wasm sandbox.
pub const MAX_INPUT_CHARS: usize = 2_000_000;

const DEFAULT_DATE_FORMAT: &str = "yyyy-MM-dd'T'HH:mm:ss";
const DEFAULT_RELATION: &str = "data";

/// Which way to convert.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    /// Sniff the input for `@relation`/`@data` and pick a direction.
    Auto,
    ArffToCsv,
    CsvToArff,
}

impl Direction {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "auto" => Ok(Direction::Auto),
            "arff-to-csv" => Ok(Direction::ArffToCsv),
            "csv-to-arff" => Ok(Direction::CsvToArff),
            other => Err(format!(
                "unknown direction '{other}': use auto/arff-to-csv/csv-to-arff"
            )),
        }
    }
}

/// Dense or sparse `@data` rows when writing ARFF.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArffFormat {
    Dense,
    Sparse,
}

impl ArffFormat {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "dense" => Ok(ArffFormat::Dense),
            "sparse" => Ok(ArffFormat::Sparse),
            other => Err(format!("unknown arff_format '{other}': use dense/sparse")),
        }
    }
}

/// An ARFF attribute type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttrType {
    Numeric,
    Nominal(Vec<String>),
    Text,
    Date(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Attribute {
    pub name: String,
    pub ty: AttrType,
}

/// A parsed dataset. `None` in a cell means the ARFF missing value `?`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dataset {
    pub relation: String,
    pub attributes: Vec<Attribute>,
    pub rows: Vec<Vec<Option<String>>>,
}

#[derive(Debug, Clone)]
pub struct Options {
    pub direction: Direction,
    pub delimiter: String,
    pub header: bool,
    pub relation: String,
    pub nominal_threshold: i64,
    pub column_types: String,
    pub date_format: String,
    pub missing_value: String,
    pub arff_format: ArffFormat,
    pub type_row: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            direction: Direction::Auto,
            delimiter: ",".into(),
            header: true,
            relation: String::new(),
            nominal_threshold: 10,
            column_types: String::new(),
            date_format: DEFAULT_DATE_FORMAT.into(),
            missing_value: String::new(),
            arff_format: ArffFormat::Dense,
            type_row: false,
        }
    }
}

/// Convert `input` in the requested direction. Returns the converted text.
pub fn convert(input: &str, opts: &Options) -> Result<String, String> {
    if input.trim().is_empty() {
        return Err("input is empty: paste an ARFF dataset or a CSV table".into());
    }
    if input.chars().count() > MAX_INPUT_CHARS {
        return Err(format!(
            "input is larger than the {MAX_INPUT_CHARS} character limit: convert it in parts"
        ));
    }
    let direction = match opts.direction {
        Direction::Auto => detect_direction(input),
        d => d,
    };
    match direction {
        Direction::ArffToCsv => {
            let ds = parse_arff(input, opts)?;
            write_csv(&ds, opts)
        }
        Direction::CsvToArff => {
            let ds = parse_csv(input, opts)?;
            write_arff(&ds, opts)
        }
        Direction::Auto => unreachable!("resolved above"),
    }
}

/// True when the text looks like ARFF (an `@relation`, `@attribute` or `@data` line).
pub fn detect_direction(input: &str) -> Direction {
    for line in input.lines() {
        let t = line.trim_start();
        if t.is_empty() || t.starts_with('%') {
            continue;
        }
        if t.starts_with('@') {
            let kw = keyword(t);
            if kw == "relation" || kw == "attribute" || kw == "data" {
                return Direction::ArffToCsv;
            }
        }
        return Direction::CsvToArff;
    }
    Direction::CsvToArff
}

fn keyword(line: &str) -> String {
    line.trim_start_matches('@')
        .split(|c: char| c.is_whitespace())
        .next()
        .unwrap_or("")
        .to_ascii_lowercase()
}

// ---------------------------------------------------------------------------
// shared helpers
// ---------------------------------------------------------------------------

/// Resolve a delimiter spec (`,`, `tab`, `semicolon`, ...) to a single byte.
pub fn delimiter_byte(spec: &str) -> Result<u8, String> {
    if spec.is_empty() {
        return Ok(b',');
    }
    let mut chars = spec.chars();
    if let (Some(c), None) = (chars.next(), chars.next()) {
        if c == '"' {
            return Err("delimiter cannot be a double quote".into());
        }
        if c.is_ascii() {
            return Ok(c as u8);
        }
        return Err(format!("delimiter '{c}' must be a single ASCII character"));
    }
    match spec.trim().to_ascii_lowercase().as_str() {
        "comma" => Ok(b','),
        "tab" | "\\t" => Ok(b'\t'),
        "semicolon" => Ok(b';'),
        "pipe" => Ok(b'|'),
        "space" => Ok(b' '),
        other => Err(format!(
            "unknown delimiter '{other}': use a single character or comma/tab/semicolon/pipe/space"
        )),
    }
}

fn arff_needs_quote(s: &str) -> bool {
    s.is_empty()
        || s == "?"
        || s.chars().any(|c| {
            c.is_whitespace()
                || matches!(c, ',' | '{' | '}' | '%' | '\'' | '"' | '\\')
                || (c as u32) < 0x20
        })
}

/// Quote + escape a value for the ARFF header or `@data` section.
pub fn quote_arff(s: &str) -> String {
    if !arff_needs_quote(s) {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len() + 2);
    out.push('\'');
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\'' => out.push_str("\\'"),
            '"' => out.push_str("\\\""),
            '%' => out.push_str("\\%"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            other => out.push(other),
        }
    }
    out.push('\'');
    out
}

fn unescape_arff(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match it.next() {
            Some('n') => out.push('\n'),
            Some('r') => out.push('\r'),
            Some('t') => out.push('\t'),
            Some('0') => out.push('\0'),
            Some(other) => out.push(other),
            None => out.push('\\'),
        }
    }
    out
}

/// Read one token (bare or quoted) from `s`; returns the token and the rest.
fn read_token(s: &str) -> Result<(String, &str), String> {
    let s = s.trim_start();
    if s.is_empty() {
        return Err("expected a value but the line ended".into());
    }
    let quote = s.chars().next().unwrap();
    if quote == '\'' || quote == '"' {
        let mut escaped = false;
        for (i, c) in s.char_indices().skip(1) {
            if escaped {
                escaped = false;
            } else if c == '\\' {
                escaped = true;
            } else if c == quote {
                return Ok((unescape_arff(&s[1..i]), &s[i + c.len_utf8()..]));
            }
        }
        return Err(format!("unterminated {quote} quote in: {s}"));
    }
    let end = s.find(char::is_whitespace).unwrap_or(s.len());
    Ok((unescape_arff(&s[..end]), &s[end..]))
}

/// Render an attribute type the way it appears after `@attribute <name>`.
pub fn format_attr_type(ty: &AttrType) -> String {
    match ty {
        AttrType::Numeric => "numeric".into(),
        AttrType::Text => "string".into(),
        AttrType::Date(f) => format!("date {}", quote_arff(f)),
        AttrType::Nominal(labels) => {
            let inner: Vec<String> = labels.iter().map(|l| quote_arff(l)).collect();
            format!("{{{}}}", inner.join(","))
        }
    }
}

/// Parse an attribute type declaration (`numeric`, `{a,b}`, `date 'fmt'`, ...).
pub fn parse_attr_type(spec: &str, default_date_format: &str) -> Result<AttrType, String> {
    let spec = spec.trim();
    if spec.is_empty() {
        return Err("attribute is missing a type".into());
    }
    if spec.starts_with('{') {
        let close = spec
            .rfind('}')
            .ok_or_else(|| format!("nominal list is missing a closing brace: {spec}"))?;
        let inner = &spec[1..close];
        let labels = if inner.trim().is_empty() {
            Vec::new()
        } else {
            split_arff_values(inner)?
                .into_iter()
                .map(|v| v.unwrap_or_else(|| "?".to_string()))
                .collect()
        };
        return Ok(AttrType::Nominal(labels));
    }
    let (word, rest) = read_token(spec)?;
    match word.to_ascii_lowercase().as_str() {
        "numeric" | "real" | "integer" => Ok(AttrType::Numeric),
        "string" => Ok(AttrType::Text),
        "date" => {
            let rest = rest.trim();
            if rest.is_empty() {
                Ok(AttrType::Date(default_date_format.to_string()))
            } else {
                let (fmt, _) = read_token(rest)?;
                Ok(AttrType::Date(fmt))
            }
        }
        "relational" => Err(
            "relational (multi-instance) attributes are not supported: flatten the dataset first"
                .into(),
        ),
        other => Err(format!(
            "unknown attribute type '{other}': expected numeric/real/integer, string, date, or a {{...}} nominal list"
        )),
    }
}

/// Split one `@data` row into values. `None` is the missing value `?`.
fn split_arff_values(line: &str) -> Result<Vec<Option<String>>, String> {
    let mut out = Vec::new();
    let bytes: Vec<char> = line.chars().collect();
    let mut i = 0usize;
    loop {
        while i < bytes.len() && (bytes[i] == ' ' || bytes[i] == '\t') {
            i += 1;
        }
        if i < bytes.len() && bytes[i] == '%' {
            // an unquoted % starts a comment: the record ends here
            if out.is_empty() {
                return Ok(out);
            }
            break;
        }
        let value: Option<String>;
        if i < bytes.len() && (bytes[i] == '\'' || bytes[i] == '"') {
            let quote = bytes[i];
            let mut buf = String::new();
            let mut escaped = false;
            i += 1;
            let mut closed = false;
            while i < bytes.len() {
                let c = bytes[i];
                i += 1;
                if escaped {
                    buf.push('\\');
                    buf.push(c);
                    escaped = false;
                } else if c == '\\' {
                    escaped = true;
                } else if c == quote {
                    closed = true;
                    break;
                } else {
                    buf.push(c);
                }
            }
            if !closed {
                return Err(format!("unterminated {quote} quote in row: {line}"));
            }
            value = Some(unescape_arff(&buf));
        } else {
            let start = i;
            while i < bytes.len() && bytes[i] != ',' && bytes[i] != '%' {
                i += 1;
            }
            let raw: String = bytes[start..i].iter().collect();
            let raw = raw.trim();
            value = if raw == "?" {
                None
            } else {
                Some(unescape_arff(raw))
            };
        }
        out.push(value);
        while i < bytes.len() && (bytes[i] == ' ' || bytes[i] == '\t') {
            i += 1;
        }
        if i < bytes.len() && bytes[i] == '%' {
            break;
        }
        if i >= bytes.len() {
            break;
        }
        if bytes[i] != ',' {
            return Err(format!(
                "unexpected character '{}' after a value in row: {line}",
                bytes[i]
            ));
        }
        i += 1;
    }
    Ok(out)
}

/// Drop a trailing `,{2.0}` instance weight, returning the bare record.
fn strip_instance_weight(line: &str) -> &str {
    let t = line.trim_end();
    if !t.ends_with('}') {
        return t;
    }
    let Some(open) = t.rfind('{') else {
        return t;
    };
    let inner = &t[open + 1..t.len() - 1];
    if inner.trim().parse::<f64>().is_err() {
        return t;
    }
    let before = t[..open].trim_end();
    match before.strip_suffix(',') {
        Some(rest) => rest.trim_end(),
        None => t,
    }
}

// ---------------------------------------------------------------------------
// ARFF -> CSV
// ---------------------------------------------------------------------------

/// Parse an ARFF document into a [`Dataset`].
pub fn parse_arff(input: &str, opts: &Options) -> Result<Dataset, String> {
    let mut relation = String::new();
    let mut attributes: Vec<Attribute> = Vec::new();
    let mut rows: Vec<Vec<Option<String>>> = Vec::new();
    let mut in_data = false;
    let date_format = if opts.date_format.trim().is_empty() {
        DEFAULT_DATE_FORMAT
    } else {
        opts.date_format.trim()
    };

    for (lineno, raw_line) in input.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('%') {
            continue;
        }
        if !in_data {
            if !line.starts_with('@') {
                return Err(format!(
                    "line {}: expected an @relation/@attribute/@data declaration, found: {line}",
                    lineno + 1
                ));
            }
            let kw = keyword(line);
            let rest = line[1 + kw.len()..].trim();
            match kw.as_str() {
                "relation" => {
                    relation = if rest.is_empty() {
                        DEFAULT_RELATION.to_string()
                    } else {
                        read_token(rest)
                            .map_err(|e| format!("line {}: {e}", lineno + 1))?
                            .0
                    };
                }
                "attribute" => {
                    let (name, tail) =
                        read_token(rest).map_err(|e| format!("line {}: {e}", lineno + 1))?;
                    let ty = parse_attr_type(tail, date_format)
                        .map_err(|e| format!("line {}: {e}", lineno + 1))?;
                    attributes.push(Attribute { name, ty });
                }
                "data" => {
                    if attributes.is_empty() {
                        return Err(format!(
                            "line {}: @data reached before any @attribute declaration",
                            lineno + 1
                        ));
                    }
                    in_data = true;
                }
                "end" => {
                    return Err(format!(
                        "line {}: relational (multi-instance) attributes are not supported",
                        lineno + 1
                    ))
                }
                other => {
                    return Err(format!(
                        "line {}: unknown declaration '@{other}'",
                        lineno + 1
                    ))
                }
            }
            continue;
        }

        let record = strip_instance_weight(line);
        if record.is_empty() {
            continue;
        }
        let values = if record.starts_with('{') {
            let close = record.rfind('}').ok_or_else(|| {
                format!("line {}: sparse row is missing a closing brace", lineno + 1)
            })?;
            parse_sparse_row(&record[1..close], &attributes)
                .map_err(|e| format!("line {}: {e}", lineno + 1))?
        } else {
            split_arff_values(record).map_err(|e| format!("line {}: {e}", lineno + 1))?
        };
        if values.len() != attributes.len() {
            return Err(format!(
                "line {}: row has {} values but the header declares {} attributes",
                lineno + 1,
                values.len(),
                attributes.len()
            ));
        }
        rows.push(values);
    }

    if attributes.is_empty() {
        return Err(
            "no @attribute declarations found: this does not look like an ARFF file".into(),
        );
    }
    if !in_data {
        return Err("no @data section found: the ARFF file has a header but no data".into());
    }
    if relation.is_empty() {
        relation = DEFAULT_RELATION.to_string();
    }
    Ok(Dataset {
        relation,
        attributes,
        rows,
    })
}

/// Expand a sparse `index value` list into a full row.
fn parse_sparse_row(inner: &str, attributes: &[Attribute]) -> Result<Vec<Option<String>>, String> {
    let mut row: Vec<Option<String>> = attributes.iter().map(|a| sparse_default(&a.ty)).collect();
    if inner.trim().is_empty() {
        return Ok(row);
    }
    for part in split_top_level_commas(inner) {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (idx_str, rest) = match part.find(char::is_whitespace) {
            Some(p) => (&part[..p], &part[p..]),
            None => return Err(format!("sparse entry '{part}' is missing a value")),
        };
        let idx: usize = idx_str
            .parse()
            .map_err(|_| format!("sparse entry '{part}' does not start with a column index"))?;
        if idx >= attributes.len() {
            return Err(format!(
                "sparse index {idx} is past the last attribute (0-{})",
                attributes.len() - 1
            ));
        }
        let mut vals = split_arff_values(rest)?;
        if vals.len() != 1 {
            return Err(format!("sparse entry '{part}' must hold exactly one value"));
        }
        row[idx] = vals.remove(0);
    }
    Ok(row)
}

fn sparse_default(ty: &AttrType) -> Option<String> {
    match ty {
        AttrType::Numeric | AttrType::Date(_) => Some("0".to_string()),
        AttrType::Text => Some(String::new()),
        AttrType::Nominal(labels) => labels.first().cloned(),
    }
}

/// Split on commas that are outside quotes.
fn split_top_level_commas(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut buf = String::new();
    let mut quote: Option<char> = None;
    let mut escaped = false;
    for c in s.chars() {
        if escaped {
            buf.push(c);
            escaped = false;
            continue;
        }
        match quote {
            Some(q) => {
                buf.push(c);
                if c == '\\' {
                    escaped = true;
                } else if c == q {
                    quote = None;
                }
            }
            None => {
                if c == '\'' || c == '"' {
                    quote = Some(c);
                    buf.push(c);
                } else if c == ',' {
                    out.push(std::mem::take(&mut buf));
                } else {
                    buf.push(c);
                }
            }
        }
    }
    out.push(buf);
    out
}

fn write_csv(ds: &Dataset, opts: &Options) -> Result<String, String> {
    let delim = delimiter_byte(&opts.delimiter)?;
    let mut wtr = csv::WriterBuilder::new()
        .delimiter(delim)
        .from_writer(Vec::new());
    if opts.header {
        wtr.write_record(ds.attributes.iter().map(|a| a.name.as_str()))
            .map_err(|e| e.to_string())?;
    }
    if opts.type_row {
        wtr.write_record(ds.attributes.iter().map(|a| format_attr_type(&a.ty)))
            .map_err(|e| e.to_string())?;
    }
    for row in &ds.rows {
        let cells: Vec<&str> = row
            .iter()
            .map(|v| v.as_deref().unwrap_or(opts.missing_value.as_str()))
            .collect();
        wtr.write_record(&cells).map_err(|e| e.to_string())?;
    }
    let bytes = wtr.into_inner().map_err(|e| e.to_string())?;
    let mut out = String::from_utf8(bytes).map_err(|e| e.to_string())?;
    while out.ends_with('\n') || out.ends_with('\r') {
        out.pop();
    }
    Ok(out)
}

// ---------------------------------------------------------------------------
// CSV -> ARFF
// ---------------------------------------------------------------------------

/// Parse a CSV table and infer ARFF attribute types for it.
pub fn parse_csv(input: &str, opts: &Options) -> Result<Dataset, String> {
    let delim = delimiter_byte(&opts.delimiter)?;
    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delim)
        .has_headers(false)
        .flexible(true)
        .from_reader(input.as_bytes());

    let mut records: Vec<Vec<String>> = Vec::new();
    for rec in rdr.records() {
        let rec = rec.map_err(|e| format!("CSV parse error: {e}"))?;
        let cells: Vec<String> = rec.iter().map(|c| c.to_string()).collect();
        if cells.iter().all(|c| c.trim().is_empty()) {
            continue;
        }
        records.push(cells);
    }
    if records.is_empty() {
        return Err("no CSV rows found in the input".into());
    }

    let names: Vec<String> = if opts.header {
        let head = records.remove(0);
        head.iter()
            .enumerate()
            .map(|(i, n)| {
                let n = n.trim();
                if n.is_empty() {
                    format!("att{}", i + 1)
                } else {
                    n.to_string()
                }
            })
            .collect()
    } else {
        let width = records.iter().map(|r| r.len()).max().unwrap_or(0);
        (1..=width).map(|i| format!("att{i}")).collect()
    };
    if names.is_empty() {
        return Err("the CSV header row has no columns".into());
    }

    let date_format = if opts.date_format.trim().is_empty() {
        DEFAULT_DATE_FORMAT
    } else {
        opts.date_format.trim()
    };

    // An optional declared-types row (the counterpart of `type_row` on the way out).
    let mut declared: Option<Vec<AttrType>> = None;
    if opts.type_row {
        if records.is_empty() {
            return Err("the type row is enabled but the CSV has no row after the header".into());
        }
        let row = records.remove(0);
        if row.len() != names.len() {
            return Err(format!(
                "the type row has {} cells but the header has {} columns",
                row.len(),
                names.len()
            ));
        }
        let mut types = Vec::with_capacity(row.len());
        for (i, spec) in row.iter().enumerate() {
            types.push(
                parse_attr_type(spec, date_format)
                    .map_err(|e| format!("type row, column {}: {e}", i + 1))?,
            );
        }
        declared = Some(types);
    }

    // Normalise widths and map empty / missing-token cells to `None`.
    let mut rows: Vec<Vec<Option<String>>> = Vec::with_capacity(records.len());
    for (r, rec) in records.iter().enumerate() {
        if rec.len() > names.len() {
            return Err(format!(
                "data row {} has {} values but the header has {} columns",
                r + 1,
                rec.len(),
                names.len()
            ));
        }
        let mut row: Vec<Option<String>> = Vec::with_capacity(names.len());
        for i in 0..names.len() {
            let cell = rec.get(i).map(|s| s.as_str()).unwrap_or("");
            row.push(cell_to_value(cell, &opts.missing_value));
        }
        rows.push(row);
    }

    let overrides = parse_column_types(&opts.column_types, &names, date_format)?;
    let mut attributes = Vec::with_capacity(names.len());
    for (i, name) in names.iter().enumerate() {
        let ty = if let Some(o) = overrides.get(&i) {
            materialise(o, i, &rows)
        } else if let Some(d) = declared.as_ref() {
            d[i].clone()
        } else {
            infer_type(i, &rows, opts.nominal_threshold)
        };
        attributes.push(Attribute {
            name: name.clone(),
            ty,
        });
    }
    let relation = if opts.relation.trim().is_empty() {
        DEFAULT_RELATION.to_string()
    } else {
        opts.relation.trim().to_string()
    };
    Ok(Dataset {
        relation,
        attributes,
        rows,
    })
}

fn cell_to_value(cell: &str, missing_value: &str) -> Option<String> {
    if cell.is_empty() {
        return None;
    }
    if !missing_value.is_empty() && cell == missing_value {
        return None;
    }
    if missing_value.is_empty() && cell == "?" {
        return None;
    }
    Some(cell.to_string())
}

/// A type override before its label set is known.
#[derive(Debug, Clone, PartialEq, Eq)]
enum TypeOverride {
    Numeric,
    Nominal,
    Text,
    Date(String),
}

fn parse_column_types(
    spec: &str,
    names: &[String],
    date_format: &str,
) -> Result<std::collections::BTreeMap<usize, TypeOverride>, String> {
    let mut out = std::collections::BTreeMap::new();
    if spec.trim().is_empty() {
        return Ok(out);
    }
    for part in spec.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (key, ty) = part.rsplit_once(':').ok_or_else(|| {
            format!(
                "column_types entry '{part}' must look like 'column:type' (e.g. 'class:nominal')"
            )
        })?;
        let key = key.trim();
        let ty = match ty.trim().to_ascii_lowercase().as_str() {
            "numeric" | "real" | "integer" => TypeOverride::Numeric,
            "nominal" => TypeOverride::Nominal,
            "string" => TypeOverride::Text,
            "date" => TypeOverride::Date(date_format.to_string()),
            other => {
                return Err(format!(
                    "unknown type '{other}' in column_types: use numeric/nominal/string/date"
                ))
            }
        };
        let idx = if let Some(pos) = names.iter().position(|n| n.eq_ignore_ascii_case(key)) {
            pos
        } else if let Ok(n) = key.parse::<usize>() {
            if n == 0 || n > names.len() {
                return Err(format!(
                    "column_types index {n} is out of range 1-{}",
                    names.len()
                ));
            }
            n - 1
        } else {
            return Err(format!(
                "column_types refers to unknown column '{key}': use a header name or a 1-based index"
            ));
        };
        out.insert(idx, ty);
    }
    Ok(out)
}

fn materialise(o: &TypeOverride, col: usize, rows: &[Vec<Option<String>>]) -> AttrType {
    match o {
        TypeOverride::Numeric => AttrType::Numeric,
        TypeOverride::Text => AttrType::Text,
        TypeOverride::Date(f) => AttrType::Date(f.clone()),
        TypeOverride::Nominal => AttrType::Nominal(distinct_values(col, rows)),
    }
}

fn distinct_values(col: usize, rows: &[Vec<Option<String>>]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut out = Vec::new();
    for row in rows {
        if let Some(Some(v)) = row.get(col) {
            if seen.insert(v.clone()) {
                out.push(v.clone());
            }
        }
    }
    out
}

fn infer_type(col: usize, rows: &[Vec<Option<String>>], nominal_threshold: i64) -> AttrType {
    let mut all_numeric = true;
    for row in rows {
        if let Some(Some(v)) = row.get(col) {
            if v.trim().parse::<f64>().is_err() {
                all_numeric = false;
                break;
            }
        }
    }
    if all_numeric {
        return AttrType::Numeric;
    }
    if nominal_threshold > 0 {
        let distinct = distinct_values(col, rows);
        if (distinct.len() as i64) <= nominal_threshold {
            return AttrType::Nominal(distinct);
        }
    }
    AttrType::Text
}

fn write_arff(ds: &Dataset, opts: &Options) -> Result<String, String> {
    let mut out = String::new();
    out.push_str("@relation ");
    out.push_str(&quote_arff(&ds.relation));
    out.push_str("\n\n");
    for a in &ds.attributes {
        out.push_str("@attribute ");
        out.push_str(&quote_arff(&a.name));
        out.push(' ');
        out.push_str(&format_attr_type(&a.ty));
        out.push('\n');
    }
    out.push_str("\n@data\n");
    for row in &ds.rows {
        match opts.arff_format {
            ArffFormat::Dense => {
                let cells: Vec<String> = row
                    .iter()
                    .map(|v| match v {
                        None => "?".to_string(),
                        Some(v) => quote_arff(v),
                    })
                    .collect();
                out.push_str(&cells.join(","));
            }
            ArffFormat::Sparse => {
                let mut parts: Vec<String> = Vec::new();
                for (i, v) in row.iter().enumerate() {
                    match v {
                        None => parts.push(format!("{i} ?")),
                        Some(v) => {
                            let ty = &ds.attributes[i].ty;
                            if !is_sparse_zero(v, ty) {
                                parts.push(format!("{i} {}", quote_arff(v)));
                            }
                        }
                    }
                }
                out.push('{');
                out.push_str(&parts.join(","));
                out.push('}');
            }
        }
        out.push('\n');
    }
    while out.ends_with('\n') {
        out.pop();
    }
    Ok(out)
}

fn is_sparse_zero(v: &str, ty: &AttrType) -> bool {
    match ty {
        AttrType::Numeric | AttrType::Date(_) => v.trim().parse::<f64>() == Ok(0.0),
        AttrType::Text => v.is_empty(),
        AttrType::Nominal(labels) => labels.first().map(|l| l == v).unwrap_or(false),
    }
}

// ---------------------------------------------------------------------------
// tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const IRIS: &str = "% a tiny sample\n\
@relation iris\n\
\n\
@attribute sepallength numeric\n\
@attribute petalwidth numeric\n\
@attribute class {Iris-setosa,Iris-versicolor}\n\
\n\
@data\n\
5.1,0.2,Iris-setosa\n\
7.0,1.4,Iris-versicolor\n\
4.9,?,Iris-setosa\n";

    fn opts() -> Options {
        Options::default()
    }

    #[test]
    fn arff_to_csv_happy_path() {
        let out = convert(IRIS, &opts()).unwrap();
        assert_eq!(
            out,
            "sepallength,petalwidth,class\n\
5.1,0.2,Iris-setosa\n\
7.0,1.4,Iris-versicolor\n\
4.9,,Iris-setosa"
        );
    }

    #[test]
    fn missing_value_token_is_configurable() {
        let o = Options {
            missing_value: "NA".into(),
            ..opts()
        };
        let out = convert(IRIS, &o).unwrap();
        assert!(out.ends_with("4.9,NA,Iris-setosa"), "{out}");
    }

    #[test]
    fn type_row_carries_attribute_types_into_csv() {
        let o = Options {
            type_row: true,
            ..opts()
        };
        let out = convert(IRIS, &o).unwrap();
        let second = out.lines().nth(1).unwrap();
        assert_eq!(second, "numeric,numeric,\"{Iris-setosa,Iris-versicolor}\"");
    }

    #[test]
    fn round_trip_through_a_type_row_preserves_types() {
        let o = Options {
            type_row: true,
            relation: "iris".into(),
            ..opts()
        };
        let csv = convert(IRIS, &o).unwrap();
        let back = convert(&csv, &o).unwrap();
        let a = parse_arff(IRIS, &opts()).unwrap();
        let b = parse_arff(&back, &opts()).unwrap();
        assert_eq!(a.attributes, b.attributes);
        assert_eq!(a.rows, b.rows);
    }

    #[test]
    fn quoted_values_escapes_and_comments_are_handled() {
        let src = "@relation q\n\
@attribute note string\n\
@attribute n numeric\n\
@data\n\
'a, b',1 % trailing comment\n\
'line\\nbreak',2\n";
        let ds = parse_arff(src, &opts()).unwrap();
        assert_eq!(ds.rows[0][0], Some("a, b".to_string()));
        assert_eq!(ds.rows[1][0], Some("line\nbreak".to_string()));
        let out = convert(src, &opts()).unwrap();
        assert_eq!(out, "note,n\n\"a, b\",1\n\"line\nbreak\",2");
    }

    #[test]
    fn sparse_rows_expand_with_type_aware_defaults() {
        let src = "@relation s\n\
@attribute a numeric\n\
@attribute b {yes,no}\n\
@attribute c string\n\
@data\n\
{0 5,2 hi}\n\
{1 no}\n";
        let out = convert(src, &opts()).unwrap();
        assert_eq!(out, "a,b,c\n5,yes,hi\n0,no,");
    }

    #[test]
    fn instance_weights_are_stripped() {
        let src = "@relation w\n@attribute a numeric\n@attribute b numeric\n@data\n1,2,{2.0}\n";
        let out = convert(src, &opts()).unwrap();
        assert_eq!(out, "a,b\n1,2");
    }

    #[test]
    fn date_attribute_keeps_its_format() {
        let src = "@relation d\n@attribute when date \"yyyy-MM-dd\"\n@data\n2026-08-08\n";
        let ds = parse_arff(src, &opts()).unwrap();
        assert_eq!(ds.attributes[0].ty, AttrType::Date("yyyy-MM-dd".into()));
    }

    #[test]
    fn csv_to_arff_infers_numeric_nominal_and_string() {
        let csv = "id,grade,note\n\
1,A,short text one\n\
2,B,another distinct note\n\
3,A,a third distinct note\n";
        let o = Options {
            nominal_threshold: 2,
            relation: "grades".into(),
            ..opts()
        };
        let out = convert(csv, &o).unwrap();
        assert_eq!(
            out,
            "@relation grades\n\n\
@attribute id numeric\n\
@attribute grade {A,B}\n\
@attribute note string\n\
\n@data\n\
1,A,'short text one'\n\
2,B,'another distinct note'\n\
3,A,'a third distinct note'"
        );
    }

    #[test]
    fn column_types_override_by_name_and_index() {
        let csv = "id,grade\n1,A\n2,B\n";
        let o = Options {
            column_types: "id:nominal,2:string".into(),
            ..opts()
        };
        let out = convert(csv, &o).unwrap();
        assert!(out.contains("@attribute id {1,2}"), "{out}");
        assert!(out.contains("@attribute grade string"), "{out}");
    }

    #[test]
    fn date_columns_come_from_an_explicit_override() {
        let csv = "when,v\n2026-08-08,1\n";
        let o = Options {
            column_types: "when:date".into(),
            date_format: "yyyy-MM-dd".into(),
            ..opts()
        };
        let out = convert(csv, &o).unwrap();
        assert!(out.contains("@attribute when date yyyy-MM-dd"), "{out}");
    }

    #[test]
    fn sparse_output_omits_zeros_and_first_labels() {
        let csv = "a,b,c\n0,yes,\n5,no,x\n";
        let o = Options {
            arff_format: ArffFormat::Sparse,
            ..opts()
        };
        let out = convert(csv, &o).unwrap();
        let data: Vec<&str> = out.lines().skip_while(|l| *l != "@data").skip(1).collect();
        // numeric 0 and the first nominal label are omitted; missing values stay explicit.
        assert_eq!(data, vec!["{2 ?}", "{0 5,1 no}"]);
    }

    #[test]
    fn headerless_csv_gets_generated_attribute_names() {
        let o = Options {
            header: false,
            ..opts()
        };
        let out = convert("1,2\n3,4\n", &o).unwrap();
        assert!(out.contains("@attribute att1 numeric"), "{out}");
        assert!(out.contains("@attribute att2 numeric"), "{out}");
    }

    #[test]
    fn tab_delimiter_is_accepted_by_name() {
        let o = Options {
            delimiter: "tab".into(),
            ..opts()
        };
        let out = convert(IRIS, &o).unwrap();
        assert_eq!(
            out.lines().next().unwrap(),
            "sepallength\tpetalwidth\tclass"
        );
    }

    #[test]
    fn auto_direction_detects_both_inputs() {
        assert_eq!(detect_direction(IRIS), Direction::ArffToCsv);
        assert_eq!(detect_direction("a,b\n1,2\n"), Direction::CsvToArff);
        assert_eq!(
            detect_direction("% only a comment\n@DATA\n1\n"),
            Direction::ArffToCsv
        );
    }

    #[test]
    fn forcing_a_direction_overrides_detection() {
        let o = Options {
            direction: Direction::CsvToArff,
            ..opts()
        };
        // Treated as CSV: the '@relation iris' line becomes a one-column header.
        let out = convert("@relation iris\n1\n2\n", &o).unwrap();
        assert!(out.contains("@attribute '@relation iris' numeric"), "{out}");
    }

    #[test]
    fn error_row_width_mismatch() {
        let src = "@relation r\n@attribute a numeric\n@attribute b numeric\n@data\n1,2,3\n";
        let err = convert(src, &opts()).unwrap_err();
        assert!(
            err.contains("row has 3 values but the header declares 2 attributes"),
            "{err}"
        );
    }

    #[test]
    fn error_relational_attributes_are_rejected() {
        let src = "@relation r\n@attribute bag relational\n";
        let err = convert(src, &opts()).unwrap_err();
        assert!(err.contains("relational"), "{err}");
    }

    #[test]
    fn error_missing_data_section() {
        let src = "@relation r\n@attribute a numeric\n";
        let err = convert(src, &opts()).unwrap_err();
        assert!(err.contains("no @data section"), "{err}");
    }

    #[test]
    fn error_empty_input() {
        let err = convert("   \n", &opts()).unwrap_err();
        assert!(err.contains("input is empty"), "{err}");
    }

    #[test]
    fn error_unknown_attribute_type() {
        let src = "@relation r\n@attribute a widget\n@data\n1\n";
        let err = convert(src, &opts()).unwrap_err();
        assert!(err.contains("unknown attribute type 'widget'"), "{err}");
    }

    #[test]
    fn error_unknown_column_types_column() {
        let o = Options {
            column_types: "nope:nominal".into(),
            ..opts()
        };
        let err = convert("a,b\n1,2\n", &o).unwrap_err();
        assert!(err.contains("unknown column 'nope'"), "{err}");
    }

    #[test]
    fn error_bad_delimiter() {
        let o = Options {
            delimiter: "commas".into(),
            ..opts()
        };
        let err = convert(IRIS, &o).unwrap_err();
        assert!(err.contains("unknown delimiter"), "{err}");
    }

    #[test]
    fn error_input_over_the_size_cap() {
        let big = "a,b\n".to_string() + &"1,2\n".repeat(MAX_INPUT_CHARS / 4);
        let err = convert(&big, &opts()).unwrap_err();
        assert!(err.contains("character limit"), "{err}");
    }
}
