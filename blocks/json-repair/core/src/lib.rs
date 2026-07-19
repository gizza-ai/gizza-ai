//! gizza-ai/json-repair core — repair malformed JSON with a tolerant
//! recursive-descent parser, then re-serialize valid JSON. Fixes trailing
//! commas, single/smart quotes, unquoted keys and values, missing commas,
//! `//` + `/* */` comments, markdown ```json fences, Python literals
//! (True/False/None), undefined/NaN/Infinity, raw control characters in
//! strings, bracket mismatches, and truncated input. Key order is preserved
//! (serde_json `preserve_order`); duplicate keys keep the last value.
//! Pure-Rust, no wasm/wafer deps.

use serde::Serialize;
use serde_json::{Map, Number, Value};

/// Maximum nesting depth accepted (stack-safety in wasm). 200 levels parse;
/// 201 is an error — the exact boundary is unit- and page-tested.
pub const MAX_DEPTH: usize = 200;

/// Repair `input` and format the result. `indent` is one of `"2"`, `"4"`,
/// `"tab"`, `"minify"`.
pub fn repair(input: &str, indent: &str) -> Result<String, String> {
    let value = repair_to_value(input)?;
    render(&value, indent)
}

/// Repair `input` into a `serde_json::Value` (the always-valid intermediate).
pub fn repair_to_value(input: &str) -> Result<Value, String> {
    let body = strip_fence(input);
    let chars: Vec<char> = body.chars().collect();
    let mut p = Parser { c: chars, i: 0 };
    p.skip_junk();
    if p.eof() {
        return Err(
            "nothing to repair: the input is empty or contains only comments/whitespace — paste the broken JSON text"
                .into(),
        );
    }
    let first = p.value(0)?;
    let mut vals = vec![first];
    loop {
        p.skip_junk();
        while p.peek() == Some(',') {
            p.i += 1;
            p.skip_junk();
        }
        match p.peek() {
            None => break,
            Some(ch)
                if matches!(ch, '{' | '[') || is_open_quote(ch) || ch.is_ascii_digit() || ch == '-' =>
            {
                // another top-level value (e.g. newline-delimited JSON) — collect
                vals.push(p.value(0)?);
            }
            _ => break, // trailing non-JSON garbage — ignore
        }
    }
    Ok(if vals.len() == 1 {
        vals.pop().unwrap()
    } else {
        Value::Array(vals)
    })
}

fn render(v: &Value, indent: &str) -> Result<String, String> {
    let pad: &[u8] = match indent {
        "minify" => {
            return serde_json::to_string(v).map_err(|e| format!("serialize failed: {e}"))
        }
        "2" => b"  ",
        "4" => b"    ",
        "tab" => b"\t",
        other => {
            return Err(format!(
                "unknown indent '{other}': expected one of 2, 4, tab, minify"
            ))
        }
    };
    let mut buf = Vec::new();
    let fmt = serde_json::ser::PrettyFormatter::with_indent(pad);
    let mut ser = serde_json::Serializer::with_formatter(&mut buf, fmt);
    v.serialize(&mut ser)
        .map_err(|e| format!("serialize failed: {e}"))?;
    String::from_utf8(buf).map_err(|e| format!("utf8: {e}"))
}

/// If the input contains a markdown code fence, extract the fenced body
/// (handles LLM output like "Here you go:\n```json\n{...}\n```").
fn strip_fence(input: &str) -> String {
    match input.find("```") {
        None => input.to_string(),
        Some(start) => {
            let after = &input[start + 3..];
            let body_start = after.find('\n').map(|n| n + 1).unwrap_or(after.len());
            let body = &after[body_start..];
            let body = match body.find("```") {
                Some(end) => &body[..end],
                None => body,
            };
            body.to_string()
        }
    }
}

fn is_open_quote(c: char) -> bool {
    matches!(c, '"' | '\'' | '`' | '\u{201C}' | '\u{201D}' | '\u{2018}' | '\u{2019}')
}

fn is_word_start(c: char) -> bool {
    c.is_alphabetic() || c == '_' || c == '$'
}

struct Parser {
    c: Vec<char>,
    i: usize,
}

impl Parser {
    fn eof(&self) -> bool {
        self.i >= self.c.len()
    }
    fn peek(&self) -> Option<char> {
        self.c.get(self.i).copied()
    }
    fn peek_at(&self, off: usize) -> Option<char> {
        self.c.get(self.i + off).copied()
    }
    fn bump(&mut self) -> Option<char> {
        let ch = self.peek();
        if ch.is_some() {
            self.i += 1;
        }
        ch
    }

    /// Skip whitespace and `//` / `/* */` comments.
    fn skip_junk(&mut self) {
        loop {
            while matches!(self.peek(), Some(ch) if ch.is_whitespace()) {
                self.i += 1;
            }
            if self.peek() == Some('/') && self.peek_at(1) == Some('/') {
                while !self.eof() && self.peek() != Some('\n') {
                    self.i += 1;
                }
            } else if self.peek() == Some('/') && self.peek_at(1) == Some('*') {
                self.i += 2;
                while !self.eof() && !(self.peek() == Some('*') && self.peek_at(1) == Some('/')) {
                    self.i += 1;
                }
                self.i = (self.i + 2).min(self.c.len());
            } else {
                break;
            }
        }
    }

    fn depth_guard(depth: usize) -> Result<(), String> {
        if depth >= MAX_DEPTH {
            return Err(format!(
                "JSON nested deeper than {MAX_DEPTH} levels — refusing to repair (flatten the structure)"
            ));
        }
        Ok(())
    }

    fn value(&mut self, depth: usize) -> Result<Value, String> {
        loop {
            self.skip_junk();
            match self.peek() {
                None => return Ok(Value::Null), // truncated input → null
                Some('{') => return self.object(depth),
                Some('[') => return self.array(depth),
                Some(q) if is_open_quote(q) => {
                    self.i += 1;
                    return Ok(Value::String(self.string(q)));
                }
                Some(ch) if ch.is_ascii_digit() || matches!(ch, '-' | '+' | '.') => {
                    return Ok(self.number_or_word())
                }
                Some(ch) if is_word_start(ch) => return Ok(self.word_value()),
                Some(_) => {
                    // stray punctuation where a value should be — skip and retry
                    self.i += 1;
                }
            }
        }
    }

    fn object(&mut self, depth: usize) -> Result<Value, String> {
        Self::depth_guard(depth)?;
        self.i += 1; // consume '{'
        let mut map = Map::new();
        loop {
            self.skip_junk();
            match self.peek() {
                None => break, // truncated → close the object
                Some('}') => {
                    self.i += 1;
                    break;
                }
                Some(']') => {
                    // bracket mismatch (e.g. `{"a":1]`) — treat as the closer
                    self.i += 1;
                    break;
                }
                Some(',') => {
                    self.i += 1; // stray / extra comma
                    continue;
                }
                _ => {}
            }
            // --- key: quoted or bare ---
            let key = match self.peek() {
                Some(q) if is_open_quote(q) => {
                    self.i += 1;
                    self.string(q)
                }
                _ => {
                    let start = self.i;
                    while let Some(ch) = self.peek() {
                        if matches!(ch, ':' | '=' | ',' | '{' | '}' | '[' | ']' | '\n' | '\r') {
                            break;
                        }
                        self.i += 1;
                    }
                    let k: String = self.c[start..self.i].iter().collect();
                    k.trim().to_string()
                }
            };
            self.skip_junk();
            // --- value: after ':' (or '='); tolerate a missing separator ---
            let val = match self.peek() {
                Some(':') | Some('=') => {
                    self.i += 1;
                    self.value(depth + 1)?
                }
                Some('}') | Some(',') | None => Value::Null, // key without value
                _ => self.value(depth + 1)?, // missing colon, value follows
            };
            if !key.is_empty() {
                map.insert(key, val); // duplicate keys: last value wins
            }
            // --- separator ---
            self.skip_junk();
            match self.peek() {
                Some(',') => {
                    self.i += 1;
                }
                Some('}') => {
                    self.i += 1;
                    break;
                }
                None => break,
                _ => {} // missing comma → next iteration parses the next key
            }
        }
        Ok(Value::Object(map))
    }

    fn array(&mut self, depth: usize) -> Result<Value, String> {
        Self::depth_guard(depth)?;
        self.i += 1; // consume '['
        let mut arr = Vec::new();
        loop {
            self.skip_junk();
            match self.peek() {
                None => break, // truncated → close the array
                Some(']') => {
                    self.i += 1;
                    break;
                }
                Some('}') => {
                    // brace mismatch (e.g. `[1,2}`) — treat as the closer
                    self.i += 1;
                    break;
                }
                Some(',') => {
                    self.i += 1; // stray / extra comma
                    continue;
                }
                _ => {}
            }
            arr.push(self.value(depth + 1)?);
            self.skip_junk();
            match self.peek() {
                Some(',') => {
                    self.i += 1;
                }
                Some(']') => {
                    self.i += 1;
                    break;
                }
                Some('}') => {
                    self.i += 1;
                    break;
                }
                None => break,
                _ => {} // missing comma → next iteration parses the next element
            }
        }
        Ok(Value::Array(arr))
    }

    /// Parse a string opened with `open` (double/single/backtick/smart quote).
    /// Raw control characters are kept (serde escapes them on output); an
    /// unterminated string is closed at EOF.
    fn string(&mut self, open: char) -> String {
        let mut s = String::new();
        let is_close = |ch: char| match open {
            '"' => ch == '"',
            '\'' => ch == '\'',
            '`' => ch == '`',
            '\u{201C}' | '\u{201D}' => matches!(ch, '\u{201D}' | '"'),
            '\u{2018}' | '\u{2019}' => matches!(ch, '\u{2019}' | '\''),
            _ => ch == open,
        };
        while let Some(ch) = self.bump() {
            if is_close(ch) {
                return s;
            }
            if ch != '\\' {
                s.push(ch);
                continue;
            }
            match self.bump() {
                None => break,
                Some('n') => s.push('\n'),
                Some('t') => s.push('\t'),
                Some('r') => s.push('\r'),
                Some('b') => s.push('\u{0008}'),
                Some('f') => s.push('\u{000C}'),
                Some('u') => match self.unicode_escape() {
                    Some(c) => s.push(c),
                    None => s.push('\u{FFFD}'),
                },
                Some(other) => s.push(other), // \" \' \\ \/ + unknown escapes
            }
        }
        s // EOF before the closing quote → truncated string, close it here
    }

    fn hex4(&mut self) -> Option<u32> {
        let mut v: u32 = 0;
        for off in 0..4 {
            let d = self.peek_at(off)?.to_digit(16)?;
            v = v * 16 + d;
        }
        self.i += 4;
        Some(v)
    }

    /// `\u` already consumed. Handles surrogate pairs.
    fn unicode_escape(&mut self) -> Option<char> {
        let hi = self.hex4()?;
        if (0xD800..0xDC00).contains(&hi) {
            if self.peek() == Some('\\') && self.peek_at(1) == Some('u') {
                let save = self.i;
                self.i += 2;
                if let Some(lo) = self.hex4() {
                    if (0xDC00..0xE000).contains(&lo) {
                        let cp = 0x10000 + ((hi - 0xD800) << 10) + (lo - 0xDC00);
                        return char::from_u32(cp);
                    }
                }
                self.i = save; // not a low surrogate — rewind
            }
            return None; // lone high surrogate
        }
        if (0xDC00..0xE000).contains(&hi) {
            return None; // lone low surrogate
        }
        char::from_u32(hi)
    }

    /// Starting at a digit/sign/dot: a number, a signed word (-Infinity), or an
    /// unquoted string fallback (`123abc`, `+1-555-0100`).
    fn number_or_word(&mut self) -> Value {
        let start = self.i;
        if matches!(self.peek(), Some('+') | Some('-')) {
            self.i += 1;
        }
        if matches!(self.peek(), Some(ch) if is_word_start(ch)) {
            let wstart = self.i;
            while matches!(self.peek(), Some(ch) if ch.is_alphanumeric() || ch == '_') {
                self.i += 1;
            }
            let w: String = self.c[wstart..self.i].iter().collect();
            if matches!(w.to_ascii_lowercase().as_str(), "infinity" | "nan") {
                return Value::Null; // JSON has no Infinity/NaN
            }
            return self.unquoted_from(start);
        }
        while matches!(self.peek(), Some(ch) if ch.is_ascii_digit() || matches!(ch, '.' | 'e' | 'E' | '+' | '-'))
        {
            self.i += 1;
        }
        // a word continuing right after digits means it never was a number
        if matches!(self.peek(), Some(ch) if ch.is_alphanumeric() || ch == '_') {
            return self.unquoted_from(start);
        }
        let raw: String = self.c[start..self.i].iter().collect();
        let cleaned = raw.strip_prefix('+').unwrap_or(&raw);
        if !cleaned.contains(['.', 'e', 'E']) {
            if let Ok(n) = cleaned.parse::<i64>() {
                return Value::Number(n.into());
            }
        }
        if let Ok(f) = cleaned.parse::<f64>() {
            if f.is_finite() {
                if let Some(n) = Number::from_f64(f) {
                    return Value::Number(n);
                }
            }
            return Value::Null;
        }
        self.unquoted_from(start)
    }

    /// A bare word: true/false/null literal (incl. Python/JS spellings) when
    /// followed by a delimiter, else an unquoted string value.
    fn word_value(&mut self) -> Value {
        let start = self.i;
        while matches!(self.peek(), Some(ch) if ch.is_alphanumeric() || ch == '_' || ch == '$') {
            self.i += 1;
        }
        let word: String = self.c[start..self.i].iter().collect();
        // literal only if the word stands alone (delimiter next) — so
        // `{a: true story}` repairs to the STRING "true story", not `true`.
        let mut j = self.i;
        while matches!(self.c.get(j), Some(' ') | Some('\t')) {
            j += 1;
        }
        let delim = match self.c.get(j) {
            None => true,
            Some(ch) => matches!(ch, ',' | '}' | ']' | '\n' | '\r'),
        };
        if delim {
            match word.to_ascii_lowercase().as_str() {
                "true" => return Value::Bool(true),
                "false" => return Value::Bool(false),
                "null" | "none" | "nil" | "undefined" | "nan" | "infinity" => return Value::Null,
                _ => {}
            }
        }
        self.unquoted_from(start)
    }

    /// Unquoted string value: everything from `start` to `,` `}` `]` or EOL.
    fn unquoted_from(&mut self, start: usize) -> Value {
        while let Some(ch) = self.peek() {
            if matches!(ch, ',' | '}' | ']' | '\n' | '\r') {
                break;
            }
            self.i += 1;
        }
        let s: String = self.c[start..self.i].iter().collect();
        Value::String(s.trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn min(input: &str) -> String {
        repair(input, "minify").expect("repair should succeed")
    }

    #[test]
    fn trailing_commas_and_single_quotes() {
        assert_eq!(
            min("{'name': 'John', 'age': 30,}"),
            r#"{"name":"John","age":30}"#
        );
        assert_eq!(min("[1, 2, 3,]"), "[1,2,3]");
    }

    #[test]
    fn unquoted_keys_and_values() {
        assert_eq!(min(r#"{name: "gizza", fast: true,}"#), r#"{"name":"gizza","fast":true}"#);
        assert_eq!(min("{city: New York}"), r#"{"city":"New York"}"#);
    }

    #[test]
    fn missing_commas() {
        assert_eq!(min(r#"{"a":1 "b":2}"#), r#"{"a":1,"b":2}"#);
        assert_eq!(min(r#"[1 2 "x"]"#), r#"[1,2,"x"]"#);
    }

    #[test]
    fn comments_stripped() {
        assert_eq!(
            min("{ // user record\n \"a\": 1, /* legacy */ \"b\": 2 }"),
            r#"{"a":1,"b":2}"#
        );
    }

    #[test]
    fn markdown_fence_stripped() {
        assert_eq!(min("Sure!\n```json\n{\"a\": 1}\n```\nHope that helps."), r#"{"a":1}"#);
        assert_eq!(
            min("```json\n{\"users\": [\"Alice\", \"Bob\"\n```"),
            r#"{"users":["Alice","Bob"]}"#
        );
    }

    #[test]
    fn truncated_input_closed() {
        assert_eq!(min(r#"{"a": [1, 2"#), r#"{"a":[1,2]}"#);
        assert_eq!(min(r#"{"name": "Jo"#), r#"{"name":"Jo"}"#);
        assert_eq!(min(r#"{"a":"#), r#"{"a":null}"#);
    }

    #[test]
    fn python_and_js_literals() {
        assert_eq!(
            min("{'a': True, 'b': False, 'c': None, 'd': undefined, 'e': NaN, 'f': -Infinity}"),
            r#"{"a":true,"b":false,"c":null,"d":null,"e":null,"f":null}"#
        );
    }

    #[test]
    fn literal_needs_delimiter() {
        assert_eq!(min("{a: true story}"), r#"{"a":"true story"}"#);
    }

    #[test]
    fn raw_control_chars_escaped_on_output() {
        assert_eq!(min("{\"a\": \"line1\nline2\tend\"}"), r#"{"a":"line1\nline2\tend"}"#);
    }

    #[test]
    fn smart_quotes_and_backticks() {
        assert_eq!(min("{\u{201C}a\u{201D}: \u{2018}b\u{2019}, `c`: 1}"), r#"{"a":"b","c":1}"#);
    }

    #[test]
    fn bracket_mismatch() {
        assert_eq!(min("[1, 2}"), "[1,2]");
        assert_eq!(min(r#"{"a": 1]"#), r#"{"a":1}"#);
    }

    #[test]
    fn ndjson_wrapped_into_array() {
        assert_eq!(min("{\"a\":1}\n{\"a\":2}"), r#"[{"a":1},{"a":2}]"#);
    }

    #[test]
    fn key_order_preserved_and_duplicates_last_win() {
        assert_eq!(min(r#"{"z":1,"a":2}"#), r#"{"z":1,"a":2}"#);
        assert_eq!(min(r#"{"a":1,"a":2}"#), r#"{"a":2}"#);
    }

    #[test]
    fn unicode_escapes_and_surrogates() {
        assert_eq!(
            min(r#"{"e": "é", "smile": "😀"}"#),
            "{\"e\":\"\u{e9}\",\"smile\":\"\u{1F600}\"}"
        );
    }

    #[test]
    fn backslash_u_escapes_decoded() {
        assert_eq!(
            min("[\"\\u00e9\", \"\\ud83d\\ude00\"]"),
            "[\"\u{e9}\",\"\u{1F600}\"]"
        );
    }

    #[test]
    fn numbers_keep_shape() {
        assert_eq!(min("[1, 1.5, -2, .5, +7]"), "[1,1.5,-2,0.5,7]");
    }

    #[test]
    fn indent_variants() {
        let input = "{'a': 1,}";
        assert_eq!(repair(input, "2").unwrap(), "{\n  \"a\": 1\n}");
        assert_eq!(repair(input, "4").unwrap(), "{\n    \"a\": 1\n}");
        assert_eq!(repair(input, "tab").unwrap(), "{\n\t\"a\": 1\n}");
        assert_eq!(repair(input, "minify").unwrap(), r#"{"a":1}"#);
    }

    #[test]
    fn valid_json_passes_through() {
        assert_eq!(
            min(r#"{"a": [1, {"b": null}], "c": "x"}"#),
            r#"{"a":[1,{"b":null}],"c":"x"}"#
        );
    }

    #[test]
    fn error_on_empty_or_comment_only_input() {
        assert!(repair("", "2").is_err());
        assert!(repair("   \n\t", "2").is_err());
        assert!(repair("// just a comment", "2").is_err());
    }

    #[test]
    fn error_on_unknown_indent() {
        let e = repair("{}", "3").unwrap_err();
        assert!(e.contains("unknown indent"), "{e}");
    }

    #[test]
    fn depth_cap_exact_boundary() {
        let ok = "[".repeat(MAX_DEPTH) + &"]".repeat(MAX_DEPTH);
        assert!(repair(&ok, "minify").is_ok(), "exactly {MAX_DEPTH} levels must parse");
        let too_deep = "[".repeat(MAX_DEPTH + 1);
        let e = repair(&too_deep, "minify").unwrap_err();
        assert!(e.contains("200 levels"), "{e}");
    }
}
