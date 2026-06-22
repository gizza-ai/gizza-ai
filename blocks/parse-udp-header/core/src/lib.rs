//! gizza-ai/parse-udp-header core — decode a raw UDP datagram header (RFC 768),
//! given as a hex string, into its four 16-bit fields: source port, destination
//! port, length (header + data), and checksum. Also reports the implied payload
//! length (length - 8), whether the UDP checksum is disabled (0x0000 over IPv4),
//! and the well-known service name for either port when recognised.
//! Pure-Rust, no wafer/wasm-bindgen deps.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Header {
    /// Source port.
    pub source_port: u16,
    /// Well-known service name for the source port, when recognised.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_service: Option<&'static str>,
    /// Destination port.
    pub destination_port: u16,
    /// Well-known service name for the destination port, when recognised.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub destination_service: Option<&'static str>,
    /// Total datagram length in bytes (the 8-byte header plus the data).
    pub length: u16,
    /// Implied payload (data) length in bytes (length - 8), when length >= 8.
    pub payload_length: u16,
    /// Stored 16-bit checksum value as 0xNNNN.
    pub checksum: String,
    /// True when the checksum field is 0x0000 — checksum disabled (allowed over IPv4).
    pub checksum_disabled: bool,
}

/// Well-known UDP service names for a handful of common ports.
fn service_name(port: u16) -> Option<&'static str> {
    Some(match port {
        7 => "echo",
        19 => "chargen",
        53 => "DNS",
        67 => "DHCP (server)",
        68 => "DHCP (client)",
        69 => "TFTP",
        123 => "NTP",
        137 => "NetBIOS Name",
        138 => "NetBIOS Datagram",
        161 => "SNMP",
        162 => "SNMP Trap",
        443 => "QUIC / HTTP-3",
        500 => "IKE / IPsec",
        514 => "syslog",
        520 => "RIP",
        1194 => "OpenVPN",
        1701 => "L2TP",
        1812 => "RADIUS (auth)",
        1813 => "RADIUS (acct)",
        1900 => "SSDP",
        4500 => "IPsec NAT-T",
        5353 => "mDNS",
        51820 => "WireGuard",
        _ => return None,
    })
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
        return Err("input is empty — paste the UDP header as a hex string".into());
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

/// Parse a UDP datagram header given as hex.
pub fn parse(input: &str) -> Result<Header, String> {
    let bytes = parse_hex(input)?;
    if bytes.len() < 8 {
        return Err(format!(
            "input is only {} bytes — a UDP header is exactly 8 bytes",
            bytes.len()
        ));
    }

    let source_port = u16::from_be_bytes([bytes[0], bytes[1]]);
    let destination_port = u16::from_be_bytes([bytes[2], bytes[3]]);
    let length = u16::from_be_bytes([bytes[4], bytes[5]]);
    let checksum_field = u16::from_be_bytes([bytes[6], bytes[7]]);

    // The length field counts the 8-byte header plus the data; a valid datagram
    // therefore has length >= 8. payload_length saturates at 0 for malformed input.
    let payload_length = length.saturating_sub(8);

    Ok(Header {
        source_port,
        source_service: service_name(source_port),
        destination_port,
        destination_service: service_name(destination_port),
        length,
        payload_length,
        checksum: format!("0x{checksum_field:04x}"),
        checksum_disabled: checksum_field == 0,
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
    let src = match h.source_service {
        Some(s) => format!("{} ({})", h.source_port, s),
        None => h.source_port.to_string(),
    };
    let dst = match h.destination_service {
        Some(s) => format!("{} ({})", h.destination_port, s),
        None => h.destination_port.to_string(),
    };
    out.push_str(&format!("Source Port:        {src}\n"));
    out.push_str(&format!("Destination Port:   {dst}\n"));
    out.push_str(&format!(
        "Length:             {} bytes (8-byte header + {} payload)\n",
        h.length, h.payload_length
    ));
    let cksum = if h.checksum_disabled {
        format!("{} (disabled)", h.checksum)
    } else {
        h.checksum.clone()
    };
    out.push_str(&format!("Checksum:           {cksum}"));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Real DNS query header: src port 0xc3d2 = 50130, dst 0x0035 = 53 (DNS),
    // length 0x0028 = 40 bytes, checksum 0x1b6e.
    const DNS: &str = "c3 d2 00 35 00 28 1b 6e";

    #[test]
    fn parses_dns_query_header() {
        let h = parse(DNS).unwrap();
        assert_eq!(h.source_port, 50130);
        assert_eq!(h.destination_port, 53);
        assert_eq!(h.destination_service, Some("DNS"));
        assert_eq!(h.source_service, None);
        assert_eq!(h.length, 40);
        assert_eq!(h.payload_length, 32);
        assert_eq!(h.checksum, "0x1b6e");
        assert!(!h.checksum_disabled);
    }

    #[test]
    fn ignores_trailing_payload_bytes() {
        // 8-byte header followed by payload bytes — only the first 8 are read.
        let h = parse("c3 d2 00 35 00 28 1b 6e de ad be ef").unwrap();
        assert_eq!(h.source_port, 50130);
        assert_eq!(h.destination_port, 53);
        assert_eq!(h.length, 40);
    }

    #[test]
    fn detects_disabled_checksum() {
        // checksum 0x0000 = disabled (legal over IPv4).
        let h = parse("00 35 c3 d2 00 1c 00 00").unwrap();
        assert!(h.checksum_disabled);
        assert_eq!(h.checksum, "0x0000");
        assert_eq!(h.source_port, 53);
        assert_eq!(h.source_service, Some("DNS"));
    }

    #[test]
    fn recognises_ntp_and_quic_services() {
        let ntp = parse("00 7b 00 7b 00 38 12 34").unwrap();
        assert_eq!(ntp.source_service, Some("NTP"));
        assert_eq!(ntp.destination_service, Some("NTP"));
        let quic = parse("c0 00 01 bb 04 d2 ab cd").unwrap();
        assert_eq!(quic.destination_service, Some("QUIC / HTTP-3"));
        assert_eq!(quic.destination_port, 443);
    }

    #[test]
    fn header_only_has_zero_payload() {
        // length 0x0008 = 8 → header only, no data.
        let h = parse("04 d2 00 50 00 08 ff ff").unwrap();
        assert_eq!(h.length, 8);
        assert_eq!(h.payload_length, 0);
    }

    #[test]
    fn run_emits_json() {
        let j = run(DNS).unwrap();
        let v: serde_json::Value = serde_json::from_str(&j).unwrap();
        assert_eq!(v["source_port"], 50130);
        assert_eq!(v["destination_port"], 53);
        assert_eq!(v["destination_service"], "DNS");
        assert_eq!(v["length"], 40);
        assert_eq!(v["payload_length"], 32);
        assert_eq!(v["checksum"], "0x1b6e");
        assert_eq!(v["checksum_disabled"], false);
    }

    #[test]
    fn json_omits_absent_services() {
        // both ports ephemeral / unknown → service fields skipped.
        let j = run("c3 d2 c3 d3 00 28 1b 6e").unwrap();
        let v: serde_json::Value = serde_json::from_str(&j).unwrap();
        assert!(v.get("source_service").is_none());
        assert!(v.get("destination_service").is_none());
    }

    #[test]
    fn render_includes_fields() {
        let r = render(DNS).unwrap();
        assert!(r.contains("Source Port:"));
        assert!(r.contains("50130"));
        assert!(r.contains("Destination Port:"));
        assert!(r.contains("53 (DNS)"));
        assert!(r.contains("Length:"));
        assert!(r.contains("40 bytes"));
        assert!(r.contains("0x1b6e"));
    }

    #[test]
    fn render_marks_disabled_checksum() {
        let r = render("00 35 c3 d2 00 1c 00 00").unwrap();
        assert!(r.contains("(disabled)"));
    }

    #[test]
    fn accepts_separators_and_prefix() {
        let h = parse("0xc3d2:0035-0028.1b6e").unwrap();
        assert_eq!(h.source_port, 50130);
        assert_eq!(h.destination_port, 53);
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
        assert!(parse("c3 d2 00 35").is_err());
    }

    #[test]
    fn errors_on_bad_hex() {
        assert!(parse("zz d2 00 35 00 28 1b 6e").is_err());
    }
}
