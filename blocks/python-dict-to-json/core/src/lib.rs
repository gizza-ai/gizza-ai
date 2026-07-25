//! python-dict-to-json core — convert a Python dict/list literal (as printed by
//! `print(obj)` / `repr(obj)`) into valid JSON. Pure compute, shared by the chat
//! skill block and the web page; no wafer/wasm-bindgen deps.
//!
//! A hand-written recursive-descent parser accepts the Python literal grammar and
//! its conveniences that plain JSON rejects:
//! - `True` / `False` / `None`            → `true` / `false` / `null`
//! - single-quoted, triple-quoted, `r"..."` (raw), `b"..."` (bytes), `f"..."`,
//!   `u"..."` string prefixes
//! - tuples `(1, 2)` and sets `{1, 2}`    → JSON arrays
//! - trailing commas, `#` line comments
//! - non-decimal ints `0xff` / `0o17` / `0b101` and digit separators `1_000`
//! - non-string dict keys (`1`, `True`, `None`) → stringified JSON keys
//!
//! Output key order is preserved (serde_json `preserve_order`); the serializer is
//! hand-rolled so `indent`, `sort_keys`, and `ensure_ascii` are all honored.

use serde_json::{Map, Number, Value};

/// Maximum accepted input size in bytes (2 MB). The browser page and chat sandbox
/// share small memory budgets, so anything larger is rejected with an actionable
/// error rather than risking an out-of-memory trap.
pub const MAX_INPUT_BYTES: usize = 2 * 1024 * 1024;

/// Maximum nesting depth accepted (stack-safety in wasm). 200 levels parse; 201 is
/// an error — the exact boundary is unit- and page-tested.
pub const MAX_DEPTH: usize = 200;

/// Convert a Python literal `input` into a formatted JSON string.
///
/// - `indent`: `"2"` / `"4"` spaces, `"tab"`, or `"minify"` for a single compact
///   line. Blank or unknown → `"2"`.
/// - `sort_keys`: sort every object's keys lexicographically (like Python's
///   `json.dumps(..., sort_keys=True)`).
/// - `ensure_ascii`: escape every non-ASCII character as `\uXXXX` (Python's
///   `json.dumps` default). When false, UTF-8 is emitted verbatim.
pub fn convert(input: &str, indent: &str, sort_keys: bool, ensure_ascii: bool) -> Result<String, String> {
    if input.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "input is {} bytes, over the {} byte ({} MB) limit",
            input.len(),
            MAX_INPUT_BYTES,
            MAX_INPUT_BYTES / (1024 * 1024)
        ));
    }
    if input.trim().is_empty() {
        return Err("input is empty — paste a Python dict or list literal".into());
    }

    let mut value = Parser::new(input).parse_document()?;
    if sort_keys {
        sort_value(&mut value);
    }

    let mode = IndentMode::parse(indent);
    let mut out = String::new();
    write_value(&mut out, &value, mode, 0, ensure_ascii);
    Ok(out)
}

/// Output formatting mode derived from the `indent` argument.
#[derive(Clone, Copy)]
enum IndentMode {
    Minify,
    Spaces(usize),
    Tab,
}

impl IndentMode {
    fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "minify" | "compact" | "none" | "0" => IndentMode::Minify,
            "4" => IndentMode::Spaces(4),
            "tab" | "\t" => IndentMode::Tab,
            // Blank or "2" or anything else → the friendly default.
            _ => IndentMode::Spaces(2),
        }
    }

    fn is_pretty(self) -> bool {
        !matches!(self, IndentMode::Minify)
    }

    fn push_indent(self, out: &mut String, level: usize) {
        match self {
            IndentMode::Minify => {}
            IndentMode::Spaces(n) => {
                for _ in 0..(n * level) {
                    out.push(' ');
                }
            }
            IndentMode::Tab => {
                for _ in 0..level {
                    out.push('\t');
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

struct Parser {
    chars: Vec<char>,
    pos: usize,
    depth: usize,
}

impl Parser {
    fn new(input: &str) -> Self {
        Parser { chars: input.chars().collect(), pos: 0, depth: 0 }
    }

    fn parse_document(&mut self) -> Result<Value, String> {
        let value = self.parse_value()?;
        self.skip_ws();
        if self.pos < self.chars.len() {
            return Err(format!(
                "unexpected trailing input at character {} ('{}') — expected a single Python literal",
                self.pos + 1,
                self.chars[self.pos]
            ));
        }
        Ok(value)
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn peek_at(&self, offset: usize) -> Option<char> {
        self.chars.get(self.pos + offset).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.chars.get(self.pos).copied();
        if c.is_some() {
            self.pos += 1;
        }
        c
    }

    /// Skip whitespace and `#...` line comments. Backslash line-continuations
    /// (`\` at end of line) are also swallowed so multi-line pastes work.
    fn skip_ws(&mut self) {
        while let Some(c) = self.peek() {
            if c == '#' {
                while let Some(c) = self.peek() {
                    if c == '\n' {
                        break;
                    }
                    self.pos += 1;
                }
            } else if c == '\\' && matches!(self.peek_at(1), Some('\n') | Some('\r')) {
                self.pos += 1;
            } else if c.is_whitespace() {
                self.pos += 1;
            } else {
                break;
            }
        }
    }

    fn parse_value(&mut self) -> Result<Value, String> {
        self.skip_ws();
        let c = self.peek().ok_or_else(|| "unexpected end of input — expected a value".to_string())?;
        match c {
            '{' => self.parse_brace(),
            '[' => self.parse_seq('[', ']').map(Value::Array),
            '(' => self.parse_tuple(),
            '\'' | '"' => self.parse_string().map(Value::String),
            '+' | '-' | '.' | '0'..='9' => self.parse_number(),
            c if c.is_alphabetic() || c == '_' => self.parse_word(),
            _ => Err(format!("unexpected character '{}' at position {}", c, self.pos + 1)),
        }
    }

    fn enter(&mut self) -> Result<(), String> {
        self.depth += 1;
        if self.depth > MAX_DEPTH {
            return Err(format!("nesting deeper than {} levels is not supported", MAX_DEPTH));
        }
        Ok(())
    }

    /// `{ ... }` — a dict if the first item is followed by `:`, otherwise a set
    /// (rendered as a JSON array). Empty `{}` is a dict, matching Python.
    fn parse_brace(&mut self) -> Result<Value, String> {
        self.enter()?;
        self.bump(); // consume '{'
        self.skip_ws();
        if self.peek() == Some('}') {
            self.bump();
            self.depth -= 1;
            return Ok(Value::Object(Map::new()));
        }

        let first = self.parse_value()?;
        self.skip_ws();
        let result = if self.peek() == Some(':') {
            // Dict.
            self.bump(); // ':'
            let mut map = Map::new();
            let val = self.parse_value()?;
            map.insert(key_to_string(&first)?, val);
            loop {
                self.skip_ws();
                match self.peek() {
                    Some('}') => {
                        self.bump();
                        break;
                    }
                    Some(',') => {
                        self.bump();
                        self.skip_ws();
                        if self.peek() == Some('}') {
                            self.bump();
                            break;
                        }
                        let key = self.parse_value()?;
                        self.skip_ws();
                        if self.peek() != Some(':') {
                            return Err(self.expected(": after a dict key"));
                        }
                        self.bump();
                        let val = self.parse_value()?;
                        map.insert(key_to_string(&key)?, val);
                    }
                    _ => return Err(self.expected(", or }")),
                }
            }
            Value::Object(map)
        } else {
            // Set → array.
            let mut arr = vec![first];
            loop {
                self.skip_ws();
                match self.peek() {
                    Some('}') => {
                        self.bump();
                        break;
                    }
                    Some(',') => {
                        self.bump();
                        self.skip_ws();
                        if self.peek() == Some('}') {
                            self.bump();
                            break;
                        }
                        arr.push(self.parse_value()?);
                    }
                    _ => return Err(self.expected(", or }")),
                }
            }
            Value::Array(arr)
        };
        self.depth -= 1;
        Ok(result)
    }

    /// `[ ... ]` list or the body of a set — a comma-separated value sequence with
    /// an optional trailing comma, terminated by `close`.
    fn parse_seq(&mut self, open: char, close: char) -> Result<Vec<Value>, String> {
        self.enter()?;
        debug_assert_eq!(self.peek(), Some(open));
        self.bump(); // consume open
        let mut items = Vec::new();
        loop {
            self.skip_ws();
            match self.peek() {
                Some(c) if c == close => {
                    self.bump();
                    break;
                }
                None => return Err(format!("unclosed '{}' — expected '{}'", open, close)),
                _ => {
                    items.push(self.parse_value()?);
                    self.skip_ws();
                    match self.peek() {
                        Some(',') => {
                            self.bump();
                        }
                        Some(c) if c == close => {
                            self.bump();
                            break;
                        }
                        _ => return Err(self.expected(&format!(", or {}", close))),
                    }
                }
            }
        }
        self.depth -= 1;
        Ok(items)
    }

    /// `( ... )` — a tuple becomes a JSON array. A single parenthesized value with
    /// no trailing comma is just that value (Python: `(x)` == `x`); `(x,)`, `()`,
    /// and multi-element tuples become arrays.
    fn parse_tuple(&mut self) -> Result<Value, String> {
        self.enter()?;
        self.bump(); // consume '('
        let mut items = Vec::new();
        let mut trailing_comma = false;
        loop {
            self.skip_ws();
            match self.peek() {
                Some(')') => {
                    self.bump();
                    break;
                }
                None => return Err("unclosed '(' — expected ')'".to_string()),
                _ => {
                    items.push(self.parse_value()?);
                    trailing_comma = false;
                    self.skip_ws();
                    match self.peek() {
                        Some(',') => {
                            self.bump();
                            trailing_comma = true;
                        }
                        Some(')') => {
                            self.bump();
                            break;
                        }
                        _ => return Err(self.expected(", or )")),
                    }
                }
            }
        }
        self.depth -= 1;
        if items.len() == 1 && !trailing_comma {
            Ok(items.into_iter().next().unwrap())
        } else {
            Ok(Value::Array(items))
        }
    }

    /// A bare word: the constants `True`/`False`/`None` (JSON `true`/`false`/`null`)
    /// or a string prefix (`r`, `b`, `u`, `f` and combinations) immediately
    /// followed by a quote.
    fn parse_word(&mut self) -> Result<Value, String> {
        // Look ahead for a string prefix: up to two of r/b/u/f then a quote.
        let mut n = 0;
        while n < 2 {
            match self.peek_at(n) {
                Some(c) if matches!(c.to_ascii_lowercase(), 'r' | 'b' | 'u' | 'f') => n += 1,
                _ => break,
            }
        }
        if n > 0 && matches!(self.peek_at(n), Some('\'') | Some('"')) {
            let raw = (0..n).any(|i| matches!(self.peek_at(i), Some('r') | Some('R')));
            self.pos += n;
            return self.parse_string_inner(raw).map(Value::String);
        }

        // Otherwise a keyword identifier.
        let start = self.pos;
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '_' {
                self.pos += 1;
            } else {
                break;
            }
        }
        let word: String = self.chars[start..self.pos].iter().collect();
        match word.as_str() {
            "True" => Ok(Value::Bool(true)),
            "False" => Ok(Value::Bool(false)),
            "None" => Ok(Value::Null),
            other => Err(format!(
                "unknown identifier '{}' at position {} — only True, False, None and quoted strings are valid",
                other,
                start + 1
            )),
        }
    }

    fn parse_string(&mut self) -> Result<String, String> {
        self.parse_string_inner(false)
    }

    /// Parse a (possibly triple-quoted) string. `raw` disables backslash escape
    /// processing. The opening quote is at the current position.
    fn parse_string_inner(&mut self, raw: bool) -> Result<String, String> {
        let quote = self.bump().expect("caller ensured a quote");
        let triple = self.peek() == Some(quote) && self.peek_at(1) == Some(quote);
        if triple {
            self.pos += 2;
        }
        let mut s = String::new();
        loop {
            let c = match self.peek() {
                Some(c) => c,
                None => return Err("unterminated string literal".to_string()),
            };
            if c == quote {
                if triple {
                    if self.peek_at(1) == Some(quote) && self.peek_at(2) == Some(quote) {
                        self.pos += 3;
                        return Ok(s);
                    }
                    s.push(c);
                    self.pos += 1;
                } else {
                    self.pos += 1;
                    return Ok(s);
                }
            } else if c == '\\' && !raw {
                self.pos += 1;
                self.read_escape(&mut s)?;
            } else if c == '\\' && raw {
                // Raw string: keep the backslash, but a backslash still escapes the
                // closing quote for tokenizing (Python keeps both chars).
                s.push('\\');
                self.pos += 1;
                if let Some(next) = self.peek() {
                    s.push(next);
                    self.pos += 1;
                }
            } else {
                s.push(c);
                self.pos += 1;
            }
        }
    }

    /// Handle the character(s) after a backslash in a non-raw string.
    fn read_escape(&mut self, out: &mut String) -> Result<(), String> {
        let c = match self.bump() {
            Some(c) => c,
            None => return Err("string ends with a dangling backslash".to_string()),
        };
        match c {
            'n' => out.push('\n'),
            't' => out.push('\t'),
            'r' => out.push('\r'),
            '\\' => out.push('\\'),
            '\'' => out.push('\''),
            '"' => out.push('"'),
            '0' => out.push('\0'),
            'a' => out.push('\u{07}'),
            'b' => out.push('\u{08}'),
            'f' => out.push('\u{0C}'),
            'v' => out.push('\u{0B}'),
            '\n' => {} // line continuation inside a string
            'x' => self.read_hex_escape(out, 2)?,
            'u' => self.read_hex_escape(out, 4)?,
            'U' => self.read_hex_escape(out, 8)?,
            // Unknown escape: Python keeps the backslash and the character.
            other => {
                out.push('\\');
                out.push(other);
            }
        }
        Ok(())
    }

    fn read_hex_escape(&mut self, out: &mut String, digits: usize) -> Result<(), String> {
        let mut code: u32 = 0;
        for _ in 0..digits {
            let d = self
                .peek()
                .and_then(|c| c.to_digit(16))
                .ok_or_else(|| format!("invalid \\{} escape — expected {} hex digits", if digits == 2 { "x" } else { "u" }, digits))?;
            code = code * 16 + d;
            self.pos += 1;
        }
        // Lone surrogates and out-of-range code points fall back to U+FFFD.
        out.push(char::from_u32(code).unwrap_or('\u{FFFD}'));
        Ok(())
    }

    fn parse_number(&mut self) -> Result<Value, String> {
        let start = self.pos;
        let mut sign = 1i8;
        if matches!(self.peek(), Some('+') | Some('-')) {
            if self.peek() == Some('-') {
                sign = -1;
            }
            self.pos += 1;
        }

        // Detect a base prefix: 0x / 0o / 0b.
        let base = if self.peek() == Some('0') {
            match self.peek_at(1).map(|c| c.to_ascii_lowercase()) {
                Some('x') => Some(16),
                Some('o') => Some(8),
                Some('b') => Some(2),
                _ => None,
            }
        } else {
            None
        };

        if let Some(base) = base {
            self.pos += 2; // consume the prefix
            let digit_start = self.pos;
            let mut digits = String::new();
            while let Some(c) = self.peek() {
                if c == '_' {
                    self.pos += 1;
                } else if c.to_digit(base as u32).is_some() {
                    digits.push(c);
                    self.pos += 1;
                } else {
                    break;
                }
            }
            if digits.is_empty() {
                return Err(format!("malformed number at position {}", digit_start + 1));
            }
            let magnitude = i128::from_str_radix(&digits, base as u32)
                .map_err(|_| format!("integer literal at position {} is too large", start + 1))?;
            return int_value(magnitude * sign as i128, start);
        }

        // Decimal / float. Scan the numeric token, allowing digit separators and a
        // signed exponent.
        let mut is_float = false;
        let mut has_digits = false;
        while let Some(c) = self.peek() {
            match c {
                '0'..='9' => {
                    has_digits = true;
                    self.pos += 1;
                }
                '_' => self.pos += 1,
                '.' => {
                    is_float = true;
                    self.pos += 1;
                }
                'e' | 'E' => {
                    is_float = true;
                    self.pos += 1;
                    if matches!(self.peek(), Some('+') | Some('-')) {
                        self.pos += 1;
                    }
                }
                'j' | 'J' => {
                    return Err(format!(
                        "complex number at position {} cannot be represented in JSON",
                        start + 1
                    ));
                }
                _ => break,
            }
        }
        if !has_digits {
            return Err(format!("malformed number at position {}", start + 1));
        }
        let raw: String = self.chars[start..self.pos].iter().filter(|c| **c != '_').collect();
        if is_float {
            let f: f64 = raw
                .parse()
                .map_err(|_| format!("malformed float '{}' at position {}", raw, start + 1))?;
            let n = Number::from_f64(f)
                .ok_or_else(|| format!("float '{}' is not finite and cannot be JSON", raw))?;
            Ok(Value::Number(n))
        } else {
            let magnitude: i128 = raw
                .parse()
                .map_err(|_| format!("integer literal at position {} is too large", start + 1))?;
            int_value(magnitude, start)
        }
    }

    /// Build an "expected X, got Y" error at the current position.
    fn expected(&self, what: &str) -> String {
        match self.peek() {
            Some(c) => format!("expected {} at position {}, found '{}'", what, self.pos + 1, c),
            None => format!("expected {} but reached end of input", what),
        }
    }
}

/// Convert a parsed dict key into a JSON object key. JSON keys must be strings, so
/// scalar Python keys are stringified the way `json.dumps` does.
fn key_to_string(v: &Value) -> Result<String, String> {
    match v {
        Value::String(s) => Ok(s.clone()),
        Value::Bool(true) => Ok("true".to_string()),
        Value::Bool(false) => Ok("false".to_string()),
        Value::Null => Ok("null".to_string()),
        Value::Number(n) => Ok(n.to_string()),
        Value::Array(_) | Value::Object(_) => {
            Err("dict keys must be strings, numbers, booleans, or None — tuple/list/dict keys cannot become JSON keys".to_string())
        }
    }
}

/// Build a JSON integer `Value`, choosing i64 or u64, falling back to f64 for the
/// rare literal outside the i64/u64 range.
fn int_value(magnitude: i128, _start: usize) -> Result<Value, String> {
    if let Ok(i) = i64::try_from(magnitude) {
        Ok(Value::Number(Number::from(i)))
    } else if let Ok(u) = u64::try_from(magnitude) {
        Ok(Value::Number(Number::from(u)))
    } else {
        // Beyond 64-bit: keep the magnitude as a float (lossy but valid JSON).
        let n = Number::from_f64(magnitude as f64)
            .ok_or_else(|| "integer literal is not representable".to_string())?;
        Ok(Value::Number(n))
    }
}

/// Recursively sort every object's keys lexicographically.
fn sort_value(v: &mut Value) {
    match v {
        Value::Object(map) => {
            let mut entries: Vec<(String, Value)> = map.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            let mut sorted = Map::new();
            for (k, mut val) in entries {
                sort_value(&mut val);
                sorted.insert(k, val);
            }
            *map = sorted;
        }
        Value::Array(arr) => {
            for item in arr.iter_mut() {
                sort_value(item);
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Serializer
// ---------------------------------------------------------------------------

fn write_value(out: &mut String, value: &Value, mode: IndentMode, level: usize, ensure_ascii: bool) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Number(n) => out.push_str(&n.to_string()),
        Value::String(s) => write_json_string(out, s, ensure_ascii),
        Value::Array(arr) => {
            if arr.is_empty() {
                out.push_str("[]");
                return;
            }
            out.push('[');
            for (i, item) in arr.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                if mode.is_pretty() {
                    out.push('\n');
                    mode.push_indent(out, level + 1);
                }
                write_value(out, item, mode, level + 1, ensure_ascii);
            }
            if mode.is_pretty() {
                out.push('\n');
                mode.push_indent(out, level);
            }
            out.push(']');
        }
        Value::Object(map) => {
            if map.is_empty() {
                out.push_str("{}");
                return;
            }
            out.push('{');
            for (i, (k, v)) in map.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                if mode.is_pretty() {
                    out.push('\n');
                    mode.push_indent(out, level + 1);
                }
                write_json_string(out, k, ensure_ascii);
                out.push(':');
                if mode.is_pretty() {
                    out.push(' ');
                }
                write_value(out, v, mode, level + 1, ensure_ascii);
            }
            if mode.is_pretty() {
                out.push('\n');
                mode.push_indent(out, level);
            }
            out.push('}');
        }
    }
}

/// Write a JSON string literal (surrounding quotes included) with correct escaping.
fn write_json_string(out: &mut String, s: &str, ensure_ascii: bool) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0C}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c if ensure_ascii && (c as u32) > 0x7f => {
                let cp = c as u32;
                if cp > 0xFFFF {
                    // Encode as a UTF-16 surrogate pair.
                    let v = cp - 0x10000;
                    let hi = 0xD800 + (v >> 10);
                    let lo = 0xDC00 + (v & 0x3FF);
                    out.push_str(&format!("\\u{:04x}\\u{:04x}", hi, lo));
                } else {
                    out.push_str(&format!("\\u{:04x}", cp));
                }
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn happy_dict_pretty() {
        let got = convert("{'name': 'Ann', 'active': True, 'age': 30, 'tags': None}", "2", false, false).unwrap();
        assert_eq!(
            got,
            "{\n  \"name\": \"Ann\",\n  \"active\": true,\n  \"age\": 30,\n  \"tags\": null\n}"
        );
    }

    #[test]
    fn minify_and_single_quotes() {
        let got = convert("{'a': 1, 'b': [1, 2, 3]}", "minify", false, false).unwrap();
        assert_eq!(got, "{\"a\":1,\"b\":[1,2,3]}");
    }

    #[test]
    fn tuples_become_arrays() {
        let got = convert("(1, 2, 3)", "minify", false, false).unwrap();
        assert_eq!(got, "[1,2,3]");
        // Single parenthesized value is not a tuple.
        assert_eq!(convert("(1)", "minify", false, false).unwrap(), "1");
        // Trailing comma makes a 1-tuple.
        assert_eq!(convert("(1,)", "minify", false, false).unwrap(), "[1]");
        // Empty tuple.
        assert_eq!(convert("()", "minify", false, false).unwrap(), "[]");
    }

    #[test]
    fn set_becomes_array() {
        let got = convert("{1, 2, 3}", "minify", false, false).unwrap();
        assert_eq!(got, "[1,2,3]");
        // Empty braces are a dict, not a set.
        assert_eq!(convert("{}", "minify", false, false).unwrap(), "{}");
    }

    #[test]
    fn trailing_commas_and_comments() {
        let src = "{\n  'a': 1,  # a comment\n  'b': 2,\n}";
        assert_eq!(convert(src, "minify", false, false).unwrap(), "{\"a\":1,\"b\":2}");
    }

    #[test]
    fn non_decimal_ints_and_separators() {
        assert_eq!(convert("0xff", "minify", false, false).unwrap(), "255");
        assert_eq!(convert("0o17", "minify", false, false).unwrap(), "15");
        assert_eq!(convert("0b101", "minify", false, false).unwrap(), "5");
        assert_eq!(convert("1_000_000", "minify", false, false).unwrap(), "1000000");
        assert_eq!(convert("-42", "minify", false, false).unwrap(), "-42");
    }

    #[test]
    fn floats_and_exponents() {
        assert_eq!(convert("3.14", "minify", false, false).unwrap(), "3.14");
        assert_eq!(convert("1e3", "minify", false, false).unwrap(), "1000.0");
        assert_eq!(convert("-2.5e-2", "minify", false, false).unwrap(), "-0.025");
    }

    #[test]
    fn nonstring_keys_stringified() {
        assert_eq!(convert("{1: 'a', True: 'b', None: 'c'}", "minify", false, false).unwrap(), "{\"1\":\"a\",\"true\":\"b\",\"null\":\"c\"}");
    }

    #[test]
    fn sort_keys_option() {
        let got = convert("{'b': 1, 'a': 2, 'c': 3}", "minify", true, false).unwrap();
        assert_eq!(got, "{\"a\":2,\"b\":1,\"c\":3}");
    }

    #[test]
    fn ensure_ascii_option() {
        assert_eq!(convert("{'x': 'José'}", "minify", false, true).unwrap(), "{\"x\":\"Jos\\u00e9\"}");
        // Off: UTF-8 verbatim.
        assert_eq!(convert("{'x': 'José'}", "minify", false, false).unwrap(), "{\"x\":\"José\"}");
        // Astral plane → surrogate pair.
        assert_eq!(convert("'😀'", "minify", false, true).unwrap(), "\"\\ud83d\\ude00\"");
    }

    #[test]
    fn string_prefixes_and_triple_quotes() {
        assert_eq!(convert(r"r'a\nb'", "minify", false, false).unwrap(), "\"a\\\\nb\"");
        assert_eq!(convert("b'bytes'", "minify", false, false).unwrap(), "\"bytes\"");
        assert_eq!(convert("'''multi\nline'''", "minify", false, false).unwrap(), "\"multi\\nline\"");
    }

    #[test]
    fn indent_four_and_tab() {
        assert_eq!(convert("{'a': 1}", "4", false, false).unwrap(), "{\n    \"a\": 1\n}");
        assert_eq!(convert("{'a': 1}", "tab", false, false).unwrap(), "{\n\t\"a\": 1\n}");
    }

    #[test]
    fn err_empty_input() {
        assert!(convert("   ", "2", false, false).is_err());
    }

    #[test]
    fn err_trailing_garbage() {
        let e = convert("{'a': 1} extra", "2", false, false).unwrap_err();
        assert!(e.contains("trailing"), "got: {e}");
    }

    #[test]
    fn err_unknown_identifier() {
        let e = convert("{'a': undefined}", "2", false, false).unwrap_err();
        assert!(e.contains("undefined"), "got: {e}");
    }

    #[test]
    fn err_complex_number() {
        assert!(convert("3j", "2", false, false).is_err());
    }

    #[test]
    fn err_depth_limit() {
        let deep = format!("{}1{}", "[".repeat(MAX_DEPTH + 1), "]".repeat(MAX_DEPTH + 1));
        let e = convert(&deep, "minify", false, false).unwrap_err();
        assert!(e.contains("nesting"), "got: {e}");
    }

    #[test]
    fn err_too_large() {
        let big = format!("'{}'", "a".repeat(MAX_INPUT_BYTES + 1));
        assert!(convert(&big, "minify", false, false).is_err());
    }

    #[test]
    fn depth_limit_boundary_ok() {
        // Exactly MAX_DEPTH levels must parse.
        let ok = format!("{}1{}", "[".repeat(MAX_DEPTH), "]".repeat(MAX_DEPTH));
        assert!(convert(&ok, "minify", false, false).is_ok());
    }
}
