//! npy-array-decoder core — pure compute, shared by the chat skill block and the
//! web page. No wafer/wasm-bindgen deps, no third-party crates: the whole `.npy`
//! reader (magic, versioned header, Python-literal dict, dtype descriptor, and
//! every element decoder) is std-only Rust, so it compiles and instantiates for
//! both `wasm32-wasip1` (chat/CLI) and `wasm32-unknown-unknown` (page).
//!
//! Format handled: NumPy's `.npy` v1.0, v2.0 and v3.0.
//!
//! ```text
//! \x93NUMPY  major minor  <header_len>  <header dict, space padded, \n terminated>  <raw data>
//!    6 B       1 B  1 B    2 B (v1)                                                  C or F order
//!                          4 B (v2/v3)
//! ```
//!
//! The header dict always carries three keys — `descr` (the dtype string),
//! `fortran_order` (bool) and `shape` (a tuple). Data follows immediately, with
//! no padding between elements.
//!
//! Supported dtypes: `b1`/`?` (bool), `i1`–`i8`, `u1`–`u8`, `f2`/`f4`/`f8`,
//! `c8`/`c16`, `S<n>` (fixed-width bytes) and `U<n>` (fixed-width UCS-4 text),
//! in little-endian, big-endian or byte-order-agnostic form. Structured/record
//! dtypes, object arrays, `V`oid, `datetime64` and `timedelta64` are rejected
//! with an explanatory error rather than guessed at.

// ---------------------------------------------------------------------------
// Limits
// ---------------------------------------------------------------------------

/// Largest `.npy` payload accepted after base64/hex decoding. Every element is
/// materialised in memory before rendering, and the rendered text is several
/// times larger again, so stay well inside the wasm sandbox.
pub const MAX_BYTES: usize = 8 * 1024 * 1024; // 8 MiB

/// Largest value of `limit` (values rendered into the output).
pub const MAX_LIMIT: usize = 100_000;

/// `limit` used when the caller passes 0 / leaves the field blank.
pub const DEFAULT_LIMIT: usize = 1_000;

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Decode a `.npy` file and render it.
///
/// - `input`: the file bytes as base64 (standard or URL-safe, padding optional,
///   `data:` URI prefix tolerated) or hex.
/// - `input_format`: `"auto"` (default), `"base64"` or `"hex"`.
/// - `output`: `"summary"` (default), `"json"`, `"csv"` or `"header"`.
/// - `limit`: maximum number of values rendered (1..=[`MAX_LIMIT`]; 0 → [`DEFAULT_LIMIT`]).
/// - `delimiter`: CSV field separator — a single character, or the word `"tab"`.
pub fn run(
    input: &str,
    input_format: &str,
    output: &str,
    limit: usize,
    delimiter: &str,
) -> Result<String, String> {
    let bytes = decode_bytes(input, input_format)?;
    let file = parse_npy(&bytes)?;

    let limit = match limit {
        0 => DEFAULT_LIMIT,
        n if n > MAX_LIMIT => {
            return Err(format!(
                "limit {n} is too large: expected 1-{MAX_LIMIT} values (use a smaller limit, or 0 for the default {DEFAULT_LIMIT})"
            ))
        }
        n => n,
    };

    match output {
        "" | "summary" => render_summary(&file, limit),
        "header" => Ok(render_header_json(&file)),
        "json" => render_json(&file, limit),
        "csv" => render_csv(&file, limit, delimiter),
        other => Err(format!(
            "invalid output {other:?}: expected \"summary\", \"json\", \"csv\" or \"header\""
        )),
    }
}

// ---------------------------------------------------------------------------
// Input decoding (base64 / hex / auto)
// ---------------------------------------------------------------------------

fn decode_bytes(input: &str, input_format: &str) -> Result<Vec<u8>, String> {
    let trimmed = strip_data_uri(input.trim());
    if trimmed.is_empty() {
        return Err(
            "input is empty: paste the contents of a .npy file as base64 (or hex)".to_string(),
        );
    }
    let bytes = match input_format {
        "" | "auto" => {
            if looks_like_hex(trimmed) {
                decode_hex(trimmed)?
            } else {
                decode_base64(trimmed)?
            }
        }
        "base64" => decode_base64(trimmed)?,
        "hex" => decode_hex(trimmed)?,
        other => {
            return Err(format!(
                "invalid input_format {other:?}: expected \"auto\", \"base64\" or \"hex\""
            ))
        }
    };
    if bytes.len() > MAX_BYTES {
        return Err(format!(
            "input is too large: {} bytes decoded, the limit is {} bytes ({} MiB)",
            bytes.len(),
            MAX_BYTES,
            MAX_BYTES / (1024 * 1024)
        ));
    }
    Ok(bytes)
}

/// Accept a pasted `data:application/octet-stream;base64,<payload>` URI.
fn strip_data_uri(s: &str) -> &str {
    if let Some(rest) = s.strip_prefix("data:") {
        if let Some(idx) = rest.find(',') {
            return rest[idx + 1..].trim();
        }
    }
    s
}

/// `auto` detection: a `.npy` file always starts with byte `0x93`, so its hex
/// form begins `93` and its base64 form begins `k05VTVBZ` — the two never look
/// alike. Treat an even-length, all-hex-digit string as hex.
fn looks_like_hex(s: &str) -> bool {
    let mut digits = 0usize;
    for c in s.chars() {
        if c.is_whitespace() || c == ':' || c == '-' {
            continue;
        }
        if !c.is_ascii_hexdigit() {
            return false;
        }
        digits += 1;
    }
    digits > 0 && digits % 2 == 0
}

fn decode_hex(s: &str) -> Result<Vec<u8>, String> {
    let compact: Vec<u8> = s
        .bytes()
        .filter(|c| !c.is_ascii_whitespace() && *c != b':' && *c != b'-')
        .collect();
    if compact.len() % 2 != 0 {
        return Err(format!(
            "invalid hex input: {} digits is an odd count, every byte needs two",
            compact.len()
        ));
    }
    let mut out = Vec::with_capacity(compact.len() / 2);
    for pair in compact.chunks(2) {
        out.push((hex_val(pair[0])? << 4) | hex_val(pair[1])?);
    }
    Ok(out)
}

fn hex_val(c: u8) -> Result<u8, String> {
    match c {
        b'0'..=b'9' => Ok(c - b'0'),
        b'a'..=b'f' => Ok(c - b'a' + 10),
        b'A'..=b'F' => Ok(c - b'A' + 10),
        _ => Err(format!("invalid hex digit {:?} in the input", c as char)),
    }
}

/// Standard + URL-safe base64, padding optional, whitespace ignored.
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
            return Err(format!(
                "invalid base64 character {:?} in the input (expected A-Z a-z 0-9 + / = or the URL-safe - _)",
                c as char
            ));
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

// ---------------------------------------------------------------------------
// dtype descriptor
// ---------------------------------------------------------------------------

/// The element kinds this decoder can materialise.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// One byte, 0 or 1.
    Bool,
    /// Signed integer of `item_size` bytes.
    Int,
    /// Unsigned integer of `item_size` bytes.
    UInt,
    /// IEEE-754 binary16/32/64.
    Float,
    /// Two floats (real, imaginary) of half the item size each.
    Complex,
    /// `S<n>` — fixed-width byte string, NUL padded.
    Bytes,
    /// `U<n>` — fixed-width UCS-4 text, NUL padded.
    Unicode,
}

/// A parsed `descr` string, e.g. `<f8` → `Float`, 8 bytes, little-endian.
#[derive(Debug, Clone)]
pub struct DType {
    /// The raw `descr` string from the header.
    pub descr: String,
    pub kind: Kind,
    /// Bytes per element.
    pub item_size: usize,
    /// True when multi-byte values are stored most-significant byte first.
    pub big_endian: bool,
    /// Human name, e.g. `float64`, `uint8`, `bytes10`, `str5`.
    pub name: String,
}

impl DType {
    /// `"little-endian"` / `"big-endian"` / `"not applicable"` (single-byte).
    pub fn byte_order(&self) -> &'static str {
        if self.item_size == 1 || self.kind == Kind::Bytes {
            "not applicable"
        } else if self.big_endian {
            "big-endian"
        } else {
            "little-endian"
        }
    }
}

fn parse_dtype(descr: &str) -> Result<DType, String> {
    let mut chars = descr.chars();
    let first = chars
        .next()
        .ok_or_else(|| "unsupported dtype: the header's descr is an empty string".to_string())?;

    let (big_endian, type_char) = match first {
        '<' | '=' | '|' => (
            false,
            chars.next().ok_or_else(|| {
                format!("unsupported dtype {descr:?}: a byte-order character with no type character")
            })?,
        ),
        '>' => (
            true,
            chars.next().ok_or_else(|| {
                format!("unsupported dtype {descr:?}: a byte-order character with no type character")
            })?,
        ),
        c => (false, c),
    };
    let digits: String = chars.collect();
    let width: Option<usize> = if digits.is_empty() {
        None
    } else {
        digits.parse().ok()
    };

    let unsupported = |why: &str| -> String { format!("unsupported dtype {descr:?}: {why}") };

    let (kind, item_size, name) = match type_char {
        'b' | '?' => {
            // NumPy writes bool as `|b1`; `b` with any other width is not valid.
            match width.unwrap_or(1) {
                1 => (Kind::Bool, 1usize, "bool".to_string()),
                w => return Err(unsupported(&format!("bool must be 1 byte wide, got {w}"))),
            }
        }
        'i' => match width.unwrap_or(4) {
            w @ (1 | 2 | 4 | 8) => (Kind::Int, w, format!("int{}", w * 8)),
            w => {
                return Err(unsupported(&format!(
                    "signed integers must be 1, 2, 4 or 8 bytes wide, got {w}"
                )))
            }
        },
        'u' => match width.unwrap_or(4) {
            w @ (1 | 2 | 4 | 8) => (Kind::UInt, w, format!("uint{}", w * 8)),
            w => {
                return Err(unsupported(&format!(
                    "unsigned integers must be 1, 2, 4 or 8 bytes wide, got {w}"
                )))
            }
        },
        'f' => match width.unwrap_or(8) {
            w @ (2 | 4 | 8) => (Kind::Float, w, format!("float{}", w * 8)),
            w @ (12 | 16) => {
                return Err(unsupported(&format!(
                    "{w}-byte extended-precision floats (numpy longdouble) are platform specific and are not decoded"
                )))
            }
            w => {
                return Err(unsupported(&format!(
                    "floats must be 2, 4 or 8 bytes wide, got {w}"
                )))
            }
        },
        'c' => match width.unwrap_or(16) {
            w @ (8 | 16) => (Kind::Complex, w, format!("complex{}", w * 8)),
            w => {
                return Err(unsupported(&format!(
                    "complex values must be 8 or 16 bytes wide, got {w}"
                )))
            }
        },
        'S' | 'a' => {
            let w = width.unwrap_or(0);
            (Kind::Bytes, w.max(1), format!("bytes{}", w.max(1)))
        }
        'U' => {
            let chars_per_item = width.unwrap_or(0).max(1);
            (
                Kind::Unicode,
                chars_per_item * 4,
                format!("str{chars_per_item}"),
            )
        }
        'O' => {
            return Err(unsupported(
                "object arrays hold pickled Python objects, which cannot be decoded safely without executing code (re-save with allow_pickle=False)",
            ))
        }
        'V' => {
            return Err(unsupported(
                "raw void / structured records have no numeric interpretation",
            ))
        }
        'M' => {
            return Err(unsupported(
                "datetime64 arrays are not supported yet (their unit is encoded in the descr, e.g. <M8[ns])",
            ))
        }
        'm' => {
            return Err(unsupported(
                "timedelta64 arrays are not supported yet (their unit is encoded in the descr, e.g. <m8[s])",
            ))
        }
        c => {
            return Err(unsupported(&format!(
                "unknown type character {c:?} (expected one of b i u f c S U)"
            )))
        }
    };

    Ok(DType {
        descr: descr.to_string(),
        kind,
        item_size,
        big_endian,
        name,
    })
}

// ---------------------------------------------------------------------------
// Python-literal parsing (the header dict)
// ---------------------------------------------------------------------------

/// The subset of Python literals a `.npy` header can contain.
#[derive(Debug, Clone, PartialEq)]
enum PyVal {
    Str(String),
    Bool(bool),
    Int(i64),
    /// A tuple `(...)` or a list `[...]`.
    Seq(Vec<PyVal>),
    None,
}

struct PyParser<'a> {
    b: &'a [u8],
    i: usize,
}

impl<'a> PyParser<'a> {
    fn new(s: &'a str) -> Self {
        PyParser {
            b: s.as_bytes(),
            i: 0,
        }
    }

    fn skip_ws(&mut self) {
        while self.i < self.b.len() && (self.b[self.i] as char).is_whitespace() {
            self.i += 1;
        }
    }

    fn peek(&mut self) -> Option<u8> {
        self.skip_ws();
        self.b.get(self.i).copied()
    }

    /// Parse the top-level `{'descr': …, 'fortran_order': …, 'shape': …}` dict.
    fn parse_dict(&mut self) -> Result<Vec<(String, PyVal)>, String> {
        if self.peek() != Some(b'{') {
            return Err("malformed .npy header: expected the header to be a Python dict starting with '{'".to_string());
        }
        self.i += 1;
        let mut out = Vec::new();
        loop {
            match self.peek() {
                Some(b'}') => {
                    self.i += 1;
                    return Ok(out);
                }
                Some(b',') => {
                    self.i += 1;
                }
                Some(_) => {
                    let key = match self.parse_value()? {
                        PyVal::Str(s) => s,
                        other => {
                            return Err(format!(
                                "malformed .npy header: expected a quoted key, found {other:?}"
                            ))
                        }
                    };
                    if self.peek() != Some(b':') {
                        return Err(format!(
                            "malformed .npy header: expected ':' after the key {key:?}"
                        ));
                    }
                    self.i += 1;
                    let value = self.parse_value()?;
                    out.push((key, value));
                }
                None => {
                    return Err(
                        "malformed .npy header: the header dict is missing its closing '}'"
                            .to_string(),
                    )
                }
            }
        }
    }

    fn parse_value(&mut self) -> Result<PyVal, String> {
        match self.peek() {
            Some(q @ (b'\'' | b'"')) => {
                self.i += 1;
                let mut s = String::new();
                loop {
                    let c = *self.b.get(self.i).ok_or_else(|| {
                        "malformed .npy header: an unterminated string literal".to_string()
                    })?;
                    self.i += 1;
                    if c == q {
                        break;
                    }
                    if c == b'\\' {
                        let n = *self.b.get(self.i).ok_or_else(|| {
                            "malformed .npy header: a trailing backslash in a string literal"
                                .to_string()
                        })?;
                        self.i += 1;
                        s.push(n as char);
                    } else {
                        s.push(c as char);
                    }
                }
                Ok(PyVal::Str(s))
            }
            Some(b'(') | Some(b'[') => {
                let close = if self.b[self.i] == b'(' { b')' } else { b']' };
                self.i += 1;
                let mut items = Vec::new();
                loop {
                    match self.peek() {
                        Some(c) if c == close => {
                            self.i += 1;
                            return Ok(PyVal::Seq(items));
                        }
                        Some(b',') => self.i += 1,
                        Some(_) => items.push(self.parse_value()?),
                        None => {
                            return Err(
                                "malformed .npy header: an unterminated tuple or list".to_string()
                            )
                        }
                    }
                }
            }
            Some(c) if c.is_ascii_digit() || c == b'-' || c == b'+' => {
                let start = self.i;
                self.i += 1;
                while self
                    .b
                    .get(self.i)
                    .is_some_and(|c| c.is_ascii_digit() || *c == b'L')
                {
                    self.i += 1;
                }
                let raw: String = String::from_utf8_lossy(&self.b[start..self.i])
                    .trim_end_matches('L')
                    .to_string();
                raw.parse::<i64>()
                    .map(PyVal::Int)
                    .map_err(|_| format!("malformed .npy header: {raw:?} is not an integer"))
            }
            Some(_) => {
                let start = self.i;
                while self
                    .b
                    .get(self.i)
                    .is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_')
                {
                    self.i += 1;
                }
                if self.i == start {
                    return Err(format!(
                        "malformed .npy header: unexpected character {:?}",
                        self.b[self.i] as char
                    ));
                }
                let word = String::from_utf8_lossy(&self.b[start..self.i]).to_string();
                match word.as_str() {
                    "True" => Ok(PyVal::Bool(true)),
                    "False" => Ok(PyVal::Bool(false)),
                    "None" => Ok(PyVal::None),
                    other => Err(format!(
                        "malformed .npy header: unexpected token {other:?} (expected True, False or None)"
                    )),
                }
            }
            None => Err("malformed .npy header: unexpected end of the header dict".to_string()),
        }
    }
}

// ---------------------------------------------------------------------------
// The parsed file
// ---------------------------------------------------------------------------

/// One decoded element.
#[derive(Debug, Clone, PartialEq)]
pub enum Val {
    Bool(bool),
    Int(i64),
    UInt(u64),
    Float(f64),
    Complex(f64, f64),
    Text(String),
}

/// A fully decoded `.npy` file: header metadata plus every element, already in
/// C (row-major) logical order regardless of how it was stored.
#[derive(Debug, Clone)]
pub struct NpyFile {
    pub version: (u8, u8),
    pub dtype: DType,
    pub shape: Vec<usize>,
    pub fortran_order: bool,
    /// Length of the header dict text, as declared by the length field.
    pub header_len: usize,
    /// Byte offset at which the raw data begins.
    pub data_offset: usize,
    /// Bytes of raw data actually needed by `shape` × `item_size`.
    pub data_bytes: usize,
    /// Bytes present after the array data (normally 0).
    pub trailing_bytes: usize,
    /// Every element, in C order.
    pub values: Vec<Val>,
}

impl NpyFile {
    /// Number of elements, i.e. the product of `shape` (1 for a 0-d scalar).
    pub fn total_elements(&self) -> usize {
        self.values.len()
    }

    /// `(2, 3)` — Python's tuple form, including the 1-tuple trailing comma.
    pub fn shape_str(&self) -> String {
        match self.shape.len() {
            0 => "()".to_string(),
            1 => format!("({},)", self.shape[0]),
            _ => {
                let parts: Vec<String> = self.shape.iter().map(|n| n.to_string()).collect();
                format!("({})", parts.join(", "))
            }
        }
    }
}

/// Parse a complete `.npy` file.
pub fn parse_npy(bytes: &[u8]) -> Result<NpyFile, String> {
    const MAGIC: &[u8; 6] = b"\x93NUMPY";

    if bytes.len() < 10 {
        return Err(format!(
            "not a .npy file: only {} bytes were given, a .npy file is at least 10 bytes (6-byte magic + version + header length)",
            bytes.len()
        ));
    }
    if &bytes[..6] != MAGIC {
        return Err(format!(
            "not a .npy file: expected the magic bytes \\x93NUMPY, found {} — a .npy file always starts with byte 0x93 then \"NUMPY\" (a .npz file is a ZIP archive of .npy members and must be unzipped first)",
            hex_preview(&bytes[..6.min(bytes.len())])
        ));
    }

    let (major, minor) = (bytes[6], bytes[7]);
    if minor != 0 || !(1..=3).contains(&major) {
        return Err(format!(
            "unsupported .npy format version {major}.{minor}: this decoder reads versions 1.0, 2.0 and 3.0"
        ));
    }

    // v1.0 stores the header length in 2 bytes, v2.0/v3.0 in 4.
    let (len_bytes, header_start) = if major == 1 { (2usize, 10usize) } else { (4usize, 12usize) };
    if bytes.len() < header_start {
        return Err(format!(
            "truncated .npy file: version {major}.{minor} needs a {len_bytes}-byte header-length field at offset 8, but the file ends after {} bytes",
            bytes.len()
        ));
    }
    let header_len = match len_bytes {
        2 => u16::from_le_bytes([bytes[8], bytes[9]]) as usize,
        _ => u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize,
    };

    let data_offset = header_start + header_len;
    if bytes.len() < data_offset {
        return Err(format!(
            "truncated .npy file: the header declares {header_len} bytes of header text starting at offset {header_start}, but the file is only {} bytes long",
            bytes.len()
        ));
    }
    let header_text = String::from_utf8_lossy(&bytes[header_start..data_offset]).to_string();

    let dict = PyParser::new(&header_text).parse_dict()?;
    let get = |k: &str| dict.iter().find(|(key, _)| key == k).map(|(_, v)| v);

    let descr = match get("descr") {
        Some(PyVal::Str(s)) => s.clone(),
        Some(PyVal::Seq(_)) => {
            return Err(
                "unsupported dtype: this is a structured (record) array — its descr is a list of named fields, which has no single dtype or numeric value grid. Save the individual fields as separate arrays first.".to_string(),
            )
        }
        Some(other) => {
            return Err(format!(
                "malformed .npy header: 'descr' should be a dtype string, found {other:?}"
            ))
        }
        None => {
            return Err(
                "malformed .npy header: the required key 'descr' (the dtype) is missing".to_string(),
            )
        }
    };
    let dtype = parse_dtype(&descr)?;

    let fortran_order = match get("fortran_order") {
        Some(PyVal::Bool(b)) => *b,
        Some(other) => {
            return Err(format!(
                "malformed .npy header: 'fortran_order' should be True or False, found {other:?}"
            ))
        }
        None => {
            return Err(
                "malformed .npy header: the required key 'fortran_order' is missing".to_string(),
            )
        }
    };

    let shape: Vec<usize> = match get("shape") {
        Some(PyVal::Seq(items)) => {
            let mut dims = Vec::with_capacity(items.len());
            for it in items {
                match it {
                    PyVal::Int(n) if *n >= 0 => dims.push(*n as usize),
                    other => {
                        return Err(format!(
                            "malformed .npy header: 'shape' should be a tuple of non-negative integers, found {other:?}"
                        ))
                    }
                }
            }
            dims
        }
        Some(other) => {
            return Err(format!(
                "malformed .npy header: 'shape' should be a tuple, found {other:?}"
            ))
        }
        None => {
            return Err("malformed .npy header: the required key 'shape' is missing".to_string())
        }
    };

    let mut total: usize = 1;
    for d in &shape {
        total = total.checked_mul(*d).ok_or_else(|| {
            format!(
                "unreadable .npy header: the shape {:?} overflows the element count",
                shape
            )
        })?;
    }
    let data_bytes = total.checked_mul(dtype.item_size).ok_or_else(|| {
        format!(
            "unreadable .npy header: {total} elements of {} bytes overflows the data size",
            dtype.item_size
        )
    })?;

    let available = bytes.len() - data_offset;
    if available < data_bytes {
        return Err(format!(
            "truncated .npy data: the header declares shape {} of {} ({} elements x {} bytes = {} bytes), but only {} bytes follow the header",
            shape_tuple(&shape),
            dtype.name,
            total,
            dtype.item_size,
            data_bytes,
            available
        ));
    }

    let raw = &bytes[data_offset..data_offset + data_bytes];
    let mut values = Vec::with_capacity(total);
    for i in 0..total {
        values.push(decode_element(
            &raw[i * dtype.item_size..(i + 1) * dtype.item_size],
            &dtype,
        )?);
    }
    if fortran_order && shape.len() > 1 {
        values = to_c_order(&values, &shape);
    }

    Ok(NpyFile {
        version: (major, minor),
        dtype,
        shape,
        fortran_order,
        header_len,
        data_offset,
        data_bytes,
        trailing_bytes: available - data_bytes,
        values,
    })
}

fn shape_tuple(shape: &[usize]) -> String {
    match shape.len() {
        0 => "()".to_string(),
        1 => format!("({},)", shape[0]),
        _ => format!(
            "({})",
            shape
                .iter()
                .map(|n| n.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn hex_preview(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Re-index Fortran (column-major) storage into C (row-major) logical order.
fn to_c_order(values: &[Val], shape: &[usize]) -> Vec<Val> {
    let n = shape.len();
    // Column-major strides: stride[0] = 1, stride[k] = prod(shape[0..k]).
    let mut strides = vec![1usize; n];
    for k in 1..n {
        strides[k] = strides[k - 1] * shape[k - 1];
    }
    let mut idx = vec![0usize; n];
    let mut out = Vec::with_capacity(values.len());
    for _ in 0..values.len() {
        let flat: usize = idx.iter().zip(&strides).map(|(i, s)| i * s).sum();
        out.push(values[flat].clone());
        // Increment the multi-index in C order (last axis fastest).
        for axis in (0..n).rev() {
            idx[axis] += 1;
            if idx[axis] < shape[axis] {
                break;
            }
            idx[axis] = 0;
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Element decoding
// ---------------------------------------------------------------------------

fn decode_element(chunk: &[u8], dtype: &DType) -> Result<Val, String> {
    let ordered = |src: &[u8]| -> Vec<u8> {
        let mut v = src.to_vec();
        if dtype.big_endian {
            v.reverse();
        }
        v
    };
    Ok(match dtype.kind {
        Kind::Bool => Val::Bool(chunk[0] != 0),
        Kind::Int => {
            let b = ordered(chunk);
            let mut acc = [0u8; 8];
            acc[..b.len()].copy_from_slice(&b);
            // Sign-extend from the value's own width.
            let raw = u64::from_le_bytes(acc);
            let bits = b.len() * 8;
            let signed = if bits < 64 && (raw >> (bits - 1)) & 1 == 1 {
                (raw | (!0u64 << bits)) as i64
            } else {
                raw as i64
            };
            Val::Int(signed)
        }
        Kind::UInt => {
            let b = ordered(chunk);
            let mut acc = [0u8; 8];
            acc[..b.len()].copy_from_slice(&b);
            Val::UInt(u64::from_le_bytes(acc))
        }
        Kind::Float => Val::Float(decode_float(&ordered(chunk))),
        Kind::Complex => {
            let half = dtype.item_size / 2;
            let re = decode_float(&ordered(&chunk[..half]));
            let im = decode_float(&ordered(&chunk[half..]));
            Val::Complex(re, im)
        }
        Kind::Bytes => {
            let end = chunk.iter().rposition(|b| *b != 0).map_or(0, |p| p + 1);
            Val::Text(String::from_utf8_lossy(&chunk[..end]).to_string())
        }
        Kind::Unicode => {
            let mut s = String::new();
            for cp in chunk.chunks_exact(4) {
                let bits = if dtype.big_endian {
                    u32::from_be_bytes([cp[0], cp[1], cp[2], cp[3]])
                } else {
                    u32::from_le_bytes([cp[0], cp[1], cp[2], cp[3]])
                };
                if bits == 0 {
                    continue; // NUL padding to the fixed item width
                }
                s.push(char::from_u32(bits).ok_or_else(|| {
                    format!(
                        "invalid text data: U+{bits:04X} is not a Unicode scalar value (the dtype {} expects UCS-4 code points)",
                        dtype.descr
                    )
                })?);
            }
            Val::Text(s)
        }
    })
}

fn decode_float(b: &[u8]) -> f64 {
    match b.len() {
        2 => f16_to_f64(u16::from_le_bytes([b[0], b[1]])),
        4 => f32::from_le_bytes([b[0], b[1], b[2], b[3]]) as f64,
        _ => f64::from_le_bytes([b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7]]),
    }
}

/// IEEE-754 binary16 → f64. NumPy's `float16` has no Rust std counterpart, so
/// widen the sign/exponent/mantissa by hand (subnormals and NaN/Inf included).
fn f16_to_f64(bits: u16) -> f64 {
    let sign = ((bits >> 15) & 1) as u32;
    let exp = ((bits >> 10) & 0x1f) as u32;
    let frac = (bits & 0x3ff) as u32;
    let f32_bits = if exp == 0 {
        if frac == 0 {
            sign << 31 // ±0
        } else {
            // Subnormal: normalise the mantissa, then bias for f32.
            let mut f = frac;
            let mut k = 0u32;
            while f & 0x400 == 0 {
                f <<= 1;
                k += 1;
            }
            (sign << 31) | ((113 - k) << 23) | ((f & 0x3ff) << 13)
        }
    } else if exp == 0x1f {
        (sign << 31) | (0xff << 23) | (frac << 13) // ±Inf / NaN
    } else {
        (sign << 31) | ((exp + 112) << 23) | (frac << 13) // rebias 15 → 127
    };
    f32::from_bits(f32_bits) as f64
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

/// Plain-text form of one value (used by `summary` and `csv`).
fn plain(v: &Val) -> String {
    match v {
        Val::Bool(b) => b.to_string(),
        Val::Int(i) => i.to_string(),
        Val::UInt(u) => u.to_string(),
        Val::Float(f) => fmt_float(*f),
        Val::Complex(re, im) => fmt_complex(*re, *im),
        Val::Text(s) => s.clone(),
    }
}

/// JSON form of one value. Non-finite floats and complex numbers become strings
/// (JSON has no literal for either), so the output always parses.
fn json_value(v: &Val) -> String {
    match v {
        Val::Bool(b) => b.to_string(),
        Val::Int(i) => i.to_string(),
        Val::UInt(u) => u.to_string(),
        Val::Float(f) => {
            if f.is_finite() {
                fmt_float(*f)
            } else {
                format!("\"{}\"", fmt_float(*f))
            }
        }
        Val::Complex(re, im) => format!("\"{}\"", fmt_complex(*re, *im)),
        Val::Text(s) => json_string(s),
    }
}

fn fmt_float(f: f64) -> String {
    if f.is_nan() {
        "NaN".to_string()
    } else if f.is_infinite() {
        if f > 0.0 {
            "Infinity".to_string()
        } else {
            "-Infinity".to_string()
        }
    } else {
        format!("{f}")
    }
}

fn fmt_complex(re: f64, im: f64) -> String {
    let sign = if im < 0.0 { "-" } else { "+" };
    format!("{}{}{}j", fmt_float(re), sign, fmt_float(im.abs()))
}

fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Nest the rendered values into `shape`, e.g. `[[1, 2, 3], [4, 5, 6]]`.
fn nest(rendered: &[String], shape: &[usize]) -> String {
    fn go(rendered: &[String], shape: &[usize], idx: &mut usize, out: &mut String) {
        if shape.is_empty() {
            out.push_str(&rendered[*idx]);
            *idx += 1;
            return;
        }
        out.push('[');
        for i in 0..shape[0] {
            if i > 0 {
                out.push_str(", ");
            }
            go(rendered, &shape[1..], idx, out);
        }
        out.push(']');
    }
    let mut out = String::new();
    let mut idx = 0usize;
    go(rendered, shape, &mut idx, &mut out);
    out
}

/// The `data` payload plus whether it had to be flattened by truncation.
fn data_payload(file: &NpyFile, limit: usize, to_text: fn(&Val) -> String) -> (String, bool) {
    let total = file.total_elements();
    if total <= limit {
        let rendered: Vec<String> = file.values.iter().map(to_text).collect();
        (nest(&rendered, &file.shape), false)
    } else {
        let rendered: Vec<String> = file.values.iter().take(limit).map(to_text).collect();
        (format!("[{}]", rendered.join(", ")), true)
    }
}

fn render_header_json(file: &NpyFile) -> String {
    let shape_json = format!(
        "[{}]",
        file.shape
            .iter()
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    );
    format!(
        "{{\n  \"npy_version\": \"{}.{}\",\n  \"dtype\": {},\n  \"dtype_name\": {},\n  \"byte_order\": \"{}\",\n  \"item_size\": {},\n  \"shape\": {},\n  \"ndim\": {},\n  \"total_elements\": {},\n  \"fortran_order\": {},\n  \"header_length\": {},\n  \"data_offset\": {},\n  \"data_bytes\": {},\n  \"trailing_bytes\": {}\n}}",
        file.version.0,
        file.version.1,
        json_string(&file.dtype.descr),
        json_string(&file.dtype.name),
        file.dtype.byte_order(),
        file.dtype.item_size,
        shape_json,
        file.shape.len(),
        file.total_elements(),
        file.fortran_order,
        file.header_len,
        file.data_offset,
        file.data_bytes,
        file.trailing_bytes,
    )
}

fn render_json(file: &NpyFile, limit: usize) -> Result<String, String> {
    let (data, truncated) = data_payload(file, limit, json_value);
    let returned = file.total_elements().min(limit);
    let header = render_header_json(file);
    // Splice the value fields in before the closing brace of the header object.
    let head = header.trim_end_matches('}').trim_end();
    Ok(format!(
        "{head},\n  \"returned_elements\": {returned},\n  \"truncated\": {truncated},\n  \"data\": {data}\n}}"
    ))
}

fn render_summary(file: &NpyFile, limit: usize) -> Result<String, String> {
    let total = file.total_elements();
    let (data, truncated) = data_payload(file, limit, plain);
    let order = if file.fortran_order {
        "Fortran (column-major), re-ordered to row-major below"
    } else {
        "C (row-major)"
    };
    let mut s = String::new();
    s.push_str(&format!(
        "NumPy .npy file, format version {}.{}\n",
        file.version.0, file.version.1
    ));
    s.push_str(&format!(
        "dtype:    {} (descr {}, {} byte{} per element, {})\n",
        file.dtype.name,
        file.dtype.descr,
        file.dtype.item_size,
        if file.dtype.item_size == 1 { "" } else { "s" },
        file.dtype.byte_order()
    ));
    s.push_str(&format!(
        "shape:    {} - {} dimension{}, {} element{}\n",
        file.shape_str(),
        file.shape.len(),
        if file.shape.len() == 1 { "" } else { "s" },
        total,
        if total == 1 { "" } else { "s" }
    ));
    s.push_str(&format!("order:    {order}\n"));
    s.push_str(&format!(
        "layout:   header {} bytes, data starts at offset {}, data {} bytes\n",
        file.header_len, file.data_offset, file.data_bytes
    ));
    if file.trailing_bytes > 0 {
        s.push_str(&format!(
            "extra:    {} unused byte{} after the array data\n",
            file.trailing_bytes,
            if file.trailing_bytes == 1 { "" } else { "s" }
        ));
    }
    if truncated {
        s.push_str(&format!(
            "values:   first {limit} of {total}, flattened in row-major order\n"
        ));
    } else {
        s.push_str(&format!(
            "values:   all {total} element{}\n",
            if total == 1 { "" } else { "s" }
        ));
    }
    s.push_str(&data);
    Ok(s)
}

fn render_csv(file: &NpyFile, limit: usize, delimiter: &str) -> Result<String, String> {
    let sep = resolve_delimiter(delimiter)?;
    // 0-d and 1-d arrays write one value per line (like numpy.savetxt); from 2-d
    // up, the last axis becomes the columns and every earlier axis folds into
    // the rows.
    let cols = match file.shape.len() {
        0 | 1 => 1,
        n => file.shape[n - 1].max(1),
    };
    let total = file.total_elements();
    let max_rows = (limit / cols).max(1);
    let rows = total.div_ceil(cols).min(max_rows);

    let mut out = String::new();
    for r in 0..rows {
        if r > 0 {
            out.push('\n');
        }
        for c in 0..cols {
            let i = r * cols + c;
            if i >= total {
                break;
            }
            if c > 0 {
                out.push(sep);
            }
            out.push_str(&csv_field(&plain(&file.values[i]), sep));
        }
    }
    Ok(out)
}

fn resolve_delimiter(delimiter: &str) -> Result<char, String> {
    match delimiter {
        "" | "," => Ok(','),
        "tab" | "\t" | "\\t" => Ok('\t'),
        other => {
            let mut it = other.chars();
            match (it.next(), it.next()) {
                (Some(c), None) => Ok(c),
                _ => Err(format!(
                    "invalid delimiter {other:?}: expected a single character (e.g. \",\", \";\", \"|\") or the word \"tab\""
                )),
            }
        }
    }
}

fn csv_field(s: &str, sep: char) -> String {
    if s.contains(sep) || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a v1.0 `.npy` file the way `numpy.save` does: magic, version, a
    /// 2-byte little-endian header length, the space-padded dict, then data.
    fn npy_v1(descr: &str, fortran: bool, shape: &[usize], data: &[u8]) -> Vec<u8> {
        let dims = match shape.len() {
            0 => "()".to_string(),
            1 => format!("({},)", shape[0]),
            _ => format!(
                "({})",
                shape
                    .iter()
                    .map(|n| n.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        };
        let mut header = format!(
            "{{'descr': '{descr}', 'fortran_order': {}, 'shape': {dims}, }}",
            if fortran { "True" } else { "False" }
        );
        // numpy pads the header with spaces so data starts on a 64-byte bound.
        while (10 + header.len() + 1) % 64 != 0 {
            header.push(' ');
        }
        header.push('\n');
        let mut out = b"\x93NUMPY\x01\x00".to_vec();
        out.extend_from_slice(&(header.len() as u16).to_le_bytes());
        out.extend_from_slice(header.as_bytes());
        out.extend_from_slice(data);
        out
    }

    fn b64(bytes: &[u8]) -> String {
        const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut s = String::new();
        for ch in bytes.chunks(3) {
            let b = [ch[0], *ch.get(1).unwrap_or(&0), *ch.get(2).unwrap_or(&0)];
            let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
            s.push(T[(n >> 18) as usize & 63] as char);
            s.push(T[(n >> 12) as usize & 63] as char);
            s.push(if ch.len() > 1 {
                T[(n >> 6) as usize & 63] as char
            } else {
                '='
            });
            s.push(if ch.len() > 2 {
                T[n as usize & 63] as char
            } else {
                '='
            });
        }
        s
    }

    fn f64le(v: &[f64]) -> Vec<u8> {
        v.iter().flat_map(|f| f.to_le_bytes()).collect()
    }

    // -- happy paths --------------------------------------------------------

    #[test]
    fn decodes_2d_float64_to_json() {
        let file = npy_v1("<f8", false, &[2, 3], &f64le(&[1.0, 2.0, 3.5, 4.0, 5.0, 6.0]));
        let out = run(&b64(&file), "base64", "json", 0, ",").unwrap();
        assert!(out.contains("\"dtype_name\": \"float64\""), "{out}");
        assert!(out.contains("\"shape\": [2, 3]"), "{out}");
        assert!(out.contains("\"data\": [[1, 2, 3.5], [4, 5, 6]]"), "{out}");
        assert!(out.contains("\"truncated\": false"), "{out}");
    }

    #[test]
    fn summary_reports_dtype_and_shape() {
        let file = npy_v1("<i4", false, &[4], &[1, 0, 0, 0, 2, 0, 0, 0, 3, 0, 0, 0, 4, 0, 0, 0]);
        let out = run(&b64(&file), "auto", "summary", 0, ",").unwrap();
        assert!(out.contains("dtype:    int32 (descr <i4, 4 bytes per element, little-endian)"), "{out}");
        assert!(out.contains("shape:    (4,) - 1 dimension, 4 elements"), "{out}");
        assert!(out.ends_with("[1, 2, 3, 4]"), "{out}");
    }

    #[test]
    fn csv_rows_follow_the_last_axis() {
        let file = npy_v1("<i2", false, &[2, 2], &[1, 0, 2, 0, 3, 0, 4, 0]);
        let out = run(&b64(&file), "auto", "csv", 0, ",").unwrap();
        assert_eq!(out, "1,2\n3,4");
        let tsv = run(&b64(&file), "auto", "csv", 0, "tab").unwrap();
        assert_eq!(tsv, "1\t2\n3\t4");
    }

    #[test]
    fn hex_input_is_auto_detected() {
        let file = npy_v1("|b1", false, &[3], &[1, 0, 1]);
        let hex: String = file.iter().map(|b| format!("{b:02x}")).collect();
        let out = run(&hex, "auto", "csv", 0, ",").unwrap();
        assert_eq!(out, "true\nfalse\ntrue");
    }

    #[test]
    fn fortran_order_is_reindexed_to_row_major() {
        // Column-major storage of [[1, 2, 3], [4, 5, 6]] is 1 4 2 5 3 6.
        let data: Vec<u8> = [1u8, 4, 2, 5, 3, 6].to_vec();
        let file = npy_v1("|u1", true, &[2, 3], &data);
        let out = run(&b64(&file), "auto", "csv", 0, ",").unwrap();
        assert_eq!(out, "1,2,3\n4,5,6");
    }

    #[test]
    fn float16_and_big_endian_decode() {
        // 1.0, -2.0, 0.5 as big-endian float16.
        let file = npy_v1(">f2", false, &[3], &[0x3c, 0x00, 0xc0, 0x00, 0x38, 0x00]);
        let out = run(&b64(&file), "auto", "csv", 0, ",").unwrap();
        assert_eq!(out, "1\n-2\n0.5");
    }

    #[test]
    fn signed_widths_and_unsigned_max_round_trip() {
        let i8s = npy_v1("|i1", false, &[3], &[0xff, 0x80, 0x7f]);
        assert_eq!(run(&b64(&i8s), "auto", "csv", 0, ",").unwrap(), "-1\n-128\n127");
        let u64s = npy_v1("<u8", false, &[1], &[0xff; 8]);
        assert_eq!(
            run(&b64(&u64s), "auto", "csv", 0, ",").unwrap(),
            "18446744073709551615"
        );
    }

    #[test]
    fn fixed_width_text_dtypes_decode() {
        let s = npy_v1("|S3", false, &[2], b"ab\0cde");
        assert_eq!(run(&b64(&s), "auto", "csv", 0, ",").unwrap(), "ab\ncde");
        // '<U2' — two UCS-4 code points per element, NUL padded.
        let mut u = Vec::new();
        u.extend_from_slice(&('h' as u32).to_le_bytes());
        u.extend_from_slice(&('i' as u32).to_le_bytes());
        let u = npy_v1("<U2", false, &[1], &u);
        let out = run(&b64(&u), "auto", "json", 0, ",").unwrap();
        assert!(out.contains("\"data\": [\"hi\"]"), "{out}");
    }

    #[test]
    fn complex_values_render_as_strings() {
        let mut data = f64le(&[1.5, 2.0]);
        data.extend(f64le(&[-1.0, -0.5]));
        let file = npy_v1("<c16", false, &[2], &data);
        let out = run(&b64(&file), "auto", "csv", 0, ",").unwrap();
        assert_eq!(out, "1.5+2j\n-1-0.5j");
    }

    #[test]
    fn non_finite_floats_stay_valid_json() {
        let file = npy_v1("<f8", false, &[3], &f64le(&[f64::NAN, f64::INFINITY, -1.0]));
        let out = run(&b64(&file), "auto", "json", 0, ",").unwrap();
        assert!(out.contains("\"data\": [\"NaN\", \"Infinity\", -1]"), "{out}");
    }

    #[test]
    fn header_output_reports_layout_only() {
        let file = npy_v1("<f8", false, &[2, 3], &f64le(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]));
        let out = run(&b64(&file), "auto", "header", 0, ",").unwrap();
        assert!(out.contains("\"npy_version\": \"1.0\""), "{out}");
        assert!(out.contains("\"data_offset\": 128"), "{out}");
        assert!(out.contains("\"data_bytes\": 48"), "{out}");
        assert!(!out.contains("\"data\":"), "{out}");
    }

    #[test]
    fn v2_header_length_field_is_four_bytes() {
        let mut file = npy_v1("<u1", false, &[2], &[7, 9]);
        // Rewrite the v1.0 prologue as a v2.0 one (4-byte header length).
        let header_len = u16::from_le_bytes([file[8], file[9]]) as u32;
        let mut v2 = b"\x93NUMPY\x02\x00".to_vec();
        v2.extend_from_slice(&header_len.to_le_bytes());
        v2.extend_from_slice(&file.split_off(10));
        let out = run(&b64(&v2), "auto", "summary", 0, ",").unwrap();
        assert!(out.contains("format version 2.0"), "{out}");
        assert!(out.ends_with("[7, 9]"), "{out}");
    }

    #[test]
    fn limit_truncates_and_flags_it() {
        let file = npy_v1("<u1", false, &[2, 3], &[1, 2, 3, 4, 5, 6]);
        let out = run(&b64(&file), "auto", "json", 4, ",").unwrap();
        assert!(out.contains("\"truncated\": true"), "{out}");
        assert!(out.contains("\"returned_elements\": 4"), "{out}");
        assert!(out.contains("\"data\": [1, 2, 3, 4]"), "{out}");
    }

    #[test]
    fn scalar_and_empty_arrays_are_handled() {
        let scalar = npy_v1("<f8", false, &[], &f64le(&[42.0]));
        assert!(run(&b64(&scalar), "auto", "summary", 0, ",")
            .unwrap()
            .ends_with("42"));
        let empty = npy_v1("<f8", false, &[0], &[]);
        let out = run(&b64(&empty), "auto", "json", 0, ",").unwrap();
        assert!(out.contains("\"data\": []"), "{out}");
    }

    // -- errors -------------------------------------------------------------

    #[test]
    fn rejects_a_file_without_the_numpy_magic() {
        let err = run(&b64(b"PK\x03\x04not-an-npy-file"), "base64", "summary", 0, ",").unwrap_err();
        assert!(err.contains("not a .npy file"), "{err}");
        assert!(err.contains("\\x93NUMPY"), "{err}");
    }

    #[test]
    fn rejects_truncated_data() {
        // Header says 6 float64 elements (48 bytes) but only 16 bytes follow.
        let file = npy_v1("<f8", false, &[2, 3], &f64le(&[1.0, 2.0]));
        let err = run(&b64(&file), "auto", "summary", 0, ",").unwrap_err();
        assert!(err.contains("truncated .npy data"), "{err}");
        assert!(err.contains("48 bytes"), "{err}");
        assert!(err.contains("only 16 bytes follow"), "{err}");
    }

    #[test]
    fn rejects_object_arrays_with_a_pickle_explanation() {
        let file = npy_v1("|O", false, &[2], &[0, 0]);
        let err = run(&b64(&file), "auto", "summary", 0, ",").unwrap_err();
        assert!(err.contains("unsupported dtype"), "{err}");
        assert!(err.contains("pickled"), "{err}");
    }

    #[test]
    fn rejects_structured_record_arrays() {
        // A record dtype writes descr as a list of (name, format) tuples.
        let header = "{'descr': [('a', '<i4'), ('b', '<f8')], 'fortran_order': False, 'shape': (1,), }\n";
        let mut file = b"\x93NUMPY\x01\x00".to_vec();
        file.extend_from_slice(&(header.len() as u16).to_le_bytes());
        file.extend_from_slice(header.as_bytes());
        file.extend_from_slice(&[0u8; 12]);
        let err = run(&b64(&file), "auto", "summary", 0, ",").unwrap_err();
        assert!(err.contains("structured (record) array"), "{err}");
    }

    #[test]
    fn rejects_an_unsupported_version() {
        let mut file = npy_v1("<u1", false, &[1], &[1]);
        file[6] = 9;
        let err = run(&b64(&file), "auto", "summary", 0, ",").unwrap_err();
        assert!(err.contains("unsupported .npy format version 9.0"), "{err}");
    }

    #[test]
    fn rejects_a_header_missing_a_required_key() {
        let header = "{'descr': '<f8', 'shape': (1,), }\n";
        let mut file = b"\x93NUMPY\x01\x00".to_vec();
        file.extend_from_slice(&(header.len() as u16).to_le_bytes());
        file.extend_from_slice(header.as_bytes());
        file.extend_from_slice(&f64le(&[1.0]));
        let err = run(&b64(&file), "auto", "summary", 0, ",").unwrap_err();
        assert!(err.contains("'fortran_order' is missing"), "{err}");
    }

    #[test]
    fn rejects_a_bad_output_mode_and_delimiter() {
        let file = npy_v1("<u1", false, &[1], &[1]);
        let err = run(&b64(&file), "auto", "yaml", 0, ",").unwrap_err();
        assert!(err.contains("invalid output"), "{err}");
        let err = run(&b64(&file), "auto", "csv", 0, "||").unwrap_err();
        assert!(err.contains("invalid delimiter"), "{err}");
    }

    #[test]
    fn rejects_empty_and_undecodable_input() {
        let err = run("   ", "auto", "summary", 0, ",").unwrap_err();
        assert!(err.contains("input is empty"), "{err}");
        let err = run("k05VTVBZ$$$", "base64", "summary", 0, ",").unwrap_err();
        assert!(err.contains("invalid base64 character"), "{err}");
        let err = run("93 4e 55 4d 50 5", "hex", "summary", 0, ",").unwrap_err();
        assert!(err.contains("odd count"), "{err}");
    }

    #[test]
    fn rejects_an_oversized_limit() {
        let file = npy_v1("<u1", false, &[1], &[1]);
        let err = run(&b64(&file), "auto", "json", MAX_LIMIT + 1, ",").unwrap_err();
        assert!(err.contains("too large"), "{err}");
    }
}
