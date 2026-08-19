//! wireguard-keygen core — generate WireGuard Curve25519 key pairs (and
//! preshared keys) with a ready-to-paste `wg0.conf` snippet. Pure compute
//! (`x25519-dalek` + `base64` + the OS/browser CSPRNG), shared by the chat skill
//! block and the web page.
//!
//! A WireGuard key is a 32-byte Curve25519 value rendered as 44 characters of
//! standard base64 ending in `=`. The private key is **clamped exactly the way
//! `wg genkey` clamps it** (`b[0] &= 248; b[31] &= 127; b[31] |= 64`) so the
//! output is byte-indistinguishable from the real tool and round-trips through
//! `wg pubkey`. The public key is the Curve25519 base-point multiplication of
//! that scalar; the optional preshared key is 32 raw CSPRNG bytes, like
//! `wg genpsk`.
//!
//! Generation and rendering are deliberately split: [`generate_pair`] is the
//! only non-deterministic function, so [`render`] can be tested against fixed
//! keys with exact expected output.

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use x25519_dalek::{PublicKey, StaticSecret};

/// Hard cap on `pairs` — a bulk run stays a copy-pasteable page of text.
pub const MAX_PAIRS: i64 = 25;

/// One generated WireGuard key pair, base64-encoded the way `wg` prints keys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyPair {
    /// 32-byte clamped Curve25519 private scalar, base64 (`wg genkey`).
    pub private_key: String,
    /// 32-byte Curve25519 public point, base64 (`wg pubkey`).
    pub public_key: String,
    /// 32-byte symmetric preshared key, base64 (`wg genpsk`), when requested.
    pub preshared_key: Option<String>,
}

/// The output shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    /// Keys plus the annotated config snippet, human-readable.
    Text,
    /// Machine-readable object with one entry per key pair.
    Json,
    /// The `wg0.conf` snippet(s) only.
    Conf,
}

impl Format {
    fn parse(s: &str) -> Result<Format, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "text" => Ok(Format::Text),
            "json" => Ok(Format::Json),
            "conf" => Ok(Format::Conf),
            other => Err(format!(
                "invalid format {other:?}: expected 'text', 'json' or 'conf'"
            )),
        }
    }
}

/// Validated render settings — everything except the (random) keys themselves.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Settings {
    /// How many key pairs to generate (1..=[`MAX_PAIRS`]).
    pub pairs: usize,
    /// Whether each pair also gets a preshared key.
    pub preshared_key: bool,
    /// Output shape.
    pub format: Format,
    /// Interface address for the snippet's `[Interface] Address` line.
    pub address: String,
    /// Server `host:port` for the snippet's `[Peer] Endpoint` line, or `None`
    /// when the user cleared the field (the line is then omitted).
    pub endpoint: Option<String>,
}

/// Validate the user-supplied settings. Runs BEFORE any key is generated, so a
/// bad input never burns entropy or half-prints a key.
pub fn settings(
    pairs: f64,
    preshared_key: bool,
    format: &str,
    address: &str,
    endpoint: &str,
) -> Result<Settings, String> {
    let format = Format::parse(format)?;

    if !pairs.is_finite() {
        return Err("pairs must be a whole number between 1 and 25".into());
    }
    let n = pairs.round() as i64;
    if !(1..=MAX_PAIRS).contains(&n) {
        return Err(format!(
            "pairs {n} out of range: expected 1-{MAX_PAIRS} key pairs"
        ));
    }

    let address = validate_address(address)?;
    let endpoint = validate_endpoint(endpoint)?;

    Ok(Settings {
        pairs: n as usize,
        preshared_key,
        format,
        address,
        endpoint,
    })
}

/// `Address = ` value: one or more `ip/prefix` entries, comma-separated.
fn validate_address(address: &str) -> Result<String, String> {
    let raw = address.trim();
    if raw.is_empty() {
        return Err(
            "address is empty: give the tunnel address for this device, e.g. 10.0.0.2/32".into(),
        );
    }
    let mut out: Vec<String> = Vec::new();
    for part in raw.split(',') {
        let entry = part.trim();
        if entry.is_empty() {
            continue;
        }
        let (ip_str, prefix_str) = entry.split_once('/').ok_or_else(|| {
            format!("address {entry:?} has no prefix length: write it as CIDR, e.g. 10.0.0.2/32")
        })?;
        let ip: std::net::IpAddr = ip_str
            .trim()
            .parse()
            .map_err(|_| format!("address {ip_str:?} is not a valid IP address"))?;
        let max = if ip.is_ipv4() { 32 } else { 128 };
        let prefix: u32 = prefix_str
            .trim()
            .parse()
            .map_err(|_| format!("prefix length {prefix_str:?} is not a number"))?;
        if prefix > max {
            return Err(format!(
                "prefix length /{prefix} is out of range for {ip}: expected 0-{max}"
            ));
        }
        out.push(format!("{ip}/{prefix}"));
    }
    if out.is_empty() {
        return Err(
            "address is empty: give the tunnel address for this device, e.g. 10.0.0.2/32".into(),
        );
    }
    Ok(out.join(", "))
}

/// `Endpoint = ` value: `host:port`, or empty to omit the line entirely.
fn validate_endpoint(endpoint: &str) -> Result<Option<String>, String> {
    let raw = endpoint.trim();
    if raw.is_empty() {
        return Ok(None);
    }
    // An IPv6 endpoint is written [::1]:51820 — split on the LAST colon so the
    // address's own colons stay with the host.
    let (host, port_str) = raw.rsplit_once(':').ok_or_else(|| {
        format!("endpoint {raw:?} has no port: write it as host:port, e.g. vpn.example.com:51820")
    })?;
    if host.trim().is_empty() {
        return Err(format!("endpoint {raw:?} has an empty host"));
    }
    let port: u32 = port_str
        .trim()
        .parse()
        .map_err(|_| format!("endpoint port {port_str:?} is not a number"))?;
    if !(1..=65535).contains(&port) {
        return Err(format!("endpoint port {port} out of range: expected 1-65535"));
    }
    Ok(Some(format!("{}:{port}", host.trim())))
}

/// Derive the Curve25519 public key for a raw 32-byte private scalar. X25519
/// clamps internally, so this matches `wg pubkey` for clamped and unclamped
/// input alike.
pub fn derive_public(private: [u8; 32]) -> [u8; 32] {
    PublicKey::from(&StaticSecret::from(private)).to_bytes()
}

/// Clamp a 32-byte scalar the way `wg genkey` does before printing it.
fn clamp(mut b: [u8; 32]) -> [u8; 32] {
    b[0] &= 248;
    b[31] &= 127;
    b[31] |= 64;
    b
}

/// Draw 32 bytes from the platform CSPRNG (OS on native/WASI,
/// `crypto.getRandomValues` in the browser).
fn random_32() -> Result<[u8; 32], String> {
    let mut buf = [0u8; 32];
    getrandom::getrandom(&mut buf)
        .map_err(|e| format!("secure random number generator unavailable: {e}"))?;
    Ok(buf)
}

/// Generate one fresh key pair. The ONLY non-deterministic function here.
pub fn generate_pair(with_preshared_key: bool) -> Result<KeyPair, String> {
    let private = clamp(random_32()?);
    let public = derive_public(private);
    let preshared_key = if with_preshared_key {
        Some(B64.encode(random_32()?))
    } else {
        None
    };
    Ok(KeyPair {
        private_key: B64.encode(private),
        public_key: B64.encode(public),
        preshared_key,
    })
}

/// The `wg0.conf` snippet for one key pair: the `[Interface]` block to keep on
/// this device, the `[Peer]` block pointing at the remote side, and the peer
/// fragment to hand the other side so it can reach you back.
fn config_snippet(s: &Settings, kp: &KeyPair) -> String {
    let mut out = String::new();
    out.push_str("[Interface]\n");
    out.push_str(&format!("PrivateKey = {}\n", kp.private_key));
    out.push_str(&format!("Address = {}\n", s.address));
    out.push_str("# ListenPort = 51820   # servers only; clients pick a random port\n");
    out.push_str("\n[Peer]\n");
    out.push_str("# Paste the REMOTE side's public key here.\n");
    out.push_str("PublicKey = <remote peer public key>\n");
    if let Some(psk) = &kp.preshared_key {
        out.push_str(&format!("PresharedKey = {psk}\n"));
    }
    out.push_str("AllowedIPs = 0.0.0.0/0, ::/0\n");
    if let Some(endpoint) = &s.endpoint {
        out.push_str(&format!("Endpoint = {endpoint}\n"));
    }
    out.push_str("PersistentKeepalive = 25\n");
    out.push_str("\n# Hand the other side this block so it can reach you:\n");
    out.push_str("# [Peer]\n");
    out.push_str(&format!("# PublicKey = {}\n", kp.public_key));
    if let Some(psk) = &kp.preshared_key {
        out.push_str(&format!("# PresharedKey = {psk}\n"));
    }
    out.push_str(&format!("# AllowedIPs = {}\n", s.address));
    out
}

/// Render already-generated keys in the requested format. Deterministic.
pub fn render(s: &Settings, keys: &[KeyPair]) -> String {
    match s.format {
        Format::Json => {
            let mut items: Vec<String> = Vec::with_capacity(keys.len());
            for (i, kp) in keys.iter().enumerate() {
                let psk = match &kp.preshared_key {
                    Some(p) => format!("\"{}\"", json_escape(p)),
                    None => "null".to_string(),
                };
                items.push(format!(
                    "    {{\n      \"index\": {},\n      \"private_key\": \"{}\",\n      \"public_key\": \"{}\",\n      \"preshared_key\": {},\n      \"config\": \"{}\"\n    }}",
                    i + 1,
                    json_escape(&kp.private_key),
                    json_escape(&kp.public_key),
                    psk,
                    json_escape(&config_snippet(s, kp)),
                ));
            }
            format!(
                "{{\n  \"key_pairs\": [\n{}\n  ]\n}}",
                items.join(",\n")
            )
        }
        Format::Conf => {
            let mut blocks: Vec<String> = Vec::with_capacity(keys.len());
            for (i, kp) in keys.iter().enumerate() {
                let header = if keys.len() > 1 {
                    format!("# ---- wg0.conf for key pair {} of {} ----\n", i + 1, keys.len())
                } else {
                    String::new()
                };
                blocks.push(format!("{header}{}", config_snippet(s, kp)));
            }
            blocks.join("\n")
        }
        Format::Text => {
            let mut blocks: Vec<String> = Vec::with_capacity(keys.len());
            for (i, kp) in keys.iter().enumerate() {
                let mut b = String::new();
                if keys.len() > 1 {
                    b.push_str(&format!("# ---- key pair {} of {} ----\n", i + 1, keys.len()));
                }
                b.push_str(&format!("PrivateKey   = {}\n", kp.private_key));
                b.push_str(&format!("PublicKey    = {}\n", kp.public_key));
                if let Some(psk) = &kp.preshared_key {
                    b.push_str(&format!("PresharedKey = {psk}\n"));
                }
                b.push_str("\n# Sample wg0.conf — keep the private key on this device only.\n");
                b.push_str(&config_snippet(s, kp));
                blocks.push(b);
            }
            blocks.join("\n")
        }
    }
}

/// Minimal JSON string escaping (the values are base64/ASCII config text).
fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 8);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out
}

/// Validate, generate and render in one call — the entry point every surface uses.
pub fn run(
    pairs: f64,
    preshared_key: bool,
    format: &str,
    address: &str,
    endpoint: &str,
) -> Result<String, String> {
    let s = settings(pairs, preshared_key, format, address, endpoint)?;
    let mut keys = Vec::with_capacity(s.pairs);
    for _ in 0..s.pairs {
        keys.push(generate_pair(s.preshared_key)?);
    }
    Ok(render(&s, &keys))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixed_pair() -> KeyPair {
        KeyPair {
            private_key: "cGtwa3BrcGtwa3BrcGtwa3BrcGtwa3BrcGtwa3BrcGs=".into(),
            public_key: "cHVicHVicHVicHVicHVicHVicHVicHVicHVicHVicHUs=".into(),
            preshared_key: Some("cHNrcHNrcHNrcHNrcHNrcHNrcHNrcHNrcHNrcHNrcHM=".into()),
        }
    }

    fn default_settings() -> Settings {
        settings(1.0, true, "text", "10.0.0.2/32", "vpn.example.com:51820").unwrap()
    }

    // ---- happy paths ----

    #[test]
    fn generates_wg_shaped_keys() {
        let kp = generate_pair(true).unwrap();
        for key in [&kp.private_key, &kp.public_key, kp.preshared_key.as_ref().unwrap()] {
            assert_eq!(key.len(), 44, "wg keys are 44 base64 chars: {key}");
            assert!(key.ends_with('='), "32 raw bytes always pad with '=': {key}");
            assert_eq!(B64.decode(key).unwrap().len(), 32);
        }
        assert_ne!(kp.private_key, kp.public_key);
    }

    #[test]
    fn private_key_is_clamped_like_wg_genkey() {
        for _ in 0..32 {
            let kp = generate_pair(false).unwrap();
            assert!(kp.preshared_key.is_none());
            let raw = B64.decode(&kp.private_key).unwrap();
            assert_eq!(raw[0] & 7, 0, "low 3 bits cleared");
            assert_eq!(raw[31] & 128, 0, "top bit cleared");
            assert_eq!(raw[31] & 64, 64, "second-highest bit set");
        }
    }

    #[test]
    fn public_key_matches_rfc_7748_vector() {
        // RFC 7748 §6.1 Alice: private scalar → public key. External vector, so
        // this is a real check of the derivation, not a self-consistency loop.
        let private =
            hex32("77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a");
        let expected =
            hex32("8520f0098930a754748b7ddcb43ef75a0dbf3a0d26381af4eba4a98eaa9b4e6a");
        assert_eq!(derive_public(private), expected);
        // X25519 clamps internally, so clamping first must not change anything.
        assert_eq!(derive_public(clamp(private)), expected);
    }

    #[test]
    fn each_pair_is_unique() {
        let a = generate_pair(true).unwrap();
        let b = generate_pair(true).unwrap();
        assert_ne!(a.private_key, b.private_key);
        assert_ne!(a.public_key, b.public_key);
        assert_ne!(a.preshared_key, b.preshared_key);
    }

    #[test]
    fn text_output_renders_keys_and_snippet() {
        let out = render(&default_settings(), &[fixed_pair()]);
        // Built line-by-line rather than as one continued literal: a stray
        // character inside a 20-line `\`-continuation is invisible in a diff.
        let kp = fixed_pair();
        let priv_k = &kp.private_key;
        let pub_k = &kp.public_key;
        let psk = kp.preshared_key.as_ref().unwrap();
        let expected = [
            format!("PrivateKey   = {priv_k}"),
            format!("PublicKey    = {pub_k}"),
            format!("PresharedKey = {psk}"),
            String::new(),
            "# Sample wg0.conf — keep the private key on this device only.".into(),
            "[Interface]".into(),
            format!("PrivateKey = {priv_k}"),
            "Address = 10.0.0.2/32".into(),
            "# ListenPort = 51820   # servers only; clients pick a random port".into(),
            String::new(),
            "[Peer]".into(),
            "# Paste the REMOTE side's public key here.".into(),
            "PublicKey = <remote peer public key>".into(),
            format!("PresharedKey = {psk}"),
            "AllowedIPs = 0.0.0.0/0, ::/0".into(),
            "Endpoint = vpn.example.com:51820".into(),
            "PersistentKeepalive = 25".into(),
            String::new(),
            "# Hand the other side this block so it can reach you:".into(),
            "# [Peer]".into(),
            format!("# PublicKey = {pub_k}"),
            format!("# PresharedKey = {psk}"),
            "# AllowedIPs = 10.0.0.2/32".into(),
        ]
        .join("\n")
            + "\n";
        assert_eq!(out, expected);
    }

    #[test]
    fn no_preshared_key_drops_every_psk_line() {
        let s = default_settings();
        let kp = KeyPair {
            preshared_key: None,
            ..fixed_pair()
        };
        let out = render(&s, &[kp]);
        assert!(!out.contains("PresharedKey"));
        assert!(out.contains("PublicKey = <remote peer public key>"));
    }

    #[test]
    fn conf_format_is_snippet_only() {
        let s = settings(1.0, false, "conf", "10.7.0.5/32", "").unwrap();
        let out = render(&s, &[fixed_pair()]);
        assert!(out.starts_with("[Interface]\n"));
        assert!(!out.contains("PrivateKey   ="), "no key listing in conf mode");
        assert!(out.contains("Address = 10.7.0.5/32"));
        assert!(!out.contains("Endpoint ="), "blank endpoint omits the line");
    }

    #[test]
    fn json_format_is_parseable_and_indexed() {
        let s = settings(2.0, true, "json", "10.0.0.2/32", "vpn.example.com:51820").unwrap();
        let out = render(&s, &[fixed_pair(), fixed_pair()]);
        assert!(out.starts_with("{\n  \"key_pairs\": ["));
        assert!(out.contains("\"index\": 1"));
        assert!(out.contains("\"index\": 2"));
        assert!(out.contains("\"private_key\": \"cGtwa3BrcGtwa3BrcGtwa3BrcGtwa3BrcGtwa3BrcGs=\""));
        assert!(out.contains("\\n[Peer]\\n"), "config newlines are escaped");
    }

    #[test]
    fn json_null_preshared_key_when_disabled() {
        let s = settings(1.0, false, "json", "10.0.0.2/32", "").unwrap();
        let kp = KeyPair {
            preshared_key: None,
            ..fixed_pair()
        };
        assert!(render(&s, &[kp]).contains("\"preshared_key\": null"));
    }

    #[test]
    fn bulk_run_numbers_every_pair() {
        let out = run(3.0, true, "text", "10.0.0.2/32", "vpn.example.com:51820").unwrap();
        assert!(out.contains("# ---- key pair 1 of 3 ----"));
        assert!(out.contains("# ---- key pair 3 of 3 ----"));
        assert_eq!(out.matches("PrivateKey   = ").count(), 3);
    }

    #[test]
    fn address_accepts_a_comma_list_and_ipv6() {
        let s = settings(1.0, true, "text", " 10.0.0.2/32 , fd00::2/128 ", "[fd00::1]:51820").unwrap();
        assert_eq!(s.address, "10.0.0.2/32, fd00::2/128");
        assert_eq!(s.endpoint.as_deref(), Some("[fd00::1]:51820"));
    }

    #[test]
    fn format_parsing_is_case_and_space_tolerant() {
        assert_eq!(settings(1.0, true, " JSON ", "10.0.0.2/32", "").unwrap().format, Format::Json);
        assert_eq!(settings(1.0, true, "", "10.0.0.2/32", "").unwrap().format, Format::Text);
    }

    // ---- error paths ----

    #[test]
    fn rejects_unknown_format() {
        let err = settings(1.0, true, "yaml", "10.0.0.2/32", "").unwrap_err();
        assert!(err.contains("invalid format"), "{err}");
    }

    #[test]
    fn rejects_out_of_range_pair_counts() {
        assert!(settings(0.0, true, "text", "10.0.0.2/32", "")
            .unwrap_err()
            .contains("out of range"));
        assert!(settings(26.0, true, "text", "10.0.0.2/32", "")
            .unwrap_err()
            .contains("out of range"));
        assert!(settings(f64::NAN, true, "text", "10.0.0.2/32", "")
            .unwrap_err()
            .contains("whole number"));
        // the cap boundary itself is fine
        assert_eq!(settings(25.0, true, "text", "10.0.0.2/32", "").unwrap().pairs, 25);
    }

    #[test]
    fn rejects_bad_addresses() {
        assert!(settings(1.0, true, "text", "", "").unwrap_err().contains("address is empty"));
        assert!(settings(1.0, true, "text", "10.0.0.2", "")
            .unwrap_err()
            .contains("no prefix length"));
        assert!(settings(1.0, true, "text", "not-an-ip/32", "")
            .unwrap_err()
            .contains("not a valid IP address"));
        assert!(settings(1.0, true, "text", "10.0.0.2/33", "")
            .unwrap_err()
            .contains("out of range"));
    }

    #[test]
    fn rejects_bad_endpoints() {
        assert!(settings(1.0, true, "text", "10.0.0.2/32", "vpn.example.com")
            .unwrap_err()
            .contains("no port"));
        assert!(settings(1.0, true, "text", "10.0.0.2/32", "vpn.example.com:hello")
            .unwrap_err()
            .contains("not a number"));
        assert!(settings(1.0, true, "text", "10.0.0.2/32", "vpn.example.com:0")
            .unwrap_err()
            .contains("out of range"));
    }

    #[test]
    fn run_reports_errors_before_generating() {
        assert!(run(1.0, true, "xml", "10.0.0.2/32", "").is_err());
    }

    fn hex32(s: &str) -> [u8; 32] {
        let bytes: Vec<u8> = (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect();
        bytes.try_into().unwrap()
    }
}
