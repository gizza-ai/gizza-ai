//! json-to-messagepack core — encode JSON values into MessagePack bytes.
//!
//! The encoder is hand-written so every wire-level choice a user might care about
//! (key order, float width, str8 availability) is an explicit option rather than a
//! serializer default, and so `output = "annotated"` can report the exact header
//! byte emitted for each value.

use base64::Engine;
use serde_json::{Map, Number, Value};

/// Largest accepted JSON input, in UTF-8 bytes.
pub const MAX_INPUT_BYTES: usize = 1_000_000;

#[derive(Clone, Debug)]
pub struct Options {
    /// `hex` | `base64` | `bytes` | `annotated` | `summary` | `json`.
    pub output: String,
    /// `input` keeps document order; `sorted` sorts map keys by raw UTF-8 key bytes.
    pub key_order: String,
    /// Emit float32 whenever the value round-trips exactly.
    pub compact_floats: bool,
    /// `new` uses the str8 header; `old` omits it for pre-2013 decoders.
    pub spec: String,
    /// Insert a space every N bytes in hex output. 0 = continuous.
    pub group: u32,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            output: "hex".to_string(),
            key_order: "input".to_string(),
            compact_floats: false,
            spec: "new".to_string(),
            group: 0,
        }
    }
}

/// One encoded value, recorded so `output = "annotated"` can explain the payload.
struct Note {
    offset: usize,
    depth: usize,
    header: Vec<u8>,
    tag: &'static str,
    meaning: String,
}

struct Encoder {
    out: Vec<u8>,
    notes: Vec<Note>,
    sort_keys: bool,
    compact_floats: bool,
    str8: bool,
}

pub fn run(json: &str) -> Result<String, String> {
    run_with_options(json, &Options::default())
}

pub fn run_with_options(json: &str, options: &Options) -> Result<String, String> {
    let input = json.trim();
    if input.is_empty() {
        return Err("json input is required".to_string());
    }
    if input.len() > MAX_INPUT_BYTES {
        return Err(format!(
            "json input is too large (max {MAX_INPUT_BYTES} bytes)"
        ));
    }
    let sort_keys = match options.key_order.trim().to_ascii_lowercase().as_str() {
        "input" | "" => false,
        "sorted" => true,
        other => return Err(format!("unknown key_order '{other}' (use input or sorted)")),
    };
    let str8 = match options.spec.trim().to_ascii_lowercase().as_str() {
        "new" | "" => true,
        "old" => false,
        other => return Err(format!("unknown spec '{other}' (use new or old)")),
    };
    // Reject an unknown output format before doing the encoding work.
    let format = options.output.trim().to_ascii_lowercase();
    let format = match format.as_str() {
        "" => "hex",
        f @ ("hex" | "base64" | "bytes" | "annotated" | "summary" | "json") => f,
        other => {
            return Err(format!(
            "unknown output format '{other}' (use hex, base64, bytes, annotated, summary, or json)"
        ))
        }
    };

    let value: Value = serde_json::from_str(input).map_err(|e| format!("invalid JSON: {e}"))?;
    let mut enc = Encoder {
        out: Vec::new(),
        notes: Vec::new(),
        sort_keys,
        compact_floats: options.compact_floats,
        str8,
    };
    enc.value(&value, 0)?;

    let bytes = &enc.out;
    Ok(match format {
        "hex" => format_hex(bytes, options.group),
        "base64" => base64::engine::general_purpose::STANDARD.encode(bytes),
        "bytes" => format_byte_array(bytes),
        "annotated" => format_annotated(&enc.notes, bytes),
        "summary" => format_summary(input, bytes, options.group),
        _ => format_json(input, bytes, options.group),
    })
}

impl Encoder {
    fn note(
        &mut self,
        offset: usize,
        depth: usize,
        header_len: usize,
        tag: &'static str,
        meaning: String,
    ) {
        let header = self.out[offset..offset + header_len].to_vec();
        self.notes.push(Note {
            offset,
            depth,
            header,
            tag,
            meaning,
        });
    }

    fn value(&mut self, value: &Value, depth: usize) -> Result<(), String> {
        if depth > 64 {
            return Err("JSON nesting is too deep (max 64 levels)".to_string());
        }
        let at = self.out.len();
        match value {
            Value::Null => {
                self.out.push(0xc0);
                self.note(at, depth, 1, "nil", "null".to_string());
            }
            Value::Bool(false) => {
                self.out.push(0xc2);
                self.note(at, depth, 1, "false", "false".to_string());
            }
            Value::Bool(true) => {
                self.out.push(0xc3);
                self.note(at, depth, 1, "true", "true".to_string());
            }
            Value::Number(n) => self.number(n, depth)?,
            Value::String(s) => self.string(s, depth),
            Value::Array(items) => {
                let tag = self.array_header(items.len())?;
                let header_len = self.out.len() - at;
                self.note(
                    at,
                    depth,
                    header_len,
                    tag,
                    format!("array of {} element(s)", items.len()),
                );
                for item in items {
                    self.value(item, depth + 1)?;
                }
            }
            Value::Object(map) => self.object(map, depth)?,
        }
        Ok(())
    }

    fn number(&mut self, n: &Number, depth: usize) -> Result<(), String> {
        let at = self.out.len();
        if let Some(u) = n.as_u64() {
            let tag = self.uint(u);
            let header_len = self.out.len() - at;
            self.note(at, depth, header_len, tag, format!("unsigned int {u}"));
            return Ok(());
        }
        if let Some(i) = n.as_i64() {
            let tag = self.int(i);
            let header_len = self.out.len() - at;
            self.note(at, depth, header_len, tag, format!("signed int {i}"));
            return Ok(());
        }
        let f = n
            .as_f64()
            .ok_or_else(|| "JSON number cannot be represented as a 64-bit float".to_string())?;
        let tag = self.float(f);
        let header_len = self.out.len() - at;
        self.note(at, depth, header_len, tag, format!("float {f}"));
        Ok(())
    }

    fn uint(&mut self, u: u64) -> &'static str {
        match u {
            0..=0x7f => {
                self.out.push(u as u8);
                "positive fixint"
            }
            0x80..=0xff => {
                self.out.push(0xcc);
                self.out.push(u as u8);
                "uint 8"
            }
            0x100..=0xffff => {
                self.out.push(0xcd);
                self.out.extend_from_slice(&(u as u16).to_be_bytes());
                "uint 16"
            }
            0x1_0000..=0xffff_ffff => {
                self.out.push(0xce);
                self.out.extend_from_slice(&(u as u32).to_be_bytes());
                "uint 32"
            }
            _ => {
                self.out.push(0xcf);
                self.out.extend_from_slice(&u.to_be_bytes());
                "uint 64"
            }
        }
    }

    fn int(&mut self, i: i64) -> &'static str {
        // Only negative values reach here — non-negative ones took the u64 path.
        if i >= -32 {
            self.out.push((i as i8) as u8);
            "negative fixint"
        } else if i >= i8::MIN as i64 {
            self.out.push(0xd0);
            self.out.push((i as i8) as u8);
            "int 8"
        } else if i >= i16::MIN as i64 {
            self.out.push(0xd1);
            self.out.extend_from_slice(&(i as i16).to_be_bytes());
            "int 16"
        } else if i >= i32::MIN as i64 {
            self.out.push(0xd2);
            self.out.extend_from_slice(&(i as i32).to_be_bytes());
            "int 32"
        } else {
            self.out.push(0xd3);
            self.out.extend_from_slice(&i.to_be_bytes());
            "int 64"
        }
    }

    fn float(&mut self, f: f64) -> &'static str {
        if self.compact_floats && (f as f32) as f64 == f {
            self.out.push(0xca);
            self.out.extend_from_slice(&(f as f32).to_be_bytes());
            return "float 32";
        }
        self.out.push(0xcb);
        self.out.extend_from_slice(&f.to_be_bytes());
        "float 64"
    }

    fn string(&mut self, s: &str, depth: usize) {
        let at = self.out.len();
        let len = s.len();
        let tag = match len {
            0..=31 => {
                self.out.push(0xa0 | len as u8);
                "fixstr"
            }
            // str8 only exists in the 2013 spec revision; `spec = "old"` falls
            // through to str16 (the old `raw 16` header, same byte) instead.
            32..=0xff if self.str8 => {
                self.out.push(0xd9);
                self.out.push(len as u8);
                "str 8"
            }
            32..=0xffff => {
                self.out.push(0xda);
                self.out.extend_from_slice(&(len as u16).to_be_bytes());
                "str 16"
            }
            _ => {
                self.out.push(0xdb);
                self.out.extend_from_slice(&(len as u32).to_be_bytes());
                "str 32"
            }
        };
        let header_len = self.out.len() - at;
        self.out.extend_from_slice(s.as_bytes());
        self.note(
            at,
            depth,
            header_len,
            tag,
            format!("string {:?}", truncate(s)),
        );
    }

    fn array_header(&mut self, len: usize) -> Result<&'static str, String> {
        match len {
            0..=15 => {
                self.out.push(0x90 | len as u8);
                Ok("fixarray")
            }
            16..=0xffff => {
                self.out.push(0xdc);
                self.out.extend_from_slice(&(len as u16).to_be_bytes());
                Ok("array 16")
            }
            _ if len <= u32::MAX as usize => {
                self.out.push(0xdd);
                self.out.extend_from_slice(&(len as u32).to_be_bytes());
                Ok("array 32")
            }
            _ => Err("array is too long for MessagePack (max 2^32-1 elements)".to_string()),
        }
    }

    fn object(&mut self, map: &Map<String, Value>, depth: usize) -> Result<(), String> {
        let at = self.out.len();
        let len = map.len();
        let tag = match len {
            0..=15 => {
                self.out.push(0x80 | len as u8);
                "fixmap"
            }
            16..=0xffff => {
                self.out.push(0xde);
                self.out.extend_from_slice(&(len as u16).to_be_bytes());
                "map 16"
            }
            _ if len <= u32::MAX as usize => {
                self.out.push(0xdf);
                self.out.extend_from_slice(&(len as u32).to_be_bytes());
                "map 32"
            }
            _ => return Err("object has too many keys for MessagePack (max 2^32-1)".to_string()),
        };
        let header_len = self.out.len() - at;
        self.note(at, depth, header_len, tag, format!("map of {len} pair(s)"));

        let mut keys: Vec<&String> = map.keys().collect();
        if self.sort_keys {
            keys.sort_by(|a, b| a.as_bytes().cmp(b.as_bytes()));
        }
        for key in keys {
            self.string(key, depth + 1);
            self.value(&map[key], depth + 1)?;
        }
        Ok(())
    }
}

fn truncate(s: &str) -> String {
    if s.chars().count() <= 24 {
        return s.to_string();
    }
    let head: String = s.chars().take(24).collect();
    format!("{head}…")
}

fn hex_of(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn format_hex(bytes: &[u8], group: u32) -> String {
    let raw = hex_of(bytes);
    if group == 0 {
        return raw;
    }
    let chars_per_group = (group as usize).saturating_mul(2).max(2);
    raw.as_bytes()
        .chunks(chars_per_group)
        .map(|chunk| std::str::from_utf8(chunk).unwrap_or(""))
        .collect::<Vec<_>>()
        .join(" ")
}

fn format_byte_array(bytes: &[u8]) -> String {
    let inner = bytes
        .iter()
        .map(|b| b.to_string())
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{inner}]")
}

fn saving_percent(json_bytes: usize, msgpack_bytes: usize) -> f64 {
    if json_bytes == 0 {
        return 0.0;
    }
    let raw = (json_bytes as f64 - msgpack_bytes as f64) / json_bytes as f64 * 100.0;
    (raw * 10.0).round() / 10.0
}

fn format_annotated(notes: &[Note], bytes: &[u8]) -> String {
    let mut lines = vec![format!("offset  bytes      type              value")];
    for note in notes {
        let indent = "  ".repeat(note.depth.min(16));
        lines.push(format!(
            "{:<7} {:<10} {:<17} {indent}{}",
            format!("{:04x}", note.offset),
            hex_of(&note.header),
            note.tag,
            note.meaning
        ));
    }
    lines.push(String::new());
    lines.push(format!("total: {} byte(s)", bytes.len()));
    lines.join("\n")
}

fn format_summary(input: &str, bytes: &[u8], group: u32) -> String {
    let json_bytes = input.as_bytes().len();
    format!(
        "MessagePack bytes: {}\nJSON bytes: {}\nSize saving: {}%\nHex: {}\nBase64: {}",
        bytes.len(),
        json_bytes,
        saving_percent(json_bytes, bytes.len()),
        format_hex(bytes, group),
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}

fn format_json(input: &str, bytes: &[u8], group: u32) -> String {
    let json_bytes = input.as_bytes().len();
    format!(
        "{{\"encoding\":\"messagepack\",\"msgpack_bytes\":{},\"json_bytes\":{},\"saving_percent\":{},\"hex\":\"{}\",\"base64\":\"{}\"}}",
        bytes.len(),
        json_bytes,
        saving_percent(json_bytes, bytes.len()),
        format_hex(bytes, group),
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn with(output: &str) -> Options {
        Options {
            output: output.to_string(),
            ..Options::default()
        }
    }

    #[test]
    fn encodes_fixmap_of_two_pairs_as_hex() {
        // 82 = fixmap(2); a1 61 = fixstr "a"; 01; a1 62 = fixstr "b"; 02.
        assert_eq!(run(r#"{"a":1,"b":2}"#).unwrap(), "82a16101a16202");
    }

    #[test]
    fn preserves_document_key_order_by_default() {
        assert_eq!(run(r#"{"b":2,"a":1}"#).unwrap(), "82a16202a16101");
    }

    #[test]
    fn sorted_key_order_is_reproducible() {
        let opts = Options {
            key_order: "sorted".into(),
            ..Options::default()
        };
        assert_eq!(
            run_with_options(r#"{"b":2,"a":1}"#, &opts).unwrap(),
            "82a16101a16202"
        );
    }

    #[test]
    fn encodes_scalars_and_arrays() {
        // 94 = fixarray(4); 01; c3 true; c0 nil; a1 78 = "x".
        assert_eq!(run(r#"[1,true,null,"x"]"#).unwrap(), "9401c3c0a178");
    }

    #[test]
    fn negative_ints_use_the_narrowest_header() {
        assert_eq!(run("-1").unwrap(), "ff");
        assert_eq!(run("-32").unwrap(), "e0");
        assert_eq!(run("-33").unwrap(), "d0df");
        assert_eq!(run("-200").unwrap(), "d1ff38");
    }

    #[test]
    fn unsigned_ints_use_the_narrowest_header() {
        assert_eq!(run("0").unwrap(), "00");
        assert_eq!(run("127").unwrap(), "7f");
        assert_eq!(run("128").unwrap(), "cc80");
        assert_eq!(run("300").unwrap(), "cd012c");
        assert_eq!(run("70000").unwrap(), "ce00011170");
        assert_eq!(run("5000000000").unwrap(), "cf000000012a05f200");
    }

    #[test]
    fn floats_default_to_float64() {
        assert_eq!(run("1.5").unwrap(), "cb3ff8000000000000");
    }

    #[test]
    fn compact_floats_downgrades_only_when_lossless() {
        let opts = Options {
            compact_floats: true,
            ..Options::default()
        };
        // 1.5 is exact in float32; 0.1 is not, so it stays float64.
        assert_eq!(run_with_options("1.5", &opts).unwrap(), "ca3fc00000");
        assert_eq!(
            run_with_options("0.1", &opts).unwrap(),
            "cb3fb999999999999a"
        );
    }

    #[test]
    fn new_spec_uses_str8_and_old_spec_falls_back_to_str16() {
        let long = "x".repeat(40);
        let json = format!("\"{long}\"");
        assert!(run(&json).unwrap().starts_with("d928"));
        let opts = Options {
            spec: "old".into(),
            ..Options::default()
        };
        assert!(run_with_options(&json, &opts)
            .unwrap()
            .starts_with("da0028"));
    }

    #[test]
    fn base64_and_byte_array_views_match_the_hex() {
        assert_eq!(
            run_with_options(r#"[1,true,null,"x"]"#, &with("base64")).unwrap(),
            "lAHDwKF4"
        );
        assert_eq!(
            run_with_options(r#"[1,true,null,"x"]"#, &with("bytes")).unwrap(),
            "[148, 1, 195, 192, 161, 120]"
        );
    }

    #[test]
    fn summary_reports_sizes_and_saving() {
        let out = run_with_options(r#"{"ok":true}"#, &with("summary")).unwrap();
        assert!(out.contains("MessagePack bytes: 5"), "{out}");
        assert!(out.contains("JSON bytes: 11"), "{out}");
        assert!(out.contains("Size saving: 54.5%"), "{out}");
        assert!(out.contains("Hex: 81a26f6bc3"), "{out}");
    }

    #[test]
    fn json_output_is_machine_readable() {
        let out = run_with_options(r#"{"ok":true}"#, &with("json")).unwrap();
        let parsed: Value = serde_json::from_str(&out).unwrap();
        assert_eq!(parsed["msgpack_bytes"], 5);
        assert_eq!(parsed["json_bytes"], 11);
        assert_eq!(parsed["hex"], "81a26f6bc3");
    }

    #[test]
    fn annotated_output_names_each_header() {
        let out = run_with_options(r#"{"id":42}"#, &with("annotated")).unwrap();
        assert!(out.contains("fixmap"), "{out}");
        assert!(out.contains("fixstr"), "{out}");
        assert!(out.contains("positive fixint"), "{out}");
        assert!(out.contains("total: 5 byte(s)"), "{out}");
    }

    #[test]
    fn grouped_hex_inserts_spaces() {
        let opts = Options {
            group: 2,
            ..Options::default()
        };
        assert_eq!(
            run_with_options(r#"{"ok":true}"#, &opts).unwrap(),
            "81a2 6f6b c3"
        );
    }

    #[test]
    fn map_and_array_16_headers() {
        let arr: Vec<String> = (0..20).map(|i| i.to_string()).collect();
        let json = format!("[{}]", arr.join(","));
        assert!(run(&json).unwrap().starts_with("dc0014"));
    }

    #[test]
    fn rejects_invalid_json() {
        assert!(run("{bad").unwrap_err().contains("invalid JSON"));
    }

    #[test]
    fn rejects_empty_input() {
        assert_eq!(run("   ").unwrap_err(), "json input is required");
    }

    #[test]
    fn rejects_unknown_output_format() {
        let err = run_with_options("1", &with("yaml")).unwrap_err();
        assert!(err.contains("unknown output format 'yaml'"), "{err}");
    }

    #[test]
    fn rejects_unknown_key_order_and_spec() {
        let opts = Options {
            key_order: "random".into(),
            ..Options::default()
        };
        assert!(run_with_options("1", &opts)
            .unwrap_err()
            .contains("key_order"));
        let opts = Options {
            spec: "v2".into(),
            ..Options::default()
        };
        assert!(run_with_options("1", &opts).unwrap_err().contains("spec"));
    }

    #[test]
    fn rejects_oversized_input() {
        let big = format!("\"{}\"", "a".repeat(MAX_INPUT_BYTES));
        assert!(run(&big).unwrap_err().contains("too large"));
    }
}
