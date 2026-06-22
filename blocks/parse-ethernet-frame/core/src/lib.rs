//! gizza-ai/parse-ethernet-frame core — decode an Ethernet II / IEEE 802.3
//! frame given as a hex string into destination/source MAC, EtherType or
//! length, optional 802.1Q / 802.1ad VLAN tags, and the payload.
//! Pure-Rust, no wafer/wasm-bindgen deps.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Vlan {
    /// Tag Protocol Identifier of the tag (e.g. 0x8100 C-Tag, 0x88a8 S-Tag).
    pub tpid: String,
    pub tpid_name: String,
    /// Priority Code Point (802.1p), 0–7.
    pub pcp: u8,
    /// Drop Eligible Indicator (formerly CFI).
    pub dei: bool,
    /// VLAN Identifier, 0–4095.
    pub vid: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Frame {
    pub destination_mac: String,
    pub source_mac: String,
    /// True when the destination MAC is the broadcast address ff:ff:ff:ff:ff:ff.
    pub broadcast: bool,
    /// True when the destination MAC has the I/G (multicast) bit set.
    pub multicast: bool,
    /// True when the source MAC has the locally-administered (U/L) bit set.
    pub locally_administered: bool,
    /// VLAN tags in wire order (outer first). Empty for an untagged frame.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub vlan_tags: Vec<Vlan>,
    /// The frame type field interpreted as an EtherType (>= 0x0600).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ethertype: Option<String>,
    /// Human name of the EtherType when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ethertype_name: Option<String>,
    /// The frame type field interpreted as an 802.3 length (<= 1500).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub length: Option<u16>,
    /// "Ethernet II" or "IEEE 802.3".
    pub frame_format: String,
    /// Payload length in bytes.
    pub payload_length: usize,
    /// The payload, as a hex string.
    pub payload_hex: String,
}

fn ethertype_name(et: u16) -> Option<&'static str> {
    Some(match et {
        0x0800 => "IPv4",
        0x0806 => "ARP",
        0x0842 => "Wake-on-LAN",
        0x22f0 => "AVTP",
        0x22f3 => "TRILL",
        0x8035 => "RARP",
        0x809b => "AppleTalk (EtherTalk)",
        0x80f3 => "AARP",
        0x8100 => "802.1Q VLAN (C-Tag)",
        0x8137 => "IPX",
        0x86dd => "IPv6",
        0x8808 => "Ethernet Flow Control",
        0x8809 => "LACP / Slow Protocols",
        0x8819 => "CobraNet",
        0x8847 => "MPLS unicast",
        0x8848 => "MPLS multicast",
        0x8863 => "PPPoE Discovery",
        0x8864 => "PPPoE Session",
        0x886c => "HomePlug",
        0x886d => "Intel ANS",
        0x8870 => "Jumbo Frames",
        0x887b => "HomePlug AV MME",
        0x888e => "EAP over LAN (802.1X)",
        0x8892 => "PROFINET",
        0x889a => "HyperSCSI",
        0x88a2 => "ATA over Ethernet",
        0x88a4 => "EtherCAT",
        0x88a8 => "802.1ad VLAN (S-Tag / Q-in-Q)",
        0x88ab => "Ethernet Powerlink",
        0x88cc => "LLDP",
        0x88cd => "SERCOS III",
        0x88e1 => "HomePlug AV",
        0x88e3 => "MRP (802.1Q)",
        0x88e5 => "MACsec (802.1AE)",
        0x88e7 => "PBB (802.1ah)",
        0x88f7 => "PTP (1588v2)",
        0x88f8 => "NC-SI",
        0x88fb => "PRP",
        0x8902 => "CFM / OAM (802.1ag)",
        0x8906 => "FCoE",
        0x8914 => "FCoE Initialization",
        0x8915 => "RoCE",
        0x891d => "TTE",
        0x893a => "1905.1",
        0x9000 => "Ethernet Configuration Testing (Loopback)",
        _ => return None,
    })
}

fn tpid_name(tpid: u16) -> &'static str {
    match tpid {
        0x8100 => "802.1Q (C-Tag)",
        0x88a8 => "802.1ad (S-Tag)",
        0x9100 => "802.1QinQ (legacy)",
        _ => "VLAN",
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
        return Err("input is empty — paste the frame as a hex string".into());
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
        let b = u8::from_str_radix(&s, 16)
            .map_err(|_| format!("'{s}' is not a valid hex byte"))?;
        bytes.push(b);
    }
    Ok(bytes)
}

fn mac(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<Vec<_>>()
        .join(":")
}

/// Parse an Ethernet frame (without preamble/SFD/FCS) given as hex.
pub fn parse(input: &str) -> Result<Frame, String> {
    let bytes = parse_hex(input)?;
    if bytes.len() < 14 {
        return Err(format!(
            "frame is only {} bytes — an Ethernet header needs at least 14 (6 dst + 6 src + 2 type)",
            bytes.len()
        ));
    }

    let dst = &bytes[0..6];
    let src = &bytes[6..12];
    let destination_mac = mac(dst);
    let source_mac = mac(src);
    let broadcast = dst.iter().all(|b| *b == 0xff);
    let multicast = (dst[0] & 0x01) != 0;
    let locally_administered = (src[0] & 0x02) != 0;

    // Walk type/VLAN fields. The 802.1Q/802.1ad tags chain: a TPID of
    // 0x8100 / 0x88a8 / 0x9100 is followed by a 2-byte TCI then the next type.
    let mut pos = 12usize;
    let mut vlan_tags: Vec<Vlan> = Vec::new();
    loop {
        if pos + 2 > bytes.len() {
            return Err("frame truncated inside a VLAN tag / type field".into());
        }
        let field = u16::from_be_bytes([bytes[pos], bytes[pos + 1]]);
        if matches!(field, 0x8100 | 0x88a8 | 0x9100) {
            if pos + 4 > bytes.len() {
                return Err("frame truncated inside an 802.1Q VLAN tag".into());
            }
            let tci = u16::from_be_bytes([bytes[pos + 2], bytes[pos + 3]]);
            vlan_tags.push(Vlan {
                tpid: format!("0x{field:04x}"),
                tpid_name: tpid_name(field).to_string(),
                pcp: ((tci >> 13) & 0x07) as u8,
                dei: (tci & 0x1000) != 0,
                vid: tci & 0x0fff,
            });
            pos += 4;
            continue;
        }
        break;
    }

    if pos + 2 > bytes.len() {
        return Err("frame truncated — no EtherType / length field present".into());
    }
    let type_field = u16::from_be_bytes([bytes[pos], bytes[pos + 1]]);
    pos += 2;

    let payload = &bytes[pos..];
    let payload_length = payload.len();
    let payload_hex = payload.iter().map(|b| format!("{b:02x}")).collect();

    // EtherType >= 0x0600 (1536) → Ethernet II; <= 0x05dc (1500) → 802.3 length.
    let (ethertype, ethertype_name_v, length, frame_format) = if type_field >= 0x0600 {
        (
            Some(format!("0x{type_field:04x}")),
            ethertype_name(type_field).map(|s| s.to_string()),
            None,
            "Ethernet II".to_string(),
        )
    } else {
        (None, None, Some(type_field), "IEEE 802.3".to_string())
    };

    Ok(Frame {
        destination_mac,
        source_mac,
        broadcast,
        multicast,
        locally_administered,
        vlan_tags,
        ethertype,
        ethertype_name: ethertype_name_v,
        length,
        frame_format,
        payload_length,
        payload_hex,
    })
}

/// Parse and return as pretty JSON (chat / programmatic surface).
pub fn run(input: &str) -> Result<String, String> {
    let frame = parse(input)?;
    serde_json::to_string_pretty(&frame).map_err(|e| e.to_string())
}

/// Human-readable rendering (used by the page).
pub fn render(input: &str) -> Result<String, String> {
    let f = parse(input)?;
    let mut out = String::new();
    out.push_str(&format!("Frame format:     {}\n", f.frame_format));
    out.push_str(&format!("Destination MAC:  {}", f.destination_mac));
    let dflag = if f.broadcast {
        "broadcast"
    } else if f.multicast {
        "multicast"
    } else {
        "unicast"
    };
    out.push_str(&format!("  ({dflag})\n"));
    out.push_str(&format!("Source MAC:       {}", f.source_mac));
    out.push_str(&format!(
        "  ({})\n",
        if f.locally_administered {
            "locally administered"
        } else {
            "globally unique (OUI)"
        }
    ));
    for (i, v) in f.vlan_tags.iter().enumerate() {
        out.push_str(&format!(
            "VLAN tag #{}:      {} {}  VID={}  PCP={}  DEI={}\n",
            i + 1,
            v.tpid,
            v.tpid_name,
            v.vid,
            v.pcp,
            if v.dei { 1 } else { 0 }
        ));
    }
    if let Some(et) = &f.ethertype {
        out.push_str(&format!("EtherType:        {et}"));
        if let Some(name) = &f.ethertype_name {
            out.push_str(&format!("  ({name})"));
        }
        out.push('\n');
    }
    if let Some(len) = f.length {
        out.push_str(&format!("Length (802.3):   {len} bytes\n"));
    }
    out.push_str(&format!("Payload:          {} bytes\n", f.payload_length));
    if !f.payload_hex.is_empty() {
        out.push_str(&format!("Payload (hex):    {}\n", f.payload_hex));
    }
    Ok(out.trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    // dst ff:ff:ff:ff:ff:ff (broadcast), src 00:11:22:33:44:55, type 0x0806 (ARP)
    const ARP: &str = "ffffffffffff0011223344550806000108000604000100112233445500000000";

    // Ethernet II IPv4: dst 01:00:5e:00:00:fb (multicast), src 00:1a:2b:3c:4d:5e, 0x0800
    const IPV4: &str = "01005e0000fb001a2b3c4d5e08004500001c";

    // 802.1Q tagged: dst aabbccddeeff, src 112233445566, TPID 8100 TCI 200a (PCP=1 VID=10), 0x0800
    const VLAN: &str = "aabbccddeeff112233445566 8100 200a 0800 deadbeef";

    // Q-in-Q: S-Tag 88a8 (VID 100) then C-Tag 8100 (VID 10) then 0x0800
    const QINQ: &str = "aabbccddeeff112233445566 88a8 0064 8100 000a 0800 cafe";

    // 802.3: length field 0x002e (46) instead of an EtherType
    const DOT3: &str = "ffffffffffff001122334455002eaaaa0300000088b5";

    #[test]
    fn parses_arp_broadcast() {
        let f = parse(ARP).unwrap();
        assert_eq!(f.destination_mac, "ff:ff:ff:ff:ff:ff");
        assert_eq!(f.source_mac, "00:11:22:33:44:55");
        assert!(f.broadcast);
        assert!(f.multicast); // broadcast is also multicast (I/G bit set)
        assert_eq!(f.ethertype.as_deref(), Some("0x0806"));
        assert_eq!(f.ethertype_name.as_deref(), Some("ARP"));
        assert_eq!(f.frame_format, "Ethernet II");
        assert!(f.length.is_none());
        assert!(f.vlan_tags.is_empty());
    }

    #[test]
    fn parses_ipv4_multicast() {
        let f = parse(IPV4).unwrap();
        assert!(!f.broadcast);
        assert!(f.multicast);
        assert_eq!(f.ethertype_name.as_deref(), Some("IPv4"));
        assert_eq!(f.payload_length, 4); // 4500001c
    }

    #[test]
    fn parses_single_vlan() {
        let f = parse(VLAN).unwrap();
        assert_eq!(f.vlan_tags.len(), 1);
        let v = &f.vlan_tags[0];
        assert_eq!(v.tpid, "0x8100");
        assert_eq!(v.vid, 10);
        assert_eq!(v.pcp, 1);
        assert!(!v.dei);
        assert_eq!(f.ethertype_name.as_deref(), Some("IPv4"));
        assert_eq!(f.payload_hex, "deadbeef");
    }

    #[test]
    fn parses_qinq() {
        let f = parse(QINQ).unwrap();
        assert_eq!(f.vlan_tags.len(), 2);
        assert_eq!(f.vlan_tags[0].tpid, "0x88a8");
        assert_eq!(f.vlan_tags[0].vid, 100);
        assert_eq!(f.vlan_tags[1].tpid, "0x8100");
        assert_eq!(f.vlan_tags[1].vid, 10);
        assert_eq!(f.ethertype_name.as_deref(), Some("IPv4"));
    }

    #[test]
    fn parses_802_3_length() {
        let f = parse(DOT3).unwrap();
        assert_eq!(f.frame_format, "IEEE 802.3");
        assert_eq!(f.length, Some(46));
        assert!(f.ethertype.is_none());
    }

    #[test]
    fn locally_administered_source() {
        // src 02:... has the U/L bit set
        let f = parse("ffffffffffff0211223344550800").unwrap();
        assert!(f.locally_administered);
    }

    #[test]
    fn accepts_separators_and_prefix() {
        let f = parse("0x ff:ff:ff:ff:ff:ff 00-11-22-33-44-55 08 06").unwrap();
        assert_eq!(f.ethertype.as_deref(), Some("0x0806"));
    }

    #[test]
    fn run_emits_json() {
        let j = run(ARP).unwrap();
        let v: serde_json::Value = serde_json::from_str(&j).unwrap();
        assert_eq!(v["ethertype_name"], "ARP");
        assert_eq!(v["broadcast"], true);
    }

    #[test]
    fn render_includes_fields() {
        let r = render(VLAN).unwrap();
        assert!(r.contains("VLAN tag #1"));
        assert!(r.contains("VID=10"));
        assert!(r.contains("IPv4"));
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
        assert!(parse("ffffffffffff0011223344").is_err());
    }

    #[test]
    fn errors_on_bad_hex() {
        assert!(parse("zzffffffffff0011223344550800").is_err());
    }
}
