//! ip-range-to-cidr core — convert an arbitrary inclusive start–end IP range
//! into the *minimal* set of CIDR blocks that exactly covers it (no more, no
//! fewer addresses). Pure compute (`std::net` only), shared by the chat skill
//! block and the web page. Works for both IPv4 and IPv6.
//!
//! Accepted input (one per call, whitespace-trimmed) is a `start-end` range:
//! * **IPv4** — `192.168.1.0-192.168.1.255`, `10.0.0.5-10.0.0.20`. A bare final
//!   octet on the right is accepted as shorthand (`192.168.1.10-20` →
//!   `192.168.1.10-192.168.1.20`).
//! * **IPv6** — `2001:db8::-2001:db8::ffff`, `2001:db8::1-2001:db8::5`.
//! * A single address with no `-` (or `start == end`) yields a single
//!   `/32` (IPv4) or `/128` (IPv6) host route.
//!
//! Both endpoints must be the same family, and `start <= end`.
//!
//! `output = "list"` (default) returns one CIDR per line; `output = "count"`
//! returns just the number of CIDR blocks needed.

use std::net::{Ipv4Addr, Ipv6Addr};

/// Convert `input` (a `start-end` IP range) into its minimal CIDR cover.
///
/// * `output` — `"list"` (default) emits one CIDR per line; `"count"` emits the
///   decimal number of CIDR blocks. Blank → `"list"`.
pub fn run(input: &str, output: &str) -> Result<String, String> {
    let input = input.trim();
    if input.is_empty() {
        return Err("input is empty: provide a range like 192.168.1.0-192.168.1.255 or 2001:db8::-2001:db8::ffff".into());
    }
    let mode = output.trim();
    let mode = if mode.is_empty() { "list" } else { mode };
    let mode = mode.to_ascii_lowercase();
    if mode != "list" && mode != "count" {
        return Err(format!("invalid output {output:?}: expected 'list' or 'count'"));
    }

    let cidrs = to_cidrs(input)?;
    Ok(match mode.as_str() {
        "count" => cidrs.len().to_string(),
        _ => cidrs.join("\n"),
    })
}

/// Parse `input` and produce the minimal list of CIDR strings.
fn to_cidrs(input: &str) -> Result<Vec<String>, String> {
    match parse_range(input)? {
        Range::V4 { start, end } => Ok(split_v4(start, end)
            .into_iter()
            .map(|(base, prefix)| format!("{}/{}", Ipv4Addr::from(base), prefix))
            .collect()),
        Range::V6 { start, end } => Ok(split_v6(start, end)
            .into_iter()
            .map(|(base, prefix)| format!("{}/{}", Ipv6Addr::from(base), prefix))
            .collect()),
    }
}

/// A normalized, inclusive range in one of the two families.
enum Range {
    V4 { start: u32, end: u32 },
    V6 { start: u128, end: u128 },
}

/// Split the inclusive IPv4 range `[start, end]` into the minimal CIDR list.
/// Each entry is `(network base, prefix length)`. The greedy algorithm at each
/// step picks the largest CIDR block that (a) is aligned to `start` and (b)
/// does not extend past `end`, then advances `start`.
fn split_v4(mut start: u32, end: u32) -> Vec<(u32, u8)> {
    let mut out = Vec::new();
    loop {
        // Largest aligned block at `start`: prefix limited by the number of
        // trailing zero bits in `start` (alignment) and by how many addresses
        // remain to `end` (so we never overshoot).
        let max_by_align = if start == 0 { 32 } else { start.trailing_zeros().min(32) };
        let remaining = (end as u64) - (start as u64) + 1;
        // Largest power-of-two block size <= remaining, as a host-bit count.
        let max_by_size = 63 - remaining.leading_zeros(); // floor(log2(remaining))
        let host_bits = max_by_align.min(max_by_size).min(32);
        let prefix = 32 - host_bits as u8;
        out.push((start, prefix));

        // Advance to one past this block; stop if we've covered `end` (guard
        // against u32 overflow when the block reaches the top of the space).
        let block = 1u64 << host_bits;
        let next = start as u64 + block;
        if next > end as u64 {
            break;
        }
        start = next as u32;
    }
    out
}

/// IPv6 analogue of [`split_v4`], over the full 128-bit space.
fn split_v6(mut start: u128, end: u128) -> Vec<(u128, u8)> {
    let mut out = Vec::new();
    loop {
        let max_by_align = if start == 0 { 128 } else { start.trailing_zeros().min(128) };
        // remaining = end - start + 1, but that can overflow u128 for the whole
        // space; work with the host-bit count directly.
        let span = end - start; // <= u128::MAX
        let max_by_size = if span == u128::MAX {
            128
        } else {
            // floor(log2(span + 1))
            127 - (span + 1).leading_zeros()
        };
        let host_bits = max_by_align.min(max_by_size).min(128);
        let prefix = 128 - host_bits as u8;
        out.push((start, prefix));

        if host_bits == 128 {
            break; // covered the entire space
        }
        let block = 1u128 << host_bits;
        let next = start.checked_add(block);
        match next {
            Some(n) if n <= end => start = n,
            _ => break,
        }
    }
    out
}

/// Parse a `start-end` range (or a single address) into a normalized [`Range`].
fn parse_range(input: &str) -> Result<Range, String> {
    // IPv6 addresses use `:` and never `-`; IPv4 uses `.` and never `:`. So the
    // single `-` (if any) always separates the two endpoints in both families.
    let (left, right) = match input.split_once('-') {
        Some((l, r)) => (l.trim(), r.trim()),
        None => (input, ""), // single address
    };

    // Try IPv4 first.
    if let Ok(start) = left.parse::<Ipv4Addr>() {
        let start = u32::from(start);
        let end = if right.is_empty() {
            start
        } else if let Ok(e) = right.parse::<Ipv4Addr>() {
            u32::from(e)
        } else if let Ok(octet) = right.parse::<u32>() {
            // Bare final-octet shorthand: replace the low octet of `start`.
            if octet > 255 {
                return Err(format!("invalid range end {right:?}: a bare octet must be 0–255"));
            }
            (start & 0xffff_ff00) | octet
        } else {
            return Err(format!(
                "invalid range end {right:?}: expected an IPv4 address or a bare final octet (0–255)"
            ));
        };
        if start > end {
            return Err(format!(
                "range is reversed: start {} is greater than end {}",
                Ipv4Addr::from(start),
                Ipv4Addr::from(end)
            ));
        }
        return Ok(Range::V4 { start, end });
    }

    // Try IPv6.
    if let Ok(start) = left.parse::<Ipv6Addr>() {
        let start = u128::from(start);
        let end = if right.is_empty() {
            start
        } else if let Ok(e) = right.parse::<Ipv6Addr>() {
            u128::from(e)
        } else {
            return Err(format!("invalid range end {right:?}: expected an IPv6 address"));
        };
        if start > end {
            return Err(format!(
                "range is reversed: start {} is greater than end {}",
                Ipv6Addr::from(start),
                Ipv6Addr::from(end)
            ));
        }
        return Ok(Range::V6 { start, end });
    }

    Err(format!(
        "invalid range start {left:?}: expected an IPv4 (e.g. 192.168.1.0) or IPv6 (e.g. 2001:db8::) address"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn list(input: &str) -> Vec<String> {
        run(input, "list").unwrap().lines().map(String::from).collect()
    }

    #[test]
    fn aligned_full_block_is_one_cidr() {
        assert_eq!(list("192.168.1.0-192.168.1.255"), vec!["192.168.1.0/24"]);
    }

    #[test]
    fn single_address_is_a_host_route() {
        assert_eq!(list("10.0.0.5-10.0.0.5"), vec!["10.0.0.5/32"]);
        assert_eq!(list("10.0.0.5"), vec!["10.0.0.5/32"]);
    }

    #[test]
    fn unaligned_range_splits_into_minimal_set() {
        // Classic example: 10.0.0.5-10.0.0.20.
        assert_eq!(
            list("10.0.0.5-10.0.0.20"),
            vec![
                "10.0.0.5/32",
                "10.0.0.6/31",
                "10.0.0.8/29",
                "10.0.0.16/30",
                "10.0.0.20/32",
            ]
        );
    }

    #[test]
    fn bare_octet_shorthand() {
        assert_eq!(list("192.168.1.0-255"), vec!["192.168.1.0/24"]);
        assert_eq!(
            list("192.168.1.10-20"),
            list("192.168.1.10-192.168.1.20")
        );
    }

    #[test]
    fn two_adjacent_blocks_merge_when_aligned() {
        // 0..511 across two /24s aligns to a single /23.
        assert_eq!(list("192.168.0.0-192.168.1.255"), vec!["192.168.0.0/23"]);
    }

    #[test]
    fn whole_ipv4_space_is_one_cidr() {
        assert_eq!(list("0.0.0.0-255.255.255.255"), vec!["0.0.0.0/0"]);
    }

    #[test]
    fn top_of_space_no_overflow() {
        assert_eq!(
            list("255.255.255.252-255.255.255.255"),
            vec!["255.255.255.252/30"]
        );
    }

    #[test]
    fn ipv6_aligned_block() {
        assert_eq!(
            list("2001:db8::-2001:db8::ffff"),
            vec!["2001:db8::/112"]
        );
    }

    #[test]
    fn ipv6_single_address() {
        assert_eq!(list("2001:db8::1"), vec!["2001:db8::1/128"]);
    }

    #[test]
    fn ipv6_unaligned_range() {
        assert_eq!(
            list("2001:db8::1-2001:db8::5"),
            vec!["2001:db8::1/128", "2001:db8::2/127", "2001:db8::4/127"]
        );
    }

    #[test]
    fn whole_ipv6_space() {
        assert_eq!(list("::-ffff:ffff:ffff:ffff:ffff:ffff:ffff:ffff"), vec!["::/0"]);
    }

    #[test]
    fn count_mode() {
        assert_eq!(run("10.0.0.5-10.0.0.20", "count").unwrap(), "5");
        assert_eq!(run("192.168.1.0-192.168.1.255", "count").unwrap(), "1");
    }

    #[test]
    fn cover_is_exact_for_random_ish_ranges() {
        // The CIDR cover must contain exactly the addresses [start, end].
        for &(s, e) in &[(0u32, 0u32), (1, 1), (5, 20), (256, 257), (1000, 9999)] {
            let cidrs = split_v4(s, e);
            // Sum of block sizes == range size, and blocks are contiguous from s.
            let mut cur = s as u64;
            for (base, prefix) in &cidrs {
                assert_eq!(*base as u64, cur, "blocks must be contiguous");
                cur += 1u64 << (32 - prefix);
            }
            assert_eq!(cur, e as u64 + 1, "cover must end exactly at end+1");
        }
    }

    #[test]
    fn reversed_range_errors() {
        assert!(run("10.0.0.20-10.0.0.5", "list").is_err());
    }

    #[test]
    fn empty_errors() {
        assert!(run("", "list").is_err());
        assert!(run("   ", "list").is_err());
    }

    #[test]
    fn garbage_errors() {
        assert!(run("not-an-ip", "list").is_err());
        assert!(run("192.168.1.0-2001:db8::", "list").is_err());
    }

    #[test]
    fn invalid_output_errors() {
        assert!(run("10.0.0.0-10.0.0.1", "bogus").is_err());
    }
}
