//! gizza-ai/ldif-to-csv core — parse an LDAP LDIF export (RFC 2849) into CSV.
//!
//! Each LDIF entry becomes one CSV row; attribute names become columns. The
//! reader handles the full on-the-wire shape of an export: an optional
//! `version:` header, `#` comment lines, blank-line record separation, folded
//! continuation lines (a leading single space), and all three value forms —
//! plain `attr: value`, base64 `attr:: dmFsdWU=`, and URL-reference
//! `attr:< file:///path`. Change files (`changetype: modify`) are understood so
//! that the modification directives don't leak into the table.
//!
//! Pure Rust (`base64` + `csv`); no wafer/wasm-bindgen deps.

use std::collections::HashMap;

use base64::engine::general_purpose::{STANDARD, STANDARD_NO_PAD};
use base64::Engine;

/// Hard ceiling on entries so a pathological paste can't hang the browser tab.
pub const MAX_ENTRIES: usize = 50_000;
/// Upper bound accepted for the `max_indexed` option.
pub const MAX_INDEXED_LIMIT: i64 = 50;

/// LDIF change-record protocol keywords. They are structure, not data, so they
/// never become columns — but only inside a record that declares a
/// `changetype`, so an ordinary entry with a (bizarre) `add` attribute is safe.
const CHANGE_KEYWORDS: [&str; 7] = [
    "changetype",
    "add",
    "replace",
    "delete",
    "newrdn",
    "deleteoldrdn",
    "newsuperior",
];

/// How repeated attributes (`objectClass`, `memberOf`, …) map onto cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MultiValue {
    /// One cell, values joined by the separator.
    Join,
    /// One column per occurrence: `memberOf`, `memberOf.2`, `memberOf.3`, …
    Indexed,
    /// Keep only the first value.
    First,
    /// Keep only the last value.
    Last,
}

impl MultiValue {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "join" => Ok(MultiValue::Join),
            "indexed" => Ok(MultiValue::Indexed),
            "first" => Ok(MultiValue::First),
            "last" => Ok(MultiValue::Last),
            other => Err(format!(
                "multi_value must be one of join/indexed/first/last, got '{other}'"
            )),
        }
    }
}

/// Conversion options. Mirrors the block descriptor / page fields one-for-one.
#[derive(Debug, Clone)]
pub struct Options {
    /// Comma-separated attribute names to emit, in that order. Empty = every
    /// attribute found, in first-seen order.
    pub columns: String,
    /// Emit the entry DN as a column (first, unless `columns` places it).
    pub include_dn: bool,
    /// Repeated-attribute policy.
    pub multi_value: MultiValue,
    /// Separator used by `join` (and by `indexed` overflow). Empty = `|`.
    pub value_separator: String,
    /// Max columns per attribute in `indexed` mode (1..=50).
    pub max_indexed: i64,
    /// Decode `attr:: base64` values to text when the bytes are valid UTF-8.
    pub decode_base64: bool,
    /// CSV field separator: one char, or comma/tab/semicolon/pipe.
    pub delimiter: String,
    /// Emit the change-record operation as a `changetype` column.
    pub include_changetype: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            columns: String::new(),
            include_dn: true,
            multi_value: MultiValue::Join,
            value_separator: "|".to_string(),
            max_indexed: 10,
            decode_base64: true,
            delimiter: ",".to_string(),
            include_changetype: false,
        }
    }
}

/// One parsed LDIF entry.
#[derive(Debug, Default)]
struct Entry {
    dn: String,
    changetype: String,
    /// Attribute values keyed by case-folded name; index into `order`.
    values: Vec<Vec<String>>,
    index: HashMap<String, usize>,
}

impl Entry {
    fn push(&mut self, folded: &str, value: String) -> usize {
        match self.index.get(folded) {
            Some(&i) => {
                self.values[i].push(value);
                i
            }
            None => {
                let i = self.values.len();
                self.index.insert(folded.to_string(), i);
                self.values.push(vec![value]);
                i
            }
        }
    }

    fn get(&self, folded: &str) -> &[String] {
        match self.index.get(folded) {
            Some(&i) => &self.values[i],
            None => &[],
        }
    }
}

/// A resolved output column.
enum ColSpec {
    Dn,
    ChangeType,
    /// (header spelling, case-folded lookup key)
    Attr(String, String),
}

fn fold(name: &str) -> String {
    name.trim().to_ascii_lowercase()
}

fn delim_byte(d: &str) -> Result<u8, String> {
    Ok(match d {
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
                    "delimiter must be a single char or tab/comma/semicolon/pipe, got '{other}'"
                ));
            }
        }
    })
}

/// Shorten a line for an error message so a huge paste can't flood the output.
fn snip(s: &str) -> String {
    let t = s.trim();
    if t.chars().count() <= 60 {
        t.to_string()
    } else {
        format!("{}…", t.chars().take(60).collect::<String>())
    }
}

/// Undo RFC 2849 line folding: a physical line starting with a single SPACE is
/// a continuation of the previous logical line, with that space removed. An
/// empty logical line marks a record boundary and is preserved as such.
fn unfold(ldif: &str) -> Vec<String> {
    let mut logical: Vec<String> = Vec::new();
    for raw in ldif.split('\n') {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        match line.strip_prefix(' ') {
            Some(rest) => match logical.last_mut() {
                // Continuations only attach to a non-empty preceding line.
                Some(last) if !last.is_empty() => last.push_str(rest),
                _ => logical.push(rest.to_string()),
            },
            None => logical.push(line.to_string()),
        }
    }
    logical
}

/// Decode one `attr:: <base64>` payload. Invalid base64 is an error; binary
/// (non-UTF-8) payloads are returned in their original base64 form so the CSV
/// stays valid text.
fn decode_b64(name: &str, raw: &str) -> Result<String, String> {
    let compact: String = raw.chars().filter(|c| !c.is_whitespace()).collect();
    let bytes = STANDARD
        .decode(compact.as_bytes())
        .or_else(|_| STANDARD_NO_PAD.decode(compact.as_bytes()))
        .map_err(|e| format!("attribute '{name}' has an invalid base64 value: {e}"))?;
    Ok(String::from_utf8(bytes).unwrap_or_else(|_| raw.trim().to_string()))
}

/// Split one logical LDIF line into (attribute name, value), applying the
/// value-spec forms of RFC 2849.
fn parse_line(line: &str, decode_base64: bool) -> Result<(String, String), String> {
    let colon = line.find(':').ok_or_else(|| {
        format!(
            "invalid LDIF line (expected 'attribute: value'): '{}'",
            snip(line)
        )
    })?;
    let name = line[..colon].trim().to_string();
    if name.is_empty() {
        return Err(format!(
            "invalid LDIF line (empty attribute name): '{}'",
            snip(line)
        ));
    }
    let rest = &line[colon + 1..];
    let value = if let Some(b64) = rest.strip_prefix(':') {
        // `attr:: base64` — FILL is any run of spaces.
        let payload = b64.trim_start_matches(' ');
        if !decode_base64 {
            payload.to_string()
        } else {
            decode_b64(&name, payload)?
        }
    } else if let Some(url) = rest.strip_prefix('<') {
        // `attr:< url` — the value lives elsewhere; keep the reference as text.
        url.trim_start_matches(' ').to_string()
    } else {
        rest.trim_start_matches(' ').to_string()
    };
    Ok((name, value))
}

/// Parse the LDIF text into entries, remembering first-seen attribute order.
fn parse_entries(
    ldif: &str,
    decode_base64: bool,
) -> Result<(Vec<Entry>, Vec<(String, String)>), String> {
    let mut entries: Vec<Entry> = Vec::new();
    let mut order: Vec<(String, String)> = Vec::new(); // (header spelling, folded)
    let mut seen_attr: HashMap<String, ()> = HashMap::new();
    let mut cur: Option<Entry> = None;
    let mut is_change = false;
    let mut started = false;

    for line in unfold(ldif) {
        if line.trim().is_empty() {
            if let Some(e) = cur.take() {
                entries.push(e);
                if entries.len() > MAX_ENTRIES {
                    return Err(format!(
                        "too many LDIF entries (limit {MAX_ENTRIES}); split the file and convert it in parts"
                    ));
                }
            }
            is_change = false;
            continue;
        }
        if line.starts_with('#') || line.trim() == "-" {
            continue;
        }
        let (name, value) = parse_line(&line, decode_base64)?;
        let folded = fold(&name);

        // `version: 1` header, before any entry has started.
        if folded == "version" && cur.is_none() && !started {
            continue;
        }
        // `control:` is protocol framing that precedes changetype.
        if folded == "control" {
            continue;
        }

        if folded == "dn" {
            if let Some(e) = cur.take() {
                entries.push(e);
                if entries.len() > MAX_ENTRIES {
                    return Err(format!(
                        "too many LDIF entries (limit {MAX_ENTRIES}); split the file and convert it in parts"
                    ));
                }
            }
            is_change = false;
            started = true;
            cur = Some(Entry {
                dn: value,
                ..Default::default()
            });
            continue;
        }

        let entry = match cur.as_mut() {
            Some(e) => e,
            None => {
                return Err(format!(
                    "LDIF entry must start with a 'dn:' line, found '{name}'"
                ))
            }
        };

        if folded == "changetype" {
            is_change = true;
            entry.changetype = value;
            continue;
        }
        if is_change && CHANGE_KEYWORDS.contains(&folded.as_str()) {
            continue;
        }

        entry.push(&folded, value);
        if seen_attr.insert(folded.clone(), ()).is_none() {
            order.push((name, folded));
        }
    }
    if let Some(e) = cur.take() {
        entries.push(e);
    }
    if entries.len() > MAX_ENTRIES {
        return Err(format!(
            "too many LDIF entries (limit {MAX_ENTRIES}); split the file and convert it in parts"
        ));
    }
    if entries.is_empty() {
        return Err(
            "no LDIF entries found — expected at least one 'dn: …' line followed by attributes"
                .to_string(),
        );
    }
    Ok((entries, order))
}

/// Build the ordered output columns from the options and the parsed data.
fn resolve_columns(opts: &Options, order: &[(String, String)]) -> Result<Vec<ColSpec>, String> {
    let requested: Vec<String> = opts
        .columns
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();

    let mut specs: Vec<ColSpec> = Vec::new();
    if requested.is_empty() {
        if opts.include_dn {
            specs.push(ColSpec::Dn);
        }
        if opts.include_changetype {
            specs.push(ColSpec::ChangeType);
        }
        for (spelling, folded) in order {
            specs.push(ColSpec::Attr(spelling.clone(), folded.clone()));
        }
    } else {
        let folded: Vec<String> = requested.iter().map(|s| fold(s)).collect();
        if opts.include_dn && !folded.iter().any(|f| f == "dn") {
            specs.push(ColSpec::Dn);
        }
        if opts.include_changetype && !folded.iter().any(|f| f == "changetype") {
            specs.push(ColSpec::ChangeType);
        }
        let mut seen: HashMap<String, ()> = HashMap::new();
        for (name, f) in requested.iter().zip(folded.iter()) {
            if seen.insert(f.clone(), ()).is_some() {
                continue;
            }
            match f.as_str() {
                "dn" => specs.push(ColSpec::Dn),
                "changetype" => specs.push(ColSpec::ChangeType),
                _ => specs.push(ColSpec::Attr(name.clone(), f.clone())),
            }
        }
    }
    if specs.is_empty() {
        return Err(
            "no columns to output — enable the DN column or list attributes in 'columns'"
                .to_string(),
        );
    }
    Ok(specs)
}

/// Convert an LDIF export into CSV.
pub fn to_csv(ldif: &str, opts: &Options) -> Result<String, String> {
    if ldif.trim().is_empty() {
        return Err("no LDIF text supplied — paste an LDIF export to convert".to_string());
    }
    if opts.max_indexed < 1 || opts.max_indexed > MAX_INDEXED_LIMIT {
        return Err(format!(
            "max_indexed must be between 1 and {MAX_INDEXED_LIMIT}, got {}",
            opts.max_indexed
        ));
    }
    let delim = delim_byte(&opts.delimiter)?;
    let sep = if opts.value_separator.is_empty() {
        "|"
    } else {
        opts.value_separator.as_str()
    };

    let (entries, order) = parse_entries(ldif, opts.decode_base64)?;
    let specs = resolve_columns(opts, &order)?;

    // In indexed mode each attribute widens to the largest occurrence count
    // seen for it (capped); the capped column absorbs any overflow so no value
    // is silently dropped.
    let cap = opts.max_indexed as usize;
    let widths: Vec<usize> = specs
        .iter()
        .map(|s| match s {
            ColSpec::Attr(_, folded) if opts.multi_value == MultiValue::Indexed => entries
                .iter()
                .map(|e| e.get(folded).len())
                .max()
                .unwrap_or(1)
                .clamp(1, cap),
            _ => 1,
        })
        .collect();

    let mut header: Vec<String> = Vec::new();
    for (spec, &w) in specs.iter().zip(widths.iter()) {
        match spec {
            ColSpec::Dn => header.push("dn".to_string()),
            ColSpec::ChangeType => header.push("changetype".to_string()),
            ColSpec::Attr(name, _) => {
                for i in 0..w {
                    header.push(if i == 0 {
                        name.clone()
                    } else {
                        format!("{name}.{}", i + 1)
                    });
                }
            }
        }
    }

    let mut wtr = csv::WriterBuilder::new()
        .delimiter(delim)
        .terminator(csv::Terminator::Any(b'\n'))
        .from_writer(Vec::new());
    wtr.write_record(&header)
        .map_err(|e| format!("CSV write error: {e}"))?;

    for entry in &entries {
        let mut row: Vec<String> = Vec::with_capacity(header.len());
        for (spec, &w) in specs.iter().zip(widths.iter()) {
            match spec {
                ColSpec::Dn => row.push(entry.dn.clone()),
                ColSpec::ChangeType => row.push(entry.changetype.clone()),
                ColSpec::Attr(_, folded) => {
                    let vals = entry.get(folded);
                    match opts.multi_value {
                        MultiValue::Join => row.push(vals.join(sep)),
                        MultiValue::First => row.push(vals.first().cloned().unwrap_or_default()),
                        MultiValue::Last => row.push(vals.last().cloned().unwrap_or_default()),
                        MultiValue::Indexed => {
                            for i in 0..w {
                                if i + 1 == w && vals.len() > w {
                                    // Last column absorbs the overflow.
                                    row.push(vals[i..].join(sep));
                                } else {
                                    row.push(vals.get(i).cloned().unwrap_or_default());
                                }
                            }
                        }
                    }
                }
            }
        }
        wtr.write_record(&row)
            .map_err(|e| format!("CSV write error: {e}"))?;
    }

    let bytes = wtr
        .into_inner()
        .map_err(|e| format!("CSV flush error: {e}"))?;
    let out = String::from_utf8(bytes).map_err(|e| format!("output is not valid UTF-8: {e}"))?;
    Ok(out.trim_end_matches('\n').to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "dn: uid=ada,ou=people,dc=example,dc=com\n\
objectClass: top\n\
objectClass: person\n\
cn: Ada Lovelace\n\
mail: ada@example.com\n\
\n\
dn: uid=bo,ou=people,dc=example,dc=com\n\
objectClass: top\n\
cn: Bo Diaz\n\
mail: bo@example.com\n";

    fn opts() -> Options {
        Options::default()
    }

    #[test]
    fn happy_two_entries_joined_multivalue() {
        let csv = to_csv(SAMPLE, &opts()).unwrap();
        assert_eq!(
            csv,
            "dn,objectClass,cn,mail\n\
\"uid=ada,ou=people,dc=example,dc=com\",top|person,Ada Lovelace,ada@example.com\n\
\"uid=bo,ou=people,dc=example,dc=com\",top,Bo Diaz,bo@example.com"
        );
    }

    #[test]
    fn error_on_empty_input() {
        let err = to_csv("   \n\n", &opts()).unwrap_err();
        assert!(err.contains("no LDIF text supplied"), "{err}");
    }

    #[test]
    fn error_when_no_dn_line() {
        let err = to_csv("cn: Ada\nmail: ada@example.com\n", &opts()).unwrap_err();
        assert!(err.contains("must start with a 'dn:' line"), "{err}");
    }

    #[test]
    fn error_on_line_without_colon() {
        let err = to_csv("dn: cn=x\nnot an ldif line\n", &opts()).unwrap_err();
        assert!(err.contains("expected 'attribute: value'"), "{err}");
    }

    #[test]
    fn folded_continuation_lines_are_rejoined() {
        let ldif = "dn: cn=Long Name,dc=exam\n ple,dc=com\ndescription: a very long des\n cription that was\n  wrapped\n";
        let csv = to_csv(ldif, &opts()).unwrap();
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines[0], "dn,description");
        // The second continuation begins with two spaces: one is the fold
        // marker, the other is real content.
        assert_eq!(
            lines[1],
            "\"cn=Long Name,dc=example,dc=com\",a very long description that was wrapped"
        );
    }

    #[test]
    fn version_header_and_comments_are_ignored() {
        let ldif = "version: 1\n# exported 2026-08-08\n# by an admin\ndn: cn=x\ncn: x\n";
        let csv = to_csv(ldif, &opts()).unwrap();
        assert_eq!(csv, "dn,cn\ncn=x,x");
    }

    #[test]
    fn base64_text_value_is_decoded() {
        // "Café" base64-encoded, plus a base64 DN.
        let ldif = "dn:: Y249Q2Fmw6k=\ncn:: Q2Fmw6k=\n";
        let csv = to_csv(ldif, &opts()).unwrap();
        assert_eq!(csv, "dn,cn\ncn=Café,Café");
    }

    #[test]
    fn binary_base64_value_stays_base64() {
        // 0xFF 0xFE is not valid UTF-8 — keep the original base64 text.
        let ldif = "dn: cn=x\njpegPhoto:: //4=\n";
        let csv = to_csv(ldif, &opts()).unwrap();
        assert_eq!(csv, "dn,jpegPhoto\ncn=x,//4=");
    }

    #[test]
    fn decode_base64_off_keeps_encoded_text() {
        let ldif = "dn: cn=x\ncn:: Q2Fmw6k=\n";
        let o = Options {
            decode_base64: false,
            ..opts()
        };
        let csv = to_csv(ldif, &o).unwrap();
        assert_eq!(csv, "dn,cn\ncn=x,Q2Fmw6k=");
    }

    #[test]
    fn invalid_base64_is_an_error() {
        let err = to_csv("dn: cn=x\ncn:: not!base64!\n", &opts()).unwrap_err();
        assert!(err.contains("invalid base64"), "{err}");
    }

    #[test]
    fn url_reference_value_is_kept_as_text() {
        let ldif = "dn: cn=x\njpegPhoto:< file:///tmp/photo.jpg\n";
        let csv = to_csv(ldif, &opts()).unwrap();
        assert_eq!(csv, "dn,jpegPhoto\ncn=x,file:///tmp/photo.jpg");
    }

    #[test]
    fn missing_and_empty_values() {
        let ldif = "dn: cn=a\ncn: a\nmail:\n\ndn: cn=b\ncn: b\n";
        let csv = to_csv(ldif, &opts()).unwrap();
        assert_eq!(csv, "dn,cn,mail\ncn=a,a,\ncn=b,b,");
    }

    #[test]
    fn column_order_is_stable_across_entries() {
        let ldif = "dn: cn=a\nsn: A\ncn: a\n\ndn: cn=b\ncn: b\nmail: b@x\n";
        let csv = to_csv(ldif, &opts()).unwrap();
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines[0], "dn,sn,cn,mail");
        assert_eq!(lines[1], "cn=a,A,a,");
        assert_eq!(lines[2], "cn=b,,b,b@x");
    }

    #[test]
    fn attribute_names_are_case_folded_into_one_column() {
        let ldif = "dn: cn=a\ngivenName: Ada\n\ndn: cn=b\ngivenname: Bo\n";
        let csv = to_csv(ldif, &opts()).unwrap();
        assert_eq!(csv, "dn,givenName\ncn=a,Ada\ncn=b,Bo");
    }

    #[test]
    fn multi_value_indexed_expands_columns() {
        let o = Options {
            multi_value: MultiValue::Indexed,
            ..opts()
        };
        let csv = to_csv(SAMPLE, &o).unwrap();
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines[0], "dn,objectClass,objectClass.2,cn,mail");
        assert_eq!(
            lines[1],
            "\"uid=ada,ou=people,dc=example,dc=com\",top,person,Ada Lovelace,ada@example.com"
        );
        assert_eq!(
            lines[2],
            "\"uid=bo,ou=people,dc=example,dc=com\",top,,Bo Diaz,bo@example.com"
        );
    }

    #[test]
    fn multi_value_first_and_last() {
        let first = to_csv(
            SAMPLE,
            &Options {
                multi_value: MultiValue::First,
                ..opts()
            },
        )
        .unwrap();
        assert!(first.lines().nth(1).unwrap().contains(",top,"), "{first}");
        let last = to_csv(
            SAMPLE,
            &Options {
                multi_value: MultiValue::Last,
                ..opts()
            },
        )
        .unwrap();
        assert!(last.lines().nth(1).unwrap().contains(",person,"), "{last}");
    }

    #[test]
    fn custom_value_separator() {
        let o = Options {
            value_separator: "; ".to_string(),
            ..opts()
        };
        let csv = to_csv(SAMPLE, &o).unwrap();
        assert!(csv.contains("top; person"), "{csv}");
    }

    #[test]
    fn indexed_overflow_is_joined_into_the_last_column() {
        let ldif = "dn: cn=x\nmemberOf: a\nmemberOf: b\nmemberOf: c\n";
        let o = Options {
            multi_value: MultiValue::Indexed,
            max_indexed: 2,
            ..opts()
        };
        let csv = to_csv(ldif, &o).unwrap();
        assert_eq!(csv, "dn,memberOf,memberOf.2\ncn=x,a,b|c");
    }

    #[test]
    fn max_indexed_at_the_cap_is_accepted() {
        let o = Options {
            multi_value: MultiValue::Indexed,
            max_indexed: MAX_INDEXED_LIMIT,
            ..opts()
        };
        assert!(to_csv(SAMPLE, &o).is_ok());
    }

    #[test]
    fn max_indexed_over_the_cap_is_an_error() {
        let o = Options {
            max_indexed: MAX_INDEXED_LIMIT + 1,
            ..opts()
        };
        let err = to_csv(SAMPLE, &o).unwrap_err();
        assert!(
            err.contains("max_indexed must be between 1 and 50"),
            "{err}"
        );
    }

    #[test]
    fn max_indexed_below_one_is_an_error() {
        let o = Options {
            max_indexed: 0,
            ..opts()
        };
        assert!(to_csv(SAMPLE, &o).unwrap_err().contains("max_indexed"));
    }

    #[test]
    fn columns_selection_orders_and_pads() {
        let o = Options {
            columns: "mail, cn, telephoneNumber".to_string(),
            ..opts()
        };
        let csv = to_csv(SAMPLE, &o).unwrap();
        let lines: Vec<&str> = csv.lines().collect();
        assert_eq!(lines[0], "dn,mail,cn,telephoneNumber");
        assert_eq!(
            lines[1],
            "\"uid=ada,ou=people,dc=example,dc=com\",ada@example.com,Ada Lovelace,"
        );
    }

    #[test]
    fn columns_may_place_dn_explicitly() {
        let o = Options {
            columns: "cn,dn".to_string(),
            ..opts()
        };
        let csv = to_csv(SAMPLE, &o).unwrap();
        assert_eq!(csv.lines().next().unwrap(), "cn,dn");
    }

    #[test]
    fn include_dn_off_drops_the_dn_column() {
        let o = Options {
            include_dn: false,
            columns: "cn".to_string(),
            ..opts()
        };
        let csv = to_csv(SAMPLE, &o).unwrap();
        assert_eq!(csv, "cn\nAda Lovelace\nBo Diaz");
    }

    #[test]
    fn no_columns_at_all_is_an_error() {
        let o = Options {
            include_dn: false,
            ..opts()
        };
        let err = to_csv("dn: cn=x\n", &o).unwrap_err();
        assert!(err.contains("no columns to output"), "{err}");
    }

    #[test]
    fn change_records_skip_modification_directives() {
        let ldif = "version: 1\n\
dn: uid=ada,dc=example,dc=com\n\
changetype: modify\n\
replace: mail\n\
mail: ada@new.example.com\n\
-\n\
add: description\n\
description: updated\n\
-\n";
        let o = Options {
            include_changetype: true,
            ..opts()
        };
        let csv = to_csv(ldif, &o).unwrap();
        assert_eq!(
            csv,
            "dn,changetype,mail,description\n\
\"uid=ada,dc=example,dc=com\",modify,ada@new.example.com,updated"
        );
    }

    #[test]
    fn changetype_column_is_off_by_default() {
        let ldif = "dn: cn=x\nchangetype: add\ncn: x\n";
        let csv = to_csv(ldif, &opts()).unwrap();
        assert_eq!(csv, "dn,cn\ncn=x,x");
    }

    #[test]
    fn control_lines_are_ignored() {
        let ldif = "dn: cn=x\ncontrol: 1.2.840.113556.1.4.805 true\nchangetype: delete\n";
        let o = Options {
            include_changetype: true,
            ..opts()
        };
        let csv = to_csv(ldif, &o).unwrap();
        assert_eq!(csv, "dn,changetype\ncn=x,delete");
    }

    #[test]
    fn values_with_delimiter_quote_or_newline_are_quoted() {
        // The newline arrives via a base64 value, which is how LDIF carries it.
        let ldif = "dn: cn=x\ncn: Doe, John\ndescription: he said \"hi\"\nnote:: YQpi\n";
        let csv = to_csv(ldif, &opts()).unwrap();
        assert_eq!(
            csv,
            "dn,cn,description,note\ncn=x,\"Doe, John\",\"he said \"\"hi\"\"\",\"a\nb\""
        );
    }

    #[test]
    fn tab_delimiter_option() {
        let o = Options {
            delimiter: "tab".to_string(),
            columns: "cn".to_string(),
            ..opts()
        };
        let csv = to_csv(SAMPLE, &o).unwrap();
        assert_eq!(csv.lines().next().unwrap(), "dn\tcn");
    }

    #[test]
    fn multi_char_delimiter_is_an_error() {
        let o = Options {
            delimiter: "::".to_string(),
            ..opts()
        };
        let err = to_csv(SAMPLE, &o).unwrap_err();
        assert!(err.contains("delimiter must be a single char"), "{err}");
    }

    #[test]
    fn multi_value_parse_rejects_unknown_policy() {
        let err = MultiValue::parse("sideways").unwrap_err();
        assert!(err.contains("join/indexed/first/last"), "{err}");
    }

    #[test]
    fn multi_value_parse_accepts_known_policies() {
        assert_eq!(MultiValue::parse("").unwrap(), MultiValue::Join);
        assert_eq!(MultiValue::parse("Indexed").unwrap(), MultiValue::Indexed);
        assert_eq!(MultiValue::parse(" first ").unwrap(), MultiValue::First);
        assert_eq!(MultiValue::parse("LAST").unwrap(), MultiValue::Last);
    }

    #[test]
    fn crlf_input_and_missing_trailing_blank_line() {
        let ldif = "dn: cn=a\r\ncn: a\r\n\r\ndn: cn=b\r\ncn: b";
        let csv = to_csv(ldif, &opts()).unwrap();
        assert_eq!(csv, "dn,cn\ncn=a,a\ncn=b,b");
    }

    #[test]
    fn entries_may_be_separated_by_several_blank_lines() {
        let ldif = "dn: cn=a\ncn: a\n\n\n\ndn: cn=b\ncn: b\n";
        let csv = to_csv(ldif, &opts()).unwrap();
        assert_eq!(csv, "dn,cn\ncn=a,a\ncn=b,b");
    }

    #[test]
    fn consecutive_dn_lines_start_new_entries_without_a_blank_line() {
        let ldif = "dn: cn=a\ncn: a\ndn: cn=b\ncn: b\n";
        let csv = to_csv(ldif, &opts()).unwrap();
        assert_eq!(csv, "dn,cn\ncn=a,a\ncn=b,b");
    }
}
