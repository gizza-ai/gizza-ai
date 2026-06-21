//! gizza-ai/strip-udp-header core — remove the fixed 8-byte UDP header from a
//! raw UDP datagram, given as a hex string, and return the encapsulated
//! application payload as a hex string, plus the decoded header fields. The UDP
//! Length field is honored to drop any trailing link-layer padding. Pure-Rust,
//! no wafer/wasm-bindgen deps.

use serde::Serialize;

/// The fixed length of a UDP header, in bytes (RFC 768): src port, dst port,
/// length, checksum — two bytes each.
const UDP_HEADER_LEN: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Stripped {
    /// Source port.
    pub source_port: u16,
    /// Destination port.
    pub destination_port: u16,
    /// The UDP Length field: header + data, in bytes (8–65535).
    pub length: u16,
    /// The UDP Checksum field (0 means "not computed").
    pub checksum: u16,
    /// Number of header bytes removed (always 8).
    pub header_length_bytes: u16,
    /// Length of the returned payload in bytes.
    pub payload_length: u16,
    /// The encapsulated payload as a lowercase hex string (no separators).
    pub payload_hex: String,
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
        return Err("input is empty — paste the UDP datagram as a hex string".into());
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

/// Strip the 8-byte UDP header from a raw datagram given as hex, returning the
/// encapsulated application payload and the decoded header fields.
pub fn strip(input: &str) -> Result<Stripped, String> {
    let bytes = parse_hex(input)?;
    if bytes.len() < UDP_HEADER_LEN {
        return Err(format!(
            "input is only {} bytes — a UDP header is a fixed 8 bytes",
            bytes.len()
        ));
    }

    let source_port = u16::from_be_bytes([bytes[0], bytes[1]]);
    let destination_port = u16::from_be_bytes([bytes[2], bytes[3]]);
    let length = u16::from_be_bytes([bytes[4], bytes[5]]);
    let checksum = u16::from_be_bytes([bytes[6], bytes[7]]);

    // The payload begins right after the fixed 8-byte header. Prefer the byte
    // count implied by the UDP Length field when it is present and sane (it lets
    // us drop any trailing link-layer padding), otherwise use everything after
    // the header.
    let end = if (length as usize) >= UDP_HEADER_LEN && (length as usize) <= bytes.len() {
        length as usize
    } else {
        bytes.len()
    };
    let payload = &bytes[UDP_HEADER_LEN..end];

    let payload_hex: String = payload.iter().map(|b| format!("{b:02x}")).collect();

    Ok(Stripped {
        source_port,
        destination_port,
        length,
        checksum,
        header_length_bytes: UDP_HEADER_LEN as u16,
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
    out.push_str(&format!("Source port:       {}\n", s.source_port));
    out.push_str(&format!("Destination port:  {}\n", s.destination_port));
    out.push_str(&format!(
        "UDP length field:  {} bytes (header + data)\n",
        s.length
    ));
    out.push_str(&format!(
        "Checksum:          0x{:04x}{}\n",
        s.checksum,
        if s.checksum == 0 {
            " (not computed)"
        } else {
            ""
        }
    ));
    out.push_str(&format!(
        "Stripped header:   {} bytes\n",
        s.header_length_bytes
    ));
    out.push_str(&format!("Payload length:    {} bytes\n", s.payload_length));
    out.push_str("Payload (hex):\n");
    if s.payload_hex.is_empty() {
        out.push_str("(empty — the datagram has no payload after the header)");
    } else {
        out.push_str(&s.payload_hex);
    }
    Ok(out.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    // A real UDP datagram: 8-byte header + a 4-byte payload "deadbeef".
    // src port 0x0035 (53, DNS), dst port 0xa1b2 (41394), length 0x000c = 12
    // (8 header + 4 data), checksum 0x1234.
    const DGRAM: &str = "00 35 a1 b2 00 0c 12 34 de ad be ef";

    #[test]
    fn strips_header_and_returns_payload() {
        let s = strip(DGRAM).unwrap();
        assert_eq!(s.source_port, 53);
        assert_eq!(s.destination_port, 41394);
        assert_eq!(s.length, 12);
        assert_eq!(s.checksum, 0x1234);
        assert_eq!(s.header_length_bytes, 8);
        assert_eq!(s.payload_length, 4);
        assert_eq!(s.payload_hex, "deadbeef");
    }

    #[test]
    fn drops_trailing_padding_per_length_field() {
        // UDP length field 0x000c = 12 but 4 extra trailing bytes (ffffffff) of
        // link-layer padding follow — they must be dropped.
        let padded = "00 35 a1 b2 00 0c 12 34 de ad be ef ff ff ff ff";
        let s = strip(padded).unwrap();
        assert_eq!(s.payload_length, 4);
        assert_eq!(s.payload_hex, "deadbeef");
    }

    #[test]
    fn empty_payload_when_header_only() {
        // 8-byte header, length 0x0008 = 8, no payload.
        let hdr = "13 88 13 89 00 08 00 00";
        let s = strip(hdr).unwrap();
        assert_eq!(s.source_port, 5000);
        assert_eq!(s.destination_port, 5001);
        assert_eq!(s.length, 8);
        assert_eq!(s.payload_length, 0);
        assert!(s.payload_hex.is_empty());
    }

    #[test]
    fn checksum_zero_means_not_computed() {
        let s = strip("00 35 a1 b2 00 09 00 00 ff").unwrap();
        assert_eq!(s.checksum, 0);
        assert_eq!(s.payload_hex, "ff");
    }

    #[test]
    fn ignores_bogus_length_field() {
        // length field claims 0xffff but only 10 bytes were given — fall back to
        // using everything after the 8-byte header.
        let s = strip("00 35 a1 b2 ff ff 00 00 ab cd").unwrap();
        assert_eq!(s.payload_length, 2);
        assert_eq!(s.payload_hex, "abcd");
    }

    #[test]
    fn accepts_separators_and_prefix() {
        let s = strip("0x0035 a1:b2 00-0c 12.34 deadbeef").unwrap();
        assert_eq!(s.source_port, 53);
        assert_eq!(s.payload_hex, "deadbeef");
    }

    #[test]
    fn run_emits_json() {
        let j = run(DGRAM).unwrap();
        let v: serde_json::Value = serde_json::from_str(&j).unwrap();
        assert_eq!(v["source_port"], 53);
        assert_eq!(v["destination_port"], 41394);
        assert_eq!(v["payload_hex"], "deadbeef");
        assert_eq!(v["header_length_bytes"], 8);
    }

    #[test]
    fn render_includes_payload() {
        let r = render(DGRAM).unwrap();
        assert!(r.contains("Payload (hex):"));
        assert!(r.contains("deadbeef"));
        assert!(r.contains("Source port:"));
        assert!(r.contains("53"));
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
        assert!(strip("0035a1b200").is_err());
    }

    #[test]
    fn errors_on_bad_hex() {
        assert!(strip("zz 35 a1 b2 00 0c 12 34").is_err());
    }
}
