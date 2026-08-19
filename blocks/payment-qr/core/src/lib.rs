//! payment-qr core — build a BIP-21 style payment URI (or a generic text payload)
//! and render it as a scannable QR code SVG.
//!
//! Pure Rust: `qrcode` for the encode, `bs58` for Base58Check, and a hand-rolled
//! Bech32/Bech32m verifier (no extra crate). No wafer/wasm-bindgen deps, so the
//! same logic backs the chat block, the CLI and the browser page.
//!
//! Unlike most online payment-QR generators, addresses are CHECKSUM-verified, not
//! prefix-sniffed: a single mistyped character is rejected instead of silently
//! producing a scannable QR that pays nowhere.

use qrcode::{EcLevel, QrCode};

/// Payment URI scheme.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scheme {
    Bitcoin,
    Litecoin,
    Dogecoin,
    Ethereum,
    Lightning,
    Text,
}

impl Scheme {
    pub fn parse(s: &str) -> Result<Scheme, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "bitcoin" | "btc" | "" => Ok(Scheme::Bitcoin),
            "litecoin" | "ltc" => Ok(Scheme::Litecoin),
            "dogecoin" | "doge" => Ok(Scheme::Dogecoin),
            "ethereum" | "eth" => Ok(Scheme::Ethereum),
            "lightning" | "bolt11" => Ok(Scheme::Lightning),
            "text" | "generic" => Ok(Scheme::Text),
            other => Err(format!(
                "unknown scheme '{other}' (use bitcoin, litecoin, dogecoin, ethereum, lightning, or text)"
            )),
        }
    }

    fn prefix(self) -> &'static str {
        match self {
            Scheme::Bitcoin => "bitcoin",
            Scheme::Litecoin => "litecoin",
            Scheme::Dogecoin => "dogecoin",
            Scheme::Ethereum => "ethereum",
            Scheme::Lightning => "lightning",
            Scheme::Text => "",
        }
    }

    /// Human name used in error messages.
    fn coin(self) -> &'static str {
        match self {
            Scheme::Bitcoin => "Bitcoin",
            Scheme::Litecoin => "Litecoin",
            Scheme::Dogecoin => "Dogecoin",
            Scheme::Ethereum => "Ethereum",
            Scheme::Lightning => "Lightning",
            Scheme::Text => "text",
        }
    }

    /// Schemes that use the BIP-21 `scheme:address?amount=&label=&message=` grammar.
    fn is_bip21(self) -> bool {
        matches!(self, Scheme::Bitcoin | Scheme::Litecoin | Scheme::Dogecoin)
    }
}

/// QR error-correction level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ecc {
    L,
    M,
    Q,
    H,
}

impl Ecc {
    pub fn parse(s: &str) -> Result<Ecc, String> {
        match s.trim().to_ascii_uppercase().as_str() {
            "L" => Ok(Ecc::L),
            "M" | "" => Ok(Ecc::M),
            "Q" => Ok(Ecc::Q),
            "H" => Ok(Ecc::H),
            other => Err(format!(
                "unknown error_correction '{other}' (use L, M, Q, or H)"
            )),
        }
    }
    fn level(self) -> EcLevel {
        match self {
            Ecc::L => EcLevel::L,
            Ecc::M => EcLevel::M,
            Ecc::Q => EcLevel::Q,
            Ecc::H => EcLevel::H,
        }
    }
}

/// Everything the renderer needs. Built by the block, the CLI and the page alike.
#[derive(Debug, Clone)]
pub struct Options<'a> {
    pub address: &'a str,
    pub scheme: &'a str,
    pub amount: &'a str,
    pub label: &'a str,
    pub message: &'a str,
    pub error_correction: &'a str,
    pub size: u32,
    pub foreground: &'a str,
    pub background: &'a str,
    pub show_uri: bool,
}

impl Default for Options<'_> {
    fn default() -> Self {
        Options {
            address: "",
            scheme: "bitcoin",
            amount: "",
            label: "",
            message: "",
            error_correction: "M",
            size: 512,
            foreground: "#000000",
            background: "#ffffff",
            show_uri: true,
        }
    }
}

pub const MIN_SIZE: u32 = 128;
pub const MAX_SIZE: u32 = 2048;
/// Longest payload we accept before the QR encoder would reject it anyway.
pub const MAX_PAYLOAD_CHARS: usize = 2000;

// ---------------------------------------------------------------------------
// Bech32 / Bech32m
// ---------------------------------------------------------------------------

const CHARSET: &[u8] = b"qpzry9x8gf2tvdw0s3jn54khce6mua7l";
const BECH32_CONST: u32 = 1;
const BECH32M_CONST: u32 = 0x2bc8_30a3;

fn bech32_polymod(values: &[u8]) -> u32 {
    const GEN: [u32; 5] = [
        0x3b6a_57b2,
        0x2650_8e6d,
        0x1ea1_19fa,
        0x3d42_33dd,
        0x2a14_62b3,
    ];
    let mut chk: u32 = 1;
    for v in values {
        let top = chk >> 25;
        chk = ((chk & 0x1ff_ffff) << 5) ^ u32::from(*v);
        for (i, g) in GEN.iter().enumerate() {
            if (top >> i) & 1 == 1 {
                chk ^= g;
            }
        }
    }
    chk
}

fn hrp_expand(hrp: &str) -> Vec<u8> {
    let mut v: Vec<u8> = hrp.bytes().map(|c| c >> 5).collect();
    v.push(0);
    v.extend(hrp.bytes().map(|c| c & 31));
    v
}

/// Decode a bech32 string into `(hrp, data-part-without-checksum, checksum constant)`.
/// `enforce_length` applies the 90-character cap from BIP-173 (BOLT-11 invoices
/// intentionally exceed it, so lightning passes `false`).
fn bech32_decode(s: &str, enforce_length: bool) -> Result<(String, Vec<u8>, u32), String> {
    if enforce_length && s.chars().count() > 90 {
        return Err("bech32 address is longer than the 90-character limit".into());
    }
    if s.len() < 8 {
        return Err("bech32 string is too short".into());
    }
    let has_lower = s.chars().any(|c| c.is_ascii_lowercase());
    let has_upper = s.chars().any(|c| c.is_ascii_uppercase());
    if has_lower && has_upper {
        return Err("bech32 strings must be all lowercase or all uppercase, not mixed".into());
    }
    if !s.is_ascii() {
        return Err("bech32 strings must be ASCII".into());
    }
    let lower = s.to_ascii_lowercase();
    let pos = lower
        .rfind('1')
        .ok_or_else(|| "missing the '1' separator".to_string())?;
    if pos == 0 {
        return Err("missing the human-readable prefix before '1'".into());
    }
    if pos + 7 > lower.len() {
        return Err("data part is too short after the '1' separator".into());
    }
    let hrp = &lower[..pos];
    if !hrp.bytes().all(|c| (33..=126).contains(&c)) {
        return Err("human-readable prefix contains an invalid character".into());
    }
    let mut data = Vec::with_capacity(lower.len() - pos - 1);
    for c in lower[pos + 1..].bytes() {
        let idx = CHARSET
            .iter()
            .position(|&x| x == c)
            .ok_or_else(|| format!("'{}' is not a bech32 character", c as char))?;
        data.push(idx as u8);
    }
    let mut vals = hrp_expand(hrp);
    vals.extend_from_slice(&data);
    let chk = bech32_polymod(&vals);
    data.truncate(data.len() - 6);
    Ok((hrp.to_string(), data, chk))
}

fn convert_bits(data: &[u8], from: u32, to: u32) -> Result<Vec<u8>, String> {
    let mut acc: u32 = 0;
    let mut bits: u32 = 0;
    let maxv = (1u32 << to) - 1;
    let mut out = Vec::new();
    for &v in data {
        if u32::from(v) >> from != 0 {
            return Err("invalid bech32 data value".into());
        }
        acc = (acc << from) | u32::from(v);
        bits += from;
        while bits >= to {
            bits -= to;
            out.push(((acc >> bits) & maxv) as u8);
        }
    }
    if bits >= from || ((acc << (to - bits)) & maxv) != 0 {
        return Err("invalid bech32 padding".into());
    }
    Ok(out)
}

/// Verify a SegWit (bech32/bech32m) address against the allowed human-readable prefixes.
fn check_segwit(addr: &str, hrps: &[&str], coin: &str) -> Result<(), String> {
    let (hrp, data, chk) = bech32_decode(addr, true)?;
    if !hrps.contains(&hrp.as_str()) {
        return Err(format!(
            "'{hrp}' is not a {coin} address prefix (expected {})",
            hrps.join(" or ")
        ));
    }
    if data.is_empty() {
        return Err("address has no witness version".into());
    }
    let witness_version = data[0];
    if witness_version > 16 {
        return Err(format!(
            "witness version {witness_version} is not valid (0-16)"
        ));
    }
    let program = convert_bits(&data[1..], 5, 8)?;
    if program.len() < 2 || program.len() > 40 {
        return Err(format!(
            "witness program is {} bytes (must be 2-40)",
            program.len()
        ));
    }
    if witness_version == 0 && program.len() != 20 && program.len() != 32 {
        return Err(format!(
            "version-0 witness program is {} bytes (must be 20 or 32)",
            program.len()
        ));
    }
    let expected = if witness_version == 0 {
        BECH32_CONST
    } else {
        BECH32M_CONST
    };
    if chk != expected {
        let scheme = if witness_version == 0 {
            "bech32"
        } else {
            "bech32m"
        };
        return Err(format!(
            "{scheme} checksum does not match — the address has a typo"
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Base58Check
// ---------------------------------------------------------------------------

/// Verify a Base58Check address and return its version byte.
fn check_base58(addr: &str, versions: &[(u8, &str)], coin: &str) -> Result<(), String> {
    let decoded = bs58::decode(addr)
        .with_check(None)
        .into_vec()
        .map_err(|_| format!("Base58Check checksum does not match — the {coin} address has a typo or an invalid character"))?;
    if decoded.len() != 21 {
        return Err(format!(
            "decoded address is {} bytes (expected 21: 1 version + 20 hash)",
            decoded.len()
        ));
    }
    if versions.iter().any(|(v, _)| *v == decoded[0]) {
        return Ok(());
    }
    Err(format!(
        "version byte 0x{:02x} is not a {coin} address ({coin} uses {})",
        decoded[0],
        versions
            .iter()
            .map(|(v, n)| format!("0x{v:02x} {n}"))
            .collect::<Vec<_>>()
            .join(", ")
    ))
}

// ---------------------------------------------------------------------------
// Address validation per scheme
// ---------------------------------------------------------------------------

fn validate_address(scheme: Scheme, addr: &str) -> Result<(), String> {
    match scheme {
        Scheme::Bitcoin => {
            let lower = addr.to_ascii_lowercase();
            if lower.starts_with("bc1") || lower.starts_with("tb1") || lower.starts_with("bcrt1") {
                check_segwit(addr, &["bc", "tb", "bcrt"], "Bitcoin")
            } else {
                check_base58(
                    addr,
                    &[
                        (0x00, "mainnet P2PKH '1…'"),
                        (0x05, "mainnet P2SH '3…'"),
                        (0x6f, "testnet P2PKH 'm/n…'"),
                        (0xc4, "testnet P2SH '2…'"),
                    ],
                    "Bitcoin",
                )
            }
        }
        Scheme::Litecoin => {
            let lower = addr.to_ascii_lowercase();
            if lower.starts_with("ltc1") || lower.starts_with("tltc1") || lower.starts_with("rltc1")
            {
                check_segwit(addr, &["ltc", "tltc", "rltc"], "Litecoin")
            } else {
                check_base58(
                    addr,
                    &[
                        (0x30, "mainnet P2PKH 'L…'"),
                        (0x32, "mainnet P2SH 'M…'"),
                        (0x05, "legacy P2SH '3…'"),
                        (0x6f, "testnet P2PKH 'm/n…'"),
                        (0x3a, "testnet P2SH 'Q…'"),
                    ],
                    "Litecoin",
                )
            }
        }
        Scheme::Dogecoin => check_base58(
            addr,
            &[
                (0x1e, "mainnet P2PKH 'D…'"),
                (0x16, "mainnet P2SH '9/A…'"),
                (0x71, "testnet P2PKH 'n…'"),
                (0xc4, "testnet P2SH '2…'"),
            ],
            "Dogecoin",
        ),
        Scheme::Ethereum => {
            let hex = addr.strip_prefix("0x").or_else(|| addr.strip_prefix("0X"));
            let hex = hex.ok_or_else(|| {
                "Ethereum addresses start with '0x' followed by 40 hex characters".to_string()
            })?;
            if hex.len() != 40 {
                return Err(format!(
                    "Ethereum address has {} hex characters after '0x' (expected 40)",
                    hex.len()
                ));
            }
            if let Some(bad) = hex.chars().find(|c| !c.is_ascii_hexdigit()) {
                return Err(format!("'{bad}' is not a hex character"));
            }
            Ok(())
        }
        Scheme::Lightning => {
            let lower = addr.to_ascii_lowercase();
            const PREFIXES: [&str; 5] = ["lnbc", "lntb", "lntbs", "lnbcrt", "lnsb"];
            if !PREFIXES.iter().any(|p| lower.starts_with(p)) {
                return Err(
                    "a BOLT-11 invoice starts with lnbc (mainnet), lntb/lntbs (testnet/signet), \
                     lnsb or lnbcrt — paste the invoice, not a node id"
                        .into(),
                );
            }
            // BOLT-11 is bech32 with no 90-character cap.
            let (_, _, chk) = bech32_decode(addr, false)?;
            if chk != BECH32_CONST {
                return Err(
                    "bech32 checksum does not match — the invoice was truncated or mistyped".into(),
                );
            }
            Ok(())
        }
        Scheme::Text => Ok(()),
    }
}

// ---------------------------------------------------------------------------
// Amount handling
// ---------------------------------------------------------------------------

/// Split a plain decimal string into `(integer_digits, fraction_digits)`, both
/// already stripped of insignificant zeros where safe.
fn parse_decimal(amount: &str, max_dp: usize, unit: &str) -> Result<(String, String), String> {
    let a = amount.trim();
    if a.is_empty() {
        return Err("amount is empty".into());
    }
    if a.contains(',') {
        return Err(format!(
            "amount '{a}' uses a comma — write the amount in {unit} with a period, e.g. 0.25"
        ));
    }
    if a.contains('e') || a.contains('E') {
        return Err(format!(
            "amount '{a}' uses exponent notation — write it out in full, e.g. 0.0001"
        ));
    }
    if a.starts_with('-') {
        return Err("amount must be positive".into());
    }
    let (int_part, frac_part) = match a.split_once('.') {
        Some((i, f)) => {
            if f.contains('.') {
                return Err(format!("amount '{a}' has more than one decimal point"));
            }
            (i, f)
        }
        None => (a, ""),
    };
    if int_part.is_empty() && frac_part.is_empty() {
        return Err(format!("amount '{a}' has no digits"));
    }
    for c in int_part.chars().chain(frac_part.chars()) {
        if !c.is_ascii_digit() {
            return Err(format!(
                "amount '{a}' contains '{c}' — only digits and one period are allowed"
            ));
        }
    }
    if frac_part.len() > max_dp {
        return Err(format!(
            "amount '{a}' has {} decimal places — {unit} supports at most {max_dp}",
            frac_part.len()
        ));
    }
    let int_trimmed = int_part.trim_start_matches('0');
    let frac_trimmed = frac_part.trim_end_matches('0');
    if int_trimmed.is_empty() && frac_trimmed.is_empty() {
        return Err("amount must be greater than 0".into());
    }
    Ok((int_trimmed.to_string(), frac_trimmed.to_string()))
}

/// BIP-21 amount: decimal coin units, period separator, no exponent.
fn bip21_amount(scheme: Scheme, amount: &str) -> Result<String, String> {
    let (unit, max_dp) = match scheme {
        Scheme::Bitcoin => ("BTC", 8),
        Scheme::Litecoin => ("LTC", 8),
        Scheme::Dogecoin => ("DOGE", 8),
        _ => ("coin units", 8),
    };
    let (int_part, frac_part) = parse_decimal(amount, max_dp, unit)?;
    if int_part.len() > 12 {
        return Err(format!("amount '{}' is implausibly large", amount.trim()));
    }
    if scheme == Scheme::Bitcoin {
        let whole: u64 = int_part.parse().unwrap_or(0);
        if whole > 21_000_000 || (whole == 21_000_000 && !frac_part.is_empty()) {
            return Err("amount exceeds the 21,000,000 BTC supply cap".into());
        }
    }
    let int_out = if int_part.is_empty() { "0" } else { &int_part };
    Ok(if frac_part.is_empty() {
        int_out.to_string()
    } else {
        format!("{int_out}.{frac_part}")
    })
}

/// EIP-681 `value`: an integer count of wei (1 ETH = 10^18 wei).
fn eth_wei(amount: &str) -> Result<String, String> {
    let (int_part, frac_part) = parse_decimal(amount, 18, "ETH")?;
    if int_part.len() > 12 {
        return Err(format!("amount '{}' is implausibly large", amount.trim()));
    }
    let mut wei = String::with_capacity(int_part.len() + 18);
    wei.push_str(&int_part);
    wei.push_str(&frac_part);
    for _ in frac_part.len()..18 {
        wei.push('0');
    }
    let trimmed = wei.trim_start_matches('0');
    Ok(if trimmed.is_empty() {
        "0".to_string()
    } else {
        trimmed.to_string()
    })
}

// ---------------------------------------------------------------------------
// URI building
// ---------------------------------------------------------------------------

/// Percent-encode per RFC 3986: keep unreserved characters, UTF-8 + `%XX` the rest.
/// Space becomes `%20`, never `+` (that is form encoding and wallets show it literally).
fn pct_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.as_bytes() {
        let c = *b as char;
        if c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_' | '~') {
            out.push(c);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// Build the payment URI without rendering anything.
pub fn build_uri(opts: &Options) -> Result<String, String> {
    let scheme = Scheme::parse(opts.scheme)?;
    let address = opts.address.trim();
    if address.is_empty() {
        return Err(match scheme {
            Scheme::Lightning => "invoice is empty — paste a BOLT-11 invoice".into(),
            Scheme::Text => "text is empty — enter the payload to encode".into(),
            _ => format!("address is empty — enter a {} address", scheme.coin()),
        });
    }
    let max_addr = match scheme {
        Scheme::Lightning | Scheme::Text => MAX_PAYLOAD_CHARS,
        _ => 120,
    };
    if address.chars().count() > max_addr {
        return Err(format!(
            "input is {} characters — the limit is {max_addr}",
            address.chars().count()
        ));
    }
    validate_address(scheme, address)?;

    let amount = opts.amount.trim();
    let label = opts.label.trim();
    let message = opts.message.trim();

    if scheme.is_bip21() {
        let mut params: Vec<String> = Vec::new();
        if !amount.is_empty() {
            params.push(format!("amount={}", bip21_amount(scheme, amount)?));
        }
        if !label.is_empty() {
            params.push(format!("label={}", pct_encode(label)));
        }
        if !message.is_empty() {
            params.push(format!("message={}", pct_encode(message)));
        }
        let mut uri = format!("{}:{address}", scheme.prefix());
        if !params.is_empty() {
            uri.push('?');
            uri.push_str(&params.join("&"));
        }
        return Ok(uri);
    }

    if scheme == Scheme::Ethereum {
        if !label.is_empty() || !message.is_empty() {
            return Err(
                "EIP-681 Ethereum URIs have no label or message field — clear those, or switch \
                 the scheme to bitcoin, litecoin or dogecoin (BIP-21) which do"
                    .into(),
            );
        }
        let mut uri = format!("ethereum:{address}");
        if !amount.is_empty() {
            uri.push_str(&format!("?value={}", eth_wei(amount)?));
        }
        return Ok(uri);
    }

    // Lightning + generic text carry their payload verbatim.
    if !amount.is_empty() || !label.is_empty() || !message.is_empty() {
        return Err(match scheme {
            Scheme::Lightning => "a BOLT-11 invoice already carries its own amount and \
                                  description — clear amount, label and message"
                .into(),
            _ => "the text scheme encodes your payload verbatim — clear amount, label and message"
                .to_string(),
        });
    }
    Ok(if scheme == Scheme::Text {
        address.to_string()
    } else {
        format!("{}:{address}", scheme.prefix())
    })
}

// ---------------------------------------------------------------------------
// SVG rendering
// ---------------------------------------------------------------------------

/// Accept `#rgb`, `#rrggbb`, `#rrggbbaa`, `transparent` and CSS colour keywords.
/// Rejects anything that could break out of the SVG attribute.
fn check_color(value: &str, field: &str) -> Result<String, String> {
    let v = value.trim();
    if v.is_empty() {
        return Err(format!("{field} is empty"));
    }
    if v.len() > 32 {
        return Err(format!("{field} '{v}' is too long to be a colour"));
    }
    if let Some(hex) = v.strip_prefix('#') {
        if !matches!(hex.len(), 3 | 4 | 6 | 8) || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(format!(
                "{field} '{v}' is not a valid hex colour (use #rgb, #rrggbb or #rrggbbaa)"
            ));
        }
        return Ok(v.to_string());
    }
    if v.chars().all(|c| c.is_ascii_alphabetic()) {
        return Ok(v.to_ascii_lowercase());
    }
    Err(format!(
        "{field} '{v}' is not a colour — use a hex value like #1a1a1a or a CSS colour name"
    ))
}

fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

fn wrap_chars(s: &str, width: usize) -> Vec<String> {
    let width = width.max(8);
    let mut lines = Vec::new();
    let mut cur = String::new();
    let mut n = 0usize;
    for c in s.chars() {
        cur.push(c);
        n += 1;
        if n == width {
            lines.push(std::mem::take(&mut cur));
            n = 0;
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    if lines.is_empty() {
        lines.push(String::new());
    }
    lines
}

/// Quiet zone in modules, per the QR spec.
const QUIET_ZONE: usize = 4;

/// Build the payment URI and render it as an SVG QR code.
/// Returns `(uri, svg)`.
pub fn render(opts: &Options) -> Result<(String, String), String> {
    let uri = build_uri(opts)?;
    let ecc = Ecc::parse(opts.error_correction)?;
    let fg = check_color(opts.foreground, "foreground")?;
    let bg = check_color(opts.background, "background")?;
    if opts.size < MIN_SIZE || opts.size > MAX_SIZE {
        return Err(format!(
            "size {} is out of range ({MIN_SIZE}-{MAX_SIZE} pixels)",
            opts.size
        ));
    }

    let code = QrCode::with_error_correction_level(uri.as_bytes(), ecc.level()).map_err(|e| {
        format!(
            "cannot encode {} characters at error correction {:?}: {e} — shorten the payload or \
             drop to a lower error-correction level",
            uri.chars().count(),
            ecc
        )
    })?;
    let width = code.width();
    let colors = code.to_colors();
    let span = width + QUIET_ZONE * 2;

    // One compact path: horizontal runs of dark modules.
    let mut path = String::new();
    for y in 0..width {
        let mut x = 0usize;
        while x < width {
            if colors[y * width + x] == qrcode::Color::Dark {
                let start = x;
                while x < width && colors[y * width + x] == qrcode::Color::Dark {
                    x += 1;
                }
                let run = x - start;
                path.push_str(&format!(
                    "M{} {}h{run}v1h-{run}z",
                    start + QUIET_ZONE,
                    y + QUIET_ZONE
                ));
            } else {
                x += 1;
            }
        }
    }

    // Caption block, measured in module units (monospace ≈ 0.6em per character).
    let font_size = 1.0f64;
    let line_height = 1.5f64;
    let caption = if opts.show_uri {
        wrap_chars(&uri, ((span as f64 - 2.0) / (font_size * 0.6)) as usize)
    } else {
        Vec::new()
    };
    let caption_height = if caption.is_empty() {
        0.0
    } else {
        1.0 + caption.len() as f64 * line_height + 1.0
    };
    let total_height = span as f64 + caption_height;
    let px_width = opts.size;
    let px_height = ((opts.size as f64) * total_height / span as f64).round() as u32;

    let mut svg = String::with_capacity(path.len() + 1024);
    svg.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{px_width}\" height=\"{px_height}\" \
         viewBox=\"0 0 {span} {total_height:.2}\" role=\"img\" shape-rendering=\"crispEdges\">"
    ));
    svg.push_str(&format!("<title>{}</title>", xml_escape(&uri)));
    svg.push_str(&format!(
        "<rect width=\"{span}\" height=\"{total_height:.2}\" fill=\"{bg}\"/>"
    ));
    svg.push_str(&format!("<path fill=\"{fg}\" d=\"{path}\"/>"));
    if !caption.is_empty() {
        svg.push_str(&format!(
            "<g data-role=\"uri-caption\" fill=\"{fg}\" font-family=\"ui-monospace, \
             SFMono-Regular, Menlo, Consolas, monospace\" font-size=\"{font_size}\" \
             text-anchor=\"middle\">"
        ));
        for (i, line) in caption.iter().enumerate() {
            let y = span as f64 + 1.0 + (i as f64 + 1.0) * line_height;
            svg.push_str(&format!(
                "<text x=\"{:.2}\" y=\"{y:.2}\">{}</text>",
                span as f64 / 2.0,
                xml_escape(line)
            ));
        }
        svg.push_str("</g>");
    }
    svg.push_str("</svg>");
    Ok((uri, svg))
}

/// Convenience wrapper for the page/web export: SVG only.
pub fn run(opts: &Options) -> Result<String, String> {
    render(opts).map(|(_, svg)| svg)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Real BIP-173 test vector (mainnet P2WPKH).
    const BC1_P2WPKH: &str = "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4";
    /// Genesis-block payout address (mainnet P2PKH).
    const P2PKH: &str = "1A1zP1eP5QGefi2DMPTfTL5SLmv7DivfNa";

    /// Test-only bech32 encoder, so we can mint valid non-Bitcoin bech32 strings
    /// (Litecoin addresses, BOLT-11-shaped invoices) without hard-coding vectors.
    fn bech32_encode(hrp: &str, data: &[u8], constant: u32) -> String {
        let mut vals = hrp_expand(hrp);
        vals.extend_from_slice(data);
        vals.extend_from_slice(&[0, 0, 0, 0, 0, 0]);
        let polymod = bech32_polymod(&vals) ^ constant;
        let mut out = String::from(hrp);
        out.push('1');
        for v in data {
            out.push(CHARSET[*v as usize] as char);
        }
        for i in 0..6 {
            out.push(CHARSET[((polymod >> (5 * (5 - i))) & 31) as usize] as char);
        }
        out
    }

    fn opts<'a>(address: &'a str, scheme: &'a str) -> Options<'a> {
        Options {
            address,
            scheme,
            ..Default::default()
        }
    }

    // --- URI building: happy paths -----------------------------------------

    #[test]
    fn bare_bitcoin_address_becomes_a_bip21_uri() {
        let uri = build_uri(&opts(P2PKH, "bitcoin")).unwrap();
        assert_eq!(uri, format!("bitcoin:{P2PKH}"));
    }

    #[test]
    fn amount_label_and_message_are_appended_in_bip21_order() {
        let uri = build_uri(&Options {
            address: BC1_P2WPKH,
            scheme: "bitcoin",
            amount: "0.02500000",
            label: "Coffee Bar",
            message: "Table 4 / oat latte",
            ..Default::default()
        })
        .unwrap();
        assert_eq!(
            uri,
            format!(
                "bitcoin:{BC1_P2WPKH}?amount=0.025&label=Coffee%20Bar&message=Table%204%20%2F%20oat%20latte"
            )
        );
    }

    #[test]
    fn label_is_utf8_percent_encoded_and_space_never_becomes_plus() {
        let uri = build_uri(&Options {
            address: P2PKH,
            scheme: "bitcoin",
            label: "Café Ölsen",
            ..Default::default()
        })
        .unwrap();
        assert_eq!(uri, format!("bitcoin:{P2PKH}?label=Caf%C3%A9%20%C3%96lsen"));
        assert!(!uri.contains('+'));
    }

    #[test]
    fn amount_keeps_a_leading_zero_and_drops_trailing_zeros() {
        for (input, want) in [
            (".5", "0.5"),
            ("0.50", "0.5"),
            ("1.00000000", "1"),
            ("007", "7"),
            ("21000000", "21000000"),
        ] {
            let uri = build_uri(&Options {
                address: P2PKH,
                scheme: "bitcoin",
                amount: input,
                ..Default::default()
            })
            .unwrap();
            assert_eq!(
                uri,
                format!("bitcoin:{P2PKH}?amount={want}"),
                "input {input}"
            );
        }
    }

    #[test]
    fn ethereum_amount_converts_to_exact_wei() {
        for (eth, wei) in [
            ("1", "1000000000000000000"),
            ("0.1", "100000000000000000"),
            ("0.000000000000000001", "1"),
            ("2.5", "2500000000000000000"),
        ] {
            let uri = build_uri(&Options {
                address: "0x71C7656EC7ab88b098defB751B7401B5f6d8976F",
                scheme: "ethereum",
                amount: eth,
                ..Default::default()
            })
            .unwrap();
            assert_eq!(
                uri,
                format!("ethereum:0x71C7656EC7ab88b098defB751B7401B5f6d8976F?value={wei}"),
                "eth {eth}"
            );
        }
    }

    #[test]
    fn segwit_bech32m_taproot_and_testnet_addresses_are_accepted() {
        // v1 (taproot) program, minted with the bech32m constant.
        let mut data = vec![1u8];
        data.extend(convert_bits_up(&[0xab; 32]));
        let taproot = bech32_encode("bc", &data, BECH32M_CONST);
        build_uri(&opts(&taproot, "bitcoin")).unwrap();

        let mut tb = vec![0u8];
        tb.extend(convert_bits_up(&[0x11; 20]));
        let testnet = bech32_encode("tb", &tb, BECH32_CONST);
        build_uri(&opts(&testnet, "bitcoin")).unwrap();
    }

    /// 8 → 5 bit regrouping with padding, for minting test addresses.
    fn convert_bits_up(bytes: &[u8]) -> Vec<u8> {
        let mut acc = 0u32;
        let mut bits = 0u32;
        let mut out = Vec::new();
        for &b in bytes {
            acc = (acc << 8) | u32::from(b);
            bits += 8;
            while bits >= 5 {
                bits -= 5;
                out.push(((acc >> bits) & 31) as u8);
            }
        }
        if bits > 0 {
            out.push(((acc << (5 - bits)) & 31) as u8);
        }
        out
    }

    #[test]
    fn litecoin_and_dogecoin_addresses_validate_by_version_byte() {
        let ltc = bs58::encode([&[0x30u8][..], &[0x42u8; 20][..]].concat())
            .with_check()
            .into_string();
        assert!(ltc.starts_with('L'), "got {ltc}");
        build_uri(&opts(&ltc, "litecoin")).unwrap();

        let doge = bs58::encode([&[0x1eu8][..], &[0x42u8; 20][..]].concat())
            .with_check()
            .into_string();
        assert!(doge.starts_with('D'), "got {doge}");
        build_uri(&opts(&doge, "dogecoin")).unwrap();

        let mut d = vec![0u8];
        d.extend(convert_bits_up(&[0x42; 20]));
        let ltc_segwit = bech32_encode("ltc", &d, BECH32_CONST);
        build_uri(&opts(&ltc_segwit, "litecoin")).unwrap();
    }

    #[test]
    fn lightning_invoice_becomes_a_lightning_uri() {
        let invoice = bech32_encode("lnbc250u", &convert_bits_up(&[0x7f; 60]), BECH32_CONST);
        let uri = build_uri(&opts(&invoice, "lightning")).unwrap();
        assert_eq!(uri, format!("lightning:{invoice}"));
    }

    #[test]
    fn text_scheme_passes_the_payload_through_verbatim() {
        let uri = build_uri(&opts("https://example.org/pay/42", "text")).unwrap();
        assert_eq!(uri, "https://example.org/pay/42");
    }

    // --- URI building: error paths -----------------------------------------

    #[test]
    fn a_one_character_typo_fails_the_checksum() {
        // Flip the last character of a real bech32 address.
        let mut typo = BC1_P2WPKH.to_string();
        typo.pop();
        typo.push('l');
        let err = build_uri(&opts(&typo, "bitcoin")).unwrap_err();
        assert!(err.contains("checksum"), "got {err}");

        let mut b58 = P2PKH.to_string();
        b58.pop();
        b58.push('X');
        let err = build_uri(&opts(&b58, "bitcoin")).unwrap_err();
        assert!(err.contains("checksum"), "got {err}");
    }

    #[test]
    fn a_bitcoin_address_is_rejected_for_the_litecoin_scheme() {
        let err = build_uri(&opts(P2PKH, "litecoin")).unwrap_err();
        assert!(err.contains("version byte 0x00"), "got {err}");
        let err = build_uri(&opts(BC1_P2WPKH, "litecoin")).unwrap_err();
        assert!(err.contains("Base58Check"), "got {err}");
    }

    #[test]
    fn empty_address_reports_the_right_field() {
        assert!(build_uri(&opts("", "bitcoin"))
            .unwrap_err()
            .contains("address is empty"));
        assert!(build_uri(&opts("  ", "lightning"))
            .unwrap_err()
            .contains("invoice is empty"));
        assert!(build_uri(&opts("", "text"))
            .unwrap_err()
            .contains("text is empty"));
    }

    #[test]
    fn comma_exponent_and_over_precise_amounts_are_rejected() {
        let cases = [
            ("0,25", "comma"),
            ("1e-3", "exponent"),
            ("0.000000001", "decimal places"),
            ("0", "greater than 0"),
            ("-1", "positive"),
            ("21000000.1", "supply cap"),
            ("1.2.3", "decimal point"),
        ];
        for (amount, needle) in cases {
            let err = build_uri(&Options {
                address: P2PKH,
                scheme: "bitcoin",
                amount,
                ..Default::default()
            })
            .unwrap_err();
            assert!(err.contains(needle), "amount {amount} → {err}");
        }
    }

    #[test]
    fn ethereum_rejects_a_bad_address_and_label_message() {
        let err = build_uri(&opts(
            "71C7656EC7ab88b098defB751B7401B5f6d8976F",
            "ethereum",
        ))
        .unwrap_err();
        assert!(err.contains("0x"), "got {err}");
        let err = build_uri(&opts("0xdeadbeef", "ethereum")).unwrap_err();
        assert!(err.contains("40"), "got {err}");
        let err = build_uri(&Options {
            address: "0x71C7656EC7ab88b098defB751B7401B5f6d8976F",
            scheme: "ethereum",
            label: "Tip",
            ..Default::default()
        })
        .unwrap_err();
        assert!(err.contains("EIP-681"), "got {err}");
    }

    #[test]
    fn lightning_rejects_a_node_id_and_extra_params() {
        let err = build_uri(&opts("03a1b2c3", "lightning")).unwrap_err();
        assert!(err.contains("BOLT-11"), "got {err}");
        let invoice = bech32_encode("lnbc250u", &convert_bits_up(&[0x7f; 40]), BECH32_CONST);
        let err = build_uri(&Options {
            address: &invoice,
            scheme: "lightning",
            amount: "0.1",
            ..Default::default()
        })
        .unwrap_err();
        assert!(err.contains("own amount"), "got {err}");
    }

    #[test]
    fn unknown_scheme_and_ecc_are_reported() {
        assert!(build_uri(&opts(P2PKH, "monero"))
            .unwrap_err()
            .contains("unknown scheme 'monero'"));
        assert!(Ecc::parse("Z")
            .unwrap_err()
            .contains("unknown error_correction"));
    }

    // --- rendering ----------------------------------------------------------

    #[test]
    fn render_emits_an_svg_with_the_uri_in_title_and_caption() {
        let (uri, svg) = render(&Options {
            address: BC1_P2WPKH,
            scheme: "bitcoin",
            amount: "0.001",
            label: "Tip jar",
            ..Default::default()
        })
        .unwrap();
        assert_eq!(
            uri,
            format!("bitcoin:{BC1_P2WPKH}?amount=0.001&label=Tip%20jar")
        );
        assert!(svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\""));
        assert!(svg.ends_with("</svg>"));
        assert!(svg.contains(&format!("<title>{}</title>", xml_escape(&uri))));
        assert!(svg.contains("data-role=\"uri-caption\""));
        assert!(svg.contains("width=\"512\""));
        assert!(svg.contains("fill=\"#000000\""));
        assert!(svg.contains("fill=\"#ffffff\""));
    }

    #[test]
    fn show_uri_false_drops_the_caption_and_squares_the_image() {
        let (_, svg) = render(&Options {
            address: P2PKH,
            scheme: "bitcoin",
            show_uri: false,
            size: 300,
            ..Default::default()
        })
        .unwrap();
        assert!(!svg.contains("uri-caption"));
        assert!(
            svg.contains("width=\"300\" height=\"300\""),
            "got {svg:.200}"
        );
        // Still discoverable by screen readers.
        assert!(svg.contains("<title>bitcoin:"));
    }

    #[test]
    fn ampersands_in_the_uri_are_xml_escaped() {
        let (_, svg) = render(&Options {
            address: P2PKH,
            scheme: "bitcoin",
            amount: "0.5",
            label: "A",
            ..Default::default()
        })
        .unwrap();
        assert!(svg.contains("&amp;"));
        assert!(!svg.contains("label=A&label"));
    }

    #[test]
    fn custom_colours_are_applied_and_bad_ones_rejected() {
        let (_, svg) = render(&Options {
            address: P2PKH,
            scheme: "bitcoin",
            foreground: "#1a3fd0",
            background: "transparent",
            ..Default::default()
        })
        .unwrap();
        assert!(svg.contains("fill=\"#1a3fd0\""));
        assert!(svg.contains("fill=\"transparent\""));

        let err = render(&Options {
            address: P2PKH,
            scheme: "bitcoin",
            foreground: "\"><script>",
            ..Default::default()
        })
        .unwrap_err();
        assert!(err.contains("foreground"), "got {err}");
    }

    #[test]
    fn size_out_of_range_is_rejected_at_both_ends() {
        for size in [MIN_SIZE - 1, MAX_SIZE + 1] {
            let err = render(&Options {
                address: P2PKH,
                scheme: "bitcoin",
                size,
                ..Default::default()
            })
            .unwrap_err();
            assert!(err.contains("out of range"), "size {size} → {err}");
        }
        for size in [MIN_SIZE, MAX_SIZE] {
            render(&Options {
                address: P2PKH,
                scheme: "bitcoin",
                size,
                ..Default::default()
            })
            .unwrap();
        }
    }

    #[test]
    fn an_over_long_text_payload_is_rejected_before_the_encoder_panics() {
        let long = "x".repeat(MAX_PAYLOAD_CHARS + 1);
        let err = build_uri(&opts(&long, "text")).unwrap_err();
        assert!(err.contains("limit is 2000"), "got {err}");

        // At the cap the encoder itself decides; H is too dense for 2000 bytes.
        let at_cap = "x".repeat(MAX_PAYLOAD_CHARS);
        let err = render(&Options {
            address: &at_cap,
            scheme: "text",
            error_correction: "H",
            ..Default::default()
        })
        .unwrap_err();
        assert!(err.contains("cannot encode"), "got {err}");
    }

    #[test]
    fn every_error_correction_level_renders() {
        for level in ["L", "M", "Q", "H"] {
            let (_, svg) = render(&Options {
                address: P2PKH,
                scheme: "bitcoin",
                error_correction: level,
                ..Default::default()
            })
            .unwrap();
            assert!(svg.contains("<path fill="), "level {level}");
        }
    }
}
