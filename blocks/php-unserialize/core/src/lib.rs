//! php-unserialize core — parses a PHP `serialize()` string and renders it as
//! readable JSON. No wafer/wasm-bindgen deps. Shared by the chat skill block and
//! the web page.
//!
//! PHP's `serialize()` produces a compact, type-tagged wire format used by
//! `unserialize()` and stored in PHP sessions, caches (memcached/Redis when
//! configured with the PHP serializer), and WordPress `wp_options` rows. This is
//! the inverse of `php-serialize`: it reads that wire format and emits the
//! equivalent JSON so a non-PHP system (Node, Python, a shell script) can inspect
//! data a PHP app wrote.
//!
//! | PHP serialize()                       | JSON                          |
//! | ------------------------------------- | ----------------------------- |
//! | `N;`                                  | `null`                        |
//! | `b:1;` / `b:0;`                       | `true` / `false`              |
//! | `i:42;`                               | `42`                          |
//! | `d:3.5;`                              | `3.5`                         |
//! | `s:2:"hi";`                           | `"hi"` (length is in BYTES)   |
//! | `a:2:{i:0;i:1;i:1;i:2;}`              | `[1, 2]` (sequential keys)    |
//! | `a:1:{s:1:"a";i:1;}`                 | `{"a": 1}`                    |
//! | `O:3:"Foo":1:{s:1:"a";i:1;}`         | `{"a": 1}` (class name kept)  |
//!
//! A PHP array whose keys are exactly the sequential integers `0..n` is rendered
//! as a JSON array (mirroring `json_encode` of a PHP list). Any other array — a
//! string-keyed or sparse/out-of-order integer-keyed array, or a serialized object
//! `O:` — becomes a JSON object. Object class names are surfaced under a
//! `"__class"` key so the type information survives the round trip.

use serde_json::{Map, Number, Value};

/// Parse a PHP `serialize()` string and render it as pretty-printed JSON.
///
/// Returns `Err` with a human-readable message if the input is not a valid
/// serialized value (truncated, wrong length prefix, trailing garbage, etc.).
pub fn php_to_json(input: &str) -> Result<String, String> {
    let value = unserialize_to_value(input)?;
    serde_json::to_string_pretty(&value).map_err(|e| format!("could not render JSON: {e}"))
}

/// Parse a PHP `serialize()` string into a `serde_json::Value`.
pub fn unserialize_to_value(input: &str) -> Result<Value, String> {
    let bytes = input.as_bytes();
    let mut p = Parser { bytes, pos: 0 };
    let value = p.parse_value()?;
    if p.pos != bytes.len() {
        return Err(format!(
            "trailing data after the serialized value (parsed {} of {} bytes)",
            p.pos,
            bytes.len()
        ));
    }
    Ok(value)
}

struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Result<u8, String> {
        self.bytes
            .get(self.pos)
            .copied()
            .ok_or_else(|| "unexpected end of input".to_string())
    }

    fn expect(&mut self, b: u8) -> Result<(), String> {
        if self.peek()? != b {
            return Err(format!(
                "expected '{}' at byte {}, found '{}'",
                b as char,
                self.pos,
                self.bytes[self.pos] as char
            ));
        }
        self.pos += 1;
        Ok(())
    }

    /// Read up to (but not including) the delimiter byte, returning the slice.
    fn read_until(&mut self, delim: u8) -> Result<&'a [u8], String> {
        let start = self.pos;
        while self.pos < self.bytes.len() && self.bytes[self.pos] != delim {
            self.pos += 1;
        }
        if self.pos >= self.bytes.len() {
            return Err(format!("expected '{}' but reached end of input", delim as char));
        }
        let slice = &self.bytes[start..self.pos];
        self.pos += 1; // consume the delimiter
        Ok(slice)
    }

    fn parse_value(&mut self) -> Result<Value, String> {
        match self.peek()? {
            b'N' => {
                // N;
                self.pos += 1;
                self.expect(b';')?;
                Ok(Value::Null)
            }
            b'b' => {
                // b:0; or b:1;
                self.pos += 1;
                self.expect(b':')?;
                let v = self.peek()?;
                self.pos += 1;
                self.expect(b';')?;
                match v {
                    b'0' => Ok(Value::Bool(false)),
                    b'1' => Ok(Value::Bool(true)),
                    other => Err(format!("invalid boolean value '{}'", other as char)),
                }
            }
            b'i' => {
                // i:42;
                self.pos += 1;
                self.expect(b':')?;
                let raw = self.read_until(b';')?;
                let s = std::str::from_utf8(raw).map_err(|_| "invalid integer bytes".to_string())?;
                let n: i64 = s
                    .parse()
                    .map_err(|_| format!("invalid integer '{s}'"))?;
                Ok(Value::Number(n.into()))
            }
            b'd' => {
                // d:3.5; (also INF, -INF, NAN which JSON can't represent -> null)
                self.pos += 1;
                self.expect(b':')?;
                let raw = self.read_until(b';')?;
                let s = std::str::from_utf8(raw).map_err(|_| "invalid double bytes".to_string())?;
                match s {
                    "INF" | "-INF" | "NAN" => Ok(Value::Null),
                    _ => {
                        let f: f64 = s.parse().map_err(|_| format!("invalid double '{s}'"))?;
                        Ok(Number::from_f64(f).map(Value::Number).unwrap_or(Value::Null))
                    }
                }
            }
            b's' => {
                let s = self.parse_string()?;
                Ok(Value::String(s))
            }
            b'a' => self.parse_array(),
            b'O' => self.parse_object(),
            other => Err(format!(
                "unsupported type tag '{}' at byte {}",
                other as char, self.pos
            )),
        }
    }

    /// Parse `s:<len>:"<bytes>";` and return the contained string. PHP measures
    /// `<len>` in BYTES, so we slice exactly that many bytes (not characters).
    fn parse_string(&mut self) -> Result<String, String> {
        self.expect(b's')?;
        self.expect(b':')?;
        let len = self.read_len()?;
        self.expect(b':')?;
        self.expect(b'"')?;
        if self.pos + len > self.bytes.len() {
            return Err(format!(
                "string of declared length {len} runs past end of input"
            ));
        }
        let slice = &self.bytes[self.pos..self.pos + len];
        self.pos += len;
        self.expect(b'"')?;
        self.expect(b';')?;
        let s = std::str::from_utf8(slice)
            .map_err(|_| "string contains invalid UTF-8 (cannot represent in JSON)".to_string())?;
        Ok(s.to_string())
    }

    /// Parse an unsigned length/count field (digits up to a `:` or `"`).
    fn read_len(&mut self) -> Result<usize, String> {
        let start = self.pos;
        while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_digit() {
            self.pos += 1;
        }
        if self.pos == start {
            return Err(format!("expected a length/count at byte {}", start));
        }
        let s = std::str::from_utf8(&self.bytes[start..self.pos]).unwrap();
        s.parse::<usize>()
            .map_err(|_| format!("length '{s}' is out of range"))
    }

    /// Parse the `{<key><value>...}` body shared by `a:` arrays and `O:` objects.
    /// Returns the (key, value) pairs in serialized order. Keys are PHP int or
    /// string scalars.
    fn parse_members(&mut self, count: usize) -> Result<Vec<(Value, Value)>, String> {
        self.expect(b'{')?;
        let mut pairs = Vec::with_capacity(count);
        for _ in 0..count {
            let key = self.parse_value()?;
            let val = self.parse_value()?;
            pairs.push((key, val));
        }
        self.expect(b'}')?;
        Ok(pairs)
    }

    fn parse_array(&mut self) -> Result<Value, String> {
        self.expect(b'a')?;
        self.expect(b':')?;
        let count = self.read_len()?;
        self.expect(b':')?;
        let pairs = self.parse_members(count)?;
        Ok(members_to_value(pairs, None))
    }

    fn parse_object(&mut self) -> Result<Value, String> {
        // O:<class-name-len>:"<class>":<prop-count>:{...}
        self.expect(b'O')?;
        self.expect(b':')?;
        let name_len = self.read_len()?;
        self.expect(b':')?;
        self.expect(b'"')?;
        if self.pos + name_len > self.bytes.len() {
            return Err("class name runs past end of input".to_string());
        }
        let name = std::str::from_utf8(&self.bytes[self.pos..self.pos + name_len])
            .map_err(|_| "class name contains invalid UTF-8".to_string())?
            .to_string();
        self.pos += name_len;
        self.expect(b'"')?;
        self.expect(b':')?;
        let count = self.read_len()?;
        self.expect(b':')?;
        let pairs = self.parse_members(count)?;
        Ok(members_to_value(pairs, Some(name)))
    }
}

/// Convert a key `Value` into a JSON object-key string. PHP array/object keys are
/// always integers or strings.
fn key_to_string(key: &Value) -> String {
    match key {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        // PHP coerces other key types; fall back to a JSON rendering so nothing
        // is silently dropped.
        other => other.to_string(),
    }
}

/// Decide whether a set of array members is a JSON list or object, and build it.
///
/// A list iff `class_name` is None AND the keys are exactly the sequential
/// integers `0, 1, …, n-1` in order — matching how PHP's own `json_encode`
/// distinguishes a list array from an associative one. Objects (`O:`) always
/// become JSON objects and carry a `"__class"` field with the class name.
fn members_to_value(pairs: Vec<(Value, Value)>, class_name: Option<String>) -> Value {
    let is_sequential_list = class_name.is_none()
        && pairs.iter().enumerate().all(|(idx, (k, _))| match k {
            Value::Number(n) => n.as_u64() == Some(idx as u64),
            _ => false,
        });

    if is_sequential_list {
        return Value::Array(pairs.into_iter().map(|(_, v)| v).collect());
    }

    let mut map = Map::new();
    if let Some(class) = class_name {
        map.insert("__class".to_string(), Value::String(class));
    }
    for (k, v) in pairs {
        map.insert(key_to_string(&k), v);
    }
    Value::Object(map)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn val(s: &str) -> Value {
        unserialize_to_value(s).unwrap()
    }

    #[test]
    fn scalars() {
        assert_eq!(val("N;"), Value::Null);
        assert_eq!(val("b:1;"), Value::Bool(true));
        assert_eq!(val("b:0;"), Value::Bool(false));
        assert_eq!(val("i:42;"), Value::Number(42.into()));
        assert_eq!(val("i:-7;"), Value::Number((-7i64).into()));
    }

    #[test]
    fn doubles() {
        assert_eq!(val("d:3.5;"), Value::Number(Number::from_f64(3.5).unwrap()));
        assert_eq!(val("d:0.1;"), Value::Number(Number::from_f64(0.1).unwrap()));
        // Non-finite doubles can't live in JSON -> null.
        assert_eq!(val("d:INF;"), Value::Null);
        assert_eq!(val("d:NAN;"), Value::Null);
    }

    #[test]
    fn strings_use_byte_length() {
        assert_eq!(val("s:2:\"hi\";"), Value::String("hi".into()));
        assert_eq!(val("s:0:\"\";"), Value::String("".into()));
        // "é" is 2 bytes in UTF-8; the length prefix is 2.
        assert_eq!(val("s:2:\"é\";"), Value::String("é".into()));
        // "€" is 3 bytes.
        assert_eq!(val("s:3:\"€\";"), Value::String("€".into()));
    }

    #[test]
    fn string_with_embedded_delimiters() {
        // A serialized string can legally contain `;` and `"` — the byte length
        // tells us where it ends, so a naive delimiter scan would break here.
        assert_eq!(
            val("s:4:\"a;\"b\";"),
            Value::String("a;\"b".into())
        );
    }

    #[test]
    fn list_array_becomes_json_array() {
        assert_eq!(
            php_to_json("a:3:{i:0;i:1;i:1;i:2;i:2;i:3;}").unwrap(),
            "[\n  1,\n  2,\n  3\n]"
        );
        assert_eq!(php_to_json("a:0:{}").unwrap(), "[]");
    }

    #[test]
    fn assoc_array_becomes_json_object_in_order() {
        assert_eq!(
            php_to_json("a:2:{s:4:\"name\";s:2:\"Al\";s:3:\"age\";i:30;}").unwrap(),
            "{\n  \"name\": \"Al\",\n  \"age\": 30\n}"
        );
    }

    #[test]
    fn integer_keyed_non_sequential_is_object() {
        // Keys 0 and 2 (gap) -> not a list -> object with stringified int keys.
        let v = val("a:2:{i:0;s:1:\"a\";i:2;s:1:\"c\";}");
        let obj = v.as_object().unwrap();
        assert_eq!(obj.get("0").unwrap(), &Value::String("a".into()));
        assert_eq!(obj.get("2").unwrap(), &Value::String("c".into()));
    }

    #[test]
    fn out_of_order_int_keys_is_object() {
        // Keys 1 then 0 -> not in 0..n order -> object, not a reordered list.
        let v = val("a:2:{i:1;i:9;i:0;i:8;}");
        assert!(v.is_object(), "got {v}");
    }

    #[test]
    fn nested() {
        assert_eq!(
            php_to_json("a:1:{s:5:\"items\";a:2:{i:0;b:1;i:1;N;}}").unwrap(),
            "{\n  \"items\": [\n    true,\n    null\n  ]\n}"
        );
    }

    #[test]
    fn object_keeps_class_name() {
        let v = val("O:4:\"User\":2:{s:2:\"id\";i:7;s:4:\"name\";s:2:\"Al\";}");
        let obj = v.as_object().unwrap();
        assert_eq!(obj.get("__class").unwrap(), &Value::String("User".into()));
        assert_eq!(obj.get("id").unwrap(), &Value::Number(7.into()));
        assert_eq!(obj.get("name").unwrap(), &Value::String("Al".into()));
    }

    #[test]
    fn rejects_truncated() {
        assert!(unserialize_to_value("i:42").is_err()); // missing ;
        assert!(unserialize_to_value("s:5:\"hi\";").is_err()); // length too long
        assert!(unserialize_to_value("").is_err());
    }

    #[test]
    fn rejects_trailing_garbage() {
        let err = unserialize_to_value("i:1;extra").unwrap_err();
        assert!(err.contains("trailing data"), "got: {err}");
    }

    #[test]
    fn rejects_unknown_tag() {
        let err = unserialize_to_value("x:1;").unwrap_err();
        assert!(err.contains("unsupported type tag"), "got: {err}");
    }
}
