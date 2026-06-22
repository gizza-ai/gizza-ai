//! gizza-ai/strip-ipv4-header core — remove the IPv4 header from a raw packet,
//! given as a hex string, honoring the IHL field (so any IP options are stripped
//! too), and return the encapsulated payload (the transport segment, e.g. the
//! TCP/UDP datagram) as a hex string. Pure-Rust, no wafer/wasm-bindgen deps.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Stripped {
    /// IP version (4 for IPv4).
    pub version: u8,
    /// Internet Header Length, in 32-bit words (5–15).
    pub ihl: u8,
    /// Header length in bytes (ihl * 4) that was removed.
    pub header_length_bytes: u16,
    /// Protocol number of the encapsulated payload.
    pub protocol: u8,
    /// Human name of the protocol when known (e.g. TCP, UDP, ICMP).
    pub protocol_name: String,
    /// Length of the returned payload in bytes.
    pub payload_length: u16,
    /// The encapsulated payload as a lowercase hex string (no separators).
    pub payload_hex: String,
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
        return Err("input is empty — paste the IPv4 packet as a hex string".into());
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

/// Strip the IPv4 header from a raw packet given as hex, returning the
/// encapsulated payload.
pub fn strip(input: &str) -> Result<Stripped, String> {
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
            "version field is {version}, not 4 — this does not look like an IPv4 packet"
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

    let total_length = u16::from_be_bytes([bytes[2], bytes[3]]);
    let protocol = bytes[9];

    // The payload begins right after the header (which is header_length_bytes
    // long, including any options). Prefer the byte count implied by the
    // packet's Total Length field when it is present and sane (it lets us drop
    // any trailing link-layer padding), otherwise use everything after the
    // header.
    let header = header_length_bytes as usize;
    let end = if (total_length as usize) >= header && (total_length as usize) <= bytes.len() {
        total_length as usize
    } else {
        bytes.len()
    };
    let payload = &bytes[header..end];

    let payload_hex: String = payload.iter().map(|b| format!("{b:02x}")).collect();

    Ok(Stripped {
        version,
        ihl,
        header_length_bytes,
        protocol,
        protocol_name: protocol_name(protocol).to_string(),
        payload_length: payload.len() as u16,
        payload_hex,
    })
}

/// Strip and return as pretty JSON (chat / programmatic surface).
pub fn run(input: &str) -> Result<String, String> {
    let s = strip(input)?;
    serde_json::to_string_pretty(&s).map_err(|e| e.to_string())
}

/// Human-readable rendering (used by the page).
pub fn render(input: &str) -> Result<String, String> {
    let s = strip(input)?;
    let mut out = String::new();
    out.push_str(&format!(
        "Stripped header:   {} bytes (IHL {} words)\n",
        s.header_length_bytes, s.ihl
    ));
    out.push_str(&format!(
        "Payload protocol:  {} ({})\n",
        s.protocol, s.protocol_name
    ));
    out.push_str(&format!("Payload length:    {} bytes\n", s.payload_length));
    out.push_str("Payload (hex):\n");
    if s.payload_hex.is_empty() {
        out.push_str("(empty — the packet has no payload after the header)");
    } else {
        out.push_str(&s.payload_hex);
    }
    Ok(out.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Real IPv4 packet: 20-byte header + an 8-byte payload (a UDP header stub).
    // version 4, IHL 5, total len 28, proto UDP(17), src/dst arbitrary,
    // payload = 00 35 00 35 00 08 00 00.
    const PKT: &str =
        "45 00 00 1c 1c 46 40 00 40 11 00 00 c0 a8 00 68 c0 a8 00 01 00 35 00 35 00 08 00 00";

    #[test]
    fn strips_header_and_returns_payload() {
        let s = strip(PKT).unwrap();
        assert_eq!(s.version, 4);
        assert_eq!(s.ihl, 5);
        assert_eq!(s.header_length_bytes, 20);
        assert_eq!(s.protocol, 17);
        assert_eq!(s.protocol_name, "UDP");
        assert_eq!(s.payload_length, 8);
        assert_eq!(s.payload_hex, "0035003500080000");
    }

    #[test]
    fn honors_ihl_with_options() {
        // IHL 6 (24-byte header): 20 bytes + a 4-byte option, then a 4-byte
        // payload "deadbeef". Total length 0x1c = 28.
        let withopt =
            "46 00 00 1c 1c 46 40 00 40 06 00 00 c0 a8 00 68 c0 a8 00 01 07 04 05 00 de ad be ef";
        let s = strip(withopt).unwrap();
        assert_eq!(s.ihl, 6);
        assert_eq!(s.header_length_bytes, 24);
        // The 4 option bytes (07 04 05 00) must NOT appear in the payload.
        assert_eq!(s.payload_hex, "deadbeef");
        assert_eq!(s.payload_length, 4);
        assert_eq!(s.protocol_name, "TCP");
    }

    #[test]
    fn drops_trailing_padding_per_total_length() {
        // Total length 0x1c = 28 but 4 extra trailing bytes (ffffffff) of
        // link-layer padding follow — they must be dropped.
        let padded =
            "45 00 00 1c 1c 46 40 00 40 11 00 00 c0 a8 00 68 c0 a8 00 01 00 35 00 35 00 08 00 00 ff ff ff ff";
        let s = strip(padded).unwrap();
        assert_eq!(s.payload_length, 8);
        assert_eq!(s.payload_hex, "0035003500080000");
    }

    #[test]
    fn empty_payload_when_header_only() {
        // 20-byte header, total length 0x14 = 20, no payload.
        let hdr = "45 00 00 14 1c 46 40 00 40 06 00 00 c0 a8 00 68 c0 a8 00 01";
        let s = strip(hdr).unwrap();
        assert_eq!(s.payload_length, 0);
        assert!(s.payload_hex.is_empty());
    }

    #[test]
    fn accepts_separators_and_prefix() {
        let s = strip(
            "0x4500001c1c46400040 11 0000 c0:a8:00:68 c0-a8-00-01 0035 0035 0008 0000",
        )
        .unwrap();
        assert_eq!(s.protocol_name, "UDP");
        assert_eq!(s.payload_hex, "0035003500080000");
    }

    #[test]
    fn run_emits_json() {
        let j = run(PKT).unwrap();
        let v: serde_json::Value = serde_json::from_str(&j).unwrap();
        assert_eq!(v["protocol_name"], "UDP");
        assert_eq!(v["payload_hex"], "0035003500080000");
        assert_eq!(v["header_length_bytes"], 20);
    }

    #[test]
    fn render_includes_payload() {
        let r = render(PKT).unwrap();
        assert!(r.contains("Payload (hex):"));
        assert!(r.contains("0035003500080000"));
        assert!(r.contains("UDP"));
    }

    #[test]
    fn errors_on_empty() {
        assert!(strip("").is_err());
    }

    #[test]
    fn errors_on_odd_hex() {
        assert!(strip("abc").is_err());
    }

    #[test]
    fn errors_on_too_short() {
        assert!(strip("4500003c1c46").is_err());
    }

    #[test]
    fn errors_on_bad_version() {
        assert!(strip("6500001c1c46400040 11 0000 c0a80068 c0a80001 0035003500080000").is_err());
    }

    #[test]
    fn errors_on_bad_hex() {
        assert!(strip("zz00001c1c46400040 11 0000 c0a80068 c0a80001").is_err());
    }
}
