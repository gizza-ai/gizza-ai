//! bencode-decoder core — convert between bencode (the BitTorrent serialization
//! format) and JSON, in both directions. Pure Rust, no external crates, no
//! wafer/wasm-bindgen deps.
//!
//! Bencode has four types:
//!   * integer        `i<digits>e`            -> JSON number
//!   * byte string    `<len>:<len bytes>`     -> JSON string (or hex sentinel, below)
//!   * list           `l <items...> e`        -> JSON array
//!   * dictionary     `d <key value ...> e`   -> JSON object (keys sorted, byte-string keys)
//!
//! Byte strings are raw bytes, not necessarily UTF-8 (e.g. a torrent's `pieces`
//! field is a concatenation of 20-byte SHA-1 hashes). To stay LOSSLESS and
//! round-trippable we represent a byte string that is valid UTF-8 as a plain
//! JSON string, and a byte string that is NOT valid UTF-8 as a one-key sentinel
//! object `{ "_bencode_bytes_hex": "<lowercase hex>" }`. The JSON->bencode
//! encoder understands the same sentinel, so decode(encode(x)) == x for any
//! bencode input.

use serde_json::{Map, Number, Value};

/// Sentinel key used to carry non-UTF-8 byte strings through JSON losslessly.
const BYTES_SENTINEL: &str = "_bencode_bytes_hex";

// ----------------------------------------------------------------------------
// decode: bencode bytes -> JSON Value
// ----------------------------------------------------------------------------

/// Decode bencode `data` to a pretty (or compact) JSON string.
pub fn decode_to_json(data: &[u8], pretty: bool) -> Result<String, String> {
    let mut p = Parser { data, pos: 0 };
    let v = p.parse_value()?;
    if p.pos != data.len() {
        return Err(format!(
            "trailing data after top-level bencode value (parsed {} of {} bytes)",
            p.pos,
            data.len()
        ));
    }
    if pretty {
        serde_json::to_string_pretty(&v).map_err(|e| format!("JSON serialize failed: {e}"))
    } else {
        serde_json::to_string(&v).map_err(|e| format!("JSON serialize failed: {e}"))
    }
}

struct Parser<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn peek(&self) -> Result<u8, String> {
        self.data
            .get(self.pos)
            .copied()
            .ok_or_else(|| "unexpected end of input".to_string())
    }

    fn parse_value(&mut self) -> Result<Value, String> {
        match self.peek()? {
            b'i' => self.parse_int(),
            b'l' => self.parse_list(),
            b'd' => self.parse_dict(),
            b'0'..=b'9' => {
                let bytes = self.parse_byte_string()?;
                Ok(bytes_to_json(&bytes))
            }
            other => Err(format!(
                "unexpected byte 0x{other:02x} ('{}') at position {}",
                escape_byte(other),
                self.pos
            )),
        }
    }

    fn parse_int(&mut self) -> Result<Value, String> {
        // 'i' <optional '-'> <digits> 'e'   (no leading zeros, no "-0")
        debug_assert_eq!(self.data[self.pos], b'i');
        self.pos += 1;
        let start = self.pos;
        while self.pos < self.data.len() && self.data[self.pos] != b'e' {
            self.pos += 1;
        }
        if self.pos >= self.data.len() {
            return Err("unterminated integer (missing 'e')".into());
        }
        let s = std::str::from_utf8(&self.data[start..self.pos])
            .map_err(|_| "integer contains non-ASCII bytes".to_string())?;
        self.pos += 1; // consume 'e'
        if s.is_empty() {
            return Err("empty integer".into());
        }
        if s == "-0" {
            return Err("invalid integer '-0'".into());
        }
        // Reject leading zeros like "03" or "-03" (canonical bencode forbids them).
        let digits = s.strip_prefix('-').unwrap_or(s);
        if digits.len() > 1 && digits.starts_with('0') {
            return Err(format!("invalid integer with leading zero: '{s}'"));
        }
        let n: i64 = s
            .parse()
            .map_err(|_| format!("integer '{s}' is not a valid i64"))?;
        Ok(Value::Number(Number::from(n)))
    }

    fn parse_byte_string(&mut self) -> Result<Vec<u8>, String> {
        // <len> ':' <len bytes>
        let start = self.pos;
        while self.pos < self.data.len() && self.data[self.pos] != b':' {
            self.pos += 1;
        }
        if self.pos >= self.data.len() {
            return Err("byte string missing ':' length separator".into());
        }
        let len_str = std::str::from_utf8(&self.data[start..self.pos])
            .map_err(|_| "byte-string length is not ASCII".to_string())?;
        if len_str.len() > 1 && len_str.starts_with('0') {
            return Err(format!("byte-string length has leading zero: '{len_str}'"));
        }
        let len: usize = len_str
            .parse()
            .map_err(|_| format!("invalid byte-string length: '{len_str}'"))?;
        self.pos += 1; // consume ':'
        let end = self
            .pos
            .checked_add(len)
            .ok_or_else(|| "byte-string length overflow".to_string())?;
        if end > self.data.len() {
            return Err(format!(
                "byte string claims {len} bytes but only {} remain",
                self.data.len() - self.pos
            ));
        }
        let out = self.data[self.pos..end].to_vec();
        self.pos = end;
        Ok(out)
    }

    fn parse_list(&mut self) -> Result<Value, String> {
        debug_assert_eq!(self.data[self.pos], b'l');
        self.pos += 1;
        let mut items = Vec::new();
        loop {
            if self.peek()? == b'e' {
                self.pos += 1;
                break;
            }
            items.push(self.parse_value()?);
        }
        Ok(Value::Array(items))
    }

    fn parse_dict(&mut self) -> Result<Value, String> {
        debug_assert_eq!(self.data[self.pos], b'd');
        self.pos += 1;
        let mut map = Map::new();
        loop {
            if self.peek()? == b'e' {
                self.pos += 1;
                break;
            }
            // Keys MUST be byte strings.
            if !self.peek()?.is_ascii_digit() {
                return Err("dictionary key is not a byte string".into());
            }
            let key_bytes = self.parse_byte_string()?;
            let key = match String::from_utf8(key_bytes.clone()) {
                Ok(s) => s,
                // A non-UTF-8 dict key is exotic; carry it as a hex-tagged key so
                // it still round-trips.
                Err(_) => format!("{BYTES_SENTINEL}:{}", to_hex(&key_bytes)),
            };
            let value = self.parse_value()?;
            map.insert(key, value);
        }
        Ok(Value::Object(map))
    }
}

/// A bencode byte string becomes a plain JSON string when valid UTF-8, else a
/// `{ "_bencode_bytes_hex": "<hex>" }` sentinel object.
fn bytes_to_json(bytes: &[u8]) -> Value {
    match std::str::from_utf8(bytes) {
        Ok(s) => Value::String(s.to_string()),
        Err(_) => {
            let mut m = Map::new();
            m.insert(BYTES_SENTINEL.to_string(), Value::String(to_hex(bytes)));
            Value::Object(m)
        }
    }
}

// ----------------------------------------------------------------------------
// encode: JSON Value -> bencode bytes
// ----------------------------------------------------------------------------

/// Encode a JSON string to canonical bencode bytes.
pub fn encode_from_json(json: &str) -> Result<Vec<u8>, String> {
    let v: Value = serde_json::from_str(json).map_err(|e| format!("invalid JSON: {e}"))?;
    let mut out = Vec::new();
    encode_value(&v, &mut out)?;
    Ok(out)
}

fn encode_value(v: &Value, out: &mut Vec<u8>) -> Result<(), String> {
    match v {
        Value::Number(n) => {
            let i = n
                .as_i64()
                .ok_or_else(|| format!("bencode integers must be whole i64 (got {n})"))?;
            out.extend_from_slice(format!("i{i}e").as_bytes());
            Ok(())
        }
        Value::String(s) => {
            encode_bytes(s.as_bytes(), out);
            Ok(())
        }
        Value::Array(items) => {
            out.push(b'l');
            for it in items {
                encode_value(it, out)?;
            }
            out.push(b'e');
            Ok(())
        }
        Value::Object(map) => {
            // The sentinel object encodes back to its original raw bytes.
            if map.len() == 1 {
                if let Some(Value::String(hex)) = map.get(BYTES_SENTINEL) {
                    let raw = from_hex(hex)?;
                    encode_bytes(&raw, out);
                    return Ok(());
                }
            }
            // Bencode dict keys MUST be sorted by raw byte value.
            let mut entries: Vec<(Vec<u8>, &Value)> = Vec::with_capacity(map.len());
            for (k, val) in map {
                entries.push((key_to_bytes(k)?, val));
            }
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            out.push(b'd');
            for (kb, val) in entries {
                encode_bytes(&kb, out);
                encode_value(val, out)?;
            }
            out.push(b'e');
            Ok(())
        }
        Value::Bool(_) => Err("bencode has no boolean type (use 0/1)".into()),
        Value::Null => Err("bencode has no null type".into()),
    }
}

/// A dict key is normally a UTF-8 string, but a non-UTF-8 key produced by the
/// decoder is carried as `"_bencode_bytes_hex:<hex>"`.
fn key_to_bytes(k: &str) -> Result<Vec<u8>, String> {
    if let Some(hex) = k.strip_prefix(&format!("{BYTES_SENTINEL}:")) {
        from_hex(hex)
    } else {
        Ok(k.as_bytes().to_vec())
    }
}

fn encode_bytes(bytes: &[u8], out: &mut Vec<u8>) {
    out.extend_from_slice(format!("{}:", bytes.len()).as_bytes());
    out.extend_from_slice(bytes);
}

// ----------------------------------------------------------------------------
// hex helpers
// ----------------------------------------------------------------------------

fn to_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(char::from_digit((b >> 4) as u32, 16).unwrap());
        s.push(char::from_digit((b & 0x0f) as u32, 16).unwrap());
    }
    s
}

fn from_hex(s: &str) -> Result<Vec<u8>, String> {
    if s.len() % 2 != 0 {
        return Err(format!("hex string has odd length ({})", s.len()));
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let hi = hex_nibble(bytes[i])?;
        let lo = hex_nibble(bytes[i + 1])?;
        out.push((hi << 4) | lo);
        i += 2;
    }
    Ok(out)
}

fn hex_nibble(c: u8) -> Result<u8, String> {
    match c {
        b'0'..=b'9' => Ok(c - b'0'),
        b'a'..=b'f' => Ok(c - b'a' + 10),
        b'A'..=b'F' => Ok(c - b'A' + 10),
        other => Err(format!("invalid hex digit '{}'", escape_byte(other))),
    }
}

fn escape_byte(b: u8) -> String {
    if b.is_ascii_graphic() || b == b' ' {
        (b as char).to_string()
    } else {
        format!("\\x{b:02x}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_int() {
        assert_eq!(decode_to_json(b"i42e", false).unwrap(), "42");
        assert_eq!(decode_to_json(b"i-7e", false).unwrap(), "-7");
        assert_eq!(decode_to_json(b"i0e", false).unwrap(), "0");
    }

    #[test]
    fn decode_string() {
        assert_eq!(decode_to_json(b"4:spam", false).unwrap(), "\"spam\"");
        assert_eq!(decode_to_json(b"0:", false).unwrap(), "\"\"");
    }

    #[test]
    fn decode_list() {
        assert_eq!(
            decode_to_json(b"l4:spami42ee", false).unwrap(),
            "[\"spam\",42]"
        );
        assert_eq!(decode_to_json(b"le", false).unwrap(), "[]");
    }

    #[test]
    fn decode_dict_sorts_keys() {
        // serde_json preserves insertion order; bencode dicts are stored sorted,
        // and the decoder inserts in stream order which is already sorted.
        let json = decode_to_json(b"d3:cow3:moo4:spam4:eggse", false).unwrap();
        assert_eq!(json, "{\"cow\":\"moo\",\"spam\":\"eggs\"}");
    }

    #[test]
    fn decode_non_utf8_string_sentinel() {
        // 2:<0xff 0xfe>  -> sentinel object with hex
        let data = [b'2', b':', 0xff, 0xfe];
        let json = decode_to_json(&data, false).unwrap();
        assert_eq!(json, "{\"_bencode_bytes_hex\":\"fffe\"}");
    }

    #[test]
    fn decode_errors() {
        assert!(decode_to_json(b"i03e", false).is_err()); // leading zero
        assert!(decode_to_json(b"i-0e", false).is_err()); // negative zero
        assert!(decode_to_json(b"ie", false).is_err()); // empty int
        assert!(decode_to_json(b"5:abc", false).is_err()); // length too long
        assert!(decode_to_json(b"4:spamextra", false).is_err()); // trailing data
        assert!(decode_to_json(b"l4:spam", false).is_err()); // unterminated list
        assert!(decode_to_json(b"di1e3:fooe", false).is_err()); // non-string key
    }

    #[test]
    fn encode_basic() {
        assert_eq!(encode_from_json("42").unwrap(), b"i42e");
        assert_eq!(encode_from_json("\"spam\"").unwrap(), b"4:spam");
        assert_eq!(encode_from_json("[\"spam\",42]").unwrap(), b"l4:spami42ee");
    }

    #[test]
    fn encode_sorts_dict_keys() {
        // Input order spam, cow -> output must be sorted cow, spam.
        let out = encode_from_json("{\"spam\":\"eggs\",\"cow\":\"moo\"}").unwrap();
        assert_eq!(out, b"d3:cow3:moo4:spam4:eggse".to_vec());
    }

    #[test]
    fn encode_sentinel_roundtrip() {
        let out = encode_from_json("{\"_bencode_bytes_hex\":\"fffe\"}").unwrap();
        assert_eq!(out, vec![b'2', b':', 0xff, 0xfe]);
    }

    #[test]
    fn encode_rejects_bool_null() {
        assert!(encode_from_json("true").is_err());
        assert!(encode_from_json("null").is_err());
    }

    #[test]
    fn roundtrip_lossless_binary() {
        // A "torrent-ish" structure with binary pieces (non-UTF8). Keys are in
        // canonical (byte-sorted) order — "name" < "pieces" — so the encoder's
        // re-sort reproduces the exact input.
        let mut data = Vec::new();
        data.extend_from_slice(b"d4:name5:hello6:pieces");
        let pieces = vec![0x00u8, 0xff, 0x01, 0xfe, 0x80];
        data.extend_from_slice(format!("{}:", pieces.len()).as_bytes());
        data.extend_from_slice(&pieces);
        data.extend_from_slice(b"e");

        let json = decode_to_json(&data, true).unwrap();
        let back = encode_from_json(&json).unwrap();
        assert_eq!(back, data, "decode->encode must reproduce the exact bytes");
    }

    #[test]
    fn roundtrip_nested() {
        let data = b"d4:listl1:a1:b3:cccee";
        let json = decode_to_json(data, false).unwrap();
        let back = encode_from_json(&json).unwrap();
        assert_eq!(back, data.to_vec());
    }
}
