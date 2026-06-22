//! ip-range-expand core — expand a CIDR block or `start-end` IP range into the
//! full list of addresses (or just a count), for IPv4 and IPv6. No deps beyond
//! `std::net`; shared by the chat skill block and the web page.
//!
//! Accepted inputs (one per call, whitespace-trimmed):
//! * **CIDR** — `192.168.1.0/24`, `10.0.0.0/30`, `2001:db8::/126`.
//! * **Range** — `192.168.1.10-192.168.1.20`, `10.0.0.1-10.0.0.5`,
//!   `2001:db8::1-2001:db8::5`. A bare suffix on the right side is also
//!   accepted for IPv4 (`192.168.1.10-20` → ends at `192.168.1.20`).
//!
//! `output = "list"` returns one address per line; `output = "count"` returns
//! the total number of addresses in the range. `limit` caps how many addresses
//! `list` will enumerate (counting is always exact and unbounded).

use std::net::{Ipv4Addr, Ipv6Addr};

/// Default cap on how many addresses `list` mode will enumerate.
pub const DEFAULT_LIMIT: u64 = 65536;

/// Expand `input` into either the address list or a count.
///
/// * `output` — `"list"` (default) emits one address per line; `"count"` emits
///   the decimal total. Blank/unknown → error.
/// * `limit` — for `list`, the maximum number of addresses to emit; if the
///   range is larger the call errors with the actual size so nothing is
///   silently truncated. Ignored for `count`.
pub fn expand(input: &str, output: &str, limit: u64) -> Result<String, String> {
    let input = input.trim();
    if input.is_empty() {
        return Err("input is empty: provide a CIDR (e.g. 192.168.1.0/24) or a range (e.g. 192.168.1.10-192.168.1.20)".into());
    }
    let mode = output.trim();
    let mode = if mode.is_empty() { "list" } else { mode };
    let range = parse(input)?;

    match mode.to_ascii_lowercase().as_str() {
        "list" => list(range, limit),
        "count" => Ok(range.count()),
        other => Err(format!(
            "invalid output {other:?}: expected 'list' or 'count'"
        )),
    }
}

/// A normalized, inclusive address range, in one of the two families.
enum Range {
    V4 { start: u32, end: u32 },
    V6 { start: u128, end: u128 },
}

impl Range {
    /// Number of addresses in the inclusive range, as a decimal string. A string
    /// (not an integer) because an IPv6 `/0` spans 2^128 addresses, which does
    /// not fit in a `u128` (`end - start + 1` would overflow).
    fn count(&self) -> String {
        match *self {
            Range::V4 { start, end } => ((end as u128) - (start as u128) + 1).to_string(),
            Range::V6 { start, end } => {
                let span = end - start; // ≤ u128::MAX, no overflow.
                if span == u128::MAX {
                    // Whole address space: 2^128 = u128::MAX + 1.
                    "340282366920938463463374607431768211456".to_string()
                } else {
                    (span + 1).to_string()
                }
            }
        }
    }

    /// Number of addresses as a `u128`, saturating at `u128::MAX` for the one
    /// case (IPv6 `/0`) whose true count is 2^128. Used only for the `list`
    /// limit check, where any value ≥ the limit (max `u64`) means "too big".
    fn count_for_limit(&self) -> u128 {
        match *self {
            Range::V4 { start, end } => (end as u128) - (start as u128) + 1,
            Range::V6 { start, end } => (end - start).saturating_add(1),
        }
    }
}

/// Parse an input string into a normalized [`Range`], rejecting malformed
/// CIDRs/ranges and ranges whose start is greater than their end.
fn parse(input: &str) -> Result<Range, String> {
    if let Some((addr, prefix)) = input.split_once('/') {
        parse_cidr(addr.trim(), prefix.trim())
    } else if let Some((a, b)) = input.split_once('-') {
        parse_range(a.trim(), b.trim())
    } else {
        Err(format!(
            "could not parse {input:?}: expected a CIDR like '192.168.1.0/24' or a range like '192.168.1.10-192.168.1.20'"
        ))
    }
}

/// Parse `addr/prefix` into the inclusive range it covers. The network and
/// broadcast (all-ones) addresses ARE included — this expands the whole block.
fn parse_cidr(addr: &str, prefix: &str) -> Result<Range, String> {
    if let Ok(v4) = addr.parse::<Ipv4Addr>() {
        let p: u32 = prefix
            .parse()
            .map_err(|_| format!("invalid IPv4 prefix length {prefix:?}: expected 0–32"))?;
        if p > 32 {
            return Err(format!("invalid IPv4 prefix length {p}: expected 0–32"));
        }
        let base = u32::from(v4);
        let mask: u32 = if p == 0 { 0 } else { u32::MAX << (32 - p) };
        let start = base & mask;
        let end = start | !mask;
        Ok(Range::V4 { start, end })
    } else if let Ok(v6) = addr.parse::<Ipv6Addr>() {
        let p: u32 = prefix
            .parse()
            .map_err(|_| format!("invalid IPv6 prefix length {prefix:?}: expected 0–128"))?;
        if p > 128 {
            return Err(format!("invalid IPv6 prefix length {p}: expected 0–128"));
        }
        let base = u128::from(v6);
        let mask: u128 = if p == 0 { 0 } else { u128::MAX << (128 - p) };
        let start = base & mask;
        let end = start | !mask;
        Ok(Range::V6 { start, end })
    } else {
        Err(format!(
            "invalid IP address {addr:?} in CIDR: expected an IPv4 or IPv6 address"
        ))
    }
}

/// Parse a `start-end` range. Both sides must be the same family. For IPv4 the
/// end may be a bare last octet (`192.168.1.10-20`) for convenience.
fn parse_range(a: &str, b: &str) -> Result<Range, String> {
    if a.is_empty() || b.is_empty() {
        return Err("invalid range: expected 'start-end' with both endpoints, e.g. 10.0.0.1-10.0.0.5".into());
    }
    // IPv4 first (the common case), then IPv6.
    if let Ok(start) = a.parse::<Ipv4Addr>() {
        let start = u32::from(start);
        let end = if let Ok(e) = b.parse::<Ipv4Addr>() {
            u32::from(e)
        } else if let Ok(suffix) = b.parse::<u8>() {
            // Bare suffix: replace the last octet of the start address.
            (start & 0xFFFF_FF00) | (suffix as u32)
        } else {
            return Err(format!(
                "invalid range end {b:?}: expected an IPv4 address or a final octet 0–255"
            ));
        };
        if start > end {
            return Err(format!(
                "range start is greater than end ({}-{}): start must be ≤ end",
                Ipv4Addr::from(start),
                Ipv4Addr::from(end)
            ));
        }
        return Ok(Range::V4 { start, end });
    }
    if let Ok(start) = a.parse::<Ipv6Addr>() {
        let end = b.parse::<Ipv6Addr>().map_err(|_| {
            format!("invalid range end {b:?}: start is IPv6 so end must be an IPv6 address")
        })?;
        let start = u128::from(start);
        let end = u128::from(end);
        if start > end {
            return Err(format!(
                "range start is greater than end ({}-{}): start must be ≤ end",
                Ipv6Addr::from(start),
                Ipv6Addr::from(end)
            ));
        }
        return Ok(Range::V6 { start, end });
    }
    Err(format!(
        "invalid range start {a:?}: expected an IPv4 or IPv6 address"
    ))
}

/// Enumerate the range as newline-separated addresses, refusing to expand more
/// than `limit` (so callers get a clear error instead of a multi-megabyte dump).
fn list(range: Range, limit: u64) -> Result<String, String> {
    let total = range.count_for_limit();
    if total > limit as u128 {
        return Err(format!(
            "range has {total} addresses, which exceeds the limit of {limit}. Raise the limit, narrow the range, or use output='count'."
        ));
    }
    let mut out = String::new();
    match range {
        Range::V4 { start, end } => {
            for n in start..=end {
                out.push_str(&Ipv4Addr::from(n).to_string());
                out.push('\n');
            }
        }
        Range::V6 { start, end } => {
            // total is already ≤ limit, so this loop is bounded.
            let mut n = start;
            loop {
                out.push_str(&Ipv6Addr::from(n).to_string());
                out.push('\n');
                if n == end {
                    break;
                }
                n += 1;
            }
        }
    }
    if out.ends_with('\n') {
        out.pop();
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cidr_v4_slash30() {
        let out = expand("10.0.0.0/30", "list", DEFAULT_LIMIT).unwrap();
        assert_eq!(out, "10.0.0.0\n10.0.0.1\n10.0.0.2\n10.0.0.3");
    }

    #[test]
    fn cidr_v4_count() {
        assert_eq!(expand("192.168.1.0/24", "count", DEFAULT_LIMIT).unwrap(), "256");
        assert_eq!(expand("10.0.0.0/8", "count", DEFAULT_LIMIT).unwrap(), "16777216");
        assert_eq!(expand("0.0.0.0/0", "count", DEFAULT_LIMIT).unwrap(), "4294967296");
    }

    #[test]
    fn cidr_normalizes_non_aligned_base() {
        // 192.168.1.130/29 → network is 192.168.1.128.
        let out = expand("192.168.1.130/29", "list", DEFAULT_LIMIT).unwrap();
        assert!(out.starts_with("192.168.1.128\n"), "got: {out}");
        assert!(out.ends_with("192.168.1.135"), "got: {out}");
        assert_eq!(out.lines().count(), 8);
    }

    #[test]
    fn cidr_slash32_is_single_address() {
        assert_eq!(expand("8.8.8.8/32", "list", DEFAULT_LIMIT).unwrap(), "8.8.8.8");
        assert_eq!(expand("8.8.8.8/32", "count", DEFAULT_LIMIT).unwrap(), "1");
    }

    #[test]
    fn range_v4_full() {
        let out = expand("192.168.1.10-192.168.1.13", "list", DEFAULT_LIMIT).unwrap();
        assert_eq!(out, "192.168.1.10\n192.168.1.11\n192.168.1.12\n192.168.1.13");
    }

    #[test]
    fn range_v4_bare_suffix() {
        let out = expand("192.168.1.10-12", "list", DEFAULT_LIMIT).unwrap();
        assert_eq!(out, "192.168.1.10\n192.168.1.11\n192.168.1.12");
    }

    #[test]
    fn range_v4_count() {
        assert_eq!(expand("10.0.0.1-10.0.0.5", "count", DEFAULT_LIMIT).unwrap(), "5");
    }

    #[test]
    fn range_start_after_end_errors() {
        let e = expand("10.0.0.5-10.0.0.1", "list", DEFAULT_LIMIT).unwrap_err();
        assert!(e.contains("greater than end"), "got: {e}");
    }

    #[test]
    fn cidr_v6() {
        let out = expand("2001:db8::/126", "list", DEFAULT_LIMIT).unwrap();
        assert_eq!(out, "2001:db8::\n2001:db8::1\n2001:db8::2\n2001:db8::3");
        assert_eq!(expand("2001:db8::/126", "count", DEFAULT_LIMIT).unwrap(), "4");
    }

    #[test]
    fn count_v6_huge() {
        // /64 has 2^64 addresses; count is exact and unbounded.
        assert_eq!(
            expand("2001:db8::/64", "count", DEFAULT_LIMIT).unwrap(),
            "18446744073709551616"
        );
        // /0 = 2^128.
        assert_eq!(
            expand("::/0", "count", DEFAULT_LIMIT).unwrap(),
            "340282366920938463463374607431768211456"
        );
    }

    #[test]
    fn range_v6() {
        let out = expand("2001:db8::1-2001:db8::4", "list", DEFAULT_LIMIT).unwrap();
        assert_eq!(out, "2001:db8::1\n2001:db8::2\n2001:db8::3\n2001:db8::4");
    }

    #[test]
    fn list_over_limit_errors_but_count_does_not() {
        let e = expand("10.0.0.0/16", "list", 100).unwrap_err();
        assert!(e.contains("exceeds the limit"), "got: {e}");
        // count ignores the limit.
        assert_eq!(expand("10.0.0.0/16", "count", 100).unwrap(), "65536");
    }

    #[test]
    fn limit_boundary_exact() {
        // /30 = 4 addresses; limit of exactly 4 should pass.
        assert!(expand("10.0.0.0/30", "list", 4).is_ok());
        assert!(expand("10.0.0.0/30", "list", 3).is_err());
    }

    #[test]
    fn output_defaults_to_list() {
        assert_eq!(expand("8.8.8.8/32", "", DEFAULT_LIMIT).unwrap(), "8.8.8.8");
    }

    #[test]
    fn rejects_garbage() {
        assert!(expand("not an ip", "list", DEFAULT_LIMIT).is_err());
        assert!(expand("", "list", DEFAULT_LIMIT).is_err());
        assert!(expand("10.0.0.0/33", "list", DEFAULT_LIMIT).is_err());
        assert!(expand("2001:db8::/129", "list", DEFAULT_LIMIT).is_err());
        assert!(expand("10.0.0.0/abc", "list", DEFAULT_LIMIT).is_err());
    }

    #[test]
    fn rejects_unknown_output() {
        let e = expand("8.8.8.8/32", "json", DEFAULT_LIMIT).unwrap_err();
        assert!(e.contains("invalid output"), "got: {e}");
    }

    #[test]
    fn mixed_family_range_errors() {
        let e = expand("10.0.0.1-2001:db8::5", "list", DEFAULT_LIMIT).unwrap_err();
        assert!(e.contains("expected an IPv4 address or a final octet"), "got: {e}");
    }
}
