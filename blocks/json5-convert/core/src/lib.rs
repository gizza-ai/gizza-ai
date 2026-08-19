//! json5-convert core — a spec-shaped JSON5/JSONC parser plus strict-JSON and
//! JSON5 writers. Pure compute, shared by the chat skill block and the web page.
//!
//! The parser accepts the JSON5 grammar (which is a superset of both JSONC and
//! strict JSON): `//` and `/* */` comments, trailing commas, unquoted
//! identifier keys, single-quoted strings, escaped line continuations, `\x`
//! escapes, hexadecimal / leading-dot / trailing-dot / leading-`+` numbers, and
//! `Infinity` / `-Infinity` / `NaN`. Every JSON5-only construct it consumes
//! flips a flag, which is what `direction = "auto"` reads to decide which way
//! to convert.
//!
//! Object key ORDER is preserved (no map is used), so round-tripping a config
//! file does not reshuffle it. Repeated keys collapse last-wins, at the first
//! occurrence's position — the same result a JavaScript object literal gives.

use std::collections::HashMap;

/// Largest accepted input, in bytes. Above this the conversion is refused
/// rather than risking an out-of-memory trap inside the wasm sandbox.
pub const MAX_INPUT_BYTES: usize = 1_000_000;

/// Largest accepted nesting depth (arrays + objects combined).
pub const MAX_DEPTH: usize = 200;

/// A parsed JSON5 value. Numbers keep their normalized *text* rather than an
/// `f64` so that large integers and long decimals survive a round trip
/// unchanged (`f64` would silently reshape `12345678901234567890`).
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Null,
    Bool(bool),
    /// A number, already normalized to a strict-JSON numeric literal.
    Num(String),
    /// `NaN` / `Infinity` / `-Infinity` — expressible in JSON5, never in JSON.
    NonFinite(NonFinite),
    Str(String),
    Arr(Vec<Value>),
    Obj(Vec<(String, Value)>),
}

/// The three non-finite literals JSON5 allows and strict JSON does not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NonFinite {
    Nan,
    Inf,
    NegInf,
}

impl NonFinite {
    /// The JSON5 source text for this literal.
    pub fn literal(self) -> &'static str {
        match self {
            NonFinite::Nan => "NaN",
            NonFinite::Inf => "Infinity",
            NonFinite::NegInf => "-Infinity",
        }
    }
}

/// What to do with `NaN`/`Infinity` when writing strict JSON, which has no way
/// to spell them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NonFiniteMode {
    /// Emit `null` (what `JSON.stringify` does).
    Null,
    /// Emit the literal as a JSON string, e.g. `"Infinity"`.
    Text,
    /// Refuse the conversion.
    Fail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Direction {
    ToJson,
    ToJson5,
    Auto,
}

/// Convert `text` between JSON5/JSONC and strict JSON.
///
/// * `direction` — `"to-json"` (default), `"to-json5"`, or `"auto"` (emit
///   strict JSON when the input used any JSON5-only syntax, otherwise JSON5).
/// * `indent` — `"2"` (default), `"4"`, `"tab"`, or `"minify"`.
/// * `sort_keys` — sort every object's keys by code point.
/// * `nonfinite` — `"null"` (default), `"string"`, or `"error"`; only consulted
///   when writing strict JSON.
/// * `quote_style` — `"single"` (default) or `"double"`; JSON5 output only.
/// * `unquote_keys` — leave identifier-safe keys unquoted; JSON5 output only.
/// * `trailing_commas` — add a trailing comma to non-empty arrays/objects;
///   JSON5 output only.
///
/// Blank strings select the documented default so the chat/CLI surfaces can
/// omit optional params entirely.
#[allow(clippy::too_many_arguments)]
pub fn convert(
    text: &str,
    direction: &str,
    indent: &str,
    sort_keys: bool,
    nonfinite: &str,
    quote_style: &str,
    unquote_keys: bool,
    trailing_commas: bool,
) -> Result<String, String> {
    if text.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "input is {} bytes; the limit is {} bytes (1 MB) — split the file and convert it in parts",
            text.len(),
            MAX_INPUT_BYTES
        ));
    }
    let direction = match direction.trim() {
        "" | "to-json" => Direction::ToJson,
        "to-json5" => Direction::ToJson5,
        "auto" => Direction::Auto,
        other => {
            return Err(format!(
                "unknown direction '{other}' — use 'to-json', 'to-json5' or 'auto'"
            ))
        }
    };
    let indent: Option<&str> = match indent.trim() {
        "" | "2" => Some("  "),
        "4" => Some("    "),
        "tab" => Some("\t"),
        "minify" => None,
        other => {
            return Err(format!(
                "unknown indent '{other}' — use '2', '4', 'tab' or 'minify'"
            ))
        }
    };
    let nonfinite = match nonfinite.trim() {
        "" | "null" => NonFiniteMode::Null,
        "string" => NonFiniteMode::Text,
        "error" => NonFiniteMode::Fail,
        other => {
            return Err(format!(
                "unknown nonfinite '{other}' — use 'null', 'string' or 'error'"
            ))
        }
    };
    let quote = match quote_style.trim() {
        "" | "single" => '\'',
        "double" => '"',
        other => {
            return Err(format!(
                "unknown quote_style '{other}' — use 'single' or 'double'"
            ))
        }
    };
    if text.trim().is_empty() {
        return Err("input is empty — paste some JSON5, JSONC or JSON".to_string());
    }

    let mut parser = Parser::new(text);
    let mut value = parser.parse_value(0)?;
    parser.skip_ws()?;
    if parser.pos < parser.src.len() {
        return parser.fail("unexpected trailing content after the top-level value");
    }
    if sort_keys {
        sort_value(&mut value);
    }

    let emit_json = match direction {
        Direction::ToJson => true,
        Direction::ToJson5 => false,
        // The input already told us which dialect it is written in.
        Direction::Auto => parser.saw_json5,
    };

    let mut out = String::with_capacity(text.len() + text.len() / 4);
    if emit_json {
        write_json(&value, indent, nonfinite, 0, &mut out)?;
    } else {
        let opts = Json5Opts {
            indent,
            quote,
            unquote_keys,
            trailing_commas,
        };
        write_json5(&value, &opts, 0, &mut out);
    }
    Ok(out)
}

/// True when `text` parses as JSON5 *and* used at least one construct that
/// strict JSON rejects. Used by `direction = "auto"`; exposed for tests.
pub fn uses_json5_syntax(text: &str) -> Result<bool, String> {
    let mut parser = Parser::new(text);
    parser.parse_value(0)?;
    parser.skip_ws()?;
    if parser.pos < parser.src.len() {
        return parser.fail("unexpected trailing content after the top-level value");
    }
    Ok(parser.saw_json5)
}

// ---------------------------------------------------------------------------
// Parser
// ---------------------------------------------------------------------------

struct Parser<'a> {
    src: &'a [u8],
    pos: usize,
    line: usize,
    col: usize,
    /// Set the moment any JSON5-only construct is consumed.
    saw_json5: bool,
}

impl<'a> Parser<'a> {
    fn new(text: &'a str) -> Self {
        Parser {
            src: text.as_bytes(),
            pos: 0,
            line: 1,
            col: 1,
            saw_json5: false,
        }
    }

    fn peek(&self) -> Option<u8> {
        self.src.get(self.pos).copied()
    }

    fn peek_at(&self, offset: usize) -> Option<u8> {
        self.src.get(self.pos + offset).copied()
    }

    /// Consume one byte, keeping the human-facing line/column in step. Column
    /// counts CHARACTERS, so UTF-8 continuation bytes do not advance it.
    fn bump(&mut self) -> Option<u8> {
        let b = *self.src.get(self.pos)?;
        self.pos += 1;
        if b == b'\n' {
            self.line += 1;
            self.col = 1;
        } else if b & 0xC0 != 0x80 {
            self.col += 1;
        }
        Some(b)
    }

    fn fail<T>(&self, what: &str) -> Result<T, String> {
        Err(format!(
            "{what} at line {}, column {}",
            self.line, self.col
        ))
    }

    fn starts_with(&self, word: &[u8]) -> bool {
        self.src[self.pos..].starts_with(word)
    }

    /// Skip whitespace and comments. JSON5 whitespace includes a few non-ASCII
    /// code points; a BOM is tolerated anywhere for convenience.
    fn skip_ws(&mut self) -> Result<(), String> {
        loop {
            match self.peek() {
                Some(b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c) => {
                    self.bump();
                }
                // U+00A0 NO-BREAK SPACE
                Some(0xc2) if self.peek_at(1) == Some(0xa0) => {
                    self.saw_json5 = true;
                    self.bump();
                    self.bump();
                }
                // U+FEFF BYTE ORDER MARK
                Some(0xef) if self.peek_at(1) == Some(0xbb) && self.peek_at(2) == Some(0xbf) => {
                    self.bump();
                    self.bump();
                    self.bump();
                }
                // U+2028 LINE SEPARATOR / U+2029 PARAGRAPH SEPARATOR
                Some(0xe2)
                    if self.peek_at(1) == Some(0x80)
                        && matches!(self.peek_at(2), Some(0xa8 | 0xa9)) =>
                {
                    self.saw_json5 = true;
                    self.bump();
                    self.bump();
                    self.bump();
                }
                Some(b'/') => match self.peek_at(1) {
                    Some(b'/') => {
                        self.saw_json5 = true;
                        self.bump();
                        self.bump();
                        while let Some(b) = self.peek() {
                            if b == b'\n' {
                                break;
                            }
                            self.bump();
                        }
                    }
                    Some(b'*') => {
                        self.saw_json5 = true;
                        let (line, col) = (self.line, self.col);
                        self.bump();
                        self.bump();
                        loop {
                            match self.peek() {
                                None => {
                                    return Err(format!(
                                        "unterminated block comment opened at line {line}, column {col}"
                                    ))
                                }
                                Some(b'*') if self.peek_at(1) == Some(b'/') => {
                                    self.bump();
                                    self.bump();
                                    break;
                                }
                                _ => {
                                    self.bump();
                                }
                            }
                        }
                    }
                    _ => return self.fail("unexpected '/' — a comment starts with '//' or '/*'"),
                },
                _ => return Ok(()),
            }
        }
    }

    fn parse_value(&mut self, depth: usize) -> Result<Value, String> {
        if depth > MAX_DEPTH {
            return Err(format!(
                "input nests deeper than {MAX_DEPTH} levels at line {}, column {}",
                self.line, self.col
            ));
        }
        self.skip_ws()?;
        match self.peek() {
            None => self.fail("unexpected end of input; expected a value"),
            Some(b'{') => self.parse_object(depth),
            Some(b'[') => self.parse_array(depth),
            Some(b'"') => Ok(Value::Str(self.parse_string()?)),
            Some(b'\'') => {
                self.saw_json5 = true;
                Ok(Value::Str(self.parse_string()?))
            }
            Some(b't') => {
                self.expect_word(b"true")?;
                Ok(Value::Bool(true))
            }
            Some(b'f') => {
                self.expect_word(b"false")?;
                Ok(Value::Bool(false))
            }
            Some(b'n') => {
                self.expect_word(b"null")?;
                Ok(Value::Null)
            }
            _ => self.parse_number(),
        }
    }

    fn expect_word(&mut self, word: &[u8]) -> Result<(), String> {
        if !self.starts_with(word) {
            let name = std::str::from_utf8(word).unwrap_or("a literal");
            return self.fail(&format!("expected '{name}'"));
        }
        for _ in 0..word.len() {
            self.bump();
        }
        Ok(())
    }

    fn parse_object(&mut self, depth: usize) -> Result<Value, String> {
        self.bump(); // '{'
        let mut members: Vec<(String, Value)> = Vec::new();
        // Key -> index in `members`, so a repeated key is last-wins in O(1)
        // instead of rescanning a large object for every member.
        let mut seen: HashMap<String, usize> = HashMap::new();
        loop {
            self.skip_ws()?;
            match self.peek() {
                None => return self.fail("unexpected end of input; expected '}'"),
                Some(b'}') => {
                    self.bump();
                    return Ok(Value::Obj(members));
                }
                _ => {}
            }
            let key = self.parse_key()?;
            self.skip_ws()?;
            if self.peek() != Some(b':') {
                return self.fail("expected ':' after the object key");
            }
            self.bump();
            let value = self.parse_value(depth + 1)?;
            match seen.get(&key) {
                Some(&i) => members[i].1 = value,
                None => {
                    seen.insert(key.clone(), members.len());
                    members.push((key, value));
                }
            }
            self.skip_ws()?;
            match self.peek() {
                Some(b',') => {
                    self.bump();
                    self.skip_ws()?;
                    if self.peek() == Some(b'}') {
                        self.saw_json5 = true; // trailing comma
                        self.bump();
                        return Ok(Value::Obj(members));
                    }
                }
                Some(b'}') => {
                    self.bump();
                    return Ok(Value::Obj(members));
                }
                None => return self.fail("unexpected end of input; expected ',' or '}'"),
                _ => return self.fail("expected ',' or '}' after the object member"),
            }
        }
    }

    fn parse_array(&mut self, depth: usize) -> Result<Value, String> {
        self.bump(); // '['
        let mut items: Vec<Value> = Vec::new();
        loop {
            self.skip_ws()?;
            match self.peek() {
                None => return self.fail("unexpected end of input; expected ']'"),
                Some(b']') => {
                    self.bump();
                    return Ok(Value::Arr(items));
                }
                _ => {}
            }
            items.push(self.parse_value(depth + 1)?);
            self.skip_ws()?;
            match self.peek() {
                Some(b',') => {
                    self.bump();
                    self.skip_ws()?;
                    if self.peek() == Some(b']') {
                        self.saw_json5 = true; // trailing comma
                        self.bump();
                        return Ok(Value::Arr(items));
                    }
                }
                Some(b']') => {
                    self.bump();
                    return Ok(Value::Arr(items));
                }
                None => return self.fail("unexpected end of input; expected ',' or ']'"),
                _ => return self.fail("expected ',' or ']' after the array element"),
            }
        }
    }

    fn parse_key(&mut self) -> Result<String, String> {
        match self.peek() {
            Some(b'"') => self.parse_string(),
            Some(b'\'') => {
                self.saw_json5 = true;
                self.parse_string()
            }
            Some(b) if is_ident_start(b) || b >= 0x80 => {
                self.saw_json5 = true;
                self.parse_ident()
            }
            _ => self.fail("expected an object key (a quoted string or an unquoted identifier)"),
        }
    }

    fn parse_ident(&mut self) -> Result<String, String> {
        let src = self.src;
        let start = self.pos;
        while matches!(self.peek(), Some(b) if is_ident_part(b) || b >= 0x80) {
            self.bump();
        }
        std::str::from_utf8(&src[start..self.pos])
            .map(|s| s.to_string())
            .map_err(|_| "invalid UTF-8 in an unquoted key".to_string())
    }

    fn parse_string(&mut self) -> Result<String, String> {
        let src = self.src;
        let (line, col) = (self.line, self.col);
        let quote = self.bump().expect("caller checked the opening quote");
        let mut out = String::new();
        loop {
            let b = match self.peek() {
                None => {
                    return Err(format!(
                        "unterminated string starting at line {line}, column {col}"
                    ))
                }
                Some(b) => b,
            };
            if b == quote {
                self.bump();
                return Ok(out);
            }
            match b {
                b'\\' => {
                    self.bump();
                    self.parse_escape(&mut out)?;
                }
                b'\n' | b'\r' => {
                    return self
                        .fail("unescaped newline inside a string — use \\n or end the line with a backslash")
                }
                0x00..=0x1f => {
                    return self.fail("unescaped control character inside a string — use a \\u escape")
                }
                0x20..=0x7f => {
                    self.bump();
                    out.push(b as char);
                }
                _ => {
                    // Multi-byte UTF-8: copy the whole sequence through verbatim.
                    let start = self.pos;
                    self.bump();
                    while matches!(self.peek(), Some(x) if x & 0xC0 == 0x80) {
                        self.bump();
                    }
                    let seg = &src[start..self.pos];
                    if seg == [0xe2, 0x80, 0xa8] || seg == [0xe2, 0x80, 0xa9] {
                        // Raw U+2028/U+2029 in a string is JSON5-only.
                        self.saw_json5 = true;
                    }
                    out.push_str(
                        std::str::from_utf8(seg)
                            .map_err(|_| "invalid UTF-8 inside a string".to_string())?,
                    );
                }
            }
        }
    }

    fn parse_escape(&mut self, out: &mut String) -> Result<(), String> {
        let src = self.src;
        let b = match self.bump() {
            None => return self.fail("unfinished escape at the end of the input"),
            Some(b) => b,
        };
        match b {
            b'"' => out.push('"'),
            b'\'' => {
                self.saw_json5 = true;
                out.push('\'');
            }
            b'\\' => out.push('\\'),
            b'/' => out.push('/'),
            b'b' => out.push('\u{8}'),
            b'f' => out.push('\u{c}'),
            b'n' => out.push('\n'),
            b'r' => out.push('\r'),
            b't' => out.push('\t'),
            b'v' => {
                self.saw_json5 = true;
                out.push('\u{b}');
            }
            b'0' => {
                if matches!(self.peek(), Some(d) if d.is_ascii_digit()) {
                    return self.fail("'\\0' may not be followed by a digit");
                }
                self.saw_json5 = true;
                out.push('\0');
            }
            b'x' => {
                self.saw_json5 = true;
                let v = self.hex_digits(2)?;
                out.push(char::from_u32(v).expect("two hex digits are always a valid char"));
            }
            b'u' => self.parse_unicode_escape(out)?,
            // A backslash at end of line continues the string on the next line.
            b'\n' => self.saw_json5 = true,
            b'\r' => {
                self.saw_json5 = true;
                if self.peek() == Some(b'\n') {
                    self.bump();
                }
            }
            b'1'..=b'9' => return self.fail("escaped digits 1-9 are not allowed"),
            _ => {
                // Any other escaped character stands for itself (`\a` -> `a`).
                self.saw_json5 = true;
                let start = self.pos - 1;
                while matches!(self.peek(), Some(x) if x & 0xC0 == 0x80) {
                    self.bump();
                }
                let seg = &src[start..self.pos];
                // Escaped U+2028/U+2029 are line continuations, not characters.
                if seg != [0xe2, 0x80, 0xa8] && seg != [0xe2, 0x80, 0xa9] {
                    out.push_str(
                        std::str::from_utf8(seg)
                            .map_err(|_| "invalid UTF-8 after a backslash".to_string())?,
                    );
                }
            }
        }
        Ok(())
    }

    fn parse_unicode_escape(&mut self, out: &mut String) -> Result<(), String> {
        let first = self.hex_digits(4)?;
        // A high surrogate must be followed by its low half; combine the pair.
        if (0xD800..=0xDBFF).contains(&first) {
            if self.peek() == Some(b'\\') && self.peek_at(1) == Some(b'u') {
                let (line, col) = (self.line, self.col);
                self.bump();
                self.bump();
                let second = self.hex_digits(4)?;
                if (0xDC00..=0xDFFF).contains(&second) {
                    let combined = 0x10000 + ((first - 0xD800) << 10) + (second - 0xDC00);
                    out.push(char::from_u32(combined).expect("valid surrogate pair"));
                    return Ok(());
                }
                return Err(format!(
                    "\\u{first:04X} is a high surrogate but is not followed by a low surrogate (line {line}, column {col})"
                ));
            }
            return self.fail(&format!(
                "\\u{first:04X} is a lone high surrogate; it must be followed by a low surrogate"
            ));
        }
        match char::from_u32(first) {
            Some(c) => out.push(c),
            None => return self.fail(&format!("\\u{first:04X} is not a valid character")),
        }
        Ok(())
    }

    fn hex_digits(&mut self, count: usize) -> Result<u32, String> {
        let mut value: u32 = 0;
        for _ in 0..count {
            let b = match self.peek() {
                Some(b) if b.is_ascii_hexdigit() => b,
                _ => return self.fail(&format!("expected {count} hexadecimal digits in the escape")),
            };
            self.bump();
            value = value * 16 + (b as char).to_digit(16).expect("checked hex digit");
        }
        Ok(value)
    }

    fn parse_number(&mut self) -> Result<Value, String> {
        let src = self.src;
        let (line, col) = (self.line, self.col);
        let mut negative = false;
        match self.peek() {
            Some(b'-') => {
                negative = true;
                self.bump();
            }
            Some(b'+') => {
                self.saw_json5 = true;
                self.bump();
            }
            _ => {}
        }

        if self.starts_with(b"Infinity") {
            self.saw_json5 = true;
            for _ in 0..8 {
                self.bump();
            }
            return Ok(Value::NonFinite(if negative {
                NonFinite::NegInf
            } else {
                NonFinite::Inf
            }));
        }
        if self.starts_with(b"NaN") {
            self.saw_json5 = true;
            for _ in 0..3 {
                self.bump();
            }
            return Ok(Value::NonFinite(NonFinite::Nan));
        }

        if self.peek() == Some(b'0') && matches!(self.peek_at(1), Some(b'x' | b'X')) {
            self.saw_json5 = true;
            self.bump();
            self.bump();
            let start = self.pos;
            while matches!(self.peek(), Some(d) if d.is_ascii_hexdigit()) {
                self.bump();
            }
            if self.pos == start {
                return self.fail("expected hexadecimal digits after '0x'");
            }
            let digits = std::str::from_utf8(&src[start..self.pos]).expect("ASCII hex digits");
            return Ok(hex_to_value(digits, negative));
        }

        let int_start = self.pos;
        while matches!(self.peek(), Some(d) if d.is_ascii_digit()) {
            self.bump();
        }
        let int_part = &src[int_start..self.pos];
        let mut frac_part: &[u8] = b"";
        if self.peek() == Some(b'.') {
            self.bump();
            let frac_start = self.pos;
            while matches!(self.peek(), Some(d) if d.is_ascii_digit()) {
                self.bump();
            }
            frac_part = &src[frac_start..self.pos];
            // `.5` and `5.` are JSON5-only spellings.
            if int_part.is_empty() || frac_part.is_empty() {
                self.saw_json5 = true;
            }
        }
        if int_part.is_empty() && frac_part.is_empty() {
            return Err(format!("expected a value at line {line}, column {col}"));
        }
        // `01` is not a legal strict-JSON number either, so treat it as loose.
        if int_part.len() > 1 && int_part[0] == b'0' {
            self.saw_json5 = true;
        }

        let mut exponent = String::new();
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.bump();
            let mut sign = "";
            match self.peek() {
                Some(b'-') => {
                    sign = "-";
                    self.bump();
                }
                Some(b'+') => {
                    self.bump();
                }
                _ => {}
            }
            let exp_start = self.pos;
            while matches!(self.peek(), Some(d) if d.is_ascii_digit()) {
                self.bump();
            }
            if self.pos == exp_start {
                return self.fail("expected digits in the number's exponent");
            }
            let digits = std::str::from_utf8(&src[exp_start..self.pos]).expect("ASCII digits");
            exponent = format!("e{sign}{digits}");
        }

        let int_text = std::str::from_utf8(int_part).expect("ASCII digits");
        let frac_text = std::str::from_utf8(frac_part).expect("ASCII digits");
        let mut normalized = String::new();
        if negative {
            normalized.push('-');
        }
        let trimmed = int_text.trim_start_matches('0');
        normalized.push_str(if trimmed.is_empty() { "0" } else { trimmed });
        if !frac_text.is_empty() {
            normalized.push('.');
            normalized.push_str(frac_text);
        }
        normalized.push_str(&exponent);
        Ok(Value::Num(normalized))
    }
}

/// A hexadecimal literal becomes a decimal integer; a value too large for
/// `u128` falls back to a float, and one too large for that becomes infinity.
fn hex_to_value(digits: &str, negative: bool) -> Value {
    if let Ok(v) = u128::from_str_radix(digits, 16) {
        return Value::Num(format!("{}{v}", if negative { "-" } else { "" }));
    }
    let mut v = 0f64;
    for d in digits.bytes() {
        v = v * 16.0 + f64::from((d as char).to_digit(16).expect("checked hex digit"));
    }
    if negative {
        v = -v;
    }
    if v.is_finite() {
        Value::Num(format!("{v:.0}"))
    } else if negative {
        Value::NonFinite(NonFinite::NegInf)
    } else {
        Value::NonFinite(NonFinite::Inf)
    }
}

fn is_ident_start(b: u8) -> bool {
    b.is_ascii_alphabetic() || b == b'_' || b == b'$'
}

fn is_ident_part(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'$'
}

/// True when `key` can be written without quotes in JSON5. Deliberately
/// conservative: only ASCII identifiers stay bare, everything else is quoted.
fn is_bare_key(key: &str) -> bool {
    let mut bytes = key.bytes();
    match bytes.next() {
        Some(b) if is_ident_start(b) => {}
        _ => return false,
    }
    bytes.all(is_ident_part)
}

fn sort_value(value: &mut Value) {
    match value {
        Value::Obj(members) => {
            members.sort_by(|a, b| a.0.cmp(&b.0));
            for (_, v) in members.iter_mut() {
                sort_value(v);
            }
        }
        Value::Arr(items) => {
            for v in items.iter_mut() {
                sort_value(v);
            }
        }
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Writers
// ---------------------------------------------------------------------------

fn newline_indent(out: &mut String, indent: Option<&str>, depth: usize) {
    if let Some(unit) = indent {
        out.push('\n');
        for _ in 0..depth {
            out.push_str(unit);
        }
    }
}

/// Escape `s` into `out` between `quote` characters. Only the active quote is
/// escaped, so single-quoted JSON5 output keeps `"` readable and vice versa.
fn write_quoted(s: &str, quote: char, out: &mut String) {
    out.push(quote);
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if c == quote => {
                out.push('\\');
                out.push(c);
            }
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push(quote);
}

fn write_json(
    value: &Value,
    indent: Option<&str>,
    nonfinite: NonFiniteMode,
    depth: usize,
    out: &mut String,
) -> Result<(), String> {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Num(text) => out.push_str(text),
        Value::NonFinite(kind) => match nonfinite {
            NonFiniteMode::Null => out.push_str("null"),
            NonFiniteMode::Text => write_quoted(kind.literal(), '"', out),
            NonFiniteMode::Fail => {
                return Err(format!(
                    "'{}' has no strict-JSON equivalent — set nonfinite to 'null' or 'string'",
                    kind.literal()
                ))
            }
        },
        Value::Str(s) => write_quoted(s, '"', out),
        Value::Arr(items) => {
            if items.is_empty() {
                out.push_str("[]");
                return Ok(());
            }
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                newline_indent(out, indent, depth + 1);
                write_json(item, indent, nonfinite, depth + 1, out)?;
            }
            newline_indent(out, indent, depth);
            out.push(']');
        }
        Value::Obj(members) => {
            if members.is_empty() {
                out.push_str("{}");
                return Ok(());
            }
            out.push('{');
            for (i, (key, val)) in members.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                newline_indent(out, indent, depth + 1);
                write_quoted(key, '"', out);
                out.push(':');
                if indent.is_some() {
                    out.push(' ');
                }
                write_json(val, indent, nonfinite, depth + 1, out)?;
            }
            newline_indent(out, indent, depth);
            out.push('}');
        }
    }
    Ok(())
}

struct Json5Opts<'a> {
    indent: Option<&'a str>,
    quote: char,
    unquote_keys: bool,
    trailing_commas: bool,
}

fn write_json5(value: &Value, opts: &Json5Opts, depth: usize, out: &mut String) {
    match value {
        Value::Null => out.push_str("null"),
        Value::Bool(true) => out.push_str("true"),
        Value::Bool(false) => out.push_str("false"),
        Value::Num(text) => out.push_str(text),
        Value::NonFinite(kind) => out.push_str(kind.literal()),
        Value::Str(s) => write_quoted(s, opts.quote, out),
        Value::Arr(items) => {
            if items.is_empty() {
                out.push_str("[]");
                return;
            }
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                newline_indent(out, opts.indent, depth + 1);
                write_json5(item, opts, depth + 1, out);
            }
            if opts.trailing_commas {
                out.push(',');
            }
            newline_indent(out, opts.indent, depth);
            out.push(']');
        }
        Value::Obj(members) => {
            if members.is_empty() {
                out.push_str("{}");
                return;
            }
            out.push('{');
            for (i, (key, val)) in members.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                newline_indent(out, opts.indent, depth + 1);
                if opts.unquote_keys && is_bare_key(key) {
                    out.push_str(key);
                } else {
                    write_quoted(key, opts.quote, out);
                }
                out.push(':');
                if opts.indent.is_some() {
                    out.push(' ');
                }
                write_json5(val, opts, depth + 1, out);
            }
            if opts.trailing_commas {
                out.push(',');
            }
            newline_indent(out, opts.indent, depth);
            out.push('}');
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// `convert` with every option at its documented default.
    fn to_json(text: &str) -> Result<String, String> {
        convert(text, "to-json", "2", false, "null", "single", true, false)
    }

    fn minify(text: &str) -> Result<String, String> {
        convert(text, "to-json", "minify", false, "null", "single", true, false)
    }

    #[test]
    fn happy_path_strips_comments_and_trailing_commas() {
        let input = r#"{
            // the app's port
            port: 8080,
            /* block comment */
            hosts: ['a', 'b',],
        }"#;
        assert_eq!(
            to_json(input).unwrap(),
            "{\n  \"port\": 8080,\n  \"hosts\": [\n    \"a\",\n    \"b\"\n  ]\n}"
        );
    }

    #[test]
    fn error_on_unterminated_string_reports_position() {
        let err = to_json("{\n  \"a\": \"oops\n}").unwrap_err();
        assert!(err.contains("line 2"), "expected a position in: {err}");
    }

    #[test]
    fn error_on_unknown_direction() {
        let err = convert("{}", "sideways", "2", false, "null", "single", true, false).unwrap_err();
        assert!(err.contains("unknown direction"), "{err}");
    }

    #[test]
    fn error_on_unknown_indent() {
        let err = convert("{}", "to-json", "8", false, "null", "single", true, false).unwrap_err();
        assert!(err.contains("unknown indent"), "{err}");
    }

    #[test]
    fn error_on_empty_input() {
        assert!(to_json("   \n ").unwrap_err().contains("input is empty"));
    }

    #[test]
    fn error_on_input_over_the_size_cap() {
        let big = format!("\"{}\"", "x".repeat(MAX_INPUT_BYTES));
        let err = to_json(&big).unwrap_err();
        assert!(err.contains("the limit is 1000000 bytes"), "{err}");
    }

    #[test]
    fn accepts_input_exactly_at_the_size_cap() {
        let filler = "x".repeat(MAX_INPUT_BYTES - 2);
        let input = format!("\"{filler}\"");
        assert_eq!(input.len(), MAX_INPUT_BYTES);
        assert_eq!(minify(&input).unwrap().len(), MAX_INPUT_BYTES);
    }

    #[test]
    fn error_beyond_the_depth_cap() {
        let deep = format!("{}1{}", "[".repeat(MAX_DEPTH + 2), "]".repeat(MAX_DEPTH + 2));
        let err = to_json(&deep).unwrap_err();
        assert!(err.contains("nests deeper than 200 levels"), "{err}");
    }

    #[test]
    fn accepts_nesting_at_the_depth_cap() {
        let deep = format!("{}1{}", "[".repeat(MAX_DEPTH), "]".repeat(MAX_DEPTH));
        assert!(minify(&deep).is_ok());
    }

    #[test]
    fn indent_choices() {
        let input = "{a:1}";
        assert_eq!(to_json(input).unwrap(), "{\n  \"a\": 1\n}");
        assert_eq!(
            convert(input, "to-json", "4", false, "null", "single", true, false).unwrap(),
            "{\n    \"a\": 1\n}"
        );
        assert_eq!(
            convert(input, "to-json", "tab", false, "null", "single", true, false).unwrap(),
            "{\n\t\"a\": 1\n}"
        );
        assert_eq!(minify(input).unwrap(), "{\"a\":1}");
    }

    #[test]
    fn numbers_are_normalized_for_strict_json() {
        assert_eq!(
            minify("[0xdecaf, +1, .5, 5., 1.5e+3, 007, -0x10]").unwrap(),
            "[912559,1,0.5,5,1.5e3,7,-16]"
        );
    }

    #[test]
    fn big_integers_keep_full_precision() {
        assert_eq!(
            minify("[12345678901234567890123]").unwrap(),
            "[12345678901234567890123]"
        );
    }

    #[test]
    fn nonfinite_modes() {
        let input = "[NaN, Infinity, -Infinity]";
        assert_eq!(minify(input).unwrap(), "[null,null,null]");
        assert_eq!(
            convert(input, "to-json", "minify", false, "string", "single", true, false).unwrap(),
            "[\"NaN\",\"Infinity\",\"-Infinity\"]"
        );
        let err = convert(input, "to-json", "minify", false, "error", "single", true, false)
            .unwrap_err();
        assert!(err.contains("no strict-JSON equivalent"), "{err}");
    }

    #[test]
    fn string_escapes_and_continuations() {
        // \x, \0, \v, an escaped newline continuation and a surrogate pair.
        let input = "['\\x41\\u0042\\v\\0', \"one\\\ntwo\", \"\\uD83D\\uDE00\"]";
        assert_eq!(
            minify(input).unwrap(),
            "[\"AB\\u000b\\u0000\",\"onetwo\",\"😀\"]"
        );
    }

    #[test]
    fn a_url_inside_a_string_is_not_mistaken_for_a_comment() {
        assert_eq!(
            minify("{ site: 'https://example.com/a' /* real comment */ }").unwrap(),
            "{\"site\":\"https://example.com/a\"}"
        );
    }

    #[test]
    fn to_json5_emits_bare_keys_and_single_quotes() {
        assert_eq!(
            convert(
                "{\"name\":\"ada\",\"two words\":1}",
                "to-json5",
                "2",
                false,
                "null",
                "single",
                true,
                false
            )
            .unwrap(),
            "{\n  name: 'ada',\n  'two words': 1\n}"
        );
    }

    #[test]
    fn to_json5_honours_quote_style_and_key_quoting() {
        assert_eq!(
            convert(
                "{\"name\":\"ada\"}",
                "to-json5",
                "minify",
                false,
                "null",
                "double",
                false,
                false
            )
            .unwrap(),
            "{\"name\":\"ada\"}"
        );
    }

    #[test]
    fn to_json5_trailing_commas() {
        assert_eq!(
            convert(
                "{\"a\":[1,2]}",
                "to-json5",
                "2",
                false,
                "null",
                "single",
                true,
                true
            )
            .unwrap(),
            "{\n  a: [\n    1,\n    2,\n  ],\n}"
        );
    }

    #[test]
    fn to_json5_keeps_nonfinite_literals() {
        assert_eq!(
            convert(
                "[NaN,Infinity]",
                "to-json5",
                "minify",
                false,
                "null",
                "single",
                true,
                false
            )
            .unwrap(),
            "[NaN,Infinity]"
        );
    }

    #[test]
    fn auto_direction_picks_the_opposite_dialect() {
        // Loose input -> strict JSON.
        assert_eq!(
            convert("{a:1,}", "auto", "minify", false, "null", "single", true, false).unwrap(),
            "{\"a\":1}"
        );
        // Already-strict input -> JSON5.
        assert_eq!(
            convert(
                "{\"a\":1}",
                "auto",
                "minify",
                false,
                "null",
                "single",
                true,
                false
            )
            .unwrap(),
            "{a:1}"
        );
        assert!(uses_json5_syntax("{a:1}").unwrap());
        assert!(!uses_json5_syntax("{\"a\":1}").unwrap());
    }

    #[test]
    fn key_order_is_preserved_and_duplicates_collapse_last_wins() {
        assert_eq!(
            minify("{z:1, a:2, z:3}").unwrap(),
            "{\"z\":3,\"a\":2}"
        );
    }

    #[test]
    fn sort_keys_is_recursive() {
        assert_eq!(
            convert(
                "{b:1, a:{d:1, c:2}}",
                "to-json",
                "minify",
                true,
                "null",
                "single",
                true,
                false
            )
            .unwrap(),
            "{\"a\":{\"c\":2,\"d\":1},\"b\":1}"
        );
    }

    #[test]
    fn unterminated_block_comment_is_reported() {
        let err = to_json("{ /* nope\n }").unwrap_err();
        assert!(err.contains("unterminated block comment"), "{err}");
    }

    #[test]
    fn trailing_content_is_rejected() {
        let err = to_json("{} {}").unwrap_err();
        assert!(err.contains("unexpected trailing content"), "{err}");
    }

    #[test]
    fn scalars_and_empty_containers_round_trip() {
        assert_eq!(minify("[]").unwrap(), "[]");
        assert_eq!(minify("{}").unwrap(), "{}");
        assert_eq!(minify("  null ").unwrap(), "null");
        assert_eq!(minify("'hi'").unwrap(), "\"hi\"");
    }
}
