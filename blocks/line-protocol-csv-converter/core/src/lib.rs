//! line-protocol-csv-converter core — converts between InfluxDB line protocol
//! and CSV, in both directions, with no I/O and no host calls. Shared by the
//! chat skill block, the `gizza` CLI and the web page.
//!
//! ## Line protocol → CSV
//!
//! Each line is parsed per the InfluxDB line protocol grammar
//! (`<measurement>[,<tag>=<v>…] <field>=<v>[,…] [<timestamp>]`), honouring
//! backslash escapes and double-quoted string field values, then rendered as
//! either a `wide` table (one row per point, one column per distinct tag/field
//! key) or a `long` table (one row per field value). Optionally prefixed with an
//! InfluxDB `#datatype` annotation row so the CSV can be written straight back
//! with `influx write --format csv`.
//!
//! ## CSV → line protocol
//!
//! Column roles come from, in order of precedence: `#datatype`/`#constant`
//! annotation rows, the `name|datatype|default` inline header syntax, the
//! explicit `measurement`/`tag_columns`/`field_columns`/`time_column`
//! parameters, and finally name-based auto-detection. Values are typed and
//! escaped per the line protocol rules (`1i`, `1u`, `"quoted"`, bare booleans).
//!
//! Everything is deterministic: one pass in input order, no clock, no state
//! between runs, so every surface produces byte-identical output for the same
//! input.

use std::collections::HashMap;

use chrono::{DateTime, NaiveDate, NaiveDateTime, SecondsFormat, Utc};

/// Largest input accepted, in Unicode characters.
pub const MAX_CHARS: usize = 2_000_000;
/// Largest number of input lines accepted.
pub const MAX_LINES: usize = 200_000;
/// Largest number of distinct output columns accepted.
pub const MAX_COLUMNS: usize = 1_000;

/// Line protocol field types, in line protocol spelling.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FieldType {
    Float,
    Integer,
    Unsigned,
    Boolean,
    Str,
}

impl FieldType {
    /// The InfluxDB annotated-CSV `#datatype` name for this field type.
    fn datatype(self) -> &'static str {
        match self {
            FieldType::Float => "double",
            FieldType::Integer => "long",
            FieldType::Unsigned => "unsignedLong",
            FieldType::Boolean => "boolean",
            FieldType::Str => "string",
        }
    }

    /// Widening join used when one field key carries several types across points.
    fn merge(self, other: FieldType) -> FieldType {
        if self == other {
            return self;
        }
        match (self, other) {
            // Any two numeric types widen to a float; anything else falls back
            // to a string, which always round-trips.
            (FieldType::Float | FieldType::Integer | FieldType::Unsigned, FieldType::Float)
            | (FieldType::Float, FieldType::Integer | FieldType::Unsigned)
            | (FieldType::Integer, FieldType::Unsigned)
            | (FieldType::Unsigned, FieldType::Integer) => FieldType::Float,
            _ => FieldType::Str,
        }
    }
}

/// One parsed field value: its line protocol type plus the plain (unquoted,
/// unescaped, suffix-free) text a CSV cell should contain.
#[derive(Clone, Debug)]
pub struct FieldValue {
    pub ty: FieldType,
    pub text: String,
}

/// One parsed line protocol point.
#[derive(Clone, Debug)]
pub struct Point {
    pub measurement: String,
    pub tags: Vec<(String, String)>,
    pub fields: Vec<(String, FieldValue)>,
    /// Timestamp in nanoseconds since the Unix epoch, if the point carried one.
    pub timestamp: Option<i128>,
}

// ---------------------------------------------------------------------------
// Option plumbing
// ---------------------------------------------------------------------------

fn resolve_delimiter(name: &str) -> Result<u8, String> {
    match name.trim() {
        "" | "comma" | "," => Ok(b','),
        "semicolon" | ";" => Ok(b';'),
        "tab" | "\t" | "\\t" => Ok(b'\t'),
        "pipe" | "|" => Ok(b'|'),
        other => Err(format!(
            "unknown delimiter {other:?} -- use comma, semicolon, tab or pipe"
        )),
    }
}

/// Nanoseconds per unit for the `precision` option.
fn precision_scale(precision: &str) -> Result<i128, String> {
    match precision.trim() {
        "" | "ns" => Ok(1),
        "us" => Ok(1_000),
        "ms" => Ok(1_000_000),
        "s" => Ok(1_000_000_000),
        other => Err(format!(
            "unknown precision {other:?} -- use ns, us, ms or s"
        )),
    }
}

fn check_size(data: &str) -> Result<(), String> {
    let chars = data.chars().count();
    if chars > MAX_CHARS {
        return Err(format!(
            "input is {chars} characters, over the {MAX_CHARS} character limit"
        ));
    }
    let lines = data.lines().count();
    if lines > MAX_LINES {
        return Err(format!(
            "input has {lines} lines, over the {MAX_LINES} line limit"
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Line protocol parsing
// ---------------------------------------------------------------------------

/// True when `line` is a line protocol comment or is blank.
fn is_lp_skippable(line: &str) -> bool {
    let t = line.trim();
    t.is_empty() || t.starts_with('#')
}

/// Split a line protocol line into (measurement+tag set, field set, timestamp).
///
/// Splits on the first two spaces that are neither backslash-escaped nor inside
/// a double-quoted string field value.
fn split_lp_line(line: &str) -> Result<(&str, &str, &str), String> {
    let mut esc = false;
    let mut in_quote = false;
    let mut first: Option<usize> = None;
    let mut second: Option<usize> = None;

    for (i, c) in line.char_indices() {
        if esc {
            esc = false;
            continue;
        }
        match c {
            '\\' => esc = true,
            // Quotes only delimit string values, which live in the field set.
            '"' if first.is_some() => in_quote = !in_quote,
            ' ' if !in_quote => {
                if first.is_none() {
                    first = Some(i);
                } else {
                    second = Some(i);
                    break;
                }
            }
            _ => {}
        }
    }
    if in_quote {
        return Err("unterminated double-quoted string field value".to_string());
    }
    let first = first.ok_or_else(|| {
        "expected a space between the tag set and the field set (e.g. `cpu,host=a load=1`)"
            .to_string()
    })?;
    let head = &line[..first];
    match second {
        Some(sec) => Ok((head, &line[first + 1..sec], line[sec + 1..].trim())),
        None => Ok((head, &line[first + 1..], "")),
    }
}

/// Split on `sep` where it is neither escaped nor inside a quoted string.
fn split_unescaped(s: &str, sep: char, respect_quotes: bool) -> Vec<&str> {
    let mut out = Vec::new();
    let mut esc = false;
    let mut in_quote = false;
    let mut start = 0usize;
    for (i, c) in s.char_indices() {
        if esc {
            esc = false;
            continue;
        }
        match c {
            '\\' => esc = true,
            '"' if respect_quotes => in_quote = !in_quote,
            c if c == sep && !in_quote => {
                out.push(&s[start..i]);
                start = i + c.len_utf8();
            }
            _ => {}
        }
    }
    out.push(&s[start..]);
    out
}

/// Index of the first `=` that is neither escaped nor inside a quoted string.
fn find_unescaped_eq(s: &str) -> Option<usize> {
    let mut esc = false;
    let mut in_quote = false;
    for (i, c) in s.char_indices() {
        if esc {
            esc = false;
            continue;
        }
        match c {
            '\\' => esc = true,
            '"' => in_quote = !in_quote,
            '=' if !in_quote => return Some(i),
            _ => {}
        }
    }
    None
}

/// Undo line protocol backslash escapes. `\X` for the escapable characters
/// becomes `X`; any other backslash is kept verbatim (the spec leaves it alone).
fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some(n @ (',' | '=' | ' ' | '"' | '\\')) => out.push(n),
            Some(n) => {
                out.push('\\');
                out.push(n);
            }
            None => out.push('\\'),
        }
    }
    out
}

/// Type + plain text for one raw line protocol field value.
fn parse_field_value(raw: &str) -> Result<FieldValue, String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err("empty field value".to_string());
    }
    if let Some(rest) = raw.strip_prefix('"') {
        let inner = rest
            .strip_suffix('"')
            .ok_or_else(|| "unterminated double-quoted string field value".to_string())?;
        return Ok(FieldValue {
            ty: FieldType::Str,
            text: unescape(inner),
        });
    }
    match raw {
        "t" | "T" | "true" | "True" | "TRUE" => {
            return Ok(FieldValue {
                ty: FieldType::Boolean,
                text: "true".to_string(),
            })
        }
        "f" | "F" | "false" | "False" | "FALSE" => {
            return Ok(FieldValue {
                ty: FieldType::Boolean,
                text: "false".to_string(),
            })
        }
        _ => {}
    }
    if let Some(num) = raw.strip_suffix('i') {
        return num
            .parse::<i64>()
            .map(|n| FieldValue {
                ty: FieldType::Integer,
                text: n.to_string(),
            })
            .map_err(|_| format!("{raw:?} is not a valid integer field value"));
    }
    if let Some(num) = raw.strip_suffix('u') {
        return num
            .parse::<u64>()
            .map(|n| FieldValue {
                ty: FieldType::Unsigned,
                text: n.to_string(),
            })
            .map_err(|_| format!("{raw:?} is not a valid unsigned integer field value"));
    }
    match raw.parse::<f64>() {
        Ok(n) if n.is_finite() => Ok(FieldValue {
            ty: FieldType::Float,
            text: raw.to_string(),
        }),
        _ => Err(format!(
            "{raw:?} is not a valid field value -- quote strings (\"text\"), suffix integers with i, unsigned with u"
        )),
    }
}

/// Parse one line protocol line into a [`Point`].
pub fn parse_lp_line(line: &str, scale: i128) -> Result<Point, String> {
    let (head, field_set, ts) = split_lp_line(line)?;
    let mut head_parts = split_unescaped(head, ',', false).into_iter();
    let measurement = unescape(head_parts.next().unwrap_or(""));
    if measurement.is_empty() {
        return Err("missing measurement name".to_string());
    }

    let mut tags = Vec::new();
    for part in head_parts {
        if part.trim().is_empty() {
            continue;
        }
        let eq = find_unescaped_eq(part)
            .ok_or_else(|| format!("tag {part:?} is missing an `=` (expected key=value)"))?;
        let key = unescape(&part[..eq]);
        let value = unescape(&part[eq + 1..]);
        if key.is_empty() {
            return Err(format!("tag {part:?} has an empty key"));
        }
        tags.push((key, value));
    }

    if field_set.trim().is_empty() {
        return Err("missing field set -- every point needs at least one field".to_string());
    }
    let mut fields = Vec::new();
    for part in split_unescaped(field_set, ',', true) {
        if part.trim().is_empty() {
            continue;
        }
        let eq = find_unescaped_eq(part)
            .ok_or_else(|| format!("field {part:?} is missing an `=` (expected key=value)"))?;
        let key = unescape(&part[..eq]);
        if key.is_empty() {
            return Err(format!("field {part:?} has an empty key"));
        }
        let value =
            parse_field_value(&part[eq + 1..]).map_err(|e| format!("field {key:?}: {e}"))?;
        fields.push((key, value));
    }
    if fields.is_empty() {
        return Err("missing field set -- every point needs at least one field".to_string());
    }

    let timestamp = if ts.is_empty() {
        None
    } else {
        let n = ts
            .parse::<i64>()
            .map_err(|_| format!("{ts:?} is not a valid integer timestamp"))?;
        Some(n as i128 * scale)
    };

    Ok(Point {
        measurement,
        tags,
        fields,
        timestamp,
    })
}

// ---------------------------------------------------------------------------
// Line protocol escaping (CSV → line protocol)
// ---------------------------------------------------------------------------

fn escape_with(s: &str, specials: &[char]) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if specials.contains(&c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Measurement names escape commas and spaces.
fn escape_measurement(s: &str) -> String {
    escape_with(s, &[',', ' '])
}

/// Tag keys, tag values and field keys escape commas, equals signs and spaces.
fn escape_key(s: &str) -> String {
    escape_with(s, &[',', '=', ' '])
}

/// String field values escape double quotes and backslashes.
fn escape_string_value(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if c == '"' || c == '\\' {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

// ---------------------------------------------------------------------------
// Timestamps
// ---------------------------------------------------------------------------

fn ns_to_rfc3339(ns: i128) -> Result<String, String> {
    let secs = ns.div_euclid(1_000_000_000);
    let sub = ns.rem_euclid(1_000_000_000) as u32;
    let secs = i64::try_from(secs).map_err(|_| format!("timestamp {ns} is out of range"))?;
    DateTime::<Utc>::from_timestamp(secs, sub)
        .map(|dt| dt.to_rfc3339_opts(SecondsFormat::AutoSi, true))
        .ok_or_else(|| format!("timestamp {ns} is out of range"))
}

/// Parse a CSV timestamp cell to nanoseconds. Accepts RFC3339, a few common
/// naive forms (assumed UTC), and a bare integer in `precision` units.
fn parse_timestamp_cell(raw: &str, scale: i128) -> Result<i128, String> {
    let s = raw.trim();
    if let Ok(n) = s.parse::<i64>() {
        return Ok(n as i128 * scale);
    }
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return dt
            .timestamp_nanos_opt()
            .map(|n| n as i128)
            .ok_or_else(|| format!("timestamp {s:?} is out of the nanosecond range"));
    }
    for fmt in ["%Y-%m-%dT%H:%M:%S%.f", "%Y-%m-%d %H:%M:%S%.f"] {
        if let Ok(dt) = NaiveDateTime::parse_from_str(s, fmt) {
            return dt
                .and_utc()
                .timestamp_nanos_opt()
                .map(|n| n as i128)
                .ok_or_else(|| format!("timestamp {s:?} is out of the nanosecond range"));
        }
    }
    if let Ok(d) = NaiveDate::parse_from_str(s, "%Y-%m-%d") {
        return d
            .and_hms_opt(0, 0, 0)
            .and_then(|dt| dt.and_utc().timestamp_nanos_opt())
            .map(|n| n as i128)
            .ok_or_else(|| format!("timestamp {s:?} is out of the nanosecond range"));
    }
    Err(format!(
        "{s:?} is not a recognised timestamp -- use RFC3339 (2020-01-01T00:00:00Z) or a Unix integer in the selected precision"
    ))
}

// ---------------------------------------------------------------------------
// Direction detection
// ---------------------------------------------------------------------------

/// Annotation / directive prefixes that only ever appear in annotated CSV.
const CSV_DIRECTIVES: [&str; 5] = ["#datatype", "#constant", "#default", "#group", "#timezone"];

/// Decide whether `data` is line protocol or CSV.
pub fn detect_direction(data: &str, scale: i128) -> &'static str {
    for line in data.lines() {
        let t = line.trim();
        if t.is_empty() {
            continue;
        }
        if t.starts_with("sep=") && t.len() == 5 {
            return "csv-to-lp";
        }
        if CSV_DIRECTIVES.iter().any(|d| t.starts_with(d)) {
            return "csv-to-lp";
        }
        if t.starts_with('#') {
            // A line protocol comment -- keep looking for real content.
            continue;
        }
        return if parse_lp_line(t, scale).is_ok() {
            "lp-to-csv"
        } else {
            "csv-to-lp"
        };
    }
    "csv-to-lp"
}

// ---------------------------------------------------------------------------
// Line protocol → CSV
// ---------------------------------------------------------------------------

/// Ordered key registry: keeps first-appearance order, optionally sorted later.
#[derive(Default)]
struct KeySet {
    order: Vec<String>,
    seen: HashMap<String, usize>,
}

impl KeySet {
    fn insert(&mut self, key: &str) -> usize {
        if let Some(&i) = self.seen.get(key) {
            return i;
        }
        let i = self.order.len();
        self.order.push(key.to_string());
        self.seen.insert(key.to_string(), i);
        i
    }
}

fn render_timestamp(ns: i128, timestamp_format: &str) -> Result<String, String> {
    match timestamp_format.trim() {
        "" | "rfc3339" => ns_to_rfc3339(ns),
        "unix_ns" => Ok(ns.to_string()),
        "unix_us" => Ok(ns.div_euclid(1_000).to_string()),
        "unix_ms" => Ok(ns.div_euclid(1_000_000).to_string()),
        "unix_s" => Ok(ns.div_euclid(1_000_000_000).to_string()),
        other => Err(format!(
            "unknown timestamp_format {other:?} -- use rfc3339, unix_ns, unix_us, unix_ms or unix_s"
        )),
    }
}

fn timestamp_datatype(timestamp_format: &str) -> &'static str {
    match timestamp_format.trim() {
        "" | "rfc3339" => "dateTime:RFC3339",
        _ => "dateTime:number",
    }
}

#[allow(clippy::too_many_arguments)]
fn lp_to_csv(
    data: &str,
    csv_layout: &str,
    delimiter: u8,
    timestamp_format: &str,
    scale: i128,
    emit_annotations: bool,
    sort_keys: bool,
    skip_bad: bool,
) -> Result<String, String> {
    let wide = match csv_layout.trim() {
        "" | "wide" => true,
        "long" => false,
        other => return Err(format!("unknown csv_layout {other:?} -- use wide or long")),
    };
    if emit_annotations && !wide {
        return Err("emit_annotations needs csv_layout=wide -- annotated CSV maps one column per field, but the long layout keeps field names in a column".to_string());
    }
    // Validate the format up front so an empty input still reports a bad option.
    let _ = render_timestamp(0, timestamp_format)?;

    let mut points: Vec<Point> = Vec::new();
    let mut tag_keys = KeySet::default();
    let mut field_keys = KeySet::default();
    let mut field_types: HashMap<String, FieldType> = HashMap::new();

    for (i, line) in data.lines().enumerate() {
        if is_lp_skippable(line) {
            continue;
        }
        match parse_lp_line(line.trim_end(), scale) {
            Ok(p) => {
                for (k, _) in &p.tags {
                    tag_keys.insert(k);
                }
                for (k, v) in &p.fields {
                    field_keys.insert(k);
                    field_types
                        .entry(k.clone())
                        .and_modify(|t| *t = t.merge(v.ty))
                        .or_insert(v.ty);
                }
                points.push(p);
            }
            Err(e) if skip_bad => {
                let _ = (i, e);
                continue;
            }
            Err(e) => return Err(format!("line {}: {e}", i + 1)),
        }
    }
    if points.is_empty() {
        return Err("no line protocol points found -- expected lines like `cpu,host=a load=0.9 1577836800000000000`".to_string());
    }

    let mut tags = tag_keys.order.clone();
    let mut fields = field_keys.order.clone();
    if sort_keys {
        tags.sort();
        fields.sort();
    }

    let column_count = if wide {
        2 + tags.len() + fields.len()
    } else {
        4 + tags.len()
    };
    if column_count > MAX_COLUMNS {
        return Err(format!(
            "the input needs {column_count} CSV columns, over the {MAX_COLUMNS} column limit -- use csv_layout=long"
        ));
    }

    let mut wtr = csv::WriterBuilder::new()
        .delimiter(delimiter)
        .terminator(csv::Terminator::Any(b'\n'))
        .from_writer(Vec::new());

    if emit_annotations {
        let mut row = vec!["#datatype measurement".to_string()];
        row.extend(tags.iter().map(|_| "tag".to_string()));
        row.extend(fields.iter().map(|f| field_types[f].datatype().to_string()));
        row.push(timestamp_datatype(timestamp_format).to_string());
        wtr.write_record(&row).map_err(|e| e.to_string())?;
    }

    let mut header = vec!["measurement".to_string()];
    header.extend(tags.iter().cloned());
    if wide {
        header.extend(fields.iter().cloned());
    } else {
        header.push("field".to_string());
        header.push("value".to_string());
    }
    header.push("time".to_string());
    wtr.write_record(&header).map_err(|e| e.to_string())?;

    for p in &points {
        let tag_map: HashMap<&str, &str> = p
            .tags
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        let time_cell = match p.timestamp {
            Some(ns) => render_timestamp(ns, timestamp_format)?,
            None => String::new(),
        };
        if wide {
            let field_map: HashMap<&str, &str> = p
                .fields
                .iter()
                .map(|(k, v)| (k.as_str(), v.text.as_str()))
                .collect();
            let mut row = vec![p.measurement.clone()];
            row.extend(
                tags.iter()
                    .map(|t| tag_map.get(t.as_str()).unwrap_or(&"").to_string()),
            );
            row.extend(
                fields
                    .iter()
                    .map(|f| field_map.get(f.as_str()).unwrap_or(&"").to_string()),
            );
            row.push(time_cell);
            wtr.write_record(&row).map_err(|e| e.to_string())?;
        } else {
            for (k, v) in &p.fields {
                let mut row = vec![p.measurement.clone()];
                row.extend(
                    tags.iter()
                        .map(|t| tag_map.get(t.as_str()).unwrap_or(&"").to_string()),
                );
                row.push(k.clone());
                row.push(v.text.clone());
                row.push(time_cell.clone());
                wtr.write_record(&row).map_err(|e| e.to_string())?;
            }
        }
    }

    let bytes = wtr.into_inner().map_err(|e| e.to_string())?;
    String::from_utf8(bytes)
        .map(|s| s.trim_end_matches('\n').to_string())
        .map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// CSV → line protocol
// ---------------------------------------------------------------------------

/// What one CSV column contributes to each point.
#[derive(Clone, PartialEq, Debug)]
enum Role {
    Measurement,
    Tag,
    Field(Option<FieldType>),
    Time,
    Ignore,
}

/// Map an InfluxDB annotated-CSV datatype name to a column role.
fn role_from_datatype(dt: &str) -> Result<Role, String> {
    let base = dt.split(':').next().unwrap_or("").trim();
    match base {
        "" | "ignore" | "ignored" => Ok(Role::Ignore),
        "measurement" => Ok(Role::Measurement),
        "tag" => Ok(Role::Tag),
        "dateTime" | "time" => Ok(Role::Time),
        "double" => Ok(Role::Field(Some(FieldType::Float))),
        "long" => Ok(Role::Field(Some(FieldType::Integer))),
        "unsignedLong" => Ok(Role::Field(Some(FieldType::Unsigned))),
        "boolean" => Ok(Role::Field(Some(FieldType::Boolean))),
        // `duration` is accepted as an opaque string: durations have no line
        // protocol type of their own.
        "string" | "duration" => Ok(Role::Field(Some(FieldType::Str))),
        "field" => Ok(Role::Field(None)),
        other => Err(format!(
            "unknown datatype {other:?} -- use measurement, tag, field, string, double, long, unsignedLong, boolean, dateTime or ignored"
        )),
    }
}

fn split_list(s: &str) -> Vec<String> {
    s.split(',')
        .map(|p| p.trim().to_string())
        .filter(|p| !p.is_empty())
        .collect()
}

/// Format one typed CSV cell as a line protocol field value.
fn format_field_value(
    raw: &str,
    ty: Option<FieldType>,
    number_type: &str,
) -> Result<String, String> {
    let s = raw.trim();
    match ty {
        Some(FieldType::Str) => Ok(format!("\"{}\"", escape_string_value(raw))),
        Some(FieldType::Boolean) => parse_bool_cell(s).map(|b| b.to_string()),
        Some(FieldType::Integer) => s
            .parse::<i64>()
            .map(|n| format!("{n}i"))
            .map_err(|_| format!("{s:?} is not a valid long")),
        Some(FieldType::Unsigned) => s
            .parse::<u64>()
            .map(|n| format!("{n}u"))
            .map_err(|_| format!("{s:?} is not a valid unsignedLong")),
        Some(FieldType::Float) => match s.parse::<f64>() {
            Ok(n) if n.is_finite() => Ok(s.to_string()),
            _ => Err(format!("{s:?} is not a valid double")),
        },
        None => {
            // Untyped: infer per value, the same way `influx write` does for a
            // bare `field` column.
            if let Ok(b) = parse_bool_cell(s) {
                return Ok(b.to_string());
            }
            let fractional = s.contains('.') || s.contains('e') || s.contains('E');
            if let Ok(n) = s.parse::<f64>() {
                if n.is_finite() {
                    if !fractional && number_type.trim() == "integer" {
                        if let Ok(i) = s.parse::<i64>() {
                            return Ok(format!("{i}i"));
                        }
                    }
                    return Ok(s.to_string());
                }
            }
            Ok(format!("\"{}\"", escape_string_value(raw)))
        }
    }
}

fn parse_bool_cell(s: &str) -> Result<bool, String> {
    match s.trim() {
        "t" | "T" | "true" | "True" | "TRUE" | "y" | "Y" | "yes" | "1" => Ok(true),
        "f" | "F" | "false" | "False" | "FALSE" | "n" | "N" | "no" | "0" => Ok(false),
        other => Err(format!("{other:?} is not a valid boolean")),
    }
}

/// Auto-detected roles for unannotated CSV, by column name.
fn auto_role(name: &str) -> Option<Role> {
    match name.trim().to_ascii_lowercase().as_str() {
        "measurement" | "_measurement" => Some(Role::Measurement),
        "time" | "_time" | "timestamp" | "date" | "datetime" => Some(Role::Time),
        _ => None,
    }
}

struct Constants {
    measurement: Option<String>,
    tags: Vec<(String, String)>,
    fields: Vec<(String, String)>,
    time: Option<String>,
}

#[allow(clippy::too_many_arguments)]
fn csv_to_lp(
    data: &str,
    delimiter: u8,
    scale: i128,
    measurement_opt: &str,
    tag_columns: &str,
    field_columns: &str,
    time_column: &str,
    number_type: &str,
    sort_keys: bool,
    skip_bad: bool,
) -> Result<String, String> {
    if !matches!(number_type.trim(), "" | "float" | "integer") {
        return Err(format!(
            "unknown number_type {:?} -- use float or integer",
            number_type.trim()
        ));
    }

    // `sep=;` on the very first line overrides the delimiter (csv2lp compatible).
    let mut body = data;
    let mut delimiter = delimiter;
    if let Some(first) = data.lines().next() {
        let t = first.trim();
        if let Some(sep) = t.strip_prefix("sep=") {
            if sep.chars().count() == 1 {
                delimiter = sep.as_bytes()[0];
                body = &data[first.len().min(data.len())..];
                body = body.strip_prefix('\n').unwrap_or(body);
                body = body.strip_prefix('\r').unwrap_or(body);
                body = body.strip_prefix('\n').unwrap_or(body);
            }
        }
    }

    let mut rdr = csv::ReaderBuilder::new()
        .delimiter(delimiter)
        .flexible(true)
        .has_headers(false)
        .from_reader(body.as_bytes());

    let mut records: Vec<(u64, Vec<String>)> = Vec::new();
    for result in rdr.records() {
        let rec = result.map_err(|e| format!("CSV parse error: {e}"))?;
        let line = rec.position().map(|p| p.line()).unwrap_or(0);
        let cells: Vec<String> = rec.iter().map(|c| c.to_string()).collect();
        if cells.iter().all(|c| c.trim().is_empty()) {
            continue;
        }
        records.push((line, cells));
    }
    if records.is_empty() {
        return Err("no CSV rows found".to_string());
    }

    // 1. Annotation rows (they must precede the header row).
    let mut datatypes: Vec<String> = Vec::new();
    let mut defaults: Vec<String> = Vec::new();
    let mut constants = Constants {
        measurement: None,
        tags: Vec::new(),
        fields: Vec::new(),
        time: None,
    };
    let mut idx = 0usize;
    while idx < records.len() {
        let (line, cells) = &records[idx];
        let head = cells[0].trim();
        if !head.starts_with('#') {
            break;
        }
        // The first cell may carry the directive alone (`#datatype`) or the
        // directive plus column 0's value (`#datatype measurement`).
        let (directive, inline) = match head.find(char::is_whitespace) {
            Some(p) => (&head[..p], head[p..].trim().to_string()),
            None => (head, String::new()),
        };
        match directive {
            "#datatype" => {
                datatypes = std::iter::once(inline)
                    .chain(cells[1..].iter().map(|c| c.trim().to_string()))
                    .collect();
            }
            "#default" => {
                defaults = std::iter::once(inline)
                    .chain(cells[1..].iter().cloned())
                    .collect();
            }
            "#constant" => {
                let mut toks: Vec<String> = Vec::new();
                if !inline.is_empty() {
                    toks.push(inline);
                }
                toks.extend(cells[1..].iter().map(|c| c.trim().to_string()));
                let (dt, name, value) = match toks.len() {
                    2 => (toks[0].clone(), String::new(), toks[1].clone()),
                    3 => (toks[0].clone(), toks[1].clone(), toks[2].clone()),
                    _ => {
                        return Err(format!(
                            "line {line}: #constant needs `#constant,<datatype>,<name>,<value>` or `#constant <datatype>,<value>`"
                        ))
                    }
                };
                match role_from_datatype(&dt).map_err(|e| format!("line {line}: {e}"))? {
                    Role::Measurement => constants.measurement = Some(value),
                    Role::Tag => constants.tags.push((name, value)),
                    Role::Time => constants.time = Some(value),
                    Role::Field(_) => constants.fields.push((name, value)),
                    Role::Ignore => {}
                }
            }
            "#group" => {} // grouping is a Flux concept with no line protocol effect
            "#timezone" => {
                return Err(format!(
                    "line {line}: the #timezone annotation is not supported -- convert timestamps to RFC3339 with an offset, or to Unix numbers, first"
                ))
            }
            other => {
                return Err(format!(
                    "line {line}: unknown annotation {other:?} -- supported: #datatype, #default, #constant, #group"
                ))
            }
        }
        idx += 1;
    }

    // 2. Header row.
    let (_, header_cells) = records
        .get(idx)
        .ok_or_else(|| "no header row found after the annotation rows".to_string())?;
    let mut names: Vec<String> = Vec::new();
    let mut inline_types: Vec<String> = Vec::new();
    let mut inline_defaults: Vec<String> = Vec::new();
    for cell in header_cells {
        // csv2lp's inline `name|datatype|default` header syntax.
        let mut parts = cell.split('|');
        names.push(parts.next().unwrap_or("").trim().to_string());
        inline_types.push(parts.next().unwrap_or("").trim().to_string());
        inline_defaults.push(parts.next().unwrap_or("").to_string());
    }
    idx += 1;

    if names.len() > MAX_COLUMNS {
        return Err(format!(
            "the CSV has {} columns, over the {MAX_COLUMNS} column limit",
            names.len()
        ));
    }

    // 3. Column roles: #datatype > inline header type > explicit params > name.
    let tag_list = split_list(tag_columns);
    let field_list = split_list(field_columns);
    let time_col = time_column.trim();
    let measurement_opt = measurement_opt.trim();
    let measurement_is_column =
        !measurement_opt.is_empty() && names.iter().any(|n| n == measurement_opt);

    let mut roles: Vec<Role> = Vec::with_capacity(names.len());
    for (i, name) in names.iter().enumerate() {
        let declared = datatypes
            .get(i)
            .map(|s| s.as_str())
            .filter(|s| !s.is_empty())
            .or_else(|| {
                inline_types
                    .get(i)
                    .map(|s| s.as_str())
                    .filter(|s| !s.is_empty())
            });
        let role = match declared {
            Some(dt) => role_from_datatype(dt)?,
            None if measurement_is_column && name == measurement_opt => Role::Measurement,
            None if !time_col.is_empty() && name == time_col => Role::Time,
            None if tag_list.iter().any(|t| t == name) => Role::Tag,
            None if field_list.iter().any(|f| f == name) => Role::Field(None),
            None if !field_list.is_empty() => Role::Ignore,
            None => auto_role(name).unwrap_or(Role::Field(None)),
        };
        roles.push(role);
    }
    for want in tag_list.iter().chain(field_list.iter()) {
        if !names.iter().any(|n| n == want) {
            return Err(format!(
                "no column named {want:?} -- the CSV header has: {}",
                names.join(", ")
            ));
        }
    }
    if !time_col.is_empty() && !names.iter().any(|n| n == time_col) {
        return Err(format!(
            "no column named {time_col:?} -- the CSV header has: {}",
            names.join(", ")
        ));
    }

    let literal_measurement = if measurement_is_column {
        None
    } else if !measurement_opt.is_empty() {
        Some(measurement_opt.to_string())
    } else {
        constants.measurement.clone()
    };
    if literal_measurement.is_none()
        && constants.measurement.is_none()
        && !roles.contains(&Role::Measurement)
    {
        return Err("no measurement -- set the measurement parameter, name a column `measurement`, add a `#constant measurement,<name>` row, or mark a column `measurement` in #datatype".to_string());
    }
    if !roles.iter().any(|r| matches!(r, Role::Field(_))) && constants.fields.is_empty() {
        return Err("no field columns -- line protocol needs at least one field per point; check field_columns / #datatype".to_string());
    }

    // 4. Rows.
    let mut out = String::new();
    let mut emitted = 0usize;
    let mut first_error: Option<String> = None;
    for (line, cells) in &records[idx..] {
        match build_lp_line(
            cells,
            &names,
            &roles,
            &defaults,
            &inline_defaults,
            &constants,
            literal_measurement.as_deref(),
            number_type,
            scale,
            sort_keys,
        ) {
            Ok(l) => {
                out.push_str(&l);
                out.push('\n');
                emitted += 1;
            }
            Err(e) => {
                if first_error.is_none() {
                    first_error = Some(format!("line {line}: {e}"));
                }
                if !skip_bad {
                    return Err(format!("line {line}: {e}"));
                }
            }
        }
    }
    if emitted == 0 {
        return Err(
            first_error.unwrap_or_else(|| "no data rows found after the header".to_string())
        );
    }
    Ok(out.trim_end_matches('\n').to_string())
}

#[allow(clippy::too_many_arguments)]
fn build_lp_line(
    cells: &[String],
    names: &[String],
    roles: &[Role],
    defaults: &[String],
    inline_defaults: &[String],
    constants: &Constants,
    literal_measurement: Option<&str>,
    number_type: &str,
    scale: i128,
    sort_keys: bool,
) -> Result<String, String> {
    let mut measurement = literal_measurement.map(|s| s.to_string());
    let mut tags: Vec<(String, String)> = constants.tags.clone();
    let mut fields: Vec<(String, String)> = Vec::new();
    let mut time: Option<i128> = None;

    for (name, value) in &constants.fields {
        fields.push((
            name.clone(),
            format_field_value(value, None, number_type)
                .map_err(|e| format!("constant field {name:?}: {e}"))?,
        ));
    }

    for (i, role) in roles.iter().enumerate() {
        let raw = cells.get(i).map(|s| s.as_str()).unwrap_or("");
        let value = if raw.trim().is_empty() {
            let d = defaults
                .get(i)
                .map(|s| s.as_str())
                .filter(|s| !s.trim().is_empty())
                .or_else(|| {
                    inline_defaults
                        .get(i)
                        .map(|s| s.as_str())
                        .filter(|s| !s.trim().is_empty())
                });
            match d {
                Some(d) => d,
                None => continue, // empty and no default: the column is absent
            }
        } else {
            raw
        };
        let name = names.get(i).map(|s| s.as_str()).unwrap_or("");
        match role {
            Role::Measurement => measurement = Some(value.trim().to_string()),
            Role::Tag => tags.push((name.to_string(), value.trim().to_string())),
            Role::Field(ty) => {
                let v = format_field_value(value, *ty, number_type)
                    .map_err(|e| format!("column {name:?}: {e}"))?;
                fields.push((name.to_string(), v));
            }
            Role::Time => {
                time = Some(
                    parse_timestamp_cell(value, scale)
                        .map_err(|e| format!("column {name:?}: {e}"))?,
                )
            }
            Role::Ignore => {}
        }
    }
    if time.is_none() {
        if let Some(c) = &constants.time {
            time = Some(parse_timestamp_cell(c, scale)?);
        }
    }
    if measurement.is_none() {
        measurement = constants.measurement.clone();
    }

    let measurement = measurement.filter(|m| !m.is_empty()).ok_or_else(|| {
        "empty measurement -- the measurement column has no value and no default".to_string()
    })?;
    if fields.is_empty() {
        return Err(
            "no field values -- every point needs at least one non-empty field".to_string(),
        );
    }
    if sort_keys {
        tags.sort_by(|a, b| a.0.cmp(&b.0));
    }

    let mut line = escape_measurement(&measurement);
    for (k, v) in &tags {
        if v.is_empty() {
            continue; // an empty tag value is the same as no tag in InfluxDB
        }
        line.push(',');
        line.push_str(&escape_key(k));
        line.push('=');
        line.push_str(&escape_key(v));
    }
    line.push(' ');
    for (i, (k, v)) in fields.iter().enumerate() {
        if i > 0 {
            line.push(',');
        }
        line.push_str(&escape_key(k));
        line.push('=');
        line.push_str(v);
    }
    if let Some(ns) = time {
        let ticks = ns.div_euclid(scale);
        line.push(' ');
        line.push_str(&ticks.to_string());
    }
    Ok(line)
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Convert between InfluxDB line protocol and CSV.
///
/// See the descriptor in `src/lib.rs` for the meaning of every parameter; each
/// one is validated here so all three surfaces report the same errors.
#[allow(clippy::too_many_arguments)]
pub fn convert(
    data: &str,
    direction: &str,
    csv_layout: &str,
    delimiter: &str,
    timestamp_format: &str,
    precision: &str,
    emit_annotations: bool,
    measurement: &str,
    tag_columns: &str,
    field_columns: &str,
    time_column: &str,
    number_type: &str,
    sort_keys: bool,
    on_error: &str,
) -> Result<String, String> {
    check_size(data)?;
    if data.trim().is_empty() {
        return Err("input is empty -- paste InfluxDB line protocol or CSV".to_string());
    }
    let delim = resolve_delimiter(delimiter)?;
    let scale = precision_scale(precision)?;
    let skip_bad = match on_error.trim() {
        "" | "stop" => false,
        "skip" => true,
        other => return Err(format!("unknown on_error {other:?} -- use stop or skip")),
    };

    let direction = match direction.trim() {
        "" | "auto" => detect_direction(data, scale),
        "lp-to-csv" => "lp-to-csv",
        "csv-to-lp" => "csv-to-lp",
        other => {
            return Err(format!(
                "unknown direction {other:?} -- use auto, lp-to-csv or csv-to-lp"
            ))
        }
    };

    if direction == "lp-to-csv" {
        lp_to_csv(
            data,
            csv_layout,
            delim,
            timestamp_format,
            scale,
            emit_annotations,
            sort_keys,
            skip_bad,
        )
    } else {
        csv_to_lp(
            data,
            delim,
            scale,
            measurement,
            tag_columns,
            field_columns,
            time_column,
            number_type,
            sort_keys,
            skip_bad,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lp2csv(data: &str) -> Result<String, String> {
        convert(
            data, "auto", "wide", "comma", "rfc3339", "ns", false, "", "", "", "", "float", true,
            "stop",
        )
    }

    #[test]
    fn line_protocol_to_wide_csv() {
        let out = lp2csv("cpu,host=host1 usage=64.23 1577836800000000000").unwrap();
        assert_eq!(
            out,
            "measurement,host,usage,time\ncpu,host1,64.23,2020-01-01T00:00:00Z"
        );
    }

    #[test]
    fn wide_csv_unions_tags_and_fields_and_sorts_keys() {
        let out = lp2csv(
            "cpu,host=b usage=1i 1577836800000000000\ncpu,region=eu free=2u,usage=3i 1577836801000000000",
        )
        .unwrap();
        assert_eq!(
            out,
            "measurement,host,region,free,usage,time\n\
             cpu,b,,,1,2020-01-01T00:00:00Z\n\
             cpu,,eu,2,3,2020-01-01T00:00:01Z"
        );
    }

    #[test]
    fn long_layout_emits_one_row_per_field() {
        let out = convert(
            "cpu,host=a load=0.5,busy=true 1000000000",
            "lp-to-csv",
            "long",
            "comma",
            "unix_s",
            "ns",
            false,
            "",
            "",
            "",
            "",
            "float",
            true,
            "stop",
        )
        .unwrap();
        assert_eq!(
            out,
            "measurement,host,field,value,time\ncpu,a,load,0.5,1\ncpu,a,busy,true,1"
        );
    }

    #[test]
    fn annotations_round_trip_back_to_line_protocol() {
        let lp = "mem,host=host1 used_percent=64.23 1577836800000000000";
        let csv = convert(
            lp,
            "lp-to-csv",
            "wide",
            "comma",
            "rfc3339",
            "ns",
            true,
            "",
            "",
            "",
            "",
            "float",
            true,
            "stop",
        )
        .unwrap();
        assert_eq!(
            csv,
            "#datatype measurement,tag,double,dateTime:RFC3339\n\
             measurement,host,used_percent,time\n\
             mem,host1,64.23,2020-01-01T00:00:00Z"
        );
        let back = convert(
            &csv,
            "csv-to-lp",
            "wide",
            "comma",
            "rfc3339",
            "ns",
            false,
            "",
            "",
            "",
            "",
            "float",
            true,
            "stop",
        )
        .unwrap();
        assert_eq!(back, lp);
    }

    #[test]
    fn escapes_and_string_fields_survive_both_directions() {
        let lp = r#"my\ m,tag\ key=tag\,val msg="he said \"hi\"",n=1i"#;
        let csv = convert(
            lp,
            "lp-to-csv",
            "wide",
            "comma",
            "rfc3339",
            "ns",
            false,
            "",
            "",
            "",
            "",
            "float",
            true,
            "stop",
        )
        .unwrap();
        assert_eq!(
            csv,
            "measurement,tag key,msg,n,time\nmy m,\"tag,val\",\"he said \"\"hi\"\"\",1,"
        );
    }

    #[test]
    fn csv_to_line_protocol_with_explicit_columns() {
        let out = convert(
            "time,host,region,usage\n2020-01-01T00:00:00Z,a,eu,1.5",
            "csv-to-lp",
            "wide",
            "comma",
            "rfc3339",
            "ns",
            false,
            "cpu",
            "host,region",
            "usage",
            "time",
            "float",
            true,
            "stop",
        )
        .unwrap();
        assert_eq!(out, "cpu,host=a,region=eu usage=1.5 1577836800000000000");
    }

    #[test]
    fn csv_to_line_protocol_respects_number_type_and_precision() {
        let out = convert(
            "host,count\na,7",
            "csv-to-lp",
            "wide",
            "comma",
            "rfc3339",
            "s",
            false,
            "hits",
            "host",
            "count",
            "",
            "integer",
            true,
            "stop",
        )
        .unwrap();
        assert_eq!(out, "hits,host=a count=7i");
    }

    #[test]
    fn inline_header_datatypes_and_constants_are_honoured() {
        let csv = "#constant measurement,disk\nhost|tag,free|long|0,time|dateTime:RFC3339\na,,2020-01-01T00:00:00Z";
        let out = convert(
            csv,
            "csv-to-lp",
            "wide",
            "comma",
            "rfc3339",
            "ns",
            false,
            "",
            "",
            "",
            "",
            "float",
            true,
            "stop",
        )
        .unwrap();
        assert_eq!(out, "disk,host=a free=0i 1577836800000000000");
    }

    #[test]
    fn semicolon_delimiter_and_sep_directive() {
        let out = convert(
            "sep=;\n#datatype measurement;tag;double\nm;host;v\ncpu;a;1.5",
            "csv-to-lp",
            "wide",
            "comma",
            "rfc3339",
            "ns",
            false,
            "",
            "",
            "",
            "",
            "float",
            true,
            "stop",
        )
        .unwrap();
        assert_eq!(out, "cpu,host=a v=1.5");
    }

    #[test]
    fn direction_is_auto_detected_both_ways() {
        assert_eq!(detect_direction("cpu,host=a v=1 1000", 1), "lp-to-csv");
        assert_eq!(detect_direction("time,host,v\n1,a,2", 1), "csv-to-lp");
        assert_eq!(
            detect_direction("#datatype measurement,tag\nm,host\ncpu,a", 1),
            "csv-to-lp"
        );
    }

    #[test]
    fn skip_mode_drops_only_the_bad_lines() {
        let out = convert(
            "cpu,host=a v=1i 1000000000\nnot line protocol at all\ncpu,host=b v=2i 2000000000",
            "lp-to-csv",
            "wide",
            "comma",
            "unix_s",
            "ns",
            false,
            "",
            "",
            "",
            "",
            "float",
            true,
            "skip",
        )
        .unwrap();
        assert_eq!(out, "measurement,host,v,time\ncpu,a,1,1\ncpu,b,2,2");
    }

    // ---- error paths -----------------------------------------------------

    #[test]
    fn malformed_line_protocol_reports_the_line_number() {
        let err = lp2csv("cpu,host=a v=1i 1000\ncpu,host=b oops 2000").unwrap_err();
        assert!(err.starts_with("line 2: "), "got {err}");
        assert!(err.contains("missing an `=`"), "got {err}");
    }

    #[test]
    fn csv_without_a_measurement_is_rejected() {
        let err = convert(
            "host,usage\na,1.5",
            "csv-to-lp",
            "wide",
            "comma",
            "rfc3339",
            "ns",
            false,
            "",
            "",
            "",
            "",
            "float",
            true,
            "stop",
        )
        .unwrap_err();
        assert!(err.contains("no measurement"), "got {err}");
    }

    #[test]
    fn unknown_datatype_is_rejected() {
        let err = convert(
            "#datatype measurement,widget\nm,x\ncpu,1",
            "csv-to-lp",
            "wide",
            "comma",
            "rfc3339",
            "ns",
            false,
            "",
            "",
            "",
            "",
            "float",
            true,
            "stop",
        )
        .unwrap_err();
        assert!(err.contains("unknown datatype"), "got {err}");
    }

    #[test]
    fn unknown_option_values_are_rejected() {
        let err = convert(
            "cpu v=1", "auto", "wide", "colon", "rfc3339", "ns", false, "", "", "", "", "float",
            true, "stop",
        )
        .unwrap_err();
        assert!(err.contains("unknown delimiter"), "got {err}");
        let err = convert(
            "cpu v=1", "auto", "long", "comma", "rfc3339", "ns", true, "", "", "", "", "float",
            true, "stop",
        )
        .unwrap_err();
        assert!(err.contains("csv_layout=wide"), "got {err}");
    }

    #[test]
    fn empty_input_is_rejected() {
        let err = lp2csv("   \n\n").unwrap_err();
        assert!(err.contains("input is empty"), "got {err}");
    }

    #[test]
    fn missing_named_column_is_rejected() {
        let err = convert(
            "host,usage\na,1.5",
            "csv-to-lp",
            "wide",
            "comma",
            "rfc3339",
            "ns",
            false,
            "cpu",
            "nope",
            "",
            "",
            "float",
            true,
            "stop",
        )
        .unwrap_err();
        assert!(err.contains("no column named \"nope\""), "got {err}");
    }

    #[test]
    fn over_the_character_limit_is_rejected() {
        let big = "x".repeat(MAX_CHARS + 1);
        let err = lp2csv(&big).unwrap_err();
        assert!(err.contains("character limit"), "got {err}");
    }
}
