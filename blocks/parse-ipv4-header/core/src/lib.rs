//! gizza-ai/parse-ipv4-header core — decode a raw IPv4 packet header, given as a
//! hex string, into version, IHL, DSCP/ECN, total length, identification, flags
//! (DF/MF), fragment offset, TTL, protocol (named when known), header checksum
//! (with a validity check), source/destination addresses, and any IP options.
//! Pure-Rust, no wafer/wasm-bindgen deps.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Flags {
    /// Reserved bit (must be 0).
    pub reserved: bool,
    /// Don't Fragment.
    pub df: bool,
    /// More Fragments.
    pub mf: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Header {
    /// IP version (4 for IPv4).
    pub version: u8,
    /// Internet Header Length, in 32-bit words (5–15).
    pub ihl: u8,
    /// Header length in bytes (ihl * 4).
    pub header_length_bytes: u16,
    /// Differentiated Services Code Point (top 6 bits of the ToS byte).
    pub dscp: u8,
    /// Human name of the DSCP when a well-known class (e.g. CS0, EF, AF21).
    pub dscp_name: String,
    /// Explicit Congestion Notification (bottom 2 bits of the ToS byte).
    pub ecn: u8,
    pub ecn_name: String,
    /// Total packet length (header + data) in bytes.
    pub total_length: u16,
    /// Payload length implied by total_length - header_length_bytes.
    pub payload_length: u16,
    /// Identification field.
    pub identification: u16,
    /// Identification as 0xNNNN.
    pub identification_hex: String,
    pub flags: Flags,
    /// Fragment offset in 8-byte units (the raw 13-bit value).
    pub fragment_offset: u16,
    /// Fragment offset in bytes (fragment_offset * 8).
    pub fragment_offset_bytes: u32,
    /// Time To Live.
    pub ttl: u8,
    /// Protocol number.
    pub protocol: u8,
    /// Human name of the protocol when known (e.g. TCP, UDP, ICMP).
    pub protocol_name: String,
    /// Header checksum as 0xNNNN.
    pub header_checksum: String,
    /// Whether the header checksum is correct over the header bytes.
    pub header_checksum_valid: bool,
    pub source_address: String,
    pub destination_address: String,
    /// Raw IP options bytes (hex), present when IHL > 5. Empty otherwise.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub options_hex: String,
}

fn protocol_name(p: u8) -> &'static str {
    match p {
        0 => "HOPOPT (IPv6 Hop-by-Hop)",
        1 => "ICMP",
        2 => "IGMP",
        4 => "IPv4 (encapsulation)",
        6 => "TCP",
        8 => "EGP",
        9 => "IGP",
        17 => "UDP",
        41 => "IPv6 (encapsulation)",
        43 => "IPv6-Route",
        44 => "IPv6-Frag",
        46 => "RSVP",
        47 => "GRE",
        50 => "ESP",
        51 => "AH",
        58 => "ICMPv6",
        59 => "IPv6-NoNxt",
        60 => "IPv6-Opts",
        88 => "EIGRP",
        89 => "OSPF",
        94 => "IPIP",
        103 => "PIM",
        112 => "VRRP",
        115 => "L2TP",
        132 => "SCTP",
        137 => "MPLS-in-IP",
        _ => "Unknown",
    }
}

fn dscp_name(dscp: u8) -> &'static str {
    match dscp {
        0 => "CS0 (Best Effort)",
        8 => "CS1",
        10 => "AF11",
        12 => "AF12",
        14 => "AF13",
        16 => "CS2",
        18 => "AF21",
        20 => "AF22",
        22 => "AF23",
        24 => "CS3",
        26 => "AF31",
        28 => "AF32",
        30 => "AF33",
        32 => "CS4",
        34 => "AF41",
        36 => "AF42",
        38 => "AF43",
        40 => "CS5",
        44 => "Voice-Admit",
        46 => "EF (Expedited Forwarding)",
        48 => "CS6",
        56 => "CS7",
        _ => "—",
    }
}

fn ecn_name(ecn: u8) -> &'static str {
    match ecn {
        0 => "Not-ECT",
        1 => "ECT(1)",
        2 => "ECT(0)",
        3 => "CE (Congestion Encountered)",
        _ => "—",
    }
}

/// Strip whitespace and common separators from a hex string and parse it into
/// raw bytes.
fn parse_hex(input: &str) -> Result<Vec<u8>, String> {
    let cleaned: String = input
        .chars()
        .filter(|c| !c.is_whitespace() && *c != ':' && *c != '-' && *c != '.')
        .collect();
    let cleaned = cleaned.strip_prefix("0x").unwrap_or(&cleaned);
    let cleaned = cleaned.strip_prefix("0X").unwrap_or(cleaned);
    if cleaned.is_empty() {
        return Err("input is empty — paste the IPv4 header as a hex string".into());
    }
    if cleaned.len() % 2 != 0 {
        return Err(format!(
            "hex has an odd number of digits ({}) — each byte needs two hex digits",
            cleaned.len()
        ));
    }
    let mut bytes = Vec::with_capacity(cleaned.len() / 2);
    let chars: Vec<char> = cleaned.chars().collect();
    for pair in chars.chunks(2) {
        let s: String = pair.iter().collect();
        let b = u8::from_str_radix(&s, 16).map_err(|_| format!("'{s}' is not a valid hex byte"))?;
        bytes.push(b);
    }
    Ok(bytes)
}

fn ipv4_addr(bytes: &[u8]) -> String {
    format!("{}.{}.{}.{}", bytes[0], bytes[1], bytes[2], bytes[3])
}

/// One's-complement 16-bit checksum over a byte slice (RFC 1071).
fn checksum(bytes: &[u8]) -> u16 {
    let mut sum: u32 = 0;
    let mut i = 0;
    while i + 1 < bytes.len() {
        sum += u16::from_be_bytes([bytes[i], bytes[i + 1]]) as u32;
        i += 2;
    }
    if i < bytes.len() {
        sum += (bytes[i] as u32) << 8;
    }
    while (sum >> 16) != 0 {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    !(sum as u16)
}

/// Parse an IPv4 packet header given as hex.
pub fn parse(input: &str) -> Result<Header, String> {
    let bytes = parse_hex(input)?;
    if bytes.len() < 20 {
        return Err(format!(
            "input is only {} bytes — an IPv4 header needs at least 20",
            bytes.len()
        ));
    }

    let version = bytes[0] >> 4;
    if version != 4 {
        return Err(format!(
            "version field is {version}, not 4 — this does not look like an IPv4 header"
        ));
    }
    let ihl = bytes[0] & 0x0f;
    if ihl < 5 {
        return Err(format!(
            "IHL is {ihl} (< 5) — the header length must be at least 5 words (20 bytes)"
        ));
    }
    let header_length_bytes = (ihl as u16) * 4;
    if bytes.len() < header_length_bytes as usize {
        return Err(format!(
            "IHL says the header is {header_length_bytes} bytes but only {} bytes were given",
            bytes.len()
        ));
    }

    let tos = bytes[1];
    let dscp = tos >> 2;
    let ecn = tos & 0x03;

    let total_length = u16::from_be_bytes([bytes[2], bytes[3]]);
    let payload_length = total_length.saturating_sub(header_length_bytes);

    let identification = u16::from_be_bytes([bytes[4], bytes[5]]);

    let flags_frag = u16::from_be_bytes([bytes[6], bytes[7]]);
    let flags = Flags {
        reserved: (flags_frag & 0x8000) != 0,
        df: (flags_frag & 0x4000) != 0,
        mf: (flags_frag & 0x2000) != 0,
    };
    let fragment_offset = flags_frag & 0x1fff;

    let ttl = bytes[8];
    let protocol = bytes[9];
    let checksum_field = u16::from_be_bytes([bytes[10], bytes[11]]);

    let source_address = ipv4_addr(&bytes[12..16]);
    let destination_address = ipv4_addr(&bytes[16..20]);

    // Validate the header checksum: the computed checksum over the full header
    // (with the checksum field in place) must be 0.
    let header_checksum_valid = checksum(&bytes[0..header_length_bytes as usize]) == 0;

    let options_hex = if ihl > 5 {
        bytes[20..header_length_bytes as usize]
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    } else {
        String::new()
    };

    Ok(Header {
        version,
        ihl,
        header_length_bytes,
        dscp,
        dscp_name: dscp_name(dscp).to_string(),
        ecn,
        ecn_name: ecn_name(ecn).to_string(),
        total_length,
        payload_length,
        identification,
        identification_hex: format!("0x{identification:04x}"),
        flags,
        fragment_offset,
        fragment_offset_bytes: (fragment_offset as u32) * 8,
        ttl,
        protocol,
        protocol_name: protocol_name(protocol).to_string(),
        header_checksum: format!("0x{checksum_field:04x}"),
        header_checksum_valid,
        source_address,
        destination_address,
        options_hex,
    })
}

/// Parse and return as pretty JSON (chat / programmatic surface).
pub fn run(input: &str) -> Result<String, String> {
    let h = parse(input)?;
    serde_json::to_string_pretty(&h).map_err(|e| e.to_string())
}

/// Human-readable rendering (used by the page).
pub fn render(input: &str) -> Result<String, String> {
    let h = parse(input)?;
    let mut out = String::new();
    out.push_str(&format!(
        "Version:           {} (IPv4)\n",
        h.version
    ));
    out.push_str(&format!(
        "IHL:               {} words ({} bytes)\n",
        h.ihl, h.header_length_bytes
    ));
    out.push_str(&format!(
        "DSCP:              {} ({})\n",
        h.dscp, h.dscp_name
    ));
    out.push_str(&format!("ECN:               {} ({})\n", h.ecn, h.ecn_name));
    out.push_str(&format!(
        "Total Length:      {} bytes (payload {} bytes)\n",
        h.total_length, h.payload_length
    ));
    out.push_str(&format!(
        "Identification:    {} ({})\n",
        h.identification, h.identification_hex
    ));
    let mut flagstr = Vec::new();
    if h.flags.reserved {
        flagstr.push("reserved");
    }
    if h.flags.df {
        flagstr.push("DF");
    }
    if h.flags.mf {
        flagstr.push("MF");
    }
    let flagstr = if flagstr.is_empty() {
        "none".to_string()
    } else {
        flagstr.join(", ")
    };
    out.push_str(&format!("Flags:             {flagstr}\n"));
    out.push_str(&format!(
        "Fragment Offset:   {} ({} bytes)\n",
        h.fragment_offset, h.fragment_offset_bytes
    ));
    out.push_str(&format!("TTL:               {}\n", h.ttl));
    out.push_str(&format!(
        "Protocol:          {} ({})\n",
        h.protocol, h.protocol_name
    ));
    out.push_str(&format!(
        "Header Checksum:   {} ({})\n",
        h.header_checksum,
        if h.header_checksum_valid {
            "valid"
        } else {
            "INVALID"
        }
    ));
    out.push_str(&format!("Source:            {}\n", h.source_address));
    out.push_str(&format!("Destination:       {}\n", h.destination_address));
    if !h.options_hex.is_empty() {
        out.push_str(&format!("Options (hex):     {}\n", h.options_hex));
    }
    Ok(out.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Real IPv4 header: 45 00 00 3c 1c 46 40 00 40 06 9c bc c0 a8 00 68 c0 a8 00 01
    // version 4, IHL 5, ToS 0, total len 60, id 0x1c46, DF set, TTL 64, proto TCP(6),
    // checksum 0x9cbc (valid), src 192.168.0.104, dst 192.168.0.1.
    const TCP: &str = "45 00 00 3c 1c 46 40 00 40 06 9c bc c0 a8 00 68 c0 a8 00 01";

    #[test]
    fn parses_tcp_header() {
        let h = parse(TCP).unwrap();
        assert_eq!(h.version, 4);
        assert_eq!(h.ihl, 5);
        assert_eq!(h.header_length_bytes, 20);
        assert_eq!(h.dscp, 0);
        assert_eq!(h.ecn, 0);
        assert_eq!(h.total_length, 60);
        assert_eq!(h.payload_length, 40);
        assert_eq!(h.identification, 0x1c46);
        assert!(h.flags.df);
        assert!(!h.flags.mf);
        assert_eq!(h.fragment_offset, 0);
        assert_eq!(h.ttl, 64);
        assert_eq!(h.protocol, 6);
        assert_eq!(h.protocol_name, "TCP");
        assert_eq!(h.header_checksum, "0x9cbc");
        assert!(h.header_checksum_valid);
        assert_eq!(h.source_address, "192.168.0.104");
        assert_eq!(h.destination_address, "192.168.0.1");
        assert!(h.options_hex.is_empty());
    }

    #[test]
    fn detects_invalid_checksum() {
        // Same header but checksum zeroed → invalid.
        let bad = "45 00 00 3c 1c 46 40 00 40 06 00 00 c0 a8 00 68 c0 a8 00 01";
        let h = parse(bad).unwrap();
        assert!(!h.header_checksum_valid);
    }

    #[test]
    fn parses_more_fragments_and_offset() {
        // flags/frag = 0x202a → MF set, offset 0x2a = 42 (336 bytes); proto UDP(17).
        let frag = "45 00 05 dc 1c 46 20 2a 40 11 00 00 0a 00 00 01 0a 00 00 02";
        let h = parse(frag).unwrap();
        assert!(h.flags.mf);
        assert!(!h.flags.df);
        assert_eq!(h.fragment_offset, 42);
        assert_eq!(h.fragment_offset_bytes, 336);
        assert_eq!(h.protocol_name, "UDP");
    }

    #[test]
    fn parses_dscp_ecn() {
        // ToS byte 0xb8 → DSCP 46 (EF), ECN 0.
        let ef = "45 b8 00 3c 1c 46 40 00 40 06 9c 04 c0 a8 00 68 c0 a8 00 01";
        let h = parse(ef).unwrap();
        assert_eq!(h.dscp, 46);
        assert_eq!(h.dscp_name, "EF (Expedited Forwarding)");
    }

    #[test]
    fn parses_options() {
        // IHL 6 (24-byte header) with a 4-byte option (record route stub) appended.
        let withopt = "46 00 00 28 1c 46 40 00 40 06 00 00 c0 a8 00 68 c0 a8 00 01 07 04 05 00";
        let h = parse(withopt).unwrap();
        assert_eq!(h.ihl, 6);
        assert_eq!(h.header_length_bytes, 24);
        assert_eq!(h.options_hex, "07040500");
    }

    #[test]
    fn run_emits_json() {
        let j = run(TCP).unwrap();
        let v: serde_json::Value = serde_json::from_str(&j).unwrap();
        assert_eq!(v["protocol_name"], "TCP");
        assert_eq!(v["source_address"], "192.168.0.104");
        assert_eq!(v["header_checksum_valid"], true);
    }

    #[test]
    fn render_includes_fields() {
        let r = render(TCP).unwrap();
        assert!(r.contains("Version:"));
        assert!(r.contains("TCP"));
        assert!(r.contains("192.168.0.1"));
        assert!(r.contains("valid"));
    }

    #[test]
    fn accepts_separators_and_prefix() {
        let h = parse("0x4500003c1c46400040 06 9cbc c0:a8:00:68 c0-a8-00-01").unwrap();
        assert_eq!(h.protocol_name, "TCP");
    }

    #[test]
    fn errors_on_empty() {
        assert!(parse("").is_err());
    }

    #[test]
    fn errors_on_odd_hex() {
        assert!(parse("abc").is_err());
    }

    #[test]
    fn errors_on_too_short() {
        assert!(parse("4500003c1c46").is_err());
    }

    #[test]
    fn errors_on_bad_version() {
        // first nibble 6 → not IPv4
        assert!(parse("6500003c1c46400040 06 b1e6 c0a80068 c0a80001").is_err());
    }

    #[test]
    fn errors_on_bad_hex() {
        assert!(parse("zz00003c1c46400040 06 b1e6 c0a80068 c0a80001").is_err());
    }
}
