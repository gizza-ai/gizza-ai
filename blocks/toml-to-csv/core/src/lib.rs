//! toml-to-csv core — pure compute, shared by the chat skill block and the web
//! page. No wafer/wasm-bindgen deps.
//!
//! Turns a TOML array-of-tables (`[[users]]`) into a CSV table: one row per
//! entry, one column per key, with the header taken from the union of keys seen
//! across every entry so ragged entries don't silently lose data.
//!
//! Nested tables can be flattened into dot-notation columns (`address.city`),
//! JSON-stringified into a single cell, or skipped. Scalar arrays can be
//! JSON-stringified, joined with `; `, or expanded into indexed columns
//! (`tags.1`, `tags.2`).
//!
//! It is a reshaper, not a TOML interpreter: comments are dropped (TOML parsers
//! don't retain them), dates/times are emitted in their canonical TOML text
//! form, and no arithmetic or type coercion is applied to values.

use csv::WriterBuilder;
use std::collections::BTreeMap;
use toml::Value;

/// Hard cap on rows (array-of-tables entries) converted in one call, so a huge
/// paste can't blow up memory. The chat/CLI schema advertises this same bound.
pub const MAX_ROWS: usize = 20_000;

/// Hard cap on distinct columns in the unioned header. Deeply nested flattening
/// of a wide document can otherwise explode the header.
pub const MAX_COLUMNS: usize = 2_000;

/// Separator used by `array_format = "join"` when folding a scalar array into
/// one cell.
const JOIN_SEPARATOR: &str = "; ";

/// How a nested table inside a row is rendered.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Nested {
    /// `address = { city = "Berlin" }` → an `address.city` column.
    Flatten,
    /// `address` → one cell holding `{"city":"Berlin"}`.
    Json,
    /// The nested table is dropped entirely.
    Skip,
}

impl Nested {
    fn parse(s: &str) -> Result<Nested, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "flatten" => Ok(Nested::Flatten),
            "json" => Ok(Nested::Json),
            "skip" => Ok(Nested::Skip),
            other => Err(format!(
                "unknown nested '{other}' (expected flatten, json, or skip)"
            )),
        }
    }
}

/// How an array value inside a row is rendered.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ArrayFormat {
    /// `tags = ["a","b"]` → one cell holding `["a","b"]`.
    Json,
    /// `tags = ["a","b"]` → one cell holding `a; b`.
    Join,
    /// `tags = ["a","b"]` → `tags.1` = `a`, `tags.2` = `b`.
    Columns,
}

impl ArrayFormat {
    fn parse(s: &str) -> Result<ArrayFormat, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "json" => Ok(ArrayFormat::Json),
            "join" => Ok(ArrayFormat::Join),
            "columns" => Ok(ArrayFormat::Columns),
            other => Err(format!(
                "unknown array_format '{other}' (expected json, join, or columns)"
            )),
        }
    }
}

/// Header column order.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Columns {
    /// Every key seen in any row, in first-seen document order.
    Union,
    /// Every key seen in any row, sorted alphabetically.
    Sorted,
    /// Only the keys of the first row — later extra keys are dropped.
    First,
}

impl Columns {
    fn parse(s: &str) -> Result<Columns, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "union" => Ok(Columns::Union),
            "sorted" => Ok(Columns::Sorted),
            "first" => Ok(Columns::First),
            other => Err(format!(
                "unknown columns '{other}' (expected union, sorted, or first)"
            )),
        }
    }
}

fn delimiter_byte(s: &str) -> Result<u8, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "" | "comma" | "," => Ok(b','),
        "semicolon" | ";" => Ok(b';'),
        "tab" | "\t" => Ok(b'\t'),
        "pipe" | "|" => Ok(b'|'),
        other => Err(format!(
            "unknown delimiter '{other}' (expected comma, semicolon, tab, or pipe)"
        )),
    }
}

/// Render a TOML scalar the way it appeared in the document: strings unquoted,
/// numbers/booleans/datetimes in their canonical text form.
fn scalar_to_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Integer(i) => i.to_string(),
        Value::Float(f) => {
            // Keep integral floats readable (1.0 stays "1.0", per TOML's own
            // Display) but avoid "inf"/"NaN" surprises leaking as bare words.
            if f.is_finite() {
                let s = f.to_string();
                if s.contains('.') || s.contains('e') || s.contains('E') {
                    s
                } else {
                    format!("{s}.0")
                }
            } else {
                v.to_string()
            }
        }
        Value::Boolean(b) => b.to_string(),
        Value::Datetime(d) => d.to_string(),
        // Tables/arrays never reach here — handled by flatten_value.
        other => other.to_string(),
    }
}

/// TOML → JSON text, for the `json` cell renderings. `toml::Value` and
/// `serde_json::Value` don't share a type, so map it explicitly (datetimes have
/// no JSON equivalent and become their TOML text form).
fn to_json(v: &Value) -> serde_json::Value {
    match v {
        Value::String(s) => serde_json::Value::String(s.clone()),
        Value::Integer(i) => serde_json::Value::from(*i),
        Value::Float(f) => serde_json::Number::from_f64(*f)
            .map(serde_json::Value::Number)
            .unwrap_or(serde_json::Value::Null),
        Value::Boolean(b) => serde_json::Value::Bool(*b),
        Value::Datetime(d) => serde_json::Value::String(d.to_string()),
        Value::Array(items) => serde_json::Value::Array(items.iter().map(to_json).collect()),
        Value::Table(t) => serde_json::Value::Object(
            t.iter()
                .map(|(k, v)| (k.clone(), to_json(v)))
                .collect::<serde_json::Map<_, _>>(),
        ),
    }
}

/// Flatten one value into `(column, cell)` pairs under `prefix`, honouring the
/// nested-table and array policies.
fn flatten_value(
    prefix: &str,
    value: &Value,
    nested: Nested,
    array_format: ArrayFormat,
    out: &mut Vec<(String, String)>,
) {
    match value {
        Value::Table(table) => match nested {
            Nested::Skip => {}
            Nested::Json => out.push((prefix.to_string(), to_json(value).to_string())),
            Nested::Flatten => {
                if table.is_empty() {
                    // An empty inline table has no leaf to name; keep the column
                    // so the key isn't silently invisible.
                    out.push((prefix.to_string(), String::new()));
                }
                for (k, v) in table.iter() {
                    let key = if prefix.is_empty() {
                        k.clone()
                    } else {
                        format!("{prefix}.{k}")
                    };
                    flatten_value(&key, v, nested, array_format, out);
                }
            }
        },
        Value::Array(items) => match array_format {
            ArrayFormat::Json => out.push((prefix.to_string(), to_json(value).to_string())),
            ArrayFormat::Join => {
                let joined = items
                    .iter()
                    .map(|item| match item {
                        Value::Table(_) | Value::Array(_) => to_json(item).to_string(),
                        scalar => scalar_to_string(scalar),
                    })
                    .collect::<Vec<_>>()
                    .join(JOIN_SEPARATOR);
                out.push((prefix.to_string(), joined));
            }
            ArrayFormat::Columns => {
                if items.is_empty() {
                    out.push((prefix.to_string(), String::new()));
                }
                for (i, item) in items.iter().enumerate() {
                    // 1-based, so `tags.1` reads as "the first tag" on a sheet.
                    let key = format!("{prefix}.{}", i + 1);
                    flatten_value(&key, item, nested, array_format, out);
                }
            }
        },
        scalar => out.push((prefix.to_string(), scalar_to_string(scalar))),
    }
}

/// Depth-first search for an array-of-tables, in document order, returning its
/// dotted path. Root-level arrays win over nested ones so the common
/// `[[users]]` case is picked even when a sub-table also holds one.
fn find_array_of_tables(table: &toml::map::Map<String, Value>, prefix: &str) -> Option<String> {
    for (k, v) in table.iter() {
        if let Value::Array(items) = v {
            if !items.is_empty() && items.iter().all(|i| matches!(i, Value::Table(_))) {
                return Some(join_path(prefix, k));
            }
        }
    }
    for (k, v) in table.iter() {
        if let Value::Table(sub) = v {
            if let Some(found) = find_array_of_tables(sub, &join_path(prefix, k)) {
                return Some(found);
            }
        }
    }
    None
}

/// Collect every array-of-tables path in the document, for the "not found"
/// error message (so the user learns what they *could* have asked for).
fn collect_array_paths(table: &toml::map::Map<String, Value>, prefix: &str, out: &mut Vec<String>) {
    for (k, v) in table.iter() {
        match v {
            Value::Array(items)
                if !items.is_empty() && items.iter().all(|i| matches!(i, Value::Table(_))) =>
            {
                out.push(join_path(prefix, k));
            }
            Value::Table(sub) => collect_array_paths(sub, &join_path(prefix, k), out),
            _ => {}
        }
    }
}

fn join_path(prefix: &str, key: &str) -> String {
    if prefix.is_empty() {
        key.to_string()
    } else {
        format!("{prefix}.{key}")
    }
}

/// Resolve a dotted path (`servers.pool`) to a value. Quoted TOML keys are not
/// supported in the path — plain dotted segments only.
fn resolve_path<'a>(root: &'a Value, path: &str) -> Option<&'a Value> {
    let mut current = root;
    for segment in path.split('.') {
        let table = current.as_table()?;
        current = table.get(segment)?;
    }
    Some(current)
}

/// Convert a TOML document into CSV text.
///
/// * `input` — the TOML document.
/// * `table` — dotted path of the array-of-tables to convert; empty auto-detects.
/// * `nested` — `flatten` | `json` | `skip` for nested tables.
/// * `array_format` — `json` | `join` | `columns` for array values.
/// * `columns` — `union` | `sorted` | `first` header ordering.
/// * `delimiter` — `comma` | `semicolon` | `tab` | `pipe`.
/// * `include_header` — write the header row.
pub fn convert(
    input: &str,
    table: &str,
    nested: &str,
    array_format: &str,
    columns: &str,
    delimiter: &str,
    include_header: bool,
) -> Result<String, String> {
    let nested = Nested::parse(nested)?;
    let array_format = ArrayFormat::parse(array_format)?;
    let columns = Columns::parse(columns)?;
    let delim = delimiter_byte(delimiter)?;

    if input.trim().is_empty() {
        return Err("input is empty — paste a TOML document with an array-of-tables, e.g. [[users]]".into());
    }

    let doc: Value = toml::from_str::<Value>(input).map_err(|e| format!("invalid TOML: {e}"))?;
    let root = doc
        .as_table()
        .ok_or_else(|| "invalid TOML: the document root must be a table".to_string())?;

    // Pick the rows: the requested path, the auto-detected array-of-tables, or
    // the root table itself as a single row.
    let path = table.trim();
    let rows: Vec<&toml::map::Map<String, Value>> = if path.is_empty() {
        match find_array_of_tables(root, "") {
            Some(found) => match resolve_path(&doc, &found) {
                Some(Value::Array(items)) => items.iter().filter_map(|i| i.as_table()).collect(),
                _ => return Err(format!("internal: '{found}' is not an array-of-tables")),
            },
            // No array-of-tables anywhere: treat a flat document as one row.
            None => {
                if root.is_empty() {
                    return Err("no data found: the TOML document is empty".into());
                }
                vec![root]
            }
        }
    } else {
        match resolve_path(&doc, path) {
            Some(Value::Array(items)) => {
                if items.is_empty() {
                    return Err(format!("table '{path}' is an empty array — nothing to convert"));
                }
                if !items.iter().all(|i| matches!(i, Value::Table(_))) {
                    return Err(format!(
                        "table '{path}' is an array of values, not an array-of-tables — each entry must be a [[{path}]] table"
                    ));
                }
                items.iter().filter_map(|i| i.as_table()).collect()
            }
            Some(Value::Table(t)) => vec![t],
            Some(_) => {
                return Err(format!(
                    "table '{path}' is a scalar value, not an array-of-tables or table"
                ))
            }
            None => {
                let mut found = Vec::new();
                collect_array_paths(root, "", &mut found);
                let hint = if found.is_empty() {
                    "the document has no array-of-tables".to_string()
                } else {
                    format!("available: {}", found.join(", "))
                };
                return Err(format!("table '{path}' not found — {hint}"));
            }
        }
    };

    if rows.len() > MAX_ROWS {
        return Err(format!(
            "too many rows: {} entries exceeds the {MAX_ROWS} limit",
            rows.len()
        ));
    }

    // Flatten every row first, then derive the header from what was produced.
    let mut flat_rows: Vec<Vec<(String, String)>> = Vec::with_capacity(rows.len());
    let mut header: Vec<String> = Vec::new();
    let mut seen: BTreeMap<String, ()> = BTreeMap::new();
    for row in &rows {
        let mut cells = Vec::new();
        for (k, v) in row.iter() {
            flatten_value(k, v, nested, array_format, &mut cells);
        }
        for (k, _) in &cells {
            if seen.insert(k.clone(), ()).is_none() {
                header.push(k.clone());
                if header.len() > MAX_COLUMNS {
                    return Err(format!(
                        "too many columns: the flattened header exceeds the {MAX_COLUMNS} limit — try nested=json or a narrower table"
                    ));
                }
            }
        }
        flat_rows.push(cells);
    }

    let header: Vec<String> = match columns {
        Columns::Union => header,
        Columns::Sorted => {
            let mut sorted = header;
            sorted.sort();
            sorted
        }
        Columns::First => {
            let mut first: Vec<String> = Vec::new();
            if let Some(cells) = flat_rows.first() {
                for (k, _) in cells {
                    if !first.contains(k) {
                        first.push(k.clone());
                    }
                }
            }
            first
        }
    };

    if header.is_empty() {
        return Err(
            "no columns to write — every value was skipped (try nested=flatten or nested=json)"
                .into(),
        );
    }

    let mut writer = WriterBuilder::new()
        .delimiter(delim)
        .from_writer(Vec::new());
    if include_header {
        writer
            .write_record(&header)
            .map_err(|e| format!("CSV write error: {e}"))?;
    }
    for cells in &flat_rows {
        // Later duplicates can't occur (TOML keys are unique per table), but a
        // lookup map keeps the per-row cost linear rather than quadratic.
        let lookup: BTreeMap<&str, &str> = cells
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        let record: Vec<&str> = header
            .iter()
            .map(|h| *lookup.get(h.as_str()).unwrap_or(&""))
            .collect();
        writer
            .write_record(&record)
            .map_err(|e| format!("CSV write error: {e}"))?;
    }
    let bytes = writer
        .into_inner()
        .map_err(|e| format!("CSV write error: {e}"))?;
    String::from_utf8(bytes).map_err(|e| format!("CSV encoding error: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const USERS: &str = r#"
[[users]]
name = "Ada"
role = "admin"

[[users]]
name = "Linus"
role = "dev"
"#;

    fn conv(input: &str) -> String {
        convert(input, "", "flatten", "json", "union", "comma", true).unwrap()
    }

    #[test]
    fn array_of_tables_becomes_rows() {
        assert_eq!(conv(USERS), "name,role\nAda,admin\nLinus,dev\n");
    }

    #[test]
    fn header_is_the_union_of_keys_in_first_seen_order() {
        let out = conv("[[r]]\na = 1\n\n[[r]]\nb = 2\na = 3\n");
        assert_eq!(out, "a,b\n1,\n3,2\n");
    }

    #[test]
    fn columns_sorted_and_first_modes() {
        let toml = "[[r]]\nb = 1\na = 2\n\n[[r]]\nc = 3\n";
        let sorted = convert(toml, "", "flatten", "json", "sorted", "comma", true).unwrap();
        assert_eq!(sorted, "a,b,c\n2,1,\n,,3\n");
        let first = convert(toml, "", "flatten", "json", "first", "comma", true).unwrap();
        assert_eq!(first, "b,a\n1,2\n,\n");
    }

    #[test]
    fn nested_tables_flatten_to_dot_columns() {
        let out = conv("[[s]]\nname = \"web\"\n\n[s.addr]\ncity = \"Berlin\"\nzip = \"10115\"\n");
        assert_eq!(out, "name,addr.city,addr.zip\nweb,Berlin,10115\n");
    }

    #[test]
    fn nested_json_and_skip_modes() {
        let toml = "[[s]]\nname = \"web\"\naddr = { city = \"Berlin\" }\n";
        let json = convert(toml, "", "json", "json", "union", "comma", true).unwrap();
        assert_eq!(json, "name,addr\nweb,\"{\"\"city\"\":\"\"Berlin\"\"}\"\n");
        let skip = convert(toml, "", "skip", "json", "union", "comma", true).unwrap();
        assert_eq!(skip, "name\nweb\n");
    }

    #[test]
    fn array_values_json_join_and_columns() {
        let toml = "[[p]]\nname = \"a\"\ntags = [\"x\", \"y\"]\n";
        let json = convert(toml, "", "flatten", "json", "union", "comma", true).unwrap();
        assert_eq!(json, "name,tags\na,\"[\"\"x\"\",\"\"y\"\"]\"\n");
        let join = convert(toml, "", "flatten", "join", "union", "comma", true).unwrap();
        assert_eq!(join, "name,tags\na,x; y\n");
        let cols = convert(toml, "", "flatten", "columns", "union", "comma", true).unwrap();
        assert_eq!(cols, "name,tags.1,tags.2\na,x,y\n");
    }

    #[test]
    fn explicit_dotted_table_path_selects_the_right_array() {
        let toml = "[[users]]\nname = \"Ada\"\n\n[servers]\n[[servers.pool]]\nhost = \"a\"\n";
        let out = convert(toml, "servers.pool", "flatten", "json", "union", "comma", true).unwrap();
        assert_eq!(out, "host\na\n");
    }

    #[test]
    fn root_level_array_wins_over_nested_on_auto_detect() {
        let toml = "[[users]]\nname = \"Ada\"\n\n[servers]\n[[servers.pool]]\nhost = \"a\"\n";
        assert_eq!(conv(toml), "name\nAda\n");
    }

    #[test]
    fn flat_document_without_array_of_tables_becomes_one_row() {
        let out = conv("title = \"cfg\"\nport = 8080\n");
        assert_eq!(out, "title,port\ncfg,8080\n");
    }

    #[test]
    fn scalar_types_render_in_toml_text_form() {
        let out = conv("[[r]]\ni = 42\nf = 1.0\nb = true\nd = 1979-05-27T07:32:00Z\n");
        assert_eq!(out, "i,f,b,d\n42,1.0,true,1979-05-27T07:32:00Z\n");
    }

    #[test]
    fn delimiters_and_header_toggle() {
        let semi = convert(USERS, "", "flatten", "json", "union", "semicolon", true).unwrap();
        assert_eq!(semi, "name;role\nAda;admin\nLinus;dev\n");
        let tab = convert(USERS, "", "flatten", "json", "union", "tab", true).unwrap();
        assert_eq!(tab, "name\trole\nAda\tadmin\nLinus\tdev\n");
        let pipe = convert(USERS, "", "flatten", "json", "union", "pipe", true).unwrap();
        assert_eq!(pipe, "name|role\nAda|admin\nLinus|dev\n");
        let bare = convert(USERS, "", "flatten", "json", "union", "comma", false).unwrap();
        assert_eq!(bare, "Ada,admin\nLinus,dev\n");
    }

    #[test]
    fn values_with_separators_are_quoted() {
        let out = conv("[[r]]\nname = \"Doe, Jane\"\nquote = \"say \\\"hi\\\"\"\n");
        assert_eq!(out, "name,quote\n\"Doe, Jane\",\"say \"\"hi\"\"\"\n");
    }

    #[test]
    fn empty_input_is_an_error() {
        let err = convert("   ", "", "flatten", "json", "union", "comma", true).unwrap_err();
        assert!(err.contains("input is empty"), "{err}");
    }

    #[test]
    fn invalid_toml_is_an_error() {
        let err = convert("[[users]\nname =", "", "flatten", "json", "union", "comma", true)
            .unwrap_err();
        assert!(err.starts_with("invalid TOML:"), "{err}");
    }

    #[test]
    fn missing_table_lists_what_is_available() {
        let err = convert(USERS, "people", "flatten", "json", "union", "comma", true).unwrap_err();
        assert_eq!(err, "table 'people' not found — available: users");
    }

    #[test]
    fn array_of_scalars_is_rejected_with_a_clear_message() {
        let err = convert(
            "ports = [1, 2]\n",
            "ports",
            "flatten",
            "json",
            "union",
            "comma",
            true,
        )
        .unwrap_err();
        assert!(err.contains("array of values, not an array-of-tables"), "{err}");
    }

    #[test]
    fn unknown_enum_values_are_rejected() {
        assert!(convert(USERS, "", "nope", "json", "union", "comma", true)
            .unwrap_err()
            .contains("unknown nested 'nope'"));
        assert!(convert(USERS, "", "flatten", "nope", "union", "comma", true)
            .unwrap_err()
            .contains("unknown array_format 'nope'"));
        assert!(convert(USERS, "", "flatten", "json", "nope", "comma", true)
            .unwrap_err()
            .contains("unknown columns 'nope'"));
        assert!(convert(USERS, "", "flatten", "json", "union", "nope", true)
            .unwrap_err()
            .contains("unknown delimiter 'nope'"));
    }

    #[test]
    fn skipping_every_value_is_an_error_not_an_empty_file() {
        let err = convert(
            "[[r]]\naddr = { city = \"Berlin\" }\n",
            "",
            "skip",
            "json",
            "union",
            "comma",
            true,
        )
        .unwrap_err();
        assert!(err.contains("no columns to write"), "{err}");
    }

    #[test]
    fn row_cap_is_enforced() {
        let big: String = (0..MAX_ROWS + 1).map(|i| format!("[[r]]\nn = {i}\n")).collect();
        let err = convert(&big, "", "flatten", "json", "union", "comma", true).unwrap_err();
        assert!(err.contains("too many rows"), "{err}");
    }

    #[test]
    fn row_cap_boundary_is_accepted() {
        let big: String = (0..MAX_ROWS).map(|i| format!("[[r]]\nn = {i}\n")).collect();
        let out = convert(&big, "", "flatten", "json", "union", "comma", true).unwrap();
        assert_eq!(out.lines().count(), MAX_ROWS + 1);
    }
}
