//! mac-vendor-lookup core — resolve the manufacturer (vendor) of a MAC / EUI
//! address from its OUI (Organizationally Unique Identifier) prefix, using a
//! bundled IEEE registry. Fully offline, no I/O, no wafer/wasm-bindgen deps.
//! Shared by the chat skill block and the web page.

use std::sync::OnceLock;

/// The bundled IEEE OUI registry, one entry per line as `PREFIX\tORG`, where
/// `PREFIX` is the 6-hex-digit (24-bit) assignment, uppercase, sorted ascending.
/// Source: IEEE MA-L registry (standards-oui.ieee.org/oui/oui.csv).
const OUI_TSV: &str = include_str!("oui.tsv");

/// Parsed registry as a sorted `(prefix, org)` slice, built once on first use so
/// we can binary-search it. The TSV is pre-sorted by prefix at build time, so we
/// just borrow the &str slices — no allocation per entry.
fn registry() -> &'static [(&'static str, &'static str)] {
    static REG: OnceLock<Vec<(&'static str, &'static str)>> = OnceLock::new();
    REG.get_or_init(|| {
        OUI_TSV
            .lines()
            .filter_map(|line| {
                let (prefix, org) = line.split_once('\t')?;
                Some((prefix, org))
            })
            .collect()
    })
}

/// The result of a lookup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lookup {
    /// The input MAC normalized to canonical colon-separated uppercase form
    /// (e.g. `28:6F:B9:00:00:00`). Only the supplied octets are shown.
    pub mac: String,
    /// The 24-bit OUI prefix, uppercase, colon-separated (e.g. `28:6F:B9`).
    pub oui: String,
    /// The registered manufacturer / organization name, or `None` if the OUI is
    /// not in the IEEE registry (e.g. a locally-administered or unassigned MAC).
    pub vendor: Option<String>,
    /// True if the address is locally administered (bit 1 of the first octet
    /// set) — these are not IEEE-assigned, so `vendor` will usually be `None`.
    pub locally_administered: bool,
    /// True if the address is a multicast/group address (bit 0 of the first
    /// octet set); false for a normal unicast address.
    pub multicast: bool,
}

/// Number of IEEE OUI assignments in the bundled registry.
pub fn registry_len() -> usize {
    registry().len()
}

/// Parse and validate `input`, then resolve its vendor from the bundled OUI
/// registry.
///
/// Accepts the common MAC/EUI textual forms (case-insensitive), with or without
/// separators:
/// - colon:  `28:6f:b9:00:1a:2b`
/// - hyphen: `28-6F-B9-00-1A-2B`
/// - dot:    `286f.b900.1a2b` (Cisco)
/// - bare:   `286fb9001a2b`
///
/// At least the first 3 octets (6 hex digits — the OUI) are required; up to 8
/// octets (EUI-64) are accepted. Returns `Err` if the input has no hex digits,
/// an odd number of hex digits, fewer than 6 / more than 16 hex digits, or any
/// non-hex character.
pub fn lookup(input: &str) -> Result<Lookup, String> {
    let hex: String = input
        .chars()
        .filter(|c| !matches!(c, ':' | '-' | '.' | ' ' | '\t'))
        .collect();

    if hex.is_empty() {
        return Err("no MAC address found in input".to_string());
    }
    if let Some(bad) = hex.chars().find(|c| !c.is_ascii_hexdigit()) {
        return Err(format!(
            "invalid character {bad:?} in MAC address: expected hex digits and : - . separators"
        ));
    }
    if hex.len() % 2 != 0 {
        return Err(format!(
            "MAC address has an odd number of hex digits ({}); each octet needs 2",
            hex.len()
        ));
    }
    if hex.len() < 6 {
        return Err(format!(
            "MAC address too short: need at least 3 octets (6 hex digits) for the OUI, got {}",
            hex.len() / 2
        ));
    }
    if hex.len() > 16 {
        return Err(format!(
            "MAC address too long: at most 8 octets (16 hex digits), got {}",
            hex.len() / 2
        ));
    }

    let upper = hex.to_ascii_uppercase();
    let prefix = &upper[..6];

    // First octet carries the I/G (multicast) and U/L (locally administered) bits.
    let first_octet = u8::from_str_radix(&upper[..2], 16).expect("validated hex");
    let multicast = first_octet & 0x01 != 0;
    let locally_administered = first_octet & 0x02 != 0;

    let vendor = registry()
        .binary_search_by(|(p, _)| (*p).cmp(prefix))
        .ok()
        .map(|i| registry()[i].1.to_string());

    let octets: Vec<&str> = (0..upper.len() / 2)
        .map(|i| &upper[i * 2..i * 2 + 2])
        .collect();

    Ok(Lookup {
        mac: octets.join(":"),
        oui: octets[..3].join(":"),
        vendor,
        locally_administered,
        multicast,
    })
}

/// Render a human-readable report for `input`, which may contain **several**
/// MAC addresses (one per line) — handy for a batch lookup of an ARP table or a
/// device list. A single address yields the detailed multi-line block from
/// [`report_one`]; multiple addresses yield one compact `MAC — Vendor` line each
/// (blank lines are skipped). The standalone page and chat both call this.
pub fn report(input: &str) -> String {
    let lines: Vec<&str> = input.lines().filter(|l| !l.trim().is_empty()).collect();
    match lines.as_slice() {
        // Nothing but blanks → fall through to the single-line error path.
        [] => report_one(input),
        [only] => report_one(only),
        many => many
            .iter()
            .map(|line| match lookup(line) {
                Ok(r) => format!(
                    "{} — {}",
                    r.mac,
                    r.vendor.as_deref().unwrap_or("(not in IEEE registry)")
                ),
                Err(e) => format!("{} — Error: {e}", line.trim()),
            })
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

/// Resolve a single `input` and render the detailed multi-line report. A
/// malformed input is reported as an `Error:` line rather than failing — so the
/// page and chat always show something useful. The structured fields are still
/// available via [`lookup`] for programmatic callers.
pub fn report_one(input: &str) -> String {
    match lookup(input) {
        Ok(r) => {
            let mut lines = vec![
                format!("MAC:    {}", r.mac),
                format!("OUI:    {}", r.oui),
                format!(
                    "Vendor: {}",
                    r.vendor.as_deref().unwrap_or("(not in IEEE registry)")
                ),
            ];
            let mut notes = Vec::new();
            if r.locally_administered {
                notes.push("locally administered (not IEEE-assigned)");
            } else {
                notes.push("globally unique (IEEE-assigned)");
            }
            notes.push(if r.multicast {
                "multicast / group address"
            } else {
                "unicast address"
            });
            lines.push(format!("Type:   {}", notes.join(", ")));
            lines.join("\n")
        }
        Err(e) => format!("Error: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn report_renders_known_vendor() {
        let out = report("28:6F:B9:00:1A:2B");
        assert!(out.contains("Vendor:"), "got {out}");
        assert!(out.contains("Nokia"), "got {out}");
        assert!(out.contains("OUI:    28:6F:B9"), "got {out}");
        assert!(out.contains("unicast"), "got {out}");
    }

    #[test]
    fn report_handles_unknown_and_error() {
        assert!(report("FF:FF:FF:FF:FF:FF").contains("not in IEEE registry"));
        assert!(report("xyz").starts_with("Error:"));
    }

    #[test]
    fn report_batch_one_line_per_address() {
        let out = report("28:6F:B9:00:00:00\n00:00:0C:11:22:33\nzz");
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 3, "got {out}");
        assert!(lines[0].contains("Nokia"), "got {out}");
        assert!(lines[1].contains("Cisco"), "got {out}");
        assert!(lines[2].contains("Error:"), "got {out}");
        // Blank lines are ignored, so a single real address stays detailed.
        assert!(report("\n28:6F:B9:00:00:00\n").contains("Vendor: Nokia"));
    }

    #[test]
    fn resolves_known_vendor_colon_form() {
        let r = lookup("28:6F:B9:12:34:56").unwrap();
        assert_eq!(r.oui, "28:6F:B9");
        assert_eq!(r.mac, "28:6F:B9:12:34:56");
        // 28:6F:B9 is a Nokia Shanghai Bell assignment.
        assert!(
            r.vendor.as_deref().unwrap().contains("Nokia"),
            "got {:?}",
            r.vendor
        );
        assert!(!r.multicast);
        assert!(!r.locally_administered);
    }

    #[test]
    fn accepts_hyphen_dot_and_bare_forms() {
        let a = lookup("28-6F-B9-00-1A-2B").unwrap();
        let b = lookup("286f.b900.1a2b").unwrap();
        let c = lookup("286fb9001a2b").unwrap();
        assert_eq!(a.oui, "28:6F:B9");
        assert_eq!(b.oui, "28:6F:B9");
        assert_eq!(c.oui, "28:6F:B9");
        assert_eq!(a.mac, b.mac);
        assert_eq!(b.mac, c.mac);
    }

    #[test]
    fn oui_only_input_is_accepted() {
        let r = lookup("00:00:0C").unwrap();
        assert_eq!(r.oui, "00:00:0C");
        assert_eq!(r.mac, "00:00:0C");
        // 00:00:0C is Cisco.
        assert!(
            r.vendor
                .as_deref()
                .unwrap()
                .to_ascii_uppercase()
                .contains("CISCO"),
            "got {:?}",
            r.vendor
        );
    }

    #[test]
    fn detects_multicast_and_locally_administered_bits() {
        // 02 = U/L bit set (locally administered), I/G clear (unicast).
        let r = lookup("02:00:00:00:00:01").unwrap();
        assert!(r.locally_administered);
        assert!(!r.multicast);
        // 01 = I/G bit set (multicast).
        let m = lookup("01:00:5E:00:00:01").unwrap();
        assert!(m.multicast);
        assert!(!m.locally_administered);
    }

    #[test]
    fn unknown_oui_returns_no_vendor() {
        // FF:FF:FF is the broadcast OUI — not an IEEE assignment.
        let r = lookup("FF:FF:FF:FF:FF:FF").unwrap();
        assert_eq!(r.vendor, None);
    }

    #[test]
    fn rejects_non_hex() {
        let e = lookup("28:6F:B9:GG:00:00").unwrap_err();
        assert!(e.contains("invalid character"), "got {e}");
    }

    #[test]
    fn rejects_too_short() {
        let e = lookup("28:6F").unwrap_err();
        assert!(e.contains("too short"), "got {e}");
    }

    #[test]
    fn rejects_odd_hex_count() {
        let e = lookup("28:6F:B").unwrap_err();
        assert!(e.contains("odd number"), "got {e}");
    }

    #[test]
    fn rejects_empty() {
        let e = lookup("::").unwrap_err();
        assert!(e.contains("no MAC"), "got {e}");
    }

    #[test]
    fn registry_is_non_trivial() {
        assert!(registry_len() > 30_000, "registry only {}", registry_len());
    }
}
