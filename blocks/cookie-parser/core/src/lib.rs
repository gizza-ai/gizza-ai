//! cookie-parser core — split a raw HTTP `Cookie:` or `Set-Cookie:` header into
//! individual cookies with their attributes (Domain, Path, Expires, Max-Age,
//! Secure, HttpOnly, SameSite, Priority, Partitioned).
//!
//! Handles both header directions: the request `Cookie:` header is a flat
//! `name=value; name=value` list, while each response `Set-Cookie:` line carries
//! ONE cookie plus its attributes. `mode = "auto"` detects which one was pasted.
//! `Expires` is normalized to an ISO-8601 UTC timestamp with the lenient RFC 6265
//! §5.1.1 date algorithm, per-cookie byte size is reported against the common
//! 4096-byte browser cap, and structural problems (`SameSite=None` without
//! `Secure`, `__Host-` prefix violations, a deleting `Max-Age`) are flagged as
//! warnings. Deterministic: nothing here reads the clock.
//!
//! Pure-Rust, no wafer/wasm-bindgen deps. Shared by the chat block and the page.

use serde_json::{json, Map, Value};

/// Per-cookie byte budget browsers commonly enforce (RFC 6265 §6.1 minimum).
pub const MAX_COOKIE_BYTES: usize = 4096;

/// Which header direction the input is parsed as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Request `Cookie:` header — a flat `name=value; …` list, no attributes.
    Cookie,
    /// Response `Set-Cookie:` header — one cookie per line, plus attributes.
    SetCookie,
}

impl Mode {
    /// The canonical wire name, as reported in the output.
    pub fn as_str(self) -> &'static str {
        match self {
            Mode::Cookie => "cookie",
            Mode::SetCookie => "set-cookie",
        }
    }
}

/// One parsed cookie, in source order.
#[derive(Debug, Clone, PartialEq)]
pub struct ParsedCookie {
    pub name: String,
    pub value: String,
    /// Bytes of the raw `name=value` pair (pre-decoding) — what browsers count.
    pub size: usize,
    pub domain: Option<String>,
    pub path: Option<String>,
    /// The `Expires` attribute exactly as written.
    pub expires: Option<String>,
    /// `Expires` normalized to `YYYY-MM-DDTHH:MM:SSZ`, when parseable.
    pub expires_iso: Option<String>,
    pub max_age: Option<i64>,
    /// The `Max-Age` attribute as written (kept when it isn't an integer).
    pub max_age_raw: Option<String>,
    pub secure: bool,
    pub http_only: bool,
    pub same_site: Option<String>,
    pub priority: Option<String>,
    pub partitioned: bool,
    /// Attributes outside the RFC 6265 / CHIPS set, in source order.
    pub other: Vec<(String, String)>,
    /// Each attribute segment verbatim, as written after the first `;`.
    pub attributes_raw: Vec<String>,
    /// The whole cookie line verbatim (header name stripped).
    pub raw: String,
    /// No `Expires` and no `Max-Age` — dropped when the browser session ends.
    pub session: bool,
    /// No `Domain` attribute — the cookie is sent to the origin host only.
    pub host_only: bool,
    pub warnings: Vec<String>,
}

// ---------------------------------------------------------------------------
// decoding helpers
// ---------------------------------------------------------------------------

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Percent-decode a string. Invalid `%` escapes are left literal (lenient — a
/// stray `%` shouldn't error). Unlike a URL query string a `+` is NOT decoded to
/// a space: cookie values are not `application/x-www-form-urlencoded`. Bytes are
/// decoded as UTF-8 lossily so non-UTF-8 sequences degrade to U+FFFD.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'%' && i + 2 < bytes.len() {
            if let (Some(h), Some(l)) = (hex_val(bytes[i + 1]), hex_val(bytes[i + 2])) {
                out.push((h << 4) | l);
                i += 3;
                continue;
            }
        }
        out.push(b);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Strip a pasted `Set-Cookie:` / `Cookie:` header name (case-insensitive) from
/// the front of a line so a whole header line can be pasted as-is. Returns the
/// remainder plus whether a `Set-Cookie:` name was seen (a direction hint).
fn strip_header_name(line: &str) -> (&str, bool) {
    let s = line.trim();
    let lower = s.to_ascii_lowercase();
    if lower.starts_with("set-cookie:") {
        (s["set-cookie:".len()..].trim(), true)
    } else if lower.starts_with("cookie:") {
        (s["cookie:".len()..].trim(), false)
    } else {
        (s, false)
    }
}

/// Unwrap an RFC 6265 double-quoted value (`"a b"` → `a b`).
fn unquote(v: &str) -> &str {
    if v.len() >= 2 && v.starts_with('"') && v.ends_with('"') {
        &v[1..v.len() - 1]
    } else {
        v
    }
}

/// Split `name=value`; a segment with no `=` is a name with an empty value.
fn split_pair(seg: &str) -> (String, String) {
    match seg.find('=') {
        Some(i) => (seg[..i].trim().to_string(), seg[i + 1..].trim().to_string()),
        None => (seg.trim().to_string(), String::new()),
    }
}

// ---------------------------------------------------------------------------
// date normalization (RFC 6265 §5.1.1, lenient)
// ---------------------------------------------------------------------------

const MONTHS: [&str; 12] = [
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];

fn is_leap(y: i64) -> bool {
    (y % 4 == 0 && y % 100 != 0) || y % 400 == 0
}

fn days_in_month(y: i64, m: u32) -> u32 {
    match m {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap(y) => 29,
        2 => 28,
        _ => 0,
    }
}

/// Normalize a cookie `Expires` date to `YYYY-MM-DDTHH:MM:SSZ`.
///
/// Follows the lenient RFC 6265 algorithm: split on delimiters, then take the
/// first token that looks like a time, a day, a month name, and a year. That
/// accepts every shape seen in the wild — `Wed, 21 Oct 2015 07:28:00 GMT`,
/// `Wed, 21-Oct-2015 07:28:00 GMT`, and the asctime `Wed Oct 21 07:28:00 2015`.
/// Returns `None` when a required component is missing or out of range.
pub fn normalize_expires(raw: &str) -> Option<String> {
    let mut time: Option<(u32, u32, u32)> = None;
    let mut day: Option<u32> = None;
    let mut month: Option<u32> = None;
    let mut year: Option<i64> = None;

    for token in raw.split(|c: char| !(c.is_ascii_alphanumeric() || c == ':')) {
        if token.is_empty() {
            continue;
        }
        if time.is_none() && token.contains(':') {
            let parts: Vec<&str> = token.split(':').collect();
            if parts.len() == 3 && parts.iter().all(|p| !p.is_empty() && p.len() <= 2) {
                let nums: Option<Vec<u32>> = parts.iter().map(|p| p.parse::<u32>().ok()).collect();
                if let Some(n) = nums {
                    time = Some((n[0], n[1], n[2]));
                }
            }
            continue;
        }
        let lower = token.to_ascii_lowercase();
        if month.is_none() && lower.len() >= 3 {
            if let Some(i) = MONTHS.iter().position(|m| lower.starts_with(m)) {
                month = Some(i as u32 + 1);
                continue;
            }
        }
        if token.chars().all(|c| c.is_ascii_digit()) {
            if day.is_none() && token.len() <= 2 {
                day = token.parse::<u32>().ok();
            } else if year.is_none() {
                year = token.parse::<i64>().ok();
            }
        }
    }

    let (h, mi, s) = time?;
    let d = day?;
    let m = month?;
    let mut y = year?;
    // Two-digit years: 70–99 → 19xx, 00–69 → 20xx (RFC 6265 §5.1.1).
    if y <= 69 {
        y += 2000;
    } else if y <= 99 {
        y += 1900;
    }
    if h > 23
        || mi > 59
        || s > 59
        || d < 1
        || d > days_in_month(y, m)
        || !(1601..=9999).contains(&y)
    {
        return None;
    }
    Some(format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y, m, d, h, mi, s
    ))
}

// ---------------------------------------------------------------------------
// parsing
// ---------------------------------------------------------------------------

/// True when a cookie line carries at least one `Set-Cookie` attribute after the
/// first `;` — the auto-detection signal.
fn looks_like_set_cookie(line: &str) -> bool {
    const ATTRS: [&str; 9] = [
        "path",
        "domain",
        "expires",
        "max-age",
        "secure",
        "httponly",
        "samesite",
        "priority",
        "partitioned",
    ];
    let mut segs = line.split(';');
    let _ = segs.next();
    segs.any(|seg| {
        let key = seg
            .split('=')
            .next()
            .unwrap_or("")
            .trim()
            .to_ascii_lowercase();
        ATTRS.contains(&key.as_str())
    })
}

/// Detect the header direction of `input` (used when `mode = "auto"`).
pub fn detect_mode(input: &str) -> Mode {
    for line in input.lines() {
        let (body, was_set_cookie) = strip_header_name(line);
        if was_set_cookie {
            return Mode::SetCookie;
        }
        if !body.is_empty() && looks_like_set_cookie(body) {
            return Mode::SetCookie;
        }
    }
    Mode::Cookie
}

fn canon_same_site(v: &str) -> Option<&'static str> {
    match v.trim().to_ascii_lowercase().as_str() {
        "strict" => Some("Strict"),
        "lax" => Some("Lax"),
        "none" => Some("None"),
        _ => None,
    }
}

fn canon_priority(v: &str) -> Option<&'static str> {
    match v.trim().to_ascii_lowercase().as_str() {
        "low" => Some("Low"),
        "medium" => Some("Medium"),
        "high" => Some("High"),
        _ => None,
    }
}

fn finish(name_raw: &str, value_raw: &str, decode: bool, c: &mut ParsedCookie) {
    let value_unquoted = unquote(value_raw);
    c.size = name_raw.len() + 1 + value_raw.len();
    c.name = if decode {
        percent_decode(name_raw)
    } else {
        name_raw.to_string()
    };
    c.value = if decode {
        percent_decode(value_unquoted)
    } else {
        value_unquoted.to_string()
    };
    c.session = c.expires.is_none() && c.max_age_raw.is_none();
    c.host_only = c.domain.is_none();
}

fn blank_cookie(raw: &str) -> ParsedCookie {
    ParsedCookie {
        name: String::new(),
        value: String::new(),
        size: 0,
        domain: None,
        path: None,
        expires: None,
        expires_iso: None,
        max_age: None,
        max_age_raw: None,
        secure: false,
        http_only: false,
        same_site: None,
        priority: None,
        partitioned: false,
        other: Vec::new(),
        attributes_raw: Vec::new(),
        raw: raw.to_string(),
        session: true,
        host_only: true,
        warnings: Vec::new(),
    }
}

/// Parse one `Set-Cookie` line (header name already stripped).
fn parse_set_cookie_line(line: &str, decode: bool) -> ParsedCookie {
    let mut c = blank_cookie(line);
    let mut segs = line.split(';');
    let pair = segs.next().unwrap_or("");
    let (name_raw, value_raw) = split_pair(pair);

    for seg in segs {
        let seg_trim = seg.trim();
        if seg_trim.is_empty() {
            continue;
        }
        c.attributes_raw.push(seg_trim.to_string());
        let (key, val) = split_pair(seg_trim);
        match key.to_ascii_lowercase().as_str() {
            "domain" => c.domain = Some(val),
            "path" => c.path = Some(val),
            "expires" => {
                c.expires_iso = normalize_expires(&val);
                c.expires = Some(val);
            }
            "max-age" => {
                c.max_age = val.trim().parse::<i64>().ok();
                c.max_age_raw = Some(val);
            }
            "secure" => c.secure = true,
            "httponly" => c.http_only = true,
            "samesite" => {
                c.same_site = Some(canon_same_site(&val).map(str::to_string).unwrap_or(val))
            }
            "priority" => {
                c.priority = Some(canon_priority(&val).map(str::to_string).unwrap_or(val))
            }
            "partitioned" => c.partitioned = true,
            _ => c.other.push((key, val)),
        }
    }
    finish(&name_raw, &value_raw, decode, &mut c);
    c
}

/// Parse a request `Cookie:` header body into its `name=value` pairs.
fn parse_cookie_header(input: &str, decode: bool) -> Vec<ParsedCookie> {
    let mut out = Vec::new();
    for line in input.lines() {
        let (body, _) = strip_header_name(line);
        for seg in body.split(';') {
            let seg_trim = seg.trim();
            if seg_trim.is_empty() {
                continue;
            }
            let mut c = blank_cookie(seg_trim);
            let (name_raw, value_raw) = split_pair(seg_trim);
            finish(&name_raw, &value_raw, decode, &mut c);
            out.push(c);
        }
    }
    out
}

fn add_warnings(cookies: &mut [ParsedCookie], mode: Mode) {
    let names: Vec<String> = cookies.iter().map(|c| c.name.clone()).collect();
    for (i, c) in cookies.iter_mut().enumerate() {
        if c.name.is_empty() {
            c.warnings
                .push("no cookie name — expected `name=value` before the first `;`".into());
        }
        if c.size > MAX_COOKIE_BYTES {
            c.warnings.push(format!(
                "{} bytes — over the common {MAX_COOKIE_BYTES}-byte per-cookie browser limit",
                c.size
            ));
        }
        if names.iter().enumerate().any(|(j, n)| j < i && n == &c.name) {
            c.warnings
                .push("duplicate cookie name — an earlier cookie uses the same name".into());
        }
        if mode == Mode::Cookie {
            continue;
        }
        if c.same_site.as_deref() == Some("None") && !c.secure {
            c.warnings
                .push("SameSite=None without Secure — browsers reject this cookie".into());
        }
        if let Some(ss) = &c.same_site {
            if canon_same_site(ss).is_none() {
                c.warnings.push(format!(
                    "unknown SameSite value `{ss}` — expected Strict, Lax, or None"
                ));
            }
        } else {
            c.warnings
                .push("no SameSite — browser defaults vary; set it explicitly".into());
        }
        if !c.secure {
            c.warnings
                .push("no Secure — the cookie may be sent over plain HTTP".into());
        }
        if !c.http_only {
            c.warnings
                .push("no HttpOnly — the cookie is readable from JavaScript".into());
        }
        if c.expires.is_some() && c.max_age_raw.is_some() {
            c.warnings
                .push("both Expires and Max-Age set — Max-Age wins where supported".into());
        }
        if let Some(raw) = &c.expires {
            if c.expires_iso.is_none() {
                c.warnings.push(format!(
                    "unparseable Expires `{raw}` — expected e.g. `Wed, 21 Oct 2015 07:28:00 GMT`"
                ));
            }
        }
        match (&c.max_age_raw, c.max_age) {
            (Some(raw), None) => c.warnings.push(format!(
                "Max-Age `{raw}` is not an integer — expected a number of seconds"
            )),
            (Some(_), Some(n)) if n <= 0 => c
                .warnings
                .push(format!("Max-Age {n} — this deletes the cookie immediately")),
            _ => {}
        }
        if let Some(d) = &c.domain {
            if d.starts_with('.') {
                c.warnings
                    .push("leading dot in Domain is ignored by modern browsers (RFC 6265)".into());
            }
        }
        if c.partitioned && !c.secure {
            c.warnings
                .push("Partitioned requires Secure — browsers reject this cookie".into());
        }
        if let Some(p) = &c.priority {
            if canon_priority(p).is_none() {
                c.warnings.push(format!(
                    "unknown Priority value `{p}` — expected Low, Medium, or High"
                ));
            }
        }
        if c.name.starts_with("__Secure-") && !c.secure {
            c.warnings
                .push("`__Secure-` name prefix requires the Secure attribute".into());
        }
        if c.name.starts_with("__Host-") {
            if !c.secure || c.domain.is_some() || c.path.as_deref() != Some("/") {
                c.warnings
                    .push("`__Host-` name prefix requires Secure, no Domain, and Path=/".into());
            }
        }
    }
}

// ---------------------------------------------------------------------------
// rendering
// ---------------------------------------------------------------------------

fn cookie_json(c: &ParsedCookie, mode: Mode, raw_attributes: bool, warnings: bool) -> Value {
    let mut m = Map::new();
    m.insert("name".into(), json!(c.name));
    m.insert("value".into(), json!(c.value));
    m.insert("size".into(), json!(c.size));
    if mode == Mode::SetCookie {
        let mut a = Map::new();
        a.insert("domain".into(), json!(c.domain));
        a.insert("path".into(), json!(c.path));
        a.insert("expires".into(), json!(c.expires));
        a.insert("expires_iso".into(), json!(c.expires_iso));
        a.insert("max_age".into(), json!(c.max_age));
        a.insert("secure".into(), json!(c.secure));
        a.insert("http_only".into(), json!(c.http_only));
        a.insert("same_site".into(), json!(c.same_site));
        a.insert("priority".into(), json!(c.priority));
        a.insert("partitioned".into(), json!(c.partitioned));
        if !c.other.is_empty() {
            let other: Map<String, Value> =
                c.other.iter().map(|(k, v)| (k.clone(), json!(v))).collect();
            a.insert("other".into(), Value::Object(other));
        }
        m.insert("attributes".into(), Value::Object(a));
        m.insert("session".into(), json!(c.session));
        m.insert("host_only".into(), json!(c.host_only));
    }
    if raw_attributes {
        m.insert("raw".into(), json!(c.raw));
        if mode == Mode::SetCookie {
            m.insert("attributes_raw".into(), json!(c.attributes_raw));
        }
    }
    if warnings {
        m.insert("warnings".into(), json!(c.warnings));
    }
    Value::Object(m)
}

fn dash(v: Option<&String>) -> String {
    v.cloned().unwrap_or_else(|| "-".to_string())
}

fn yes_no(b: bool) -> String {
    if b {
        "yes".into()
    } else {
        "no".into()
    }
}

/// Header row + one row per cookie, shared by the table/csv/markdown renderers.
fn grid(
    cookies: &[ParsedCookie],
    mode: Mode,
    raw_attributes: bool,
    plain: bool,
) -> (Vec<String>, Vec<Vec<String>>) {
    let mut headers: Vec<String> = vec!["Name".into(), "Value".into(), "Size".into()];
    if mode == Mode::SetCookie {
        headers.extend(
            [
                "Domain",
                "Path",
                "Expires",
                "Max-Age",
                "Secure",
                "HttpOnly",
                "SameSite",
                "Priority",
                "Partitioned",
            ]
            .iter()
            .map(|s| s.to_string()),
        );
    }
    if raw_attributes {
        headers.push(if mode == Mode::SetCookie {
            "Attributes".into()
        } else {
            "Raw".into()
        });
    }
    let empty = |s: String| if plain && s == "-" { String::new() } else { s };
    let rows = cookies
        .iter()
        .map(|c| {
            let mut row = vec![c.name.clone(), c.value.clone(), c.size.to_string()];
            if mode == Mode::SetCookie {
                row.push(empty(dash(c.domain.as_ref())));
                row.push(empty(dash(c.path.as_ref())));
                row.push(empty(dash(c.expires_iso.as_ref().or(c.expires.as_ref()))));
                row.push(empty(dash(c.max_age_raw.as_ref())));
                row.push(yes_no(c.secure));
                row.push(yes_no(c.http_only));
                row.push(empty(dash(c.same_site.as_ref())));
                row.push(empty(dash(c.priority.as_ref())));
                row.push(yes_no(c.partitioned));
            }
            if raw_attributes {
                row.push(if mode == Mode::SetCookie {
                    c.attributes_raw.join("; ")
                } else {
                    c.raw.clone()
                });
            }
            row
        })
        .collect();
    (headers, rows)
}

fn warning_lines(cookies: &[ParsedCookie], bullet: bool) -> Vec<String> {
    let mut out = Vec::new();
    for c in cookies {
        for w in &c.warnings {
            let label = if c.name.is_empty() {
                "(unnamed)"
            } else {
                &c.name
            };
            out.push(if bullet {
                format!("- `{label}` — {w}")
            } else {
                format!("  {label}: {w}")
            });
        }
    }
    out
}

fn render_table(cookies: &[ParsedCookie], mode: Mode, raw_attributes: bool, warn: bool) -> String {
    let (headers, rows) = grid(cookies, mode, raw_attributes, false);
    let mut widths: Vec<usize> = headers.iter().map(|h| h.chars().count()).collect();
    for row in &rows {
        for (i, cell) in row.iter().enumerate() {
            widths[i] = widths[i].max(cell.chars().count());
        }
    }
    let fmt_row = |cells: &[String]| {
        cells
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let pad = widths[i] - c.chars().count();
                if i == cells.len() - 1 {
                    c.clone()
                } else {
                    format!("{c}{}", " ".repeat(pad))
                }
            })
            .collect::<Vec<_>>()
            .join("  ")
    };
    let mut out = String::new();
    out.push_str(&format!(
        "{} cookie{} ({} header)\n\n",
        cookies.len(),
        if cookies.len() == 1 { "" } else { "s" },
        if mode == Mode::SetCookie {
            "Set-Cookie"
        } else {
            "Cookie"
        }
    ));
    out.push_str(&fmt_row(&headers));
    out.push('\n');
    out.push_str(
        &widths
            .iter()
            .map(|w| "-".repeat(*w))
            .collect::<Vec<_>>()
            .join("  "),
    );
    for row in &rows {
        out.push('\n');
        out.push_str(fmt_row(row).trim_end());
    }
    if warn {
        let lines = warning_lines(cookies, false);
        if !lines.is_empty() {
            out.push_str("\n\nWarnings:\n");
            out.push_str(&lines.join("\n"));
        }
    }
    out
}

fn csv_cell(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') || s.trim() != s {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn render_csv(cookies: &[ParsedCookie], mode: Mode, raw_attributes: bool, warn: bool) -> String {
    let (mut headers, mut rows) = grid(cookies, mode, raw_attributes, true);
    if warn {
        headers.push("Warnings".into());
        for (row, c) in rows.iter_mut().zip(cookies) {
            row.push(c.warnings.join("; "));
        }
    }
    let mut out = String::new();
    out.push_str(
        &headers
            .iter()
            .map(|h| csv_cell(h))
            .collect::<Vec<_>>()
            .join(","),
    );
    for row in &rows {
        out.push('\n');
        out.push_str(
            &row.iter()
                .map(|c| csv_cell(c))
                .collect::<Vec<_>>()
                .join(","),
        );
    }
    out
}

fn md_cell(s: &str) -> String {
    let s = s.replace('|', "\\|");
    if s.is_empty() {
        "-".into()
    } else {
        s
    }
}

fn render_markdown(
    cookies: &[ParsedCookie],
    mode: Mode,
    raw_attributes: bool,
    warn: bool,
) -> String {
    let (headers, rows) = grid(cookies, mode, raw_attributes, false);
    let mut out = String::new();
    out.push_str(&format!(
        "| {} |\n",
        headers
            .iter()
            .map(|h| md_cell(h))
            .collect::<Vec<_>>()
            .join(" | ")
    ));
    out.push_str(&format!(
        "| {} |",
        headers
            .iter()
            .map(|_| "---".to_string())
            .collect::<Vec<_>>()
            .join(" | ")
    ));
    for row in &rows {
        out.push_str(&format!(
            "\n| {} |",
            row.iter()
                .map(|c| md_cell(c))
                .collect::<Vec<_>>()
                .join(" | ")
        ));
    }
    if warn {
        let lines = warning_lines(cookies, true);
        if !lines.is_empty() {
            out.push_str("\n\n**Warnings**\n\n");
            out.push_str(&lines.join("\n"));
        }
    }
    out
}

// ---------------------------------------------------------------------------
// entry point
// ---------------------------------------------------------------------------

/// Parse `input` and render it as `format`.
///
/// * `mode` — `auto` (default) | `cookie` | `set-cookie`
/// * `format` — `json` | `table` | `csv` | `markdown`
/// * `decode` — percent-decode cookie names and values
/// * `raw_attributes` — also emit each cookie verbatim
/// * `warnings` — flag structural problems per cookie
pub fn run(
    input: &str,
    mode: &str,
    format: &str,
    decode: bool,
    raw_attributes: bool,
    warnings: bool,
) -> Result<String, String> {
    let mode_sel = match mode.trim().to_ascii_lowercase().as_str() {
        "" | "auto" => None,
        "cookie" => Some(Mode::Cookie),
        "set-cookie" => Some(Mode::SetCookie),
        other => {
            return Err(format!(
                "unknown mode `{other}` — expected auto, cookie, or set-cookie"
            ))
        }
    };
    let format_norm = format.trim().to_ascii_lowercase();
    let format_norm = if format_norm.is_empty() {
        "json".to_string()
    } else {
        format_norm
    };
    if !["json", "table", "csv", "markdown"].contains(&format_norm.as_str()) {
        return Err(format!(
            "unknown format `{format}` — expected json, table, csv, or markdown"
        ));
    }
    if input.trim().is_empty() {
        return Err(
            "cookie string is empty — paste a Cookie or Set-Cookie header, e.g. \
             `Set-Cookie: sid=abc123; Path=/; Secure; HttpOnly`"
                .into(),
        );
    }

    let resolved = mode_sel.unwrap_or_else(|| detect_mode(input));
    let mut cookies: Vec<ParsedCookie> = match resolved {
        Mode::Cookie => parse_cookie_header(input, decode),
        Mode::SetCookie => input
            .lines()
            .map(|l| strip_header_name(l).0)
            .filter(|l| !l.is_empty())
            .map(|l| parse_set_cookie_line(l, decode))
            .collect(),
    };
    if cookies.is_empty() {
        return Err(format!(
            "no cookies found in the input — expected `name=value` pairs, got `{}`",
            input.trim()
        ));
    }
    add_warnings(&mut cookies, resolved);

    Ok(match format_norm.as_str() {
        "table" => render_table(&cookies, resolved, raw_attributes, warnings),
        "csv" => render_csv(&cookies, resolved, raw_attributes, warnings),
        "markdown" => render_markdown(&cookies, resolved, raw_attributes, warnings),
        _ => {
            let doc = json!({
                "mode": resolved.as_str(),
                "count": cookies.len(),
                "cookies": cookies
                    .iter()
                    .map(|c| cookie_json(c, resolved, raw_attributes, warnings))
                    .collect::<Vec<_>>(),
            });
            serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())?
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> Value {
        serde_json::from_str(s).unwrap()
    }

    #[test]
    fn parses_cookie_header_pairs() {
        let out = run(
            "sessionid=abc123; theme=dark",
            "auto",
            "json",
            true,
            false,
            false,
        )
        .unwrap();
        let j = v(&out);
        assert_eq!(j["mode"], "cookie");
        assert_eq!(j["count"], 2);
        assert_eq!(j["cookies"][0]["name"], "sessionid");
        assert_eq!(j["cookies"][0]["value"], "abc123");
        assert_eq!(j["cookies"][0]["size"], 16);
        assert_eq!(j["cookies"][1]["name"], "theme");
        // Request cookies carry no attributes.
        assert!(j["cookies"][0].get("attributes").is_none());
    }

    #[test]
    fn parses_set_cookie_attributes() {
        let out = run(
            "Set-Cookie: sid=abc; Domain=example.com; Path=/; Expires=Wed, 21 Oct 2015 07:28:00 GMT; Max-Age=3600; Secure; HttpOnly; SameSite=lax; Priority=high; Partitioned",
            "auto",
            "json",
            true,
            false,
            false,
        )
        .unwrap();
        let j = v(&out);
        assert_eq!(j["mode"], "set-cookie");
        let a = &j["cookies"][0]["attributes"];
        assert_eq!(a["domain"], "example.com");
        assert_eq!(a["path"], "/");
        assert_eq!(a["expires_iso"], "2015-10-21T07:28:00Z");
        assert_eq!(a["max_age"], 3600);
        assert_eq!(a["secure"], true);
        assert_eq!(a["http_only"], true);
        assert_eq!(a["same_site"], "Lax");
        assert_eq!(a["priority"], "High");
        assert_eq!(a["partitioned"], true);
        assert_eq!(j["cookies"][0]["session"], false);
        assert_eq!(j["cookies"][0]["host_only"], false);
    }

    #[test]
    fn auto_detects_set_cookie_without_header_name() {
        assert_eq!(detect_mode("sid=abc; Path=/; Secure"), Mode::SetCookie);
        assert_eq!(detect_mode("a=1; b=2"), Mode::Cookie);
        assert_eq!(detect_mode("Set-Cookie: a=1"), Mode::SetCookie);
    }

    #[test]
    fn mode_override_forces_cookie_parsing() {
        let out = run("sid=abc; Path=/", "cookie", "json", true, false, false).unwrap();
        let j = v(&out);
        assert_eq!(j["mode"], "cookie");
        assert_eq!(j["count"], 2);
        assert_eq!(j["cookies"][1]["name"], "Path");
    }

    #[test]
    fn multiple_set_cookie_lines() {
        let out = run(
            "Set-Cookie: a=1; Path=/\nSet-Cookie: b=2; Secure",
            "auto",
            "json",
            true,
            false,
            false,
        )
        .unwrap();
        let j = v(&out);
        assert_eq!(j["count"], 2);
        assert_eq!(j["cookies"][1]["name"], "b");
        assert_eq!(j["cookies"][1]["attributes"]["secure"], true);
    }

    #[test]
    fn decoding_can_be_disabled_and_quotes_are_unwrapped() {
        let on = run("redirect=%2Faccount", "cookie", "json", true, false, false).unwrap();
        assert_eq!(v(&on)["cookies"][0]["value"], "/account");
        let off = run("redirect=%2Faccount", "cookie", "json", false, false, false).unwrap();
        assert_eq!(v(&off)["cookies"][0]["value"], "%2Faccount");
        let q = run("sid=\"a b\"", "cookie", "json", true, false, false).unwrap();
        assert_eq!(v(&q)["cookies"][0]["value"], "a b");
        // `+` stays literal — cookies are not form-urlencoded.
        let plus = run("q=a+b", "cookie", "json", true, false, false).unwrap();
        assert_eq!(v(&plus)["cookies"][0]["value"], "a+b");
    }

    #[test]
    fn raw_attributes_are_optional() {
        let out = run("sid=abc; Path=/; Secure", "auto", "json", true, true, false).unwrap();
        let j = v(&out);
        assert_eq!(j["cookies"][0]["raw"], "sid=abc; Path=/; Secure");
        assert_eq!(j["cookies"][0]["attributes_raw"][0], "Path=/");
        assert_eq!(j["cookies"][0]["attributes_raw"][1], "Secure");
    }

    #[test]
    fn unknown_attributes_are_preserved() {
        let out = run(
            "sid=abc; Path=/; Comment=hi",
            "auto",
            "json",
            true,
            false,
            false,
        )
        .unwrap();
        assert_eq!(
            v(&out)["cookies"][0]["attributes"]["other"]["Comment"],
            "hi"
        );
    }

    #[test]
    fn warnings_flag_structural_problems() {
        let out = run(
            "Set-Cookie: sid=abc; SameSite=None; Max-Age=0",
            "auto",
            "json",
            true,
            false,
            true,
        )
        .unwrap();
        let w = v(&out)["cookies"][0]["warnings"].to_string();
        assert!(w.contains("SameSite=None without Secure"), "{w}");
        assert!(w.contains("deletes the cookie"), "{w}");
        assert!(w.contains("no HttpOnly"), "{w}");
    }

    #[test]
    fn host_prefix_warning() {
        let ok = run(
            "Set-Cookie: __Host-id=1; Path=/; Secure; HttpOnly; SameSite=Lax",
            "auto",
            "json",
            true,
            false,
            true,
        )
        .unwrap();
        assert!(!v(&ok)["cookies"][0]["warnings"]
            .to_string()
            .contains("__Host-"));
        let bad = run(
            "Set-Cookie: __Host-id=1; Domain=example.com; Path=/; Secure",
            "auto",
            "json",
            true,
            false,
            true,
        )
        .unwrap();
        assert!(v(&bad)["cookies"][0]["warnings"]
            .to_string()
            .contains("__Host-"));
    }

    #[test]
    fn oversized_cookie_is_flagged() {
        let big = format!("Set-Cookie: big={}; Path=/", "x".repeat(4100));
        let out = run(&big, "auto", "json", true, false, true).unwrap();
        assert!(v(&out)["cookies"][0]["warnings"]
            .to_string()
            .contains("over the common 4096-byte"));
    }

    #[test]
    fn expires_date_variants_normalize() {
        assert_eq!(
            normalize_expires("Wed, 21 Oct 2015 07:28:00 GMT").as_deref(),
            Some("2015-10-21T07:28:00Z")
        );
        assert_eq!(
            normalize_expires("Wed, 21-Oct-2015 07:28:00 GMT").as_deref(),
            Some("2015-10-21T07:28:00Z")
        );
        assert_eq!(
            normalize_expires("Wed Oct 21 07:28:00 2015").as_deref(),
            Some("2015-10-21T07:28:00Z")
        );
        assert_eq!(
            normalize_expires("Thu, 01-Jan-70 00:00:00 GMT").as_deref(),
            Some("1970-01-01T00:00:00Z")
        );
        assert_eq!(normalize_expires("not a date"), None);
        assert_eq!(normalize_expires("Wed, 31 Feb 2015 07:28:00 GMT"), None);
    }

    #[test]
    fn table_format_is_aligned() {
        let out = run(
            "Set-Cookie: sid=abc; Path=/; Secure",
            "auto",
            "table",
            true,
            false,
            false,
        )
        .unwrap();
        assert_eq!(
            out,
            "1 cookie (Set-Cookie header)\n\n\
             Name  Value  Size  Domain  Path  Expires  Max-Age  Secure  HttpOnly  SameSite  Priority  Partitioned\n\
             ----  -----  ----  ------  ----  -------  -------  ------  --------  --------  --------  -----------\n\
             sid   abc    7     -       /     -        -        yes     no        -         -         no"
        );
    }

    #[test]
    fn csv_and_markdown_formats() {
        let csv = run("a=1; b=2", "cookie", "csv", true, false, false).unwrap();
        assert_eq!(csv, "Name,Value,Size\na,1,3\nb,2,3");
        let md = run("a=1", "cookie", "markdown", true, false, false).unwrap();
        assert_eq!(
            md,
            "| Name | Value | Size |\n| --- | --- | --- |\n| a | 1 | 3 |"
        );
    }

    #[test]
    fn empty_input_is_an_error() {
        let e = run("   ", "auto", "json", true, false, false).unwrap_err();
        assert!(e.contains("cookie string is empty"), "{e}");
    }

    #[test]
    fn header_name_with_no_cookies_is_an_error() {
        let e = run("Cookie:", "auto", "json", true, false, false).unwrap_err();
        assert!(e.contains("no cookies found"), "{e}");
    }

    #[test]
    fn unknown_mode_and_format_are_errors() {
        let e = run("a=1", "banana", "json", true, false, false).unwrap_err();
        assert!(e.contains("unknown mode"), "{e}");
        let e = run("a=1", "auto", "yaml", true, false, false).unwrap_err();
        assert!(e.contains("unknown format"), "{e}");
    }
}
