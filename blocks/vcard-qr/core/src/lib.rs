//! vcard-qr core — build an RFC 6350 / RFC 2426 vCard from single-contact fields
//! and render it as a scannable QR code SVG.
//!
//! Pure Rust: `qrcode` for the encode (SVG feature only, no image dep). No
//! wafer/wasm-bindgen deps, so the same logic backs the chat block, the CLI and
//! the browser page.
//!
//! The exact vCard source is never hidden: it goes into the SVG `<desc>` element
//! and is returned alongside the SVG so callers can save it as a `.vcf` file.

use qrcode::{EcLevel, QrCode};

/// Smallest / largest rendered SVG width, in pixels.
pub const MIN_SIZE: u32 = 128;
pub const MAX_SIZE: u32 = 2048;

/// Quiet zone in modules, per the QR spec.
const QUIET_ZONE: usize = 4;

/// vCard dialect to emit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Version {
    V3,
    V4,
}

impl Version {
    pub fn parse(s: &str) -> Result<Version, String> {
        match s.trim() {
            "3.0" | "3" | "" => Ok(Version::V3),
            "4.0" | "4" => Ok(Version::V4),
            other => Err(format!(
                "unknown version '{other}' (use 3.0 for maximum phone compatibility, or 4.0)"
            )),
        }
    }

    fn label(self) -> &'static str {
        match self {
            Version::V3 => "3.0",
            Version::V4 => "4.0",
        }
    }

    /// vCard 3.0 spells TYPE values in upper case, 4.0 in lower case.
    fn type_value(self, upper: &'static str, lower: &'static str) -> &'static str {
        match self {
            Version::V3 => upper,
            Version::V4 => lower,
        }
    }
}

/// QR error-correction level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ecc {
    L,
    M,
    Q,
    H,
}

impl Ecc {
    pub fn parse(s: &str) -> Result<Ecc, String> {
        match s.trim().to_ascii_uppercase().as_str() {
            "L" => Ok(Ecc::L),
            "M" | "" => Ok(Ecc::M),
            "Q" => Ok(Ecc::Q),
            "H" => Ok(Ecc::H),
            other => Err(format!(
                "unknown error_correction '{other}' (use L, M, Q, or H)"
            )),
        }
    }

    fn level(self) -> EcLevel {
        match self {
            Ecc::L => EcLevel::L,
            Ecc::M => EcLevel::M,
            Ecc::Q => EcLevel::Q,
            Ecc::H => EcLevel::H,
        }
    }
}

/// Every input the builder + renderer needs. Built by the block, the CLI and the
/// page alike.
#[derive(Debug, Clone)]
pub struct Options<'a> {
    pub first_name: &'a str,
    pub last_name: &'a str,
    pub organization: &'a str,
    pub job_title: &'a str,
    pub mobile: &'a str,
    pub phone: &'a str,
    pub email: &'a str,
    pub website: &'a str,
    pub street: &'a str,
    pub city: &'a str,
    pub region: &'a str,
    pub postal_code: &'a str,
    pub country: &'a str,
    pub note: &'a str,
    pub birthday: &'a str,
    pub version: &'a str,
    pub error_correction: &'a str,
    pub size: u32,
    pub foreground: &'a str,
    pub background: &'a str,
    pub show_details: bool,
}

impl Default for Options<'_> {
    fn default() -> Self {
        Options {
            first_name: "",
            last_name: "",
            organization: "",
            job_title: "",
            mobile: "",
            phone: "",
            email: "",
            website: "",
            street: "",
            city: "",
            region: "",
            postal_code: "",
            country: "",
            note: "",
            birthday: "",
            version: "3.0",
            error_correction: "M",
            size: 512,
            foreground: "#000000",
            background: "#ffffff",
            show_details: true,
        }
    }
}

/// Escape one vCard property VALUE (RFC 6350 §3.4): backslash, newline, comma
/// and semicolon. Structured values escape each component, then join the
/// components with an UNescaped `;`.
fn escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => {}
            ',' => out.push_str("\\,"),
            ';' => out.push_str("\\;"),
            c => out.push(c),
        }
    }
    out
}

/// Fold one logical line to ≤75 octets with CRLF + single-space continuations
/// (RFC 6350 §3.2), never splitting a multibyte char. Short lines are untouched,
/// which is the common case for contact fields.
fn fold(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut bytes = 0usize;
    let mut started = false;
    for ch in line.chars() {
        let cl = ch.len_utf8();
        if started && bytes + cl > 75 {
            out.push_str("\r\n ");
            bytes = 1; // the leading continuation space
        }
        out.push(ch);
        bytes += cl;
        started = true;
    }
    out
}

fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

/// Accept a hex colour (`#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`) or a CSS colour
/// name (which keeps `transparent` expressible).
fn check_color(value: &str, field: &str) -> Result<String, String> {
    let v = value.trim();
    if v.is_empty() {
        return Err(format!("{field} is empty"));
    }
    if v.len() > 32 {
        return Err(format!("{field} '{v}' is too long to be a colour"));
    }
    if let Some(hex) = v.strip_prefix('#') {
        if !matches!(hex.len(), 3 | 4 | 6 | 8) || !hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(format!(
                "{field} '{v}' is not a valid hex colour (use #rgb, #rrggbb or #rrggbbaa)"
            ));
        }
        return Ok(v.to_string());
    }
    if v.chars().all(|c| c.is_ascii_alphabetic()) {
        return Ok(v.to_ascii_lowercase());
    }
    Err(format!(
        "{field} '{v}' is not a colour — use a hex value like #1a1a1a or a CSS colour name"
    ))
}

/// Minimal sanity check — a QR that carries a typo'd address is worse than an
/// error, because nobody notices until the contact has been saved.
fn check_email(value: &str) -> Result<String, String> {
    let v = value.trim();
    if v.is_empty() {
        return Ok(String::new());
    }
    let mut parts = v.split('@');
    let local = parts.next().unwrap_or("");
    let domain = parts.next().unwrap_or("");
    let extra = parts.next();
    if local.is_empty()
        || domain.is_empty()
        || extra.is_some()
        || !domain.contains('.')
        || domain.starts_with('.')
        || domain.ends_with('.')
        || v.chars().any(|c| c.is_whitespace())
    {
        return Err(format!(
            "email '{v}' is not a valid address (expected something like name@example.com)"
        ));
    }
    Ok(v.to_string())
}

/// A bare host is what people type; scanners need a scheme to open it.
fn normalize_website(value: &str) -> Result<String, String> {
    let v = value.trim();
    if v.is_empty() {
        return Ok(String::new());
    }
    if v.chars().any(|c| c.is_whitespace()) {
        return Err(format!("website '{v}' must not contain spaces"));
    }
    if v.contains("://") || v.starts_with("mailto:") {
        return Ok(v.to_string());
    }
    Ok(format!("https://{v}"))
}

/// `BDAY` wants an ISO 8601 calendar date. Accept `YYYY-MM-DD` and the basic
/// `YYYYMMDD` form, and always emit the extended form.
fn normalize_birthday(value: &str) -> Result<String, String> {
    let v = value.trim();
    if v.is_empty() {
        return Ok(String::new());
    }
    let digits: String = v.chars().filter(|c| *c != '-').collect();
    let bad = || format!("birthday '{v}' is not a date (use YYYY-MM-DD, e.g. 1987-04-23)");
    if digits.len() != 8 || !digits.chars().all(|c| c.is_ascii_digit()) {
        return Err(bad());
    }
    if v.contains('-') && (v.len() != 10 || v.as_bytes()[4] != b'-' || v.as_bytes()[7] != b'-') {
        return Err(bad());
    }
    let year: u32 = digits[0..4].parse().map_err(|_| bad())?;
    let month: u32 = digits[4..6].parse().map_err(|_| bad())?;
    let day: u32 = digits[6..8].parse().map_err(|_| bad())?;
    if !(1..=12).contains(&month) {
        return Err(format!("birthday '{v}' has month {month} (must be 01-12)"));
    }
    let leap = (year % 4 == 0 && year % 100 != 0) || year % 400 == 0;
    let max_day = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ if leap => 29,
        _ => 28,
    };
    if day < 1 || day > max_day {
        return Err(format!(
            "birthday '{v}' has day {day}, but month {month:02} of {year} has {max_day} days"
        ));
    }
    Ok(format!("{year:04}-{month:02}-{day:02}"))
}

/// The display name: "First Last", falling back to whichever part exists, then
/// to the organization. vCard requires a non-empty FN.
fn formatted_name(first: &str, last: &str, org: &str) -> String {
    let parts: Vec<&str> = [first, last]
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    if !parts.is_empty() {
        return parts.join(" ");
    }
    org.trim().to_string()
}

/// Build the vCard text (CRLF line endings, folded, escaped).
///
/// Returns the `.vcf` source exactly as it is encoded into the QR code.
pub fn build_vcard(opts: &Options) -> Result<String, String> {
    let version = Version::parse(opts.version)?;
    let first = opts.first_name.trim();
    let last = opts.last_name.trim();
    let org = opts.organization.trim();
    let fname = formatted_name(first, last, org);
    if fname.is_empty() {
        return Err(
            "a contact needs a name — set first_name and/or last_name (or at least organization)"
                .into(),
        );
    }
    let email = check_email(opts.email)?;
    let website = normalize_website(opts.website)?;
    let birthday = normalize_birthday(opts.birthday)?;

    let mut lines: Vec<String> = Vec::new();
    let mut push = |prop: String| lines.push(fold(&prop));

    push("BEGIN:VCARD".to_string());
    push(format!("VERSION:{}", version.label()));
    // N = family;given;additional;prefixes;suffixes
    push(format!("N:{};{};;;", escape(last), escape(first)));
    push(format!("FN:{}", escape(&fname)));
    if !org.is_empty() {
        push(format!("ORG:{}", escape(org)));
    }
    if !opts.job_title.trim().is_empty() {
        push(format!("TITLE:{}", escape(opts.job_title.trim())));
    }
    if !opts.mobile.trim().is_empty() {
        push(format!(
            "TEL;TYPE={}:{}",
            version.type_value("CELL", "cell"),
            escape(opts.mobile.trim())
        ));
    }
    if !opts.phone.trim().is_empty() {
        push(format!(
            "TEL;TYPE={}:{}",
            version.type_value("WORK,VOICE", "work,voice"),
            escape(opts.phone.trim())
        ));
    }
    if !email.is_empty() {
        push(format!(
            "EMAIL;TYPE={}:{}",
            version.type_value("INTERNET", "internet"),
            escape(&email)
        ));
    }
    if !website.is_empty() {
        push(format!("URL:{}", escape(&website)));
    }
    // ADR = po-box;extended;street;locality;region;postal-code;country
    let adr = [
        opts.street.trim(),
        opts.city.trim(),
        opts.region.trim(),
        opts.postal_code.trim(),
        opts.country.trim(),
    ];
    if adr.iter().any(|p| !p.is_empty()) {
        push(format!(
            "ADR;TYPE={}:;;{};{};{};{};{}",
            version.type_value("WORK", "work"),
            escape(adr[0]),
            escape(adr[1]),
            escape(adr[2]),
            escape(adr[3]),
            escape(adr[4]),
        ));
    }
    if !birthday.is_empty() {
        push(format!("BDAY:{birthday}"));
    }
    if !opts.note.trim().is_empty() {
        push(format!("NOTE:{}", escape(opts.note.trim())));
    }
    push("END:VCARD".to_string());
    Ok(lines.join("\r\n"))
}

/// Human-readable caption lines printed under the code (not the vCard source —
/// a printed badge wants the details, not the property names).
fn caption_lines(opts: &Options) -> Vec<String> {
    let mut out = Vec::new();
    let name = formatted_name(
        opts.first_name.trim(),
        opts.last_name.trim(),
        opts.organization.trim(),
    );
    if !name.is_empty() {
        out.push(name);
    }
    let title = opts.job_title.trim();
    // Skip the org here when the name line already IS the org (org-only contact).
    let org = match opts.organization.trim() {
        o if o.is_empty() || Some(o) == out.first().map(|s| s.as_str()) => "",
        o => o,
    };
    let role = match (title.is_empty(), org.is_empty()) {
        (false, false) => format!("{title}, {org}"),
        (false, true) => title.to_string(),
        (true, false) => org.to_string(),
        (true, true) => String::new(),
    };
    if !role.is_empty() {
        out.push(role);
    }
    for v in [opts.mobile.trim(), opts.phone.trim(), opts.email.trim()] {
        if !v.is_empty() {
            out.push(v.to_string());
        }
    }
    let website = opts.website.trim();
    if !website.is_empty() {
        out.push(website.to_string());
    }
    out
}

fn wrap_chars(s: &str, width: usize) -> Vec<String> {
    let width = width.max(8);
    let mut lines = Vec::new();
    let mut cur = String::new();
    let mut n = 0usize;
    for c in s.chars() {
        cur.push(c);
        n += 1;
        if n == width {
            lines.push(std::mem::take(&mut cur));
            n = 0;
        }
    }
    if !cur.is_empty() {
        lines.push(cur);
    }
    lines
}

/// Build the vCard and render it as an SVG QR code.
///
/// Returns `(vcard, svg)` — the `.vcf` source and the SVG markup.
pub fn render(opts: &Options) -> Result<(String, String), String> {
    let vcard = build_vcard(opts)?;
    let ecc = Ecc::parse(opts.error_correction)?;
    let fg = check_color(opts.foreground, "foreground")?;
    let bg = check_color(opts.background, "background")?;
    if opts.size < MIN_SIZE || opts.size > MAX_SIZE {
        return Err(format!(
            "size {} is out of range ({MIN_SIZE}-{MAX_SIZE} pixels)",
            opts.size
        ));
    }

    let code = QrCode::with_error_correction_level(vcard.as_bytes(), ecc.level()).map_err(|e| {
        format!(
            "the contact is too long to encode at error correction {:?} ({} bytes of vCard): {e} — \
             shorten the note/address or drop to a lower error-correction level",
            ecc,
            vcard.len()
        )
    })?;
    let width = code.width();
    let colors = code.to_colors();
    let span = width + QUIET_ZONE * 2;

    // One compact path: horizontal runs of dark modules.
    let mut path = String::new();
    for y in 0..width {
        let mut x = 0usize;
        while x < width {
            if colors[y * width + x] == qrcode::Color::Dark {
                let start = x;
                while x < width && colors[y * width + x] == qrcode::Color::Dark {
                    x += 1;
                }
                let run = x - start;
                path.push_str(&format!(
                    "M{} {}h{run}v1h-{run}z",
                    start + QUIET_ZONE,
                    y + QUIET_ZONE
                ));
            } else {
                x += 1;
            }
        }
    }

    // Caption block, measured in module units (monospace ≈ 0.6em per character).
    let font_size = 1.0f64;
    let line_height = 1.5f64;
    let cols = ((span as f64 - 2.0) / (font_size * 0.6)) as usize;
    let caption: Vec<String> = if opts.show_details {
        caption_lines(opts)
            .iter()
            .flat_map(|l| wrap_chars(l, cols))
            .collect()
    } else {
        Vec::new()
    };
    let caption_height = if caption.is_empty() {
        0.0
    } else {
        1.0 + caption.len() as f64 * line_height + 1.0
    };
    let total_height = span as f64 + caption_height;
    let px_width = opts.size;
    let px_height = ((opts.size as f64) * total_height / span as f64).round() as u32;

    let display_name = formatted_name(
        opts.first_name.trim(),
        opts.last_name.trim(),
        opts.organization.trim(),
    );
    let mut svg = String::with_capacity(path.len() + vcard.len() + 1024);
    svg.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{px_width}\" height=\"{px_height}\" \
         viewBox=\"0 0 {span} {total_height:.2}\" role=\"img\" shape-rendering=\"crispEdges\">"
    ));
    svg.push_str(&format!(
        "<title>Contact QR code for {}</title>",
        xml_escape(&display_name)
    ));
    // The exact .vcf source, so the SVG file itself carries the contact data.
    svg.push_str(&format!("<desc>{}</desc>", xml_escape(&vcard)));
    svg.push_str(&format!(
        "<rect width=\"{span}\" height=\"{total_height:.2}\" fill=\"{bg}\"/>"
    ));
    svg.push_str(&format!("<path fill=\"{fg}\" d=\"{path}\"/>"));
    if !caption.is_empty() {
        svg.push_str(&format!(
            "<g data-role=\"contact-caption\" fill=\"{fg}\" font-family=\"ui-monospace, \
             SFMono-Regular, Menlo, Consolas, monospace\" font-size=\"{font_size}\" \
             text-anchor=\"middle\">"
        ));
        for (i, line) in caption.iter().enumerate() {
            let y = span as f64 + 1.0 + (i as f64 + 1.0) * line_height;
            svg.push_str(&format!(
                "<text x=\"{:.2}\" y=\"{y:.2}\">{}</text>",
                span as f64 / 2.0,
                xml_escape(line)
            ));
        }
        svg.push_str("</g>");
    }
    svg.push_str("</svg>");
    Ok((vcard, svg))
}

/// Convenience wrapper for the page/web export: SVG only.
pub fn run(opts: &Options) -> Result<String, String> {
    render(opts).map(|(_, svg)| svg)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample<'a>() -> Options<'a> {
        Options {
            first_name: "Ada",
            last_name: "Lovelace",
            organization: "Analytical Engines",
            job_title: "Chief Analyst",
            mobile: "+44 7700 900123",
            email: "ada@example.com",
            website: "example.com/ada",
            ..Options::default()
        }
    }

    #[test]
    fn builds_a_minimal_vcard_exactly() {
        let vcard = build_vcard(&Options {
            first_name: "John",
            last_name: "Doe",
            phone: "555-1234",
            ..Options::default()
        })
        .unwrap();
        assert_eq!(
            vcard,
            "BEGIN:VCARD\r\nVERSION:3.0\r\nN:Doe;John;;;\r\nFN:John Doe\r\n\
             TEL;TYPE=WORK,VOICE:555-1234\r\nEND:VCARD"
        );
    }

    /// Reverse RFC 6350 §3.2 folding, the way an importer does, so assertions can
    /// be written against logical lines.
    fn unfold(vcard: &str) -> String {
        vcard.replace("\r\n ", "")
    }

    #[test]
    fn full_card_carries_every_property() {
        let vcard = build_vcard(&Options {
            street: "12 Baker Street",
            city: "London",
            region: "Greater London",
            postal_code: "NW1 6XE",
            country: "United Kingdom",
            note: "Met at the 2026 expo",
            birthday: "1815-12-10",
            phone: "+44 20 7946 0000",
            ..sample()
        })
        .unwrap();
        assert!(vcard.contains("ORG:Analytical Engines"));
        assert!(vcard.contains("TITLE:Chief Analyst"));
        assert!(vcard.contains("TEL;TYPE=CELL:+44 7700 900123"));
        assert!(vcard.contains("TEL;TYPE=WORK,VOICE:+44 20 7946 0000"));
        assert!(vcard.contains("EMAIL;TYPE=INTERNET:ada@example.com"));
        assert!(vcard.contains("URL:https://example.com/ada"));
        // Long enough to be folded on the wire — assert the logical line.
        assert!(unfold(&vcard).contains(
            "ADR;TYPE=WORK:;;12 Baker Street;London;Greater London;NW1 6XE;United Kingdom"
        ));
        assert!(vcard.contains("BDAY:1815-12-10"));
        assert!(vcard.contains("NOTE:Met at the 2026 expo"));
        assert!(vcard.ends_with("END:VCARD"));
    }

    #[test]
    fn version_4_uses_lowercase_type_values() {
        let vcard = build_vcard(&Options {
            version: "4.0",
            city: "Paris",
            ..sample()
        })
        .unwrap();
        assert!(vcard.contains("VERSION:4.0"));
        assert!(vcard.contains("TEL;TYPE=cell:"));
        assert!(vcard.contains("EMAIL;TYPE=internet:"));
        assert!(vcard.contains("ADR;TYPE=work:;;;Paris;;;"));
    }

    #[test]
    fn organization_only_contact_gets_an_fn() {
        let vcard = build_vcard(&Options {
            organization: "Analytical Engines",
            email: "hello@example.com",
            ..Options::default()
        })
        .unwrap();
        assert!(vcard.contains("FN:Analytical Engines"));
        assert!(vcard.contains("N:;;;;"));
    }

    #[test]
    fn special_characters_are_escaped() {
        let vcard = build_vcard(&Options {
            organization: "Smith, Jones & Co",
            note: "floor 3; ring twice\\then wait",
            ..sample()
        })
        .unwrap();
        assert!(vcard.contains("ORG:Smith\\, Jones & Co"));
        assert!(vcard.contains("NOTE:floor 3\\; ring twice\\\\then wait"));
    }

    #[test]
    fn long_values_are_folded_at_75_octets() {
        let note = "x".repeat(200);
        let vcard = build_vcard(&Options {
            note: &note,
            ..sample()
        })
        .unwrap();
        let note_line = vcard
            .split("\r\n")
            .skip_while(|l| !l.starts_with("NOTE:"))
            .take(1)
            .next()
            .unwrap();
        assert!(note_line.len() <= 75, "unfolded: {}", note_line.len());
        assert!(vcard.contains("\r\n x"), "expected a continuation line");
    }

    #[test]
    fn a_nameless_contact_is_rejected() {
        let err = build_vcard(&Options {
            email: "nobody@example.com",
            ..Options::default()
        })
        .unwrap_err();
        assert!(err.contains("needs a name"), "got {err}");
    }

    #[test]
    fn a_bad_email_is_rejected() {
        let err = build_vcard(&Options {
            email: "ada(at)example.com",
            ..sample()
        })
        .unwrap_err();
        assert!(err.contains("not a valid address"), "got {err}");
    }

    #[test]
    fn birthday_accepts_both_iso_forms_and_rejects_impossible_dates() {
        assert!(build_vcard(&Options {
            birthday: "19871231",
            ..sample()
        })
        .unwrap()
        .contains("BDAY:1987-12-31"));
        assert!(build_vcard(&Options {
            birthday: "2024-02-29",
            ..sample()
        })
        .unwrap()
        .contains("BDAY:2024-02-29"));
        let err = build_vcard(&Options {
            birthday: "2023-02-29",
            ..sample()
        })
        .unwrap_err();
        assert!(err.contains("28 days"), "got {err}");
        let err = build_vcard(&Options {
            birthday: "23 April 1987",
            ..sample()
        })
        .unwrap_err();
        assert!(err.contains("not a date"), "got {err}");
    }

    #[test]
    fn website_keeps_an_explicit_scheme_and_adds_https_otherwise() {
        assert!(build_vcard(&Options {
            website: "http://example.org",
            ..sample()
        })
        .unwrap()
        .contains("URL:http://example.org"));
        assert!(build_vcard(&Options {
            website: "example.org",
            ..sample()
        })
        .unwrap()
        .contains("URL:https://example.org"));
    }

    #[test]
    fn unknown_version_is_rejected() {
        let err = build_vcard(&Options {
            version: "2.1",
            ..sample()
        })
        .unwrap_err();
        assert!(err.contains("unknown version"), "got {err}");
    }

    #[test]
    fn render_emits_svg_with_title_desc_and_caption() {
        let (vcard, svg) = render(&sample()).unwrap();
        assert!(svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\""));
        assert!(svg.ends_with("</svg>"));
        assert!(svg.contains("<title>Contact QR code for Ada Lovelace</title>"));
        assert!(svg.contains(&xml_escape(&vcard)));
        assert!(svg.contains("data-role=\"contact-caption\""));
        assert!(svg.contains("Ada Lovelace"));
        assert!(svg.contains("Chief Analyst, Analytical Engines"));
        assert!(svg.contains("width=\"512\""));
        assert!(svg.contains("fill=\"#000000\""));
        assert!(svg.contains("fill=\"#ffffff\""));
    }

    #[test]
    fn show_details_false_drops_the_caption_and_squares_the_svg() {
        let (_, svg) = render(&Options {
            show_details: false,
            size: 300,
            ..sample()
        })
        .unwrap();
        assert!(!svg.contains("contact-caption"));
        assert!(
            svg.contains("width=\"300\" height=\"300\""),
            "got {svg:.200}"
        );
    }

    #[test]
    fn every_error_correction_level_renders() {
        for level in ["L", "M", "Q", "H"] {
            let (_, svg) = render(&Options {
                error_correction: level,
                ..sample()
            })
            .unwrap();
            assert!(svg.contains("<path fill="), "level {level}");
        }
    }

    #[test]
    fn colors_accept_hex_and_names_but_reject_junk() {
        let (_, svg) = render(&Options {
            foreground: "#1a3fd0",
            background: "transparent",
            ..sample()
        })
        .unwrap();
        assert!(svg.contains("fill=\"#1a3fd0\""));
        assert!(svg.contains("fill=\"transparent\""));
        let err = render(&Options {
            foreground: "#12345",
            ..sample()
        })
        .unwrap_err();
        assert!(err.contains("not a valid hex colour"), "got {err}");
    }

    #[test]
    fn size_bounds_are_enforced() {
        assert!(render(&Options {
            size: MIN_SIZE,
            ..sample()
        })
        .is_ok());
        assert!(render(&Options {
            size: MAX_SIZE,
            ..sample()
        })
        .is_ok());
        let err = render(&Options {
            size: MIN_SIZE - 1,
            ..sample()
        })
        .unwrap_err();
        assert!(err.contains("out of range"), "got {err}");
    }

    #[test]
    fn an_over_capacity_contact_fails_with_an_actionable_error() {
        let note = "n".repeat(4000);
        let err = render(&Options {
            note: &note,
            error_correction: "H",
            ..sample()
        })
        .unwrap_err();
        assert!(err.contains("too long to encode"), "got {err}");
    }

    #[test]
    fn xml_special_characters_do_not_break_the_svg() {
        let (_, svg) = render(&Options {
            organization: "Smith & <Sons>",
            ..sample()
        })
        .unwrap();
        assert!(svg.contains("&amp;"));
        assert!(!svg.contains("<Sons>"));
    }
}
