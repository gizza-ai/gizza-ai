//! wireguard-config-builder core — build and validate a complete WireGuard
//! `[Interface]` + `[Peer]` config from entered values. Pure compute (`std::net`
//! + base64 only), shared by the chat skill block and the web page.
//!
//! WireGuard keys are Curve25519 keys: 32 raw bytes, base64-encoded to a
//! 44-character string ending in `=`. This tool does NOT generate keys (that
//! needs a CSPRNG and is non-deterministic) — it validates the keys you paste
//! and assembles them, with every Address / AllowedIPs / DNS / Endpoint field
//! parsed and range-checked, into a ready-to-save `wg0.conf`.
//!
//! `format = "conf"` (default) renders the config file text; `format = "json"`
//! renders a machine-readable object with the parsed interface + peer fields.

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use std::net::{IpAddr, Ipv6Addr};

/// Typed, already-string-split inputs. Optional numeric fields are `None` when
/// the user left them blank; optional string fields are empty strings.
pub struct WgInput {
    pub private_key: String,
    pub address: String,
    pub listen_port: Option<u32>,
    pub dns: String,
    pub mtu: Option<u32>,
    pub peer_public_key: String,
    pub preshared_key: String,
    pub allowed_ips: String,
    pub endpoint: String,
    pub persistent_keepalive: Option<u32>,
}

/// Build + validate the config, rendering it in `format` ("conf" | "json").
pub fn build(inp: &WgInput, format: &str) -> Result<String, String> {
    let fmt = format.trim();
    let fmt = if fmt.is_empty() { "conf" } else { fmt };
    let fmt = fmt.to_ascii_lowercase();
    if fmt != "conf" && fmt != "json" {
        return Err(format!("invalid format {format:?}: expected 'conf' or 'json'"));
    }

    // ---- [Interface] ----
    let private_key = validate_key(&inp.private_key, "PrivateKey (interface private key)")?;
    let address = validate_ip_list(&inp.address, "Address", true)?;
    if address.is_empty() {
        return Err("Address is empty: give at least one interface address, e.g. 10.0.0.2/32".into());
    }
    let listen_port = match inp.listen_port {
        Some(p) => Some(validate_port(p, "ListenPort")?),
        None => None,
    };
    let dns = validate_ip_list(&inp.dns, "DNS", false)?;
    let mtu = match inp.mtu {
        Some(m) => {
            if !(576..=9000).contains(&m) {
                return Err(format!(
                    "MTU {m} out of range: expected 576-9000 (WireGuard's typical value is 1420)"
                ));
            }
            Some(m)
        }
        None => None,
    };

    // ---- [Peer] ----
    let peer_public_key = validate_key(&inp.peer_public_key, "PublicKey (peer public key)")?;
    let preshared_key = if inp.preshared_key.trim().is_empty() {
        None
    } else {
        Some(validate_key(&inp.preshared_key, "PresharedKey")?)
    };
    let allowed_ips = validate_ip_list(&inp.allowed_ips, "AllowedIPs", true)?;
    if allowed_ips.is_empty() {
        return Err(
            "AllowedIPs is empty: give at least one CIDR, e.g. 0.0.0.0/0, ::/0 for a full tunnel"
                .into(),
        );
    }
    let endpoint = if inp.endpoint.trim().is_empty() {
        None
    } else {
        Some(validate_endpoint(inp.endpoint.trim())?)
    };
    let persistent_keepalive = match inp.persistent_keepalive {
        Some(k) => {
            if k > 65535 {
                return Err(format!(
                    "PersistentKeepalive {k} out of range: expected 0-65535 seconds (25 is typical for NAT traversal)"
                ));
            }
            Some(k)
        }
        None => None,
    };

    let cfg = WgConfig {
        private_key,
        address,
        listen_port,
        dns,
        mtu,
        peer_public_key,
        preshared_key,
        allowed_ips,
        endpoint,
        persistent_keepalive,
    };

    Ok(match fmt.as_str() {
        "json" => cfg.to_json(),
        _ => cfg.to_conf(),
    })
}

/// Validated, ready-to-render config.
struct WgConfig {
    private_key: String,
    address: Vec<String>,
    listen_port: Option<u32>,
    dns: Vec<String>,
    mtu: Option<u32>,
    peer_public_key: String,
    preshared_key: Option<String>,
    allowed_ips: Vec<String>,
    endpoint: Option<String>,
    persistent_keepalive: Option<u32>,
}

impl WgConfig {
    fn to_conf(&self) -> String {
        let mut s = String::new();
        s.push_str("[Interface]\n");
        s.push_str(&format!("PrivateKey = {}\n", self.private_key));
        s.push_str(&format!("Address = {}\n", self.address.join(", ")));
        if let Some(p) = self.listen_port {
            s.push_str(&format!("ListenPort = {p}\n"));
        }
        if !self.dns.is_empty() {
            s.push_str(&format!("DNS = {}\n", self.dns.join(", ")));
        }
        if let Some(m) = self.mtu {
            s.push_str(&format!("MTU = {m}\n"));
        }
        s.push_str("\n[Peer]\n");
        s.push_str(&format!("PublicKey = {}\n", self.peer_public_key));
        if let Some(k) = &self.preshared_key {
            s.push_str(&format!("PresharedKey = {k}\n"));
        }
        s.push_str(&format!("AllowedIPs = {}\n", self.allowed_ips.join(", ")));
        if let Some(e) = &self.endpoint {
            s.push_str(&format!("Endpoint = {e}\n"));
        }
        if let Some(k) = self.persistent_keepalive {
            s.push_str(&format!("PersistentKeepalive = {k}\n"));
        }
        // Trim the trailing newline so the file ends cleanly.
        s.trim_end().to_string()
    }

    fn to_json(&self) -> String {
        let q = |v: &str| v.replace('\\', "\\\\").replace('"', "\\\"");
        let arr = |xs: &[String]| {
            let items: Vec<String> = xs.iter().map(|x| format!("\"{}\"", q(x))).collect();
            format!("[{}]", items.join(", "))
        };
        let num = |o: Option<u32>| match o {
            Some(n) => n.to_string(),
            None => "null".to_string(),
        };
        let ostr = |o: &Option<String>| match o {
            Some(v) => format!("\"{}\"", q(v)),
            None => "null".to_string(),
        };
        format!(
            "{{\n  \"interface\": {{\n    \"private_key\": \"{pk}\",\n    \"address\": {addr},\n    \
             \"listen_port\": {port},\n    \"dns\": {dns},\n    \"mtu\": {mtu}\n  }},\n  \
             \"peer\": {{\n    \"public_key\": \"{ppk}\",\n    \"preshared_key\": {psk},\n    \
             \"allowed_ips\": {aips},\n    \"endpoint\": {ep},\n    \
             \"persistent_keepalive\": {ka}\n  }}\n}}",
            pk = q(&self.private_key),
            addr = arr(&self.address),
            port = num(self.listen_port),
            dns = arr(&self.dns),
            mtu = num(self.mtu),
            ppk = q(&self.peer_public_key),
            psk = ostr(&self.preshared_key),
            aips = arr(&self.allowed_ips),
            ep = ostr(&self.endpoint),
            ka = num(self.persistent_keepalive),
        )
    }
}

/// A WireGuard key is a Curve25519 key: base64 of exactly 32 raw bytes.
/// Returns the trimmed, re-normalized key on success.
fn validate_key(raw: &str, label: &str) -> Result<String, String> {
    let key = raw.trim();
    if key.is_empty() {
        return Err(format!("{label} is empty: paste a base64 WireGuard key (44 characters ending in '=')"));
    }
    if key.contains(char::is_whitespace) {
        return Err(format!("{label} contains whitespace: a WireGuard key is a single 44-character base64 string"));
    }
    let bytes = B64
        .decode(key)
        .map_err(|_| format!("{label} is not valid base64: expected a 44-character key like 'gI6EdU...4WRUE='"))?;
    if bytes.len() != 32 {
        return Err(format!(
            "{label} decodes to {} bytes, not 32: WireGuard keys are 32-byte Curve25519 keys (44 base64 chars)",
            bytes.len()
        ));
    }
    Ok(key.to_string())
}

/// Validate a comma-separated list of IPs/CIDRs. When `allow_cidr` is false
/// (DNS), a `/prefix` is rejected; when true (Address/AllowedIPs) both a bare
/// address and `IP/prefix` are accepted. Entries are echoed unchanged; an
/// all-blank input yields an empty Vec.
fn validate_ip_list(raw: &str, label: &str, allow_cidr: bool) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for part in raw.split(',') {
        let entry = part.trim();
        if entry.is_empty() {
            continue;
        }
        validate_ip_entry(entry, label, allow_cidr)?;
        out.push(entry.to_string());
    }
    Ok(out)
}

/// Validate a single entry: either a bare IP or (if `allow_cidr`) `IP/prefix`.
fn validate_ip_entry(entry: &str, label: &str, allow_cidr: bool) -> Result<(), String> {
    if let Some((addr, prefix)) = entry.split_once('/') {
        if !allow_cidr {
            return Err(format!(
                "{label} entry {entry:?} should be a plain IP address, not a CIDR"
            ));
        }
        let ip: IpAddr = addr.trim().parse().map_err(|_| {
            format!("{label} entry {entry:?}: {:?} is not a valid IP address", addr.trim())
        })?;
        let prefix: u32 = prefix.trim().parse().map_err(|_| {
            format!("{label} entry {entry:?}: prefix {:?} must be a number", prefix.trim())
        })?;
        let max = if ip.is_ipv4() { 32 } else { 128 };
        if prefix > max {
            return Err(format!(
                "{label} entry {entry:?}: prefix /{prefix} out of range (0-{max} for {})",
                if ip.is_ipv4() { "IPv4" } else { "IPv6" }
            ));
        }
        Ok(())
    } else {
        entry
            .parse::<IpAddr>()
            .map(|_| ())
            .map_err(|_| format!("{label} entry {entry:?} is not a valid IP address"))
    }
}

/// Validate a peer Endpoint: `host:port`, or `[v6]:port` for IPv6 literals.
/// Host may be a domain, IPv4, or bracketed IPv6. Returns the trimmed endpoint.
fn validate_endpoint(ep: &str) -> Result<String, String> {
    let (host, port_str) = if let Some(rest) = ep.strip_prefix('[') {
        // [IPv6]:port
        let close = rest.find(']').ok_or_else(|| {
            format!("Endpoint {ep:?}: bracketed IPv6 host is missing its closing ']'")
        })?;
        let host = &rest[..close];
        host.parse::<Ipv6Addr>()
            .map_err(|_| format!("Endpoint {ep:?}: {host:?} inside brackets is not a valid IPv6 address"))?;
        let after = &rest[close + 1..];
        let port = after.strip_prefix(':').ok_or_else(|| {
            format!("Endpoint {ep:?}: expected ']:port', e.g. [2001:db8::1]:51820")
        })?;
        (host.to_string(), port.to_string())
    } else {
        let (h, p) = ep.rsplit_once(':').ok_or_else(|| {
            format!("Endpoint {ep:?} is missing a port: expected host:port, e.g. vpn.example.com:51820")
        })?;
        if h.is_empty() {
            return Err(format!("Endpoint {ep:?} has an empty host"));
        }
        if h.contains(':') {
            return Err(format!(
                "Endpoint {ep:?}: an IPv6 host must be bracketed, e.g. [{h}]:{p}"
            ));
        }
        (h.to_string(), p.to_string())
    };
    let port: u32 = port_str
        .trim()
        .parse()
        .map_err(|_| format!("Endpoint {ep:?}: port {port_str:?} must be a number"))?;
    validate_port(port, "Endpoint port")?;
    let _ = host;
    Ok(ep.to_string())
}

fn validate_port(p: u32, label: &str) -> Result<u32, String> {
    if !(1..=65535).contains(&p) {
        return Err(format!("{label} {p} out of range: expected 1-65535"));
    }
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    // Two valid 32-byte Curve25519 keys (base64 of 32 bytes). Not real secrets.
    const PRIV: &str = "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA=";
    const PUB: &str = "MTIzNDU2Nzg5MDEyMzQ1Njc4OTAxMjM0NTY3ODkwMTI=";
    const PSK: &str = "YWJjZGVmZ2hpamtsbW5vcHFyc3R1dnd4eXoxMjM0NTY=";

    fn base_input() -> WgInput {
        WgInput {
            private_key: PRIV.into(),
            address: "10.0.0.2/32".into(),
            listen_port: None,
            dns: String::new(),
            mtu: None,
            peer_public_key: PUB.into(),
            preshared_key: String::new(),
            allowed_ips: "0.0.0.0/0, ::/0".into(),
            endpoint: String::new(),
            persistent_keepalive: None,
        }
    }

    #[test]
    fn happy_minimal() {
        let out = build(&base_input(), "conf").unwrap();
        assert!(out.contains("[Interface]"));
        assert!(out.contains(&format!("PrivateKey = {PRIV}")));
        assert!(out.contains("Address = 10.0.0.2/32"));
        assert!(out.contains("[Peer]"));
        assert!(out.contains(&format!("PublicKey = {PUB}")));
        assert!(out.contains("AllowedIPs = 0.0.0.0/0, ::/0"));
        // Optional fields omitted when unset.
        assert!(!out.contains("ListenPort"));
        assert!(!out.contains("DNS"));
        assert!(!out.contains("MTU"));
        assert!(!out.contains("Endpoint"));
        assert!(!out.contains("PresharedKey"));
        assert!(!out.contains("PersistentKeepalive"));
    }

    #[test]
    fn happy_full() {
        let mut inp = base_input();
        inp.address = "10.0.0.2/32, fd00::2/128".into();
        inp.listen_port = Some(51820);
        inp.dns = "1.1.1.1, 8.8.8.8".into();
        inp.mtu = Some(1420);
        inp.preshared_key = PSK.into();
        inp.endpoint = "vpn.example.com:51820".into();
        inp.persistent_keepalive = Some(25);
        let out = build(&inp, "conf").unwrap();
        assert!(out.contains("Address = 10.0.0.2/32, fd00::2/128"));
        assert!(out.contains("ListenPort = 51820"));
        assert!(out.contains("DNS = 1.1.1.1, 8.8.8.8"));
        assert!(out.contains("MTU = 1420"));
        assert!(out.contains(&format!("PresharedKey = {PSK}")));
        assert!(out.contains("Endpoint = vpn.example.com:51820"));
        assert!(out.contains("PersistentKeepalive = 25"));
        // Interface section comes before peer section.
        assert!(out.find("[Interface]").unwrap() < out.find("[Peer]").unwrap());
    }

    #[test]
    fn ipv6_endpoint_bracketed() {
        let mut inp = base_input();
        inp.endpoint = "[2001:db8::1]:51820".into();
        let out = build(&inp, "conf").unwrap();
        assert!(out.contains("Endpoint = [2001:db8::1]:51820"));
    }

    #[test]
    fn json_output() {
        let mut inp = base_input();
        inp.listen_port = Some(51820);
        inp.endpoint = "vpn.example.com:51820".into();
        let out = build(&inp, "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["interface"]["private_key"], PRIV);
        assert_eq!(v["interface"]["listen_port"], 51820);
        assert_eq!(v["interface"]["mtu"], serde_json::Value::Null);
        assert_eq!(v["peer"]["public_key"], PUB);
        assert_eq!(v["peer"]["allowed_ips"][0], "0.0.0.0/0");
        assert_eq!(v["peer"]["allowed_ips"][1], "::/0");
        assert_eq!(v["peer"]["endpoint"], "vpn.example.com:51820");
        assert_eq!(v["peer"]["preshared_key"], serde_json::Value::Null);
    }

    #[test]
    fn err_bad_key_length() {
        let mut inp = base_input();
        inp.private_key = "dG9vc2hvcnQ=".into(); // "tooshort" -> 8 bytes
        let e = build(&inp, "conf").unwrap_err();
        assert!(e.contains("32"), "got: {e}");
    }

    #[test]
    fn err_not_base64() {
        let mut inp = base_input();
        inp.peer_public_key = "this is not base64!!!".into();
        assert!(build(&inp, "conf").is_err());
    }

    #[test]
    fn err_bad_cidr() {
        let mut inp = base_input();
        inp.allowed_ips = "10.0.0.0/40".into(); // prefix too big
        assert!(build(&inp, "conf").unwrap_err().contains("prefix"));
    }

    #[test]
    fn err_bad_address() {
        let mut inp = base_input();
        inp.address = "999.1.1.1/24".into();
        assert!(build(&inp, "conf").is_err());
    }

    #[test]
    fn err_empty_allowed_ips() {
        let mut inp = base_input();
        inp.allowed_ips = "  ,  ".into();
        assert!(build(&inp, "conf").unwrap_err().contains("AllowedIPs"));
    }

    #[test]
    fn err_port_range() {
        let mut inp = base_input();
        inp.listen_port = Some(70000);
        assert!(build(&inp, "conf").unwrap_err().contains("ListenPort"));
    }

    #[test]
    fn err_mtu_range() {
        let mut inp = base_input();
        inp.mtu = Some(100);
        assert!(build(&inp, "conf").unwrap_err().contains("MTU"));
    }

    #[test]
    fn err_endpoint_no_port() {
        let mut inp = base_input();
        inp.endpoint = "vpn.example.com".into();
        assert!(build(&inp, "conf").unwrap_err().contains("port"));
    }

    #[test]
    fn err_unbracketed_ipv6_endpoint() {
        let mut inp = base_input();
        inp.endpoint = "2001:db8::1:51820".into();
        assert!(build(&inp, "conf").unwrap_err().contains("bracket"));
    }

    #[test]
    fn err_bad_format() {
        assert!(build(&base_input(), "xml").is_err());
    }

    #[test]
    fn dns_bare_ips_only() {
        let mut inp = base_input();
        inp.dns = "1.1.1.1/24".into(); // CIDR not allowed for DNS
        assert!(build(&inp, "conf").is_err());
    }
}
