//! gizza-ai/otpauth-qr-generator core — render a scannable QR code from an
//! `otpauth://` provisioning URI (or its individual fields) so an authenticator
//! app can import a 2FA account by scanning it. Pure-Rust (`qrcode` + `image`),
//! so it runs on ALL backends incl. the chat Service Worker. The otpauth URI is
//! built/validated by the shared `otpauth-uri` core; this crate only adds the
//! QR rendering (SVG or PNG, error-correction level, size, colours).

use gizza_ai_otpauth_uri_core::{build, Algo, OtpAuth, OtpType};
use qrcode::render::svg;
use qrcode::{EcLevel, QrCode};

/// Error-correction level. Higher levels survive more damage/occlusion at the
/// cost of a denser (larger) code. `M` (~15%) is the authenticator-app default.
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

/// The individual otpauth fields used when no full `uri` is supplied. Mirrors
/// the `otpauth-uri` tool's inputs.
#[derive(Debug, Clone)]
pub struct Fields {
    pub otp_type: String,
    pub issuer: String,
    pub account: String,
    pub secret: String,
    pub algorithm: String,
    pub digits: u32,
    pub period: u64,
    pub counter: u64,
}

impl Default for Fields {
    fn default() -> Self {
        Fields {
            otp_type: "totp".into(),
            issuer: String::new(),
            account: String::new(),
            secret: String::new(),
            algorithm: "sha1".into(),
            digits: 6,
            period: 30,
            counter: 0,
        }
    }
}

/// Validate a value for the QR colour: a `#rgb` / `#rrggbb` hex string. Returns
/// it lower-cased with a leading `#`. Empty → the supplied default.
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

fn hex_to_rgb(s: &str) -> [u8; 3] {
    let body = s.trim_start_matches('#');
    let full = if body.len() == 3 {
        body.chars().flat_map(|c| [c, c]).collect::<String>()
    } else {
        body.to_string()
    };
    let r = u8::from_str_radix(&full[0..2], 16).unwrap_or(0);
    let g = u8::from_str_radix(&full[2..4], 16).unwrap_or(0);
    let b = u8::from_str_radix(&full[4..6], 16).unwrap_or(255);
    [r, g, b]
}

/// Basic sanity check on a pasted otpauth URI: it must be a totp/hotp
/// provisioning URI and carry a secret. We deliberately keep this lenient (we
/// pass the URI through verbatim) so exotic-but-valid URIs still encode.
fn validate_uri(uri: &str) -> Result<(), String> {
    let lower = uri.to_ascii_lowercase();
    if !(lower.starts_with("otpauth://totp/") || lower.starts_with("otpauth://hotp/")) {
        return Err(
            "uri must be an otpauth:// provisioning URI starting with otpauth://totp/ or otpauth://hotp/"
                .into(),
        );
    }
    if !lower.contains("secret=") {
        return Err("otpauth uri must include a secret= parameter".into());
    }
    Ok(())
}

/// Resolve the otpauth URI to encode: use `uri` verbatim if it is non-empty
/// (after a light validation), otherwise build it from the individual fields
/// via the shared otpauth-uri core.
pub fn resolve_uri(uri: &str, f: &Fields) -> Result<String, String> {
    let uri = uri.trim();
    if !uri.is_empty() {
        validate_uri(uri)?;
        return Ok(uri.to_string());
    }
    let otp_type = OtpType::parse(&f.otp_type)?;
    let algorithm = Algo::parse(&f.algorithm)?;
    build(&OtpAuth {
        otp_type,
        issuer: f.issuer.clone(),
        account: f.account.clone(),
        secret: f.secret.clone(),
        algorithm,
        digits: f.digits,
        period: f.period,
        counter: f.counter,
    })
}

/// Render a QR code (already built) to an SVG string with the given colours.
fn render_svg(code: &QrCode, dark: &str, light: &str) -> String {
    let mut r = code.render::<svg::Color>();
    r.min_dimensions(240, 240)
        .quiet_zone(true)
        .dark_color(svg::Color(dark))
        .light_color(svg::Color(light));
    r.build()
}

/// Render a QR code to PNG bytes. `size` is the target image edge in pixels
/// (scaled to the nearest whole-module multiple ≥ size), with a 4-module quiet
/// zone, recoloured to the requested dark/light colours.
fn render_png(code: &QrCode, size: u32, dark: [u8; 3], light: [u8; 3]) -> Result<Vec<u8>, String> {
    use image::{ImageEncoder, Luma};
    let img = code.render::<Luma<u8>>().min_dimensions(size, size).quiet_zone(true).build();
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

/// The rendered QR code plus the otpauth URI it encodes.
pub struct Generated {
    pub bytes: Vec<u8>,
    pub format: Format,
    /// The otpauth:// URI that was encoded (handy for the summary; contains the secret).
    pub uri: String,
}

/// Build the otpauth URI (from `uri` or `fields`) and render it as a QR code.
/// `size` is only used for PNG output (pixels per edge, clamped to a sane range).
#[allow(clippy::too_many_arguments)]
pub fn generate(
    uri: &str,
    fields: &Fields,
    format: Format,
    ecc: Ecc,
    size: u32,
    dark: &str,
    light: &str,
) -> Result<Generated, String> {
    let otpauth = resolve_uri(uri, fields)?;
    let dark = norm_color(dark, "#000000")?;
    let light = norm_color(light, "#ffffff")?;
    let code = QrCode::with_error_correction_level(otpauth.as_bytes(), ecc.level())
        .map_err(|e| format!("failed to build QR code: {e}"))?;
    let bytes = match format {
        Format::Svg => render_svg(&code, &dark, &light).into_bytes(),
        Format::Png => {
            let size = size.clamp(64, 2048);
            render_png(&code, size, hex_to_rgb(&dark), hex_to_rgb(&light))?
        }
    };
    Ok(Generated { bytes, format, uri: otpauth })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn totp_fields() -> Fields {
        Fields {
            issuer: "Example".into(),
            account: "alice@google.com".into(),
            secret: "JBSWY3DPEHPK3PXP".into(),
            ..Default::default()
        }
    }

    #[test]
    fn resolves_uri_from_fields() {
        let uri = resolve_uri("", &totp_fields()).unwrap();
        assert_eq!(
            uri,
            "otpauth://totp/Example:alice%40google.com?secret=JBSWY3DPEHPK3PXP&issuer=Example&algorithm=SHA1&digits=6&period=30"
        );
    }

    #[test]
    fn passes_through_a_pasted_uri_verbatim() {
        let pasted = "otpauth://totp/ACME:bob?secret=GEZDGNBVGY3TQOJQ&issuer=ACME&digits=6&period=30";
        let uri = resolve_uri(pasted, &Fields::default()).unwrap();
        assert_eq!(uri, pasted);
    }

    #[test]
    fn hotp_uri_from_fields() {
        let f = Fields { otp_type: "hotp".into(), counter: 5, ..totp_fields() };
        let uri = resolve_uri("", &f).unwrap();
        assert!(uri.starts_with("otpauth://hotp/"));
        assert!(uri.ends_with("&counter=5"));
    }

    #[test]
    fn generates_svg() {
        let g = generate("", &totp_fields(), Format::Svg, Ecc::Medium, 240, "#000", "#fff").unwrap();
        let s = String::from_utf8(g.bytes).unwrap();
        assert!(s.contains("<svg"));
        assert!(s.contains("</svg>"));
        assert!(s.contains("fill=\"#000\"")); // dark colour applied
        assert!(g.uri.starts_with("otpauth://totp/"));
    }

    #[test]
    fn generates_png() {
        let g = generate("", &totp_fields(), Format::Png, Ecc::Quartile, 300, "#102030", "#ffffff")
            .unwrap();
        assert_eq!(g.format, Format::Png);
        // PNG magic bytes.
        assert_eq!(&g.bytes[0..8], &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
    }

    #[test]
    fn parsers() {
        assert_eq!(Ecc::parse("h").unwrap(), Ecc::High);
        assert!(Ecc::parse("zzz").is_err());
        assert_eq!(Format::parse("PNG").unwrap(), Format::Png);
        assert!(Format::parse("gif").is_err());
    }

    #[test]
    fn errors() {
        // No uri and no fields → the otpauth-uri core rejects the empty account/secret.
        assert!(generate("", &Fields::default(), Format::Svg, Ecc::Medium, 240, "", "").is_err());
        // A pasted non-otpauth URI is rejected.
        assert!(resolve_uri("https://example.com", &Fields::default()).is_err());
        // A pasted otpauth URI with no secret is rejected.
        assert!(resolve_uri("otpauth://totp/X:y?issuer=X", &Fields::default()).is_err());
        // Bad colour.
        assert!(generate("", &totp_fields(), Format::Svg, Ecc::Medium, 240, "#xyz", "").is_err());
        // Bad secret (not base32) via fields.
        let bad = Fields { secret: "not!base32".into(), ..totp_fields() };
        assert!(generate("", &bad, Format::Svg, Ecc::Medium, 240, "", "").is_err());
    }
}
