//! vcard-to-json core — pure compute, shared by the chat skill block and the web
//! page. No wafer/wasm-bindgen deps. Parses vCard (.vcf) contact cards — vCard
//! 3.0 (RFC 2426) and 4.0 (RFC 6350), plus the common 2.1 bare-type form — into a
//! JSON array of contacts. Handles line unfolding, multi-contact files, structured
//! Name (N) and Address (ADR) values, and multi-TYPE parameters.

use serde_json::{json, Map, Value};

/// How to wrap the parsed contacts in the top-level JSON output.
pub enum Wrap {
    /// A bare JSON array of contact objects.
    Array,
    /// An object `{ "contacts": [...], "count": N }`.
    Object,
}

impl Wrap {
    pub fn parse(s: &str) -> Result<Wrap, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "array" => Ok(Wrap::Array),
            "object" => Ok(Wrap::Object),
            other => Err(format!("unknown wrap '{other}' (use 'array' or 'object')")),
        }
    }
}

/// One parsed content line: property NAME (upper-cased, group prefix stripped),
/// params, and the raw (still-escaped) value string.
struct Line {
    name: String,
    params: Vec<(String, String)>, // (key-lowercased, value) — bare 2.1 types stored as ("type", v)
    value: String,
}

/// Parse a vCard document into JSON.
///
/// - `structured`: split N and ADR into their component fields; when false they
///   are returned as raw (unescaped) strings.
/// - `wrap`: bare array vs. `{contacts, count}` object.
/// - `pretty`: indent the JSON output.
pub fn run(input: &str, structured: bool, wrap: Wrap, pretty: bool) -> Result<String, String> {
    let logical = unfold(input);
    let cards = split_cards(&logical)?;
    let contacts: Vec<Value> = cards
        .iter()
        .map(|card| card_to_json(card, structured))
        .collect();

    let root = match wrap {
        Wrap::Array => Value::Array(contacts),
        Wrap::Object => {
            let mut m = Map::new();
            let count = contacts.len();
            m.insert("contacts".into(), Value::Array(contacts));
            m.insert("count".into(), json!(count));
            Value::Object(m)
        }
    };

    let out = if pretty {
        serde_json::to_string_pretty(&root)
    } else {
        serde_json::to_string(&root)
    };
    out.map_err(|e| format!("failed to serialize JSON: {e}"))
}

/// RFC 6350 §3.2 line unfolding: normalize newlines, then a line beginning with a
/// single space or tab is a continuation of the previous line (that one leading
/// whitespace char is removed).
fn unfold(input: &str) -> Vec<String> {
    let normalized = input.replace("\r\n", "\n").replace('\r', "\n");
    let mut out: Vec<String> = Vec::new();
    for raw in normalized.split('\n') {
        if let Some(rest) = raw.strip_prefix(' ').or_else(|| raw.strip_prefix('\t')) {
            if let Some(last) = out.last_mut() {
                last.push_str(rest);
                continue;
            }
        }
        out.push(raw.to_string());
    }
    out
}

/// Split logical lines into cards delimited by BEGIN:VCARD / END:VCARD
/// (case-insensitive). Content lines between the markers belong to the card;
/// lines outside any card are ignored. Errors if no card is found.
fn split_cards(lines: &[String]) -> Result<Vec<Vec<Line>>, String> {
    let mut cards: Vec<Vec<Line>> = Vec::new();
    let mut current: Option<Vec<Line>> = None;
    for raw in lines {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        let upper = trimmed.to_ascii_uppercase();
        if upper == "BEGIN:VCARD" {
            current = Some(Vec::new());
            continue;
        }
        if upper == "END:VCARD" {
            if let Some(card) = current.take() {
                cards.push(card);
            }
            continue;
        }
        if let Some(card) = current.as_mut() {
            if let Some(line) = parse_line(trimmed) {
                card.push(line);
            }
        }
    }
    if cards.is_empty() {
        return Err(
            "no vCard found: expected at least one 'BEGIN:VCARD ... END:VCARD' block".into(),
        );
    }
    Ok(cards)
}

/// Parse one content line `NAME;PARAM=v;TYPE=work:value` (with optional
/// `group.` prefix) into its parts. Returns None for a line with no ':'.
fn parse_line(line: &str) -> Option<Line> {
    let colon = find_unquoted_colon(line)?;
    let (head, rest) = line.split_at(colon);
    let value = rest[1..].to_string(); // skip the ':'

    let mut parts = split_semicolons_quoted(head);
    if parts.is_empty() {
        return None;
    }
    // First part = [group.]NAME ; drop the group prefix, upper-case the name.
    let name_tok = parts.remove(0);
    let name = name_tok
        .rsplit('.')
        .next()
        .unwrap_or(&name_tok)
        .trim()
        .to_ascii_uppercase();
    if name.is_empty() {
        return None;
    }

    let mut params: Vec<(String, String)> = Vec::new();
    for p in parts {
        if let Some(eq) = p.find('=') {
            let key = p[..eq].trim().to_ascii_lowercase();
            let val = &p[eq + 1..];
            for v in split_commas_quoted(val) {
                params.push((key.clone(), strip_quotes(&v).to_string()));
            }
        } else {
            // vCard 2.1 bare type, e.g. TEL;WORK;VOICE:
            let v = strip_quotes(p.trim());
            if !v.is_empty() {
                params.push(("type".into(), v.to_string()));
            }
        }
    }

    Some(Line { name, params, value })
}

/// Collect the TYPE param values (lower-cased) for a line, in order.
fn types_of(line: &Line) -> Vec<Value> {
    line.params
        .iter()
        .filter(|(k, _)| k == "type")
        .flat_map(|(_, v)| v.split(',').map(|s| s.trim().to_ascii_lowercase()))
        .filter(|s| !s.is_empty())
        .map(Value::from)
        .collect()
}

/// Build the JSON object for one card.
fn card_to_json(card: &[Line], structured: bool) -> Value {
    let mut obj = Map::new();
    let mut emails: Vec<Value> = Vec::new();
    let mut phones: Vec<Value> = Vec::new();
    let mut urls: Vec<Value> = Vec::new();
    let mut addresses: Vec<Value> = Vec::new();
    let mut other: Vec<Value> = Vec::new();

    // Order-preserving singular setter (first occurrence wins).
    fn set_once(obj: &mut Map<String, Value>, key: &str, val: Value) {
        obj.entry(key.to_string()).or_insert(val);
    }

    for line in card {
        match line.name.as_str() {
            "VERSION" => set_once(&mut obj, "version", json!(unescape(&line.value))),
            "FN" => set_once(&mut obj, "fn", json!(unescape(&line.value))),
            "N" => {
                let val = if structured {
                    name_object(&line.value)
                } else {
                    json!(unescape(&line.value))
                };
                set_once(&mut obj, "name", val);
            }
            "NICKNAME" => set_once(&mut obj, "nickname", json!(unescape(&line.value))),
            "ORG" => set_once(&mut obj, "org", json!(unescape(&line.value))),
            "TITLE" => set_once(&mut obj, "title", json!(unescape(&line.value))),
            "ROLE" => set_once(&mut obj, "role", json!(unescape(&line.value))),
            "BDAY" => set_once(&mut obj, "birthday", json!(unescape(&line.value))),
            "NOTE" => set_once(&mut obj, "note", json!(unescape(&line.value))),
            "EMAIL" => emails.push(json!({
                "value": unescape(&line.value),
                "types": types_of(line),
            })),
            "TEL" => phones.push(json!({
                "value": unescape(&line.value),
                "types": types_of(line),
            })),
            "URL" => urls.push(json!(unescape(&line.value))),
            "ADR" => {
                let mut a = if structured {
                    address_object(&line.value)
                } else {
                    let mut m = Map::new();
                    m.insert("value".into(), json!(unescape(&line.value)));
                    m
                };
                a.insert("types".into(), Value::Array(types_of(line)));
                addresses.push(Value::Object(a));
            }
            _ => {
                let mut m = Map::new();
                m.insert("property".into(), json!(line.name.to_ascii_lowercase()));
                m.insert("value".into(), json!(unescape(&line.value)));
                let types = types_of(line);
                if !types.is_empty() {
                    m.insert("types".into(), Value::Array(types));
                }
                other.push(Value::Object(m));
            }
        }
    }

    if !emails.is_empty() {
        obj.insert("emails".into(), Value::Array(emails));
    }
    if !phones.is_empty() {
        obj.insert("phones".into(), Value::Array(phones));
    }
    if !urls.is_empty() {
        obj.insert("urls".into(), Value::Array(urls));
    }
    if !addresses.is_empty() {
        obj.insert("addresses".into(), Value::Array(addresses));
    }
    if !other.is_empty() {
        obj.insert("other".into(), Value::Array(other));
    }
    Value::Object(obj)
}

/// Structured N value: Family;Given;Additional;Prefixes;Suffixes.
fn name_object(value: &str) -> Value {
    let parts = split_components(value);
    let get = |i: usize| unescape(parts.get(i).map(|s| s.as_str()).unwrap_or(""));
    json!({
        "family": get(0),
        "given": get(1),
        "middle": get(2),
        "prefix": get(3),
        "suffix": get(4),
    })
}

/// Structured ADR value: PO Box;Extended;Street;Locality;Region;Postal Code;Country.
fn address_object(value: &str) -> Map<String, Value> {
    let parts = split_components(value);
    let get = |i: usize| unescape(parts.get(i).map(|s| s.as_str()).unwrap_or(""));
    let mut m = Map::new();
    m.insert("poBox".into(), json!(get(0)));
    m.insert("extended".into(), json!(get(1)));
    m.insert("street".into(), json!(get(2)));
    m.insert("locality".into(), json!(get(3)));
    m.insert("region".into(), json!(get(4)));
    m.insert("postalCode".into(), json!(get(5)));
    m.insert("country".into(), json!(get(6)));
    m
}

/// Split a structured value on unescaped ';' (backslash escapes a separator).
fn split_components(value: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut chars = value.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            cur.push(c);
            if let Some(n) = chars.next() {
                cur.push(n);
            }
        } else if c == ';' {
            out.push(std::mem::take(&mut cur));
        } else {
            cur.push(c);
        }
    }
    out.push(cur);
    out
}

/// Unescape a vCard TEXT value: `\n`/`\N` → newline, `\,` → `,`, `\;` → `;`,
/// `\\` → `\`. Any other `\x` becomes `x`.
fn unescape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('n') | Some('N') => out.push('\n'),
                Some(other) => out.push(other),
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Find the first ':' not inside a double-quoted param value.
fn find_unquoted_colon(s: &str) -> Option<usize> {
    let mut in_quote = false;
    for (i, c) in s.char_indices() {
        match c {
            '"' => in_quote = !in_quote,
            ':' if !in_quote => return Some(i),
            _ => {}
        }
    }
    None
}

/// Split the head (name+params) on ';' respecting double-quoted param values.
fn split_semicolons_quoted(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_quote = false;
    for c in s.chars() {
        match c {
            '"' => {
                in_quote = !in_quote;
                cur.push(c);
            }
            ';' if !in_quote => out.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
    }
    out.push(cur);
    out
}

/// Split a param value list on ',' respecting double-quoted segments.
fn split_commas_quoted(s: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_quote = false;
    for c in s.chars() {
        match c {
            '"' => {
                in_quote = !in_quote;
                cur.push(c);
            }
            ',' if !in_quote => out.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
    }
    out.push(cur);
    out
}

fn strip_quotes(s: &str) -> &str {
    let t = s.trim();
    if t.len() >= 2 && t.starts_with('"') && t.ends_with('"') {
        &t[1..t.len() - 1]
    } else {
        t
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(input: &str) -> Value {
        let s = run(input, true, Wrap::Array, false).unwrap();
        serde_json::from_str(&s).unwrap()
    }

    #[test]
    fn happy_single_contact() {
        let vcf = "BEGIN:VCARD\r\nVERSION:3.0\r\nFN:John Doe\r\nN:Doe;John;Q;Mr.;Jr.\r\nEMAIL;TYPE=work:john@example.com\r\nTEL;TYPE=CELL,voice:+15551234567\r\nEND:VCARD\r\n";
        let v = parse(vcf);
        assert!(v.is_array());
        let c = &v[0];
        assert_eq!(c["version"], "3.0");
        assert_eq!(c["fn"], "John Doe");
        assert_eq!(c["name"]["family"], "Doe");
        assert_eq!(c["name"]["given"], "John");
        assert_eq!(c["name"]["prefix"], "Mr.");
        assert_eq!(c["name"]["suffix"], "Jr.");
        assert_eq!(c["emails"][0]["value"], "john@example.com");
        assert_eq!(c["emails"][0]["types"][0], "work");
        assert_eq!(c["phones"][0]["value"], "+15551234567");
        assert_eq!(c["phones"][0]["types"], json!(["cell", "voice"]));
    }

    #[test]
    fn multi_contact_and_wrap_object() {
        let vcf = "BEGIN:VCARD\nVERSION:4.0\nFN:A\nEND:VCARD\nBEGIN:VCARD\nVERSION:4.0\nFN:B\nEND:VCARD\n";
        let s = run(vcf, true, Wrap::Object, false).unwrap();
        let v: Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v["count"], 2);
        assert_eq!(v["contacts"][0]["fn"], "A");
        assert_eq!(v["contacts"][1]["fn"], "B");
    }

    #[test]
    fn line_folding_and_escapes() {
        // Folded NOTE continues on the next line; escaped comma/semicolon/newline.
        let vcf = "BEGIN:VCARD\nVERSION:3.0\nFN:Jane\nNOTE:Line one\\nsecond\\, still\n  more text\nEND:VCARD\n";
        let v = parse(vcf);
        assert_eq!(v[0]["note"], "Line one\nsecond, still more text");
    }

    #[test]
    fn structured_address_and_group_and_bare_type() {
        // group prefix (item1.), structured ADR, vCard 2.1 bare type on TEL.
        let vcf = "BEGIN:VCARD\nVERSION:2.1\nFN:Sam\nitem1.ADR;TYPE=home:;;123 Main St;Springfield;IL;62704;USA\nTEL;WORK;VOICE:555\nEND:VCARD\n";
        let v = parse(vcf);
        let adr = &v[0]["addresses"][0];
        assert_eq!(adr["street"], "123 Main St");
        assert_eq!(adr["locality"], "Springfield");
        assert_eq!(adr["region"], "IL");
        assert_eq!(adr["postalCode"], "62704");
        assert_eq!(adr["country"], "USA");
        assert_eq!(adr["types"][0], "home");
        assert_eq!(v[0]["phones"][0]["types"], json!(["work", "voice"]));
    }

    #[test]
    fn unstructured_name_when_structured_off() {
        let vcf = "BEGIN:VCARD\nVERSION:3.0\nN:Doe;John\nFN:John Doe\nEND:VCARD\n";
        let s = run(vcf, false, Wrap::Array, false).unwrap();
        let v: Value = serde_json::from_str(&s).unwrap();
        assert_eq!(v[0]["name"], "Doe;John");
    }

    #[test]
    fn error_no_vcard() {
        let err = run("not a vcard at all", true, Wrap::Array, false).unwrap_err();
        assert!(err.contains("no vCard found"), "got: {err}");
    }
}
