//! memory-strings core — a `strings`-style extractor for memory / process dumps.
//!
//! Two steps, both pure-Rust and browser-local (no I/O, nothing uploaded):
//!
//! 1. **Extract** the printable runs from the dump bytes — ASCII and/or UTF-16LE
//!    (wide) sequences of at least `min_length` characters, exactly like the
//!    classic `strings -n <len>` / `strings -e l` utility. The dump can be pasted
//!    as raw text (its UTF-8 bytes are the input) or as a hex-encoded dump
//!    (`48 65 6c 6c 6f`, `48:65`, `0x48 0x65`, or contiguous hex).
//! 2. **Categorize** the recovered strings into the artifact classes an analyst
//!    triages first — URLs, IPv4, IPv6, emails, domains, file paths (Windows,
//!    UNC and Unix) and Windows registry keys — de-duplicated, sorted and counted.
//!
//! Hash extraction is intentionally left to `blocks/ioc-extract` so the two tools
//! don't overlap. The IOC scanners here are hand-rolled token validators (no
//! false-positive-prone giant regex); the path/registry scanners use `regex`.

use std::collections::BTreeSet;

use regex::Regex;

/// A category of recovered string, in display order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Category {
    Url,
    Ipv4,
    Ipv6,
    Email,
    Domain,
    Path,
    Registry,
}

impl Category {
    const ALL: [Category; 7] = [
        Category::Url,
        Category::Ipv4,
        Category::Ipv6,
        Category::Email,
        Category::Domain,
        Category::Path,
        Category::Registry,
    ];

    fn label(self) -> &'static str {
        match self {
            Category::Url => "URLs",
            Category::Ipv4 => "IPv4 addresses",
            Category::Ipv6 => "IPv6 addresses",
            Category::Email => "Emails",
            Category::Domain => "Domains",
            Category::Path => "File paths",
            Category::Registry => "Registry keys",
        }
    }
}

/// Which printable encodings to recover before categorizing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Encoding {
    Ascii,
    Utf16le,
    Both,
}

impl Encoding {
    fn parse(s: &str) -> Result<Encoding, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "both" | "all" => Ok(Encoding::Both),
            "ascii" | "utf8" | "utf-8" | "7bit" | "8bit" => Ok(Encoding::Ascii),
            "utf16le" | "utf-16le" | "utf16" | "wide" | "unicode" | "l" => Ok(Encoding::Utf16le),
            other => Err(format!(
                "expected encoding to be 'ascii', 'utf16le' or 'both', got '{other}'"
            )),
        }
    }
    fn wants_ascii(self) -> bool {
        matches!(self, Encoding::Ascii | Encoding::Both)
    }
    fn wants_wide(self) -> bool {
        matches!(self, Encoding::Utf16le | Encoding::Both)
    }
}

/// Which input representation the dump text is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputFormat {
    Text,
    Hex,
}

impl InputFormat {
    fn parse(s: &str) -> Result<InputFormat, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "text" | "raw" | "bytes" | "utf8" => Ok(InputFormat::Text),
            "hex" | "hexdump" | "hexadecimal" => Ok(InputFormat::Hex),
            other => Err(format!(
                "expected input_format to be 'text' or 'hex', got '{other}'"
            )),
        }
    }
}

/// Parse the `categories` selector into the ordered set of categories to report.
fn parse_categories(s: &str) -> Result<Vec<Category>, String> {
    let s = s.trim();
    if s.is_empty() || s.eq_ignore_ascii_case("all") {
        return Ok(Category::ALL.to_vec());
    }
    let mut wanted: Vec<Category> = Vec::new();
    let push = |c: Category, w: &mut Vec<Category>| {
        if !w.contains(&c) {
            w.push(c);
        }
    };
    for raw in s.split(|c: char| c == ',' || c == ' ' || c == '\n' || c == '\t' || c == ';') {
        let name = raw.trim().to_ascii_lowercase();
        if name.is_empty() {
            continue;
        }
        match name.as_str() {
            "url" | "urls" => push(Category::Url, &mut wanted),
            "ipv4" | "ip4" => push(Category::Ipv4, &mut wanted),
            "ipv6" | "ip6" => push(Category::Ipv6, &mut wanted),
            "ip" | "ips" => {
                push(Category::Ipv4, &mut wanted);
                push(Category::Ipv6, &mut wanted);
            }
            "email" | "emails" | "mail" => push(Category::Email, &mut wanted),
            "domain" | "domains" | "host" | "hosts" => push(Category::Domain, &mut wanted),
            "path" | "paths" | "file" | "files" | "filepath" | "filepaths" => {
                push(Category::Path, &mut wanted)
            }
            "registry" | "reg" | "registrykey" | "registrykeys" | "regkey" => {
                push(Category::Registry, &mut wanted)
            }
            other => {
                return Err(format!(
                    "unknown category '{other}'; expected any of url, ipv4, ipv6, ip, email, domain, path, registry (or 'all')"
                ))
            }
        }
    }
    if wanted.is_empty() {
        return Err("no valid categories selected".into());
    }
    // keep canonical display order regardless of input order
    Ok(Category::ALL
        .iter()
        .copied()
        .filter(|c| wanted.contains(c))
        .collect())
}

/// Decode a hex-encoded dump. Accepts contiguous hex, whitespace/`:,-|_.`-separated
/// bytes, and `0x`/`0X` byte prefixes. Errors on stray non-hex characters or an
/// odd number of hex digits.
fn decode_hex(s: &str) -> Result<Vec<u8>, String> {
    let mut hex = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '0' && matches!(chars.peek(), Some('x') | Some('X')) {
            chars.next(); // consume the 'x' of a 0x byte prefix
            continue;
        }
        if c.is_ascii_hexdigit() {
            hex.push(c);
        } else if c.is_whitespace() || matches!(c, ':' | ',' | '-' | '|' | '_' | '.') {
            // byte separator — ignore
        } else {
            return Err(format!(
                "hex input contains a non-hex character '{c}'; expected hex bytes like '48 65 6c 6c 6f'"
            ));
        }
    }
    if hex.len() % 2 != 0 {
        return Err(format!(
            "hex input has an odd number of hex digits ({}); each byte needs two",
            hex.len()
        ));
    }
    let bytes = hex.as_bytes();
    let mut out = Vec::with_capacity(bytes.len() / 2);
    let mut i = 0;
    while i < bytes.len() {
        let hi = (bytes[i] as char).to_digit(16).unwrap() as u8;
        let lo = (bytes[i + 1] as char).to_digit(16).unwrap() as u8;
        out.push((hi << 4) | lo);
        i += 2;
    }
    Ok(out)
}

#[inline]
fn is_print(b: u8) -> bool {
    // Printable ASCII incl. space and tab (matches `strings` defaults).
    (0x20..=0x7e).contains(&b) || b == 0x09
}

/// Extract printable ASCII runs of length >= `min` from raw bytes.
fn extract_ascii(bytes: &[u8], min: usize, out: &mut Vec<String>) {
    let mut run = String::new();
    for &b in bytes {
        if is_print(b) {
            run.push(b as char);
        } else if run.len() >= min {
            out.push(std::mem::take(&mut run));
        } else {
            run.clear();
        }
    }
    if run.len() >= min {
        out.push(run);
    }
}

/// Extract printable UTF-16LE (wide) runs of length >= `min`: sequences of
/// `[printable-byte, 0x00]` pairs, as `strings -e l` does.
fn extract_utf16le(bytes: &[u8], min: usize, out: &mut Vec<String>) {
    let len = bytes.len();
    let mut i = 0;
    while i + 1 < len {
        if is_print(bytes[i]) && bytes[i + 1] == 0 {
            let mut run = String::new();
            while i + 1 < len && is_print(bytes[i]) && bytes[i + 1] == 0 {
                run.push(bytes[i] as char);
                i += 2;
            }
            if run.len() >= min {
                out.push(run);
            }
        } else {
            i += 1;
        }
    }
}

/// Defang an indicator so it's safe to paste into a report/ticket.
fn defang(s: &str) -> String {
    let mut out = s.to_string();
    out = out.replace("https://", "hxxps[://]");
    out = out.replace("http://", "hxxp[://]");
    out = out.replace("ftp://", "fxp[://]");
    out = out.replace('@', "[at]");
    out = out.replace('.', "[.]");
    out
}

// ---------------------------------------------------------------------------
// Token-based IOC scanners (URL, IPv4, IPv6, email, domain).
// ---------------------------------------------------------------------------

/// Tokenize on anything that can't be part of an indicator.
fn tokens(text: &str) -> Vec<&str> {
    text.split(|c: char| {
        !(c.is_ascii_alphanumeric()
            || matches!(
                c,
                '.' | ':' | '/' | '@' | '-' | '_' | '~' | '?' | '=' | '&' | '%' | '#' | '+'
            ))
    })
    .filter(|s| !s.is_empty())
    .collect()
}

fn trim_edges(s: &str) -> &str {
    s.trim_matches(|c: char| {
        matches!(
            c,
            '.' | ',' | ';' | ':' | '"' | '\'' | ')' | '(' | '<' | '>' | '/'
        )
    })
}

fn is_ipv4(s: &str) -> bool {
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() != 4 {
        return false;
    }
    parts.iter().all(|p| {
        !p.is_empty()
            && p.len() <= 3
            && p.bytes().all(|b| b.is_ascii_digit())
            && p.parse::<u16>().map(|n| n <= 255).unwrap_or(false)
            && !(p.len() > 1 && p.starts_with('0'))
    })
}

fn scan_ipv4(text: &str) -> BTreeSet<String> {
    let mut set = BTreeSet::new();
    for tok in tokens(text) {
        let t = trim_edges(tok);
        if is_ipv4(t) {
            set.insert(t.to_string());
        }
    }
    set
}

fn is_ipv6(s: &str) -> bool {
    if !s.contains(':') {
        return false;
    }
    if s.matches("::").count() > 1 {
        return false;
    }
    if s.contains(":::")
        || (s.starts_with(':') && !s.starts_with("::"))
        || (s.ends_with(':') && !s.ends_with("::"))
    {
        return false;
    }
    let has_compress = s.contains("::");
    let groups: Vec<&str> = s.split(':').collect();
    if groups.len() < 3 || groups.len() > 9 {
        return false;
    }
    let mut hextet_count = 0usize;
    for (i, g) in groups.iter().enumerate() {
        if g.is_empty() {
            continue;
        }
        if i == groups.len() - 1 && g.contains('.') {
            if !is_ipv4(g) {
                return false;
            }
            hextet_count += 2;
            continue;
        }
        if g.len() > 4 || !g.bytes().all(|b| b.is_ascii_hexdigit()) {
            return false;
        }
        hextet_count += 1;
    }
    if has_compress {
        (1..=7).contains(&hextet_count)
    } else {
        hextet_count == 8
    }
}

fn scan_ipv6(text: &str) -> BTreeSet<String> {
    let mut set = BTreeSet::new();
    for tok in tokens(text) {
        let t = tok.trim_matches(|c: char| matches!(c, '[' | ']'));
        let t = t.split('%').next().unwrap_or(t);
        let t = trim_edges(t);
        if is_ipv6(t) {
            set.insert(t.to_ascii_lowercase());
        }
    }
    set
}

fn scan_url(text: &str) -> BTreeSet<String> {
    let mut set = BTreeSet::new();
    for tok in tokens(text) {
        let t = trim_edges(tok);
        let lower = t.to_ascii_lowercase();
        if (lower.starts_with("http://")
            || lower.starts_with("https://")
            || lower.starts_with("ftp://"))
            && t.len() > 8
            && t[t.find("://").map(|i| i + 3).unwrap_or(t.len())..].len() >= 3
        {
            set.insert(t.to_string());
        }
    }
    set
}

fn is_domain(s: &str) -> bool {
    if s.len() > 253 || !s.contains('.') || is_ipv4(s) {
        return false;
    }
    let labels: Vec<&str> = s.split('.').collect();
    if labels.len() < 2 {
        return false;
    }
    let tld = labels[labels.len() - 1];
    if tld.len() < 2 || !tld.bytes().all(|b| b.is_ascii_alphabetic()) {
        return false;
    }
    // A `strings` dump token such as `cmd.exe` or `payload.dll` is a filename,
    // not a bare domain. Keep common artifact extensions out of the domain
    // bucket so file paths don't create noisy pseudo-hosts.
    if matches!(
        tld.to_ascii_lowercase().as_str(),
        "exe"
            | "dll"
            | "sys"
            | "bat"
            | "cmd"
            | "ps1"
            | "vbs"
            | "scr"
            | "dat"
            | "bin"
            | "tmp"
            | "log"
            | "txt"
            | "ini"
            | "cfg"
            | "conf"
            | "json"
            | "xml"
            | "html"
            | "htm"
            | "doc"
            | "docx"
            | "xls"
            | "xlsx"
            | "pdf"
            | "zip"
            | "rar"
            | "gz"
            | "tar"
    ) {
        return false;
    }
    labels.iter().all(|l| {
        !l.is_empty()
            && l.len() <= 63
            && !l.starts_with('-')
            && !l.ends_with('-')
            && l.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    })
}

fn url_host(url: &str) -> Option<String> {
    let after_scheme = &url[url.find("://").map(|i| i + 3)?..];
    let host = after_scheme
        .split(|c: char| matches!(c, '/' | '?' | '#'))
        .next()
        .unwrap_or(after_scheme);
    let host = host.rsplit('@').next().unwrap_or(host);
    let host = host.split(':').next().unwrap_or(host);
    if host.is_empty() {
        None
    } else {
        Some(host.to_ascii_lowercase())
    }
}

fn is_email(s: &str) -> bool {
    let at = match s.find('@') {
        Some(i) if s.matches('@').count() == 1 => i,
        _ => return false,
    };
    let (local, domain) = (&s[..at], &s[at + 1..]);
    if local.is_empty() || local.len() > 64 {
        return false;
    }
    if !local
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b'+' | b'%'))
    {
        return false;
    }
    is_domain(domain)
}

fn scan_email(text: &str) -> BTreeSet<String> {
    let mut set = BTreeSet::new();
    for tok in tokens(text) {
        let t = trim_edges(tok);
        if is_email(t) {
            set.insert(t.to_ascii_lowercase());
        }
    }
    set
}

fn scan_domain(text: &str) -> BTreeSet<String> {
    // Bare hostnames only — a host that is exactly a URL's or email's own host is
    // excluded so the categories don't double-report it.
    let url_hosts: BTreeSet<String> = scan_url(text).iter().filter_map(|u| url_host(u)).collect();
    let email_hosts: BTreeSet<String> = scan_email(text)
        .iter()
        .filter_map(|e| e.rsplit('@').next().map(|h| h.to_ascii_lowercase()))
        .collect();
    let mut set = BTreeSet::new();
    for tok in tokens(text) {
        let t = trim_edges(tok);
        if t.contains("://") || t.contains('@') {
            continue;
        }
        let host = t.split('/').next().unwrap_or(t);
        if !is_domain(host) {
            continue;
        }
        let host = host.to_ascii_lowercase();
        if url_hosts.contains(&host) || email_hosts.contains(&host) {
            continue;
        }
        set.insert(host);
    }
    set
}

// ---------------------------------------------------------------------------
// Regex-based path + registry scanners (the memory-forensics-specific ones).
// ---------------------------------------------------------------------------

thread_local! {
    // Windows drive path (C:\Windows\System32\cmd.exe) and UNC (\\server\share\x).
    // No whitespace inside a match: in a `strings` dump whitespace delimits
    // tokens, so allowing spaces would let one greedy match swallow the prose
    // that follows the path (e.g. "...\cmd.exe key HKLM\...").
    static WIN_PATH: Regex = Regex::new(
        r#"(?i)(?:[a-z]:\\|\\\\[a-z0-9._$-]+\\)[^\s"*?<>|]*"#
    ).unwrap();
    // Absolute Unix path with >= 2 segments (/etc/passwd, /usr/bin/curl). No
    // space inside a segment: in a `strings` dump whitespace delimits tokens, so
    // allowing spaces would let one greedy match swallow surrounding prose.
    static UNIX_PATH: Regex = Regex::new(
        r#"(?:/[A-Za-z0-9._+~%@$-]+){2,}/?"#
    ).unwrap();
    // Registry key: a hive prefix followed by a backslash and sub-key path.
    // Whitespace terminates the key for the same reason as the path scanners.
    static REGISTRY: Regex = Regex::new(
        r#"(?i)\b(?:HKEY_LOCAL_MACHINE|HKEY_CURRENT_USER|HKEY_CLASSES_ROOT|HKEY_USERS|HKEY_CURRENT_CONFIG|HKEY_PERFORMANCE_DATA|HKLM|HKCU|HKCR|HKU|HKCC)\\[^\s"*?<>|]*"#
    ).unwrap();
}

/// Trim trailing whitespace and prose punctuation from a greedily-matched path.
fn trim_path(s: &str) -> &str {
    s.trim_end_matches(|c: char| {
        c.is_whitespace()
            || matches!(
                c,
                '.' | ',' | ';' | ':' | ')' | '(' | '"' | '\'' | '>' | '<'
            )
    })
}

fn scan_path(text: &str) -> BTreeSet<String> {
    let mut set = BTreeSet::new();
    WIN_PATH.with(|re| {
        for m in re.find_iter(text) {
            let p = trim_path(m.as_str());
            // require at least one sub-key/segment beyond the drive/UNC root marker
            let unc = p.starts_with("\\\\");
            if (unc && p.matches('\\').count() >= 3) || (!unc && p.matches('\\').count() >= 2) {
                set.insert(p.to_string());
            }
        }
    });
    UNIX_PATH.with(|re| {
        for m in re.find_iter(text) {
            // skip a URL's "//host/path" tail and any match glued to the previous char
            let before = text[..m.start()].chars().next_back();
            if matches!(before, Some(':') | Some('/')) {
                continue;
            }
            let p = trim_path(m.as_str());
            if p.matches('/').count() >= 2 {
                set.insert(p.to_string());
            }
        }
    });
    set
}

fn scan_registry(text: &str) -> BTreeSet<String> {
    let mut set = BTreeSet::new();
    REGISTRY.with(|re| {
        for m in re.find_iter(text) {
            let k = trim_path(m.as_str());
            if k.matches('\\').count() >= 1 && k.len() > 5 {
                set.insert(k.to_string());
            }
        }
    });
    set
}

fn scan(text: &str, cat: Category) -> BTreeSet<String> {
    match cat {
        Category::Url => scan_url(text),
        Category::Ipv4 => scan_ipv4(text),
        Category::Ipv6 => scan_ipv6(text),
        Category::Email => scan_email(text),
        Category::Domain => scan_domain(text),
        Category::Path => scan_path(text),
        Category::Registry => scan_registry(text),
    }
}

/// Whether a category's values should be defanged when `defang` is on.
fn defangable(cat: Category) -> bool {
    matches!(
        cat,
        Category::Url | Category::Ipv4 | Category::Ipv6 | Category::Email | Category::Domain
    )
}

/// Extract + categorize strings from a memory dump.
///
/// * `dump` — the pasted dump (raw text bytes, or hex per `input_format`).
/// * `input_format` — `text` (default) or `hex`.
/// * `encoding` — `ascii`, `utf16le`, or `both` (default).
/// * `min_length` — minimum printable run length (default 4; clamped to >= 1).
/// * `categories` — comma/space list, or `all` (default).
/// * `defang_out` — defang URLs/IPs/domains/emails in the output.
pub fn run(
    dump: &str,
    input_format: &str,
    encoding: &str,
    min_length: usize,
    categories: &str,
    defang_out: bool,
) -> Result<String, String> {
    if dump.trim().is_empty() {
        return Err("dump is empty — paste a memory/process dump or a hex-encoded dump".into());
    }
    let fmt = InputFormat::parse(input_format)?;
    let enc = Encoding::parse(encoding)?;
    let cats = parse_categories(categories)?;
    let min = min_length.max(1);

    let bytes: Vec<u8> = match fmt {
        InputFormat::Text => dump.as_bytes().to_vec(),
        InputFormat::Hex => decode_hex(dump)?,
    };

    // Step 1 — recover the printable strings.
    let mut strings: Vec<String> = Vec::new();
    if enc.wants_ascii() {
        extract_ascii(&bytes, min, &mut strings);
    }
    if enc.wants_wide() {
        extract_utf16le(&bytes, min, &mut strings);
    }
    let blob = strings.join("\n");

    // Step 2 — categorize.
    let mut sections: Vec<String> = Vec::new();
    let mut grand_total = 0usize;
    for &cat in &cats {
        let mut items: Vec<String> = scan(&blob, cat).into_iter().collect();
        if defang_out && defangable(cat) {
            items = items.iter().map(|v| defang(v)).collect();
        }
        grand_total += items.len();
        let mut section = format!("{} ({}):", cat.label(), items.len());
        if items.is_empty() {
            section.push_str("\n  (none)");
        } else {
            for it in &items {
                section.push_str("\n  ");
                section.push_str(it);
            }
        }
        sections.push(section);
    }

    let header = format!(
        "Extracted {} printable string{} — {} categorized item{}",
        strings.len(),
        if strings.len() == 1 { "" } else { "s" },
        grand_total,
        if grand_total == 1 { "" } else { "s" }
    );
    Ok(format!("{header}\n\n{}", sections.join("\n\n")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_and_categorizes_all() {
        let dump = "GET http://evil.example.com/a\0\0beacon 203.0.113.5\0\
            open C:\\Windows\\System32\\cmd.exe key HKLM\\Software\\Run mail bad@phish.net";
        let out = run(dump, "text", "ascii", 4, "all", false).unwrap();
        assert!(out.contains("http://evil.example.com/a"), "{out}");
        assert!(out.contains("203.0.113.5"), "{out}");
        assert!(out.contains(r"C:\Windows\System32\cmd.exe"), "{out}");
        assert!(out.contains(r"HKLM\Software\Run"), "{out}");
        assert!(out.contains("bad@phish.net"), "{out}");
    }

    #[test]
    fn min_length_filters_noise() {
        // "ok" is length 2, below the default 4 floor; "beacon" survives.
        let out = run("ok\0\0beacon\0", "text", "ascii", 4, "all", false).unwrap();
        assert!(out.starts_with("Extracted 1 printable string"), "{out}");
    }

    #[test]
    fn utf16le_wide_strings() {
        // "http://evil.com/x" encoded as UTF-16LE (each ascii byte followed by 0x00).
        let ascii = "http://evil.com/x";
        let mut wide = Vec::new();
        for b in ascii.bytes() {
            wide.push(b);
            wide.push(0u8);
        }
        // Feed as a hex dump so the null bytes survive the text round-trip.
        let hex: String = wide.iter().map(|b| format!("{b:02x} ")).collect();
        let out = run(&hex, "hex", "utf16le", 4, "url", false).unwrap();
        assert!(out.contains("http://evil.com/x"), "{out}");
    }

    #[test]
    fn hex_input_decodes() {
        // "Visit http://x.io" -> hex, mixed separators + 0x prefixes.
        let text = "Visit http://x.io";
        let hex: String = text
            .bytes()
            .enumerate()
            .map(|(i, b)| {
                if i % 2 == 0 {
                    format!("0x{b:02x} ")
                } else {
                    format!("{b:02x}:")
                }
            })
            .collect();
        let out = run(&hex, "hex", "ascii", 4, "url", false).unwrap();
        assert!(out.contains("http://x.io"), "{out}");
    }

    #[test]
    fn category_filter_restricts() {
        let dump = "1.2.3.4 evil.com bad@evil.com";
        let out = run(dump, "text", "ascii", 3, "ipv4", false).unwrap();
        assert!(out.contains("1.2.3.4"), "{out}");
        assert!(!out.contains("bad@evil.com"), "{out}");
        assert!(!out.contains("Domains"), "{out}");
    }

    #[test]
    fn defang_output() {
        let out = run("visit http://evil.com now", "text", "ascii", 3, "url", true).unwrap();
        assert!(out.contains("hxxp[://]evil[.]com"), "{out}");
        assert!(!out.contains("http://evil.com"), "{out}");
    }

    #[test]
    fn unix_paths_not_from_urls() {
        let dump = "curl http://host.com/a/b/c ran /usr/bin/curl then /etc/passwd";
        let out = run(dump, "text", "ascii", 3, "path", false).unwrap();
        assert!(out.contains("/usr/bin/curl"), "{out}");
        assert!(out.contains("/etc/passwd"), "{out}");
        // the URL's /a/b/c tail must not be reported as a file path
        assert!(!out.contains("/a/b/c"), "{out}");
    }

    #[test]
    fn dedup_and_sort() {
        let out = run("8.8.8.8 8.8.8.8 1.1.1.1", "text", "ascii", 3, "ipv4", false).unwrap();
        let i1 = out.find("1.1.1.1").unwrap();
        let i8 = out.find("8.8.8.8").unwrap();
        assert!(i1 < i8, "{out}");
        assert_eq!(out.matches("8.8.8.8").count(), 1, "{out}");
        assert!(out.contains("IPv4 addresses (2):"), "{out}");
    }

    #[test]
    fn empty_dump_errors() {
        assert!(run("   ", "text", "both", 4, "all", false).is_err());
    }

    #[test]
    fn unknown_category_errors() {
        assert!(run("x", "text", "ascii", 4, "banana", false).is_err());
    }

    #[test]
    fn paths_and_registry_stop_at_prose() {
        // A single-line strings dump: path and registry scanners must stop at the
        // space before the following prose, not greedily swallow it.
        let dump = "GET http://evil.example.com/a from 203.0.113.5 open \
            C:\\Windows\\System32\\cmd.exe key HKLM\\Software\\Run mail bad@phish.net host cdn.badsite.io";
        let out = run(
            dump,
            "text",
            "ascii",
            4,
            "url,ipv4,path,registry,email,domain",
            false,
        )
        .unwrap();
        assert!(out.contains("File paths (1):"), "{out}");
        assert!(out.contains(r"C:\Windows\System32\cmd.exe"), "{out}");
        assert!(!out.contains("cmd.exe key"), "{out}");
        assert!(out.contains("Registry keys (1):"), "{out}");
        assert!(out.contains(r"HKLM\Software\Run"), "{out}");
        assert!(!out.contains("Run mail"), "{out}");
        // the other categories still resolve cleanly
        assert!(out.contains("http://evil.example.com/a"), "{out}");
        assert!(out.contains("203.0.113.5"), "{out}");
        assert!(out.contains("bad@phish.net"), "{out}");
        assert!(out.contains("Domains (1):"), "{out}");
        assert!(out.contains("cdn.badsite.io"), "{out}");
    }

    #[test]
    fn unc_path() {
        let out = run(
            r"copy \\fileserver\share\payload.dll here",
            "text",
            "ascii",
            3,
            "path",
            false,
        )
        .unwrap();
        assert!(out.contains(r"\\fileserver\share\payload.dll"), "{out}");
    }

    #[test]
    fn odd_hex_errors() {
        assert!(run("48 65 6", "hex", "ascii", 4, "all", false).is_err());
    }
}
