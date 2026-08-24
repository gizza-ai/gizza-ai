//! ip-address-validate core — validate, canonicalize and classify IPv4/IPv6
//! addresses. Pure compute (`std::net` only), shared by the chat skill block
//! and the web page.
//!
//! For every input line the engine:
//! * splits off an optional `[...]` bracket wrapper, `%zone` id, `/prefix`
//!   length and `:port`;
//! * validates the address itself, with a *specific* reason when it fails
//!   ("octet 4 (256) is out of range 0-255", "'::' may appear only once");
//! * canonicalizes it — IPv4 octets lose leading zeros, IPv6 is rewritten to
//!   the RFC 5952 lowercase compressed form (or fully expanded on request);
//! * classifies it against the IANA special-purpose registries (private,
//!   loopback, link-local, multicast, documentation, CGNAT, ULA, Teredo,
//!   6to4, NAT64, benchmarking, reserved, global);
//! * derives the integer/hex/binary value and the reverse-DNS pointer name.
//!
//! Six output shapes are available: an aligned `report`, a `table` CSV, a
//! `json` array, the `valid` canonical list, the `invalid` list with reasons,
//! and a `summary` CSV of the totals.

use std::collections::BTreeMap;
use std::collections::HashSet;
use std::net::{Ipv4Addr, Ipv6Addr};

/// Maximum number of non-blank input lines accepted in one run.
pub const MAX_ADDRESSES: usize = 5_000;
/// Maximum size of the whole input blob, in bytes.
pub const MAX_BYTES: usize = 500_000;

/// Width the report labels are padded to (longest is "Embedded IPv4:").
const LABEL_WIDTH: usize = 16;

/// One fully analysed input line.
struct Entry {
    line: usize,
    input: String,
    valid: bool,
    version: &'static str,
    /// Canonical address text in the requested IPv6 form (no zone/prefix/port).
    canonical: String,
    /// Fully expanded IPv6 form; empty for IPv4.
    expanded: String,
    kind: &'static str,
    scope: String,
    routable: bool,
    integer: String,
    hex: String,
    /// Dotted-binary; IPv4 only.
    binary: String,
    reverse: String,
    /// Legacy classful letter; IPv4 only.
    class: &'static str,
    prefix: Option<u32>,
    port: Option<u32>,
    zone: String,
    /// IPv4 address embedded in a mapped/6to4/NAT64 IPv6 address.
    embedded: String,
    error: String,
    duplicate: bool,
}

impl Entry {
    fn invalid(line: usize, input: &str, error: String) -> Self {
        Entry {
            line,
            input: input.to_string(),
            valid: false,
            version: "",
            canonical: String::new(),
            expanded: String::new(),
            kind: "",
            scope: String::new(),
            routable: false,
            integer: String::new(),
            hex: String::new(),
            binary: String::new(),
            reverse: String::new(),
            class: "",
            prefix: None,
            port: None,
            zone: String::new(),
            embedded: String::new(),
            error,
            duplicate: false,
        }
    }

    /// The canonical address with its zone, prefix and port put back on — the
    /// round-trippable form emitted by `output = "valid"`.
    fn canonical_full(&self) -> String {
        let mut s = self.canonical.clone();
        if !self.zone.is_empty() {
            s.push('%');
            s.push_str(&self.zone);
        }
        if let Some(p) = self.prefix {
            s = format!("{s}/{p}");
        }
        if let Some(port) = self.port {
            s = if self.version == "IPv6" {
                format!("[{s}]:{port}")
            } else {
                format!("{s}:{port}")
            };
        }
        s
    }
}

/// Validate, canonicalize and classify every address in `addresses`.
///
/// * `family` — `any` | `ipv4` | `ipv6`; a mismatch is reported as invalid.
/// * `allow_prefix` / `allow_port` — accept a `/24` suffix or a `:8080` port.
/// * `allow_leading_zeros` — accept `192.168.01.1` as decimal instead of
///   rejecting it as ambiguous.
/// * `ipv6_form` — `compressed` (RFC 5952) | `expanded` for the canonical text.
/// * `dedupe` — drop later lines that canonicalize to an earlier result.
/// * `output` — `report` | `table` | `json` | `valid` | `invalid` | `summary`.
#[allow(clippy::too_many_arguments)]
pub fn run(
    addresses: &str,
    family: &str,
    allow_prefix: bool,
    allow_port: bool,
    allow_leading_zeros: bool,
    ipv6_form: &str,
    dedupe: bool,
    output: &str,
) -> Result<String, String> {
    let family = pick(family, "family", "any", &["any", "ipv4", "ipv6"])?;
    let ipv6_form = pick(ipv6_form, "ipv6_form", "compressed", &["compressed", "expanded"])?;
    let output = pick(
        output,
        "output",
        "report",
        &["report", "table", "json", "valid", "invalid", "summary"],
    )?;

    if addresses.len() > MAX_BYTES {
        return Err(format!(
            "input is too large: {} bytes, limit is {MAX_BYTES} bytes",
            addresses.len()
        ));
    }

    let lines: Vec<&str> = addresses
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .collect();
    if lines.is_empty() {
        return Err(
            "input is empty: provide at least one IP address, e.g. 192.0.2.1 or 2001:db8::1".into(),
        );
    }
    if lines.len() > MAX_ADDRESSES {
        return Err(format!(
            "too many addresses: {} lines, limit is {MAX_ADDRESSES}",
            lines.len()
        ));
    }

    let mut seen: HashSet<String> = HashSet::new();
    let mut entries: Vec<Entry> = Vec::with_capacity(lines.len());
    for (i, raw) in lines.iter().enumerate() {
        let mut e = analyse(
            i + 1,
            raw,
            &family,
            allow_prefix,
            allow_port,
            allow_leading_zeros,
            &ipv6_form,
        );
        if e.valid && !seen.insert(e.canonical_full()) {
            e.duplicate = true;
        }
        entries.push(e);
    }

    let kept: Vec<&Entry> = if dedupe {
        entries.iter().filter(|e| !e.duplicate).collect()
    } else {
        entries.iter().collect()
    };

    Ok(match output.as_str() {
        "table" => render_table(&kept),
        "json" => render_json(&kept),
        "valid" => render_valid(&kept),
        "invalid" => render_invalid(&kept),
        "summary" => render_summary(&entries),
        _ => render_report(&kept),
    })
}

/// Normalize a blank-or-set enum argument, or explain the accepted values.
fn pick(value: &str, name: &str, default: &str, allowed: &[&str]) -> Result<String, String> {
    let v = value.trim().to_ascii_lowercase();
    let v = if v.is_empty() { default.to_string() } else { v };
    if allowed.contains(&v.as_str()) {
        Ok(v)
    } else {
        Err(format!(
            "invalid {name} {value:?}: expected one of {}",
            allowed.join(", ")
        ))
    }
}

// ---------------------------------------------------------------- parsing ---

/// Split one raw line into address / zone / prefix / port, then validate and
/// classify the address part.
fn analyse(
    line: usize,
    raw: &str,
    family: &str,
    allow_prefix: bool,
    allow_port: bool,
    allow_leading_zeros: bool,
    ipv6_form: &str,
) -> Entry {
    let mut rest = raw;
    let mut host: String;
    let mut port: Option<u32> = None;
    let mut prefix: Option<u32> = None;

    if let Some(stripped) = rest.strip_prefix('[') {
        // Bracketed IPv6 authority: [2001:db8::1]:443 or [2001:db8::1]/64.
        let Some(close) = stripped.find(']') else {
            return Entry::invalid(
                line,
                raw,
                "unterminated '[' — a bracketed IPv6 address needs a closing ']', e.g. [2001:db8::1]:443".into(),
            );
        };
        host = stripped[..close].to_string();
        rest = &stripped[close + 1..];
        if let Some(p) = rest.strip_prefix(':') {
            match parse_port(p, allow_port) {
                Ok(v) => port = Some(v),
                Err(e) => return Entry::invalid(line, raw, e),
            }
        } else if let Some(p) = rest.strip_prefix('/') {
            match parse_prefix_num(p, allow_prefix) {
                Ok(v) => prefix = Some(v),
                Err(e) => return Entry::invalid(line, raw, e),
            }
        } else if !rest.is_empty() {
            return Entry::invalid(
                line,
                raw,
                format!("unexpected text {rest:?} after the closing ']'"),
            );
        }
    } else {
        // Optional /prefix comes off first, then an unambiguous :port.
        if let Some(slash) = rest.rfind('/') {
            match parse_prefix_num(&rest[slash + 1..], allow_prefix) {
                Ok(v) => prefix = Some(v),
                Err(e) => return Entry::invalid(line, raw, e),
            }
            rest = &rest[..slash];
        }
        // A single ':' can only be a port separator — two or more mean IPv6.
        if rest.matches(':').count() == 1 {
            let (h, p) = rest.split_once(':').unwrap();
            match parse_port(p, allow_port) {
                Ok(v) => port = Some(v),
                Err(e) => return Entry::invalid(line, raw, e),
            }
            host = h.to_string();
        } else {
            host = rest.to_string();
        }
    }

    // Scope/zone id, e.g. fe80::1%eth0.
    let mut zone = String::new();
    if let Some((h, z)) = host.split_once('%') {
        if z.is_empty() {
            return Entry::invalid(
                line,
                raw,
                "empty zone id after '%' — write the interface, e.g. fe80::1%eth0".into(),
            );
        }
        zone = z.to_string();
        host = h.to_string();
    }

    if host.is_empty() {
        return Entry::invalid(line, raw, "no address before the separator".into());
    }

    let looks_v6 = host.contains(':');
    if looks_v6 {
        finish_v6(
            line, raw, &host, zone, prefix, port, family, ipv6_form, allow_prefix,
        )
    } else {
        finish_v4(
            line,
            raw,
            &host,
            zone,
            prefix,
            port,
            family,
            allow_leading_zeros,
            allow_prefix,
        )
    }
}

fn parse_port(text: &str, allow_port: bool) -> Result<u32, String> {
    if !allow_port {
        return Err(format!(
            "a ':{text}' port suffix is not accepted here — turn on 'Allow :port suffix' to parse it"
        ));
    }
    if text.is_empty() {
        return Err("empty port after ':'".into());
    }
    if !text.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!("port {text:?} is not a number"));
    }
    match text.parse::<u32>() {
        Ok(p) if p <= 65535 => Ok(p),
        _ => Err(format!("port {text} is out of range 0-65535")),
    }
}

fn parse_prefix_num(text: &str, allow_prefix: bool) -> Result<u32, String> {
    if !allow_prefix {
        return Err(format!(
            "a '/{text}' prefix suffix is not accepted here — turn on 'Allow /prefix suffix' to parse it"
        ));
    }
    if text.is_empty() {
        return Err("empty prefix length after '/'".into());
    }
    if !text.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!("prefix length {text:?} is not a number"));
    }
    text.parse::<u32>()
        .map_err(|_| format!("prefix length {text} is out of range"))
}

#[allow(clippy::too_many_arguments)]
fn finish_v4(
    line: usize,
    raw: &str,
    host: &str,
    zone: String,
    prefix: Option<u32>,
    port: Option<u32>,
    family: &str,
    allow_leading_zeros: bool,
    _allow_prefix: bool,
) -> Entry {
    let ip = match parse_v4(host, allow_leading_zeros) {
        Ok(ip) => ip,
        Err(e) => return Entry::invalid(line, raw, e),
    };
    if family == "ipv6" {
        return Entry::invalid(
            line,
            raw,
            "this is an IPv4 address but the family filter is set to IPv6".into(),
        );
    }
    if let Some(p) = prefix {
        if p > 32 {
            return Entry::invalid(
                line,
                raw,
                format!("prefix length /{p} is out of range for IPv4 (0-32)"),
            );
        }
    }
    if !zone.is_empty() {
        return Entry::invalid(
            line,
            raw,
            "a '%zone' scope id is only meaningful on an IPv6 address".into(),
        );
    }

    let o = ip.octets();
    let n = u32::from(ip);
    let (kind, scope, routable) = classify_v4(ip);
    Entry {
        line,
        input: raw.to_string(),
        valid: true,
        version: "IPv4",
        canonical: ip.to_string(),
        expanded: String::new(),
        kind,
        scope,
        routable,
        integer: n.to_string(),
        hex: format!("0x{n:08x}"),
        binary: format!("{:08b}.{:08b}.{:08b}.{:08b}", o[0], o[1], o[2], o[3]),
        reverse: format!("{}.{}.{}.{}.in-addr.arpa", o[3], o[2], o[1], o[0]),
        class: class_v4(o[0]),
        prefix,
        port,
        zone,
        embedded: String::new(),
        error: String::new(),
        duplicate: false,
    }
}

#[allow(clippy::too_many_arguments)]
fn finish_v6(
    line: usize,
    raw: &str,
    host: &str,
    zone: String,
    prefix: Option<u32>,
    port: Option<u32>,
    family: &str,
    ipv6_form: &str,
    _allow_prefix: bool,
) -> Entry {
    let ip: Ipv6Addr = match host.parse() {
        Ok(ip) => ip,
        Err(_) => return Entry::invalid(line, raw, explain_v6(host)),
    };
    if family == "ipv4" {
        return Entry::invalid(
            line,
            raw,
            "this is an IPv6 address but the family filter is set to IPv4".into(),
        );
    }
    if let Some(p) = prefix {
        if p > 128 {
            return Entry::invalid(
                line,
                raw,
                format!("prefix length /{p} is out of range for IPv6 (0-128)"),
            );
        }
    }

    let n = u128::from(ip);
    let compressed = ip.to_string();
    let expanded = expand_v6(ip);
    let (kind, scope, routable, embedded) = classify_v6(ip);
    Entry {
        line,
        input: raw.to_string(),
        valid: true,
        version: "IPv6",
        canonical: if ipv6_form == "expanded" {
            expanded.clone()
        } else {
            compressed
        },
        expanded,
        kind,
        scope,
        routable,
        integer: n.to_string(),
        hex: format!("0x{n:032x}"),
        binary: String::new(),
        reverse: reverse_v6(n),
        class: "",
        prefix,
        port,
        zone,
        embedded,
        error: String::new(),
        duplicate: false,
    }
}

/// Strict dotted-quad parser with per-octet error messages. Leading zeros are
/// rejected by default (they read as octal in several C libraries), but can be
/// accepted and normalized away.
fn parse_v4(host: &str, allow_leading_zeros: bool) -> Result<Ipv4Addr, String> {
    if !host.contains('.') {
        return Err(format!(
            "{host:?} is not an IP address: expected a dotted-quad IPv4 like 192.0.2.1 or a colon-hex IPv6 like 2001:db8::1"
        ));
    }
    let parts: Vec<&str> = host.split('.').collect();
    if parts.len() != 4 {
        return Err(format!(
            "expected 4 dot-separated octets, found {}",
            parts.len()
        ));
    }
    let mut octets = [0u8; 4];
    for (i, part) in parts.iter().enumerate() {
        let n = i + 1;
        if part.is_empty() {
            return Err(format!("octet {n} is empty"));
        }
        if let Some(c) = part.chars().find(|c| !c.is_ascii_digit()) {
            return Err(format!(
                "octet {n} ({part:?}) contains {c:?}, which is not a digit"
            ));
        }
        if part.len() > 1 && part.starts_with('0') && !allow_leading_zeros {
            return Err(format!(
                "octet {n} ({part:?}) has a leading zero, which is ambiguous (octal in some resolvers) — turn on 'Allow leading zeros' to read it as decimal"
            ));
        }
        if part.len() > 3 {
            return Err(format!("octet {n} ({part:?}) has more than 3 digits"));
        }
        let v: u32 = part.parse().map_err(|_| format!("octet {n} ({part:?}) is not a number"))?;
        if v > 255 {
            return Err(format!("octet {n} ({v}) is out of range 0-255"));
        }
        octets[i] = v as u8;
    }
    Ok(Ipv4Addr::new(octets[0], octets[1], octets[2], octets[3]))
}

/// Explain *why* an IPv6 literal failed, instead of "invalid IP address syntax".
fn explain_v6(host: &str) -> String {
    if host.contains(":::") {
        return "':::' is not valid — '::' may appear once and already stands for one or more zero groups".into();
    }
    let compressed = host.contains("::");
    if host.match_indices("::").count() > 1 {
        return "'::' may appear only once in an IPv6 address".into();
    }
    if !compressed {
        if host.starts_with(':') {
            return "an address may not start with a single ':' — did you mean '::'?".into();
        }
        if host.ends_with(':') {
            return "an address may not end with a single ':' — did you mean '::'?".into();
        }
    }
    let (head, tail) = match host.find("::") {
        Some(i) => (&host[..i], &host[i + 2..]),
        None => (host, ""),
    };
    let split = |s: &str| -> Vec<String> {
        if s.is_empty() {
            Vec::new()
        } else {
            s.split(':').map(str::to_string).collect()
        }
    };
    let mut groups = split(head);
    groups.extend(split(tail));

    let last = groups.len();
    let mut count = 0usize;
    for (i, g) in groups.iter().enumerate() {
        let n = i + 1;
        if g.is_empty() {
            return "an empty group is only allowed as part of '::'".into();
        }
        if g.contains('.') {
            if n != last {
                return format!(
                    "group {n} ({g:?}) looks like an IPv4 address — an embedded IPv4 tail may only be the last part, e.g. ::ffff:192.0.2.1"
                );
            }
            if let Err(e) = parse_v4(g, false) {
                return format!("the embedded IPv4 tail ({g:?}) is invalid: {e}");
            }
            count += 2;
            continue;
        }
        if let Some(c) = g.chars().find(|c| !c.is_ascii_hexdigit()) {
            return format!(
                "group {n} ({g:?}) contains {c:?}, which is not a hex digit (0-9, a-f)"
            );
        }
        if g.len() > 4 {
            return format!("group {n} ({g:?}) has more than 4 hex digits");
        }
        count += 1;
    }

    if compressed {
        if count >= 8 {
            return format!(
                "'::' must stand for at least one zero group, but this address already has {count} groups"
            );
        }
    } else if count != 8 {
        return format!("expected 8 groups separated by ':', found {count}");
    }
    format!("{host:?} is not a valid IPv6 address")
}

fn expand_v6(ip: Ipv6Addr) -> String {
    ip.segments()
        .iter()
        .map(|s| format!("{s:04x}"))
        .collect::<Vec<_>>()
        .join(":")
}

fn reverse_v6(n: u128) -> String {
    let hex = format!("{n:032x}");
    let mut out = String::with_capacity(72);
    for c in hex.chars().rev() {
        out.push(c);
        out.push('.');
    }
    out.push_str("ip6.arpa");
    out
}

// --------------------------------------------------------- classification ---

/// Classify an IPv4 address against the IANA special-purpose registry.
/// Returns (one-word kind, human detail with its RFC, globally routable?).
fn classify_v4(ip: Ipv4Addr) -> (&'static str, String, bool) {
    let o = ip.octets();
    let n = u32::from(ip);
    let d = |s: &str| s.to_string();
    if n == 0 {
        return ("unspecified", d("unspecified address — \"this host on this network\" (RFC 1122)"), false);
    }
    if n == u32::MAX {
        return ("broadcast", d("limited broadcast — 255.255.255.255 (RFC 919)"), false);
    }
    if o[0] == 0 {
        return ("this-network", d("\"this network\" — 0.0.0.0/8, valid as a source only (RFC 1122)"), false);
    }
    if o[0] == 127 {
        return ("loopback", d("loopback — 127.0.0.0/8, never leaves the host (RFC 1122)"), false);
    }
    if o[0] == 10 {
        return ("private", d("private use — 10.0.0.0/8 (RFC 1918)"), false);
    }
    if o[0] == 172 && (16..=31).contains(&o[1]) {
        return ("private", d("private use — 172.16.0.0/12 (RFC 1918)"), false);
    }
    if o[0] == 192 && o[1] == 168 {
        return ("private", d("private use — 192.168.0.0/16 (RFC 1918)"), false);
    }
    if o[0] == 100 && (64..=127).contains(&o[1]) {
        return ("shared", d("shared address space for carrier-grade NAT — 100.64.0.0/10 (RFC 6598)"), false);
    }
    if o[0] == 169 && o[1] == 254 {
        return ("link-local", d("link-local — 169.254.0.0/16, self-assigned when DHCP fails (RFC 3927)"), false);
    }
    if o[0] == 192 && o[1] == 0 && o[2] == 0 {
        return ("protocol-assignment", d("IETF protocol assignments — 192.0.0.0/24 (RFC 6890)"), false);
    }
    if o[0] == 192 && o[1] == 0 && o[2] == 2 {
        return ("documentation", d("documentation TEST-NET-1 — 192.0.2.0/24 (RFC 5737)"), false);
    }
    if o[0] == 198 && o[1] == 51 && o[2] == 100 {
        return ("documentation", d("documentation TEST-NET-2 — 198.51.100.0/24 (RFC 5737)"), false);
    }
    if o[0] == 203 && o[1] == 0 && o[2] == 113 {
        return ("documentation", d("documentation TEST-NET-3 — 203.0.113.0/24 (RFC 5737)"), false);
    }
    if o[0] == 192 && o[1] == 88 && o[2] == 99 {
        return ("6to4-relay", d("deprecated 6to4 relay anycast — 192.88.99.0/24 (RFC 7526)"), false);
    }
    if o[0] == 198 && (o[1] == 18 || o[1] == 19) {
        return ("benchmarking", d("network benchmark testing — 198.18.0.0/15 (RFC 2544)"), false);
    }
    if (224..=239).contains(&o[0]) {
        let detail = if o[0] == 224 && o[1] == 0 && o[2] == 0 {
            "multicast, local network control block — 224.0.0.0/24 (RFC 5771)"
        } else if o[0] == 239 {
            "multicast, administratively scoped — 239.0.0.0/8 (RFC 2365)"
        } else {
            "multicast — 224.0.0.0/4 (RFC 5771)"
        };
        return ("multicast", d(detail), false);
    }
    if o[0] >= 240 {
        return ("reserved", d("reserved for future use — 240.0.0.0/4 (RFC 1112)"), false);
    }
    ("global", d("globally routable public unicast"), true)
}

/// Legacy classful letter, still asked for by network exams and old tooling.
fn class_v4(first: u8) -> &'static str {
    match first {
        0..=127 => "A",
        128..=191 => "B",
        192..=223 => "C",
        224..=239 => "D (multicast)",
        _ => "E (reserved)",
    }
}

/// Classify an IPv6 address. Returns (kind, detail, routable, embedded IPv4).
fn classify_v6(ip: Ipv6Addr) -> (&'static str, String, bool, String) {
    let s = ip.segments();
    let n = u128::from(ip);
    let d = |k: &'static str, s: &str, r: bool| (k, s.to_string(), r, String::new());
    let low_v4 = || Ipv4Addr::from((n & 0xffff_ffff) as u32).to_string();

    if n == 0 {
        return d("unspecified", "unspecified address :: (RFC 4291)", false);
    }
    if n == 1 {
        return d("loopback", "loopback ::1 — never leaves the host (RFC 4291)", false);
    }
    if s[0] == 0 && s[1] == 0 && s[2] == 0 && s[3] == 0 && s[4] == 0 && s[5] == 0xffff {
        return (
            "ipv4-mapped",
            "IPv4-mapped IPv6 address — ::ffff:0:0/96 (RFC 4291)".into(),
            false,
            low_v4(),
        );
    }
    if s[0] == 0 && s[1] == 0 && s[2] == 0 && s[3] == 0 && s[4] == 0 && s[5] == 0 {
        return (
            "ipv4-compatible",
            "deprecated IPv4-compatible address — ::/96 (RFC 4291)".into(),
            false,
            low_v4(),
        );
    }
    if s[0] == 0x0064 && s[1] == 0xff9b {
        if s[2] == 0 && s[3] == 0 && s[4] == 0 && s[5] == 0 {
            return (
                "nat64",
                "NAT64 well-known prefix — 64:ff9b::/96 (RFC 6052)".into(),
                false,
                low_v4(),
            );
        }
        if s[2] == 0x0001 {
            return d("nat64", "NAT64 local-use prefix — 64:ff9b:1::/48 (RFC 8215)", false);
        }
    }
    if s[0] == 0x0100 && s[1] == 0 && s[2] == 0 && s[3] == 0 {
        return d("discard", "discard-only prefix — 100::/64, traffic is black-holed (RFC 6666)", false);
    }
    if s[0] == 0x2001 && s[1] == 0x0db8 {
        return d("documentation", "documentation — 2001:db8::/32 (RFC 3849)", false);
    }
    if s[0] == 0x2001 && s[1] == 0 {
        return d("teredo", "Teredo tunneling — 2001::/32 (RFC 4380)", false);
    }
    if s[0] == 0x2001 && (s[1] & 0xfff0) == 0x0020 {
        return d("orchid", "ORCHIDv2 cryptographic hash identifier — 2001:20::/28 (RFC 7343)", false);
    }
    if s[0] == 0x2001 && (s[1] & 0xfe00) == 0 {
        return d("protocol-assignment", "IETF protocol assignments — 2001::/23 (RFC 2928)", false);
    }
    if s[0] == 0x2002 {
        return (
            "6to4",
            "deprecated 6to4 transition prefix — 2002::/16 (RFC 3056, RFC 7526)".into(),
            false,
            Ipv4Addr::from(((s[1] as u32) << 16) | s[2] as u32).to_string(),
        );
    }
    if (s[0] & 0xff00) == 0xfd00 {
        return d("unique-local", "unique local address (the IPv6 private range), locally assigned — fd00::/8 (RFC 4193)", false);
    }
    if (s[0] & 0xfe00) == 0xfc00 {
        return d("unique-local", "unique local address (the IPv6 private range) — fc00::/7 (RFC 4193)", false);
    }
    if (s[0] & 0xffc0) == 0xfe80 {
        return d("link-local", "link-local unicast — fe80::/10, valid only on one link (RFC 4291)", false);
    }
    if (s[0] & 0xffc0) == 0xfec0 {
        return d("site-local", "deprecated site-local address — fec0::/10 (RFC 3879)", false);
    }
    if (s[0] & 0xff00) == 0xff00 {
        let scope = match s[0] & 0x000f {
            0 => "reserved",
            1 => "interface-local",
            2 => "link-local",
            3 => "realm-local",
            4 => "admin-local",
            5 => "site-local",
            8 => "organization-local",
            0x0e => "global",
            0x0f => "reserved",
            _ => "unassigned",
        };
        let flags = (s[0] >> 4) & 0x000f;
        let mut set: Vec<&str> = Vec::new();
        if flags & 0x1 != 0 {
            set.push("transient");
        }
        if flags & 0x2 != 0 {
            set.push("prefix-based");
        }
        if flags & 0x4 != 0 {
            set.push("embedded rendezvous point");
        }
        let extra = if set.is_empty() {
            ", permanently assigned".to_string()
        } else {
            format!(", {}", set.join(" + "))
        };
        return (
            "multicast",
            format!("multicast, {scope} scope{extra} — ff00::/8 (RFC 4291)"),
            false,
            String::new(),
        );
    }
    if (s[0] & 0xe000) == 0x2000 {
        return d("global", "global unicast — 2000::/3, globally routable (RFC 4291)", true);
    }
    d("reserved", "reserved by the IETF — not currently assigned (RFC 4291)", false)
}

// ---------------------------------------------------------------- output ----

fn field(out: &mut String, label: &str, value: &str) {
    let mut l = String::from(label);
    l.push(':');
    while l.chars().count() < LABEL_WIDTH {
        l.push(' ');
    }
    out.push_str(&l);
    out.push_str(value);
    out.push('\n');
}

fn render_report(entries: &[&Entry]) -> String {
    let mut out = String::new();
    for (i, e) in entries.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        field(&mut out, "Input", &e.input);
        if !e.valid {
            field(&mut out, "Status", "invalid");
            field(&mut out, "Error", &e.error);
            continue;
        }
        field(&mut out, "Status", if e.duplicate { "valid (duplicate)" } else { "valid" });
        field(&mut out, "Version", e.version);
        field(&mut out, "Canonical", &e.canonical);
        if !e.expanded.is_empty() && e.canonical != e.expanded {
            field(&mut out, "Expanded", &e.expanded);
        }
        if !e.zone.is_empty() {
            field(&mut out, "Zone", &e.zone);
        }
        if let Some(p) = e.prefix {
            field(&mut out, "Prefix", &format!("/{p}"));
        }
        if let Some(p) = e.port {
            field(&mut out, "Port", &p.to_string());
        }
        field(&mut out, "Type", e.kind);
        field(&mut out, "Scope", &e.scope);
        field(&mut out, "Routable", if e.routable { "yes" } else { "no" });
        if !e.embedded.is_empty() {
            field(&mut out, "Embedded IPv4", &e.embedded);
        }
        if !e.class.is_empty() {
            field(&mut out, "IPv4 class", e.class);
        }
        field(&mut out, "Integer", &e.integer);
        field(&mut out, "Hex", &e.hex);
        if !e.binary.is_empty() {
            field(&mut out, "Binary", &e.binary);
        }
        field(&mut out, "Reverse DNS", &e.reverse);
    }
    out.trim_end().to_string()
}

fn csv_cell(v: &str) -> String {
    if v.contains(',') || v.contains('"') || v.contains('\n') {
        format!("\"{}\"", v.replace('"', "\"\""))
    } else {
        v.to_string()
    }
}

fn render_table(entries: &[&Entry]) -> String {
    let mut out = String::from(
        "line,input,status,version,canonical,expanded,type,scope,routable,prefix,port,zone,embedded_ipv4,integer,hex,reverse_dns,error\n",
    );
    for e in entries {
        let status = if !e.valid {
            "invalid"
        } else if e.duplicate {
            "duplicate"
        } else {
            "valid"
        };
        let row = [
            e.line.to_string(),
            csv_cell(&e.input),
            status.to_string(),
            e.version.to_string(),
            csv_cell(&e.canonical),
            csv_cell(&e.expanded),
            e.kind.to_string(),
            csv_cell(&e.scope),
            if e.valid {
                if e.routable { "yes" } else { "no" }.to_string()
            } else {
                String::new()
            },
            e.prefix.map(|p| p.to_string()).unwrap_or_default(),
            e.port.map(|p| p.to_string()).unwrap_or_default(),
            csv_cell(&e.zone),
            e.embedded.clone(),
            e.integer.clone(),
            e.hex.clone(),
            e.reverse.clone(),
            csv_cell(&e.error),
        ];
        out.push_str(&row.join(","));
        out.push('\n');
    }
    out.trim_end().to_string()
}

fn json_str(v: &str) -> String {
    let mut out = String::with_capacity(v.len() + 2);
    out.push('"');
    for c in v.chars() {
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

fn json_opt(v: &str) -> String {
    if v.is_empty() {
        "null".into()
    } else {
        json_str(v)
    }
}

fn render_json(entries: &[&Entry]) -> String {
    let mut out = String::from("[\n");
    for (i, e) in entries.iter().enumerate() {
        if i > 0 {
            out.push_str(",\n");
        }
        out.push_str("  {\n");
        let mut rows: Vec<String> = vec![
            format!("    \"line\": {}", e.line),
            format!("    \"input\": {}", json_str(&e.input)),
            format!("    \"valid\": {}", e.valid),
        ];
        if e.valid {
            rows.push(format!("    \"duplicate\": {}", e.duplicate));
            rows.push(format!("    \"version\": {}", json_str(e.version)));
            rows.push(format!("    \"canonical\": {}", json_str(&e.canonical)));
            rows.push(format!("    \"expanded\": {}", json_opt(&e.expanded)));
            rows.push(format!("    \"type\": {}", json_str(e.kind)));
            rows.push(format!("    \"scope\": {}", json_str(&e.scope)));
            rows.push(format!("    \"globally_routable\": {}", e.routable));
            rows.push(format!(
                "    \"prefix_length\": {}",
                e.prefix.map(|p| p.to_string()).unwrap_or_else(|| "null".into())
            ));
            rows.push(format!(
                "    \"port\": {}",
                e.port.map(|p| p.to_string()).unwrap_or_else(|| "null".into())
            ));
            rows.push(format!("    \"zone\": {}", json_opt(&e.zone)));
            rows.push(format!("    \"embedded_ipv4\": {}", json_opt(&e.embedded)));
            rows.push(format!("    \"ipv4_class\": {}", json_opt(e.class)));
            rows.push(format!("    \"integer\": {}", json_str(&e.integer)));
            rows.push(format!("    \"hex\": {}", json_str(&e.hex)));
            rows.push(format!("    \"binary\": {}", json_opt(&e.binary)));
            rows.push(format!("    \"reverse_dns\": {}", json_str(&e.reverse)));
        } else {
            rows.push(format!("    \"error\": {}", json_str(&e.error)));
        }
        out.push_str(&rows.join(",\n"));
        out.push_str("\n  }");
    }
    out.push_str("\n]");
    out
}

fn render_valid(entries: &[&Entry]) -> String {
    let list: Vec<String> = entries
        .iter()
        .filter(|e| e.valid)
        .map(|e| e.canonical_full())
        .collect();
    if list.is_empty() {
        "(no valid addresses)".into()
    } else {
        list.join("\n")
    }
}

fn render_invalid(entries: &[&Entry]) -> String {
    let list: Vec<String> = entries
        .iter()
        .filter(|e| !e.valid)
        .map(|e| format!("line {}: {} — {}", e.line, e.input, e.error))
        .collect();
    if list.is_empty() {
        "(no invalid addresses)".into()
    } else {
        list.join("\n")
    }
}

fn render_summary(entries: &[Entry]) -> String {
    let total = entries.len();
    let valid = entries.iter().filter(|e| e.valid).count();
    let duplicates = entries.iter().filter(|e| e.duplicate).count();
    let ipv4 = entries.iter().filter(|e| e.version == "IPv4").count();
    let ipv6 = entries.iter().filter(|e| e.version == "IPv6").count();
    let routable = entries.iter().filter(|e| e.valid && e.routable).count();
    let mut kinds: BTreeMap<&str, usize> = BTreeMap::new();
    for e in entries.iter().filter(|e| e.valid) {
        *kinds.entry(e.kind).or_insert(0) += 1;
    }
    let mut out = String::from("metric,value\n");
    out.push_str(&format!("total,{total}\n"));
    out.push_str(&format!("valid,{valid}\n"));
    out.push_str(&format!("invalid,{}\n", total - valid));
    out.push_str(&format!("duplicates,{duplicates}\n"));
    out.push_str(&format!("ipv4,{ipv4}\n"));
    out.push_str(&format!("ipv6,{ipv6}\n"));
    out.push_str(&format!("globally_routable,{routable}\n"));
    for (k, v) in kinds {
        out.push_str(&format!("type:{k},{v}\n"));
    }
    out.trim_end().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(addrs: &str) -> String {
        run(addrs, "any", true, true, false, "compressed", false, "report").unwrap()
    }

    #[test]
    fn private_ipv4_report() {
        let out = r("192.168.1.1");
        assert_eq!(
            out,
            "Input:          192.168.1.1\n\
             Status:         valid\n\
             Version:        IPv4\n\
             Canonical:      192.168.1.1\n\
             Type:           private\n\
             Scope:          private use — 192.168.0.0/16 (RFC 1918)\n\
             Routable:       no\n\
             IPv4 class:     C\n\
             Integer:        3232235777\n\
             Hex:            0xc0a80101\n\
             Binary:         11000000.10101000.00000001.00000001\n\
             Reverse DNS:    1.1.168.192.in-addr.arpa"
        );
    }

    #[test]
    fn ipv6_documentation_is_canonicalized() {
        let out = r("2001:0DB8:0000:0000:0000:0000:0000:0001");
        assert!(out.contains("Canonical:      2001:db8::1"), "{out}");
        assert!(
            out.contains("Expanded:       2001:0db8:0000:0000:0000:0000:0000:0001"),
            "{out}"
        );
        assert!(out.contains("Type:           documentation"), "{out}");
        assert!(out.contains("Scope:          documentation — 2001:db8::/32 (RFC 3849)"), "{out}");
        assert!(
            out.contains(
                "Reverse DNS:    1.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.0.8.b.d.0.1.0.0.2.ip6.arpa"
            ),
            "{out}"
        );
    }

    #[test]
    fn classifies_the_documented_categories() {
        for (addr, kind) in [
            ("10.0.0.1", "private"),
            ("172.31.255.254", "private"),
            ("127.0.0.1", "loopback"),
            ("169.254.10.1", "link-local"),
            ("224.0.0.251", "multicast"),
            ("203.0.113.9", "documentation"),
            ("100.64.0.1", "shared"),
            ("198.18.0.1", "benchmarking"),
            ("240.0.0.1", "reserved"),
            ("255.255.255.255", "broadcast"),
            ("0.0.0.0", "unspecified"),
            ("8.8.8.8", "global"),
            ("::1", "loopback"),
            ("fe80::1", "link-local"),
            ("fd00::1", "unique-local"),
            ("ff02::1", "multicast"),
            ("2001:db8::1", "documentation"),
            ("2001::1", "teredo"),
            ("2002:c000:204::1", "6to4"),
            ("64:ff9b::c000:201", "nat64"),
            ("::ffff:192.0.2.1", "ipv4-mapped"),
            ("2606:4700::1111", "global"),
        ] {
            let out = r(addr);
            assert!(
                out.contains(&format!("Type:           {kind}")),
                "{addr} should classify as {kind}:\n{out}"
            );
        }
    }

    #[test]
    fn only_global_addresses_are_routable() {
        assert!(r("8.8.8.8").contains("Routable:       yes"));
        assert!(r("2606:4700::1111").contains("Routable:       yes"));
        assert!(r("192.168.0.1").contains("Routable:       no"));
        assert!(r("fe80::1").contains("Routable:       no"));
    }

    #[test]
    fn embedded_ipv4_is_decoded() {
        assert!(r("::ffff:192.0.2.128").contains("Embedded IPv4:  192.0.2.128"));
        assert!(r("2002:c000:0204::1").contains("Embedded IPv4:  192.0.2.4"));
        assert!(r("64:ff9b::c000:201").contains("Embedded IPv4:  192.0.2.1"));
    }

    #[test]
    fn octet_out_of_range_is_explained() {
        let out = r("192.168.1.256");
        assert!(out.contains("Status:         invalid"), "{out}");
        assert!(out.contains("octet 4 (256) is out of range 0-255"), "{out}");
    }

    #[test]
    fn leading_zeros_rejected_by_default_and_normalized_on_request() {
        assert!(r("192.168.01.1").contains("has a leading zero"));
        let out = run("192.168.01.1", "any", true, true, true, "compressed", false, "report").unwrap();
        assert!(out.contains("Canonical:      192.168.1.1"), "{out}");
    }

    #[test]
    fn ipv6_syntax_errors_are_specific() {
        for (addr, needle) in [
            ("2001:db8::1::2", "'::' may appear only once"),
            ("2001:db8:zz::1", "is not a hex digit"),
            ("2001:db8:12345::1", "more than 4 hex digits"),
            ("2001:db8:1:2:3:4:5", "expected 8 groups separated by ':', found 7"),
            ("1:2:3:4:5:6:7:8::9", "'::' must stand for at least one zero group"),
        ] {
            let out = r(addr);
            assert!(out.contains(needle), "{addr} should say {needle:?}:\n{out}");
        }
    }

    #[test]
    fn ports_prefixes_and_zones_round_trip() {
        let out = r("[2001:0db8::1]:443");
        assert!(out.contains("Canonical:      2001:db8::1"), "{out}");
        assert!(out.contains("Port:           443"), "{out}");
        let out = r("10.0.0.0/8");
        assert!(out.contains("Prefix:         /8"), "{out}");
        let out = r("192.0.2.7:8080");
        assert!(out.contains("Port:           8080"), "{out}");
        let out = r("fe80::1%eth0");
        assert!(out.contains("Zone:           eth0"), "{out}");
        let valid = run(
            "[2001:0db8::1]:443\n10.0.0.0/8\nfe80::1%eth0",
            "any", true, true, false, "compressed", false, "valid",
        )
        .unwrap();
        assert_eq!(valid, "[2001:db8::1]:443\n10.0.0.0/8\nfe80::1%eth0");
    }

    #[test]
    fn prefix_range_is_checked_per_family() {
        assert!(r("10.0.0.0/33").contains("out of range for IPv4 (0-32)"));
        assert!(r("2001:db8::/129").contains("out of range for IPv6 (0-128)"));
    }

    #[test]
    fn family_filter_rejects_the_other_family() {
        let out = run("2001:db8::1", "ipv4", true, true, false, "compressed", false, "report").unwrap();
        assert!(out.contains("family filter is set to IPv4"), "{out}");
        let out = run("192.0.2.1", "ipv6", true, true, false, "compressed", false, "report").unwrap();
        assert!(out.contains("family filter is set to IPv6"), "{out}");
    }

    #[test]
    fn expanded_form_option() {
        let out = run("2001:db8::1", "any", true, true, false, "expanded", false, "valid").unwrap();
        assert_eq!(out, "2001:0db8:0000:0000:0000:0000:0000:0001");
    }

    #[test]
    fn dedupe_drops_equivalent_spellings() {
        let out = run(
            "2001:DB8::1\n2001:0db8:0000:0000:0000:0000:0000:0001\n192.0.2.1",
            "any", true, true, false, "compressed", true, "valid",
        )
        .unwrap();
        assert_eq!(out, "2001:db8::1\n192.0.2.1");
    }

    #[test]
    fn table_and_invalid_and_summary_outputs() {
        let input = "192.168.1.1\n2001:db8::1\nnot-an-ip\n192.168.1.1";
        let table = run(input, "any", true, true, false, "compressed", false, "table").unwrap();
        assert!(table.starts_with("line,input,status,version,canonical,"), "{table}");
        assert!(table.contains("\n4,192.168.1.1,duplicate,IPv4,192.168.1.1,"), "{table}");

        let bad = run(input, "any", true, true, false, "compressed", false, "invalid").unwrap();
        assert!(bad.starts_with("line 3: not-an-ip — "), "{bad}");

        let summary = run(input, "any", true, true, false, "compressed", false, "summary").unwrap();
        assert_eq!(
            summary,
            "metric,value\ntotal,4\nvalid,3\ninvalid,1\nduplicates,1\nipv4,2\nipv6,1\nglobally_routable,0\ntype:documentation,1\ntype:private,2"
        );
    }

    #[test]
    fn json_output_is_parseable_shape() {
        let out = run("192.0.2.1", "any", true, true, false, "compressed", false, "json").unwrap();
        assert!(out.starts_with("[\n  {\n"), "{out}");
        assert!(out.contains("\"canonical\": \"192.0.2.1\""), "{out}");
        assert!(out.contains("\"type\": \"documentation\""), "{out}");
        assert!(out.contains("\"globally_routable\": false"), "{out}");
        assert!(out.ends_with("\n]"), "{out}");
    }

    #[test]
    fn port_and_prefix_can_be_refused() {
        let out = run("192.0.2.1:80", "any", true, false, false, "compressed", false, "report").unwrap();
        assert!(out.contains("port suffix is not accepted"), "{out}");
        let out = run("192.0.2.0/24", "any", false, true, false, "compressed", false, "report").unwrap();
        assert!(out.contains("prefix suffix is not accepted"), "{out}");
    }

    #[test]
    fn empty_input_is_an_error() {
        let err = run("   \n\n", "any", true, true, false, "compressed", false, "report").unwrap_err();
        assert!(err.contains("input is empty"), "{err}");
    }

    #[test]
    fn bad_enum_is_an_error() {
        let err = run("192.0.2.1", "any", true, true, false, "compressed", false, "yaml").unwrap_err();
        assert!(err.contains("invalid output"), "{err}");
        let err = run("192.0.2.1", "ipv7", true, true, false, "compressed", false, "report").unwrap_err();
        assert!(err.contains("invalid family"), "{err}");
    }

    #[test]
    fn too_many_addresses_is_capped() {
        let big = (0..MAX_ADDRESSES + 1)
            .map(|_| "192.0.2.1")
            .collect::<Vec<_>>()
            .join("\n");
        let err = run(&big, "any", true, true, false, "compressed", false, "report").unwrap_err();
        assert!(err.contains("too many addresses"), "{err}");
    }

    #[test]
    fn bracketed_form_needs_a_closing_bracket() {
        assert!(r("[2001:db8::1").contains("unterminated '['"));
    }

    #[test]
    fn multicast_scope_is_decoded() {
        assert!(r("ff02::1").contains("multicast, link-local scope, permanently assigned"));
        assert!(r("ff15::1").contains("multicast, site-local scope, transient"));
    }
}
