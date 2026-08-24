//! config-file-validator core — syntax-validate pasted JSON, YAML, TOML, INI or
//! XML configuration text and report every problem as a normalised diagnostic
//! with a 1-based line, a 1-based column and a 0-based byte offset.
//!
//! Pure compute, no I/O: the same entry point backs the chat/CLI block and the
//! browser page. Each format is checked by a real parser (serde_json, serde_yml,
//! toml, quick-xml) except INI, which has no single specification and is checked
//! by the hand-rolled line scanner below.

use std::collections::HashSet;
use std::fmt::Write as _;

use serde::Deserialize;
use serde_json::{json, Value};

/// Inputs larger than this are rejected outright — a config file is not a data set.
pub const MAX_INPUT_BYTES: usize = 1_048_576;
/// At most this many diagnostics are reported; the rest are summarised as truncated.
pub const MAX_DIAGNOSTICS: usize = 100;
/// Upper bound for the `context_lines` parameter.
pub const MAX_CONTEXT_LINES: usize = 10;

// ---------------------------------------------------------------------------
// Formats
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Json,
    Yaml,
    Toml,
    Ini,
    Xml,
}

impl Format {
    pub fn id(self) -> &'static str {
        match self {
            Format::Json => "json",
            Format::Yaml => "yaml",
            Format::Toml => "toml",
            Format::Ini => "ini",
            Format::Xml => "xml",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Format::Json => "JSON",
            Format::Yaml => "YAML",
            Format::Toml => "TOML",
            Format::Ini => "INI",
            Format::Xml => "XML",
        }
    }

    fn from_id(s: &str) -> Option<Format> {
        match s.trim().to_ascii_lowercase().as_str() {
            "json" => Some(Format::Json),
            "yaml" | "yml" => Some(Format::Yaml),
            "toml" => Some(Format::Toml),
            "ini" | "conf" | "cfg" => Some(Format::Ini),
            "xml" => Some(Format::Xml),
            _ => None,
        }
    }
}

/// Every format, in the order used to complete a ranked candidate list.
const ALL_FORMATS: [Format; 5] = [
    Format::Json,
    Format::Yaml,
    Format::Toml,
    Format::Ini,
    Format::Xml,
];

// ---------------------------------------------------------------------------
// Diagnostics
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub severity: Severity,
    /// 1-based line, when the parser exposes a position.
    pub line: Option<usize>,
    /// 1-based column counted in characters, when the parser exposes one.
    pub column: Option<usize>,
    /// 0-based byte offset into the input, when known.
    pub offset: Option<usize>,
    pub message: String,
    pub hint: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
}

impl Severity {
    fn id(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
        }
    }
}

impl Diagnostic {
    fn error(
        line: Option<usize>,
        column: Option<usize>,
        offset: Option<usize>,
        message: impl Into<String>,
        hint: Option<String>,
    ) -> Diagnostic {
        Diagnostic {
            severity: Severity::Error,
            line,
            column,
            offset,
            message: message.into(),
            hint,
        }
    }

    fn warning(
        line: usize,
        column: usize,
        offset: usize,
        message: impl Into<String>,
        hint: Option<String>,
    ) -> Diagnostic {
        Diagnostic {
            severity: Severity::Warning,
            line: Some(line),
            column: Some(column),
            offset: Some(offset),
            message: message.into(),
            hint,
        }
    }
}

/// The outcome of checking one candidate format.
#[derive(Debug, Clone)]
pub struct Check {
    pub format: Format,
    pub diagnostics: Vec<Diagnostic>,
}

impl Check {
    fn error_count(&self) -> usize {
        self.diagnostics
            .iter()
            .filter(|d| d.severity == Severity::Error)
            .count()
    }
}

// ---------------------------------------------------------------------------
// Position helpers
// ---------------------------------------------------------------------------

/// Convert a 0-based byte offset into a 1-based (line, column) pair. The column
/// counts characters, so multi-byte text lines up with what an editor shows.
fn line_col(text: &str, offset: usize) -> (usize, usize) {
    let offset = offset.min(text.len());
    let mut line = 1usize;
    let mut col = 1usize;
    for (idx, ch) in text.char_indices() {
        if idx >= offset {
            return (line, col);
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

/// Inverse of [`line_col`] — used for parsers that report a position but no offset.
fn offset_from_line_col(text: &str, line: usize, column: usize) -> usize {
    let mut cur_line = 1usize;
    let mut cur_col = 1usize;
    for (idx, ch) in text.char_indices() {
        if cur_line == line && cur_col == column {
            return idx;
        }
        if ch == '\n' {
            cur_line += 1;
            cur_col = 1;
        } else {
            cur_col += 1;
        }
    }
    text.len()
}

/// Strip the position suffix parsers append to their messages, so every
/// diagnostic renders its position exactly once and in one style.
fn strip_position_suffix(msg: &str) -> String {
    let mut s = msg.trim().to_string();
    for marker in [" at line ", " at position ", " at byte ", " at offset "] {
        if let Some(idx) = s.rfind(marker) {
            let tail = &s[idx + marker.len()..];
            if tail.chars().next().is_some_and(|c| c.is_ascii_digit()) {
                s.truncate(idx);
            }
        }
    }
    // toml prefixes its own banner; the position is re-rendered by the report.
    if let Some(rest) = s.strip_prefix("TOML parse error") {
        s = rest.trim_start_matches([' ', '\n', ':']).to_string();
    }
    s.trim().trim_end_matches(',').to_string()
}

// ---------------------------------------------------------------------------
// Auto-detection
// ---------------------------------------------------------------------------

/// Rank the candidate formats for `text`, most likely first. The caller tries
/// each in turn and keeps the first that parses cleanly, so a wrong guess costs
/// nothing but a second parse.
pub fn rank_formats(text: &str) -> Vec<Format> {
    let mut ranked: Vec<Format> = Vec::new();
    let body = text.trim_start_matches('\u{feff}');
    let first = first_significant_line(body);
    let trimmed = first.trim();

    if trimmed.starts_with("<?xml") || trimmed.starts_with("<!DOCTYPE") || trimmed.starts_with("<!--")
    {
        ranked.push(Format::Xml);
    } else if trimmed.starts_with('<') {
        ranked.push(Format::Xml);
    } else if trimmed.starts_with('{') {
        ranked.push(Format::Json);
        ranked.push(Format::Yaml);
    } else if is_section_header(trimmed) {
        // `[section]` — a TOML table or an INI section, never a JSON array.
        ranked.push(Format::Toml);
        ranked.push(Format::Ini);
    } else if trimmed.starts_with('[') {
        ranked.push(Format::Json);
    } else if trimmed.starts_with("---") || trimmed.starts_with("- ") {
        ranked.push(Format::Yaml);
    } else {
        let (assigns, colons) = assignment_vs_colon_lines(body);
        if assigns > colons {
            ranked.push(Format::Toml);
            ranked.push(Format::Ini);
        } else if colons > 0 {
            ranked.push(Format::Yaml);
        }
    }

    for f in ALL_FORMATS {
        if !ranked.contains(&f) {
            ranked.push(f);
        }
    }
    ranked
}

fn first_significant_line(text: &str) -> &str {
    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') || t.starts_with(';') || t.starts_with("//") {
            continue;
        }
        return line;
    }
    ""
}

fn is_section_header(line: &str) -> bool {
    let l = line.trim();
    if !l.starts_with('[') {
        return false;
    }
    let Some(close) = l.rfind(']') else {
        return false;
    };
    let inner = &l[1..close];
    let rest = l[close + 1..].trim();
    let rest_is_comment = rest.is_empty() || rest.starts_with('#') || rest.starts_with(';');
    // A JSON array of scalars would contain commas or quotes at the top level.
    !inner.is_empty()
        && rest_is_comment
        && !inner.contains(',')
        && !inner.contains(':')
        && !inner.contains('{')
}

/// Count lines that look like `key = value` versus `key: value`, ignoring
/// comments — the cheapest reliable TOML/INI vs YAML signal.
fn assignment_vs_colon_lines(text: &str) -> (usize, usize) {
    let mut assigns = 0usize;
    let mut colons = 0usize;
    for line in text.lines() {
        let t = line.trim();
        if t.is_empty() || t.starts_with('#') || t.starts_with(';') || t.starts_with("//") {
            continue;
        }
        let eq = t.find('=');
        let colon = t.find(": ").or_else(|| {
            if t.ends_with(':') {
                Some(t.len() - 1)
            } else {
                None
            }
        });
        match (eq, colon) {
            (Some(e), Some(c)) => {
                if e < c {
                    assigns += 1
                } else {
                    colons += 1
                }
            }
            (Some(_), None) => assigns += 1,
            (None, Some(_)) => colons += 1,
            (None, None) => {}
        }
    }
    (assigns, colons)
}

// ---------------------------------------------------------------------------
// Per-format checks
// ---------------------------------------------------------------------------

pub fn check(text: &str, format: Format, strict: bool) -> Check {
    let mut diagnostics = match format {
        Format::Json => check_json(text),
        Format::Yaml => check_yaml(text),
        Format::Toml => check_toml(text),
        Format::Ini => check_ini(text, strict),
        Format::Xml => check_xml(text),
    };
    if strict {
        // Portability checks only make sense once the shape is known; they are
        // warnings, never errors, so a strict run can still be "valid".
        if diagnostics.iter().all(|d| d.severity != Severity::Error) {
            if format == Format::Json {
                diagnostics.extend(json_duplicate_keys(text));
            }
            diagnostics.extend(portability_warnings(text, format));
        }
    }
    diagnostics.sort_by_key(|d| (d.line.unwrap_or(usize::MAX), d.column.unwrap_or(0)));
    Check {
        format,
        diagnostics,
    }
}

fn check_json(text: &str) -> Vec<Diagnostic> {
    match serde_json::from_str::<Value>(text) {
        Ok(_) => Vec::new(),
        Err(e) => {
            let (line, column) = (e.line(), e.column());
            let offset = if line > 0 && column > 0 {
                Some(offset_from_line_col(text, line, column))
            } else {
                None
            };
            let msg = strip_position_suffix(&e.to_string());
            let hint = json_hint(&msg);
            vec![Diagnostic::error(
                (line > 0).then_some(line),
                (column > 0).then_some(column),
                offset,
                msg,
                hint,
            )]
        }
    }
}

fn json_hint(msg: &str) -> Option<String> {
    let m = msg.to_ascii_lowercase();
    let hint = if m.contains("trailing comma") {
        "JSON forbids a comma after the last element — delete it."
    } else if m.contains("key must be a string") {
        "Object keys must be wrapped in double quotes: {\"key\": 1}."
    } else if m.contains("trailing characters") {
        "A JSON document holds exactly one top-level value; move the extra content inside it, or use JSON Lines."
    } else if m.contains("eof while parsing") {
        "The document ends before a bracket, brace or quote was closed — check the end of the file."
    } else if m.contains("control character") {
        "Literal tabs and newlines are not allowed inside a JSON string — escape them as \\t and \\n."
    } else if m.contains("invalid escape") {
        "Only \\\" \\\\ \\/ \\b \\f \\n \\r \\t and \\uXXXX are valid escapes; a lone backslash must be written \\\\."
    } else if m.contains("expected `:`") || m.contains("expected ':'") {
        "Every object member needs a colon between key and value."
    } else if m.contains("expected `,`") || m.contains("expected ','") {
        "Separate object members and array elements with commas."
    } else if m.contains("expected value") {
        "Values must be a string, number, object, array, true, false or null — single quotes, bare words and NaN are not JSON."
    } else if m.contains("invalid number") {
        "JSON numbers cannot have a leading +, a leading zero, or a bare leading/trailing dot."
    } else {
        return None;
    };
    Some(hint.to_string())
}

fn check_yaml(text: &str) -> Vec<Diagnostic> {
    // Deserializer::from_str iterates documents, so multi-document configs
    // (`---` separated, as Kubernetes writes them) validate as one input.
    let mut out = Vec::new();
    let mut documents = 0usize;
    for de in serde_yml::Deserializer::from_str(text) {
        documents += 1;
        if let Err(e) = Value::deserialize(de) {
            out.push(yaml_diagnostic(&e));
            break;
        }
    }
    if documents == 0 && !text.trim().is_empty() {
        // A stream the scanner could not open at all.
        if let Err(e) = serde_yml::from_str::<Value>(text) {
            out.push(yaml_diagnostic(&e));
        }
    }
    out
}

fn yaml_diagnostic(e: &serde_yml::Error) -> Diagnostic {
    let msg = strip_position_suffix(&e.to_string());
    let hint = yaml_hint(&msg);
    match e.location() {
        Some(loc) => Diagnostic::error(
            Some(loc.line()),
            Some(loc.column()),
            Some(loc.index()),
            msg,
            hint,
        ),
        None => Diagnostic::error(None, None, None, msg, hint),
    }
}

fn yaml_hint(msg: &str) -> Option<String> {
    let m = msg.to_ascii_lowercase();
    let hint = if m.contains("found character that cannot start any token") {
        "YAML forbids tab characters in indentation — indent with spaces."
    } else if m.contains("mapping values are not allowed") {
        "A plain scalar containing \": \" is read as a new mapping — wrap the whole value in quotes."
    } else if m.contains("did not find expected key") || m.contains("expected <block end>") {
        "Sibling keys must line up in the same column — check for an extra or missing indent."
    } else if m.contains("could not find expected ':'") {
        "Each mapping entry needs a colon and a space after the key: `key: value`."
    } else if m.contains("unexpected end of stream") || m.contains("while parsing a quoted scalar")
    {
        "A quoted scalar or flow collection was never closed — check the end of the document."
    } else if m.contains("found undefined alias") {
        "An alias (*name) must refer to an anchor (&name) defined earlier in the same document."
    } else if m.contains("did not find expected node content") {
        "A value is missing after the key, or a stray character follows a dash or colon."
    } else {
        return None;
    };
    Some(hint.to_string())
}

fn check_toml(text: &str) -> Vec<Diagnostic> {
    match text.parse::<toml::Table>() {
        Ok(_) => Vec::new(),
        Err(e) => {
            let msg = strip_position_suffix(e.message());
            let hint = toml_hint(&msg);
            match e.span() {
                Some(span) => {
                    let (line, column) = line_col(text, span.start);
                    vec![Diagnostic::error(
                        Some(line),
                        Some(column),
                        Some(span.start),
                        msg,
                        hint,
                    )]
                }
                None => vec![Diagnostic::error(None, None, None, msg, hint)],
            }
        }
    }
}

fn toml_hint(msg: &str) -> Option<String> {
    let m = msg.to_ascii_lowercase();
    let hint = if m.contains("duplicate key") {
        "TOML rejects a key defined twice in the same table — rename or remove one."
    } else if m.contains("invalid string") || m.contains("basic string") {
        "TOML strings must be double-quoted (\"…\") or literal-quoted ('…'); bare words are not values."
    } else if m.contains("expected") && m.contains("newline") {
        "Every key/value pair needs its own line — TOML has no statement separator."
    } else if m.contains("invalid table header") || m.contains("expected `]`") {
        "A table header looks like [section] and a table-array header like [[section]]."
    } else if m.contains("unspecified") || m.contains("invalid key") {
        "Bare keys allow only A-Z a-z 0-9 _ and -; quote anything else."
    } else if m.contains("expected") {
        "TOML requires `key = value` with the value on the same line."
    } else {
        return None;
    };
    Some(hint.to_string())
}

fn check_xml(text: &str) -> Vec<Diagnostic> {
    use quick_xml::events::Event;
    use quick_xml::Reader;

    let mut reader = Reader::from_str(text);
    reader.config_mut().check_end_names = true;
    let mut depth: i32 = 0;
    let mut roots = 0usize;
    loop {
        match reader.read_event() {
            Ok(Event::Eof) => break,
            Ok(Event::Start(_)) => {
                if depth == 0 {
                    roots += 1;
                }
                depth += 1;
            }
            Ok(Event::End(_)) => depth -= 1,
            Ok(Event::Empty(_)) => {
                if depth == 0 {
                    roots += 1;
                }
            }
            Ok(Event::Text(t)) => {
                if depth == 0 && !t.iter().all(|b| b.is_ascii_whitespace()) {
                    let offset = reader.buffer_position() as usize;
                    let (line, column) = line_col(text, offset.min(text.len()));
                    return vec![Diagnostic::error(
                        Some(line),
                        Some(column),
                        Some(offset.min(text.len())),
                        "text appears outside the root element",
                        Some(
                            "Everything except the declaration, comments and processing instructions must sit inside a single root element."
                                .to_string(),
                        ),
                    )];
                }
            }
            Ok(_) => {}
            Err(e) => {
                let offset = (reader.error_position() as usize).min(text.len());
                let (line, column) = line_col(text, offset);
                let msg = strip_position_suffix(&e.to_string());
                let hint = xml_hint(&msg);
                return vec![Diagnostic::error(
                    Some(line),
                    Some(column),
                    Some(offset),
                    msg,
                    hint,
                )];
            }
        }
        if roots > 1 {
            let offset = (reader.buffer_position() as usize).min(text.len());
            let (line, column) = line_col(text, offset);
            return vec![Diagnostic::error(
                Some(line),
                Some(column),
                Some(offset),
                "document has more than one root element",
                Some(
                    "A well-formed XML document has exactly one root — wrap the siblings in a shared parent element."
                        .to_string(),
                ),
            )];
        }
    }
    if roots == 0 {
        return vec![Diagnostic::error(
            Some(1),
            Some(1),
            Some(0),
            "no root element found",
            Some("An XML document needs at least one element, for example <config></config>.".to_string()),
        )];
    }
    if depth != 0 {
        let (line, column) = line_col(text, text.len());
        return vec![Diagnostic::error(
            Some(line),
            Some(column),
            Some(text.len()),
            "document ends with unclosed elements",
            Some("Every opening tag needs a matching closing tag, or must be written self-closing as <tag/>.".to_string()),
        )];
    }
    Vec::new()
}

fn xml_hint(msg: &str) -> Option<String> {
    let m = msg.to_ascii_lowercase();
    let hint = if m.contains("expecting </") || m.contains("mismatch") {
        "The closing tag does not match the innermost open tag — XML elements must nest, never overlap."
    } else if m.contains("attribute") && m.contains("duplicat") {
        "An element cannot repeat the same attribute name."
    } else if m.contains("attribute") {
        "Every attribute value must be quoted: <tag name=\"value\"/>."
    } else if m.contains("ill-formed") || m.contains("unexpected token") {
        "A bare < or & is not allowed in text — escape them as &lt; and &amp;."
    } else if m.contains("unexpected eof") || m.contains("unclosed") {
        "A tag, comment or CDATA section was never closed."
    } else {
        return None;
    };
    Some(hint.to_string())
}

/// INI has no specification, so this scanner implements the common denominator:
/// `[section]` headers, `key = value` / `key: value` pairs, `#`/`;` comments and
/// indented continuation lines. Unlike the other formats it reports every bad
/// line, not just the first.
fn check_ini(text: &str, strict: bool) -> Vec<Diagnostic> {
    let mut out: Vec<Diagnostic> = Vec::new();
    let mut offset = 0usize;
    let mut section = String::new();
    let mut seen_sections: HashSet<String> = HashSet::new();
    let mut seen_keys: HashSet<(String, String)> = HashSet::new();
    let mut prev_was_pair = false;

    for (idx, raw) in text.lines().enumerate() {
        let line_no = idx + 1;
        let line_start = offset;
        offset += raw.len() + 1;
        let line = raw.trim_start_matches('\u{feff}');
        let trimmed = line.trim();

        if trimmed.is_empty() {
            prev_was_pair = false;
            continue;
        }
        if trimmed.starts_with('#') || trimmed.starts_with(';') {
            continue;
        }
        let indent = line.len() - line.trim_start().len();
        if indent > 0 && prev_was_pair && !trimmed.starts_with('[') {
            // Indented continuation of the previous value (RFC 822 style).
            continue;
        }

        if trimmed.starts_with('[') {
            prev_was_pair = false;
            let Some(close) = trimmed.rfind(']') else {
                let col = char_col(line, indent) + 1;
                out.push(Diagnostic::error(
                    Some(line_no),
                    Some(col),
                    Some(line_start + indent),
                    "unterminated section header",
                    Some("A section header must close with a bracket: [database].".to_string()),
                ));
                continue;
            };
            let name = trimmed[1..close].trim();
            let rest = trimmed[close + 1..].trim();
            if name.is_empty() {
                let col = char_col(line, indent) + 1;
                out.push(Diagnostic::error(
                    Some(line_no),
                    Some(col),
                    Some(line_start + indent),
                    "empty section name",
                    Some("Give the section a name, for example [server].".to_string()),
                ));
            } else if !rest.is_empty() && !rest.starts_with('#') && !rest.starts_with(';') {
                let byte_in_line = line.len() - rest.len();
                let col = char_col(line, byte_in_line) + 1;
                out.push(Diagnostic::error(
                    Some(line_no),
                    Some(col),
                    Some(line_start + byte_in_line),
                    format!("unexpected text after the section header: '{rest}'"),
                    Some("Nothing but a comment may follow ] on a section line.".to_string()),
                ));
            } else if strict && !seen_sections.insert(name.to_string()) {
                let col = char_col(line, indent) + 1;
                out.push(Diagnostic::warning(
                    line_no,
                    col,
                    line_start + indent,
                    format!("duplicate section [{name}]"),
                    Some(
                        "Most INI readers merge repeated sections; some keep only the last. Merge them yourself to be safe."
                            .to_string(),
                    ),
                ));
            }
            section = name.to_string();
            continue;
        }

        let sep = trimmed
            .find('=')
            .into_iter()
            .chain(trimmed.find(':'))
            .min();
        let Some(sep) = sep else {
            let col = char_col(line, indent) + 1;
            out.push(Diagnostic::error(
                Some(line_no),
                Some(col),
                Some(line_start + indent),
                format!("expected 'key = value', a [section] header or a comment, found '{trimmed}'"),
                Some("Add a = or : between the key and its value, or start the line with # or ; to comment it out.".to_string()),
            ));
            prev_was_pair = false;
            continue;
        };
        let key = trimmed[..sep].trim();
        if key.is_empty() {
            let col = char_col(line, indent) + 1;
            out.push(Diagnostic::error(
                Some(line_no),
                Some(col),
                Some(line_start + indent),
                "missing key before the separator",
                Some("Write the setting as name = value; a line may not begin with = or :.".to_string()),
            ));
            prev_was_pair = false;
            continue;
        }
        if strict && !seen_keys.insert((section.clone(), key.to_string())) {
            let col = char_col(line, indent) + 1;
            let where_ = if section.is_empty() {
                "before the first section".to_string()
            } else {
                format!("in [{section}]")
            };
            out.push(Diagnostic::warning(
                line_no,
                col,
                line_start + indent,
                format!("duplicate key '{key}' {where_}"),
                Some(
                    "INI readers disagree about repeated keys — some keep the first, some the last, some build a list."
                        .to_string(),
                ),
            ));
        }
        prev_was_pair = true;
    }
    out
}

/// Column (0-based, in characters) of a byte index within a single line.
fn char_col(line: &str, byte_idx: usize) -> usize {
    line[..byte_idx.min(line.len())].chars().count()
}

/// Format-independent portability warnings, only emitted with `strict = true`.
fn portability_warnings(text: &str, format: Format) -> Vec<Diagnostic> {
    let mut out = Vec::new();
    if text.starts_with('\u{feff}') {
        out.push(Diagnostic::warning(
            1,
            1,
            0,
            "file starts with a UTF-8 byte-order mark",
            Some(
                "Many parsers and shells treat the BOM as content — save the file as UTF-8 without a BOM."
                    .to_string(),
            ),
        ));
    }
    let has_crlf = text.contains("\r\n");
    let has_bare_lf = text
        .match_indices('\n')
        .any(|(i, _)| i == 0 || text.as_bytes()[i - 1] != b'\r');
    if has_crlf && has_bare_lf {
        out.push(Diagnostic::warning(
            1,
            1,
            0,
            "file mixes CRLF and LF line endings",
            Some("Normalise the file to one line ending so diffs and parsers stay predictable.".to_string()),
        ));
    }
    if matches!(format, Format::Yaml | Format::Toml | Format::Ini) {
        let mut offset = 0usize;
        for (idx, raw) in text.lines().enumerate() {
            let indent_bytes = raw.len() - raw.trim_start().len();
            if raw[..indent_bytes].contains('\t') {
                out.push(Diagnostic::warning(
                    idx + 1,
                    1,
                    offset,
                    "tab character used for indentation",
                    Some(
                        "Indent with spaces — YAML forbids tabs outright and TOML/INI tools disagree about their width."
                            .to_string(),
                    ),
                ));
            }
            offset += raw.len() + 1;
        }
    }
    out
}

/// Report keys repeated inside one JSON object. The input has already parsed, so
/// the token walk below can assume a well-formed document.
fn json_duplicate_keys(text: &str) -> Vec<Diagnostic> {
    let b = text.as_bytes();
    let mut i = 0usize;
    let mut stack: Vec<Option<HashSet<String>>> = Vec::new();
    let mut expect_key = false;
    let mut out = Vec::new();
    while i < b.len() {
        match b[i] {
            b'{' => {
                stack.push(Some(HashSet::new()));
                expect_key = true;
                i += 1;
            }
            b'[' => {
                stack.push(None);
                expect_key = false;
                i += 1;
            }
            b'}' | b']' => {
                stack.pop();
                expect_key = false;
                i += 1;
            }
            b',' => {
                expect_key = matches!(stack.last(), Some(Some(_)));
                i += 1;
            }
            b':' => {
                expect_key = false;
                i += 1;
            }
            b'"' => {
                let start = i;
                let (raw, next) = scan_json_string(b, i);
                i = next;
                if expect_key {
                    if let Some(Some(set)) = stack.last_mut() {
                        if !set.insert(raw.clone()) {
                            let (line, column) = line_col(text, start);
                            out.push(Diagnostic::warning(
                                line,
                                column,
                                start,
                                format!("duplicate key \"{raw}\" in the same object"),
                                Some(
                                    "JSON parsers keep the last occurrence and silently drop the earlier one."
                                        .to_string(),
                                ),
                            ));
                        }
                    }
                    expect_key = false;
                }
            }
            _ => i += 1,
        }
    }
    out
}

/// Return the raw (still escaped) body of the JSON string starting at `start`,
/// plus the index just past its closing quote.
fn scan_json_string(b: &[u8], start: usize) -> (String, usize) {
    let mut i = start + 1;
    let body_start = i;
    while i < b.len() {
        match b[i] {
            b'\\' => i += 2,
            b'"' => {
                let raw = String::from_utf8_lossy(&b[body_start..i]).into_owned();
                return (raw, i + 1);
            }
            _ => i += 1,
        }
    }
    (
        String::from_utf8_lossy(&b[body_start..b.len()]).into_owned(),
        b.len(),
    )
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn render_snippet(text: &str, diag: &Diagnostic, context_lines: usize) -> Option<String> {
    if context_lines == 0 {
        return None;
    }
    let line_no = diag.line?;
    let lines: Vec<&str> = text.lines().collect();
    if line_no == 0 || line_no > lines.len() {
        return None;
    }
    let first = line_no.saturating_sub(context_lines).max(1);
    let last = (line_no + context_lines).min(lines.len());
    let gutter = last.to_string().len();
    let mut out = String::new();
    for n in first..=last {
        let _ = writeln!(out, "{:>gutter$} | {}", n, lines[n - 1].replace('\t', "    "));
        if n == line_no {
            if let Some(col) = diag.column {
                let pad = lines[n - 1]
                    .chars()
                    .take(col.saturating_sub(1))
                    .map(|c| if c == '\t' { 4 } else { 1 })
                    .sum::<usize>();
                let _ = writeln!(out, "{:>gutter$} | {}^", "", " ".repeat(pad));
            }
        }
    }
    Some(out)
}

fn render_report(
    text: &str,
    check: &Check,
    auto_detected: bool,
    strict: bool,
    context_lines: usize,
) -> String {
    let errors = check.error_count();
    let warnings = check.diagnostics.len() - errors;
    let source = if auto_detected {
        " (auto-detected)"
    } else {
        ""
    };
    let mut out = String::new();

    if errors == 0 {
        let _ = writeln!(out, "VALID — {}{}", check.format.label(), source);
    } else {
        let _ = writeln!(out, "INVALID — {}{}", check.format.label(), source);
    }

    let line_count = text.lines().count();
    let _ = writeln!(
        out,
        "{} line{} · {} byte{}",
        line_count,
        if line_count == 1 { "" } else { "s" },
        text.len(),
        if text.len() == 1 { "" } else { "s" }
    );
    let _ = writeln!(out);

    if check.diagnostics.is_empty() {
        let _ = writeln!(
            out,
            "No syntax errors found.{}",
            if strict {
                ""
            } else {
                " Turn on strict checks for duplicate keys, tab indentation and BOM warnings."
            }
        );
        return out;
    }

    let _ = writeln!(
        out,
        "{} error{}, {} warning{}",
        errors,
        if errors == 1 { "" } else { "s" },
        warnings,
        if warnings == 1 { "" } else { "s" }
    );
    let _ = writeln!(out);

    let shown = check.diagnostics.len().min(MAX_DIAGNOSTICS);
    for (i, d) in check.diagnostics.iter().take(shown).enumerate() {
        let position = match (d.line, d.column, d.offset) {
            (Some(l), Some(c), Some(o)) => format!("line {l}, column {c} (offset {o})"),
            (Some(l), Some(c), None) => format!("line {l}, column {c}"),
            (Some(l), None, _) => format!("line {l}"),
            _ => "position not reported by the parser".to_string(),
        };
        let _ = writeln!(out, "{}. {} · {}", i + 1, d.severity.id(), position);
        let _ = writeln!(out, "   {}", d.message);
        if let Some(h) = &d.hint {
            let _ = writeln!(out, "   fix: {h}");
        }
        if let Some(snippet) = render_snippet(text, d, context_lines) {
            let _ = writeln!(out);
            for line in snippet.lines() {
                let _ = writeln!(out, "   {line}");
            }
        }
        if i + 1 < shown {
            let _ = writeln!(out);
        }
    }
    if check.diagnostics.len() > shown {
        let _ = writeln!(out);
        let _ = writeln!(
            out,
            "… and {} more (report truncated at {MAX_DIAGNOSTICS}).",
            check.diagnostics.len() - shown
        );
    }
    out
}

fn render_json(text: &str, check: &Check, auto_detected: bool, strict: bool) -> String {
    let errors = check.error_count();
    let diagnostics: Vec<Value> = check
        .diagnostics
        .iter()
        .take(MAX_DIAGNOSTICS)
        .map(|d| {
            json!({
                "severity": d.severity.id(),
                "line": d.line,
                "column": d.column,
                "offset": d.offset,
                "message": d.message,
                "hint": d.hint,
            })
        })
        .collect();
    let doc = json!({
        "valid": errors == 0,
        "format": check.format.id(),
        "format_source": if auto_detected { "auto-detected" } else { "specified" },
        "strict": strict,
        "error_count": errors,
        "warning_count": check.diagnostics.len() - errors,
        "truncated": check.diagnostics.len() > MAX_DIAGNOSTICS,
        "diagnostics": diagnostics,
        "summary": { "lines": text.lines().count(), "bytes": text.len() },
    });
    serde_json::to_string_pretty(&doc).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Validate `text` and render the result.
///
/// * `format` — `auto`, `json`, `yaml`, `toml`, `ini` or `xml`.
/// * `strict` — add duplicate-key, tab-indentation, BOM and line-ending warnings.
/// * `report_format` — `report` (human-readable) or `json` (machine-readable).
/// * `context_lines` — source lines shown above and below each flagged line (0-10).
pub fn validate(
    text: &str,
    format: &str,
    strict: bool,
    report_format: &str,
    context_lines: usize,
) -> Result<String, String> {
    if text.trim().is_empty() {
        return Err("input is empty — paste the config file you want to check".to_string());
    }
    if text.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "input is {} bytes; the limit is {} bytes (1 MiB)",
            text.len(),
            MAX_INPUT_BYTES
        ));
    }
    if context_lines > MAX_CONTEXT_LINES {
        return Err(format!(
            "context_lines must be between 0 and {MAX_CONTEXT_LINES}, got {context_lines}"
        ));
    }
    let report_format = match report_format.trim() {
        "" => "report",
        r => r,
    };
    if !matches!(report_format, "report" | "json") {
        return Err(format!(
            "report_format must be 'report' or 'json', got '{report_format}'"
        ));
    }

    let requested = format.trim();
    let (check_result, auto_detected) = if requested.is_empty() || requested.eq_ignore_ascii_case("auto")
    {
        let ranked = rank_formats(text);
        let mut first: Option<Check> = None;
        let mut chosen: Option<Check> = None;
        for candidate in ranked {
            let c = check(text, candidate, strict);
            if c.error_count() == 0 {
                chosen = Some(c);
                break;
            }
            if first.is_none() {
                first = Some(c);
            }
        }
        (chosen.or(first).expect("at least one candidate"), true)
    } else {
        let f = Format::from_id(requested).ok_or_else(|| {
            format!("format must be auto, json, yaml, toml, ini or xml, got '{requested}'")
        })?;
        (check(text, f, strict), false)
    };

    Ok(match report_format {
        "json" => render_json(text, &check_result, auto_detected, strict),
        _ => render_report(text, &check_result, auto_detected, strict, context_lines),
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn json_doc(text: &str, format: &str, strict: bool) -> Value {
        serde_json::from_str(&validate(text, format, strict, "json", 0).unwrap()).unwrap()
    }

    #[test]
    fn valid_json_is_detected_and_accepted() {
        let doc = json_doc("{\"name\": \"Ada\", \"port\": 8080}", "auto", false);
        assert_eq!(doc["valid"], true);
        assert_eq!(doc["format"], "json");
        assert_eq!(doc["format_source"], "auto-detected");
        assert_eq!(doc["error_count"], 0);
    }

    #[test]
    fn trailing_comma_reports_line_and_column() {
        let doc = json_doc("{\n  \"a\": 1,\n  \"b\": 2,\n}\n", "json", false);
        assert_eq!(doc["valid"], false);
        assert_eq!(doc["error_count"], 1);
        assert_eq!(doc["diagnostics"][0]["line"], 4);
        assert_eq!(doc["diagnostics"][0]["column"], 1);
        assert_eq!(doc["diagnostics"][0]["severity"], "error");
        assert!(doc["diagnostics"][0]["hint"]
            .as_str()
            .unwrap()
            .contains("comma"));
    }

    #[test]
    fn yaml_bad_indentation_reports_a_position() {
        let doc = json_doc("server:\n  host: localhost\n   port: 8080\n", "yaml", false);
        assert_eq!(doc["valid"], false);
        assert!(doc["diagnostics"][0]["line"].as_u64().unwrap() >= 2);
        assert!(doc["diagnostics"][0]["column"].as_u64().is_some());
    }

    #[test]
    fn multi_document_yaml_is_valid() {
        let doc = json_doc("---\na: 1\n---\nb: 2\n", "yaml", false);
        assert_eq!(doc["valid"], true, "{doc}");
    }

    #[test]
    fn toml_error_has_line_and_column() {
        let doc = json_doc("[server]\nhost = localhost\n", "toml", false);
        assert_eq!(doc["valid"], false);
        assert_eq!(doc["diagnostics"][0]["line"], 2);
        assert!(doc["diagnostics"][0]["column"].as_u64().unwrap() >= 8);
    }

    #[test]
    fn valid_toml_beats_ini_in_auto_detection() {
        let doc = json_doc("[server]\nhost = \"localhost\"\nport = 8080\n", "auto", false);
        assert_eq!(doc["valid"], true);
        assert_eq!(doc["format"], "toml");
    }

    #[test]
    fn bare_ini_values_fall_through_to_ini() {
        let doc = json_doc("[server]\nhost = localhost\nport = 8080\n", "auto", false);
        assert_eq!(doc["valid"], true, "{doc}");
        assert_eq!(doc["format"], "ini");
    }

    #[test]
    fn ini_reports_every_bad_line() {
        let doc = json_doc("[server\nhost localhost\n= 5\n", "ini", false);
        assert_eq!(doc["error_count"], 3, "{doc}");
        assert_eq!(doc["diagnostics"][0]["line"], 1);
        assert_eq!(doc["diagnostics"][1]["line"], 2);
        assert_eq!(doc["diagnostics"][2]["line"], 3);
    }

    #[test]
    fn xml_mismatched_tag_is_an_error() {
        let doc = json_doc("<config>\n  <host>a</hostname>\n</config>\n", "xml", false);
        assert_eq!(doc["valid"], false);
        assert_eq!(doc["diagnostics"][0]["line"], 2);
    }

    #[test]
    fn xml_with_declaration_is_detected() {
        let doc = json_doc(
            "<?xml version=\"1.0\"?>\n<config><host>localhost</host></config>\n",
            "auto",
            false,
        );
        assert_eq!(doc["valid"], true, "{doc}");
        assert_eq!(doc["format"], "xml");
    }

    #[test]
    fn xml_needs_exactly_one_root() {
        let doc = json_doc("<a/>\n<b/>\n", "xml", false);
        assert_eq!(doc["valid"], false);
        assert!(doc["diagnostics"][0]["message"]
            .as_str()
            .unwrap()
            .contains("root"));
    }

    #[test]
    fn strict_flags_duplicate_json_keys_as_warnings() {
        let doc = json_doc("{\n  \"a\": 1,\n  \"a\": 2\n}\n", "json", true);
        assert_eq!(doc["valid"], true, "duplicates are a warning, not an error");
        assert_eq!(doc["warning_count"], 1);
        assert_eq!(doc["diagnostics"][0]["line"], 3);
        assert_eq!(doc["diagnostics"][0]["severity"], "warning");
    }

    #[test]
    fn strict_flags_duplicate_ini_keys_and_tabs() {
        let doc = json_doc("[a]\nk = 1\nk = 2\n\tcontinued value\n", "ini", true);
        assert_eq!(doc["error_count"], 0, "{doc}");
        assert_eq!(doc["warning_count"], 2, "{doc}");
    }

    #[test]
    fn strict_is_off_by_default_so_duplicates_stay_quiet() {
        let doc = json_doc("{\n  \"a\": 1,\n  \"a\": 2\n}\n", "json", false);
        assert_eq!(doc["warning_count"], 0);
    }

    #[test]
    fn report_shows_a_caret_under_the_column() {
        let out = validate("{\n  \"a\": 1,\n}\n", "json", false, "report", 1).unwrap();
        assert!(out.starts_with("INVALID — JSON"), "{out}");
        assert!(out.contains("line 3, column 1"), "{out}");
        assert!(out.contains("^"), "{out}");
    }

    #[test]
    fn valid_report_names_the_format() {
        let out = validate("a: 1\n", "auto", false, "report", 2).unwrap();
        assert!(out.starts_with("VALID — YAML (auto-detected)"), "{out}");
    }

    #[test]
    fn empty_input_is_an_error() {
        assert!(validate("   \n", "auto", false, "report", 2)
            .unwrap_err()
            .contains("empty"));
    }

    #[test]
    fn unknown_format_is_an_error() {
        let err = validate("a: 1", "hcl", false, "report", 2).unwrap_err();
        assert!(err.contains("format must be"), "{err}");
    }

    #[test]
    fn unknown_report_format_is_an_error() {
        let err = validate("a: 1", "auto", false, "yaml", 2).unwrap_err();
        assert!(err.contains("report_format"), "{err}");
    }

    #[test]
    fn context_lines_above_the_cap_is_an_error() {
        let err = validate("a: 1", "auto", false, "report", 11).unwrap_err();
        assert!(err.contains("context_lines"), "{err}");
    }

    #[test]
    fn oversized_input_is_rejected() {
        let big = "a: 1\n".repeat(MAX_INPUT_BYTES / 5 + 10);
        let err = validate(&big, "yaml", false, "report", 0).unwrap_err();
        assert!(err.contains("limit is"), "{err}");
    }

    #[test]
    fn context_lines_zero_omits_the_snippet() {
        let out = validate("{\n  \"a\": 1,\n}\n", "json", false, "report", 0).unwrap();
        assert!(!out.contains(" | "), "{out}");
    }

    #[test]
    fn every_format_can_be_requested_explicitly() {
        for (fmt, sample) in [
            ("json", "{\"a\": 1}"),
            ("yaml", "a: 1\n"),
            ("toml", "a = 1\n"),
            ("ini", "a = 1\n"),
            ("xml", "<a>1</a>"),
        ] {
            let doc = json_doc(sample, fmt, false);
            assert_eq!(doc["valid"], true, "{fmt} failed: {doc}");
            assert_eq!(doc["format"], fmt);
            assert_eq!(doc["format_source"], "specified");
        }
    }
}
