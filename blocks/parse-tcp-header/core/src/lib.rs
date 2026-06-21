//! gizza-ai/parse-tcp-header core — decode a raw TCP segment header, given as a
//! hex string, into source/destination ports, sequence and acknowledgement
//! numbers, data offset (header length), the control flags (NS/CWR/ECE/URG/ACK/
//! PSH/RST/SYN/FIN), window size, checksum, urgent pointer, and any TCP options
//! (parsed into named kind/length/value entries) when the data offset > 5.
//! Pure-Rust, no wafer/wasm-bindgen deps.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Flags {
    /// Nonce Sum (ECN, RFC 3540) — bit 8 of the data-offset/flags word.
    pub ns: bool,
    /// Congestion Window Reduced.
    pub cwr: bool,
    /// ECN-Echo.
    pub ece: bool,
    /// Urgent pointer field is significant.
    pub urg: bool,
    /// Acknowledgement field is significant.
    pub ack: bool,
    /// Push function.
    pub psh: bool,
    /// Reset the connection.
    pub rst: bool,
    /// Synchronize sequence numbers.
    pub syn: bool,
    /// No more data from sender (finish).
    pub fin: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TcpOption {
    /// Option kind number.
    pub kind: u8,
    /// Human name of the option kind when known (e.g. MSS, SACK-Permitted).
    pub name: String,
    /// Total option length in bytes (kind + length + data), when present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub length: Option<u8>,
    /// Option payload bytes as hex (empty for single-byte options).
    #[serde(skip_serializing_if = "String::is_empty")]
    pub data_hex: String,
    /// Decoded value for the well-known fixed-shape options (MSS, Window Scale).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Header {
    /// Source port.
    pub source_port: u16,
    /// Destination port.
    pub destination_port: u16,
    /// Sequence number.
    pub sequence_number: u32,
    /// Sequence number as 0xNNNNNNNN.
    pub sequence_number_hex: String,
    /// Acknowledgement number (meaningful only when the ACK flag is set).
    pub acknowledgement_number: u32,
    /// Acknowledgement number as 0xNNNNNNNN.
    pub acknowledgement_number_hex: String,
    /// Data offset in 32-bit words (5–15) — the header length in words.
    pub data_offset: u8,
    /// Header length in bytes (data_offset * 4).
    pub header_length_bytes: u16,
    /// Reserved bits (3 bits, should be 0).
    pub reserved: u8,
    pub flags: Flags,
    /// Space-separated list of the set flags (e.g. "SYN ACK"), or "none".
    pub flags_set: String,
    /// Advertised receive window size.
    pub window: u16,
    /// Checksum as 0xNNNN.
    pub checksum: String,
    /// Urgent pointer (meaningful only when the URG flag is set).
    pub urgent_pointer: u16,
    /// Parsed TCP options, present when the data offset > 5. Empty otherwise.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<TcpOption>,
}

fn option_name(kind: u8) -> &'static str {
    match kind {
        0 => "End of Option List",
        1 => "No-Operation (NOP)",
        2 => "Maximum Segment Size (MSS)",
        3 => "Window Scale",
        4 => "SACK Permitted",
        5 => "SACK",
        8 => "Timestamps",
        28 => "User Timeout",
        29 => "TCP Authentication (TCP-AO)",
        34 => "TCP Fast Open Cookie",
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
        return Err("input is empty — paste the TCP header as a hex string".into());
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

/// Parse the TCP options field (bytes after the fixed 20-byte header, up to the
/// data-offset boundary) into structured entries.
fn parse_options(opts: &[u8]) -> Vec<TcpOption> {
    let mut out = Vec::new();
    let mut i = 0;
    while i < opts.len() {
        let kind = opts[i];
        // Kinds 0 (EOL) and 1 (NOP) are single-byte options with no length.
        if kind == 0 {
            out.push(TcpOption {
                kind,
                name: option_name(kind).to_string(),
                length: None,
                data_hex: String::new(),
                value: None,
            });
            break;
        }
        if kind == 1 {
            out.push(TcpOption {
                kind,
                name: option_name(kind).to_string(),
                length: None,
                data_hex: String::new(),
                value: None,
            });
            i += 1;
            continue;
        }
        // All other kinds are length-prefixed: kind, length, then length-2 bytes.
        if i + 1 >= opts.len() {
            // Truncated option; record the kind and stop.
            out.push(TcpOption {
                kind,
                name: option_name(kind).to_string(),
                length: None,
                data_hex: String::new(),
                value: None,
            });
            break;
        }
        let len = opts[i + 1];
        let total = len as usize;
        // A valid length is at least 2 and must not run past the option area.
        if total < 2 || i + total > opts.len() {
            out.push(TcpOption {
                kind,
                name: option_name(kind).to_string(),
                length: Some(len),
                data_hex: String::new(),
                value: None,
            });
            break;
        }
        let data = &opts[i + 2..i + total];
        let data_hex: String = data.iter().map(|b| format!("{b:02x}")).collect();
        // Decode the simple fixed-shape options to a numeric value.
        let value = match kind {
            2 if data.len() == 2 => Some(u16::from_be_bytes([data[0], data[1]]) as u32),
            3 if data.len() == 1 => Some(data[0] as u32),
            _ => None,
        };
        out.push(TcpOption {
            kind,
            name: option_name(kind).to_string(),
            length: Some(len),
            data_hex,
            value,
        });
        i += total;
    }
    out
}

/// Parse a TCP segment header given as hex.
pub fn parse(input: &str) -> Result<Header, String> {
    let bytes = parse_hex(input)?;
    if bytes.len() < 20 {
        return Err(format!(
            "input is only {} bytes — a TCP header needs at least 20",
            bytes.len()
        ));
    }

    let source_port = u16::from_be_bytes([bytes[0], bytes[1]]);
    let destination_port = u16::from_be_bytes([bytes[2], bytes[3]]);
    let sequence_number = u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    let acknowledgement_number = u32::from_be_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);

    let data_offset = bytes[12] >> 4;
    if data_offset < 5 {
        return Err(format!(
            "data offset is {data_offset} (< 5) — the TCP header must be at least 5 words (20 bytes)"
        ));
    }
    let header_length_bytes = (data_offset as u16) * 4;
    if bytes.len() < header_length_bytes as usize {
        return Err(format!(
            "data offset says the header is {header_length_bytes} bytes but only {} bytes were given",
            bytes.len()
        ));
    }
    // Reserved = the low 3 bits of byte 12 (bit 0 of byte 12 is the NS flag).
    let reserved = (bytes[12] >> 1) & 0x07;

    let flags_byte = bytes[13];
    let flags = Flags {
        ns: (bytes[12] & 0x01) != 0,
        cwr: (flags_byte & 0x80) != 0,
        ece: (flags_byte & 0x40) != 0,
        urg: (flags_byte & 0x20) != 0,
        ack: (flags_byte & 0x10) != 0,
        psh: (flags_byte & 0x08) != 0,
        rst: (flags_byte & 0x04) != 0,
        syn: (flags_byte & 0x02) != 0,
        fin: (flags_byte & 0x01) != 0,
    };
    let mut set = Vec::new();
    if flags.ns {
        set.push("NS");
    }
    if flags.cwr {
        set.push("CWR");
    }
    if flags.ece {
        set.push("ECE");
    }
    if flags.urg {
        set.push("URG");
    }
    if flags.ack {
        set.push("ACK");
    }
    if flags.psh {
        set.push("PSH");
    }
    if flags.rst {
        set.push("RST");
    }
    if flags.syn {
        set.push("SYN");
    }
    if flags.fin {
        set.push("FIN");
    }
    let flags_set = if set.is_empty() {
        "none".to_string()
    } else {
        set.join(" ")
    };

    let window = u16::from_be_bytes([bytes[14], bytes[15]]);
    let checksum_field = u16::from_be_bytes([bytes[16], bytes[17]]);
    let urgent_pointer = u16::from_be_bytes([bytes[18], bytes[19]]);

    let options = if data_offset > 5 {
        parse_options(&bytes[20..header_length_bytes as usize])
    } else {
        Vec::new()
    };

    Ok(Header {
        source_port,
        destination_port,
        sequence_number,
        sequence_number_hex: format!("0x{sequence_number:08x}"),
        acknowledgement_number,
        acknowledgement_number_hex: format!("0x{acknowledgement_number:08x}"),
        data_offset,
        header_length_bytes,
        reserved,
        flags,
        flags_set,
        window,
        checksum: format!("0x{checksum_field:04x}"),
        urgent_pointer,
        options,
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
    out.push_str(&format!("Source Port:        {}\n", h.source_port));
    out.push_str(&format!("Destination Port:   {}\n", h.destination_port));
    out.push_str(&format!(
        "Sequence Number:    {} ({})\n",
        h.sequence_number, h.sequence_number_hex
    ));
    out.push_str(&format!(
        "Acknowledgement:    {} ({})\n",
        h.acknowledgement_number, h.acknowledgement_number_hex
    ));
    out.push_str(&format!(
        "Data Offset:        {} words ({} bytes)\n",
        h.data_offset, h.header_length_bytes
    ));
    out.push_str(&format!("Flags:              {}\n", h.flags_set));
    out.push_str(&format!("Window:             {}\n", h.window));
    out.push_str(&format!("Checksum:           {}\n", h.checksum));
    out.push_str(&format!("Urgent Pointer:     {}\n", h.urgent_pointer));
    if !h.options.is_empty() {
        out.push_str("Options:\n");
        for o in &h.options {
            let mut line = format!("  - kind {} ({})", o.kind, o.name);
            if let Some(v) = o.value {
                line.push_str(&format!(" = {v}"));
            } else if !o.data_hex.is_empty() {
                line.push_str(&format!(" data {}", o.data_hex));
            }
            out.push_str(&line);
            out.push('\n');
        }
    }
    Ok(out.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    // Real TCP SYN header (20 bytes, no options):
    // src port 0xc213 = 49683, dst 0x0050 = 80, seq 0x4f8e... ack 0,
    // data offset 5, flags SYN, window 0xffff, checksum 0xfe34, urg 0.
    const SYN: &str =
        "c2 13 00 50 4f 8e 6b 1a 00 00 00 00 50 02 ff ff fe 34 00 00";

    #[test]
    fn parses_syn_header() {
        let h = parse(SYN).unwrap();
        assert_eq!(h.source_port, 49683);
        assert_eq!(h.destination_port, 80);
        assert_eq!(h.sequence_number, 0x4f8e6b1a);
        assert_eq!(h.acknowledgement_number, 0);
        assert_eq!(h.data_offset, 5);
        assert_eq!(h.header_length_bytes, 20);
        assert_eq!(h.reserved, 0);
        assert!(h.flags.syn);
        assert!(!h.flags.ack);
        assert!(!h.flags.fin);
        assert_eq!(h.flags_set, "SYN");
        assert_eq!(h.window, 0xffff);
        assert_eq!(h.checksum, "0xfe34");
        assert_eq!(h.urgent_pointer, 0);
        assert!(h.options.is_empty());
    }

    #[test]
    fn parses_syn_ack_flags() {
        // flags byte 0x12 → SYN + ACK; ack number non-zero.
        let synack =
            "00 50 c2 13 11 22 33 44 4f 8e 6b 1b 50 12 72 10 ab cd 00 00";
        let h = parse(synack).unwrap();
        assert!(h.flags.syn);
        assert!(h.flags.ack);
        assert_eq!(h.flags_set, "ACK SYN");
        assert_eq!(h.acknowledgement_number, 0x4f8e6b1b);
    }

    #[test]
    fn parses_fin_psh_ack() {
        // flags byte 0x19 → FIN + PSH + ACK.
        let h = parse("00 50 c2 13 00 00 00 01 00 00 00 02 50 19 01 00 00 00 00 00").unwrap();
        assert!(h.flags.fin);
        assert!(h.flags.psh);
        assert!(h.flags.ack);
        assert!(!h.flags.syn);
        assert_eq!(h.flags_set, "ACK PSH FIN");
    }

    #[test]
    fn parses_options_mss_and_window_scale() {
        // Data offset 7 (28-byte header). Options: MSS(2) len4 = 1460 (0x05b4),
        // NOP(1), Window Scale(3) len3 = 7.
        // byte12 = 0x70 (offset 7), flags 0x02 (SYN).
        let withopts = "c2 13 00 50 4f 8e 6b 1a 00 00 00 00 \
                        70 02 ff ff fe 34 00 00 \
                        02 04 05 b4 01 03 03 07";
        let h = parse(withopts).unwrap();
        assert_eq!(h.data_offset, 7);
        assert_eq!(h.header_length_bytes, 28);
        assert_eq!(h.options.len(), 3);
        assert_eq!(h.options[0].kind, 2);
        assert_eq!(h.options[0].name, "Maximum Segment Size (MSS)");
        assert_eq!(h.options[0].length, Some(4));
        assert_eq!(h.options[0].value, Some(1460));
        assert_eq!(h.options[1].kind, 1);
        assert_eq!(h.options[1].name, "No-Operation (NOP)");
        assert_eq!(h.options[2].kind, 3);
        assert_eq!(h.options[2].value, Some(7));
    }

    #[test]
    fn parses_sack_permitted_and_timestamps() {
        // offset 8 (32 bytes). Options: SACK-Permitted(4) len2, Timestamps(8) len10.
        let withopts = "00 50 c2 13 00 00 00 01 00 00 00 02 \
                        80 10 72 10 00 00 00 00 \
                        04 02 08 0a 11 22 33 44 55 66 77 88";
        let h = parse(withopts).unwrap();
        assert_eq!(h.data_offset, 8);
        assert_eq!(h.options.len(), 2);
        assert_eq!(h.options[0].name, "SACK Permitted");
        assert_eq!(h.options[1].kind, 8);
        assert_eq!(h.options[1].name, "Timestamps");
        assert_eq!(h.options[1].length, Some(10));
        assert_eq!(h.options[1].data_hex, "1122334455667788");
    }

    #[test]
    fn parses_ns_flag_and_reserved() {
        // byte12 = 0x51 → offset 5, NS bit set; flags 0x10 (ACK).
        let h = parse("00 50 c2 13 00 00 00 01 00 00 00 00 51 10 72 10 00 00 00 00").unwrap();
        assert!(h.flags.ns);
        assert_eq!(h.flags_set, "NS ACK");
        assert_eq!(h.reserved, 0);
    }

    #[test]
    fn run_emits_json() {
        let j = run(SYN).unwrap();
        let v: serde_json::Value = serde_json::from_str(&j).unwrap();
        assert_eq!(v["source_port"], 49683);
        assert_eq!(v["destination_port"], 80);
        assert_eq!(v["flags_set"], "SYN");
        assert_eq!(v["flags"]["syn"], true);
        assert_eq!(v["checksum"], "0xfe34");
    }

    #[test]
    fn render_includes_fields() {
        let r = render(SYN).unwrap();
        assert!(r.contains("Source Port:"));
        assert!(r.contains("49683"));
        assert!(r.contains("Destination Port:"));
        assert!(r.contains("SYN"));
    }

    #[test]
    fn accepts_separators_and_prefix() {
        let h = parse("0xc21300504f8e6b1a00000000 50:02 ff-ff fe34 0000").unwrap();
        assert_eq!(h.destination_port, 80);
        assert!(h.flags.syn);
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
        assert!(parse("c21300504f8e").is_err());
    }

    #[test]
    fn errors_on_bad_offset() {
        // byte12 nibble high = 4 → data offset 4 (< 5).
        assert!(parse("00 50 c2 13 00 00 00 01 00 00 00 00 40 10 72 10 00 00 00 00").is_err());
    }

    #[test]
    fn errors_on_bad_hex() {
        assert!(parse("zz 50 c2 13 00 00 00 01 00 00 00 00 50 10 72 10 00 00 00 00").is_err());
    }
}
