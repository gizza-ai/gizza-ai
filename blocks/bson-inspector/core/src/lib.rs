//! bson-inspector core — pure compute, shared by the chat skill block and the
//! web page. No wafer/wasm-bindgen deps.
//!
//! Parses a single BSON document (given as base64 or hex bytes) into an ordered,
//! typed representation, then renders it either as an indented type `tree` or as
//! canonical MongoDB Extended JSON v2 (`json`). BSON is an *ordered* binary
//! format — field order is preserved by walking the bytes directly rather than
//! going through any key-sorting map.
//!
//! Coverage: document, array, string, int32, int64, double, boolean, null,
//! ObjectId, UTC datetime, binary+subtype, regex, timestamp, decimal128, minKey,
//! maxKey, undefined, symbol, JavaScript code, JavaScript code w/ scope,
//! dbPointer. Unknown element types and malformed structure produce a clear
//! error instead of a panic.

// ---------------------------------------------------------------------------
// Typed representation
// ---------------------------------------------------------------------------

/// One decoded BSON value.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Double(f64),
    Str(String),
    Document(Vec<Element>),
    Array(Vec<Element>),
    Binary { subtype: u8, bytes: Vec<u8> },
    Undefined,
    ObjectId([u8; 12]),
    Bool(bool),
    DateTime(i64),
    Null,
    Regex { pattern: String, options: String },
    DbPointer { namespace: String, id: [u8; 12] },
    JavaScript(String),
    Symbol(String),
    CodeWithScope { code: String, scope: Vec<Element> },
    Int32(i32),
    Timestamp { seconds: u32, increment: u32 },
    Int64(i64),
    Decimal128([u8; 16]),
    MinKey,
    MaxKey,
}

/// One `name: value` pair, tagged with the byte offset of its type marker so the
/// tree view can optionally show where each element begins.
#[derive(Debug, Clone, PartialEq)]
pub struct Element {
    pub offset: usize,
    pub name: String,
    pub value: Value,
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Parse `input` (base64 or hex bytes of a single BSON document) and render it.
///
/// - `input_format`: `"base64"` (default) or `"hex"`.
/// - `output`: `"tree"` (default) or `"json"` (canonical Extended JSON v2).
/// - `indent`: spaces per nesting level, 0..=8. In `json` mode `0` minifies.
/// - `show_offsets`: prefix each `tree` line with the element's byte offset.
pub fn run(
    input: &str,
    input_format: &str,
    output: &str,
    indent: usize,
    show_offsets: bool,
) -> Result<String, String> {
    let bytes = decode_bytes(input, input_format)?;
    let doc = parse_document_root(&bytes)?;
    let indent = indent.min(8);

    match output {
        "" | "tree" => Ok(render_tree(&doc, indent, show_offsets)),
        "json" => Ok(render_json_document(&doc, indent)),
        other => Err(format!(
            "invalid output {other:?}: expected \"tree\" or \"json\""
        )),
    }
}

// ---------------------------------------------------------------------------
// Byte decoding (hex / base64)
// ---------------------------------------------------------------------------

fn decode_bytes(input: &str, input_format: &str) -> Result<Vec<u8>, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("input is empty: paste the BSON document as base64 or hex".into());
    }
    match input_format {
        "" | "base64" => decode_base64(trimmed),
        "hex" => decode_hex(trimmed),
        other => Err(format!(
            "invalid input_format {other:?}: expected \"base64\" or \"hex\""
        )),
    }
}

fn decode_hex(s: &str) -> Result<Vec<u8>, String> {
    let compact: String = s
        .chars()
        .filter(|c| !c.is_whitespace() && *c != ':' && *c != '-')
        .collect();
    if compact.len() % 2 != 0 {
        return Err("invalid hex: odd number of digits".into());
    }
    let bytes = compact.as_bytes();
    let mut out = Vec::with_capacity(compact.len() / 2);
    for pair in bytes.chunks(2) {
        let hi = hex_val(pair[0])?;
        let lo = hex_val(pair[1])?;
        out.push((hi << 4) | lo);
    }
    Ok(out)
}

fn hex_val(c: u8) -> Result<u8, String> {
    match c {
        b'0'..=b'9' => Ok(c - b'0'),
        b'a'..=b'f' => Ok(c - b'a' + 10),
        b'A'..=b'F' => Ok(c - b'A' + 10),
        _ => Err(format!("invalid hex digit {:?}", c as char)),
    }
}

/// Standard + URL-safe base64, padding optional.
fn decode_base64(s: &str) -> Result<Vec<u8>, String> {
    const INVALID: u8 = 255;
    let val = |c: u8| -> u8 {
        match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => INVALID,
        }
    };
    let mut buf = 0u32;
    let mut bits = 0u32;
    let mut out = Vec::new();
    for &c in s.as_bytes() {
        if c == b'=' || c.is_ascii_whitespace() {
            continue;
        }
        let v = val(c);
        if v == INVALID {
            return Err(format!("invalid base64 character {:?}", c as char));
        }
        buf = (buf << 6) | v as u32;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((buf >> bits) as u8);
        }
    }
    Ok(out)
}

fn encode_base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((bytes.len() + 2) / 3 * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(TABLE[(n >> 18) as usize & 63] as char);
        out.push(TABLE[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            TABLE[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            TABLE[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for &b in bytes {
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 0xf) as usize] as char);
    }
    s
}

// ---------------------------------------------------------------------------
// BSON parser
// ---------------------------------------------------------------------------

struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Reader { buf, pos: 0 }
    }

    fn remaining(&self) -> usize {
        self.buf.len() - self.pos
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], String> {
        if self.remaining() < n {
            return Err(format!(
                "unexpected end of BSON: need {n} more byte(s) at offset {}",
                self.pos
            ));
        }
        let s = &self.buf[self.pos..self.pos + n];
        self.pos += n;
        Ok(s)
    }

    fn u8(&mut self) -> Result<u8, String> {
        Ok(self.take(1)?[0])
    }

    fn i32(&mut self) -> Result<i32, String> {
        let b = self.take(4)?;
        Ok(i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn u32(&mut self) -> Result<u32, String> {
        let b = self.take(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn i64(&mut self) -> Result<i64, String> {
        let b = self.take(8)?;
        Ok(i64::from_le_bytes([
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
        ]))
    }

    fn f64(&mut self) -> Result<f64, String> {
        let b = self.take(8)?;
        Ok(f64::from_le_bytes([
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
        ]))
    }

    /// A NUL-terminated modified UTF-8 key name.
    fn cstring(&mut self) -> Result<String, String> {
        let start = self.pos;
        while self.pos < self.buf.len() && self.buf[self.pos] != 0 {
            self.pos += 1;
        }
        if self.pos >= self.buf.len() {
            return Err(format!(
                "unterminated cstring starting at offset {start}: missing NUL terminator"
            ));
        }
        let raw = &self.buf[start..self.pos];
        self.pos += 1; // consume NUL
        std::str::from_utf8(raw)
            .map(|s| s.to_string())
            .map_err(|_| format!("invalid UTF-8 in field name at offset {start}"))
    }

    /// A length-prefixed BSON string (int32 length incl. trailing NUL).
    fn bson_string(&mut self) -> Result<String, String> {
        let at = self.pos;
        let len = self.i32()?;
        if len < 1 {
            return Err(format!(
                "invalid string length {len} at offset {at}: must be >= 1"
            ));
        }
        let len = len as usize;
        let raw = self.take(len)?;
        if raw[len - 1] != 0 {
            return Err(format!(
                "string at offset {at} is not NUL-terminated"
            ));
        }
        std::str::from_utf8(&raw[..len - 1])
            .map(|s| s.to_string())
            .map_err(|_| format!("invalid UTF-8 in string at offset {at}"))
    }
}

/// Parse the top-level document and require the declared length to span the
/// entire input exactly.
fn parse_document_root(bytes: &[u8]) -> Result<Vec<Element>, String> {
    if bytes.len() < 5 {
        return Err(format!(
            "BSON document too short: {} byte(s), minimum is 5",
            bytes.len()
        ));
    }
    let declared = i32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
    if declared < 5 {
        return Err(format!(
            "invalid document length {declared}: minimum is 5"
        ));
    }
    if declared as usize != bytes.len() {
        return Err(format!(
            "document length {declared} does not match input size {}",
            bytes.len()
        ));
    }
    let mut r = Reader::new(bytes);
    let doc = parse_document(&mut r)?;
    if r.remaining() != 0 {
        return Err(format!(
            "{} trailing byte(s) after document",
            r.remaining()
        ));
    }
    Ok(doc)
}

/// Parse a document starting at the reader's current position: int32 length,
/// elements until a 0x00 terminator, and validate the declared span.
fn parse_document(r: &mut Reader) -> Result<Vec<Element>, String> {
    let start = r.pos;
    let declared = r.i32()?;
    if declared < 5 {
        return Err(format!(
            "invalid document length {declared} at offset {start}: minimum is 5"
        ));
    }
    let end = start + declared as usize;
    if end > r.buf.len() {
        return Err(format!(
            "document at offset {start} declares length {declared} but only {} byte(s) remain",
            r.buf.len() - start
        ));
    }
    let mut elements = Vec::new();
    loop {
        if r.pos >= end {
            return Err(format!(
                "document starting at offset {start} is missing its NUL terminator"
            ));
        }
        let offset = r.pos;
        let tag = r.u8()?;
        if tag == 0x00 {
            break;
        }
        let name = r.cstring()?;
        let value = parse_value(r, tag)?;
        elements.push(Element {
            offset,
            name,
            value,
        });
    }
    if r.pos != end {
        return Err(format!(
            "document starting at offset {start} declared length {declared} but ended at {} (off by {})",
            r.pos,
            r.pos as i64 - end as i64
        ));
    }
    Ok(elements)
}

fn parse_value(r: &mut Reader, tag: u8) -> Result<Value, String> {
    Ok(match tag {
        0x01 => Value::Double(r.f64()?),
        0x02 => Value::Str(r.bson_string()?),
        0x03 => Value::Document(parse_document(r)?),
        0x04 => Value::Array(parse_document(r)?),
        0x05 => {
            let at = r.pos;
            let len = r.i32()?;
            if len < 0 {
                return Err(format!("invalid binary length {len} at offset {at}"));
            }
            let subtype = r.u8()?;
            // Deprecated subtype 0x02 wraps the payload in a second int32 length.
            let payload = if subtype == 0x02 {
                let inner = r.i32()?;
                if inner < 0 || inner as usize + 4 != len as usize {
                    return Err(format!(
                        "invalid old-binary (0x02) length at offset {at}"
                    ));
                }
                r.take(inner as usize)?.to_vec()
            } else {
                r.take(len as usize)?.to_vec()
            };
            Value::Binary {
                subtype,
                bytes: payload,
            }
        }
        0x06 => Value::Undefined,
        0x07 => {
            let mut id = [0u8; 12];
            id.copy_from_slice(r.take(12)?);
            Value::ObjectId(id)
        }
        0x08 => match r.u8()? {
            0 => Value::Bool(false),
            1 => Value::Bool(true),
            other => return Err(format!("invalid boolean byte 0x{other:02x}")),
        },
        0x09 => Value::DateTime(r.i64()?),
        0x0A => Value::Null,
        0x0B => Value::Regex {
            pattern: r.cstring()?,
            options: r.cstring()?,
        },
        0x0C => {
            let namespace = r.bson_string()?;
            let mut id = [0u8; 12];
            id.copy_from_slice(r.take(12)?);
            Value::DbPointer { namespace, id }
        }
        0x0D => Value::JavaScript(r.bson_string()?),
        0x0E => Value::Symbol(r.bson_string()?),
        0x0F => {
            let at = r.pos;
            let total = r.i32()?;
            if total < 5 {
                return Err(format!(
                    "invalid code_w_scope length {total} at offset {at}"
                ));
            }
            let code = r.bson_string()?;
            let scope = parse_document(r)?;
            if r.pos != at + total as usize {
                return Err(format!(
                    "code_w_scope at offset {at} declared length {total} but spans a different size"
                ));
            }
            Value::CodeWithScope { code, scope }
        }
        0x10 => Value::Int32(r.i32()?),
        0x11 => {
            // Timestamp: low u32 = increment, high u32 = seconds.
            let increment = r.u32()?;
            let seconds = r.u32()?;
            Value::Timestamp { seconds, increment }
        }
        0x12 => Value::Int64(r.i64()?),
        0x13 => {
            let mut d = [0u8; 16];
            d.copy_from_slice(r.take(16)?);
            Value::Decimal128(d)
        }
        0xFF => Value::MinKey,
        0x7F => Value::MaxKey,
        other => {
            return Err(format!(
                "unknown BSON element type 0x{other:02x} at offset {}",
                r.pos - 1
            ))
        }
    })
}

// ---------------------------------------------------------------------------
// Decimal128 → string (General Decimal Arithmetic to-scientific-string)
// ---------------------------------------------------------------------------

/// Render a 16-byte little-endian Decimal128 (BID encoding) as its canonical
/// `$numberDecimal` string.
fn decimal128_to_string(le: &[u8; 16]) -> String {
    let mut bits: u128 = 0;
    for (i, &b) in le.iter().enumerate() {
        bits |= (b as u128) << (8 * i);
    }
    let sign = (bits >> 127) & 1 == 1;
    let combo = ((bits >> 122) & 0x1F) as u32; // 5 bits after the sign

    // Infinity / NaN.
    if combo == 0b11110 {
        return if sign { "-Infinity".into() } else { "Infinity".into() };
    }
    if combo == 0b11111 {
        return "NaN".into();
    }

    // Exponent (14 bits) and 113-bit coefficient. For the "11" combination form
    // the implied coefficient exceeds 10^34 and is treated as zero by the spec.
    let (exponent_field, coefficient): (u32, u128) = if combo >= 0b11000 {
        (((bits >> 111) & 0x3FFF) as u32, 0)
    } else {
        (((bits >> 113) & 0x3FFF) as u32, bits & ((1u128 << 113) - 1))
    };
    let exponent: i32 = exponent_field as i32 - 6176;

    let digits = coefficient.to_string(); // no leading zeros (except "0")
    let out = to_scientific_string(&digits, exponent);
    if sign {
        format!("-{out}")
    } else {
        out
    }
}

/// The General-Decimal-Arithmetic `to-scientific-string` conversion of a
/// (unsigned) coefficient digit string and a base-10 exponent.
fn to_scientific_string(digits: &str, exp: i32) -> String {
    let ndigits = digits.len() as i32;
    let adjusted = exp + ndigits - 1;

    if exp <= 0 && adjusted >= -6 {
        // Plain (non-exponential) notation.
        if exp == 0 {
            return digits.to_string();
        }
        let point = ndigits + exp; // position of the decimal point within digits
        if point > 0 {
            let point = point as usize;
            format!("{}.{}", &digits[..point], &digits[point..])
        } else {
            format!("0.{}{}", "0".repeat((-point) as usize), digits)
        }
    } else {
        // Exponential notation.
        let mantissa = if ndigits == 1 {
            digits.to_string()
        } else {
            format!("{}.{}", &digits[..1], &digits[1..])
        };
        let sign = if adjusted >= 0 { "+" } else { "-" };
        format!("{mantissa}E{sign}{}", adjusted.abs())
    }
}

// ---------------------------------------------------------------------------
// Double formatting (round-trip, canonical-ish)
// ---------------------------------------------------------------------------

fn double_to_string(f: f64) -> String {
    if f.is_nan() {
        return "NaN".into();
    }
    if f.is_infinite() {
        return if f < 0.0 { "-Infinity".into() } else { "Infinity".into() };
    }
    let s = format!("{f}");
    // Ensure a decimal marker so a whole-valued double reads as a double.
    if s.contains('.') || s.contains('e') || s.contains('E') {
        s
    } else {
        format!("{s}.0")
    }
}

// ---------------------------------------------------------------------------
// Datetime → ISO-8601 (UTC), for the human-readable tree view
// ---------------------------------------------------------------------------

fn datetime_iso(ms: i64) -> String {
    let (days, mut rem_ms) = (ms.div_euclid(86_400_000), ms.rem_euclid(86_400_000));
    let hours = rem_ms / 3_600_000;
    rem_ms %= 3_600_000;
    let minutes = rem_ms / 60_000;
    rem_ms %= 60_000;
    let seconds = rem_ms / 1000;
    let millis = rem_ms % 1000;
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02}T{hours:02}:{minutes:02}:{seconds:02}.{millis:03}Z"
    )
}

/// Howard Hinnant's days-from-civil inverse: convert a day count since the Unix
/// epoch into a (year, month, day) triple.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32; // [1, 12]
    (if m <= 2 { y + 1 } else { y }, m, d)
}

// ---------------------------------------------------------------------------
// Tree rendering
// ---------------------------------------------------------------------------

fn render_tree(doc: &[Element], indent: usize, show_offsets: bool) -> String {
    let mut out = String::new();
    if doc.is_empty() {
        return "(empty document)".to_string();
    }
    for el in doc {
        render_tree_element(&mut out, el, 0, indent, show_offsets, false);
    }
    // Trim the trailing newline.
    if out.ends_with('\n') {
        out.pop();
    }
    out
}

fn render_tree_element(
    out: &mut String,
    el: &Element,
    depth: usize,
    indent: usize,
    show_offsets: bool,
    in_array: bool,
) {
    let pad = " ".repeat(depth * indent);
    if show_offsets {
        out.push_str(&format!("@{:<6} ", el.offset));
    }
    out.push_str(&pad);
    if in_array {
        out.push_str(&format!("[{}]", el.name));
    } else {
        out.push_str(&el.name);
    }
    out.push_str(": ");
    out.push_str(type_name(&el.value));
    match &el.value {
        Value::Document(children) => {
            out.push('\n');
            for child in children {
                render_tree_element(out, child, depth + 1, indent, show_offsets, false);
            }
        }
        Value::Array(children) => {
            out.push('\n');
            for child in children {
                render_tree_element(out, child, depth + 1, indent, show_offsets, true);
            }
        }
        Value::CodeWithScope { code, scope } => {
            out.push_str(&format!(" {:?}", code));
            out.push('\n');
            for child in scope {
                render_tree_element(out, child, depth + 1, indent, show_offsets, false);
            }
        }
        other => {
            if let Some(summary) = scalar_summary(other) {
                out.push(' ');
                out.push_str(&summary);
            }
            out.push('\n');
        }
    }
}

fn type_name(v: &Value) -> &'static str {
    match v {
        Value::Double(_) => "Double",
        Value::Str(_) => "String",
        Value::Document(_) => "Document",
        Value::Array(_) => "Array",
        Value::Binary { .. } => "Binary",
        Value::Undefined => "Undefined",
        Value::ObjectId(_) => "ObjectId",
        Value::Bool(_) => "Boolean",
        Value::DateTime(_) => "DateTime",
        Value::Null => "Null",
        Value::Regex { .. } => "Regex",
        Value::DbPointer { .. } => "DBPointer",
        Value::JavaScript(_) => "JavaScript",
        Value::Symbol(_) => "Symbol",
        Value::CodeWithScope { .. } => "JavaScriptWithScope",
        Value::Int32(_) => "Int32",
        Value::Timestamp { .. } => "Timestamp",
        Value::Int64(_) => "Int64",
        Value::Decimal128(_) => "Decimal128",
        Value::MinKey => "MinKey",
        Value::MaxKey => "MaxKey",
    }
}

fn scalar_summary(v: &Value) -> Option<String> {
    Some(match v {
        Value::Double(f) => double_to_string(*f),
        Value::Str(s) => format!("{s:?}"),
        Value::Binary { subtype, bytes } => format!(
            "subtype=0x{subtype:02x} {} byte(s) base64={}",
            bytes.len(),
            encode_base64(bytes)
        ),
        Value::ObjectId(id) => hex_lower(id),
        Value::Bool(b) => b.to_string(),
        Value::DateTime(ms) => format!("{ms} ({})", datetime_iso(*ms)),
        Value::Regex { pattern, options } => format!("/{pattern}/{options}"),
        Value::DbPointer { namespace, id } => format!("{namespace} {}", hex_lower(id)),
        Value::JavaScript(code) => format!("{code:?}"),
        Value::Symbol(s) => format!("{s:?}"),
        Value::Int32(n) => n.to_string(),
        Value::Timestamp { seconds, increment } => {
            format!("t={seconds} i={increment}")
        }
        Value::Int64(n) => n.to_string(),
        Value::Decimal128(d) => decimal128_to_string(d),
        Value::Undefined | Value::Null | Value::MinKey | Value::MaxKey => return None,
        // Composite types are handled by the caller.
        Value::Document(_) | Value::Array(_) | Value::CodeWithScope { .. } => return None,
    })
}

// ---------------------------------------------------------------------------
// Canonical MongoDB Extended JSON v2 rendering
// ---------------------------------------------------------------------------

/// A minimal, ORDER-PRESERVING JSON model. BSON documents are ordered, so we
/// never route through a sorting map — object members keep their byte order.
enum J {
    Null,
    Bool(bool),
    /// A pre-rendered numeric literal (emitted verbatim, unquoted).
    Num(String),
    Str(String),
    Arr(Vec<J>),
    Obj(Vec<(String, J)>),
}

fn render_json_document(doc: &[Element], indent: usize) -> String {
    let j = J::Obj(doc.iter().map(|e| (e.name.clone(), value_to_json(&e.value))).collect());
    let mut out = String::new();
    write_j(&mut out, &j, indent, 0);
    out
}

/// Map one BSON value to its canonical Extended JSON v2 shape.
fn value_to_json(v: &Value) -> J {
    match v {
        Value::Double(f) => J::Obj(vec![("$numberDouble".into(), J::Str(double_to_string(*f)))]),
        Value::Str(s) => J::Str(s.clone()),
        Value::Document(d) => {
            J::Obj(d.iter().map(|e| (e.name.clone(), value_to_json(&e.value))).collect())
        }
        Value::Array(a) => J::Arr(a.iter().map(|e| value_to_json(&e.value)).collect()),
        Value::Binary { subtype, bytes } => J::Obj(vec![(
            "$binary".into(),
            J::Obj(vec![
                ("base64".into(), J::Str(encode_base64(bytes))),
                ("subType".into(), J::Str(format!("{subtype:02x}"))),
            ]),
        )]),
        Value::Undefined => J::Obj(vec![("$undefined".into(), J::Bool(true))]),
        Value::ObjectId(id) => J::Obj(vec![("$oid".into(), J::Str(hex_lower(id)))]),
        Value::Bool(b) => J::Bool(*b),
        Value::DateTime(ms) => J::Obj(vec![(
            "$date".into(),
            J::Obj(vec![("$numberLong".into(), J::Str(ms.to_string()))]),
        )]),
        Value::Null => J::Null,
        Value::Regex { pattern, options } => J::Obj(vec![(
            "$regularExpression".into(),
            J::Obj(vec![
                ("pattern".into(), J::Str(pattern.clone())),
                ("options".into(), J::Str(options.clone())),
            ]),
        )]),
        Value::DbPointer { namespace, id } => J::Obj(vec![(
            "$dbPointer".into(),
            J::Obj(vec![
                ("$ref".into(), J::Str(namespace.clone())),
                (
                    "$id".into(),
                    J::Obj(vec![("$oid".into(), J::Str(hex_lower(id)))]),
                ),
            ]),
        )]),
        Value::JavaScript(code) => J::Obj(vec![("$code".into(), J::Str(code.clone()))]),
        Value::Symbol(s) => J::Obj(vec![("$symbol".into(), J::Str(s.clone()))]),
        Value::CodeWithScope { code, scope } => J::Obj(vec![
            ("$code".into(), J::Str(code.clone())),
            (
                "$scope".into(),
                J::Obj(scope.iter().map(|e| (e.name.clone(), value_to_json(&e.value))).collect()),
            ),
        ]),
        Value::Int32(n) => J::Obj(vec![("$numberInt".into(), J::Str(n.to_string()))]),
        Value::Timestamp { seconds, increment } => J::Obj(vec![(
            "$timestamp".into(),
            J::Obj(vec![
                ("t".into(), J::Num(seconds.to_string())),
                ("i".into(), J::Num(increment.to_string())),
            ]),
        )]),
        Value::Int64(n) => J::Obj(vec![("$numberLong".into(), J::Str(n.to_string()))]),
        Value::Decimal128(d) => {
            J::Obj(vec![("$numberDecimal".into(), J::Str(decimal128_to_string(d)))])
        }
        Value::MinKey => J::Obj(vec![("$minKey".into(), J::Num("1".into()))]),
        Value::MaxKey => J::Obj(vec![("$maxKey".into(), J::Num("1".into()))]),
    }
}

fn newline(out: &mut String, indent: usize, depth: usize) {
    if indent > 0 {
        out.push('\n');
        out.push_str(&" ".repeat(indent * depth));
    }
}

fn write_j(out: &mut String, j: &J, indent: usize, depth: usize) {
    match j {
        J::Null => out.push_str("null"),
        J::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        J::Num(s) => out.push_str(s),
        J::Str(s) => write_json_string(out, s),
        J::Arr(items) => {
            if items.is_empty() {
                out.push_str("[]");
                return;
            }
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                newline(out, indent, depth + 1);
                write_j(out, item, indent, depth + 1);
            }
            newline(out, indent, depth);
            out.push(']');
        }
        J::Obj(members) => {
            if members.is_empty() {
                out.push_str("{}");
                return;
            }
            out.push('{');
            for (i, (k, val)) in members.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                newline(out, indent, depth + 1);
                write_json_string(out, k);
                out.push(':');
                if indent > 0 {
                    out.push(' ');
                }
                write_j(out, val, indent, depth + 1);
            }
            newline(out, indent, depth);
            out.push('}');
        }
    }
}

fn write_json_string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

// ===========================================================================
// Tests
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Build the classic `{"hello": "world"}` BSON document as hex.
    /// 16 00 00 00  02 68 65 6c 6c 6f 00  06 00 00 00 77 6f 72 6c 64 00  00
    const HELLO_WORLD_HEX: &str =
        "16000000026865 6c6c6f000600 00007 76f726c6400 00";

    fn hello_world() -> Vec<u8> {
        decode_hex("1600000002 68656c6c6f00 06000000 776f726c6400 00").unwrap()
    }

    #[test]
    fn tree_hello_world() {
        let bytes = hello_world();
        let hex = hex_lower(&bytes);
        let out = run(&hex, "hex", "tree", 2, false).unwrap();
        assert_eq!(out, "hello: String \"world\"");
    }

    #[test]
    fn json_hello_world() {
        let bytes = hello_world();
        let hex = hex_lower(&bytes);
        let out = run(&hex, "hex", "json", 0, false).unwrap();
        assert_eq!(out, r#"{"hello":"world"}"#);
    }

    #[test]
    fn json_hello_world_pretty() {
        let bytes = hello_world();
        let hex = hex_lower(&bytes);
        let out = run(&hex, "hex", "json", 2, false).unwrap();
        assert_eq!(out, "{\n  \"hello\": \"world\"\n}");
    }

    #[test]
    fn base64_input_default_format() {
        let bytes = hello_world();
        let b64 = encode_base64(&bytes);
        // input_format "" defaults to base64
        let out = run(&b64, "", "json", 0, false).unwrap();
        assert_eq!(out, r#"{"hello":"world"}"#);
        assert!(HELLO_WORLD_HEX.len() > 0); // keep the doc constant referenced
    }

    #[test]
    fn scalar_types_json() {
        // { "i": int32(7), "l": int64(9), "b": true, "n": null }
        // doc:
        //   10 'i' 00 07000000
        //   12 'l' 00 0900000000000000
        //   08 'b' 00 01
        //   0A 'n' 00
        let hex = "1e000000106900070000 00126c000900000000000000086200010a6e0000";
        let out = run(hex, "hex", "json", 0, false).unwrap();
        assert_eq!(
            out,
            r#"{"i":{"$numberInt":"7"},"l":{"$numberLong":"9"},"b":true,"n":null}"#
        );
    }

    #[test]
    fn objectid_and_double_tree() {
        // { "_id": ObjectId(0102..0c), "pi": 3.14 (double) }
        // 07 '_id' 00 0102030405060708090a0b0c
        // 01 'pi' 00 <f64 le of 3.14>
        let mut bytes = Vec::new();
        let mut body = Vec::new();
        body.push(0x07);
        body.extend_from_slice(b"_id\0");
        body.extend_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
        body.push(0x01);
        body.extend_from_slice(b"pi\0");
        body.extend_from_slice(&3.14f64.to_le_bytes());
        body.push(0x00);
        let len = (body.len() + 4) as i32;
        bytes.extend_from_slice(&len.to_le_bytes());
        bytes.extend_from_slice(&body);

        let hex = hex_lower(&bytes);
        let out = run(&hex, "hex", "tree", 2, false).unwrap();
        assert_eq!(
            out,
            "_id: ObjectId 0102030405060708090a0b0c\npi: Double 3.14"
        );
    }

    #[test]
    fn nested_document_and_array_json() {
        // { "a": [ int32(1), int32(2) ], "d": { "x": true } }
        let mut inner_arr = Vec::new();
        inner_arr.push(0x10);
        inner_arr.extend_from_slice(b"0\0");
        inner_arr.extend_from_slice(&1i32.to_le_bytes());
        inner_arr.push(0x10);
        inner_arr.extend_from_slice(b"1\0");
        inner_arr.extend_from_slice(&2i32.to_le_bytes());
        inner_arr.push(0x00);
        let arr_doc = with_len(inner_arr);

        let mut inner_doc = Vec::new();
        inner_doc.push(0x08);
        inner_doc.extend_from_slice(b"x\0");
        inner_doc.push(0x01);
        inner_doc.push(0x00);
        let sub_doc = with_len(inner_doc);

        let mut body = Vec::new();
        body.push(0x04);
        body.extend_from_slice(b"a\0");
        body.extend_from_slice(&arr_doc);
        body.push(0x03);
        body.extend_from_slice(b"d\0");
        body.extend_from_slice(&sub_doc);
        body.push(0x00);
        let doc = with_len(body);

        let hex = hex_lower(&doc);
        let out = run(&hex, "hex", "json", 0, false).unwrap();
        assert_eq!(
            out,
            r#"{"a":[{"$numberInt":"1"},{"$numberInt":"2"}],"d":{"x":true}}"#
        );

        let tree = run(&hex, "hex", "tree", 2, true).unwrap();
        assert!(tree.contains("a: Array"), "tree was: {tree}");
        assert!(tree.contains("[0]: Int32 1"), "tree was: {tree}");
        assert!(tree.starts_with("@4"), "offsets missing: {tree}");
    }

    fn with_len(mut body: Vec<u8>) -> Vec<u8> {
        let len = (body.len() + 4) as i32;
        let mut out = len.to_le_bytes().to_vec();
        out.append(&mut body);
        out
    }

    #[test]
    fn decimal128_values() {
        // Canonical test vectors from the BSON corpus.
        // 1  ->  1E1088... let's build directly from known 16-byte encodings.
        // "0" : 0x30 40 followed by zeros -> exponent 0, coeff 0
        let mut zero = [0u8; 16];
        zero[15] = 0x30;
        zero[14] = 0x40;
        assert_eq!(decimal128_to_string(&zero), "0");

        // 0.1 : coefficient 1, exponent -1 -> bias 6175 = 0x181F.
        // exponent field occupies bits 113..126; encode via arithmetic.
        let v = build_decimal(false, 1, -1);
        assert_eq!(decimal128_to_string(&v), "0.1");

        let v = build_decimal(false, 125, -2);
        assert_eq!(decimal128_to_string(&v), "1.25");

        let v = build_decimal(true, 125, -2);
        assert_eq!(decimal128_to_string(&v), "-1.25");

        let v = build_decimal(false, 10, -1);
        assert_eq!(decimal128_to_string(&v), "1.0");

        let v = build_decimal(false, 1, 6);
        assert_eq!(decimal128_to_string(&v), "1E+6");
    }

    /// Encode a normal-form Decimal128 (coefficient < 2^113) to 16 LE bytes.
    fn build_decimal(sign: bool, coefficient: u128, exponent: i32) -> [u8; 16] {
        let exp_field = (exponent + 6176) as u128 & 0x3FFF;
        let mut bits: u128 = coefficient & ((1u128 << 113) - 1);
        bits |= exp_field << 113;
        if sign {
            bits |= 1u128 << 127;
        }
        let mut out = [0u8; 16];
        for (i, b) in out.iter_mut().enumerate() {
            *b = (bits >> (8 * i)) as u8;
        }
        out
    }

    #[test]
    fn err_bad_hex() {
        let err = run("zz", "hex", "tree", 2, false).unwrap_err();
        assert!(err.contains("invalid hex digit"), "{err}");
    }

    #[test]
    fn err_odd_hex() {
        let err = run("161", "hex", "tree", 2, false).unwrap_err();
        assert!(err.contains("odd number"), "{err}");
    }

    #[test]
    fn err_bad_base64() {
        let err = run("@@@@", "base64", "tree", 2, false).unwrap_err();
        assert!(err.contains("invalid base64"), "{err}");
    }

    #[test]
    fn err_length_mismatch() {
        // declared length 99, but only a few bytes
        let err = run("63000000 00", "hex", "tree", 2, false).unwrap_err();
        assert!(err.contains("does not match"), "{err}");
    }

    #[test]
    fn err_too_short() {
        let err = run("0100", "hex", "tree", 2, false).unwrap_err();
        assert!(err.contains("too short"), "{err}");
    }

    #[test]
    fn err_missing_terminator() {
        // length 6, one type byte, no name/terminator -> unterminated cstring or missing NUL
        // 06 00 00 00  10  (int32 tag) then nothing
        let err = run("0600000010", "hex", "tree", 2, false).unwrap_err();
        assert!(!err.is_empty());
    }

    #[test]
    fn err_unknown_type() {
        // length 8, tag 0x99, name 'a', terminator... 0x99 is unknown
        // 08 00 00 00  99 61 00 00
        let err = run("0800000099610000", "hex", "tree", 2, false).unwrap_err();
        assert!(err.contains("unknown BSON element type"), "{err}");
    }

    #[test]
    fn err_bad_output() {
        let bytes = hello_world();
        let hex = hex_lower(&bytes);
        let err = run(&hex, "hex", "xml", 2, false).unwrap_err();
        assert!(err.contains("invalid output"), "{err}");
    }

    #[test]
    fn err_bad_input_format() {
        let err = run("00", "octal", "tree", 2, false).unwrap_err();
        assert!(err.contains("invalid input_format"), "{err}");
    }

    #[test]
    fn datetime_render() {
        // { "t": UTC datetime 0 } => 1970-01-01
        let mut body = Vec::new();
        body.push(0x09);
        body.extend_from_slice(b"t\0");
        body.extend_from_slice(&0i64.to_le_bytes());
        body.push(0x00);
        let doc = with_len(body);
        let hex = hex_lower(&doc);
        let tree = run(&hex, "hex", "tree", 2, false).unwrap();
        assert_eq!(tree, "t: DateTime 0 (1970-01-01T00:00:00.000Z)");
        let json = run(&hex, "hex", "json", 0, false).unwrap();
        assert_eq!(json, r#"{"t":{"$date":{"$numberLong":"0"}}}"#);
    }
}
