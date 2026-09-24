//! charcode-codec core — turn text into a list of numeric character codes (in a
//! chosen base) and rebuild the text from such a list. Pure compute, no
//! wafer/wasm-bindgen deps — shared by the chat skill block and the web page.
//!
//! The unit a "character code" counts in is the `scope`:
//!   * `unicode-scalar` (default) — one code per Unicode scalar value / code
//!     point, so `😀` is the single code 128512 (U+1F600);
//!   * `utf8-bytes` — one code per UTF-8 byte, so `😀` is 240 159 152 128;
//!   * `utf16-units` — one code per UTF-16 code unit, so `😀` is the surrogate
//!     pair 55357 56832;
//!   * `ascii` — one code per character, rejecting anything above 127.
//!
//! Encoding renders each code in `dec`/`hex`/`bin`/`oct`, optionally zero-padded
//! to the scope's natural width and optionally prefixed (`0x`, `\x`, `U+`), joined
//! by a delimiter. Decoding is deliberately tolerant: it ignores whitespace and
//! common separators, strips those same prefixes, and can also split an
//! unseparated run of fixed-width digits (e.g. `48656c6c6f`), so anything this
//! tool emits round-trips back without describing how it was formatted.

/// Maximum number of codes handled in one call, in either direction.
pub const MAX_CODES: usize = 200_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Encode,
    Decode,
}

impl Mode {
    pub fn parse(s: &str) -> Result<Mode, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "encode" => Ok(Mode::Encode),
            "decode" => Ok(Mode::Decode),
            other => Err(format!("unknown mode '{other}' (use 'encode' or 'decode')")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Base {
    Dec,
    Hex,
    Bin,
    Oct,
}

impl Base {
    pub fn parse(s: &str) -> Result<Base, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "dec" | "decimal" => Ok(Base::Dec),
            "hex" | "hexadecimal" => Ok(Base::Hex),
            "bin" | "binary" => Ok(Base::Bin),
            "oct" | "octal" => Ok(Base::Oct),
            other => Err(format!(
                "unknown base '{other}' (use 'dec', 'hex', 'bin' or 'oct')"
            )),
        }
    }

    fn radix(self) -> u32 {
        match self {
            Base::Dec => 10,
            Base::Hex => 16,
            Base::Bin => 2,
            Base::Oct => 8,
        }
    }

    /// Human name used in error messages.
    fn name(self) -> &'static str {
        match self {
            Base::Dec => "decimal",
            Base::Hex => "hexadecimal",
            Base::Bin => "binary",
            Base::Oct => "octal",
        }
    }

    fn render(self, code: u32, width: usize, uppercase: bool) -> String {
        match self {
            Base::Dec => format!("{code:0width$}"),
            Base::Hex if uppercase => format!("{code:0width$X}"),
            Base::Hex => format!("{code:0width$x}"),
            Base::Bin => format!("{code:0width$b}"),
            Base::Oct => format!("{code:0width$o}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    UnicodeScalar,
    Utf8Bytes,
    Utf16Units,
    Ascii,
}

impl Scope {
    pub fn parse(s: &str) -> Result<Scope, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "unicode-scalar" | "unicode" | "codepoint" => Ok(Scope::UnicodeScalar),
            "utf8-bytes" | "utf8" | "bytes" => Ok(Scope::Utf8Bytes),
            "utf16-units" | "utf16" => Ok(Scope::Utf16Units),
            "ascii" => Ok(Scope::Ascii),
            other => Err(format!(
                "unknown scope '{other}' (use 'unicode-scalar', 'utf8-bytes', 'utf16-units' or 'ascii')"
            )),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Scope::UnicodeScalar => "unicode-scalar",
            Scope::Utf8Bytes => "utf8-bytes",
            Scope::Utf16Units => "utf16-units",
            Scope::Ascii => "ascii",
        }
    }

    /// Largest code this scope can hold — the decode range check.
    fn max_code(self) -> u32 {
        match self {
            Scope::UnicodeScalar => 0x10FFFF,
            Scope::Utf8Bytes => 0xFF,
            Scope::Utf16Units => 0xFFFF,
            Scope::Ascii => 0x7F,
        }
    }

    /// Zero-padding width used by `padding = "fixed"`, and the chunk size used
    /// when splitting an unseparated run of digits on decode.
    ///
    /// Every width is the exact digit count of the scope's maximum value in that
    /// base, EXCEPT `unicode-scalar` + `hex`, which uses the conventional
    /// 4-digit minimum (`U+0041`, and `U+1F600` grows naturally past it).
    fn fixed_width(self, base: Base) -> usize {
        match (self, base) {
            (Scope::UnicodeScalar, Base::Dec) => 7,
            (Scope::UnicodeScalar, Base::Hex) => 4,
            (Scope::UnicodeScalar, Base::Bin) => 21,
            (Scope::UnicodeScalar, Base::Oct) => 7,
            (Scope::Utf8Bytes, Base::Dec) => 3,
            (Scope::Utf8Bytes, Base::Hex) => 2,
            (Scope::Utf8Bytes, Base::Bin) => 8,
            (Scope::Utf8Bytes, Base::Oct) => 3,
            (Scope::Utf16Units, Base::Dec) => 5,
            (Scope::Utf16Units, Base::Hex) => 4,
            (Scope::Utf16Units, Base::Bin) => 16,
            (Scope::Utf16Units, Base::Oct) => 6,
            // ASCII is a 7-bit code, so its binary width is 7, not 8.
            (Scope::Ascii, Base::Dec) => 3,
            (Scope::Ascii, Base::Hex) => 2,
            (Scope::Ascii, Base::Bin) => 7,
            (Scope::Ascii, Base::Oct) => 3,
        }
    }

    /// `unicode-scalar` hex pads to a 4-digit MINIMUM, so a fixed-width chunk
    /// split of an unseparated run would be wrong there.
    fn chunkable(self, base: Base) -> bool {
        !matches!((self, base), (Scope::UnicodeScalar, Base::Hex))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delimiter {
    Space,
    Comma,
    Newline,
    None,
}

impl Delimiter {
    pub fn parse(s: &str) -> Result<Delimiter, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "space" => Ok(Delimiter::Space),
            "comma" => Ok(Delimiter::Comma),
            "newline" => Ok(Delimiter::Newline),
            "none" => Ok(Delimiter::None),
            other => Err(format!(
                "unknown delimiter '{other}' (use 'space', 'comma', 'newline' or 'none')"
            )),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Delimiter::Space => " ",
            Delimiter::Comma => ", ",
            Delimiter::Newline => "\n",
            Delimiter::None => "",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Prefix {
    None,
    ZeroX,
    BackslashX,
    UPlus,
}

impl Prefix {
    pub fn parse(s: &str) -> Result<Prefix, String> {
        match s.trim() {
            "" | "none" => Ok(Prefix::None),
            "0x" | "0X" => Ok(Prefix::ZeroX),
            "\\x" => Ok(Prefix::BackslashX),
            "U+" | "u+" => Ok(Prefix::UPlus),
            other => Err(format!(
                "unknown prefix '{other}' (use 'none', '0x', '\\x' or 'U+')"
            )),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Prefix::None => "",
            Prefix::ZeroX => "0x",
            Prefix::BackslashX => "\\x",
            Prefix::UPlus => "U+",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Padding {
    None,
    Fixed,
}

impl Padding {
    pub fn parse(s: &str) -> Result<Padding, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "none" => Ok(Padding::None),
            "fixed" => Ok(Padding::Fixed),
            other => Err(format!("unknown padding '{other}' (use 'none' or 'fixed')")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Text,
    Json,
}

impl Format {
    pub fn parse(s: &str) -> Result<Format, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "text" => Ok(Format::Text),
            "json" => Ok(Format::Json),
            other => Err(format!("unknown format '{other}' (use 'text' or 'json')")),
        }
    }
}

/// Parse every string param, then encode or decode. This is the single entry
/// point every surface (chat block, CLI, web page) calls.
#[allow(clippy::too_many_arguments)]
pub fn convert(
    input: &str,
    mode: &str,
    base: &str,
    scope: &str,
    delimiter: &str,
    prefix: &str,
    padding: &str,
    uppercase: bool,
    format: &str,
) -> Result<String, String> {
    let mode = Mode::parse(mode)?;
    let base = Base::parse(base)?;
    let scope = Scope::parse(scope)?;
    let delimiter = Delimiter::parse(delimiter)?;
    let prefix = Prefix::parse(prefix)?;
    let padding = Padding::parse(padding)?;
    let format = Format::parse(format)?;

    match mode {
        Mode::Encode => encode(
            input, base, scope, delimiter, prefix, padding, uppercase, format,
        ),
        Mode::Decode => decode(input, base, scope, format),
    }
}

// ---------------------------------------------------------------- encode

/// Split `text` into the numeric codes `scope` counts in.
fn codes_of(text: &str, scope: Scope) -> Result<Vec<u32>, String> {
    match scope {
        Scope::UnicodeScalar => Ok(text.chars().map(|c| c as u32).collect()),
        Scope::Utf8Bytes => Ok(text.bytes().map(u32::from).collect()),
        Scope::Utf16Units => Ok(text.encode_utf16().map(u32::from).collect()),
        Scope::Ascii => text
            .chars()
            .map(|c| {
                if c.is_ascii() {
                    Ok(c as u32)
                } else {
                    Err(format!(
                        "character '{c}' (U+{:04X}) is outside ASCII (0-127); use scope 'unicode-scalar' or 'utf8-bytes' instead",
                        c as u32
                    ))
                }
            })
            .collect(),
    }
}

#[allow(clippy::too_many_arguments)]
fn encode(
    text: &str,
    base: Base,
    scope: Scope,
    delimiter: Delimiter,
    prefix: Prefix,
    padding: Padding,
    uppercase: bool,
    format: Format,
) -> Result<String, String> {
    if text.is_empty() {
        return Err("input text is empty".into());
    }
    let codes = codes_of(text, scope)?;
    if codes.len() > MAX_CODES {
        return Err(format!(
            "input produces {} codes, above the {MAX_CODES} limit",
            codes.len()
        ));
    }
    let width = match padding {
        Padding::Fixed => scope.fixed_width(base),
        Padding::None => 1,
    };
    let rendered: Vec<String> = codes
        .iter()
        .map(|&c| format!("{}{}", prefix.as_str(), base.render(c, width, uppercase)))
        .collect();
    let joined = rendered.join(delimiter.as_str());

    match format {
        Format::Text => Ok(joined),
        Format::Json => {
            let mut out = String::from("{\n");
            out.push_str("  \"mode\": \"encode\",\n");
            out.push_str(&format!("  \"base\": \"{}\",\n", base_key(base)));
            out.push_str(&format!("  \"scope\": \"{}\",\n", scope.label()));
            out.push_str(&format!("  \"count\": {},\n", codes.len()));
            out.push_str(&format!(
                "  \"codes\": [{}],\n",
                codes
                    .iter()
                    .map(|c| c.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
            out.push_str(&format!(
                "  \"formatted\": [{}],\n",
                rendered
                    .iter()
                    .map(|s| json_string(s))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
            out.push_str(&format!("  \"output\": {}\n", json_string(&joined)));
            out.push('}');
            Ok(out)
        }
    }
}

fn base_key(base: Base) -> &'static str {
    match base {
        Base::Dec => "dec",
        Base::Hex => "hex",
        Base::Bin => "bin",
        Base::Oct => "oct",
    }
}

/// Minimal JSON string escaping — enough for arbitrary decoded text.
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

// ---------------------------------------------------------------- decode

/// Characters that separate codes on decode, whatever delimiter was used to
/// encode them. Whitespace is handled separately.
fn is_separator(c: char) -> bool {
    c.is_whitespace() || matches!(c, ',' | ';' | ':' | '|' | '-' | '/' | '\'' | '"')
}

/// Strip any of the notations this tool (or a neighbouring one) can emit.
fn strip_prefix_notation(tok: &str) -> &str {
    let t = tok.trim_end_matches(';');
    for p in [
        "0x", "0X", "\\x", "\\X", "\\u", "\\U", "U+", "u+", "0b", "0B", "0o", "0O", "&#x", "&#X",
        "&#", "#",
    ] {
        if let Some(rest) = t.strip_prefix(p) {
            if !rest.is_empty() {
                return rest;
            }
        }
    }
    t
}

/// Split the raw input into code tokens. An input with no separators at all is
/// chunked into fixed-width groups when its length is an exact multiple of the
/// scope's natural width (so `48656c6c6f` decodes as five hex bytes).
fn tokenize(input: &str, base: Base, scope: Scope) -> Vec<String> {
    let normalized = input
        .replace("\\x", " ")
        .replace("\\X", " ")
        .replace("\\u", " ")
        .replace("\\U", " ")
        .replace("U+", " ")
        .replace("u+", " ")
        .replace("0x", " ")
        .replace("0X", " ")
        .replace("0b", " ")
        .replace("0B", " ")
        .replace("0o", " ")
        .replace("0O", " ");
    let mut toks: Vec<String> = normalized
        .split(is_separator)
        .filter(|t| !t.is_empty())
        .map(|t| strip_prefix_notation(t).to_string())
        .filter(|t| !t.is_empty())
        .collect();

    if toks.len() == 1 && scope.chunkable(base) {
        let w = scope.fixed_width(base);
        let only = &toks[0];
        if only.len() > w && only.len() % w == 0 {
            toks = only
                .as_bytes()
                .chunks(w)
                .map(|c| String::from_utf8_lossy(c).into_owned())
                .collect();
        }
    }
    toks
}

fn decode(input: &str, base: Base, scope: Scope, format: Format) -> Result<String, String> {
    if input.trim().is_empty() {
        return Err("input is empty — paste the character codes to decode".into());
    }
    let toks = tokenize(input, base, scope);
    if toks.is_empty() {
        return Err("no character codes found in the input".into());
    }
    if toks.len() > MAX_CODES {
        return Err(format!(
            "input has {} codes, above the {MAX_CODES} limit",
            toks.len()
        ));
    }

    let mut codes: Vec<u32> = Vec::with_capacity(toks.len());
    for tok in &toks {
        let code = u32::from_str_radix(tok, base.radix()).map_err(|_| {
            format!(
                "'{tok}' is not a valid {} number (base {}){}",
                base.name(),
                base.radix(),
                if base != Base::Dec && tok.chars().all(|c| c.is_ascii_digit()) {
                    " — those are all decimal digits, so try base 'dec'"
                } else {
                    ""
                }
            )
        })?;
        if code > scope.max_code() {
            return Err(format!(
                "code {code} (0x{code:X}) is outside the '{}' range 0-{} ({})",
                scope.label(),
                scope.max_code(),
                match scope {
                    Scope::Ascii => "ASCII is 7-bit; try scope 'unicode-scalar'",
                    Scope::Utf8Bytes => "a UTF-8 byte is 0-255; try scope 'unicode-scalar'",
                    Scope::Utf16Units => "a UTF-16 code unit is 16-bit; try scope 'unicode-scalar'",
                    Scope::UnicodeScalar => "U+10FFFF is the highest Unicode code point",
                }
            ));
        }
        codes.push(code);
    }

    let text = match scope {
        Scope::UnicodeScalar => {
            let mut s = String::with_capacity(codes.len());
            for &c in &codes {
                let ch = char::from_u32(c).ok_or_else(|| {
                    format!(
                        "code {c} (U+{c:04X}) is a surrogate and not a Unicode scalar value — \
                         surrogates only appear in UTF-16; try scope 'utf16-units'"
                    )
                })?;
                s.push(ch);
            }
            s
        }
        Scope::Ascii => codes.iter().map(|&c| c as u8 as char).collect(),
        Scope::Utf8Bytes => {
            let bytes: Vec<u8> = codes.iter().map(|&c| c as u8).collect();
            String::from_utf8(bytes).map_err(|e| {
                format!(
                    "the byte sequence is not valid UTF-8 (first bad byte at index {}) — \
                     check the base, or try scope 'unicode-scalar'",
                    e.utf8_error().valid_up_to()
                )
            })?
        }
        Scope::Utf16Units => {
            let units: Vec<u16> = codes.iter().map(|&c| c as u16).collect();
            String::from_utf16(&units).map_err(|_| {
                "the UTF-16 code units contain an unpaired surrogate — a character above U+FFFF \
                 needs both halves of its surrogate pair"
                    .to_string()
            })?
        }
    };

    match format {
        Format::Text => Ok(text),
        Format::Json => {
            let mut out = String::from("{\n");
            out.push_str("  \"mode\": \"decode\",\n");
            out.push_str(&format!("  \"base\": \"{}\",\n", base_key(base)));
            out.push_str(&format!("  \"scope\": \"{}\",\n", scope.label()));
            out.push_str(&format!("  \"count\": {},\n", codes.len()));
            out.push_str(&format!(
                "  \"codes\": [{}],\n",
                codes
                    .iter()
                    .map(|c| c.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
            out.push_str(&format!("  \"text\": {}\n", json_string(&text)));
            out.push('}');
            Ok(out)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Encode with every default: decimal Unicode scalar values, space-joined.
    fn enc(text: &str, base: &str, scope: &str) -> String {
        convert(
            text, "encode", base, scope, "space", "none", "none", true, "text",
        )
        .unwrap()
    }

    fn dec_(codes: &str, base: &str, scope: &str) -> String {
        convert(
            codes, "decode", base, scope, "space", "none", "none", true, "text",
        )
        .unwrap()
    }

    #[test]
    fn encodes_ascii_text_to_decimal_code_points() {
        assert_eq!(enc("Hello", "dec", "unicode-scalar"), "72 101 108 108 111");
    }

    #[test]
    fn decodes_decimal_code_points_back_to_text() {
        assert_eq!(dec_("72 101 108 108 111", "dec", "unicode-scalar"), "Hello");
    }

    /// An astral character is ONE scalar value but four UTF-8 bytes and two
    /// UTF-16 units — the three scopes must disagree in exactly that way.
    #[test]
    fn scopes_split_an_astral_char_differently() {
        assert_eq!(enc("😀", "dec", "unicode-scalar"), "128512");
        assert_eq!(enc("😀", "dec", "utf8-bytes"), "240 159 152 128");
        assert_eq!(enc("😀", "dec", "utf16-units"), "55357 56832");
        assert_eq!(dec_("128512", "dec", "unicode-scalar"), "😀");
        assert_eq!(dec_("240 159 152 128", "dec", "utf8-bytes"), "😀");
        assert_eq!(dec_("55357 56832", "dec", "utf16-units"), "😀");
    }

    #[test]
    fn hex_and_binary_round_trip() {
        assert_eq!(enc("Hi", "hex", "unicode-scalar"), "48 69");
        assert_eq!(dec_("48 69", "hex", "unicode-scalar"), "Hi");
        assert_eq!(enc("Hi", "bin", "utf8-bytes"), "1001000 1101001");
        assert_eq!(dec_("1001000 1101001", "bin", "utf8-bytes"), "Hi");
        assert_eq!(enc("Hi", "oct", "ascii"), "110 151");
        assert_eq!(dec_("110 151", "oct", "ascii"), "Hi");
    }

    #[test]
    fn uppercase_toggles_hex_digit_case() {
        assert_eq!(enc("é", "hex", "unicode-scalar"), "E9");
        let lower = convert(
            "é",
            "encode",
            "hex",
            "unicode-scalar",
            "space",
            "none",
            "none",
            false,
            "text",
        )
        .unwrap();
        assert_eq!(lower, "e9");
        // Decoding is case-insensitive either way.
        assert_eq!(dec_("e9", "hex", "unicode-scalar"), "é");
        assert_eq!(dec_("E9", "hex", "unicode-scalar"), "é");
    }

    #[test]
    fn fixed_padding_uses_the_scope_width() {
        let pad = |text: &str, base: &str, scope: &str| {
            convert(
                text, "encode", base, scope, "space", "none", "fixed", true, "text",
            )
            .unwrap()
        };
        assert_eq!(pad("Hi", "dec", "ascii"), "072 105");
        assert_eq!(pad("Hi", "bin", "ascii"), "1001000 1101001"); // ASCII is 7-bit
        assert_eq!(pad("Hi", "bin", "utf8-bytes"), "01001000 01101001");
        assert_eq!(pad("Hi", "hex", "utf8-bytes"), "48 69");
        // unicode-scalar hex pads to a 4-digit MINIMUM and grows past it.
        assert_eq!(pad("A😀", "hex", "unicode-scalar"), "0041 1F600");
    }

    #[test]
    fn prefix_and_delimiter_are_applied_and_then_ignored_on_decode() {
        let out = convert(
            "Hi",
            "encode",
            "hex",
            "unicode-scalar",
            "comma",
            "U+",
            "fixed",
            true,
            "text",
        )
        .unwrap();
        assert_eq!(out, "U+0048, U+0069");
        assert_eq!(dec_(&out, "hex", "unicode-scalar"), "Hi");

        let esc = convert(
            "Hi",
            "encode",
            "hex",
            "utf8-bytes",
            "none",
            "\\x",
            "fixed",
            false,
            "text",
        )
        .unwrap();
        assert_eq!(esc, "\\x48\\x69");
        assert_eq!(dec_(&esc, "hex", "utf8-bytes"), "Hi");
    }

    /// An unseparated run of fixed-width digits is chunked on decode.
    #[test]
    fn decodes_an_unseparated_run_of_fixed_width_codes() {
        assert_eq!(dec_("48656c6c6f", "hex", "utf8-bytes"), "Hello");
        assert_eq!(dec_("0100100001101001", "bin", "utf8-bytes"), "Hi");
        // A single short token is still one code, not a chunk split.
        assert_eq!(dec_("41", "hex", "unicode-scalar"), "A");
    }

    #[test]
    fn newline_delimiter_and_json_format() {
        let nl = convert(
            "Hi",
            "encode",
            "dec",
            "unicode-scalar",
            "newline",
            "none",
            "none",
            true,
            "text",
        )
        .unwrap();
        assert_eq!(nl, "72\n105");
        let json = convert(
            "Hi",
            "encode",
            "dec",
            "unicode-scalar",
            "space",
            "none",
            "none",
            true,
            "json",
        )
        .unwrap();
        assert!(json.contains("\"codes\": [72, 105]"), "{json}");
        assert!(json.contains("\"output\": \"72 105\""), "{json}");
        assert!(json.contains("\"count\": 2"), "{json}");
        let back = convert(
            "72 105",
            "decode",
            "dec",
            "unicode-scalar",
            "space",
            "none",
            "none",
            true,
            "json",
        )
        .unwrap();
        assert!(back.contains("\"text\": \"Hi\""), "{back}");
    }

    #[test]
    fn ascii_scope_rejects_non_ascii_text_on_encode() {
        let err = convert(
            "café", "encode", "dec", "ascii", "space", "none", "none", true, "text",
        )
        .unwrap_err();
        assert!(err.contains("'é'"), "{err}");
        assert!(err.contains("outside ASCII"), "{err}");
    }

    #[test]
    fn decode_rejects_out_of_range_codes_per_scope() {
        let err = dec_err("200", "dec", "ascii");
        assert!(err.contains("outside the 'ascii' range 0-127"), "{err}");

        let err = dec_err("300", "dec", "utf8-bytes");
        assert!(
            err.contains("outside the 'utf8-bytes' range 0-255"),
            "{err}"
        );

        let err = dec_err("1114112", "dec", "unicode-scalar");
        assert!(err.contains("U+10FFFF is the highest"), "{err}");
    }

    #[test]
    fn decode_rejects_surrogates_as_scalar_values() {
        let err = dec_err("55357", "dec", "unicode-scalar");
        assert!(err.contains("surrogate"), "{err}");
        // The same value IS valid as a UTF-16 unit, but unpaired it still fails.
        let err = dec_err("55357", "dec", "utf16-units");
        assert!(err.contains("unpaired surrogate"), "{err}");
    }

    #[test]
    fn decode_rejects_digits_that_are_invalid_in_the_chosen_base() {
        let err = dec_err("1002", "bin", "utf8-bytes");
        assert!(err.contains("not a valid binary number"), "{err}");
        // All-decimal digits that are illegal in this base get a base hint.
        assert!(err.contains("try base 'dec'"), "{err}");
        let err = dec_err("zz", "hex", "unicode-scalar");
        assert!(err.contains("not a valid hexadecimal number"), "{err}");
        assert!(
            !err.contains("try base 'dec'"),
            "no bogus hint for non-digits: {err}"
        );
        // '99' IS valid hexadecimal, so it must decode rather than error.
        assert_eq!(dec_("99", "hex", "unicode-scalar"), "\u{99}");
    }

    #[test]
    fn decode_rejects_invalid_utf8_byte_sequences() {
        let err = dec_err("ff fe", "hex", "utf8-bytes");
        assert!(err.contains("not valid UTF-8"), "{err}");
    }

    #[test]
    fn empty_input_is_an_error_in_both_directions() {
        assert!(convert(
            "",
            "encode",
            "dec",
            "unicode-scalar",
            "space",
            "none",
            "none",
            true,
            "text"
        )
        .unwrap_err()
        .contains("empty"));
        assert!(convert(
            "   ",
            "decode",
            "dec",
            "unicode-scalar",
            "space",
            "none",
            "none",
            true,
            "text"
        )
        .unwrap_err()
        .contains("empty"));
    }

    #[test]
    fn unknown_enum_values_are_rejected_with_the_valid_list() {
        let bad = |field: usize| {
            let mut a = [
                "Hi",
                "encode",
                "dec",
                "unicode-scalar",
                "space",
                "none",
                "none",
                "text",
            ];
            a[field] = "nope";
            convert(a[0], a[1], a[2], a[3], a[4], a[5], a[6], true, a[7]).unwrap_err()
        };
        assert!(bad(1).contains("unknown mode"));
        assert!(bad(2).contains("unknown base"));
        assert!(bad(3).contains("unknown scope"));
        assert!(bad(4).contains("unknown delimiter"));
        assert!(bad(5).contains("unknown prefix"));
        assert!(bad(6).contains("unknown padding"));
        assert!(bad(7).contains("unknown format"));
    }

    /// Helper: return the error text, or an empty string when the call succeeded.
    fn dec_err(codes: &str, base: &str, scope: &str) -> String {
        convert(
            codes, "decode", base, scope, "space", "none", "none", true, "text",
        )
        .err()
        .unwrap_or_default()
    }
}
