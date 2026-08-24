//! url-normalizer core — pure compute, shared by the chat skill block and the
//! web page. No wafer/wasm-bindgen deps.
//!
//! Syntax-based URL normalization per RFC 3986 §6.2.2 applied to the WHOLE URL:
//! the scheme and host are lowercased, a default port is dropped, dot-segments
//! (`/a/../b`) are resolved, percent-encoding is rewritten to one canonical
//! spelling, and the query is sorted so two spellings of the same page collapse
//! to one string. Optional scheme-forcing, `www.` policy, trailing-slash policy,
//! directory-index removal, tracking-parameter removal, fragment removal and
//! cross-line deduplication turn it into the SEO canonical form as well.
//!
//! Sibling tools own the narrow jobs: `url-query-normalizer` for form-style
//! query handling (`+` as space, allow/deny lists), `url-cleaner` for
//! tracking-only stripping, `url-trailing-slash-normalizer` for slash-only
//! passes, `parse-uri` for a component breakdown.

/// Hard cap on the input size, so a paste can't wedge the browser tab.
pub const MAX_BYTES: usize = 1_000_000;
/// Hard cap on how many URLs one run normalizes.
pub const MAX_URLS: usize = 20_000;

/// Query-parameter names dropped by `drop_tracking` (matched case-insensitively).
const TRACKING_EXACT: &[&str] = &[
    "fbclid",
    "gclid",
    "gclsrc",
    "dclid",
    "gbraid",
    "wbraid",
    "msclkid",
    "yclid",
    "ysclid",
    "twclid",
    "ttclid",
    "igshid",
    "mc_eid",
    "mc_cid",
    "vero_id",
    "vero_conv",
    "oly_anon_id",
    "oly_enc_id",
    "wickedid",
    "_openstat",
    "mkt_tok",
    "fb_action_ids",
    "fb_action_types",
    "fb_source",
    "action_object_map",
    "action_type_map",
    "action_ref_map",
    "spm",
    "scm",
    "ref_src",
    "ref_url",
    "s_kwcid",
    "ml_subscriber",
    "ml_subscriber_hash",
    "trk",
    "trkcampaign",
    "__hstc",
    "__hssc",
    "__hsfp",
    "hsctatracking",
    "guccounter",
    "_hsenc",
    "_hsmi",
];

/// Query-parameter name prefixes dropped by `drop_tracking` (analytics families).
const TRACKING_PREFIX: &[&str] = &[
    "utm_", "ga_", "_ga", "pk_", "mtm_", "matomo_", "hsa_", "_hs", "mc_", "oly_", "vero_", "icid",
    "wt_", "cm_", "ns_", "__cft", "__tn",
];

/// Last path segments treated as a server directory index by `drop_index`.
const INDEX_FILES: &[&str] = &[
    "index.html",
    "index.htm",
    "index.php",
    "index.asp",
    "index.aspx",
    "index.jsp",
    "index.cgi",
    "index.shtml",
    "index.xhtml",
    "default.html",
    "default.htm",
    "default.asp",
    "default.aspx",
];

/// The port a scheme implies, which therefore never needs spelling out.
fn default_port(scheme: &str) -> Option<&'static str> {
    Some(match scheme {
        "http" | "ws" => "80",
        "https" | "wss" => "443",
        "ftp" => "21",
        "ftps" => "990",
        "sftp" | "ssh" => "22",
        "telnet" => "23",
        "smtp" => "25",
        "gopher" => "70",
        "pop3" => "110",
        "nntp" => "119",
        "imap" => "143",
        "ldap" => "389",
        "ldaps" => "636",
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// Percent-encoding
// ---------------------------------------------------------------------------

/// How percent-escapes are rewritten.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Enc {
    /// RFC 3986 §6.2.2.2/6.2.2.1: decode escapes of unreserved characters,
    /// uppercase every remaining escape, escape anything illegal.
    Normalize,
    /// Also decode escapes of characters that are legal literals in the
    /// component — including `%2F` in a path, which is what reveals a
    /// disguised traversal once dot-segments are resolved.
    Decode,
    /// Leave every escape exactly as written.
    Preserve,
}

/// RFC 3986 `unreserved`.
fn is_unreserved(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~')
}

/// RFC 3986 `sub-delims`.
fn is_subdelim(b: u8) -> bool {
    matches!(
        b,
        b'!' | b'$' | b'&' | b'\'' | b'(' | b')' | b'*' | b'+' | b',' | b';' | b'='
    )
}

/// RFC 3986 `pchar` minus `pct-encoded`.
fn is_pchar(b: u8) -> bool {
    is_unreserved(b) || is_subdelim(b) || b == b':' || b == b'@'
}

/// Legal literal bytes in a path component.
fn path_ok(b: u8) -> bool {
    is_pchar(b) || b == b'/'
}

/// Legal literal bytes in a fragment.
fn frag_ok(b: u8) -> bool {
    is_pchar(b) || b == b'/' || b == b'?'
}

/// Legal literal bytes in a query parameter NAME. `&` and `=` are excluded so a
/// normalized query re-parses into the same pairs on a second pass.
fn qkey_ok(b: u8) -> bool {
    (is_pchar(b) || b == b'/' || b == b'?') && b != b'&' && b != b'='
}

/// Legal literal bytes in a query parameter VALUE — a value may hold a literal
/// `=` because splitting a pair only ever consumes the first one.
fn qval_ok(b: u8) -> bool {
    (is_pchar(b) || b == b'/' || b == b'?') && b != b'&'
}

/// Legal literal bytes in the `userinfo` component.
fn userinfo_ok(b: u8) -> bool {
    is_unreserved(b) || is_subdelim(b) || b == b':'
}

fn hexval(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

fn push_pct(out: &mut String, b: u8) {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    out.push('%');
    out.push(HEX[(b >> 4) as usize] as char);
    out.push(HEX[(b & 15) as usize] as char);
}

/// Rewrite one component to a single canonical percent-encoding spelling.
/// `ok` says which bytes may appear literally in that component. A `%` that
/// does not introduce two hex digits is a literal percent sign and is escaped
/// as `%25` rather than treated as an error.
fn recode(s: &str, ok: fn(u8) -> bool, mode: Enc) -> String {
    if mode == Enc::Preserve {
        return s.to_string();
    }
    let raw = s.as_bytes();
    let mut out = String::with_capacity(raw.len());
    let mut i = 0;
    while i < raw.len() {
        let b = raw[i];
        if b == b'%' {
            if i + 2 < raw.len() {
                if let (Some(h), Some(l)) = (hexval(raw[i + 1]), hexval(raw[i + 2])) {
                    let d = (h << 4) | l;
                    if is_unreserved(d) || (mode == Enc::Decode && ok(d)) {
                        out.push(d as char);
                    } else {
                        push_pct(&mut out, d);
                    }
                    i += 3;
                    continue;
                }
            }
            push_pct(&mut out, b'%');
            i += 1;
            continue;
        }
        if ok(b) {
            out.push(b as char);
        } else {
            push_pct(&mut out, b);
        }
        i += 1;
    }
    out
}

// ---------------------------------------------------------------------------
// Parsing / rendering
// ---------------------------------------------------------------------------

/// A parsed URI reference. `bare_authority` records that the line named a host
/// without the `//` marker (`example.com/a`), so it renders back the same way
/// unless a scheme is forced onto it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Parts {
    scheme: Option<String>,
    userinfo: Option<String>,
    host: Option<String>,
    port: Option<String>,
    has_authority: bool,
    bare_authority: bool,
    path: String,
    query: Option<String>,
    fragment: Option<String>,
}

/// RFC 3986 `scheme`, minus candidates containing a `.` — those are legal per
/// the grammar but in practice always a bare `host.tld:port`, never a scheme.
fn is_scheme(s: &str) -> bool {
    let mut cs = s.chars();
    match cs.next() {
        Some(c) if c.is_ascii_alphabetic() => {}
        _ => return false,
    }
    cs.all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-'))
}

/// Does this line start with something that reads as `host[:port]`? Used to
/// accept scheme-less lines like `Example.COM/Path` and `localhost:8080/a`.
fn looks_like_host_start(s: &str) -> bool {
    let seg = s.split(['/', '?', '#']).next().unwrap_or("");
    if seg.is_empty() {
        return false;
    }
    let host = match seg.rsplit_once(':') {
        Some((h, p)) if !p.is_empty() && p.chars().all(|c| c.is_ascii_digit()) => h,
        Some(_) => return false,
        None => seg,
    };
    if host.eq_ignore_ascii_case("localhost") {
        return true;
    }
    let labels: Vec<&str> = host.split('.').collect();
    if labels.len() < 2 || labels.iter().any(|l| l.is_empty()) {
        return false;
    }
    if labels
        .iter()
        .all(|l| l.chars().all(|c| c.is_ascii_digit()) && l.len() <= 3)
    {
        return labels.len() == 4; // dotted-quad IPv4
    }
    let tld = labels[labels.len() - 1];
    tld.len() >= 2
        && tld.chars().all(|c| c.is_alphabetic())
        && labels.iter().all(|l| {
            l.chars()
                .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
        })
}

fn parse_authority(auth: &str, p: &mut Parts) -> Result<(), String> {
    let (ui, hostport) = match auth.rfind('@') {
        Some(i) => (Some(auth[..i].to_string()), &auth[i + 1..]),
        None => (None, auth),
    };
    p.userinfo = ui;
    if hostport.starts_with('[') {
        let rb = hostport
            .find(']')
            .ok_or_else(|| format!("IPv6 host literal '{hostport}' is missing its closing ']'"))?;
        p.host = Some(hostport[..=rb].to_string());
        let after = &hostport[rb + 1..];
        if let Some(port) = after.strip_prefix(':') {
            p.port = Some(port.to_string());
        } else if !after.is_empty() {
            return Err(format!(
                "expected ':port' or nothing after the IPv6 host literal, got '{after}'"
            ));
        }
    } else if let Some(i) = hostport.rfind(':') {
        let port = &hostport[i + 1..];
        if port.chars().all(|c| c.is_ascii_digit()) {
            p.host = Some(hostport[..i].to_string());
            p.port = Some(port.to_string());
        } else {
            return Err(format!(
                "expected a numeric port after ':', got '{port}' in authority '{auth}'"
            ));
        }
    } else {
        p.host = Some(hostport.to_string());
    }
    Ok(())
}

fn parse_ref(s: &str) -> Result<Parts, String> {
    let mut p = Parts::default();
    let mut rest = s;
    if let Some(i) = rest.find('#') {
        p.fragment = Some(rest[i + 1..].to_string());
        rest = &rest[..i];
    }
    if let Some(i) = rest.find('?') {
        p.query = Some(rest[i + 1..].to_string());
        rest = &rest[..i];
    }
    if let Some(i) = rest.find(':') {
        let cand = &rest[..i];
        if !cand.contains('/') && is_scheme(cand) {
            p.scheme = Some(cand.to_string());
            rest = &rest[i + 1..];
        }
    }
    if let Some(r) = rest.strip_prefix("//") {
        p.has_authority = true;
        let end = r.find(['/', '?', '#']).unwrap_or(r.len());
        parse_authority(&r[..end], &mut p)?;
        rest = &r[end..];
    } else if p.scheme.is_none() && !rest.starts_with('/') && looks_like_host_start(rest) {
        p.has_authority = true;
        p.bare_authority = true;
        let end = rest.find('/').unwrap_or(rest.len());
        parse_authority(&rest[..end], &mut p)?;
        rest = &rest[end..];
    }
    p.path = rest.to_string();
    Ok(p)
}

fn render(p: &Parts) -> String {
    let mut out = String::with_capacity(p.path.len() + 32);
    if let Some(s) = &p.scheme {
        out.push_str(s);
        out.push(':');
    }
    if p.has_authority {
        if !p.bare_authority {
            out.push_str("//");
        }
        if let Some(u) = &p.userinfo {
            out.push_str(u);
            out.push('@');
        }
        out.push_str(p.host.as_deref().unwrap_or(""));
        if let Some(port) = &p.port {
            out.push(':');
            out.push_str(port);
        }
    }
    out.push_str(&p.path);
    if let Some(q) = &p.query {
        out.push('?');
        out.push_str(q);
    }
    if let Some(f) = &p.fragment {
        out.push('#');
        out.push_str(f);
    }
    out
}

/// RFC 3986 §5.2.4 `remove_dot_segments`.
fn remove_dot_segments(path: &str) -> String {
    let mut input = path.to_string();
    let mut out = String::with_capacity(path.len());
    while !input.is_empty() {
        if input.starts_with("../") {
            input.replace_range(..3, "");
        } else if input.starts_with("./") {
            input.replace_range(..2, "");
        } else if input.starts_with("/./") {
            input.replace_range(..3, "/");
        } else if input == "/." {
            input.replace_range(..2, "/");
        } else if input.starts_with("/../") {
            input.replace_range(..4, "/");
            pop_segment(&mut out);
        } else if input == "/.." {
            input.replace_range(..3, "/");
            pop_segment(&mut out);
        } else if input == "." || input == ".." {
            input.clear();
        } else {
            let start = usize::from(input.starts_with('/'));
            let end = match input[start..].find('/') {
                Some(i) => start + i,
                None => input.len(),
            };
            out.push_str(&input[..end]);
            input.replace_range(..end, "");
        }
    }
    out
}

fn pop_segment(out: &mut String) {
    match out.rfind('/') {
        Some(i) => out.truncate(i),
        None => out.clear(),
    }
}

/// Collapse runs of `/` in a path to a single slash — `/a//b///c` → `/a/b/c`.
/// Most servers serve the collapsed and uncollapsed spellings identically, but
/// crawlers and caches see two different strings.
fn collapse_slash_runs(path: &str) -> String {
    let mut out = String::with_capacity(path.len());
    let mut prev_slash = false;
    for c in path.chars() {
        if c == '/' && prev_slash {
            continue;
        }
        prev_slash = c == '/';
        out.push(c);
    }
    out
}

/// Lowercase a path's letters WITHOUT touching percent-escape hex digits, so
/// `/Docs/Caf%C3%A9` becomes `/docs/caf%C3%A9` and the escape keeps its
/// canonical uppercase spelling.
fn lowercase_outside_escapes(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut hex_left = 0u8;
    for c in s.chars() {
        if hex_left > 0 {
            hex_left -= 1;
            out.push(c);
        } else if c == '%' {
            hex_left = 2;
            out.push(c);
        } else {
            out.extend(c.to_lowercase());
        }
    }
    out
}

/// RFC 3986 §5.3 `merge`.
fn merge_paths(base: &Parts, rpath: &str) -> String {
    if base.has_authority && base.path.is_empty() {
        format!("/{rpath}")
    } else {
        match base.path.rfind('/') {
            Some(i) => format!("{}{}", &base.path[..=i], rpath),
            None => rpath.to_string(),
        }
    }
}

/// RFC 3986 §5.2.2 reference resolution (strict form).
fn resolve(base: &Parts, r: &Parts) -> Parts {
    let mut t = Parts::default();
    if r.scheme.is_some() {
        t.scheme = r.scheme.clone();
        t.has_authority = r.has_authority;
        t.userinfo = r.userinfo.clone();
        t.host = r.host.clone();
        t.port = r.port.clone();
        t.path = remove_dot_segments(&r.path);
        t.query = r.query.clone();
    } else {
        if r.has_authority {
            t.has_authority = true;
            t.userinfo = r.userinfo.clone();
            t.host = r.host.clone();
            t.port = r.port.clone();
            t.path = remove_dot_segments(&r.path);
            t.query = r.query.clone();
        } else {
            if r.path.is_empty() {
                t.path = base.path.clone();
                t.query = r.query.clone().or_else(|| base.query.clone());
            } else if r.path.starts_with('/') {
                t.path = remove_dot_segments(&r.path);
                t.query = r.query.clone();
            } else {
                t.path = remove_dot_segments(&merge_paths(base, &r.path));
                t.query = r.query.clone();
            }
            t.has_authority = base.has_authority;
            t.userinfo = base.userinfo.clone();
            t.host = base.host.clone();
            t.port = base.port.clone();
        }
        t.scheme = base.scheme.clone();
    }
    t.fragment = r.fragment.clone();
    t
}

// ---------------------------------------------------------------------------
// Options
// ---------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum SchemeMode {
    Preserve,
    Https,
    Http,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum WwwMode {
    Preserve,
    Strip,
    Add,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum SlashMode {
    Preserve,
    Add,
    Remove,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum SortMode {
    Key,
    KeyValue,
    None,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Invalid {
    Keep,
    Drop,
    Error,
}

#[derive(Clone, Copy, Debug)]
struct Opts {
    scheme: SchemeMode,
    www: WwwMode,
    strip_default_port: bool,
    dot_segments: bool,
    collapse_slashes: bool,
    lowercase_path: bool,
    encoding: Enc,
    drop_index: bool,
    trailing_slash: SlashMode,
    sort_query: SortMode,
    dedupe_query: bool,
    drop_empty_params: bool,
    drop_tracking: bool,
    drop_fragment: bool,
}

fn pick<T: Copy>(value: &str, name: &str, table: &[(&str, T)]) -> Result<T, String> {
    let v = value.trim();
    if v.is_empty() {
        return Ok(table[0].1);
    }
    for (k, t) in table {
        if v.eq_ignore_ascii_case(k) {
            return Ok(*t);
        }
    }
    let choices: Vec<&str> = table.iter().map(|(k, _)| *k).collect();
    Err(format!(
        "unknown {name} '{v}' — expected one of: {}",
        choices.join(", ")
    ))
}

// ---------------------------------------------------------------------------
// Per-URL normalization
// ---------------------------------------------------------------------------

fn is_tracking(name: &str) -> bool {
    let lower = name.to_ascii_lowercase();
    TRACKING_EXACT.contains(&lower.as_str()) || TRACKING_PREFIX.iter().any(|p| lower.starts_with(p))
}

fn valid_host(h: &str) -> Result<(), String> {
    if h.starts_with('[') {
        return if h.ends_with(']') {
            Ok(())
        } else {
            Err(format!(
                "IPv6 host literal '{h}' is missing its closing ']'"
            ))
        };
    }
    for c in h.chars() {
        if c.is_control()
            || matches!(
                c,
                ' ' | '\\' | '<' | '>' | '"' | '{' | '}' | '|' | '^' | '`' | '/' | '?' | '#' | '@'
            )
        {
            return Err(format!(
                "expected a hostname, got illegal character '{c}' in host '{h}'"
            ));
        }
    }
    Ok(())
}

/// Rebuild the query string: split into pairs, recode, filter, dedupe, sort.
/// Returns `None` when nothing survives, which also drops the `?`.
fn normalize_query(raw: &str, o: &Opts) -> Option<String> {
    let mut pairs: Vec<(String, Option<String>)> = Vec::new();
    for seg in raw.split('&') {
        if seg.is_empty() {
            continue;
        }
        let (k, v) = match seg.find('=') {
            Some(i) => (&seg[..i], Some(&seg[i + 1..])),
            None => (seg, None),
        };
        let key = recode(k, qkey_ok, o.encoding);
        let val = v.map(|v| recode(v, qval_ok, o.encoding));
        if o.drop_tracking && is_tracking(&key) {
            continue;
        }
        if o.drop_empty_params && val.as_deref().unwrap_or("").is_empty() {
            continue;
        }
        pairs.push((key, val));
    }
    if o.dedupe_query {
        let mut seen: Vec<(String, Option<String>)> = Vec::new();
        pairs.retain(|p| {
            if seen.contains(p) {
                false
            } else {
                seen.push(p.clone());
                true
            }
        });
    }
    match o.sort_query {
        SortMode::Key => pairs.sort_by(|a, b| a.0.cmp(&b.0)),
        SortMode::KeyValue => pairs.sort_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1))),
        SortMode::None => {}
    }
    if pairs.is_empty() {
        return None;
    }
    let mut out = String::new();
    for (i, (k, v)) in pairs.iter().enumerate() {
        if i > 0 {
            out.push('&');
        }
        out.push_str(k);
        if let Some(v) = v {
            out.push('=');
            out.push_str(v);
        }
    }
    Some(out)
}

/// Does the last path segment look like a file (so `add` must not slash it)?
fn last_segment_is_file(path: &str) -> bool {
    let seg = path.rsplit('/').next().unwrap_or("");
    match seg.rsplit_once('.') {
        Some((stem, ext)) => {
            !stem.is_empty()
                && !ext.is_empty()
                && ext.len() <= 10
                && ext.chars().all(|c| c.is_ascii_alphanumeric())
                && ext.chars().any(|c| c.is_ascii_alphabetic())
        }
        None => false,
    }
}

fn normalize_one(line: &str, base: Option<&Parts>, o: &Opts) -> Result<String, String> {
    if let Some(c) = line.chars().find(|c| c.is_control()) {
        return Err(format!(
            "expected a URL, got a control character (U+{:04X}) in the line",
            c as u32
        ));
    }
    let mut p = parse_ref(line)?;

    // 1. Reference resolution against an optional base.
    if let Some(b) = base {
        if p.scheme.is_none() {
            let bare = p.bare_authority;
            p = resolve(b, &p);
            p.bare_authority = bare && p.scheme.is_none();
        }
    }

    // 2. Scheme: lowercase, then optionally force http(s).
    if let Some(s) = &p.scheme {
        p.scheme = Some(s.to_ascii_lowercase());
    }
    let original_scheme = p.scheme.clone().unwrap_or_default();
    let forced = match o.scheme {
        SchemeMode::Preserve => None,
        SchemeMode::Https => Some("https"),
        SchemeMode::Http => Some("http"),
    };
    if let Some(f) = forced {
        let web = matches!(p.scheme.as_deref(), Some("http") | Some("https"));
        if web || (p.scheme.is_none() && p.has_authority) {
            p.scheme = Some(f.to_string());
            p.bare_authority = false;
        }
    }
    let scheme = p.scheme.clone().unwrap_or_default();

    // 3. Host: lowercase, drop the FQDN root dot, apply the www policy.
    if let Some(h) = p.host.clone() {
        valid_host(&h)?;
        let mut host = h.to_lowercase();
        if host.len() > 1 && host.ends_with('.') && !host.ends_with("..") {
            host.pop();
        }
        if host.is_empty() && matches!(scheme.as_str(), "http" | "https") {
            return Err(format!(
                "expected a host after '//' in '{line}', got an empty authority"
            ));
        }
        host = apply_www(&host, o.www);
        p.host = Some(host);
    }

    // 4. Port: drop leading zeros, an empty `:`, and the scheme's default.
    // When the scheme is forced (for example http → https), also treat a port
    // that was default for the original scheme as removable SEO noise. That is
    // the common canonical-link expectation for `http://example.com:80/...`
    // when users choose "force https".
    if let Some(port) = p.port.clone() {
        let trimmed = port.trim_start_matches('0');
        let port = if port.is_empty() {
            String::new()
        } else if trimmed.is_empty() {
            "0".to_string()
        } else {
            trimmed.to_string()
        };
        if port.is_empty() {
            p.port = None;
        } else if o.strip_default_port
            && (default_port(&scheme) == Some(port.as_str())
                || default_port(&original_scheme) == Some(port.as_str()))
        {
            p.port = None;
        } else {
            p.port = Some(port);
        }
    }

    // 5. userinfo.
    if let Some(u) = p.userinfo.clone() {
        p.userinfo = Some(recode(&u, userinfo_ok, o.encoding));
    }

    // 6. Path: recode → collapse slash runs → resolve dot-segments →
    //    empty-path rule → index → slash → optional lowercasing.
    let mut path = recode(&p.path, path_ok, o.encoding);
    if o.collapse_slashes {
        path = collapse_slash_runs(&path);
    }
    if o.dot_segments {
        path = remove_dot_segments(&path);
    }
    if p.has_authority && path.is_empty() {
        path = "/".to_string();
    }
    if o.drop_index {
        if let Some(i) = path.rfind('/') {
            let seg = &path[i + 1..];
            if INDEX_FILES.iter().any(|f| seg.eq_ignore_ascii_case(f)) {
                path.truncate(i + 1);
            }
        }
    }
    match o.trailing_slash {
        SlashMode::Preserve => {}
        SlashMode::Add => {
            if path.is_empty() {
                if p.has_authority {
                    path.push('/');
                }
            } else if !path.ends_with('/') && !last_segment_is_file(&path) {
                path.push('/');
            }
        }
        SlashMode::Remove => {
            while path.len() > 1 && path.ends_with('/') {
                path.pop();
            }
        }
    }
    if o.lowercase_path {
        path = lowercase_outside_escapes(&path);
    }
    p.path = path;

    // 7. Query and fragment.
    p.query = p.query.as_deref().and_then(|q| normalize_query(q, o));
    p.fragment = if o.drop_fragment {
        None
    } else {
        p.fragment
            .as_deref()
            .filter(|f| !f.is_empty())
            .map(|f| recode(f, frag_ok, o.encoding))
    };

    Ok(render(&p))
}

fn apply_www(host: &str, mode: WwwMode) -> String {
    match mode {
        WwwMode::Preserve => host.to_string(),
        WwwMode::Strip => match host.strip_prefix("www.") {
            Some(rest) if rest.split('.').filter(|l| !l.is_empty()).count() >= 2 => {
                rest.to_string()
            }
            _ => host.to_string(),
        },
        WwwMode::Add => {
            let is_ip = host.starts_with('[')
                || host
                    .split('.')
                    .all(|l| !l.is_empty() && l.chars().all(|c| c.is_ascii_digit()));
            if host.starts_with("www.") || is_ip || host.split('.').count() != 2 {
                host.to_string()
            } else {
                format!("www.{host}")
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Batch entry point
// ---------------------------------------------------------------------------

/// One input line and what happened to it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Row {
    /// 1-based line number in the input.
    pub line: usize,
    /// The line as given (surrounding whitespace trimmed).
    pub original: String,
    /// The canonical form (equal to `original` when nothing changed).
    pub normalized: String,
    /// `normalized`, `unchanged`, `duplicate` or `invalid`.
    pub action: &'static str,
}

fn csv_field(s: &str) -> String {
    if s.contains(['"', ',', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Normalize a batch of URLs. Every parameter is the raw surface value; empty
/// strings select the documented default so chat, CLI and page agree.
#[allow(clippy::too_many_arguments)]
pub fn run(
    urls: &str,
    base: &str,
    scheme: &str,
    www: &str,
    strip_default_port: bool,
    dot_segments: bool,
    collapse_slashes: bool,
    lowercase_path: bool,
    encoding: &str,
    drop_index: bool,
    trailing_slash: &str,
    sort_query: &str,
    dedupe_query: bool,
    drop_empty_params: bool,
    drop_tracking: bool,
    drop_fragment: bool,
    dedupe_urls: bool,
    on_invalid: &str,
    output: &str,
) -> Result<String, String> {
    if urls.len() > MAX_BYTES {
        return Err(format!(
            "input is {} bytes, over the {MAX_BYTES}-byte limit — split the list and run it in parts",
            urls.len()
        ));
    }
    let o = Opts {
        scheme: pick(
            scheme,
            "scheme mode",
            &[
                ("preserve", SchemeMode::Preserve),
                ("https", SchemeMode::Https),
                ("http", SchemeMode::Http),
            ],
        )?,
        www: pick(
            www,
            "www mode",
            &[
                ("preserve", WwwMode::Preserve),
                ("strip", WwwMode::Strip),
                ("add", WwwMode::Add),
            ],
        )?,
        strip_default_port,
        dot_segments,
        collapse_slashes,
        lowercase_path,
        encoding: pick(
            encoding,
            "encoding mode",
            &[
                ("normalize", Enc::Normalize),
                ("decode", Enc::Decode),
                ("preserve", Enc::Preserve),
            ],
        )?,
        drop_index,
        trailing_slash: pick(
            trailing_slash,
            "trailing slash mode",
            &[
                ("preserve", SlashMode::Preserve),
                ("add", SlashMode::Add),
                ("remove", SlashMode::Remove),
            ],
        )?,
        sort_query: pick(
            sort_query,
            "query sort mode",
            &[
                ("key", SortMode::Key),
                ("key-value", SortMode::KeyValue),
                ("none", SortMode::None),
            ],
        )?,
        dedupe_query,
        drop_empty_params,
        drop_tracking,
        drop_fragment,
    };
    let invalid_mode = pick(
        on_invalid,
        "invalid-line mode",
        &[
            ("keep", Invalid::Keep),
            ("drop", Invalid::Drop),
            ("error", Invalid::Error),
        ],
    )?;
    let out_mode = pick(
        output,
        "output mode",
        &[
            ("urls", 0u8),
            ("changed", 1u8),
            ("report", 2u8),
            ("summary", 3u8),
        ],
    )?;

    let base_parts = match base.trim() {
        "" => None,
        b => {
            let mut bp =
                parse_ref(b).map_err(|e| format!("base URL '{b}' could not be parsed: {e}"))?;
            if bp.scheme.is_none() && !bp.has_authority {
                return Err(format!(
                    "expected an absolute base URL like 'https://example.com/docs/', got '{b}'"
                ));
            }
            if let Some(s) = &bp.scheme {
                bp.scheme = Some(s.to_ascii_lowercase());
            }
            if let Some(h) = &bp.host {
                bp.host = Some(h.to_lowercase());
            }
            bp.bare_authority = false;
            Some(bp)
        }
    };

    let lines: Vec<&str> = urls
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    if lines.is_empty() {
        return Err(
            "no URLs given — paste at least one URL, one per line (e.g. 'https://example.com/a')"
                .to_string(),
        );
    }
    if lines.len() > MAX_URLS {
        return Err(format!(
            "got {} URLs, over the {MAX_URLS}-URL limit — split the list and run it in parts",
            lines.len()
        ));
    }

    let mut rows: Vec<Row> = Vec::with_capacity(lines.len());
    let mut seen: Vec<String> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let n = i + 1;
        match normalize_one(line, base_parts.as_ref(), &o) {
            Err(e) => {
                if invalid_mode == Invalid::Error {
                    return Err(format!("line {n}: {e}"));
                }
                rows.push(Row {
                    line: n,
                    original: (*line).to_string(),
                    normalized: (*line).to_string(),
                    action: "invalid",
                });
            }
            Ok(norm) => {
                let action = if dedupe_urls && seen.iter().any(|s| s == &norm) {
                    "duplicate"
                } else if norm == *line {
                    "unchanged"
                } else {
                    "normalized"
                };
                if dedupe_urls && action != "duplicate" {
                    seen.push(norm.clone());
                }
                rows.push(Row {
                    line: n,
                    original: (*line).to_string(),
                    normalized: norm,
                    action,
                });
            }
        }
    }

    Ok(match out_mode {
        0 => rows
            .iter()
            .filter(|r| {
                r.action != "duplicate" && !(r.action == "invalid" && invalid_mode == Invalid::Drop)
            })
            .map(|r| r.normalized.clone())
            .collect::<Vec<_>>()
            .join("\n"),
        1 => rows
            .iter()
            .filter(|r| r.action == "normalized")
            .map(|r| r.normalized.clone())
            .collect::<Vec<_>>()
            .join("\n"),
        2 => {
            let mut s = String::from("line,original,normalized,action");
            for r in &rows {
                s.push('\n');
                s.push_str(&format!(
                    "{},{},{},{}",
                    r.line,
                    csv_field(&r.original),
                    csv_field(&r.normalized),
                    r.action
                ));
            }
            s
        }
        _ => {
            let count = |a: &str| rows.iter().filter(|r| r.action == a).count();
            let changed = count("normalized");
            let unchanged = count("unchanged");
            let duplicates = count("duplicate");
            let invalid = count("invalid");
            format!(
                "metric,value\nlines_in,{}\nchanged,{changed}\nunchanged,{unchanged}\nduplicates,{duplicates}\ninvalid,{invalid}\nurls_out,{}",
                rows.len(),
                changed + unchanged
            )
        }
    })
}

/// Convenience wrapper used by the tests: every option at its default.
#[cfg(test)]
fn defaults(urls: &str) -> Result<String, String> {
    run(
        urls,
        "",
        "preserve",
        "preserve",
        true,
        true,
        false,
        false,
        "normalize",
        false,
        "preserve",
        "key",
        false,
        false,
        false,
        false,
        false,
        "keep",
        "urls",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn happy_path_canonicalizes_scheme_host_port_dots_and_query() {
        assert_eq!(
            defaults("HTTP://User@Example.COM:80/a/b/../c?b=2&a=1#Frag").unwrap(),
            "http://User@example.com/a/c?a=1&b=2#Frag"
        );
    }

    #[test]
    fn error_on_empty_input() {
        let e = defaults("   \n\n").unwrap_err();
        assert!(e.contains("no URLs given"), "{e}");
    }

    #[test]
    fn error_on_unknown_enum_value() {
        let e = run(
            "https://example.com/",
            "",
            "ftp",
            "preserve",
            true,
            true,
            false,
            false,
            "normalize",
            false,
            "preserve",
            "key",
            false,
            false,
            false,
            false,
            false,
            "keep",
            "urls",
        )
        .unwrap_err();
        assert!(e.contains("unknown scheme mode 'ftp'"), "{e}");
        assert!(e.contains("preserve, https, http"), "{e}");
    }

    #[test]
    fn error_mode_names_the_offending_line() {
        let e = run(
            "https://example.com/\nhttps://ex ample.com/",
            "",
            "preserve",
            "preserve",
            true,
            true,
            false,
            false,
            "normalize",
            false,
            "preserve",
            "key",
            false,
            false,
            false,
            false,
            false,
            "error",
            "urls",
        )
        .unwrap_err();
        assert!(e.starts_with("line 2:"), "{e}");
        assert!(e.contains("illegal character ' '"), "{e}");
    }

    #[test]
    fn percent_encoding_is_canonicalized() {
        // %2d is unreserved → decoded; %c3%a9 is not → uppercased; a raw space
        // and a lone '%' are escaped.
        assert_eq!(
            defaults("https://example.com/a%2db/caf%c3%a9 x/100%").unwrap(),
            "https://example.com/a-b/caf%C3%A9%20x/100%25"
        );
    }

    #[test]
    fn decode_mode_reveals_an_encoded_traversal() {
        let got = run(
            "https://example.com/files/a%2F%2E%2E%2Fetc/passwd",
            "",
            "preserve",
            "preserve",
            true,
            true,
            false,
            false,
            "decode",
            false,
            "preserve",
            "key",
            false,
            false,
            false,
            false,
            false,
            "keep",
            "urls",
        )
        .unwrap();
        assert_eq!(got, "https://example.com/files/etc/passwd");
    }

    #[test]
    fn preserve_encoding_only_reorders() {
        let got = run(
            "https://example.com/a%2db?b=%2d&a=1",
            "",
            "preserve",
            "preserve",
            true,
            true,
            false,
            false,
            "preserve",
            false,
            "preserve",
            "key",
            false,
            false,
            false,
            false,
            false,
            "keep",
            "urls",
        )
        .unwrap();
        assert_eq!(got, "https://example.com/a%2db?a=1&b=%2d");
    }

    #[test]
    fn ipv6_literal_and_default_port_survive_correctly() {
        assert_eq!(
            defaults("HTTPS://[2001:DB8::1]:443/A").unwrap(),
            "https://[2001:db8::1]/A"
        );
        assert_eq!(
            defaults("https://[2001:db8::1]:8443/a").unwrap(),
            "https://[2001:db8::1]:8443/a"
        );
    }

    #[test]
    fn bare_host_line_stays_schemeless_until_a_scheme_is_forced() {
        assert_eq!(defaults("Example.COM/A/./B").unwrap(), "example.com/A/B");
        let got = run(
            "Example.COM/A",
            "",
            "https",
            "preserve",
            true,
            true,
            false,
            false,
            "normalize",
            false,
            "preserve",
            "key",
            false,
            false,
            false,
            false,
            false,
            "keep",
            "urls",
        )
        .unwrap();
        assert_eq!(got, "https://example.com/A");
    }

    #[test]
    fn www_and_index_and_slash_policies_apply() {
        let got = run(
            "http://WWW.Example.com/blog/index.html",
            "",
            "https",
            "strip",
            true,
            true,
            false,
            false,
            "normalize",
            true,
            "add",
            "key",
            false,
            false,
            false,
            false,
            false,
            "keep",
            "urls",
        )
        .unwrap();
        assert_eq!(got, "https://example.com/blog/");
        // `add` never slashes a file-looking last segment.
        let got = run(
            "https://example.com/sitemap.xml",
            "",
            "preserve",
            "preserve",
            true,
            true,
            false,
            false,
            "normalize",
            false,
            "add",
            "key",
            false,
            false,
            false,
            false,
            false,
            "keep",
            "urls",
        )
        .unwrap();
        assert_eq!(got, "https://example.com/sitemap.xml");
    }

    #[test]
    fn www_add_only_touches_a_bare_apex() {
        assert_eq!(apply_www("example.com", WwwMode::Add), "www.example.com");
        assert_eq!(
            apply_www("blog.example.com", WwwMode::Add),
            "blog.example.com"
        );
        assert_eq!(apply_www("192.168.0.1", WwwMode::Add), "192.168.0.1");
        assert_eq!(apply_www("www.example.com", WwwMode::Strip), "example.com");
        assert_eq!(apply_www("www.com", WwwMode::Strip), "www.com");
    }

    #[test]
    fn query_filters_dedupe_and_sort_modes() {
        let got = run(
            "https://example.com/p?utm_source=x&b=2&a=1&a=1&c=&tag=b&tag=a",
            "",
            "preserve",
            "preserve",
            true,
            true,
            false,
            false,
            "normalize",
            false,
            "preserve",
            "key-value",
            true,
            true,
            true,
            false,
            false,
            "keep",
            "urls",
        )
        .unwrap();
        assert_eq!(got, "https://example.com/p?a=1&b=2&tag=a&tag=b");
        // Every parameter removed → the '?' goes with them.
        let got = run(
            "https://example.com/p?utm_source=x",
            "",
            "preserve",
            "preserve",
            true,
            true,
            false,
            false,
            "normalize",
            false,
            "preserve",
            "key",
            false,
            false,
            true,
            false,
            false,
            "keep",
            "urls",
        )
        .unwrap();
        assert_eq!(got, "https://example.com/p");
    }

    #[test]
    fn fragment_removal_and_empty_fragment() {
        assert_eq!(
            defaults("https://example.com/a#").unwrap(),
            "https://example.com/a"
        );
        let got = run(
            "https://example.com/a#top",
            "",
            "preserve",
            "preserve",
            true,
            true,
            false,
            false,
            "normalize",
            false,
            "preserve",
            "key",
            false,
            false,
            false,
            true,
            false,
            "keep",
            "urls",
        )
        .unwrap();
        assert_eq!(got, "https://example.com/a");
    }

    #[test]
    fn base_url_resolves_relative_references() {
        let got = run(
            "../images/logo.png\n/about\n?page=2\nhttps://other.example/x",
            "https://example.com/docs/guide/index.html",
            "preserve",
            "preserve",
            true,
            true,
            false,
            false,
            "normalize",
            false,
            "preserve",
            "key",
            false,
            false,
            false,
            false,
            false,
            "keep",
            "urls",
        )
        .unwrap();
        assert_eq!(
            got,
            "https://example.com/docs/images/logo.png\nhttps://example.com/about\nhttps://example.com/docs/guide/index.html?page=2\nhttps://other.example/x"
        );
    }

    #[test]
    fn base_must_be_absolute() {
        let e = run(
            "a",
            "docs/",
            "preserve",
            "preserve",
            true,
            true,
            false,
            false,
            "normalize",
            false,
            "preserve",
            "key",
            false,
            false,
            false,
            false,
            false,
            "keep",
            "urls",
        )
        .unwrap_err();
        assert!(e.contains("expected an absolute base URL"), "{e}");
    }

    #[test]
    fn dedupe_urls_keeps_the_first_spelling() {
        let got = run(
            "https://Example.com/a?b=1&a=2\nhttps://example.com/a?a=2&b=1\nhttps://example.com/b",
            "",
            "preserve",
            "preserve",
            true,
            true,
            false,
            false,
            "normalize",
            false,
            "preserve",
            "key",
            false,
            false,
            false,
            false,
            true,
            "keep",
            "urls",
        )
        .unwrap();
        assert_eq!(got, "https://example.com/a?a=2&b=1\nhttps://example.com/b");
    }

    #[test]
    fn output_modes_changed_report_and_summary() {
        let input = "https://Example.com/A\nhttps://example.com/b";
        let changed = run(
            input,
            "",
            "preserve",
            "preserve",
            true,
            true,
            false,
            false,
            "normalize",
            false,
            "preserve",
            "key",
            false,
            false,
            false,
            false,
            false,
            "keep",
            "changed",
        )
        .unwrap();
        assert_eq!(changed, "https://example.com/A");
        let report = run(
            input,
            "",
            "preserve",
            "preserve",
            true,
            true,
            false,
            false,
            "normalize",
            false,
            "preserve",
            "key",
            false,
            false,
            false,
            false,
            false,
            "keep",
            "report",
        )
        .unwrap();
        assert_eq!(
            report,
            "line,original,normalized,action\n1,https://Example.com/A,https://example.com/A,normalized\n2,https://example.com/b,https://example.com/b,unchanged"
        );
        let summary = run(
            input,
            "",
            "preserve",
            "preserve",
            true,
            true,
            false,
            false,
            "normalize",
            false,
            "preserve",
            "key",
            false,
            false,
            false,
            false,
            false,
            "keep",
            "summary",
        )
        .unwrap();
        assert_eq!(
            summary,
            "metric,value\nlines_in,2\nchanged,1\nunchanged,1\nduplicates,0\ninvalid,0\nurls_out,2"
        );
    }

    #[test]
    fn collapse_slashes_and_lowercase_path_are_opt_in() {
        // Both off by default: empty segments and path case survive.
        assert_eq!(
            defaults("https://example.com/a//b///C").unwrap(),
            "https://example.com/a//b///C"
        );
        let got = run(
            "https://example.com/a//b///C",
            "",
            "preserve",
            "preserve",
            true,
            true,
            true,
            true,
            "normalize",
            false,
            "preserve",
            "key",
            false,
            false,
            false,
            false,
            false,
            "keep",
            "urls",
        )
        .unwrap();
        assert_eq!(got, "https://example.com/a/b/c");
    }

    #[test]
    fn lowercasing_the_path_leaves_escape_hex_uppercase() {
        let got = run(
            "https://example.com/Docs/Caf%c3%a9",
            "",
            "preserve",
            "preserve",
            true,
            true,
            false,
            true,
            "normalize",
            false,
            "preserve",
            "key",
            false,
            false,
            false,
            false,
            false,
            "keep",
            "urls",
        )
        .unwrap();
        assert_eq!(got, "https://example.com/docs/caf%C3%A9");
    }

    #[test]
    fn invalid_lines_are_kept_or_dropped() {
        let input = "https://example.com/a\nhttps://ex ample.com/b";
        let kept = run(
            input,
            "",
            "preserve",
            "preserve",
            true,
            true,
            false,
            false,
            "normalize",
            false,
            "preserve",
            "key",
            false,
            false,
            false,
            false,
            false,
            "keep",
            "urls",
        )
        .unwrap();
        assert_eq!(kept, "https://example.com/a\nhttps://ex ample.com/b");
        let dropped = run(
            input,
            "",
            "preserve",
            "preserve",
            true,
            true,
            false,
            false,
            "normalize",
            false,
            "preserve",
            "key",
            false,
            false,
            false,
            false,
            false,
            "drop",
            "urls",
        )
        .unwrap();
        assert_eq!(dropped, "https://example.com/a");
    }

    #[test]
    fn non_web_schemes_are_left_alone_by_scheme_forcing() {
        let got = run(
            "MAILTO:Ann@Example.com",
            "",
            "https",
            "preserve",
            true,
            true,
            false,
            false,
            "normalize",
            false,
            "preserve",
            "key",
            false,
            false,
            false,
            false,
            false,
            "keep",
            "urls",
        )
        .unwrap();
        assert_eq!(got, "mailto:Ann@Example.com");
    }

    #[test]
    fn port_zero_padding_and_empty_port() {
        assert_eq!(
            defaults("https://example.com:0443/a").unwrap(),
            "https://example.com/a"
        );
        assert_eq!(
            defaults("https://example.com:/a").unwrap(),
            "https://example.com/a"
        );
        assert_eq!(
            defaults("https://example.com:8443/a").unwrap(),
            "https://example.com:8443/a"
        );
    }

    #[test]
    fn keeping_a_default_port_is_possible() {
        let got = run(
            "https://example.com:443/a",
            "",
            "preserve",
            "preserve",
            false,
            true,
            false,
            false,
            "normalize",
            false,
            "preserve",
            "key",
            false,
            false,
            false,
            false,
            false,
            "keep",
            "urls",
        )
        .unwrap();
        assert_eq!(got, "https://example.com:443/a");
    }

    #[test]
    fn dot_segments_can_be_left_alone() {
        let got = run(
            "https://example.com/a/b/../c",
            "",
            "preserve",
            "preserve",
            true,
            false,
            false,
            false,
            "normalize",
            false,
            "preserve",
            "key",
            false,
            false,
            false,
            false,
            false,
            "keep",
            "urls",
        )
        .unwrap();
        assert_eq!(got, "https://example.com/a/b/../c");
    }

    #[test]
    fn empty_path_becomes_root_and_remove_keeps_it() {
        assert_eq!(
            defaults("https://example.com").unwrap(),
            "https://example.com/"
        );
        let got = run(
            "https://example.com/blog/",
            "",
            "preserve",
            "preserve",
            true,
            true,
            false,
            false,
            "normalize",
            false,
            "remove",
            "key",
            false,
            false,
            false,
            false,
            false,
            "keep",
            "urls",
        )
        .unwrap();
        assert_eq!(got, "https://example.com/blog");
        let got = run(
            "https://example.com/",
            "",
            "preserve",
            "preserve",
            true,
            true,
            false,
            false,
            "normalize",
            false,
            "remove",
            "key",
            false,
            false,
            false,
            false,
            false,
            "keep",
            "urls",
        )
        .unwrap();
        assert_eq!(got, "https://example.com/");
    }

    #[test]
    fn over_the_url_cap_is_an_error() {
        let many = (0..MAX_URLS + 1)
            .map(|i| format!("https://example.com/{i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let e = defaults(&many).unwrap_err();
        assert!(e.contains("over the 20000-URL limit"), "{e}");
    }

    #[test]
    fn at_the_url_cap_is_fine() {
        let many = (0..MAX_URLS)
            .map(|i| format!("https://example.com/{i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let out = defaults(&many).unwrap();
        assert_eq!(out.lines().count(), MAX_URLS);
    }

    #[test]
    fn csv_report_quotes_a_comma_bearing_url() {
        let report = run(
            "https://example.com/a?x=1,2",
            "",
            "preserve",
            "preserve",
            true,
            true,
            false,
            false,
            "normalize",
            false,
            "preserve",
            "key",
            false,
            false,
            false,
            false,
            false,
            "keep",
            "report",
        )
        .unwrap();
        assert!(
            report.contains("\"https://example.com/a?x=1,2\""),
            "{report}"
        );
    }
}
