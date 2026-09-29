//! Turn a code template into a Visual Studio Code user-snippet JSON entry.
//!
//! Two escaping layers have to be right at the same time, and getting either wrong is what
//! makes hand-written snippets misbehave:
//!
//! 1. **Snippet grammar.** Inside a snippet body a bare `$` is a tabstop/variable sigil. Text
//!    that means a literal dollar has to be written `\$` — but `$1`, `${1:label}`,
//!    `${1|a,b|}`, `$TM_FILENAME` and `${TM_FILENAME/(.*)\..+$/$1/}` must survive untouched.
//!    The `auto` mode here parses each construct and escapes only what is *not* one.
//! 2. **JSON.** The result then lives inside a JSON string, so quotes, backslashes, tabs and
//!    control characters need JSON escaping on top.
//!
//! Everything is deterministic and allocation-only — no clock, no I/O — so the same code backs
//! the chat block, the CLI and the browser page.

use std::fmt::Write as _;

/// Largest template accepted, in bytes.
pub const MAX_INPUT_BYTES: usize = 200_000;
/// Largest number of body lines accepted.
pub const MAX_LINES: usize = 5_000;
/// Largest indent width accepted, in spaces per level.
pub const MAX_TAB_SIZE: usize = 8;
/// Largest JSON indent accepted, in spaces per level.
pub const MAX_JSON_INDENT: usize = 8;

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

/// Whether to emit a whole snippets file or a single pasteable entry.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Output {
    /// A complete `{ "Name": { … } }` object, ready to save as `<language>.json`
    /// or `<name>.code-snippets`.
    SnippetsFile,
    /// Just `"Name": { … }`, to paste inside a snippets file that already exists.
    Entry,
}

/// How literal `$` characters in the template are treated.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Dollars {
    /// Keep every valid snippet construct; escape any other `$` as `\$`.
    Auto,
    /// The template is plain text: escape every `\` and every `$`.
    Literal,
    /// Pass the template through byte-for-byte; you own the escaping.
    Raw,
}

/// How leading indentation is rewritten.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Indent {
    /// Leave leading whitespace exactly as pasted.
    Keep,
    /// Convert each group of `tab_size` leading spaces to one tab.
    Tabs,
    /// Convert each leading tab to `tab_size` spaces.
    Spaces,
}

fn parse_output(s: &str) -> Result<Output, String> {
    match s.trim() {
        "" | "snippets-file" => Ok(Output::SnippetsFile),
        "entry" => Ok(Output::Entry),
        other => Err(format!(
            "output must be `snippets-file` or `entry`, got `{other}`"
        )),
    }
}

fn parse_dollars(s: &str) -> Result<Dollars, String> {
    match s.trim() {
        "" | "auto" => Ok(Dollars::Auto),
        "literal" => Ok(Dollars::Literal),
        "raw" => Ok(Dollars::Raw),
        other => Err(format!(
            "dollars must be `auto`, `literal` or `raw`, got `{other}`"
        )),
    }
}

fn parse_indent(s: &str) -> Result<Indent, String> {
    match s.trim() {
        "" | "keep" => Ok(Indent::Keep),
        "tabs" => Ok(Indent::Tabs),
        "spaces" => Ok(Indent::Spaces),
        other => Err(format!(
            "indent must be `keep`, `tabs` or `spaces`, got `{other}`"
        )),
    }
}

fn parse_size(name: &str, value: f64, max: usize) -> Result<usize, String> {
    if !value.is_finite() || value.fract() != 0.0 {
        return Err(format!("{name} must be a whole number, got `{value}`"));
    }
    if value < 0.0 || value > max as f64 {
        return Err(format!("{name} must be between 0 and {max}, got `{value}`"));
    }
    Ok(value as usize)
}

// ---------------------------------------------------------------------------
// Snippet-grammar escaping
// ---------------------------------------------------------------------------

fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}

fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Length in bytes of the `${…}` construct starting at `s[0..2] == "${"`, or `None` when the
/// braces don't form a snippet construct (unbalanced, empty, or a name VS Code wouldn't accept).
///
/// Recognised: `${1}`, `${1:default}`, `${1|a,b|}`, `${1/re/fmt/opts}`, `${VAR}`,
/// `${VAR:fallback}`, `${VAR/re/fmt/opts}`. Nested constructs inside a default are fine, and a
/// backslash escapes the next byte everywhere (that is how a literal `}` or `|` is written).
fn brace_construct_len(s: &str) -> Option<usize> {
    let b = s.as_bytes();
    if b.len() < 3 || b[0] != b'$' || b[1] != b'{' {
        return None;
    }
    let mut i = 2;
    // The name is either all digits (tabstop) or an identifier (variable).
    let name_start = i;
    if b[i].is_ascii_digit() {
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
    } else if is_ident_start(b[i] as char) {
        while i < b.len() && is_ident_char(b[i] as char) {
            i += 1;
        }
    }
    if i == name_start {
        return None; // `${}` / `${:x}` — not a construct.
    }
    if i >= b.len() {
        return None;
    }
    match b[i] {
        b'}' => Some(i + 1),
        // `:` default, `|` choices, `/` transform — all terminate at the matching `}`.
        b':' | b'|' | b'/' => {
            let mut depth = 1usize;
            i += 1;
            while i < b.len() {
                match b[i] {
                    b'\\' => i += 2, // escapes the next byte, whatever it is
                    b'{' => {
                        depth += 1;
                        i += 1;
                    }
                    b'}' => {
                        depth -= 1;
                        i += 1;
                        if depth == 0 {
                            return Some(i);
                        }
                    }
                    _ => i += 1,
                }
            }
            None // never closed
        }
        _ => None,
    }
}

/// Length in bytes of a bare `$1` / `$TM_FILENAME` starting at `s[0] == '$'`, or `None`.
fn bare_construct_len(s: &str) -> Option<usize> {
    let b = s.as_bytes();
    if b.is_empty() || b[0] != b'$' || b.len() < 2 {
        return None;
    }
    let mut i = 1;
    if b[i].is_ascii_digit() {
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        Some(i)
    } else if is_ident_start(b[i] as char) {
        while i < b.len() && is_ident_char(b[i] as char) {
            i += 1;
        }
        Some(i)
    } else {
        None
    }
}

/// `auto`: keep every valid snippet construct, escape every other `$` as `\$`.
///
/// A backslash is copied together with the byte it escapes, so text the user already escaped
/// (`\$`, `\}`, `\\`) is left alone rather than double-escaped.
fn escape_stray_dollars(line: &str) -> String {
    let mut out = String::with_capacity(line.len() + 8);
    let mut rest = line;
    while let Some(pos) = rest.find(['\\', '$']) {
        out.push_str(&rest[..pos]);
        rest = &rest[pos..];
        if rest.starts_with('\\') {
            let mut it = rest.chars();
            out.push('\\');
            it.next();
            if let Some(c) = it.next() {
                out.push(c);
            }
            rest = it.as_str();
            continue;
        }
        let len = brace_construct_len(rest).or_else(|| bare_construct_len(rest));
        match len {
            Some(n) => {
                out.push_str(&rest[..n]);
                rest = &rest[n..];
            }
            None => {
                out.push_str("\\$");
                rest = &rest['$'.len_utf8()..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// `literal`: the template carries no snippet syntax at all.
fn escape_all(line: &str) -> String {
    let mut out = String::with_capacity(line.len() + 8);
    for c in line.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '$' => out.push_str("\\$"),
            _ => out.push(c),
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Indentation
// ---------------------------------------------------------------------------

fn reindent(line: &str, mode: Indent, tab_size: usize) -> String {
    if mode == Indent::Keep || tab_size == 0 {
        return line.to_string();
    }
    let body_at = line
        .find(|c: char| c != ' ' && c != '\t')
        .unwrap_or(line.len());
    let (lead, body) = line.split_at(body_at);
    match mode {
        Indent::Keep => line.to_string(),
        Indent::Spaces => {
            let expanded: String = lead
                .chars()
                .map(|c| {
                    if c == '\t' {
                        " ".repeat(tab_size)
                    } else {
                        " ".to_string()
                    }
                })
                .collect();
            format!("{expanded}{body}")
        }
        Indent::Tabs => {
            // Expand to spaces first so mixed tab/space indentation collapses predictably,
            // then fold each full tab_size run back into one tab.
            let mut columns = 0usize;
            for c in lead.chars() {
                if c == '\t' {
                    columns += tab_size;
                } else {
                    columns += 1;
                }
            }
            let tabs = columns / tab_size;
            let spaces = columns % tab_size;
            format!("{}{}{}", "\t".repeat(tabs), " ".repeat(spaces), body)
        }
    }
}

// ---------------------------------------------------------------------------
// JSON
// ---------------------------------------------------------------------------

/// Escape `s` for use inside a JSON string (the surrounding quotes are not added).
pub fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 || c == '\u{7f}' => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out
}

fn quoted(s: &str) -> String {
    format!("\"{}\"", json_escape(s))
}

/// One `"key": value` pair of the snippet object, already JSON-encoded.
struct Field {
    key: &'static str,
    /// Pre-rendered value: a scalar, or an array whose items are rendered one per line.
    value: Value,
}

enum Value {
    Scalar(String),
    Array(Vec<String>),
}

fn render_object(fields: &[Field], indent: usize, level: usize) -> String {
    let compact = indent == 0;
    let pad = |n: usize| {
        if compact {
            String::new()
        } else {
            " ".repeat(indent * n)
        }
    };
    let nl = if compact { "" } else { "\n" };
    let sep = if compact { "," } else { ",\n" };
    let mut parts: Vec<String> = Vec::with_capacity(fields.len());
    for f in fields {
        let rendered = match &f.value {
            Value::Scalar(v) => format!("{}{}: {}", pad(level + 1), quoted(f.key), v),
            Value::Array(items) => {
                if compact {
                    format!("{}: [{}]", quoted(f.key), items.join(","))
                } else {
                    let inner = items
                        .iter()
                        .map(|i| format!("{}{}", pad(level + 2), i))
                        .collect::<Vec<_>>()
                        .join(",\n");
                    format!(
                        "{}{}: [\n{}\n{}]",
                        pad(level + 1),
                        quoted(f.key),
                        inner,
                        pad(level + 1)
                    )
                }
            }
        };
        parts.push(rendered);
    }
    format!("{{{nl}{}{nl}{}}}", parts.join(sep), pad(level))
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn split_list(s: &str) -> Vec<String> {
    s.split(',')
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .map(|p| p.to_string())
        .collect()
}

/// Derive a plausible trigger word from a snippet name: lowercase, non-alphanumerics folded to
/// a single `-`, e.g. "React function component" → "react-function-component".
fn slugify(name: &str) -> String {
    let mut out = String::with_capacity(name.len());
    let mut pending_sep = false;
    for c in name.chars() {
        if c.is_ascii_alphanumeric() {
            if pending_sep && !out.is_empty() {
                out.push('-');
            }
            pending_sep = false;
            out.push(c.to_ascii_lowercase());
        } else {
            pending_sep = true;
        }
    }
    out
}

/// Build the VS Code snippet JSON for one template.
///
/// `template` is the code body; the remaining arguments mirror the tool's parameters. Returns
/// the JSON text, or a message naming what was expected and what arrived.
#[allow(clippy::too_many_arguments)]
pub fn run(
    template: &str,
    name: &str,
    prefix: &str,
    description: &str,
    scope: &str,
    output: &str,
    dollars: &str,
    indent: &str,
    tab_size: f64,
    final_tabstop: bool,
    is_file_template: bool,
    json_indent: f64,
) -> Result<String, String> {
    if template.trim().is_empty() {
        return Err("template is empty — paste the code the snippet should insert".into());
    }
    if template.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "template is {} bytes, over the {MAX_INPUT_BYTES} byte limit — split it into smaller snippets",
            template.len()
        ));
    }

    let output = parse_output(output)?;
    let dollars = parse_dollars(dollars)?;
    let indent_mode = parse_indent(indent)?;
    let tab_size = parse_size("tab_size", tab_size, MAX_TAB_SIZE)?;
    let tab_size = if tab_size == 0 { 2 } else { tab_size };
    let json_indent = parse_size("json_indent", json_indent, MAX_JSON_INDENT)?;

    let triggers = split_list(prefix);
    let name_trimmed = name.trim();
    let snippet_name = if !name_trimmed.is_empty() {
        name_trimmed.to_string()
    } else if let Some(first) = triggers.first() {
        first.clone()
    } else {
        return Err(
            "give the snippet a name or a prefix — name is the key in the snippets file, prefix is the word you type to trigger it"
                .into(),
        );
    };
    let triggers = if triggers.is_empty() {
        let derived = slugify(&snippet_name);
        if derived.is_empty() {
            return Err(format!(
                "no prefix given and none could be derived from the name `{snippet_name}` — set prefix to the word you want to type"
            ));
        }
        vec![derived]
    } else {
        triggers
    };

    // Normalise line endings, then process line by line.
    let normalised = template.replace("\r\n", "\n").replace('\r', "\n");
    let raw_lines: Vec<&str> = normalised.split('\n').collect();
    if raw_lines.len() > MAX_LINES {
        return Err(format!(
            "template has {} lines, over the {MAX_LINES} line limit",
            raw_lines.len()
        ));
    }

    let mut body: Vec<String> = raw_lines
        .iter()
        .map(|l| {
            let reindented = reindent(l, indent_mode, tab_size);
            match dollars {
                Dollars::Auto => escape_stray_dollars(&reindented),
                Dollars::Literal => escape_all(&reindented),
                Dollars::Raw => reindented,
            }
        })
        .collect();

    // A trailing newline in the pasted text would otherwise add an empty last line that VS Code
    // inserts as a blank line; drop exactly one.
    if body.len() > 1 && body.last().map(|l| l.is_empty()).unwrap_or(false) {
        body.pop();
    }

    if final_tabstop && !body.iter().any(|l| l.contains("$0") || l.contains("${0")) {
        if let Some(last) = body.last_mut() {
            last.push_str("$0");
        }
    }

    let mut fields: Vec<Field> = Vec::with_capacity(5);
    fields.push(Field {
        key: "prefix",
        value: if triggers.len() == 1 {
            Value::Scalar(quoted(&triggers[0]))
        } else {
            Value::Array(triggers.iter().map(|t| quoted(t)).collect())
        },
    });
    fields.push(Field {
        key: "body",
        value: Value::Array(body.iter().map(|l| quoted(l)).collect()),
    });
    let description = description.trim();
    if !description.is_empty() {
        fields.push(Field {
            key: "description",
            value: Value::Scalar(quoted(description)),
        });
    }
    let scopes = split_list(scope);
    if !scopes.is_empty() {
        fields.push(Field {
            key: "scope",
            value: Value::Scalar(quoted(&scopes.join(","))),
        });
    }
    if is_file_template {
        fields.push(Field {
            key: "isFileTemplate",
            value: Value::Scalar("true".into()),
        });
    }

    let level = match output {
        Output::SnippetsFile => 1,
        Output::Entry => 0,
    };
    let obj = render_object(&fields, json_indent, level);
    Ok(match output {
        Output::Entry => format!("{}: {}", quoted(&snippet_name), obj),
        Output::SnippetsFile => {
            if json_indent == 0 {
                format!("{{{}: {}}}", quoted(&snippet_name), obj)
            } else {
                format!(
                    "{{\n{}{}: {}\n}}",
                    " ".repeat(json_indent),
                    quoted(&snippet_name),
                    obj
                )
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn simple(template: &str) -> String {
        run(
            template, "Log", "log", "", "", "snippets-file", "auto", "keep", 2.0, false, false, 2.0,
        )
        .unwrap()
    }

    #[test]
    fn happy_path_single_line() {
        let out = simple("console.log($1);");
        assert_eq!(
            out,
            "{\n  \"Log\": {\n    \"prefix\": \"log\",\n    \"body\": [\n      \"console.log($1);\"\n    ]\n  }\n}"
        );
    }

    #[test]
    fn body_is_one_array_item_per_line() {
        let out = simple("a\nb\nc");
        assert!(out.contains("\"a\",\n      \"b\",\n      \"c\""), "{out}");
    }

    #[test]
    fn trailing_newline_does_not_add_a_blank_line() {
        let out = simple("a\nb\n");
        assert!(!out.contains("\"\""), "{out}");
    }

    #[test]
    fn json_special_characters_are_escaped() {
        let out = simple("say(\"hi\\there\");");
        assert!(out.contains(r#"say(\"hi\\there\");"#), "{out}");
    }

    #[test]
    fn tabs_become_escaped_tabs_in_json() {
        let out = simple("if (x) {\n\treturn 1;\n}");
        assert!(out.contains(r#""\treturn 1;""#), "{out}");
    }

    #[test]
    fn tabstops_placeholders_choices_and_variables_survive_auto() {
        let out = simple("${1:name} $2 ${3|a,b,c|} $TM_FILENAME ${TM_FILENAME/(.*)\\..+$/$1/} $0");
        assert!(out.contains("${1:name}"), "{out}");
        assert!(out.contains("$2"), "{out}");
        assert!(out.contains("${3|a,b,c|}"), "{out}");
        assert!(out.contains("$TM_FILENAME"), "{out}");
        // The transform's own `$1` and its regex `$` anchor both stay intact.
        assert!(out.contains(r#"${TM_FILENAME/(.*)\\..+$/$1/}"#), "{out}");
        assert!(out.contains("$0"), "{out}");
    }

    #[test]
    fn stray_dollars_are_escaped_in_auto() {
        // Shell interpolation, a currency amount, and jQuery all mean a literal dollar.
        let out = simple("echo \"cost: $ 5\"; $(date); $.ajax();");
        assert!(out.contains(r#"cost: \\$ 5"#), "{out}");
        assert!(out.contains(r#"\\$(date)"#), "{out}");
        assert!(out.contains(r#"\\$.ajax()"#), "{out}");
    }

    #[test]
    fn already_escaped_dollar_is_not_double_escaped() {
        let out = simple("price = \\$5");
        // One backslash in the snippet text → `\\` after JSON escaping, not `\\\\`.
        assert!(out.contains(r#"price = \\$5"#), "{out}");
        assert!(!out.contains(r#"\\\\$5"#), "{out}");
    }

    #[test]
    fn unterminated_brace_is_treated_as_a_literal_dollar() {
        let out = simple("${1:oops");
        assert!(out.contains(r#"\\${1:oops"#), "{out}");
    }

    #[test]
    fn literal_mode_escapes_every_dollar_and_backslash() {
        let out = run(
            "$1 costs \\n", "X", "x", "", "", "snippets-file", "literal", "keep", 2.0, false,
            false, 2.0,
        )
        .unwrap();
        assert!(out.contains(r#"\\$1 costs \\\\n"#), "{out}");
    }

    #[test]
    fn raw_mode_passes_dollars_through() {
        let out = run(
            "$(date)", "X", "x", "", "", "snippets-file", "raw", "keep", 2.0, false, false, 2.0,
        )
        .unwrap();
        assert!(out.contains(r#""$(date)""#), "{out}");
    }

    #[test]
    fn indent_spaces_to_tabs_and_back() {
        let to_tabs = run(
            "fn a() {\n    let x = 1;\n}", "X", "x", "", "", "snippets-file", "auto", "tabs", 4.0,
            false, false, 2.0,
        )
        .unwrap();
        assert!(to_tabs.contains(r#""\tlet x = 1;""#), "{to_tabs}");

        let to_spaces = run(
            "fn a() {\n\tlet x = 1;\n}", "X", "x", "", "", "snippets-file", "auto", "spaces", 4.0,
            false, false, 2.0,
        )
        .unwrap();
        assert!(to_spaces.contains(r#""    let x = 1;""#), "{to_spaces}");
    }

    #[test]
    fn indent_conversion_leaves_interior_whitespace_alone() {
        let out = run(
            "    a\tb", "X", "x", "", "", "snippets-file", "auto", "tabs", 4.0, false, false, 2.0,
        )
        .unwrap();
        assert!(out.contains(r#""\ta\tb""#), "{out}");
    }

    #[test]
    fn multiple_triggers_become_an_array() {
        let out = run(
            "x", "X", "log, l , clg", "", "", "snippets-file", "auto", "keep", 2.0, false, false,
            2.0,
        )
        .unwrap();
        assert!(
            out.contains("\"prefix\": [\n      \"log\",\n      \"l\",\n      \"clg\"\n    ]"),
            "{out}"
        );
    }

    #[test]
    fn description_scope_and_file_template_are_emitted_when_set() {
        let out = run(
            "x",
            "X",
            "x",
            "Logs a value",
            "javascript, typescript",
            "snippets-file",
            "auto",
            "keep",
            2.0,
            false,
            true,
            2.0,
        )
        .unwrap();
        assert!(out.contains(r#""description": "Logs a value""#), "{out}");
        assert!(out.contains(r#""scope": "javascript,typescript""#), "{out}");
        assert!(out.contains(r#""isFileTemplate": true"#), "{out}");
    }

    #[test]
    fn optional_keys_are_omitted_when_blank() {
        let out = simple("x");
        assert!(!out.contains("description"), "{out}");
        assert!(!out.contains("scope"), "{out}");
        assert!(!out.contains("isFileTemplate"), "{out}");
    }

    #[test]
    fn entry_output_has_no_wrapping_braces() {
        let out = run(
            "x", "My Snippet", "ms", "", "", "entry", "auto", "keep", 2.0, false, false, 2.0,
        )
        .unwrap();
        assert_eq!(
            out,
            "\"My Snippet\": {\n  \"prefix\": \"ms\",\n  \"body\": [\n    \"x\"\n  ]\n}"
        );
    }

    #[test]
    fn json_indent_zero_is_compact() {
        let out = run(
            "x", "X", "x", "", "", "snippets-file", "auto", "keep", 2.0, false, false, 0.0,
        )
        .unwrap();
        assert_eq!(out, r#"{"X": {"prefix": "x","body": ["x"]}}"#);
    }

    #[test]
    fn json_indent_four_widens_every_level() {
        let out = run(
            "x", "X", "x", "", "", "snippets-file", "auto", "keep", 2.0, false, false, 4.0,
        )
        .unwrap();
        assert_eq!(
            out,
            "{\n    \"X\": {\n        \"prefix\": \"x\",\n        \"body\": [\n            \"x\"\n        ]\n    }\n}"
        );
    }

    #[test]
    fn final_tabstop_is_appended_only_when_missing() {
        let added = run(
            "a\nb", "X", "x", "", "", "snippets-file", "auto", "keep", 2.0, true, false, 2.0,
        )
        .unwrap();
        assert!(added.contains(r#""b$0""#), "{added}");

        let kept = run(
            "a$0\nb", "X", "x", "", "", "snippets-file", "auto", "keep", 2.0, true, false, 2.0,
        )
        .unwrap();
        assert!(kept.contains(r#""a$0""#) && kept.contains(r#""b""#), "{kept}");
    }

    #[test]
    fn name_defaults_to_the_first_trigger() {
        let out = run(
            "x", "", "clog, cl", "", "", "snippets-file", "auto", "keep", 2.0, false, false, 2.0,
        )
        .unwrap();
        assert!(out.starts_with("{\n  \"clog\": {"), "{out}");
    }

    #[test]
    fn prefix_defaults_to_a_slug_of_the_name() {
        let out = run(
            "x",
            "React Function Component",
            "",
            "",
            "",
            "snippets-file",
            "auto",
            "keep",
            2.0,
            false,
            false,
            2.0,
        )
        .unwrap();
        assert!(
            out.contains(r#""prefix": "react-function-component""#),
            "{out}"
        );
    }

    #[test]
    fn control_characters_are_escaped_as_unicode() {
        let out = simple("a\u{1}b");
        assert!(out.contains(r#"a\u0001b"#), "{out}");
    }

    // ---- errors ----

    #[test]
    fn empty_template_is_an_error() {
        let err = run(
            "   ", "X", "x", "", "", "snippets-file", "auto", "keep", 2.0, false, false, 2.0,
        )
        .unwrap_err();
        assert!(err.contains("template is empty"), "{err}");
    }

    #[test]
    fn missing_name_and_prefix_is_an_error() {
        let err = run(
            "x", "", "", "", "", "snippets-file", "auto", "keep", 2.0, false, false, 2.0,
        )
        .unwrap_err();
        assert!(err.contains("name or a prefix"), "{err}");
    }

    #[test]
    fn unknown_enum_values_name_what_was_expected() {
        let err = run(
            "x", "X", "x", "", "", "both", "auto", "keep", 2.0, false, false, 2.0,
        )
        .unwrap_err();
        assert_eq!(
            err,
            "output must be `snippets-file` or `entry`, got `both`"
        );
        let err = run(
            "x", "X", "x", "", "", "snippets-file", "escape", "keep", 2.0, false, false, 2.0,
        )
        .unwrap_err();
        assert!(err.contains("dollars must be"), "{err}");
        let err = run(
            "x", "X", "x", "", "", "snippets-file", "auto", "wide", 2.0, false, false, 2.0,
        )
        .unwrap_err();
        assert!(err.contains("indent must be"), "{err}");
    }

    #[test]
    fn out_of_range_numbers_are_rejected() {
        let err = run(
            "x", "X", "x", "", "", "snippets-file", "auto", "tabs", 99.0, false, false, 2.0,
        )
        .unwrap_err();
        assert!(err.contains("tab_size must be between 0 and 8"), "{err}");
    }

    #[test]
    fn oversized_template_is_rejected() {
        let big = "a\n".repeat(MAX_LINES + 1);
        let err = run(
            &big, "X", "x", "", "", "snippets-file", "auto", "keep", 2.0, false, false, 2.0,
        )
        .unwrap_err();
        assert!(err.contains("line limit"), "{err}");
    }

    #[test]
    fn name_with_no_alphanumerics_and_no_prefix_is_an_error() {
        let err = run(
            "x", "***", "", "", "", "snippets-file", "auto", "keep", 2.0, false, false, 2.0,
        )
        .unwrap_err();
        assert!(err.contains("none could be derived"), "{err}");
    }
}
