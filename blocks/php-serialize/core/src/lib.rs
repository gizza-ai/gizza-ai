//! php-serialize core — converts JSON into PHP's `serialize()` wire format.
//! No wafer/wasm-bindgen deps. Shared by the chat skill block and the web page.
//!
//! PHP's `serialize()` produces a compact, type-tagged string used by
//! `unserialize()` and stored in PHP sessions, caches (e.g. memcached/Redis when
//! configured with the PHP serializer), and `WP_Options`. This converts a JSON
//! value into the equivalent serialized string so non-PHP systems can write data
//! a PHP app will read back:
//!
//! | JSON                | PHP serialize()                                  |
//! | ------------------- | ------------------------------------------------ |
//! | `null`              | `N;`                                             |
//! | `true` / `false`    | `b:1;` / `b:0;`                                  |
//! | integer `42`        | `i:42;`                                          |
//! | float `3.5`         | `d:3.5;`                                         |
//! | string `"hi"`       | `s:2:"hi";` (length is in **bytes**, not chars)  |
//! | array `[1,2]`       | `a:2:{i:0;i:1;i:1;i:2;}`                          |
//! | object `{"a":1}`    | `a:1:{s:1:"a";i:1;}`                              |
//!
//! PHP has no distinct "object/map" type for plain associative data — both JSON
//! arrays and objects map to a PHP `array` (`a:`), keyed by sequential integer
//! indices and by the object's string keys respectively. That matches what
//! `serialize(json_decode($s, true))` produces in PHP.

use serde_json::Value;

/// Parse `json` and render it as a PHP `serialize()` string.
///
/// Returns `Err` with a human-readable message if the input is not valid JSON.
/// Integers that fit `i64`/`u64` serialize as PHP `i:` (integer); any other
/// number — fractional, or an integer too large for 64 bits — serializes as
/// PHP `d:` (double/float), matching PHP's own behaviour when a literal
/// overflows the platform int.
pub fn json_to_php(json: &str) -> Result<String, String> {
    let value: Value = serde_json::from_str(json).map_err(|e| format!("invalid JSON: {e}"))?;
    let mut out = String::new();
    serialize_value(&value, &mut out);
    Ok(out)
}

fn serialize_value(value: &Value, out: &mut String) {
    match value {
        Value::Null => out.push_str("N;"),
        Value::Bool(b) => {
            out.push_str("b:");
            out.push(if *b { '1' } else { '0' });
            out.push(';');
        }
        Value::Number(n) => serialize_number(n, out),
        Value::String(s) => serialize_string(s, out),
        Value::Array(items) => {
            out.push_str(&format!("a:{}:{{", items.len()));
            for (i, item) in items.iter().enumerate() {
                // Sequential integer keys, exactly like PHP's list arrays.
                out.push_str(&format!("i:{i};"));
                serialize_value(item, out);
            }
            out.push('}');
        }
        Value::Object(map) => {
            out.push_str(&format!("a:{}:{{", map.len()));
            for (key, val) in map {
                serialize_string(key, out);
                serialize_value(val, out);
            }
            out.push('}');
        }
    }
}

fn serialize_number(n: &serde_json::Number, out: &mut String) {
    if let Some(i) = n.as_i64() {
        out.push_str(&format!("i:{i};"));
    } else if let Some(u) = n.as_u64() {
        out.push_str(&format!("i:{u};"));
    } else if let Some(f) = n.as_f64() {
        // PHP's serialize_precision = -1 (the default since PHP 7.1) emits the
        // shortest float string that round-trips — which is exactly what Rust's
        // default `{}` float formatting produces.
        out.push_str(&format!("d:{f};"));
    } else {
        // Should be unreachable for a serde_json::Number, but never panic.
        out.push_str("d:0;");
    }
}

/// PHP measures string length in BYTES (its strings are byte arrays), so a
/// multi-byte UTF-8 string is longer than its character count.
fn serialize_string(s: &str, out: &mut String) {
    out.push_str(&format!("s:{}:\"{}\";", s.len(), s));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalars() {
        assert_eq!(json_to_php("null").unwrap(), "N;");
        assert_eq!(json_to_php("true").unwrap(), "b:1;");
        assert_eq!(json_to_php("false").unwrap(), "b:0;");
        assert_eq!(json_to_php("42").unwrap(), "i:42;");
        assert_eq!(json_to_php("-7").unwrap(), "i:-7;");
    }

    #[test]
    fn floats() {
        assert_eq!(json_to_php("3.5").unwrap(), "d:3.5;");
        assert_eq!(json_to_php("0.1").unwrap(), "d:0.1;");
        // An integer literal too large for i64/u64 falls back to a double, like PHP.
        assert_eq!(
            json_to_php("100000000000000000000").unwrap(),
            "d:100000000000000000000;"
        );
    }

    #[test]
    fn strings_use_byte_length() {
        assert_eq!(json_to_php("\"hi\"").unwrap(), "s:2:\"hi\";");
        assert_eq!(json_to_php("\"\"").unwrap(), "s:0:\"\";");
        // "é" is 2 bytes in UTF-8 -> length 2, not 1.
        assert_eq!(json_to_php("\"é\"").unwrap(), "s:2:\"é\";");
        // "€" is 3 bytes.
        assert_eq!(json_to_php("\"€\"").unwrap(), "s:3:\"€\";");
    }

    #[test]
    fn list_array() {
        assert_eq!(
            json_to_php("[1, 2, 3]").unwrap(),
            "a:3:{i:0;i:1;i:1;i:2;i:2;i:3;}"
        );
        assert_eq!(json_to_php("[]").unwrap(), "a:0:{}");
    }

    #[test]
    fn assoc_array_preserves_key_order() {
        assert_eq!(
            json_to_php("{\"name\": \"Al\", \"age\": 30}").unwrap(),
            "a:2:{s:4:\"name\";s:2:\"Al\";s:3:\"age\";i:30;}"
        );
        assert_eq!(json_to_php("{}").unwrap(), "a:0:{}");
    }

    #[test]
    fn nested() {
        assert_eq!(
            json_to_php("{\"items\": [true, null]}").unwrap(),
            "a:1:{s:5:\"items\";a:2:{i:0;b:1;i:1;N;}}"
        );
    }

    #[test]
    fn rejects_invalid_json() {
        let err = json_to_php("{not json}").unwrap_err();
        assert!(err.contains("invalid JSON"), "got: {err}");
        assert!(json_to_php("").is_err());
    }
}
