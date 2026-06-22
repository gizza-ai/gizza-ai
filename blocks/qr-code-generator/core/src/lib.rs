//! gizza-ai/qr-code-generator core — build a QR code from text, a URL, Wi-Fi
//! credentials, or a contact card, and render it as an SVG or PNG. Pure-Rust
//! (`qrcode` + `image`), so it runs on ALL backends incl. the chat Service
//! Worker. No wafer/wasm-bindgen deps.

use qrcode::render::svg;
use qrcode::{EcLevel, QrCode};

/// Error-correction level. Higher levels survive more damage/occlusion at the
/// cost of denser codes (so larger when the data is fixed).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ecc {
    Low,
    Medium,
    Quartile,
    High,
}

impl Ecc {
    pub fn parse(s: &str) -> Result<Ecc, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "l" | "low" => Ok(Ecc::Low),
            "m" | "medium" | "med" | "" => Ok(Ecc::Medium),
            "q" | "quartile" | "quart" => Ok(Ecc::Quartile),
            "h" | "high" => Ok(Ecc::High),
            other => Err(format!("unknown error correction '{other}' (use L, M, Q, or H)")),
        }
    }
    fn level(self) -> EcLevel {
        match self {
            Ecc::Low => EcLevel::L,
            Ecc::Medium => EcLevel::M,
            Ecc::Quartile => EcLevel::Q,
            Ecc::High => EcLevel::H,
        }
    }
}

/// Output image format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Svg,
    Png,
}

impl Format {
    pub fn parse(s: &str) -> Result<Format, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "svg" | "" => Ok(Format::Svg),
            "png" => Ok(Format::Png),
            other => Err(format!("unknown format '{other}' (use svg or png)")),
        }
    }
    pub fn mime(self) -> &'static str {
        match self {
            Format::Svg => "image/svg+xml",
            Format::Png => "image/png",
        }
    }
    pub fn ext(self) -> &'static str {
        match self {
            Format::Svg => "svg",
            Format::Png => "png",
        }
    }
}

/// Content type — how to interpret the inputs into the encoded payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Content {
    Text,
    Url,
    Wifi,
    Contact,
    Email,
    Sms,
    Phone,
    Geo,
}

impl Content {
    pub fn parse(s: &str) -> Result<Content, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "text" | "" => Ok(Content::Text),
            "url" | "link" => Ok(Content::Url),
            "wifi" | "wi-fi" => Ok(Content::Wifi),
            "contact" | "vcard" | "mecard" => Ok(Content::Contact),
            "email" | "mail" | "mailto" => Ok(Content::Email),
            "sms" | "text-message" => Ok(Content::Sms),
            "phone" | "tel" | "call" => Ok(Content::Phone),
            "geo" | "location" | "gps" => Ok(Content::Geo),
            other => Err(format!(
                "unknown content type '{other}' (use text, url, wifi, contact, email, sms, phone, or geo)"
            )),
        }
    }
}

/// Optional fields used to build the encoded payload depending on `Content`.
#[derive(Debug, Default, Clone)]
pub struct Fields {
    /// Free text / URL / any payload (text + url modes).
    pub data: String,
    // wifi
    pub ssid: String,
    pub password: String,
    pub security: String,
    pub hidden: bool,
    // contact (vCard)
    pub name: String,
    pub phone: String,
    pub email: String,
    pub organization: String,
    pub url: String,
    // email / sms message body
    pub subject: String,
    pub body: String,
    // geo
    pub latitude: String,
    pub longitude: String,
}

/// Validate a hex colour like `#000` or `#ffffff`; returns it lower-cased.
fn norm_color(s: &str, default: &str) -> Result<String, String> {
    let s = s.trim();
    if s.is_empty() {
        return Ok(default.to_string());
    }
    let body = s.strip_prefix('#').unwrap_or(s);
    let ok = matches!(body.len(), 3 | 6) && body.chars().all(|c| c.is_ascii_hexdigit());
    if !ok {
        return Err(format!("invalid colour '{s}' (use #rgb or #rrggbb hex)"));
    }
    Ok(format!("#{}", body.to_ascii_lowercase()))
}

/// Escape a value for a `WIFI:` payload: backslash-escape \ ; , : and ".
fn esc_wifi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(c, '\\' | ';' | ',' | ':' | '"') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// Percent-encode a string for use in a `mailto:` query (RFC 3986 — keep
/// unreserved chars, encode everything else, space → %20).
fn percent_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}

fn wifi_token(security: &str) -> Result<&'static str, String> {
    match security.trim().to_ascii_lowercase().replace(['-', '_', ' '], "").as_str() {
        "wpa" | "wpa2" | "wpa3" | "" => Ok("WPA"),
        "wep" => Ok("WEP"),
        "nopass" | "none" | "open" => Ok("nopass"),
        other => Err(format!("unknown security '{other}' (use WPA, WEP, or nopass)")),
    }
}

/// Build the encoded payload string for the chosen content type.
pub fn build_payload(content: Content, f: &Fields) -> Result<String, String> {
    match content {
        Content::Text => {
            if f.data.is_empty() {
                return Err("data is required for text content".into());
            }
            Ok(f.data.clone())
        }
        Content::Url => {
            let d = f.data.trim();
            if d.is_empty() {
                return Err("data (the URL) is required for url content".into());
            }
            // Add a scheme if the user gave a bare host so scanners open it as a link.
            if d.contains("://") || d.starts_with("mailto:") || d.starts_with("tel:") {
                Ok(d.to_string())
            } else {
                Ok(format!("https://{d}"))
            }
        }
        Content::Wifi => {
            if f.ssid.is_empty() {
                return Err("ssid is required for wifi content".into());
            }
            let tok = wifi_token(&f.security)?;
            if tok != "nopass" && f.password.is_empty() {
                return Err("password is required for WPA/WEP networks (use security=nopass for an open network)".into());
            }
            let mut s = String::from("WIFI:");
            s.push_str(&format!("T:{tok};"));
            s.push_str(&format!("S:{};", esc_wifi(&f.ssid)));
            if tok != "nopass" {
                s.push_str(&format!("P:{};", esc_wifi(&f.password)));
            }
            if f.hidden {
                s.push_str("H:true;");
            }
            s.push(';');
            Ok(s)
        }
        Content::Contact => {
            if f.name.is_empty() && f.phone.is_empty() && f.email.is_empty() {
                return Err("at least one of name, phone, or email is required for contact content".into());
            }
            // Standard vCard 3.0 — widely recognised by phone cameras.
            let mut s = String::from("BEGIN:VCARD\nVERSION:3.0\n");
            if !f.name.is_empty() {
                s.push_str(&format!("FN:{}\n", f.name));
                s.push_str(&format!("N:{};;;;\n", f.name));
            }
            if !f.organization.is_empty() {
                s.push_str(&format!("ORG:{}\n", f.organization));
            }
            if !f.phone.is_empty() {
                s.push_str(&format!("TEL:{}\n", f.phone));
            }
            if !f.email.is_empty() {
                s.push_str(&format!("EMAIL:{}\n", f.email));
            }
            if !f.url.is_empty() {
                s.push_str(&format!("URL:{}\n", f.url));
            }
            s.push_str("END:VCARD");
            Ok(s)
        }
        Content::Email => {
            // Prefer the explicit email field; fall back to data as the address.
            let addr = if !f.email.is_empty() { f.email.trim() } else { f.data.trim() };
            if addr.is_empty() {
                return Err("email (the recipient address) is required for email content".into());
            }
            let mut query = Vec::new();
            if !f.subject.is_empty() {
                query.push(format!("subject={}", percent_encode(&f.subject)));
            }
            if !f.body.is_empty() {
                query.push(format!("body={}", percent_encode(&f.body)));
            }
            let mut s = format!("mailto:{addr}");
            if !query.is_empty() {
                s.push('?');
                s.push_str(&query.join("&"));
            }
            Ok(s)
        }
        Content::Sms => {
            let num = if !f.phone.is_empty() { f.phone.trim() } else { f.data.trim() };
            if num.is_empty() {
                return Err("phone (the recipient number) is required for sms content".into());
            }
            // SMSTO:number:message is the most widely recognised SMS payload.
            if f.body.is_empty() {
                Ok(format!("SMSTO:{num}"))
            } else {
                Ok(format!("SMSTO:{num}:{}", f.body))
            }
        }
        Content::Phone => {
            let num = if !f.phone.is_empty() { f.phone.trim() } else { f.data.trim() };
            if num.is_empty() {
                return Err("phone (the number to dial) is required for phone content".into());
            }
            Ok(format!("tel:{num}"))
        }
        Content::Geo => {
            let lat = f.latitude.trim();
            let lng = f.longitude.trim();
            if lat.is_empty() || lng.is_empty() {
                return Err("latitude and longitude are required for geo content".into());
            }
            // Validate they parse as numbers so scanners get a usable geo: URI.
            lat.parse::<f64>().map_err(|_| format!("invalid latitude '{lat}'"))?;
            lng.parse::<f64>().map_err(|_| format!("invalid longitude '{lng}'"))?;
            Ok(format!("geo:{lat},{lng}"))
        }
    }
}

/// Render the payload as an SVG string with the given colours.
fn render_svg(code: &QrCode, dark: &str, light: &str) -> String {
    let mut r = code.render::<svg::Color>();
    r.min_dimensions(256, 256)
        .quiet_zone(true)
        .dark_color(svg::Color(dark))
        .light_color(svg::Color(light));
    r.build()
}

/// Render the payload as PNG bytes. `size` is the target image edge in pixels
/// (the rendered code is scaled to the nearest whole-module multiple ≥ size).
fn render_png(code: &QrCode, size: u32, dark: [u8; 3], light: [u8; 3]) -> Result<Vec<u8>, String> {
    use image::{ImageEncoder, Luma};
    // Build a luma image scaled to `size`, with a 4-module quiet zone, then
    // recolour to the requested dark/light colours.
    let img = code
        .render::<Luma<u8>>()
        .min_dimensions(size, size)
        .quiet_zone(true)
        .build();
    let (w, h) = (img.width(), img.height());
    let mut rgb = image::RgbImage::new(w, h);
    for (x, y, px) in img.enumerate_pixels() {
        let c = if px.0[0] < 128 { dark } else { light };
        rgb.put_pixel(x, y, image::Rgb(c));
    }
    let mut out = Vec::new();
    image::codecs::png::PngEncoder::new(&mut out)
        .write_image(&rgb, w, h, image::ExtendedColorType::Rgb8)
        .map_err(|e| format!("failed to encode PNG: {e}"))?;
    Ok(out)
}

fn hex_to_rgb(s: &str) -> [u8; 3] {
    let body = s.trim_start_matches('#');
    let full = if body.len() == 3 {
        body.chars().flat_map(|c| [c, c]).collect::<String>()
    } else {
        body.to_string()
    };
    let r = u8::from_str_radix(&full[0..2], 16).unwrap_or(0);
    let g = u8::from_str_radix(&full[2..4], 16).unwrap_or(0);
    let b = u8::from_str_radix(&full[4..6], 16).unwrap_or(0);
    [r, g, b]
}

/// The rendered QR code: either SVG (UTF-8 text) or PNG (binary) bytes.
pub struct Generated {
    pub bytes: Vec<u8>,
    pub format: Format,
    /// The raw payload that was encoded (handy for the LLM summary).
    pub payload: String,
}

/// Generate a QR code. `size` is only used for PNG output (pixels per edge,
/// clamped to a sane range).
#[allow(clippy::too_many_arguments)]
pub fn generate(
    content: Content,
    fields: &Fields,
    format: Format,
    ecc: Ecc,
    size: u32,
    dark: &str,
    light: &str,
) -> Result<Generated, String> {
    let payload = build_payload(content, fields)?;
    let dark = norm_color(dark, "#000000")?;
    let light = norm_color(light, "#ffffff")?;
    let code = QrCode::with_error_correction_level(payload.as_bytes(), ecc.level())
        .map_err(|e| format!("failed to build QR code: {e}"))?;
    let bytes = match format {
        Format::Svg => render_svg(&code, &dark, &light).into_bytes(),
        Format::Png => {
            let size = size.clamp(64, 2048);
            render_png(&code, size, hex_to_rgb(&dark), hex_to_rgb(&light))?
        }
    };
    Ok(Generated { bytes, format, payload })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_fields(s: &str) -> Fields {
        Fields { data: s.to_string(), ..Default::default() }
    }

    #[test]
    fn text_payload() {
        let p = build_payload(Content::Text, &text_fields("hello world")).unwrap();
        assert_eq!(p, "hello world");
    }

    #[test]
    fn url_adds_scheme() {
        let p = build_payload(Content::Url, &text_fields("example.com")).unwrap();
        assert_eq!(p, "https://example.com");
        let p2 = build_payload(Content::Url, &text_fields("http://x.io")).unwrap();
        assert_eq!(p2, "http://x.io");
    }

    #[test]
    fn wifi_payload_format() {
        let f = Fields {
            ssid: "MyNet".into(),
            password: "s3cret".into(),
            security: "WPA".into(),
            ..Default::default()
        };
        let p = build_payload(Content::Wifi, &f).unwrap();
        assert_eq!(p, "WIFI:T:WPA;S:MyNet;P:s3cret;;");
    }

    #[test]
    fn wifi_escapes_and_nopass() {
        let f = Fields { ssid: "Cafe;Net".into(), security: "nopass".into(), hidden: true, ..Default::default() };
        let p = build_payload(Content::Wifi, &f).unwrap();
        assert_eq!(p, "WIFI:T:nopass;S:Cafe\\;Net;H:true;;");
    }

    #[test]
    fn contact_vcard() {
        let f = Fields {
            name: "Jane Doe".into(),
            phone: "+15551234".into(),
            email: "jane@x.io".into(),
            organization: "Acme".into(),
            ..Default::default()
        };
        let p = build_payload(Content::Contact, &f).unwrap();
        assert!(p.starts_with("BEGIN:VCARD\nVERSION:3.0"));
        assert!(p.contains("FN:Jane Doe"));
        assert!(p.contains("TEL:+15551234"));
        assert!(p.contains("EMAIL:jane@x.io"));
        assert!(p.contains("ORG:Acme"));
        assert!(p.ends_with("END:VCARD"));
    }

    #[test]
    fn email_payload() {
        let f = Fields {
            email: "a@b.io".into(),
            subject: "Hi there".into(),
            body: "Line 1 & 2".into(),
            ..Default::default()
        };
        let p = build_payload(Content::Email, &f).unwrap();
        assert_eq!(p, "mailto:a@b.io?subject=Hi%20there&body=Line%201%20%26%202");
        // bare address, no query
        let f2 = Fields { email: "x@y.io".into(), ..Default::default() };
        assert_eq!(build_payload(Content::Email, &f2).unwrap(), "mailto:x@y.io");
    }

    #[test]
    fn sms_and_phone_payload() {
        let f = Fields { phone: "+15551234".into(), body: "hello".into(), ..Default::default() };
        assert_eq!(build_payload(Content::Sms, &f).unwrap(), "SMSTO:+15551234:hello");
        let f2 = Fields { phone: "+15551234".into(), ..Default::default() };
        assert_eq!(build_payload(Content::Sms, &f2).unwrap(), "SMSTO:+15551234");
        assert_eq!(build_payload(Content::Phone, &f2).unwrap(), "tel:+15551234");
    }

    #[test]
    fn geo_payload() {
        let f = Fields { latitude: "37.422".into(), longitude: "-122.084".into(), ..Default::default() };
        assert_eq!(build_payload(Content::Geo, &f).unwrap(), "geo:37.422,-122.084");
        // non-numeric lat errors
        let bad = Fields { latitude: "north".into(), longitude: "5".into(), ..Default::default() };
        assert!(build_payload(Content::Geo, &bad).is_err());
        // missing errors
        assert!(build_payload(Content::Geo, &Fields::default()).is_err());
    }

    #[test]
    fn generates_svg() {
        let g = generate(Content::Text, &text_fields("hi"), Format::Svg, Ecc::Medium, 256, "#000", "#fff").unwrap();
        let s = String::from_utf8(g.bytes).unwrap();
        assert!(s.contains("<svg"));
        assert!(s.contains("</svg>"));
        assert!(s.contains("fill=\"#000\"")); // dark colour applied (3-char hex kept as-is)
    }

    #[test]
    fn generates_png() {
        let g = generate(Content::Url, &text_fields("example.com"), Format::Png, Ecc::High, 300, "#102030", "#ffffff").unwrap();
        assert_eq!(g.format, Format::Png);
        // PNG magic bytes.
        assert_eq!(&g.bytes[0..8], &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
        assert_eq!(g.payload, "https://example.com");
    }

    #[test]
    fn parsers_and_errors() {
        assert_eq!(Ecc::parse("q").unwrap(), Ecc::Quartile);
        assert!(Ecc::parse("zzz").is_err());
        assert_eq!(Format::parse("PNG").unwrap(), Format::Png);
        assert!(Format::parse("gif").is_err());
        assert_eq!(Content::parse("vcard").unwrap(), Content::Contact);
        assert!(Content::parse("nope").is_err());
        // empty text errors
        assert!(generate(Content::Text, &Fields::default(), Format::Svg, Ecc::Medium, 256, "", "").is_err());
        // wifi without ssid errors
        assert!(generate(Content::Wifi, &Fields::default(), Format::Svg, Ecc::Medium, 256, "", "").is_err());
        // bad colour errors
        assert!(generate(Content::Text, &text_fields("hi"), Format::Svg, Ecc::Medium, 256, "#xyz", "").is_err());
    }
}
