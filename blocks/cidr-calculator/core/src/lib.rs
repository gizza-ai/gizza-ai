//! cidr-calculator core — compute subnet properties for an IPv4 or IPv6 CIDR.
//! Pure compute (`std::net` only), shared by the chat skill block and the web
//! page.
//!
//! Given a CIDR like `192.168.1.130/26` it derives:
//! * **network address** — the base address with host bits cleared (so a
//!   non-aligned input is normalized: `192.168.1.130/26` -> `192.168.1.128`).
//! * **broadcast address** — host bits all set (IPv4 only).
//! * **netmask** + **wildcard mask** (IPv4 only).
//! * **usable host range** — first..last assignable host (IPv4 only; for a
//!   `/31` and `/32` the special RFC-3021 / single-host cases are handled).
//! * **host / address counts**.
//! * for IPv6: network address, prefix length, total address count, and the
//!   first/last address in the block.
//!
//! `format = "text"` (default) renders an aligned human-readable report;
//! `format = "json"` renders a machine-readable JSON object (built by hand, no
//! serde dep in core).

use std::net::{Ipv4Addr, Ipv6Addr};

/// Compute the subnet report for `input` (a CIDR string) in the chosen
/// `format` (`"text"` | `"json"`).
pub fn calculate(input: &str, format: &str) -> Result<String, String> {
    let input = input.trim();
    if input.is_empty() {
        return Err("input is empty: provide a CIDR like 192.168.1.0/24 or 2001:db8::/48".into());
    }
    let fmt = format.trim();
    let fmt = if fmt.is_empty() { "text" } else { fmt };
    let fmt = fmt.to_ascii_lowercase();
    if fmt != "text" && fmt != "json" {
        return Err(format!("invalid format {format:?}: expected 'text' or 'json'"));
    }

    let report = parse(input)?;
    Ok(match fmt.as_str() {
        "json" => report.to_json(),
        _ => report.to_text(),
    })
}

/// A computed subnet, in one of the two families.
enum Report {
    V4(V4Report),
    V6(V6Report),
}

struct V4Report {
    input: String,
    prefix: u8,
    network: Ipv4Addr,
    broadcast: Ipv4Addr,
    netmask: Ipv4Addr,
    wildcard: Ipv4Addr,
    /// First usable host, if any.
    first_host: Option<Ipv4Addr>,
    /// Last usable host, if any.
    last_host: Option<Ipv4Addr>,
    /// Total addresses in the block (2^(32-prefix)).
    total: u64,
    /// Usable host count.
    usable: u64,
    /// True for an RFC 1918 / loopback / link-local etc. private network.
    is_private: bool,
}

struct V6Report {
    input: String,
    prefix: u8,
    network: Ipv6Addr,
    first: Ipv6Addr,
    last: Ipv6Addr,
    /// Total addresses as a decimal string (2^(128-prefix) overflows u128 at /0).
    total: String,
}

fn parse(input: &str) -> Result<Report, String> {
    let (addr_str, prefix_str) = input.split_once('/').ok_or_else(|| {
        format!("missing prefix length: expected ADDRESS/PREFIX (e.g. 192.168.1.0/24), got {input:?}")
    })?;
    let addr_str = addr_str.trim();
    let prefix_str = prefix_str.trim();

    if let Ok(v4) = addr_str.parse::<Ipv4Addr>() {
        let prefix: u8 = prefix_str
            .parse()
            .map_err(|_| format!("invalid prefix length {prefix_str:?}: expected 0-32 for IPv4"))?;
        if prefix > 32 {
            return Err(format!("IPv4 prefix length {prefix} out of range: expected 0-32"));
        }
        return Ok(Report::V4(calc_v4(input, v4, prefix)));
    }
    if let Ok(v6) = addr_str.parse::<Ipv6Addr>() {
        let prefix: u8 = prefix_str
            .parse()
            .map_err(|_| format!("invalid prefix length {prefix_str:?}: expected 0-128 for IPv6"))?;
        if prefix > 128 {
            return Err(format!("IPv6 prefix length {prefix} out of range: expected 0-128"));
        }
        return Ok(Report::V6(calc_v6(input, v6, prefix)));
    }
    Err(format!("invalid IP address {addr_str:?}: not a valid IPv4 or IPv6 address"))
}

fn calc_v4(input: &str, ip: Ipv4Addr, prefix: u8) -> V4Report {
    let bits = u32::from(ip);
    let mask: u32 = if prefix == 0 { 0 } else { u32::MAX << (32 - prefix) };
    let network = bits & mask;
    let broadcast = network | !mask;
    let total: u64 = 1u64 << (32 - prefix);

    // Usable host range: exclude network + broadcast for prefixes <= /30.
    // /31 (RFC 3021 point-to-point) -> both addresses usable.
    // /32 (single host) -> the address itself.
    let (first_host, last_host, usable) = match prefix {
        32 => (Some(Ipv4Addr::from(network)), Some(Ipv4Addr::from(network)), 1),
        31 => (
            Some(Ipv4Addr::from(network)),
            Some(Ipv4Addr::from(broadcast)),
            2,
        ),
        _ => {
            let first = network + 1;
            let last = broadcast - 1;
            (Some(Ipv4Addr::from(first)), Some(Ipv4Addr::from(last)), total - 2)
        }
    };

    let net_addr = Ipv4Addr::from(network);
    V4Report {
        input: input.to_string(),
        prefix,
        network: net_addr,
        broadcast: Ipv4Addr::from(broadcast),
        netmask: Ipv4Addr::from(mask),
        wildcard: Ipv4Addr::from(!mask),
        first_host,
        last_host,
        total,
        usable,
        is_private: is_private_v4(net_addr),
    }
}

/// Classify an IPv4 network address as private / special per common ranges
/// (RFC 1918 private, loopback, link-local, CGNAT, etc.).
fn is_private_v4(ip: Ipv4Addr) -> bool {
    ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_unspecified()
        // 100.64.0.0/10 — RFC 6598 carrier-grade NAT.
        || {
            let o = ip.octets();
            o[0] == 100 && (64..=127).contains(&o[1])
        }
}

fn calc_v6(input: &str, ip: Ipv6Addr, prefix: u8) -> V6Report {
    let bits = u128::from(ip);
    let mask: u128 = if prefix == 0 { 0 } else { u128::MAX << (128 - prefix) };
    let network = bits & mask;
    let last = network | !mask;

    // Total = 2^(128 - prefix). At prefix 0 that is 2^128 which overflows u128,
    // so emit that single case as a constant.
    let host_bits = 128 - prefix as u32;
    let total = if host_bits == 128 {
        "340282366920938463463374607431768211456".to_string() // 2^128
    } else {
        (1u128 << host_bits).to_string()
    };

    V6Report {
        input: input.to_string(),
        prefix,
        network: Ipv6Addr::from(network),
        first: Ipv6Addr::from(network),
        last: Ipv6Addr::from(last),
        total,
    }
}

impl Report {
    fn to_text(&self) -> String {
        match self {
            Report::V4(r) => r.to_text(),
            Report::V6(r) => r.to_text(),
        }
    }
    fn to_json(&self) -> String {
        match self {
            Report::V4(r) => r.to_json(),
            Report::V6(r) => r.to_json(),
        }
    }
}

impl V4Report {
    fn to_text(&self) -> String {
        let host_line = match (self.first_host, self.last_host) {
            (Some(f), Some(l)) if f == l => format!("{f}"),
            (Some(f), Some(l)) => format!("{f} - {l}"),
            _ => "(none)".to_string(),
        };
        let scope = if self.is_private { "Private" } else { "Public" };
        format!(
            "CIDR:               {input}\n\
             Address family:     IPv4\n\
             Network address:    {network}\n\
             Broadcast address:  {broadcast}\n\
             Netmask:            {netmask} (/{prefix})\n\
             Wildcard mask:      {wildcard}\n\
             Usable host range:  {host_line}\n\
             Total addresses:    {total}\n\
             Usable hosts:       {usable}\n\
             Scope:              {scope}",
            input = self.input,
            network = self.network,
            broadcast = self.broadcast,
            netmask = self.netmask,
            prefix = self.prefix,
            wildcard = self.wildcard,
            host_line = host_line,
            total = self.total,
            usable = self.usable,
            scope = scope,
        )
    }
    fn to_json(&self) -> String {
        let q = |v: &str| v.replace('\\', "\\\\").replace('"', "\\\"");
        let opt = |o: Option<Ipv4Addr>| match o {
            Some(a) => format!("\"{a}\""),
            None => "null".to_string(),
        };
        format!(
            "{{\n  \"input\": \"{input}\",\n  \"family\": \"IPv4\",\n  \"prefix\": {prefix},\n  \
             \"network\": \"{network}\",\n  \"broadcast\": \"{broadcast}\",\n  \
             \"netmask\": \"{netmask}\",\n  \"wildcard\": \"{wildcard}\",\n  \
             \"first_host\": {first},\n  \"last_host\": {last},\n  \
             \"total_addresses\": {total},\n  \"usable_hosts\": {usable},\n  \
             \"is_private\": {is_private}\n}}",
            input = q(&self.input),
            prefix = self.prefix,
            network = self.network,
            broadcast = self.broadcast,
            netmask = self.netmask,
            wildcard = self.wildcard,
            first = opt(self.first_host),
            last = opt(self.last_host),
            total = self.total,
            usable = self.usable,
            is_private = self.is_private,
        )
    }
}

impl V6Report {
    fn to_text(&self) -> String {
        format!(
            "CIDR:               {input}\n\
             Address family:     IPv6\n\
             Network address:    {network}\n\
             Prefix length:      /{prefix}\n\
             First address:      {first}\n\
             Last address:       {last}\n\
             Total addresses:    {total}",
            input = self.input,
            network = self.network,
            prefix = self.prefix,
            first = self.first,
            last = self.last,
            total = self.total,
        )
    }
    fn to_json(&self) -> String {
        let q = |v: &str| v.replace('\\', "\\\\").replace('"', "\\\"");
        format!(
            "{{\n  \"input\": \"{input}\",\n  \"family\": \"IPv6\",\n  \"prefix\": {prefix},\n  \
             \"network\": \"{network}\",\n  \"first_address\": \"{first}\",\n  \
             \"last_address\": \"{last}\",\n  \"total_addresses\": \"{total}\"\n}}",
            input = q(&self.input),
            prefix = self.prefix,
            network = self.network,
            first = self.first,
            last = self.last,
            total = self.total,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_24() {
        let out = calculate("192.168.1.0/24", "text").unwrap();
        assert!(out.contains("Network address:    192.168.1.0"));
        assert!(out.contains("Broadcast address:  192.168.1.255"));
        assert!(out.contains("Netmask:            255.255.255.0 (/24)"));
        assert!(out.contains("Wildcard mask:      0.0.0.255"));
        assert!(out.contains("Usable host range:  192.168.1.1 - 192.168.1.254"));
        assert!(out.contains("Total addresses:    256"));
        assert!(out.contains("Usable hosts:       254"));
        assert!(out.contains("Scope:              Private"));
    }

    #[test]
    fn non_aligned_normalized() {
        let out = calculate("192.168.1.130/26", "text").unwrap();
        assert!(out.contains("Network address:    192.168.1.128"));
        assert!(out.contains("Broadcast address:  192.168.1.191"));
        assert!(out.contains("Usable host range:  192.168.1.129 - 192.168.1.190"));
        assert!(out.contains("Total addresses:    64"));
        assert!(out.contains("Usable hosts:       62"));
    }

    #[test]
    fn slash_30() {
        let out = calculate("10.0.0.0/30", "text").unwrap();
        assert!(out.contains("Total addresses:    4"));
        assert!(out.contains("Usable hosts:       2"));
        assert!(out.contains("Usable host range:  10.0.0.1 - 10.0.0.2"));
    }

    #[test]
    fn slash_31_point_to_point() {
        let out = calculate("10.0.0.0/31", "text").unwrap();
        assert!(out.contains("Total addresses:    2"));
        assert!(out.contains("Usable hosts:       2"));
        assert!(out.contains("Usable host range:  10.0.0.0 - 10.0.0.1"));
    }

    #[test]
    fn slash_32_single_host() {
        let out = calculate("8.8.8.8/32", "text").unwrap();
        assert!(out.contains("Total addresses:    1"));
        assert!(out.contains("Usable hosts:       1"));
        assert!(out.contains("Usable host range:  8.8.8.8\n"));
        assert!(out.contains("Scope:              Public"));
    }

    #[test]
    fn slash_0_whole_space() {
        let out = calculate("0.0.0.0/0", "text").unwrap();
        assert!(out.contains("Netmask:            0.0.0.0 (/0)"));
        assert!(out.contains("Total addresses:    4294967296"));
        assert!(out.contains("Usable hosts:       4294967294"));
    }

    #[test]
    fn private_classification() {
        assert!(calculate("10.5.0.0/16", "text").unwrap().contains("Scope:              Private"));
        assert!(calculate("172.16.0.0/12", "text").unwrap().contains("Scope:              Private"));
        assert!(calculate("100.64.0.0/10", "text").unwrap().contains("Scope:              Private"));
        assert!(calculate("1.1.1.0/24", "text").unwrap().contains("Scope:              Public"));
    }

    #[test]
    fn ipv6_basic() {
        let out = calculate("2001:db8::/48", "text").unwrap();
        assert!(out.contains("Address family:     IPv6"));
        assert!(out.contains("Network address:    2001:db8::"));
        assert!(out.contains("Prefix length:      /48"));
        assert!(out.contains("Total addresses:    1208925819614629174706176")); // 2^80
    }

    #[test]
    fn ipv6_normalized_and_last() {
        let out = calculate("2001:db8::abcd/126", "text").unwrap();
        assert!(out.contains("Network address:    2001:db8::abcc"));
        assert!(out.contains("Last address:       2001:db8::abcf"));
        assert!(out.contains("Total addresses:    4"));
    }

    #[test]
    fn ipv6_slash_0() {
        let out = calculate("::/0", "text").unwrap();
        assert!(out.contains("Total addresses:    340282366920938463463374607431768211456"));
    }

    #[test]
    fn json_v4() {
        let out = calculate("192.168.1.0/24", "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["family"], "IPv4");
        assert_eq!(v["network"], "192.168.1.0");
        assert_eq!(v["broadcast"], "192.168.1.255");
        assert_eq!(v["netmask"], "255.255.255.0");
        assert_eq!(v["wildcard"], "0.0.0.255");
        assert_eq!(v["first_host"], "192.168.1.1");
        assert_eq!(v["last_host"], "192.168.1.254");
        assert_eq!(v["total_addresses"], 256);
        assert_eq!(v["usable_hosts"], 254);
        assert_eq!(v["is_private"], true);
        assert_eq!(v["prefix"], 24);
    }

    #[test]
    fn json_v6() {
        let out = calculate("2001:db8::/126", "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["family"], "IPv6");
        assert_eq!(v["network"], "2001:db8::");
        assert_eq!(v["last_address"], "2001:db8::3");
        assert_eq!(v["total_addresses"], "4");
    }

    #[test]
    fn errors() {
        assert!(calculate("", "text").is_err());
        assert!(calculate("192.168.1.0", "text").is_err());
        assert!(calculate("192.168.1.0/33", "text").is_err());
        assert!(calculate("2001:db8::/129", "text").is_err());
        assert!(calculate("not-an-ip/24", "text").is_err());
        assert!(calculate("192.168.1.0/abc", "text").is_err());
        assert!(calculate("192.168.1.0/24", "xml").is_err());
    }
}
