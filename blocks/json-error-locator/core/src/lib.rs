//! gizza-ai/json-error-locator core — pinpoint the line, column and cause of a
//! JSON syntax error.
//!
//! Two passes run over the pasted text:
//!
//! 1. `serde_json` is the authority on whether the document parses at all, and
//!    supplies the position where a real parser gives up.
//! 2. A tolerant scanner walks the same text and keeps going past the first
//!    problem, so every deviation is reported with its own line/column, a
//!    plain-English cause, and a concrete fix — a parser can only ever show
//!    the first one.
//!
//! Pure Rust, no I/O. Columns are counted in characters (what an editor shows),
//! offsets are 0-based character offsets from the start of the document.

use serde_json::{json, Value};

/// Largest input accepted, in bytes.
pub const MAX_INPUT_BYTES: usize = 1_048_576;

/// Nesting depth at which the scanner stops descending.
const MAX_DEPTH: usize = 200;

/// Never emit more than this many issues (a badly mangled file can cascade).
const MAX_ISSUES: usize = 50;

/// How wide a context line may get before it is windowed around the caret.
const MAX_CONTEXT_WIDTH: usize = 120;

/// One diagnosed problem in the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    /// Stable machine-readable slug, e.g. `trailing-comma`.
    pub kind: &'static str,
    /// Human-readable name, e.g. `trailing comma`.
    pub label: &'static str,
    /// 1-based line number.
    pub line: usize,
    /// 1-based column, counted in characters.
    pub column: usize,
    /// 0-based character offset from the start of the document.
    pub offset: usize,
    /// Why this is not valid JSON.
    pub explain: String,
    /// What to change to make it valid.
    pub fix: String,
}

/// Facts about a document that parsed cleanly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Summary {
    /// `object`, `array`, `string`, `number`, `boolean` or `null`.
    pub kind: &'static str,
    /// Members of a top-level object/array; `0` for a scalar.
    pub members: usize,
    /// Deepest nesting level (a scalar document is `0`).
    pub depth: usize,
    pub lines: usize,
    pub bytes: usize,
}

/// The full diagnosis of one document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Analysis {
    pub valid: bool,
    pub issues: Vec<Issue>,
    /// Where `serde_json` itself stopped: (line, column, message).
    pub parser_stop: Option<(usize, usize, String)>,
    pub summary: Option<Summary>,
}

/// Diagnose `json` and render the result.
///
/// `output` is `report` (human-readable, the default) or `json` (a machine
/// readable `{valid, issue_count, issues, …}` document). `context_lines` is how
/// many source lines to show either side of each caret (0–10). When `scan_all`
/// is false only the first issue is reported, the way a plain parser behaves.
/// Blank/unknown `output` falls back to `report`.
pub fn locate(
    json: &str,
    output: &str,
    context_lines: usize,
    scan_all: bool,
) -> Result<String, String> {
    if json.trim().is_empty() {
        return Err("no JSON input".into());
    }
    if json.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "input is {} bytes; the limit is {MAX_INPUT_BYTES} bytes",
            json.len()
        ));
    }
    let ctx = context_lines.min(10);
    let mut analysis = analyze(json);
    if !scan_all && analysis.issues.len() > 1 {
        analysis.issues.truncate(1);
    }
    match output.trim() {
        "" | "report" => Ok(render_report(json, &analysis, ctx)),
        "json" => Ok(render_json(json, &analysis, ctx)),
        other => Err(format!(
            "unknown output '{other}'; expected 'report' or 'json'"
        )),
    }
}

/// Parse-check `json` and, when it fails, collect every issue the scanner finds.
pub fn analyze(json: &str) -> Analysis {
    match serde_json::from_str::<Value>(json) {
        Ok(v) => Analysis {
            valid: true,
            issues: Vec::new(),
            parser_stop: None,
            summary: Some(summarize(&v, json)),
        },
        Err(e) => {
            let mut issues = scan(json);
            issues.sort_by_key(|i| i.offset);
            let (line, column) = (e.line().max(1), e.column().max(1));
            if issues.is_empty() {
                // The scanner agrees the syntax is well-formed, so the failure is
                // something only a full parser sees (a lone surrogate escape, a
                // number outside the supported range, nesting past the parser's
                // own recursion limit). Report the parser's own position.
                issues.push(Issue {
                    kind: "parse-error",
                    label: "parse error",
                    line,
                    column,
                    offset: offset_of(json, line, column),
                    explain: format!("the JSON parser rejected this document: {e}"),
                    fix: "check the value at this position against the JSON grammar (json.org)."
                        .into(),
                });
            }
            Analysis {
                valid: false,
                issues,
                parser_stop: Some((line, column, e.to_string())),
                summary: None,
            }
        }
    }
}

fn summarize(v: &Value, text: &str) -> Summary {
    let (kind, members) = match v {
        Value::Object(m) => ("object", m.len()),
        Value::Array(a) => ("array", a.len()),
        Value::String(_) => ("string", 0),
        Value::Number(_) => ("number", 0),
        Value::Bool(_) => ("boolean", 0),
        Value::Null => ("null", 0),
    };
    Summary {
        kind,
        members,
        depth: value_depth(v),
        lines: text.lines().count().max(1),
        bytes: text.len(),
    }
}

fn value_depth(v: &Value) -> usize {
    match v {
        Value::Object(m) => 1 + m.values().map(value_depth).max().unwrap_or(0),
        Value::Array(a) => 1 + a.iter().map(value_depth).max().unwrap_or(0),
        _ => 0,
    }
}

/// Character offset of a 1-based (line, column) pair.
fn offset_of(text: &str, line: usize, column: usize) -> usize {
    let mut off = 0usize;
    let mut l = 1usize;
    let mut c = 1usize;
    for ch in text.chars() {
        if l == line && c == column {
            return off;
        }
        off += 1;
        if ch == '\n' {
            l += 1;
            c = 1;
        } else {
            c += 1;
        }
    }
    off
}

// ---------------------------------------------------------------------------
// Tolerant scanner
// ---------------------------------------------------------------------------

#[derive(Clone, Copy)]
struct Ch {
    c: char,
    line: usize,
    col: usize,
    off: usize,
}

fn chars_of(s: &str) -> Vec<Ch> {
    let mut out = Vec::with_capacity(s.len());
    let (mut line, mut col, mut off) = (1usize, 1usize, 0usize);
    for c in s.chars() {
        out.push(Ch { c, line, col, off });
        off += 1;
        if c == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    out
}

struct Scan {
    c: Vec<Ch>,
    i: usize,
    issues: Vec<Issue>,
    depth: usize,
    aborted: bool,
}

/// Walk `text` and record every JSON syntax deviation, recovering after each so
/// later problems are still found.
pub fn scan(text: &str) -> Vec<Issue> {
    let mut s = Scan {
        c: chars_of(text),
        i: 0,
        issues: Vec::new(),
        depth: 0,
        aborted: false,
    };
    s.skip_ws();
    if s.peek().is_none() {
        return s.issues;
    }
    s.value();
    if !s.aborted {
        s.skip_ws();
        if s.peek().is_some() {
            let at = s.i;
            s.push(
                at,
                "trailing-content",
                "extra content after the JSON value",
                "the document already ended here, but more text follows; a JSON document holds exactly one top-level value.".into(),
                "wrap the values in an array ([a, b]) or split them into separate documents; for newline-delimited JSON, validate one line at a time.".into(),
            );
        }
    }
    s.issues
}

impl Scan {
    fn peek(&self) -> Option<char> {
        self.c.get(self.i).map(|x| x.c)
    }

    fn peek_at(&self, n: usize) -> Option<char> {
        self.c.get(self.i + n).map(|x| x.c)
    }

    /// (line, column, offset) of index `i`, extrapolating one past the end.
    fn pos(&self, i: usize) -> (usize, usize, usize) {
        if let Some(ch) = self.c.get(i) {
            return (ch.line, ch.col, ch.off);
        }
        match self.c.last() {
            None => (1, 1, 0),
            Some(last) if last.c == '\n' => (last.line + 1, 1, last.off + 1),
            Some(last) => (last.line, last.col + 1, last.off + 1),
        }
    }

    fn push(&mut self, at: usize, kind: &'static str, label: &'static str, explain: String, fix: String) {
        if self.aborted {
            return; // an unterminated string/deep nest makes everything after it noise
        }
        if self.issues.len() >= MAX_ISSUES {
            self.aborted = true;
            return;
        }
        let (line, column, offset) = self.pos(at);
        self.issues.push(Issue {
            kind,
            label,
            line,
            column,
            offset,
            explain,
            fix,
        });
    }

    fn skip_ws(&mut self) {
        loop {
            match self.peek() {
                Some(' ') | Some('\t') | Some('\n') | Some('\r') => self.i += 1,
                Some('/') if matches!(self.peek_at(1), Some('/')) => {
                    let at = self.i;
                    self.push(
                        at,
                        "comment",
                        "comment",
                        "JSON has no comment syntax, so this // line is a syntax error.".into(),
                        "delete the comment, or move the note into a string field such as \"_comment\"."
                            .into(),
                    );
                    while self.peek().is_some() && self.peek() != Some('\n') {
                        self.i += 1;
                    }
                }
                Some('/') if matches!(self.peek_at(1), Some('*')) => {
                    let at = self.i;
                    self.push(
                        at,
                        "comment",
                        "comment",
                        "JSON has no comment syntax, so this /* … */ block is a syntax error.".into(),
                        "delete the comment, or move the note into a string field such as \"_comment\"."
                            .into(),
                    );
                    self.i += 2;
                    while self.peek().is_some()
                        && !(self.peek() == Some('*') && self.peek_at(1) == Some('/'))
                    {
                        self.i += 1;
                    }
                    if self.peek().is_some() {
                        self.i += 2;
                    }
                }
                _ => return,
            }
        }
    }

    fn value(&mut self) {
        self.skip_ws();
        if self.aborted {
            return;
        }
        match self.peek() {
            None => {
                let at = self.i;
                self.push(
                    at,
                    "unexpected-end",
                    "unexpected end of input",
                    "a value was expected here, but the document ended.".into(),
                    "add the missing value, or remove the comma/colon that promised one.".into(),
                );
            }
            Some('{') => self.object(),
            Some('[') => self.array(),
            Some('"') => self.string(),
            Some('\'') => {
                let at = self.i;
                self.push(
                    at,
                    "single-quote",
                    "single-quoted string",
                    "JSON strings must use double quotes; single quotes are a JavaScript/Python habit."
                        .into(),
                    "replace the surrounding ' with \" (and escape any \" inside the text as \\\").".into(),
                );
                self.quoted('\'');
            }
            Some('-') | Some('+') | Some('.') => self.number(),
            Some(c) if c.is_ascii_digit() => self.number(),
            Some(c) if c.is_alphabetic() || c == '_' => self.ident(),
            Some(c) => {
                let at = self.i;
                self.i += 1;
                self.push(
                    at,
                    "unexpected-token",
                    "unexpected character",
                    format!("'{c}' cannot start a JSON value."),
                    "a value must be a string in double quotes, a number, true, false, null, an object {…} or an array […]."
                        .into(),
                );
            }
        }
    }

    fn object(&mut self) {
        let open = self.i;
        self.i += 1; // '{'
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            self.push(
                open,
                "too-deep",
                "nesting too deep",
                format!("nesting goes past {MAX_DEPTH} levels, deeper than this tool (and most parsers) will follow."),
                "check for an unclosed bracket earlier in the document, which makes everything after it look nested.".into(),
            );
            self.aborted = true;
            self.depth -= 1;
            return;
        }
        loop {
            let before = self.i;
            self.skip_ws();
            match self.peek() {
                None => {
                    self.unterminated(open, '{');
                    break;
                }
                Some('}') => {
                    self.i += 1;
                    break;
                }
                _ => {}
            }
            self.key();
            self.skip_ws();
            match self.peek() {
                Some(':') => self.i += 1,
                Some('=') => {
                    let at = self.i;
                    self.i += 1;
                    self.push(
                        at,
                        "missing-colon",
                        "'=' used instead of ':'",
                        "a JSON member separates its name and value with a colon, not an equals sign."
                            .into(),
                        "replace = with :".into(),
                    );
                }
                _ => {
                    let at = self.i;
                    self.push(
                        at,
                        "missing-colon",
                        "missing colon",
                        "a colon was expected after this member name.".into(),
                        "write \"name\": value".into(),
                    );
                }
            }
            self.value();
            if self.aborted {
                break;
            }
            self.skip_ws();
            match self.peek() {
                Some(',') => {
                    let comma = self.i;
                    self.i += 1;
                    self.skip_ws();
                    match self.peek() {
                        Some('}') => {
                            self.trailing_comma(comma, '}');
                            self.i += 1;
                            break;
                        }
                        None => {
                            self.unterminated(open, '{');
                            break;
                        }
                        _ => {}
                    }
                }
                Some('}') => {
                    self.i += 1;
                    break;
                }
                Some(']') => {
                    self.mismatched(self.i, '}');
                    self.i += 1; // consume it, or the leftover ']' reads as trailing content
                    break;
                }
                None => {
                    self.unterminated(open, '{');
                    break;
                }
                Some(_) => {
                    let at = self.i;
                    self.push(
                        at,
                        "missing-comma",
                        "missing comma",
                        "one member ends here and another begins, with no comma between them.".into(),
                        "add a comma after the previous value — the mistake is usually at the end of the line above."
                            .into(),
                    );
                }
            }
            if self.aborted {
                break;
            }
            if self.i == before {
                self.i += 1; // never spin on the same character
            }
        }
        self.depth -= 1;
    }

    fn array(&mut self) {
        let open = self.i;
        self.i += 1; // '['
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            self.push(
                open,
                "too-deep",
                "nesting too deep",
                format!("nesting goes past {MAX_DEPTH} levels, deeper than this tool (and most parsers) will follow."),
                "check for an unclosed bracket earlier in the document, which makes everything after it look nested.".into(),
            );
            self.aborted = true;
            self.depth -= 1;
            return;
        }
        loop {
            let before = self.i;
            self.skip_ws();
            match self.peek() {
                None => {
                    self.unterminated(open, '[');
                    break;
                }
                Some(']') => {
                    self.i += 1;
                    break;
                }
                _ => {}
            }
            self.value();
            if self.aborted {
                break;
            }
            self.skip_ws();
            match self.peek() {
                Some(',') => {
                    let comma = self.i;
                    self.i += 1;
                    self.skip_ws();
                    match self.peek() {
                        Some(']') => {
                            self.trailing_comma(comma, ']');
                            self.i += 1;
                            break;
                        }
                        None => {
                            self.unterminated(open, '[');
                            break;
                        }
                        _ => {}
                    }
                }
                Some(']') => {
                    self.i += 1;
                    break;
                }
                Some('}') => {
                    self.mismatched(self.i, ']');
                    self.i += 1; // consume it, or the leftover '}' reads as trailing content
                    break;
                }
                None => {
                    self.unterminated(open, '[');
                    break;
                }
                Some(_) => {
                    let at = self.i;
                    self.push(
                        at,
                        "missing-comma",
                        "missing comma",
                        "one element ends here and another begins, with no comma between them.".into(),
                        "add a comma after the previous element — the mistake is usually at the end of the line above."
                            .into(),
                    );
                }
            }
            if self.aborted {
                break;
            }
            if self.i == before {
                self.i += 1;
            }
        }
        self.depth -= 1;
    }

    fn trailing_comma(&mut self, at: usize, close: char) {
        let what = if close == '}' { "object" } else { "array" };
        self.push(
            at,
            "trailing-comma",
            "trailing comma",
            format!("this comma is the last thing in the {what}; JSON does not allow a comma before {close}."),
            format!("delete this comma (or add another value after it before the {close})."),
        );
    }

    fn mismatched(&mut self, at: usize, expected: char) {
        let found = self.c.get(at).map(|x| x.c).unwrap_or(expected);
        self.push(
            at,
            "mismatched-bracket",
            "mismatched bracket",
            format!("'{found}' closes the wrong kind of container here; '{expected}' was expected."),
            format!("change '{found}' to '{expected}', or fix the opening bracket it was meant to match."),
        );
    }

    fn unterminated(&mut self, open: usize, opener: char) {
        let (kind, label, close) = if opener == '{' {
            ("unterminated-object", "unclosed object", '}')
        } else {
            ("unterminated-array", "unclosed array", ']')
        };
        self.push(
            open,
            kind,
            label,
            format!("the '{opener}' opened here is never closed — the document ends first."),
            format!("add the missing '{close}' (count the brackets from this line down)."),
        );
    }

    /// An object member name.
    fn key(&mut self) {
        match self.peek() {
            Some('"') => self.string(),
            Some('\'') => {
                let at = self.i;
                self.push(
                    at,
                    "single-quote",
                    "single-quoted key",
                    "a member name must be a string in double quotes; single quotes are not JSON."
                        .into(),
                    "replace the surrounding ' with \".".into(),
                );
                self.quoted('\'');
            }
            Some(c) if c.is_alphanumeric() || c == '_' || c == '$' => {
                let at = self.i;
                let mut name = String::new();
                while let Some(ch) = self.peek() {
                    if ch.is_alphanumeric() || ch == '_' || ch == '$' || ch == '-' || ch == '.' {
                        name.push(ch);
                        self.i += 1;
                    } else {
                        break;
                    }
                }
                self.push(
                    at,
                    "unquoted-key",
                    "unquoted key",
                    format!("the member name {name} is not quoted; JSON requires double quotes around every name."),
                    format!("write \"{name}\": instead of {name}:"),
                );
            }
            None => {
                let at = self.i;
                self.push(
                    at,
                    "unexpected-end",
                    "unexpected end of input",
                    "a member name was expected here, but the document ended.".into(),
                    "close the object with } or add the missing \"name\": value pair.".into(),
                );
            }
            Some(c) => {
                let at = self.i;
                self.i += 1;
                self.push(
                    at,
                    "unexpected-token",
                    "unexpected character",
                    format!("'{c}' cannot start a member name."),
                    "a member name must be a string in double quotes, e.g. \"id\": 1".into(),
                );
            }
        }
    }

    /// A double-quoted string, checking escapes and raw control characters.
    fn string(&mut self) {
        let open = self.i;
        self.i += 1; // opening quote
        loop {
            let ch = match self.c.get(self.i) {
                None => {
                    self.push(
                        open,
                        "unterminated-string",
                        "unterminated string",
                        "the string opened here has no closing double quote before the end of the document."
                            .into(),
                        "add the closing \" (a stray \" earlier in the text usually causes this)."
                            .into(),
                    );
                    // Everything after an unterminated string was swallowed by it,
                    // so any further finding would be an artefact.
                    self.aborted = true;
                    return;
                }
                Some(ch) => *ch,
            };
            match ch.c {
                '"' => {
                    self.i += 1;
                    return;
                }
                '\\' => {
                    let esc = self.i;
                    self.i += 1;
                    match self.peek() {
                        None => {
                            self.push(
                                open,
                                "unterminated-string",
                                "unterminated string",
                                "the string opened here has no closing double quote before the end of the document."
                                    .into(),
                                "add the closing \".".into(),
                            );
                            return;
                        }
                        Some(e) if "\"\\/bfnrt".contains(e) => self.i += 1,
                        Some('u') => {
                            self.i += 1;
                            let mut n = 0;
                            while n < 4
                                && self.peek().map(|c| c.is_ascii_hexdigit()) == Some(true)
                            {
                                self.i += 1;
                                n += 1;
                            }
                            if n < 4 {
                                self.push(
                                    esc,
                                    "bad-escape",
                                    "invalid \\u escape",
                                    "\\u must be followed by exactly four hexadecimal digits, e.g. \\u00e9."
                                        .into(),
                                    "complete the four hex digits, or escape the backslash itself as \\\\u."
                                        .into(),
                                );
                            }
                        }
                        Some(e) => {
                            self.i += 1;
                            self.push(
                                esc,
                                "bad-escape",
                                "invalid escape sequence",
                                format!("\\{e} is not a JSON escape; only \\\" \\\\ \\/ \\b \\f \\n \\r \\t and \\uXXXX are allowed."),
                                format!("write a literal backslash as \\\\{e} (Windows paths need \\\\), or use a valid escape."),
                            );
                        }
                    }
                }
                c if (c as u32) < 0x20 => {
                    let at = self.i;
                    self.i += 1;
                    let (what, esc) = match c {
                        '\n' => ("a line break", "\\n"),
                        '\r' => ("a carriage return", "\\r"),
                        '\t' => ("a tab", "\\t"),
                        _ => ("a control character", "\\uXXXX"),
                    };
                    self.push(
                        at,
                        "control-char",
                        "unescaped control character",
                        format!("{what} appears raw inside a string; JSON strings may not contain unescaped characters below U+0020."),
                        format!("escape it as {esc}."),
                    );
                }
                _ => self.i += 1,
            }
        }
    }

    /// Consume a string delimited by `q` (used for the single-quote recovery path).
    fn quoted(&mut self, q: char) {
        self.i += 1;
        while let Some(c) = self.peek() {
            if c == '\\' {
                self.i += 2;
                continue;
            }
            self.i += 1;
            if c == q {
                return;
            }
        }
    }

    fn number(&mut self) {
        let at = self.i;
        if self.peek() == Some('-') && self.peek_at(1).map(|c| c.is_alphabetic()) == Some(true) {
            self.i += 1; // signed keyword such as -Infinity
            self.ident();
            return;
        }
        let mut txt = String::new();
        while let Some(c) = self.peek() {
            if c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.' {
                txt.push(c);
                self.i += 1;
            } else {
                break;
            }
        }
        if valid_json_number(&txt) {
            return;
        }
        let reason = if txt.starts_with('+') {
            "JSON numbers may not start with a plus sign."
        } else if txt.starts_with('.') {
            "a number needs a digit before the decimal point (write 0.5, not .5)."
        } else if txt.ends_with('.') {
            "a number needs at least one digit after the decimal point (write 1.0, not 1.)."
        } else if txt.contains('x') || txt.contains('X') {
            "hexadecimal literals are not valid JSON."
        } else if txt.len() > 1 && (txt.starts_with('0') || txt.starts_with("-0"))
            && txt
                .trim_start_matches('-')
                .trim_start_matches('0')
                .starts_with(|c: char| c.is_ascii_digit())
        {
            "a number may not have leading zeros (write 7, not 007)."
        } else {
            "this is not a valid JSON number."
        };
        self.push(
            at,
            "invalid-number",
            "invalid number",
            format!("'{txt}' is not a number JSON accepts: {reason}"),
            "write it as an ordinary decimal number (-12, 3.14, 1e-5), or quote it as a string."
                .into(),
        );
    }

    /// A bare word: `true`/`false`/`null` are fine, everything else is not.
    fn ident(&mut self) {
        let at = self.i;
        let mut word = String::new();
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '_' {
                word.push(c);
                self.i += 1;
            } else {
                break;
            }
        }
        match word.as_str() {
            "true" | "false" | "null" => {}
            "NaN" | "Infinity" | "inf" | "nan" => self.push(
                at,
                "invalid-literal",
                "non-finite number",
                format!("{word} is a JavaScript/Python value, not JSON — JSON numbers are always finite."),
                "use null, or quote it as a string such as \"NaN\".".into(),
            ),
            "undefined" | "None" | "nil" | "NULL" | "Null" => self.push(
                at,
                "invalid-literal",
                "invalid literal",
                format!("{word} is not a JSON value; JSON's only empty value is lowercase null."),
                "replace it with null (or omit the member entirely).".into(),
            ),
            "True" | "False" | "TRUE" | "FALSE" => self.push(
                at,
                "invalid-literal",
                "wrong-case boolean",
                format!("{word} is not JSON; JSON booleans are lowercase true and false."),
                format!("write {}.", word.to_ascii_lowercase()),
            ),
            _ => self.push(
                at,
                "unquoted-string",
                "unquoted string value",
                format!("the bare word {word} is not a JSON value."),
                format!("quote it as \"{word}\" if it is text, or use a number/true/false/null."),
            ),
        }
    }
}

/// Strict JSON number grammar: `-? (0 | [1-9][0-9]*) (. [0-9]+)? ([eE][+-]?[0-9]+)?`
fn valid_json_number(s: &str) -> bool {
    let b = s.as_bytes();
    let mut i = 0usize;
    if i < b.len() && b[i] == b'-' {
        i += 1;
    }
    let int_start = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    if i == int_start {
        return false;
    }
    if b[int_start] == b'0' && i - int_start > 1 {
        return false; // leading zero
    }
    if i < b.len() && b[i] == b'.' {
        i += 1;
        let frac_start = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i == frac_start {
            return false;
        }
    }
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        i += 1;
        if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
            i += 1;
        }
        let exp_start = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i == exp_start {
            return false;
        }
    }
    i == b.len()
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

/// Source lines with a caret under `column` of `line`, `ctx` lines either side.
fn context_block(text: &str, line: usize, column: usize, ctx: usize) -> Vec<String> {
    if ctx == 0 {
        return Vec::new();
    }
    let lines: Vec<&str> = text.split('\n').collect();
    if line == 0 || line > lines.len() {
        return Vec::new();
    }
    let first = line.saturating_sub(ctx).max(1);
    let last = (line + ctx).min(lines.len());
    let width = last.to_string().len();
    let mut out = Vec::new();
    for n in first..=last {
        let raw = lines[n - 1].trim_end_matches('\r').replace('\t', " ");
        if n == line {
            let (shown, caret_col) = window(&raw, column);
            out.push(format!("{n:>width$} | {shown}"));
            out.push(format!(
                "{:>width$} | {}^",
                "",
                " ".repeat(caret_col.saturating_sub(1))
            ));
        } else {
            let chars: Vec<char> = raw.chars().collect();
            let shown = if chars.len() > MAX_CONTEXT_WIDTH {
                format!(
                    "{}…",
                    chars[..MAX_CONTEXT_WIDTH].iter().collect::<String>()
                )
            } else {
                raw
            };
            out.push(format!("{n:>width$} | {shown}"));
        }
    }
    out
}

/// Window a long line around `column`, returning the text to show and the
/// 1-based caret column within it.
fn window(raw: &str, column: usize) -> (String, usize) {
    let chars: Vec<char> = raw.chars().collect();
    if chars.len() <= MAX_CONTEXT_WIDTH {
        return (raw.to_string(), column);
    }
    let start = column.saturating_sub(1).saturating_sub(40);
    let end = (start + MAX_CONTEXT_WIDTH).min(chars.len());
    let mut shown = String::new();
    let mut caret = column - start;
    if start > 0 {
        shown.push('…');
        caret += 1;
    }
    shown.extend(chars[start..end].iter());
    if end < chars.len() {
        shown.push('…');
    }
    (shown, caret)
}

fn render_report(text: &str, a: &Analysis, ctx: usize) -> String {
    let mut out = String::new();
    if a.valid {
        let s = a.summary.as_ref().expect("valid analysis carries a summary");
        out.push_str("Valid JSON — no syntax errors found.\n\n");
        let shape = match s.kind {
            "object" => format!("object with {} member(s)", s.members),
            "array" => format!("array with {} element(s)", s.members),
            other => format!("{other} (a single scalar value)"),
        };
        out.push_str(&format!("Top-level value: {shape}\n"));
        out.push_str(&format!("Nesting depth:   {} level(s)\n", s.depth));
        out.push_str(&format!("Size:            {} line(s), {} bytes\n", s.lines, s.bytes));
        return out;
    }

    let n = a.issues.len();
    out.push_str(&format!(
        "Invalid JSON — {n} issue{} found.\n",
        if n == 1 { "" } else { "s" }
    ));
    if let Some((line, column, _)) = &a.parser_stop {
        out.push_str(&format!("A parser stops at line {line}, column {column}.\n"));
    }
    for (idx, issue) in a.issues.iter().enumerate() {
        out.push_str(&format!(
            "\n{}. Line {}, column {} (offset {}) — {}\n",
            idx + 1,
            issue.line,
            issue.column,
            issue.offset,
            issue.label
        ));
        out.push_str(&format!("   Cause: {}\n", issue.explain));
        out.push_str(&format!("   Fix:   {}\n", issue.fix));
        let block = context_block(text, issue.line, issue.column, ctx);
        if !block.is_empty() {
            out.push('\n');
            for l in block {
                out.push_str("   ");
                out.push_str(&l);
                out.push('\n');
            }
        }
    }
    out.push_str(
        "\nNote: a position marks where the problem was detected, which can be just after the real mistake — for a missing comma or bracket, check the end of the previous line.\n",
    );
    out
}

fn render_json(text: &str, a: &Analysis, ctx: usize) -> String {
    let issues: Vec<Value> = a
        .issues
        .iter()
        .map(|i| {
            let mut o = json!({
                "kind": i.kind,
                "label": i.label,
                "line": i.line,
                "column": i.column,
                "offset": i.offset,
                "explain": i.explain,
                "fix": i.fix,
            });
            let block = context_block(text, i.line, i.column, ctx);
            if !block.is_empty() {
                o["snippet"] = Value::String(block.join("\n"));
            }
            o
        })
        .collect();
    let mut doc = json!({
        "valid": a.valid,
        "issue_count": a.issues.len(),
        "issues": issues,
    });
    if let Some((line, column, message)) = &a.parser_stop {
        doc["parser_stop"] = json!({ "line": line, "column": column, "message": message });
    }
    if let Some(s) = &a.summary {
        doc["summary"] = json!({
            "type": s.kind,
            "members": s.members,
            "depth": s.depth,
            "lines": s.lines,
            "bytes": s.bytes,
        });
    }
    serde_json::to_string_pretty(&doc).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn kinds(json: &str) -> Vec<&'static str> {
        analyze(json).issues.iter().map(|i| i.kind).collect()
    }

    #[test]
    fn valid_json_reports_a_clean_summary() {
        let out = locate("{\n  \"a\": [1, 2]\n}", "report", 2, true).unwrap();
        assert!(out.starts_with("Valid JSON — no syntax errors found."));
        assert!(out.contains("object with 1 member(s)"));
        assert!(out.contains("Nesting depth:   2 level(s)"));
    }

    #[test]
    fn empty_input_is_an_error() {
        assert_eq!(locate("   ", "report", 2, true).unwrap_err(), "no JSON input");
    }

    #[test]
    fn unknown_output_is_an_error() {
        assert!(locate("1", "xml", 2, true)
            .unwrap_err()
            .contains("unknown output"));
    }

    #[test]
    fn blank_output_falls_back_to_report() {
        assert!(locate("1", "", 0, true).unwrap().starts_with("Valid JSON"));
    }

    #[test]
    fn trailing_comma_in_object_is_located() {
        let a = analyze("{\n  \"a\": 1,\n}");
        assert!(!a.valid);
        assert_eq!(a.issues.len(), 1);
        let i = &a.issues[0];
        assert_eq!(i.kind, "trailing-comma");
        // `  "a": 1,` — the comma is the 9th character of line 2.
        assert_eq!((i.line, i.column), (2, 9));
        assert!(i.fix.contains("delete this comma"));
    }

    #[test]
    fn trailing_comma_in_array_is_located() {
        let a = analyze("[1, 2, ]");
        assert_eq!(a.issues.len(), 1);
        assert_eq!(a.issues[0].kind, "trailing-comma");
        assert_eq!((a.issues[0].line, a.issues[0].column), (1, 6));
    }

    #[test]
    fn unquoted_key_is_named() {
        let a = analyze("{ name: \"ada\" }");
        assert_eq!(a.issues[0].kind, "unquoted-key");
        assert_eq!(a.issues[0].column, 3);
        assert!(a.issues[0].fix.contains("\"name\":"));
    }

    #[test]
    fn bad_escape_is_named() {
        let a = analyze(r#"{"path": "C:\Users"}"#);
        assert_eq!(a.issues[0].kind, "bad-escape");
        assert!(a.issues[0].explain.contains("\\U is not a JSON escape"));
    }

    #[test]
    fn short_unicode_escape_is_named() {
        assert_eq!(kinds(r#""\u12""#), vec!["bad-escape"]);
    }

    #[test]
    fn single_quotes_are_named() {
        let a = analyze("{'a': 1}");
        assert_eq!(a.issues[0].kind, "single-quote");
    }

    #[test]
    fn missing_comma_is_named() {
        let a = analyze("{\n  \"a\": 1\n  \"b\": 2\n}");
        assert_eq!(a.issues[0].kind, "missing-comma");
        assert_eq!(a.issues[0].line, 3);
    }

    #[test]
    fn unterminated_string_points_at_the_opening_quote() {
        let a = analyze("{\"a\": \"unfinished}");
        assert_eq!(a.issues[0].kind, "unterminated-string");
        assert_eq!(a.issues[0].column, 7);
    }

    #[test]
    fn raw_newline_in_string_is_a_control_char() {
        assert_eq!(kinds("{\"a\": \"one\ntwo\"}"), vec!["control-char"]);
    }

    #[test]
    fn unclosed_object_points_at_the_opening_brace() {
        let a = analyze("{\"a\": {\"b\": 1}");
        assert_eq!(a.issues[0].kind, "unterminated-object");
        assert_eq!(a.issues[0].column, 1);
    }

    #[test]
    fn invalid_literals_are_named() {
        assert_eq!(kinds("{\"a\": undefined}"), vec!["invalid-literal"]);
        assert_eq!(kinds("{\"a\": NaN}"), vec!["invalid-literal"]);
        assert_eq!(kinds("{\"a\": True}"), vec!["invalid-literal"]);
    }

    #[test]
    fn invalid_numbers_are_named() {
        assert_eq!(kinds("{\"a\": .5}"), vec!["invalid-number"]);
        assert_eq!(kinds("{\"a\": 007}"), vec!["invalid-number"]);
        assert_eq!(kinds("{\"a\": 0x1F}"), vec!["invalid-number"]);
        assert_eq!(kinds("{\"a\": +1}"), vec!["invalid-number"]);
    }

    #[test]
    fn comments_are_named() {
        assert_eq!(kinds("{\n  // note\n  \"a\": 1\n}"), vec!["comment"]);
        assert_eq!(kinds("{ /* note */ \"a\": 1 }"), vec!["comment"]);
    }

    #[test]
    fn trailing_content_is_named() {
        assert_eq!(kinds("{\"a\": 1} {\"b\": 2}"), vec!["trailing-content"]);
    }

    #[test]
    fn mismatched_bracket_is_named() {
        assert_eq!(kinds("{\"a\": 1]"), vec!["mismatched-bracket"]);
    }

    #[test]
    fn several_issues_are_reported_at_once() {
        let a = analyze("{\n  name: 'ada',\n  age: 36,\n}");
        let ks = a.issues.iter().map(|i| i.kind).collect::<Vec<_>>();
        assert!(ks.contains(&"unquoted-key"));
        assert!(ks.contains(&"single-quote"));
        assert!(ks.contains(&"trailing-comma"));
        assert!(a.issues.len() >= 4, "got {ks:?}");
    }

    #[test]
    fn scan_all_false_keeps_only_the_first_issue() {
        let out = locate("{\n  name: 'ada',\n}", "json", 0, false).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["issue_count"], 1);
        assert_eq!(v["issues"][0]["kind"], "unquoted-key");
    }

    #[test]
    fn report_renders_a_caret_under_the_column() {
        let out = locate("{\n  \"a\": 1,\n}", "report", 1, true).unwrap();
        assert!(out.contains("2 |   \"a\": 1,"));
        // Caret sits under column 9 of the gutter-aligned source line.
        assert!(out.contains("  |         ^"), "{out}");
    }

    #[test]
    fn context_lines_zero_omits_the_snippet() {
        let out = locate("{\n  \"a\": 1,\n}", "report", 0, true).unwrap();
        assert!(!out.contains(" | "));
        assert!(out.contains("trailing comma"));
    }

    #[test]
    fn json_output_is_machine_readable() {
        let out = locate("[1, 2, ]", "json", 2, true).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["valid"], false);
        assert_eq!(v["issue_count"], 1);
        assert_eq!(v["issues"][0]["kind"], "trailing-comma");
        assert_eq!(v["issues"][0]["line"], 1);
        assert_eq!(v["issues"][0]["column"], 6);
        assert_eq!(v["issues"][0]["offset"], 5);
        assert!(v["parser_stop"]["line"].is_number());
    }

    #[test]
    fn json_output_for_valid_input_carries_a_summary() {
        let out = locate("{\"a\": {\"b\": [1]}}", "json", 2, true).unwrap();
        let v: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["valid"], true);
        assert_eq!(v["summary"]["type"], "object");
        assert_eq!(v["summary"]["depth"], 3);
    }

    #[test]
    fn oversized_input_is_rejected() {
        let big = format!("\"{}\"", "x".repeat(MAX_INPUT_BYTES));
        assert!(locate(&big, "report", 2, true)
            .unwrap_err()
            .contains("the limit is"));
    }

    #[test]
    fn columns_count_characters_not_bytes() {
        // "héllo" is 6 bytes but 5 characters; the trailing comma is at column 12.
        let a = analyze("{\"a\": \"héllo\",}");
        assert_eq!(a.issues[0].kind, "trailing-comma");
        assert_eq!(a.issues[0].column, 14);
    }

    #[test]
    fn deeply_nested_input_does_not_panic() {
        let deep = format!("{}1{}", "[".repeat(400), "]".repeat(400));
        let a = analyze(&deep);
        assert!(!a.valid);
        assert_eq!(a.issues[0].kind, "too-deep");
    }

    #[test]
    fn number_grammar_matches_the_spec() {
        for good in ["0", "-0", "12", "3.14", "1e5", "1E-5", "-2.5e+10"] {
            assert!(valid_json_number(good), "{good} should be valid");
        }
        for bad in ["", "+1", ".5", "1.", "01", "1e", "0x1F", "1.2.3", "--1"] {
            assert!(!valid_json_number(bad), "{bad} should be invalid");
        }
    }
}
