//! plist-viewer core — parse an Apple property list (XML `.plist` or binary
//! `bplist00`) and render it as readable JSON or a `plutil -p`-style key/value
//! tree. Pure compute, shared by the chat skill block and the web page.
//!
//! Binary plists can't survive as UTF-8 text, so the single string input accepts
//! either raw XML plist source or a Base64 blob (which is decoded first); the
//! `plist` reader then auto-detects XML vs binary from the magic bytes.

use std::io::Cursor;

use base64::Engine;
use plist::Value;
use serde::Serialize;
use serde_json::Value as Json;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Format {
    Json,
    Tree,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DataEncoding {
    Base64,
    Hex,
}

#[derive(Clone, Debug)]
pub struct Options {
    /// `json` (default) renders an equivalent JSON document; `tree` renders a
    /// readable indented key/value outline.
    pub format: Format,
    /// Spaces per indent level (clamped 0..=8).
    pub indent: usize,
    /// Sort dictionary keys alphabetically instead of keeping plist order.
    pub sort_keys: bool,
    /// How `<data>` byte blobs are rendered.
    pub data_encoding: DataEncoding,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            format: Format::Json,
            indent: 2,
            sort_keys: false,
            data_encoding: DataEncoding::Base64,
        }
    }
}

/// Parse a property list and render it per `opt`.
pub fn convert(input: &str, opt: &Options) -> Result<String, String> {
    let bytes = decode_input(input)?;
    let value = Value::from_reader(Cursor::new(bytes))
        .map_err(|e| format!("failed to parse property list: {e}"))?;
    Ok(match opt.format {
        Format::Json => render_json(&value, opt),
        Format::Tree => render_tree(&value, opt),
    })
}

/// Turn the string input into the raw bytes to feed the plist reader. XML source
/// is used verbatim; anything else is treated as a (possibly whitespace-wrapped)
/// Base64 blob and decoded so binary `bplist00` data can be pasted as text.
fn decode_input(input: &str) -> Result<Vec<u8>, String> {
    let trimmed = input.trim_start_matches('\u{feff}').trim();
    if trimmed.is_empty() {
        return Err("input is empty — paste an XML or Base64-encoded property list".into());
    }
    if trimmed.starts_with('<') {
        return Ok(trimmed.as_bytes().to_vec());
    }
    let compact: String = trimmed.chars().filter(|c| !c.is_whitespace()).collect();
    if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(compact.as_bytes()) {
        if !bytes.is_empty() {
            return Ok(bytes);
        }
    }
    // Last resort: hand the raw text to the reader (lets it report a real error).
    Ok(trimmed.as_bytes().to_vec())
}

// ---------- JSON ----------

fn render_json(v: &Value, opt: &Options) -> String {
    let json = to_json(v, opt);
    let indent = " ".repeat(opt.indent.min(8));
    let buf = Vec::new();
    let formatter = serde_json::ser::PrettyFormatter::with_indent(indent.as_bytes());
    let mut ser = serde_json::Serializer::with_formatter(buf, formatter);
    json.serialize(&mut ser).expect("serialize json value");
    String::from_utf8(ser.into_inner()).expect("json is utf8")
}

fn to_json(v: &Value, opt: &Options) -> Json {
    match v {
        Value::Array(a) => Json::Array(a.iter().map(|x| to_json(x, opt)).collect()),
        Value::Dictionary(d) => {
            let mut keys: Vec<&String> = d.keys().collect();
            if opt.sort_keys {
                keys.sort();
            }
            let mut map = serde_json::Map::with_capacity(d.len());
            for k in keys {
                map.insert(k.clone(), to_json(d.get(k).unwrap(), opt));
            }
            Json::Object(map)
        }
        Value::Boolean(b) => Json::Bool(*b),
        Value::Real(r) => serde_json::Number::from_f64(*r)
            .map(Json::Number)
            .unwrap_or(Json::Null),
        Value::Integer(i) => {
            if let Some(s) = i.as_signed() {
                Json::Number(s.into())
            } else if let Some(u) = i.as_unsigned() {
                Json::Number(u.into())
            } else {
                Json::String(i.to_string())
            }
        }
        Value::String(s) => Json::String(s.clone()),
        Value::Date(d) => Json::String(d.to_xml_format()),
        Value::Data(bytes) => Json::String(encode_data(bytes, opt.data_encoding)),
        Value::Uid(u) => {
            // The NSKeyedArchiver / CF representation of a UID.
            let mut m = serde_json::Map::new();
            m.insert("CF$UID".to_string(), Json::Number(u.get().into()));
            Json::Object(m)
        }
        _ => Json::Null,
    }
}

// ---------- tree ----------

fn render_tree(v: &Value, opt: &Options) -> String {
    let mut out = String::new();
    write_tree(v, 0, opt, &mut out);
    out
}

fn write_tree(v: &Value, depth: usize, opt: &Options, out: &mut String) {
    let step = opt.indent.clamp(1, 8);
    match v {
        Value::Dictionary(d) => {
            if d.is_empty() {
                out.push_str("{}");
                return;
            }
            out.push_str("{\n");
            let mut keys: Vec<&String> = d.keys().collect();
            if opt.sort_keys {
                keys.sort();
            }
            let inner = " ".repeat((depth + 1) * step);
            for k in keys {
                out.push_str(&inner);
                out.push('"');
                out.push_str(&escape(k));
                out.push_str("\" => ");
                write_tree(d.get(k).unwrap(), depth + 1, opt, out);
                out.push('\n');
            }
            out.push_str(&" ".repeat(depth * step));
            out.push('}');
        }
        Value::Array(a) => {
            if a.is_empty() {
                out.push_str("[]");
                return;
            }
            out.push_str("[\n");
            let inner = " ".repeat((depth + 1) * step);
            for (i, val) in a.iter().enumerate() {
                out.push_str(&inner);
                out.push_str(&i.to_string());
                out.push_str(" => ");
                write_tree(val, depth + 1, opt, out);
                out.push('\n');
            }
            out.push_str(&" ".repeat(depth * step));
            out.push(']');
        }
        Value::String(s) => {
            out.push('"');
            out.push_str(&escape(s));
            out.push('"');
        }
        Value::Boolean(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Integer(i) => out.push_str(&i.to_string()),
        Value::Real(r) => out.push_str(&r.to_string()),
        Value::Date(d) => out.push_str(&d.to_xml_format()),
        Value::Data(bytes) => {
            out.push('<');
            out.push_str(&bytes.len().to_string());
            out.push_str(" bytes: ");
            out.push_str(&encode_data(bytes, opt.data_encoding));
            out.push('>');
        }
        Value::Uid(u) => {
            out.push_str("Uid(");
            out.push_str(&u.get().to_string());
            out.push(')');
        }
        _ => out.push('?'),
    }
}

// ---------- helpers ----------

fn encode_data(bytes: &[u8], enc: DataEncoding) -> String {
    match enc {
        DataEncoding::Base64 => base64::engine::general_purpose::STANDARD.encode(bytes),
        DataEncoding::Hex => {
            let mut s = String::with_capacity(bytes.len() * 2);
            for b in bytes {
                s.push_str(&format!("{b:02x}"));
            }
            s
        }
    }
}

/// Minimal escaping for a string shown inside double quotes in the tree view.
fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleName</key>
    <string>MyApp</string>
    <key>CFBundleVersion</key>
    <integer>42</integer>
    <key>LSMinimumSystemVersion</key>
    <real>10.5</real>
    <key>Enabled</key>
    <true/>
    <key>Tags</key>
    <array>
        <string>a</string>
        <string>b</string>
    </array>
</dict>
</plist>"#;

    #[test]
    fn xml_to_json_happy() {
        let out = convert(XML, &Options::default()).unwrap();
        let j: Json = serde_json::from_str(&out).unwrap();
        assert_eq!(j["CFBundleName"], "MyApp");
        assert_eq!(j["CFBundleVersion"], 42);
        assert_eq!(j["Enabled"], true);
        assert_eq!(j["Tags"][1], "b");
    }

    #[test]
    fn tree_format_renders_outline() {
        let opt = Options {
            format: Format::Tree,
            ..Options::default()
        };
        let out = convert(XML, &opt).unwrap();
        assert!(out.starts_with("{\n"));
        assert!(out.contains("\"CFBundleName\" => \"MyApp\""));
        assert!(out.contains("0 => \"a\""));
    }

    #[test]
    fn sort_keys_reorders() {
        let opt = Options {
            sort_keys: true,
            ..Options::default()
        };
        let out = convert(XML, &opt).unwrap();
        let cf = out.find("CFBundleName").unwrap();
        let en = out.find("Enabled").unwrap();
        // Alphabetical: CFBundleName/CFBundleVersion before Enabled before LSMinimum/Tags.
        assert!(cf < en);
    }

    #[test]
    fn base64_binary_bplist_roundtrips() {
        // A real binary plist for {"hello": "world"} produced by plutil.
        // bplist00 magic, generated deterministically by the plist crate itself.
        let mut buf = Vec::new();
        let mut dict = plist::Dictionary::new();
        dict.insert("hello".into(), Value::String("world".into()));
        plist::Value::Dictionary(dict)
            .to_writer_binary(&mut buf)
            .unwrap();
        let b64 = base64::engine::general_purpose::STANDARD.encode(&buf);
        let out = convert(&b64, &Options::default()).unwrap();
        let j: Json = serde_json::from_str(&out).unwrap();
        assert_eq!(j["hello"], "world");
    }

    #[test]
    fn data_value_base64_and_hex() {
        let xml = r#"<?xml version="1.0"?><plist version="1.0"><dict><key>blob</key><data>AAEC</data></dict></plist>"#;
        let j: Json =
            serde_json::from_str(&convert(xml, &Options::default()).unwrap()).unwrap();
        assert_eq!(j["blob"], "AAEC"); // base64 of 0x00 0x01 0x02
        let hex_opt = Options {
            data_encoding: DataEncoding::Hex,
            ..Options::default()
        };
        let jh: Json = serde_json::from_str(&convert(xml, &hex_opt).unwrap()).unwrap();
        assert_eq!(jh["blob"], "000102");
    }

    #[test]
    fn custom_indent() {
        let opt = Options {
            indent: 4,
            ..Options::default()
        };
        let out = convert(XML, &opt).unwrap();
        assert!(out.contains("\n    \"CFBundleName\""));
    }

    #[test]
    fn empty_input_errors() {
        assert!(convert("   ", &Options::default()).is_err());
    }

    #[test]
    fn malformed_errors() {
        assert!(convert("<plist><dict><key>x", &Options::default()).is_err());
    }
}
