//! gizza-ai/wifi-qr-code-generator core — build a Wi-Fi join QR code (the standard
//! `WIFI:` payload phones recognise) and render it as an SVG. Pure-Rust (`qrcode`).
//! No wafer/wasm-bindgen deps.

use qrcode::render::svg;
use qrcode::{EcLevel, QrCode};

/// Wi-Fi authentication type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Security {
    Wpa,
    Wep,
    Nopass,
}

impl Security {
    pub fn parse(s: &str) -> Result<Security, String> {
        match s.trim().to_ascii_lowercase().replace(['-', '_', ' '], "").as_str() {
            "wpa" | "wpa2" | "wpa3" | "" => Ok(Security::Wpa),
            "wep" => Ok(Security::Wep),
            "nopass" | "none" | "open" => Ok(Security::Nopass),
            other => Err(format!("unknown security '{other}' (use WPA, WEP, or nopass)")),
        }
    }
    fn token(self) -> &'static str {
        match self {
            Security::Wpa => "WPA",
            Security::Wep => "WEP",
            Security::Nopass => "nopass",
        }
    }
}

/// Escape a value for the `WIFI:` payload: backslash-escape \ ; , : and ".
fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(c, '\\' | ';' | ',' | ':' | '"') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Build the standard `WIFI:` payload string for a network.
pub fn wifi_payload(ssid: &str, password: &str, security: Security, hidden: bool) -> String {
    let mut s = String::from("WIFI:");
    s.push_str(&format!("T:{};", security.token()));
    s.push_str(&format!("S:{};", esc(ssid)));
    if security != Security::Nopass {
        s.push_str(&format!("P:{};", esc(password)));
    }
    if hidden {
        s.push_str("H:true;");
    }
    s.push(';');
    s
}

/// Generate a Wi-Fi join QR code as an SVG string.
pub fn generate_svg(
    ssid: &str,
    password: &str,
    security: Security,
    hidden: bool,
) -> Result<String, String> {
    if ssid.is_empty() {
        return Err("ssid is required".into());
    }
    if security != Security::Nopass && password.is_empty() {
        return Err("password is required for WPA/WEP networks (use security=nopass for an open network)".into());
    }
    let payload = wifi_payload(ssid, password, security, hidden);
    // Medium error correction is a good default for Wi-Fi QR codes.
    let code = QrCode::with_error_correction_level(payload.as_bytes(), EcLevel::M)
        .map_err(|e| format!("failed to build QR code: {e}"))?;
    let svg = code
        .render::<svg::Color>()
        .min_dimensions(240, 240)
        .quiet_zone(true)
        .build();
    Ok(svg)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn payload_format() {
        let p = wifi_payload("MyNet", "s3cret", Security::Wpa, false);
        assert_eq!(p, "WIFI:T:WPA;S:MyNet;P:s3cret;;");
    }

    #[test]
    fn payload_escapes_special_chars() {
        let p = wifi_payload("Cafe;Net", "a:b\\c", Security::Wpa, false);
        assert_eq!(p, "WIFI:T:WPA;S:Cafe\\;Net;P:a\\:b\\\\c;;");
    }

    #[test]
    fn nopass_omits_password_and_hidden_flag() {
        let p = wifi_payload("Guest", "ignored", Security::Nopass, true);
        assert_eq!(p, "WIFI:T:nopass;S:Guest;H:true;;");
    }

    #[test]
    fn renders_svg() {
        let svg = generate_svg("MyNet", "password123", Security::Wpa, false).unwrap();
        assert!(svg.starts_with("<?xml") || svg.contains("<svg"));
        assert!(svg.contains("</svg>"));
    }

    #[test]
    fn errors() {
        assert!(generate_svg("", "pw", Security::Wpa, false).is_err()); // no ssid
        assert!(generate_svg("Net", "", Security::Wpa, false).is_err()); // no password for WPA
        // open network needs no password
        assert!(generate_svg("Open", "", Security::Nopass, false).is_ok());
        assert!(Security::parse("rot13").is_err());
    }
}
