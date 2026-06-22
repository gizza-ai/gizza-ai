//! bibtex-format core — validate, sort, and pretty-print BibTeX bibliography
//! entries. No wafer/wasm-bindgen deps; shared by the chat skill block and the
//! web page. Pure Rust, hand-rolled tokenizer (BibTeX is not valid XML/JSON and
//! has no maintained wasm-safe parsing crate worth the risk).
//!
//! Supported top-level items:
//! - `@type{key, field = value, ...}` — a normal entry (article, book, ...).
//! - `@string{name = value}` — a macro/abbreviation definition.
//! - `@preamble{...}` — a LaTeX preamble block.
//! - `@comment{...}` — an explicit comment block.
//! Anything between items (free text / implicit comments) is dropped, matching
//! standard BibTeX behaviour.

/// A single parsed top-level item, in source order.
#[derive(Debug, Clone, PartialEq)]
enum Item {
    /// `@type{key, fields...}`
    Entry {
        kind: String,
        key: String,
        fields: Vec<(String, String)>,
    },
    /// `@string{name = value}`
    StringDef { name: String, value: String },
    /// `@preamble{...}` — the raw inner text.
    Preamble(String),
    /// `@comment{...}` — the raw inner text.
    Comment(String),
}

/// Maximum field indent (spaces) `format` will honour.
pub const MAX_INDENT: u32 = 16;

/// Pretty-print BibTeX source.
///
/// - `sort` (`"none"` | `"key"` | `"type-key"`, blank → `"none"`): reorder
///   `@type` *entries* (string/preamble/comment items keep their relative
///   position at the front). `key` sorts case-insensitively by cite key;
///   `type-key` sorts by entry type then key.
/// - `sort_fields`: when true, sort the fields *within* each entry
///   alphabetically by name (default: keep source order).
/// - `indent`: spaces to indent each field, clamped to `0..=MAX_INDENT`.
/// - `align_values`: when true, pad field names so all `=` signs in an entry
///   line up (default: a single space before `=`).
/// - `lowercase_type`: when true, lowercase the entry type and the
///   `@string`/`@preamble`/`@comment` keyword. Field names are always lowercased.
/// - `check_duplicates`: when true, error if two `@type` entries share a cite
///   key (case-insensitive) — BibTeX silently keeps only the first, so a
///   duplicate key is a real bug. Set false to format anyway.
///
/// Returns `Err` with a human-readable message on a parse error (unbalanced
/// braces, missing `=`, truncated entry, etc.) or a duplicate cite key.
pub fn format(
    input: &str,
    sort: &str,
    sort_fields: bool,
    indent: u32,
    align_values: bool,
    lowercase_type: bool,
    check_duplicates: bool,
) -> Result<String, String> {
    validate_sort(sort)?;
    let indent = indent.min(MAX_INDENT) as usize;
    let mut items = parse(input)?;

    if check_duplicates {
        check_duplicate_keys(&items)?;
    }

    if sort_fields {
        for it in items.iter_mut() {
            if let Item::Entry { fields, .. } = it {
                fields.sort_by(|a, b| a.0.to_lowercase().cmp(&b.0.to_lowercase()));
            }
        }
    }

    sort_items(&mut items, sort);

    let mut out = String::new();
    for (i, it) in items.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        render_item(it, indent, align_values, lowercase_type, &mut out);
    }
    Ok(out)
}

/// Error if two `@type` entries share a cite key (case-insensitive). Names the
/// first key seen twice.
fn check_duplicate_keys(items: &[Item]) -> Result<(), String> {
    let mut seen: Vec<String> = Vec::new();
    for it in items {
        if let Item::Entry { key, .. } = it {
            let k = key.to_lowercase();
            if seen.contains(&k) {
                return Err(format!(
                    "duplicate cite key {key:?}: two entries share this key (BibTeX keeps only the first)"
                ));
            }
            seen.push(k);
        }
    }
    Ok(())
}

fn validate_sort(sort: &str) -> Result<(), String> {
    match sort {
        "" | "none" | "key" | "type-key" => Ok(()),
        other => Err(format!(
            "invalid sort {other:?}: expected \"none\", \"key\", or \"type-key\""
        )),
    }
}

fn sort_items(items: &mut [Item], sort: &str) {
    if sort.is_empty() || sort == "none" {
        return;
    }
    // Stable sort: non-entry items (string/preamble/comment) sort before
    // entries and otherwise keep their order; entries sort among themselves.
    items.sort_by(|a, b| {
        let ra = rank(a);
        let rb = rank(b);
        match (a, b) {
            (
                Item::Entry {
                    kind: ka, key: kka, ..
                },
                Item::Entry {
                    kind: kb, key: kkb, ..
                },
            ) => {
                if sort == "type-key" {
                    ka.to_lowercase()
                        .cmp(&kb.to_lowercase())
                        .then_with(|| kka.to_lowercase().cmp(&kkb.to_lowercase()))
                } else {
                    kka.to_lowercase().cmp(&kkb.to_lowercase())
                }
            }
            _ => ra.cmp(&rb),
        }
    });
}

/// Non-entry items rank 0 (stay at front, order preserved); entries rank 1.
fn rank(it: &Item) -> u8 {
    match it {
        Item::Entry { .. } => 1,
        _ => 0,
    }
}

fn render_item(
    it: &Item,
    indent: usize,
    align_values: bool,
    lowercase_type: bool,
    out: &mut String,
) {
    let pad = " ".repeat(indent);
    match it {
        Item::Entry { kind, key, fields } => {
            let kind = if lowercase_type {
                kind.to_lowercase()
            } else {
                kind.clone()
            };
            out.push('@');
            out.push_str(&kind);
            out.push('{');
            out.push_str(key);
            out.push_str(",\n");
            // Field names are normalized to lowercase always.
            let names: Vec<String> = fields.iter().map(|(n, _)| n.to_lowercase()).collect();
            let width = if align_values {
                names.iter().map(|n| n.len()).max().unwrap_or(0)
            } else {
                0
            };
            for (i, (name, value)) in names.iter().zip(fields.iter().map(|(_, v)| v)).enumerate() {
                out.push_str(&pad);
                out.push_str(name);
                if align_values && name.len() < width {
                    out.push_str(&" ".repeat(width - name.len()));
                }
                out.push_str(" = ");
                out.push_str(value);
                if i + 1 < fields.len() {
                    out.push(',');
                }
                out.push('\n');
            }
            out.push_str("}\n");
        }
        Item::StringDef { name, value } => {
            let kw = if lowercase_type { "string" } else { "STRING" };
            out.push('@');
            out.push_str(kw);
            out.push('{');
            out.push_str(name);
            out.push_str(" = ");
            out.push_str(value);
            out.push_str("}\n");
        }
        Item::Preamble(inner) => {
            let kw = if lowercase_type { "preamble" } else { "PREAMBLE" };
            out.push('@');
            out.push_str(kw);
            out.push('{');
            out.push_str(inner);
            out.push_str("}\n");
        }
        Item::Comment(inner) => {
            let kw = if lowercase_type { "comment" } else { "COMMENT" };
            out.push('@');
            out.push_str(kw);
            out.push('{');
            out.push_str(inner);
            out.push_str("}\n");
        }
    }
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

struct Parser<'a> {
    src: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(s: &'a str) -> Self {
        Parser {
            src: s.as_bytes(),
            pos: 0,
        }
    }

    fn peek(&self) -> Option<u8> {
        self.src.get(self.pos).copied()
    }

    fn bump(&mut self) -> Option<u8> {
        let c = self.peek();
        if c.is_some() {
            self.pos += 1;
        }
        c
    }

    fn skip_ws(&mut self) {
        while let Some(c) = self.peek() {
            if c.is_ascii_whitespace() {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    /// Read an identifier (entry type, field name): up to the next whitespace
    /// or one of the structural chars `{}(),=#`.
    fn read_ident(&mut self) -> String {
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c.is_ascii_whitespace()
                || matches!(c, b'{' | b'}' | b'(' | b')' | b',' | b'=' | b'#')
            {
                break;
            }
            self.pos += 1;
        }
        String::from_utf8_lossy(&self.src[start..self.pos]).into_owned()
    }
}

/// The matching close delimiter for an opening `{` or `(`.
fn matching_close(open: u8) -> u8 {
    if open == b'(' {
        b')'
    } else {
        b'}'
    }
}

fn parse(input: &str) -> Result<Vec<Item>, String> {
    let mut p = Parser::new(input);
    let mut items = Vec::new();

    loop {
        // Skip everything until the next '@' (free text between entries is dropped).
        while let Some(c) = p.peek() {
            if c == b'@' {
                break;
            }
            p.pos += 1;
        }
        if p.peek().is_none() {
            break;
        }
        p.bump(); // consume '@'
        p.skip_ws();
        let kind = p.read_ident();
        if kind.is_empty() {
            return Err("found '@' not followed by an entry type".to_string());
        }
        p.skip_ws();
        let open = match p.peek() {
            Some(c @ (b'{' | b'(')) => {
                p.pos += 1;
                c
            }
            _ => {
                return Err(format!(
                    "entry @{kind} must be followed by '{{' or '(' (at byte {})",
                    p.pos
                ))
            }
        };
        let close = matching_close(open);

        match kind.to_lowercase().as_str() {
            "comment" => {
                let inner = read_balanced(&mut p, open, close)?;
                items.push(Item::Comment(inner));
            }
            "preamble" => {
                let inner = read_balanced(&mut p, open, close)?;
                items.push(Item::Preamble(inner.trim().to_string()));
            }
            "string" => {
                let (name, value) = parse_string_def(&mut p, close)?;
                items.push(Item::StringDef { name, value });
            }
            _ => {
                let (key, fields) = parse_entry_body(&mut p, close, &kind)?;
                items.push(Item::Entry { kind, key, fields });
            }
        }
    }

    Ok(items)
}

/// Read the raw inner text of a `{...}`/`(...)` block, tracking nesting, up to
/// the matching close (which is consumed). The opening delimiter is already
/// consumed by the caller.
fn read_balanced(p: &mut Parser, open: u8, close: u8) -> Result<String, String> {
    let start = p.pos;
    let mut depth = 1i32;
    while let Some(c) = p.bump() {
        if c == open {
            depth += 1;
        } else if c == close {
            depth -= 1;
            if depth == 0 {
                let inner = String::from_utf8_lossy(&p.src[start..p.pos - 1]).into_owned();
                return Ok(inner);
            }
        } else if open == b'(' && c == b'{' {
            // Inside a (...) block, braces still nest as their own group.
            let mut bd = 1i32;
            while let Some(c2) = p.bump() {
                if c2 == b'{' {
                    bd += 1;
                } else if c2 == b'}' {
                    bd -= 1;
                    if bd == 0 {
                        break;
                    }
                }
            }
            if bd != 0 {
                return Err("unbalanced '{' inside block".to_string());
            }
        }
    }
    Err("unbalanced braces: entry was not closed".to_string())
}

fn parse_string_def(p: &mut Parser, close: u8) -> Result<(String, String), String> {
    p.skip_ws();
    let name = p.read_ident();
    if name.is_empty() {
        return Err("@string is missing a name".to_string());
    }
    p.skip_ws();
    match p.peek() {
        Some(b'=') => {
            p.pos += 1;
        }
        _ => return Err(format!("@string {name} is missing '='")),
    }
    let value = read_value(p)?;
    p.skip_ws();
    match p.peek() {
        Some(c) if c == close => {
            p.pos += 1;
            Ok((name, value))
        }
        Some(b',') => {
            // Tolerate a trailing comma before the close.
            p.pos += 1;
            p.skip_ws();
            if p.peek() == Some(close) {
                p.pos += 1;
                Ok((name, value))
            } else {
                Err(format!("@string {name} has unexpected content after value"))
            }
        }
        _ => Err(format!("@string {name} was not closed")),
    }
}

fn parse_entry_body(
    p: &mut Parser,
    close: u8,
    kind: &str,
) -> Result<(String, Vec<(String, String)>), String> {
    p.skip_ws();
    // The cite key runs up to the first comma or the close.
    let key_start = p.pos;
    while let Some(c) = p.peek() {
        if c == b',' || c == close {
            break;
        }
        p.pos += 1;
    }
    let key = String::from_utf8_lossy(&p.src[key_start..p.pos])
        .trim()
        .to_string();
    if key.is_empty() {
        return Err(format!("@{kind} entry is missing a cite key"));
    }

    let mut fields = Vec::new();
    loop {
        p.skip_ws();
        match p.peek() {
            Some(c) if c == close => {
                p.pos += 1;
                break;
            }
            Some(b',') => {
                p.pos += 1;
                p.skip_ws();
                // Allow a trailing comma right before the close.
                if p.peek() == Some(close) {
                    p.pos += 1;
                    break;
                }
                if p.peek().is_none() {
                    return Err(format!("@{kind} {key} was not closed"));
                }
                // Read one field.
                let name = p.read_ident();
                if name.is_empty() {
                    return Err(format!("@{kind} {key}: expected a field name"));
                }
                p.skip_ws();
                match p.peek() {
                    Some(b'=') => {
                        p.pos += 1;
                    }
                    _ => return Err(format!("@{kind} {key}: field '{name}' is missing '='")),
                }
                let value = read_value(p)?;
                fields.push((name, value));
            }
            None => return Err(format!("@{kind} {key} was not closed")),
            Some(other) => {
                return Err(format!(
                    "@{kind} {key}: expected ',' or '{}' but found '{}'",
                    close as char, other as char
                ))
            }
        }
    }
    Ok((key, fields))
}

/// Read a field/string value: a `{...}`-braced block, a `"..."` quoted string,
/// a bare number, or a macro name, optionally concatenated with `#`. Returns
/// the normalized source form (re-quoted), preserving delimiters so the
/// round-trip is faithful.
fn read_value(p: &mut Parser) -> Result<String, String> {
    let mut parts: Vec<String> = Vec::new();
    loop {
        p.skip_ws();
        match p.peek() {
            Some(b'{') => {
                p.pos += 1;
                let inner = read_balanced(p, b'{', b'}')?;
                parts.push(format!("{{{inner}}}"));
            }
            Some(b'"') => {
                p.pos += 1;
                let inner = read_quoted(p)?;
                parts.push(format!("\"{inner}\""));
            }
            Some(c) if c.is_ascii_digit() => {
                let start = p.pos;
                while let Some(d) = p.peek() {
                    if d.is_ascii_digit() {
                        p.pos += 1;
                    } else {
                        break;
                    }
                }
                parts.push(String::from_utf8_lossy(&p.src[start..p.pos]).into_owned());
            }
            Some(c) if c.is_ascii_alphabetic() || c == b'_' => {
                // A macro / abbreviation name.
                let name = p.read_ident();
                parts.push(name);
            }
            _ => {
                if parts.is_empty() {
                    return Err("expected a field value".to_string());
                }
                break;
            }
        }
        p.skip_ws();
        if p.peek() == Some(b'#') {
            p.pos += 1;
            continue;
        }
        break;
    }
    Ok(parts.join(" # "))
}

/// Read a `"..."` value body (the opening quote already consumed), allowing
/// `{...}` groups inside (which may contain quotes). Returns the inner text
/// without the surrounding quotes; the closing quote is consumed.
fn read_quoted(p: &mut Parser) -> Result<String, String> {
    let start = p.pos;
    let mut depth = 0i32;
    while let Some(c) = p.bump() {
        match c {
            b'{' => depth += 1,
            b'}' => {
                if depth > 0 {
                    depth -= 1;
                }
            }
            b'"' if depth == 0 => {
                return Ok(String::from_utf8_lossy(&p.src[start..p.pos - 1]).into_owned());
            }
            _ => {}
        }
    }
    Err("unterminated quoted value".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pretty_prints_basic_entry() {
        let input = "@article{key1,title={Hello World},year=2020,author=\"Doe, John\"}";
        let out = format(input, "none", false, 2, false, true, false).unwrap();
        assert_eq!(
            out,
            "@article{key1,\n  title = {Hello World},\n  year = 2020,\n  author = \"Doe, John\"\n}\n"
        );
    }

    #[test]
    fn lowercases_type_and_field_names() {
        let input = "@ARTICLE{k, Title = {X}, YEAR = 1999}";
        let out = format(input, "none", false, 2, false, true, false).unwrap();
        assert!(out.starts_with("@article{k,\n"), "got: {out}");
        assert!(out.contains("  title = {X},\n"), "got: {out}");
        assert!(out.contains("  year = 1999\n"), "got: {out}");
    }

    #[test]
    fn preserves_type_case_when_not_lowercasing() {
        let input = "@ARTICLE{k, title = {X}}";
        let out = format(input, "none", false, 2, false, false, false).unwrap();
        assert!(out.starts_with("@ARTICLE{k,\n"), "got: {out}");
    }

    #[test]
    fn sorts_entries_by_key() {
        let input = "@book{zeta, title={Z}}\n@article{alpha, title={A}}";
        let out = format(input, "key", false, 2, false, true, false).unwrap();
        let a = out.find("alpha").unwrap();
        let z = out.find("zeta").unwrap();
        assert!(a < z, "alpha should come before zeta:\n{out}");
    }

    #[test]
    fn sorts_by_type_then_key() {
        let input = "@book{b1, x={1}}\n@article{a2, x={2}}\n@article{a1, x={3}}";
        let out = format(input, "type-key", false, 2, false, true, false).unwrap();
        let a1 = out.find("a1").unwrap();
        let a2 = out.find("a2").unwrap();
        let b1 = out.find("b1").unwrap();
        assert!(a1 < a2 && a2 < b1, "got order:\n{out}");
    }

    #[test]
    fn sorts_fields_within_entry() {
        let input = "@article{k, year={2020}, author={X}, title={T}}";
        let out = format(input, "none", true, 2, false, true, false).unwrap();
        let author = out.find("author").unwrap();
        let title = out.find("title").unwrap();
        let year = out.find("year").unwrap();
        assert!(author < title && title < year, "got:\n{out}");
    }

    #[test]
    fn aligns_values_when_requested() {
        let input = "@article{k, a={1}, title={2}}";
        let out = format(input, "none", false, 2, true, true, false).unwrap();
        // "a" is padded to the width of "title" (5).
        assert!(out.contains("  a     = {1}"), "got:\n{out}");
        assert!(out.contains("  title = {2}"), "got:\n{out}");
    }

    #[test]
    fn handles_string_def() {
        let input = "@string{ieee = {IEEE}}";
        let out = format(input, "none", false, 2, false, true, false).unwrap();
        assert_eq!(out, "@string{ieee = {IEEE}}\n");
    }

    #[test]
    fn handles_preamble() {
        let input = "@preamble{ \"\\newcommand{\\x}{y}\" }";
        let out = format(input, "none", false, 2, false, true, false).unwrap();
        assert!(out.starts_with("@preamble{"), "got: {out}");
        assert!(out.contains("\\newcommand"), "got: {out}");
    }

    #[test]
    fn handles_comment() {
        let input = "@comment{ this is a note }";
        let out = format(input, "none", false, 2, false, true, false).unwrap();
        assert_eq!(out, "@comment{ this is a note }\n");
    }

    #[test]
    fn preserves_concatenation() {
        let input = "@article{k, author = ieee # \" and others\"}";
        let out = format(input, "none", false, 2, false, true, false).unwrap();
        assert!(out.contains("ieee # \" and others\""), "got:\n{out}");
    }

    #[test]
    fn drops_free_text_between_entries() {
        let input = "junk before @article{k, x={1}} junk after";
        let out = format(input, "none", false, 2, false, true, false).unwrap();
        assert_eq!(out, "@article{k,\n  x = {1}\n}\n");
    }

    #[test]
    fn paren_delimited_entry() {
        let input = "@article(k, title = {Y})";
        let out = format(input, "none", false, 2, false, true, false).unwrap();
        assert_eq!(out, "@article{k,\n  title = {Y}\n}\n");
    }

    #[test]
    fn tolerates_trailing_comma() {
        let input = "@article{k, title = {Y},}";
        let out = format(input, "none", false, 2, false, true, false).unwrap();
        assert_eq!(out, "@article{k,\n  title = {Y}\n}\n");
    }

    #[test]
    fn indent_clamped() {
        let input = "@article{k, x={1}}";
        let out = format(input, "none", false, 1000, false, true, false).unwrap();
        // clamped to MAX_INDENT=16 spaces.
        assert!(
            out.contains(&format!("{}x = {{1}}", " ".repeat(16))),
            "got:\n{out}"
        );
    }

    #[test]
    fn nested_braces_in_value() {
        let input = "@article{k, title = {A {Nested} Title}}";
        let out = format(input, "none", false, 2, false, true, false).unwrap();
        assert!(out.contains("{A {Nested} Title}"), "got:\n{out}");
    }

    #[test]
    fn quoted_value_with_braces() {
        let input = "@article{k, title = \"A {Quoted} Value\"}";
        let out = format(input, "none", false, 2, false, true, false).unwrap();
        assert!(out.contains("\"A {Quoted} Value\""), "got:\n{out}");
    }

    #[test]
    fn error_on_unbalanced_braces() {
        let err = format("@article{k, title = {oops", "none", false, 2, false, true, false).unwrap_err();
        assert!(
            err.to_lowercase().contains("unbalanced") || err.contains("not closed"),
            "got: {err}"
        );
    }

    #[test]
    fn error_on_missing_key() {
        let err = format("@article{, title = {x}}", "none", false, 2, false, true, false).unwrap_err();
        assert!(err.contains("cite key"), "got: {err}");
    }

    #[test]
    fn error_on_missing_equals() {
        let err = format("@article{k, title {x}}", "none", false, 2, false, true, false).unwrap_err();
        assert!(err.contains("missing '='"), "got: {err}");
    }

    #[test]
    fn error_on_invalid_sort() {
        let err = format("@article{k, x={1}}", "alphabetical", false, 2, false, true, false).unwrap_err();
        assert!(err.contains("invalid sort"), "got: {err}");
    }

    #[test]
    fn empty_input_yields_empty() {
        assert_eq!(format("", "none", false, 2, false, true, false).unwrap(), "");
        assert_eq!(format("   \n\n", "none", false, 2, false, true, false).unwrap(), "");
    }

    #[test]
    fn string_defs_stay_before_entries_when_sorting() {
        let input = "@article{z, x={1}}\n@string{m = {M}}\n@article{a, x={2}}";
        let out = format(input, "key", false, 2, false, true, false).unwrap();
        let m = out.find("@string").unwrap();
        let a = out.find("@article{a").unwrap();
        assert!(m < a, "string def should precede sorted entries:\n{out}");
    }

    #[test]
    fn errors_on_duplicate_cite_key() {
        let input = "@article{k, x={1}}\n@book{K, y={2}}";
        // Case-insensitive: k vs K is a duplicate.
        let err = format(input, "none", false, 2, false, true, true).unwrap_err();
        assert!(err.contains("duplicate cite key"), "got: {err}");
        // With the check off, it formats both.
        let out = format(input, "none", false, 2, false, true, false).unwrap();
        assert!(out.contains("@article{k,") && out.contains("@book{K,"), "got:\n{out}");
    }

    #[test]
    fn distinct_keys_pass_duplicate_check() {
        let input = "@article{a, x={1}}\n@book{b, y={2}}";
        let out = format(input, "none", false, 2, false, true, true).unwrap();
        assert!(out.contains("@article{a,") && out.contains("@book{b,"), "got:\n{out}");
    }

    #[test]
    fn multiple_entries_separated_by_blank_line() {
        let input = "@article{a, x={1}}@book{b, y={2}}";
        let out = format(input, "none", false, 2, false, true, false).unwrap();
        // a blank line between the two rendered entries.
        assert!(out.contains("}\n\n@book{b,"), "got:\n{out}");
    }
}
