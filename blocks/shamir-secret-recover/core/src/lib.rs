//! shamir-secret-recover core — pure compute, shared by the chat skill block and the web page.
//! No wafer/wasm-bindgen deps, no I/O: shares arrive as text and the recovered secret
//! is returned as text. Nothing is read from disk and nothing is sent anywhere.
//!
//! The scheme is byte-wise Shamir over GF(256): each byte of the secret is the
//! constant term of its own degree `k-1` polynomial, and a share is that polynomial
//! evaluated at a nonzero x coordinate. Recovery is Lagrange interpolation at x = 0.
//! Two reduction polynomials are in real-world use (`0x11b`, the AES one, and
//! `0x11d`), and picking the wrong one yields a plausible-looking but wrong secret —
//! so `field_poly = auto` resolves it from the shares themselves whenever there is a
//! spare share to cross-check against.

use std::collections::BTreeMap;

/// GF(256) x coordinates are 1..=255, so no share set can be larger.
pub const MAX_SHARES: usize = 255;
/// Per-share payload cap. A Shamir share is the same length as the secret.
pub const MAX_SHARE_BYTES: usize = 65_536;
/// Rough multiplication budget for the interpolation work (`shares × threshold × bytes`).
/// Keeps a 255-share × 64 KiB paste from wedging the browser tab.
const MAX_WORK: usize = 120_000_000;
/// Above this share count the "which share disagrees" search is skipped (it is
/// quadratic in the share count); verification still reports pass/fail.
const CULPRIT_SEARCH_MAX_SHARES: usize = 64;

// ---------------------------------------------------------------------------
// GF(256)
// ---------------------------------------------------------------------------

/// Log/exp tables for one reduction polynomial, so multiply and divide are table
/// lookups rather than eight-round shift-and-xor loops.
struct Gf {
    exp: Vec<u8>,
    log: Vec<u8>,
    poly: u16,
}

/// Shift-and-xor multiply — used only to build the tables.
fn mul_slow(mut a: u8, mut b: u8, poly_low: u8) -> u8 {
    let mut p = 0u8;
    for _ in 0..8 {
        if b & 1 != 0 {
            p ^= a;
        }
        let hi = a & 0x80;
        a <<= 1;
        if hi != 0 {
            a ^= poly_low;
        }
        b >>= 1;
    }
    p
}

impl Gf {
    fn new(poly: u16) -> Gf {
        let low = (poly & 0xff) as u8;
        let g = (2u16..256)
            .map(|g| g as u8)
            .find(|&g| {
                // A generator walks all 255 nonzero elements before returning to 1.
                let mut x = g;
                let mut order = 1usize;
                while x != 1 && order <= 255 {
                    x = mul_slow(x, g, low);
                    order += 1;
                }
                order == 255
            })
            .unwrap_or(3);
        let mut exp = vec![0u8; 512];
        let mut log = vec![0u8; 256];
        let mut x = 1u8;
        for i in 0..255 {
            exp[i] = x;
            log[x as usize] = i as u8;
            x = mul_slow(x, g, low);
        }
        for i in 255..512 {
            exp[i] = exp[i - 255];
        }
        Gf { exp, log, poly }
    }

    fn mul(&self, a: u8, b: u8) -> u8 {
        if a == 0 || b == 0 {
            0
        } else {
            self.exp[self.log[a as usize] as usize + self.log[b as usize] as usize]
        }
    }

    /// `b` must be nonzero — every caller divides by a difference of distinct x values.
    fn div(&self, a: u8, b: u8) -> u8 {
        if a == 0 {
            0
        } else {
            self.exp[self.log[a as usize] as usize + 255 - self.log[b as usize] as usize]
        }
    }

    fn name(&self) -> &'static str {
        if self.poly == 0x11b {
            "0x11b"
        } else {
            "0x11d"
        }
    }
}

// ---------------------------------------------------------------------------
// Shares
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Format {
    IndexPrefix,
    Leading,
    Trailing,
}

impl Format {
    fn name(self) -> &'static str {
        match self {
            Format::IndexPrefix => "index-prefix",
            Format::Leading => "leading-index",
            Format::Trailing => "trailing-index",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Encoding {
    Hex,
    Base64,
}

impl Encoding {
    fn name(self) -> &'static str {
        match self {
            Encoding::Hex => "hex",
            Encoding::Base64 => "base64",
        }
    }
}

struct Share {
    x: u8,
    y: Vec<u8>,
    line: usize,
}

/// One non-blank, non-comment input line, stripped of decoration.
struct Cleaned {
    line: usize,
    token: String,
    prefixed: bool,
}

fn clean_lines(input: &str) -> Vec<Cleaned> {
    let mut out = Vec::new();
    for (i, raw) in input.lines().enumerate() {
        // `#` never appears in hex or base64, so an inline comment is safe to cut.
        let mut t = match raw.find('#') {
            Some(p) => &raw[..p],
            None => raw,
        }
        .trim()
        .to_string();

        // Shares get pasted wrapped in brackets, quotes, or with a trailing
        // list separator; peel all of that off before looking at the payload.
        loop {
            let before = t.len();
            t = t.trim().to_string();
            if t.len() >= 2 {
                let (f, l) = (t.chars().next().unwrap(), t.chars().last().unwrap());
                if (f == '[' && l == ']')
                    || (f == '(' && l == ')')
                    || (f == '"' && l == '"')
                    || (f == '\'' && l == '\'')
                {
                    t = t[f.len_utf8()..t.len() - l.len_utf8()].to_string();
                }
            }
            while t.ends_with(',') || t.ends_with(';') {
                t.pop();
            }
            if t.len() == before {
                break;
            }
        }
        t = t.trim().to_string();
        if t.is_empty() {
            continue;
        }

        let prefixed = t.len() > 4 && t[..4].eq_ignore_ascii_case("sss:");
        if prefixed {
            t = t[4..].trim().to_string();
        }
        out.push(Cleaned {
            line: i + 1,
            token: t,
            prefixed,
        });
    }
    out
}

/// `12-a1b2…` / `12:a1b2…` — a decimal x coordinate, a separator, then the payload.
fn split_index_prefix(token: &str) -> Option<(u8, &str)> {
    let pos = token.find(['-', ':', ',', ';'])?;
    let (head, tail) = token.split_at(pos);
    let payload = tail[1..].trim();
    let head = head.trim();
    if head.is_empty() || payload.is_empty() {
        return None;
    }
    let x: u32 = head.parse().ok()?;
    if x == 0 || x > 255 {
        return None;
    }
    Some((x as u8, payload))
}

fn looks_hex(s: &str) -> bool {
    let s = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s);
    !s.is_empty() && s.len() % 2 == 0 && s.bytes().all(|b| b.is_ascii_hexdigit())
}

fn decode_hex(s: &str) -> Result<Vec<u8>, String> {
    let s = s
        .strip_prefix("0x")
        .or_else(|| s.strip_prefix("0X"))
        .unwrap_or(s);
    let s: String = s
        .chars()
        .filter(|c| !c.is_whitespace() && *c != '_')
        .collect();
    if s.is_empty() {
        return Err("empty hex payload".into());
    }
    if s.len() % 2 != 0 {
        return Err(format!(
            "hex payload has an odd number of digits ({})",
            s.len()
        ));
    }
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(s.len() / 2);
    for pair in bytes.chunks(2) {
        let hi = (pair[0] as char)
            .to_digit(16)
            .ok_or_else(|| format!("`{}` is not a hex digit", pair[0] as char))?;
        let lo = (pair[1] as char)
            .to_digit(16)
            .ok_or_else(|| format!("`{}` is not a hex digit", pair[1] as char))?;
        out.push(((hi << 4) | lo) as u8);
    }
    Ok(out)
}

fn b64_value(c: u8) -> Option<u8> {
    match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'+' | b'-' => Some(62),
        b'/' | b'_' => Some(63),
        _ => None,
    }
}

/// Standard and URL-safe base64, with or without `=` padding.
fn decode_base64(s: &str) -> Result<Vec<u8>, String> {
    let chars: Vec<u8> = s
        .bytes()
        .filter(|b| !b.is_ascii_whitespace() && *b != b'=')
        .collect();
    if chars.is_empty() {
        return Err("empty base64 payload".into());
    }
    if chars.len() % 4 == 1 {
        return Err("base64 payload has a leftover character".into());
    }
    let mut out = Vec::with_capacity(chars.len() * 3 / 4);
    let mut acc: u32 = 0;
    let mut bits = 0u32;
    for c in chars {
        let v = b64_value(c).ok_or_else(|| format!("`{}` is not a base64 character", c as char))?
            as u32;
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(((acc >> bits) & 0xff) as u8);
        }
    }
    Ok(out)
}

fn decode(s: &str, enc: Encoding) -> Result<Vec<u8>, String> {
    match enc {
        Encoding::Hex => decode_hex(s),
        Encoding::Base64 => decode_base64(s),
    }
}

/// Build the share list under one assumed format, resolving the encoding as we go.
fn parse_shares(
    cleaned: &[Cleaned],
    fmt: Format,
    enc_pref: Option<Encoding>,
) -> Result<(Vec<Share>, Encoding), String> {
    // Payload per line — for index-prefix that is the part after the separator.
    let mut payloads: Vec<(usize, u8, &str)> = Vec::with_capacity(cleaned.len());
    for c in cleaned {
        match fmt {
            Format::IndexPrefix => {
                let (x, payload) = split_index_prefix(&c.token).ok_or_else(|| {
                    format!(
                        "line {}: expected an index-prefixed share such as `1-a1b2c3`, got `{}`",
                        c.line,
                        truncate(&c.token, 32)
                    )
                })?;
                payloads.push((c.line, x, payload));
            }
            _ => payloads.push((c.line, 0, c.token.as_str())),
        }
    }

    // `auto` prefers hex: an all-hex payload is also legal base64, and hex is what
    // the index-prefixed splitters emit.
    let enc = enc_pref.unwrap_or_else(|| {
        if payloads.iter().all(|(_, _, p)| looks_hex(p)) {
            Encoding::Hex
        } else {
            Encoding::Base64
        }
    });

    let mut shares = Vec::with_capacity(payloads.len());
    for (line, x_pre, payload) in payloads {
        let bytes = decode(payload, enc)
            .map_err(|e| format!("line {line}: could not decode as {}: {e}", enc.name()))?;
        let (x, y) = match fmt {
            Format::IndexPrefix => (x_pre, bytes),
            Format::Leading => {
                if bytes.len() < 2 {
                    return Err(format!(
                        "line {line}: a leading-index share needs at least 2 bytes (an index byte plus the secret), got {}",
                        bytes.len()
                    ));
                }
                (bytes[0], bytes[1..].to_vec())
            }
            Format::Trailing => {
                if bytes.len() < 2 {
                    return Err(format!(
                        "line {line}: a trailing-index share needs at least 2 bytes (the secret plus an index byte), got {}",
                        bytes.len()
                    ));
                }
                (bytes[bytes.len() - 1], bytes[..bytes.len() - 1].to_vec())
            }
        };
        if x == 0 {
            return Err(format!(
                "line {line}: share index 0 is not a valid coordinate (x must be 1-255)"
            ));
        }
        if y.len() > MAX_SHARE_BYTES {
            return Err(format!(
                "line {line}: share payload is {} bytes (maximum {MAX_SHARE_BYTES})",
                y.len()
            ));
        }
        shares.push(Share { x, y, line });
    }

    // Every share must be a point on the same set of polynomials.
    let len = shares[0].y.len();
    for s in &shares {
        if s.y.len() != len {
            return Err(format!(
                "shares are not the same length: line {} carries {} byte(s) but line {} carries {} byte(s)",
                shares[0].line,
                len,
                s.line,
                s.y.len()
            ));
        }
    }
    let mut seen: BTreeMap<u8, usize> = BTreeMap::new();
    for s in &shares {
        if let Some(prev) = seen.insert(s.x, s.line) {
            return Err(format!(
                "duplicate share index x={} on lines {} and {} — every share must have a different index",
                s.x, prev, s.line
            ));
        }
    }
    Ok((shares, enc))
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let head: String = s.chars().take(n).collect();
        format!("{head}…")
    }
}

// ---------------------------------------------------------------------------
// Interpolation
// ---------------------------------------------------------------------------

/// Lagrange interpolation of the byte-wise polynomials at `at`. With `at = 0`
/// this is the secret; with `at = x_j` it predicts share `j`.
fn interpolate(subset: &[&Share], at: u8, gf: &Gf) -> Vec<u8> {
    let len = subset[0].y.len();
    let mut out = vec![0u8; len];
    for (i, si) in subset.iter().enumerate() {
        let mut num = 1u8;
        let mut den = 1u8;
        for (j, sj) in subset.iter().enumerate() {
            if i == j {
                continue;
            }
            num = gf.mul(num, at ^ sj.x);
            den = gf.mul(den, si.x ^ sj.x);
        }
        let c = gf.div(num, den);
        if c != 0 {
            for b in 0..len {
                out[b] ^= gf.mul(c, si.y[b]);
            }
        }
    }
    out
}

/// Do the first `k` shares predict every other share? This is the cross-check
/// that both detects a corrupted share and resolves `field_poly = auto`.
fn baseline_agrees(shares: &[Share], k: usize, gf: &Gf) -> (bool, Vec<u8>) {
    let subset: Vec<&Share> = shares[..k].iter().collect();
    let mut bad = Vec::new();
    for s in &shares[k..] {
        if interpolate(&subset, s.x, gf) != s.y {
            bad.push(s.x);
        }
    }
    (bad.is_empty(), bad)
}

struct Verification {
    status: &'static str,
    detail: String,
    disagreeing: Vec<u8>,
    cross_checked: usize,
}

fn verify_shares(shares: &[Share], k: usize, gf: &Gf) -> Verification {
    let n = shares.len();
    if n <= k {
        return Verification {
            status: "skipped",
            detail: format!(
                "no redundant shares — {n} share(s) supplied for a threshold of {k}, so there is nothing to cross-check against"
            ),
            disagreeing: Vec::new(),
            cross_checked: 0,
        };
    }
    let (ok, bad) = baseline_agrees(shares, k, gf);
    if ok {
        return Verification {
            status: "passed",
            detail: format!("all {n} shares lie on the same polynomial"),
            disagreeing: Vec::new(),
            cross_checked: n - k,
        };
    }

    // Something disagrees. If exactly one share can be dropped to make the rest
    // consistent, name it — that is the corrupted or foreign share. This needs
    // two spare shares: with exactly one, dropping any share leaves a set that
    // trivially "agrees" because there is nothing left to predict.
    if n <= CULPRIT_SEARCH_MAX_SHARES && n >= k + 2 {
        let mut culprits = Vec::new();
        for c in 0..n {
            let rest: Vec<&Share> = shares
                .iter()
                .enumerate()
                .filter(|(i, _)| *i != c)
                .map(|(_, s)| s)
                .collect();
            let subset: Vec<&Share> = rest[..k].to_vec();
            if rest[k..]
                .iter()
                .all(|s| interpolate(&subset, s.x, gf) == s.y)
            {
                culprits.push(c);
            }
        }
        if culprits.len() == 1 {
            let s = &shares[culprits[0]];
            return Verification {
                status: "failed",
                detail: format!(
                    "the share on line {} (x={}) does not lie on the same polynomial as the other {} — it is corrupted, from a different split, or encoded differently",
                    s.line,
                    s.x,
                    n - 1
                ),
                disagreeing: vec![s.x],
                cross_checked: n - k,
            };
        }
    }

    let hint = if n == k + 1 {
        ". Supply one more share to pin down which one is wrong"
    } else {
        ""
    };
    Verification {
        status: "failed",
        detail: format!(
            "the shares do not agree: reconstructing from the first {k} does not reproduce {} of the remaining share(s). At least one share is corrupted, they come from different splits, or the field polynomial is wrong{hint}",
            bad.len()
        ),
        disagreeing: bad,
        cross_checked: n - k,
    }
}

// ---------------------------------------------------------------------------
// Secret encoding
// ---------------------------------------------------------------------------

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn to_base64(bytes: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for chunk in bytes.chunks(3) {
        let b = [
            chunk[0],
            *chunk.get(1).unwrap_or(&0),
            *chunk.get(2).unwrap_or(&0),
        ];
        let n = ((b[0] as u32) << 16) | ((b[1] as u32) << 8) | b[2] as u32;
        out.push(A[(n >> 18) as usize & 63] as char);
        out.push(A[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 {
            A[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            A[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

fn to_binary(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:08b}"))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Printable text means valid UTF-8 with no control characters other than the
/// usual whitespace — a binary key must never render as mojibake.
fn is_printable_text(bytes: &[u8]) -> bool {
    match std::str::from_utf8(bytes) {
        Ok(s) => {
            !s.is_empty()
                && !s
                    .chars()
                    .any(|c| c.is_control() && c != '\n' && c != '\r' && c != '\t')
        }
        Err(_) => false,
    }
}

fn render_secret(bytes: &[u8], enc: &str) -> Result<(String, &'static str), String> {
    let chosen = if enc == "auto" {
        if is_printable_text(bytes) {
            "text"
        } else {
            "hex"
        }
    } else {
        enc
    };
    let text = match chosen {
        "text" => std::str::from_utf8(bytes)
            .map_err(|_| {
                "the recovered secret is not valid UTF-8 text — choose hex, base64 or binary for the secret encoding".to_string()
            })?
            .to_string(),
        "hex" => to_hex(bytes),
        "base64" => to_base64(bytes),
        "binary" => to_binary(bytes),
        _ => unreachable!(),
    };
    let name: &'static str = match chosen {
        "text" => "text",
        "hex" => "hex",
        "base64" => "base64",
        _ => "binary",
    };
    Ok((text, name))
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn pick_enum(value: &str, allowed: &[&str], default: &str, param: &str) -> Result<String, String> {
    let v = value.trim();
    let v = if v.is_empty() { default } else { v };
    let lower = v.to_ascii_lowercase();
    if allowed.contains(&lower.as_str()) {
        Ok(lower)
    } else {
        Err(format!(
            "`{param}` must be one of {}, got `{}`",
            allowed.join(", "),
            truncate(v, 24)
        ))
    }
}

#[allow(clippy::too_many_arguments)]
pub fn run(
    shares: &str,
    share_format: &str,
    share_encoding: &str,
    field_poly: &str,
    threshold: i64,
    verify: bool,
    secret_encoding: &str,
    output: &str,
) -> Result<String, String> {
    let fmt_opt = pick_enum(
        share_format,
        &["auto", "index-prefix", "leading-index", "trailing-index"],
        "auto",
        "share_format",
    )?;
    let enc_opt = pick_enum(
        share_encoding,
        &["auto", "hex", "base64"],
        "auto",
        "share_encoding",
    )?;
    let poly_opt = pick_enum(
        field_poly,
        &["auto", "0x11b", "0x11d"],
        "auto",
        "field_poly",
    )?;
    let secret_enc = pick_enum(
        secret_encoding,
        &["auto", "text", "hex", "base64", "binary"],
        "auto",
        "secret_encoding",
    )?;
    let output = pick_enum(output, &["secret", "report", "json"], "secret", "output")?;

    if threshold != 0 && !(2..=255).contains(&threshold) {
        return Err(format!(
            "`threshold` must be 0 (use every share supplied) or between 2 and 255, got {threshold}"
        ));
    }

    let cleaned = clean_lines(shares);
    if cleaned.is_empty() {
        return Err(
            "no shares found — paste one share per line (blank lines and `#` comments are ignored)"
                .into(),
        );
    }
    if cleaned.len() < 2 {
        return Err(format!(
            "at least 2 shares are required to recover a secret, got {}. A single share reveals nothing about the secret by design",
            cleaned.len()
        ));
    }
    if cleaned.len() > MAX_SHARES {
        return Err(format!(
            "too many shares: {} (maximum {MAX_SHARES}, the number of nonzero GF(256) coordinates)",
            cleaned.len()
        ));
    }

    // Candidate share formats. `auto` narrows to index-prefix when every line
    // carries one, honours an `sss:` prefix, and otherwise lets the cross-check
    // decide between a leading and a trailing index byte.
    let format_candidates: Vec<Format> = match fmt_opt.as_str() {
        "index-prefix" => vec![Format::IndexPrefix],
        "leading-index" => vec![Format::Leading],
        "trailing-index" => vec![Format::Trailing],
        _ => {
            if cleaned
                .iter()
                .all(|c| !c.prefixed && split_index_prefix(&c.token).is_some())
            {
                vec![Format::IndexPrefix]
            } else if cleaned.iter().any(|c| c.prefixed) {
                vec![Format::Leading]
            } else {
                vec![Format::Leading, Format::Trailing]
            }
        }
    };
    let enc_pref = match enc_opt.as_str() {
        "hex" => Some(Encoding::Hex),
        "base64" => Some(Encoding::Base64),
        _ => None,
    };

    let mut parsed: Vec<(Format, Vec<Share>, Encoding)> = Vec::new();
    let mut first_err: Option<String> = None;
    for fmt in &format_candidates {
        match parse_shares(&cleaned, *fmt, enc_pref) {
            Ok((s, enc)) => parsed.push((*fmt, s, enc)),
            Err(e) => {
                if first_err.is_none() {
                    first_err = Some(e);
                }
            }
        }
    }
    if parsed.is_empty() {
        return Err(first_err.unwrap_or_else(|| "could not parse any share".into()));
    }

    let n = parsed[0].1.len();
    let k = if threshold == 0 {
        n
    } else {
        threshold as usize
    };
    if k > n {
        return Err(format!(
            "threshold {k} is larger than the {n} share(s) supplied — recovery needs at least {k} shares"
        ));
    }
    let secret_len = parsed[0].1[0].y.len();
    if n.saturating_mul(k).saturating_mul(secret_len) > MAX_WORK {
        return Err(format!(
            "share set is too large to cross-check in one pass ({n} shares × {k} threshold × {secret_len} bytes) — split the recovery or reduce the share count"
        ));
    }

    let poly_candidates: Vec<u16> = match poly_opt.as_str() {
        "0x11b" => vec![0x11b],
        "0x11d" => vec![0x11d],
        _ => vec![0x11b, 0x11d],
    };

    // Resolve (format, polynomial) together: both are only decidable from the
    // shares when there is at least one redundant share to predict.
    let mut chosen: Option<(usize, u16)> = None;
    let has_redundancy = n > k;
    if has_redundancy && verify && (parsed.len() > 1 || poly_candidates.len() > 1) {
        'outer: for (pi, (_, sh, _)) in parsed.iter().enumerate() {
            for &p in &poly_candidates {
                let gf = Gf::new(p);
                if baseline_agrees(sh, k, &gf).0 {
                    chosen = Some((pi, p));
                    break 'outer;
                }
            }
        }
    }
    let resolved_by_cross_check = chosen.is_some();
    let (pi, poly) = chosen.unwrap_or((0, poly_candidates[0]));
    let (fmt, shares_list, encoding) = &parsed[pi];
    let gf = Gf::new(poly);

    let format_source = if fmt_opt != "auto" {
        "as requested"
    } else if format_candidates.len() == 1 {
        "auto-detected"
    } else if resolved_by_cross_check {
        "auto-detected, confirmed by a redundant share"
    } else {
        "auto-detected, assumed — no redundant share to confirm it"
    };
    let encoding_source = if enc_opt == "auto" {
        "auto-detected"
    } else {
        "as requested"
    };
    let poly_source = if poly_opt != "auto" {
        "as requested"
    } else if resolved_by_cross_check {
        "auto-detected, confirmed by a redundant share"
    } else if !has_redundancy {
        "auto, assumed — supply more shares than the threshold to confirm it"
    } else if !verify {
        "auto, assumed — verification is off"
    } else {
        "auto, assumed — no polynomial fits every share"
    };
    let threshold_source = if threshold == 0 {
        "every share supplied"
    } else {
        "as requested"
    };

    let subset: Vec<&Share> = shares_list[..k].iter().collect();
    let secret_bytes = interpolate(&subset, 0, &gf);

    let verification = if verify {
        verify_shares(shares_list, k, &gf)
    } else {
        Verification {
            status: "off",
            detail: "cross-checking is turned off — a corrupted share would silently produce a wrong secret".into(),
            disagreeing: Vec::new(),
            cross_checked: 0,
        }
    };

    let (secret_text, secret_enc_used) = render_secret(&secret_bytes, &secret_enc)?;
    let indices: Vec<u8> = shares_list.iter().map(|s| s.x).collect();

    match output.as_str() {
        "secret" => {
            if verification.status == "failed" {
                return Err(format!(
                    "share cross-check failed: {}. The recovered value would be wrong, so it is not returned — set output to `report` for the details, or turn verification off to recover anyway",
                    verification.detail
                ));
            }
            Ok(secret_text)
        }
        "report" => {
            let mut out = String::new();
            out.push_str("Recovered secret:\n");
            out.push_str(&secret_text);
            out.push_str("\n\n");
            out.push_str(&format!("Secret encoding:   {secret_enc_used}\n"));
            out.push_str(&format!(
                "Secret length:     {} byte(s)\n",
                secret_bytes.len()
            ));
            out.push_str(&format!("Shares supplied:   {n}\n"));
            out.push_str(&format!("Threshold used:    {k} ({threshold_source})\n"));
            out.push_str(&format!(
                "Share format:      {} ({format_source})\n",
                fmt.name()
            ));
            out.push_str(&format!(
                "Share encoding:    {} ({encoding_source})\n",
                encoding.name()
            ));
            out.push_str(&format!(
                "Field polynomial:  {} ({poly_source})\n",
                gf.name()
            ));
            out.push_str(&format!(
                "Share indices:     {}\n",
                indices
                    .iter()
                    .map(|x| x.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
            out.push_str(&format!(
                "Verification:      {} — {}\n",
                verification.status, verification.detail
            ));
            if !verification.disagreeing.is_empty() {
                out.push_str(&format!(
                    "Disagreeing share: x={}\n",
                    verification
                        .disagreeing
                        .iter()
                        .map(|x| x.to_string())
                        .collect::<Vec<_>>()
                        .join(", x=")
                ));
            }
            Ok(out.trim_end().to_string())
        }
        _ => {
            let json = serde_json::json!({
                "secret": secret_text,
                "secret_encoding": secret_enc_used,
                "secret_bytes": secret_bytes.len(),
                "secret_hex": to_hex(&secret_bytes),
                "shares_supplied": n,
                "share_indices": indices,
                "threshold": k,
                "threshold_source": threshold_source,
                "share_format": fmt.name(),
                "share_format_source": format_source,
                "share_encoding": encoding.name(),
                "share_encoding_source": encoding_source,
                "field_poly": gf.name(),
                "field_poly_source": poly_source,
                "verification": {
                    "status": verification.status,
                    "detail": verification.detail,
                    "cross_checked_shares": verification.cross_checked,
                    "disagreeing_indices": verification.disagreeing,
                },
            });
            serde_json::to_string_pretty(&json).map_err(|e| e.to_string())
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // An independent, table-free split used to build test vectors. Deliberately
    // written with its own shift-and-xor multiply so it does not share the
    // log/exp implementation it is checking.
    fn t_mul(mut a: u8, mut b: u8, poly: u16) -> u8 {
        let mut p = 0u8;
        for _ in 0..8 {
            if b & 1 != 0 {
                p ^= a;
            }
            let hi = a & 0x80;
            a <<= 1;
            if hi != 0 {
                a ^= (poly & 0xff) as u8;
            }
            b >>= 1;
        }
        p
    }

    fn t_split(secret: &[u8], k: usize, n: usize, poly: u16, seed: u32) -> Vec<(u8, Vec<u8>)> {
        let mut state = seed;
        let mut next = move || {
            state = state.wrapping_mul(1103515245).wrapping_add(12345) & 0x7fff_ffff;
            ((state >> 16) & 0xff) as u8
        };
        let mut out: Vec<(u8, Vec<u8>)> = (1..=n as u8).map(|x| (x, Vec::new())).collect();
        for &b in secret {
            let mut coeffs = vec![b];
            for _ in 1..k {
                coeffs.push(next());
            }
            while *coeffs.last().unwrap() == 0 {
                let v = next();
                *coeffs.last_mut().unwrap() = if v == 0 { 1 } else { v };
            }
            for (x, ys) in out.iter_mut() {
                let mut acc = 0u8;
                for c in coeffs.iter().rev() {
                    acc = t_mul(acc, *x, poly) ^ c;
                }
                ys.push(acc);
            }
        }
        out
    }

    fn hex(bytes: &[u8]) -> String {
        to_hex(bytes)
    }

    #[test]
    fn aes_field_vector() {
        // The canonical GF(2^8)/0x11b vector: 0x57 · 0x83 = 0xc1.
        let gf = Gf::new(0x11b);
        assert_eq!(gf.mul(0x57, 0x83), 0xc1);
        assert_eq!(gf.mul(0x57, 0x13), 0xfe);
        // Division is the exact inverse of multiplication in both fields.
        for poly in [0x11bu16, 0x11d] {
            let gf = Gf::new(poly);
            for a in 1..=255u8 {
                for b in [1u8, 7, 99, 255] {
                    assert_eq!(gf.div(gf.mul(a, b), b), a, "poly {poly:#x} a={a} b={b}");
                }
            }
        }
    }

    // ---- happy paths -----------------------------------------------------

    // Vectors below were produced by an independent Python GF(256) implementation.
    // SET_A: "hello world" split 3-of-5 over 0x11b, index-prefixed hex.
    const SET_A: &str = "1-68b509858f664dea3c3829\n2-73fb23909fc0cdded4b830\n3-732b46797f86f75b9aec7d\n4-32d6d05b38500da0cd34d9\n5-3206b5b2d8163725836094";
    // The same set with the last byte of share 3 flipped.
    const SET_A_CORRUPT: &str = "1-68b509858f664dea3c3829\n2-73fb23909fc0cdded4b830\n3-732b46797f86f75b9aec7c\n4-32d6d05b38500da0cd34d9\n5-3206b5b2d8163725836094";
    // SET_B: "correct horse" split 2-of-3 over 0x11b, `sss:` base64url leading-index.
    const SET_B: &str = "sss:AQ8hBuB2RlYOWc6_YNs\nsss:Arvzmk1DKTB8CjbzVQI\nsss:A9e97t9QDBJSO5c-Rrw";
    // SET_C: "vault-master-key" split 3-of-4 over 0x11d, trailing-index hex.
    const SET_C: &str = "7b2d29cba93975ea5108479560c3b6bb01\nc5b32e7f030860ac928276c4f6ae75d402\nc8ff72d8de1c7827b0fe5423bb06a61603\n93ed6857fd5383d902bd028a9fe6d3ee04";
    // SET_D: the 16 raw bytes 0x80..0x8f (not valid UTF-8) split 2-of-2 over 0x11b.
    const SET_D: &str =
        "[1-5f88fa0655e93ef4b4e19727719867d6]\n[2-259372923d5ded61f059b0c86da7473d]";

    #[test]
    fn index_prefix_hex_auto_everything() {
        let got = run(SET_A, "auto", "auto", "auto", 3, true, "auto", "secret").unwrap();
        assert_eq!(got, "hello world");
    }

    #[test]
    fn index_prefix_tolerates_brackets_separators_and_comments() {
        let messy = "# my 3-of-4 backup\n\n  [1-68b509858f664dea3c3829]  \n\"2:73fb23909fc0cdded4b830\",\n\n3;732b46797f86f75b9aec7d # third holder\n";
        let got = run(messy, "auto", "auto", "auto", 3, true, "auto", "secret").unwrap();
        assert_eq!(got, "hello world");
    }

    #[test]
    fn leading_index_base64url_with_sss_prefix() {
        let got = run(SET_B, "auto", "auto", "auto", 2, true, "auto", "secret").unwrap();
        assert_eq!(got, "correct horse");
        // Explicit format + encoding reaches the same answer.
        let got = run(
            SET_B,
            "leading-index",
            "base64",
            "0x11b",
            2,
            true,
            "text",
            "secret",
        )
        .unwrap();
        assert_eq!(got, "correct horse");
    }

    #[test]
    fn trailing_index_hex_with_0x11d_resolved_by_cross_check() {
        // Both a leading and a trailing index byte are structurally plausible here,
        // and both reduction polynomials are candidates — the redundant share picks.
        let report = run(SET_C, "auto", "auto", "auto", 3, true, "auto", "report").unwrap();
        assert!(
            report.starts_with("Recovered secret:\nvault-master-key\n"),
            "{report}"
        );
        assert!(
            report.contains("Share format:      trailing-index"),
            "{report}"
        );
        assert!(report.contains("Field polynomial:  0x11d"), "{report}");
        assert!(report.contains("Verification:      passed"), "{report}");
        // Naming the format explicitly gets there without the search.
        assert_eq!(
            run(
                SET_C,
                "trailing-index",
                "hex",
                "0x11d",
                3,
                true,
                "text",
                "secret"
            )
            .unwrap(),
            "vault-master-key"
        );
    }

    #[test]
    fn leading_index_hex_without_prefix() {
        let set = "017d76\n024b51\n03594c";
        assert_eq!(
            run(
                set,
                "leading-index",
                "hex",
                "0x11b",
                2,
                true,
                "text",
                "secret"
            )
            .unwrap(),
            "ok"
        );
    }

    #[test]
    fn secret_output_encodings() {
        let hexed = run(SET_D, "auto", "auto", "0x11b", 0, true, "hex", "secret").unwrap();
        assert_eq!(hexed, "808182838485868788898a8b8c8d8e8f");
        let b64 = run(SET_D, "auto", "auto", "0x11b", 0, true, "base64", "secret").unwrap();
        assert_eq!(b64, "gIGCg4SFhoeIiYqLjI2Ojw==");
        let bin = run(SET_D, "auto", "auto", "0x11b", 0, true, "binary", "secret").unwrap();
        assert!(bin.starts_with("10000000 10000001 10000010"), "{bin}");
        // `auto` refuses to render unprintable bytes as text.
        let auto = run(SET_D, "auto", "auto", "0x11b", 0, true, "auto", "secret").unwrap();
        assert_eq!(auto, hexed);
        // …and an explicit `text` request on non-UTF-8 bytes is an honest error.
        let err = run(SET_D, "auto", "auto", "0x11b", 0, true, "text", "secret").unwrap_err();
        assert!(err.contains("not valid UTF-8"), "{err}");
    }

    #[test]
    fn json_output_reports_every_resolution() {
        let json = run(SET_A, "auto", "auto", "auto", 3, true, "auto", "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["secret"], "hello world");
        assert_eq!(v["secret_encoding"], "text");
        assert_eq!(v["secret_bytes"], 11);
        assert_eq!(v["shares_supplied"], 5);
        assert_eq!(v["threshold"], 3);
        assert_eq!(v["share_format"], "index-prefix");
        assert_eq!(v["share_encoding"], "hex");
        assert_eq!(v["field_poly"], "0x11b");
        assert_eq!(v["verification"]["status"], "passed");
        assert_eq!(v["verification"]["cross_checked_shares"], 2);
        assert_eq!(v["share_indices"], serde_json::json!([1, 2, 3, 4, 5]));
    }

    #[test]
    fn threshold_unset_uses_every_share() {
        let report = run(SET_A, "auto", "auto", "0x11b", 0, true, "auto", "report").unwrap();
        assert!(
            report.contains("Threshold used:    5 (every share supplied)"),
            "{report}"
        );
        assert!(report.contains("Verification:      skipped"), "{report}");
        // Recovery still works — 4 points also determine a degree-2 polynomial.
        assert!(
            report.starts_with("Recovered secret:\nhello world\n"),
            "{report}"
        );
    }

    #[test]
    fn both_polynomials_round_trip_through_split() {
        for poly in [0x11bu16, 0x11d] {
            let secret = b"round trip";
            let shares = t_split(secret, 3, 5, poly, 2024);
            let text: String = shares
                .iter()
                .map(|(x, y)| format!("{x}-{}", hex(y)))
                .collect::<Vec<_>>()
                .join("\n");
            let name = if poly == 0x11b { "0x11b" } else { "0x11d" };
            assert_eq!(
                run(
                    &text,
                    "index-prefix",
                    "hex",
                    name,
                    3,
                    true,
                    "text",
                    "secret"
                )
                .unwrap(),
                "round trip"
            );
            // …and `auto` finds the same polynomial from the redundant shares.
            assert_eq!(
                run(&text, "auto", "auto", "auto", 3, true, "text", "secret").unwrap(),
                "round trip"
            );
        }
    }

    #[test]
    fn every_three_of_five_subset_agrees() {
        let shares = t_split(b"subset", 3, 5, 0x11b, 77);
        for i in 0..5 {
            for j in (i + 1)..5 {
                for k in (j + 1)..5 {
                    let text = [i, j, k]
                        .iter()
                        .map(|&t| format!("{}-{}", shares[t].0, hex(&shares[t].1)))
                        .collect::<Vec<_>>()
                        .join("\n");
                    assert_eq!(
                        run(&text, "auto", "auto", "0x11b", 0, true, "text", "secret").unwrap(),
                        "subset"
                    );
                }
            }
        }
    }

    #[test]
    fn boundary_255_shares_recovers_and_256_is_rejected() {
        let shares = t_split(b"max", 2, 255, 0x11b, 5);
        let text: String = shares
            .iter()
            .map(|(x, y)| format!("{x}-{}", hex(y)))
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(
            run(
                &text,
                "index-prefix",
                "hex",
                "0x11b",
                2,
                true,
                "text",
                "secret"
            )
            .unwrap(),
            "max"
        );
        // 256 lines cannot be a valid GF(256) share set.
        let over = format!("{text}\n1-000000");
        let err = run(
            &over,
            "index-prefix",
            "hex",
            "0x11b",
            2,
            true,
            "text",
            "secret",
        )
        .unwrap_err();
        assert!(err.contains("too many shares: 256"), "{err}");
    }

    // ---- errors ----------------------------------------------------------

    #[test]
    fn duplicate_index_is_rejected() {
        let set = "1-68b509858f664dea3c3829\n1-73fb23909fc0cdded4b830";
        let err = run(set, "auto", "auto", "auto", 0, true, "auto", "secret").unwrap_err();
        assert!(err.contains("duplicate share index x=1"), "{err}");
    }

    #[test]
    fn zero_index_is_rejected() {
        let set = "0-1234\n2-5678";
        let err = run(
            set,
            "index-prefix",
            "hex",
            "auto",
            0,
            true,
            "auto",
            "secret",
        )
        .unwrap_err();
        assert!(err.contains("expected an index-prefixed share"), "{err}");
        let raw = "0011\n0222";
        let err = run(
            raw,
            "leading-index",
            "hex",
            "auto",
            0,
            true,
            "auto",
            "secret",
        )
        .unwrap_err();
        assert!(
            err.contains("share index 0 is not a valid coordinate"),
            "{err}"
        );
    }

    #[test]
    fn too_few_shares_is_rejected() {
        let err = run(
            "1-68b509858f664dea3c3829",
            "auto",
            "auto",
            "auto",
            0,
            true,
            "auto",
            "secret",
        )
        .unwrap_err();
        assert!(err.contains("at least 2 shares are required"), "{err}");
        let err = run(
            "\n\n# nothing here\n",
            "auto",
            "auto",
            "auto",
            0,
            true,
            "auto",
            "secret",
        )
        .unwrap_err();
        assert!(err.contains("no shares found"), "{err}");
    }

    #[test]
    fn mismatched_lengths_are_rejected() {
        let set = "1-68b509858f664dea3c3829\n2-73fb23909fc0cdded4b8";
        let err = run(
            set,
            "index-prefix",
            "hex",
            "auto",
            0,
            true,
            "auto",
            "secret",
        )
        .unwrap_err();
        assert!(err.contains("shares are not the same length"), "{err}");
    }

    #[test]
    fn bad_threshold_is_rejected() {
        let err = run(SET_A, "auto", "auto", "auto", 1, true, "auto", "secret").unwrap_err();
        assert!(err.contains("`threshold` must be 0"), "{err}");
        let err = run(SET_A, "auto", "auto", "auto", 9, true, "auto", "secret").unwrap_err();
        assert!(
            err.contains("threshold 9 is larger than the 5 share(s) supplied"),
            "{err}"
        );
        let err = run(SET_A, "auto", "auto", "auto", 300, true, "auto", "secret").unwrap_err();
        assert!(err.contains("`threshold` must be 0"), "{err}");
    }

    #[test]
    fn bad_enum_values_are_rejected() {
        let err = run(SET_A, "sideways", "auto", "auto", 3, true, "auto", "secret").unwrap_err();
        assert!(err.contains("`share_format` must be one of"), "{err}");
        let err = run(SET_A, "auto", "auto", "0x11f", 3, true, "auto", "secret").unwrap_err();
        assert!(err.contains("`field_poly` must be one of"), "{err}");
    }

    #[test]
    fn undecodable_payload_is_rejected() {
        let set = "1-zzzz\n2-yyyy";
        let err = run(
            set,
            "index-prefix",
            "hex",
            "auto",
            0,
            true,
            "auto",
            "secret",
        )
        .unwrap_err();
        assert!(err.contains("could not decode as hex"), "{err}");
    }

    #[test]
    fn corrupted_redundant_share_is_named_and_blocked() {
        let err = run(
            SET_A_CORRUPT,
            "auto",
            "auto",
            "0x11b",
            3,
            true,
            "auto",
            "secret",
        )
        .unwrap_err();
        assert!(err.contains("share cross-check failed"), "{err}");

        let report = run(
            SET_A_CORRUPT,
            "auto",
            "auto",
            "0x11b",
            3,
            true,
            "auto",
            "report",
        )
        .unwrap();
        assert!(report.contains("Verification:      failed"), "{report}");
        assert!(report.contains("line 3 (x=3)"), "{report}");
        assert!(report.contains("Disagreeing share: x=3"), "{report}");

        // Turning verification off recovers anyway — and the first three shares
        // include the corrupted one, so the result is deliberately wrong.
        let anyway = run(
            SET_A_CORRUPT,
            "auto",
            "auto",
            "0x11b",
            3,
            false,
            "auto",
            "secret",
        )
        .unwrap();
        assert_ne!(anyway, "hello world");

        // Dropping the bad share recovers the real secret.
        let good = "1-68b509858f664dea3c3829\n2-73fb23909fc0cdded4b830\n4-32d6d05b38500da0cd34d9";
        assert_eq!(
            run(good, "auto", "auto", "0x11b", 3, true, "auto", "secret").unwrap(),
            "hello world"
        );
    }

    #[test]
    fn a_single_spare_share_detects_but_cannot_name_the_culprit() {
        let four = "1-68b509858f664dea3c3829\n2-73fb23909fc0cdded4b830\n3-732b46797f86f75b9aec7c\n4-32d6d05b38500da0cd34d9";
        let report = run(four, "auto", "auto", "0x11b", 3, true, "auto", "report").unwrap();
        assert!(report.contains("Verification:      failed"), "{report}");
        assert!(
            report.contains("Supply one more share to pin down which one is wrong"),
            "{report}"
        );
    }

    #[test]
    fn json_reports_a_failed_cross_check_instead_of_erroring() {
        let json = run(
            SET_A_CORRUPT,
            "auto",
            "auto",
            "0x11b",
            3,
            true,
            "auto",
            "json",
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(v["verification"]["status"], "failed");
        assert_eq!(
            v["verification"]["disagreeing_indices"],
            serde_json::json!([3])
        );
    }
}
