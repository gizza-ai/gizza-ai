//! pem-der-convert core — convert cryptographic objects (keys, certs, CSRs)
//! between PEM (base64 text wrapped in `-----BEGIN <label>-----`) and DER
//! (the raw binary ASN.1 DER bytes) in either direction. Pure Rust, no
//! wafer/wasm-bindgen deps.
//!
//! PEM is, structurally, just base64-of-DER with a labelled armor; this tool is
//! a *generic* re-encoder — it does not parse or validate the inner ASN.1, so
//! it works for any object (RSA/EC keys, X.509 certs, CSRs, CRLs, ...). DER is
//! binary, so on the way out it is shown as hex or base64, and on the way in it
//! is accepted as hex or base64.

use base64::engine::general_purpose::STANDARD as B64;
use base64::Engine;

/// Which direction to convert.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Direction {
    /// Detect from the input: PEM (has `-----BEGIN`) → DER, otherwise DER → PEM.
    Auto,
    /// PEM text in → DER bytes out (rendered as hex / base64).
    PemToDer,
    /// DER bytes in (hex / base64) → PEM text out.
    DerToPem,
}

/// How DER binary is represented as text (DER cannot be printed raw).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DerFormat {
    Hex,
    Base64,
}

pub fn parse_direction(s: &str) -> Result<Direction, String> {
    match s.trim().to_ascii_lowercase().replace('_', "-").as_str() {
        "auto" | "" => Ok(Direction::Auto),
        "pem-to-der" | "pem2der" => Ok(Direction::PemToDer),
        "der-to-pem" | "der2pem" => Ok(Direction::DerToPem),
        other => Err(format!(
            "unknown direction '{other}'. Use 'auto', 'pem-to-der' or 'der-to-pem'."
        )),
    }
}

pub fn parse_der_format(s: &str) -> Result<DerFormat, String> {
    match s.trim().to_ascii_lowercase().as_str() {
        "hex" | "" => Ok(DerFormat::Hex),
        "base64" | "b64" => Ok(DerFormat::Base64),
        other => Err(format!(
            "unknown der_format '{other}'. Use 'hex' or 'base64'."
        )),
    }
}

/// Lowercase hex string of `bytes`, no separators.
fn to_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

/// Parse a hex string, ignoring all ASCII whitespace and an optional `0x`
/// prefix and common separators (`:`, `-`, spaces, newlines).
fn from_hex(s: &str) -> Result<Vec<u8>, String> {
    let cleaned: String = s
        .chars()
        .filter(|c| !c.is_ascii_whitespace() && *c != ':' && *c != '-')
        .collect();
    let cleaned = cleaned
        .strip_prefix("0x")
        .or_else(|| cleaned.strip_prefix("0X"))
        .unwrap_or(&cleaned);
    if cleaned.is_empty() {
        return Err("no hex bytes found in input".into());
    }
    if cleaned.len() % 2 != 0 {
        return Err("hex input has an odd number of digits".into());
    }
    let mut out = Vec::with_capacity(cleaned.len() / 2);
    let bytes = cleaned.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        let hi = (bytes[i] as char)
            .to_digit(16)
            .ok_or_else(|| format!("invalid hex digit '{}'", bytes[i] as char))?;
        let lo = (bytes[i + 1] as char)
            .to_digit(16)
            .ok_or_else(|| format!("invalid hex digit '{}'", bytes[i + 1] as char))?;
        out.push(((hi << 4) | lo) as u8);
        i += 2;
    }
    Ok(out)
}

/// Decode a base64 string, ignoring all ASCII whitespace.
fn from_base64(s: &str) -> Result<Vec<u8>, String> {
    let cleaned: String = s.chars().filter(|c| !c.is_ascii_whitespace()).collect();
    if cleaned.is_empty() {
        return Err("no base64 data found in input".into());
    }
    B64.decode(cleaned.as_bytes())
        .map_err(|e| format!("invalid base64 DER input: {e}"))
}

/// Convert a PEM string into DER bytes, rendered in `fmt`. The result is a
/// `(label, rendered_der)` pair so the caller can report the detected label.
pub fn pem_to_der(input: &str, fmt: DerFormat) -> Result<(String, String), String> {
    let input = input.trim();
    if input.is_empty() {
        return Err("no PEM input — paste a PEM block (-----BEGIN ...-----)".into());
    }
    if !input.contains("-----BEGIN") {
        return Err(
            "input does not look like PEM (no '-----BEGIN ...-----' header). To go the other way, set direction to der-to-pem.".into(),
        );
    }
    let block = pem::parse(input.as_bytes()).map_err(|e| format!("not valid PEM: {e}"))?;
    let label = block.tag().to_string();
    let der = block.contents();
    let rendered = match fmt {
        DerFormat::Hex => to_hex(der),
        DerFormat::Base64 => B64.encode(der),
    };
    Ok((label, rendered))
}

/// Parse one OR MORE PEM blocks (e.g. a certificate chain) into DER. Returns a
/// `(label, rendered_der)` pair per block, in input order.
pub fn pem_to_der_all(input: &str, fmt: DerFormat) -> Result<Vec<(String, String)>, String> {
    let input = input.trim();
    if input.is_empty() {
        return Err("no PEM input — paste a PEM block (-----BEGIN ...-----)".into());
    }
    if !input.contains("-----BEGIN") {
        return Err(
            "input does not look like PEM (no '-----BEGIN ...-----' header). To go the other way, set direction to der-to-pem.".into(),
        );
    }
    let blocks = pem::parse_many(input.as_bytes()).map_err(|e| format!("not valid PEM: {e}"))?;
    if blocks.is_empty() {
        return Err("no PEM blocks found in input".into());
    }
    Ok(blocks
        .into_iter()
        .map(|b| {
            let der = b.contents();
            let rendered = match fmt {
                DerFormat::Hex => to_hex(der),
                DerFormat::Base64 => B64.encode(der),
            };
            (b.tag().to_string(), rendered)
        })
        .collect())
}

/// Convert DER bytes (given in `fmt`) into a PEM block with the supplied
/// `label`. The label is normalised (uppercased, surrounding `-----BEGIN/END`
/// stripped if a user pasted them); defaults to "CERTIFICATE" if blank.
pub fn der_to_pem(input: &str, fmt: DerFormat, label: &str) -> Result<String, String> {
    if input.trim().is_empty() {
        return Err("no DER input — paste the DER bytes as hex or base64".into());
    }
    let der = match fmt {
        DerFormat::Hex => from_hex(input)?,
        DerFormat::Base64 => from_base64(input)?,
    };
    if der.is_empty() {
        return Err("DER input decoded to zero bytes".into());
    }
    let label = normalise_label(label);
    // pem 3.x: build a Pem and encode with standard 64-col line wrapping + LF.
    let block = pem::Pem::new(label, der);
    let conf = pem::EncodeConfig::new().set_line_ending(pem::LineEnding::LF);
    Ok(pem::encode_config(&block, conf))
}

/// Normalise a user-supplied PEM label: strip any `-----BEGIN/END ...-----`
/// armor they may have pasted, uppercase it, and fall back to "CERTIFICATE".
fn normalise_label(label: &str) -> String {
    let mut l = label.trim().to_string();
    // If they pasted a full armor line, pull the label out of it.
    if let Some(rest) = l.strip_prefix("-----BEGIN") {
        l = rest.trim_start().trim_end_matches('-').trim().to_string();
    } else if let Some(rest) = l.strip_prefix("-----END") {
        l = rest.trim_start().trim_end_matches('-').trim().to_string();
    }
    let l = l.trim_matches('-').trim().to_uppercase();
    if l.is_empty() {
        "CERTIFICATE".to_string()
    } else {
        l
    }
}

/// Top-level: dispatch on direction. Returns the converted text. For
/// `pem-to-der`, the output is prefixed with a `# label: <LABEL>` comment line
/// so the detected object type is visible.
pub fn run(input: &str, dir: Direction, fmt: DerFormat, label: &str) -> Result<String, String> {
    // Resolve Auto by sniffing the input for PEM armor.
    let dir = match dir {
        Direction::Auto => {
            if input.contains("-----BEGIN") {
                Direction::PemToDer
            } else {
                Direction::DerToPem
            }
        }
        other => other,
    };
    match dir {
        Direction::PemToDer => {
            let blocks = pem_to_der_all(input, fmt)?;
            let kind = match fmt {
                DerFormat::Hex => "hex",
                DerFormat::Base64 => "base64",
            };
            let multi = blocks.len() > 1;
            let mut out = String::new();
            for (i, (lbl, der)) in blocks.iter().enumerate() {
                if multi {
                    out.push_str(&format!("# block {} of {}\n", i + 1, blocks.len()));
                }
                out.push_str(&format!(
                    "# label: {lbl}\n# DER ({kind}, {len} bytes):\n{der}\n",
                    len = der_byte_len(der, fmt)
                ));
            }
            Ok(out.trim_end().to_string())
        }
        Direction::DerToPem => der_to_pem(input, fmt, label),
        Direction::Auto => unreachable!("Auto resolved above"),
    }
}

/// Length in bytes of a rendered DER string (for the informational header).
fn der_byte_len(rendered: &str, fmt: DerFormat) -> usize {
    match fmt {
        DerFormat::Hex => rendered.len() / 2,
        DerFormat::Base64 => B64.decode(rendered.as_bytes()).map(|v| v.len()).unwrap_or(0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // A tiny, valid self-contained PEM (an EC P-256 PUBLIC KEY / SPKI).
    const PEM: &str = "-----BEGIN PUBLIC KEY-----\nMFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAEqIfBMr3yXVH0Nj5aJ7+iZ5l0t3pN\nm0nT5tWqI2pLZ0r0i7p8s3wV3cI3VxqJ2sZ5l4tWqI2pLZ0r0i7p8s3wV==\n-----END PUBLIC KEY-----";

    #[test]
    fn pem_to_der_hex_roundtrips_back_to_pem() {
        // Use a real, parseable PEM: base64 of arbitrary bytes.
        let der_bytes: Vec<u8> = (0u8..40).collect();
        let pem_text = der_to_pem(&to_hex(&der_bytes), DerFormat::Hex, "PRIVATE KEY").unwrap();
        assert!(pem_text.starts_with("-----BEGIN PRIVATE KEY-----"));
        assert!(pem_text.contains("-----END PRIVATE KEY-----"));

        // Now PEM -> DER (hex) should recover the exact bytes + label.
        let (label, hex) = pem_to_der(&pem_text, DerFormat::Hex).unwrap();
        assert_eq!(label, "PRIVATE KEY");
        assert_eq!(from_hex(&hex).unwrap(), der_bytes);
    }

    #[test]
    fn pem_to_der_base64_recovers_bytes() {
        let der_bytes: Vec<u8> = vec![0xDE, 0xAD, 0xBE, 0xEF, 0x00, 0xFF];
        let pem_text = der_to_pem(&B64.encode(&der_bytes), DerFormat::Base64, "certificate").unwrap();
        let (label, b64) = pem_to_der(&pem_text, DerFormat::Base64).unwrap();
        assert_eq!(label, "CERTIFICATE"); // normalised to uppercase
        assert_eq!(from_base64(&b64).unwrap(), der_bytes);
    }

    #[test]
    fn auto_detects_pem_to_der_and_der_to_pem() {
        let der_bytes: Vec<u8> = vec![9, 8, 7, 6];
        let pem_text = der_to_pem(&to_hex(&der_bytes), DerFormat::Hex, "CERTIFICATE").unwrap();
        // PEM input under Auto → DER out.
        let out = run(&pem_text, Direction::Auto, DerFormat::Hex, "").unwrap();
        assert!(out.contains("09080706"));
        // Hex input under Auto → PEM out.
        let out2 = run("09080706", Direction::Auto, DerFormat::Hex, "PRIVATE KEY").unwrap();
        assert!(out2.starts_with("-----BEGIN PRIVATE KEY-----"));
    }

    #[test]
    fn pem_to_der_handles_multiple_blocks() {
        let a = der_to_pem(&to_hex(&[1u8, 2, 3]), DerFormat::Hex, "CERTIFICATE").unwrap();
        let b = der_to_pem(&to_hex(&[4u8, 5, 6]), DerFormat::Hex, "CERTIFICATE").unwrap();
        let chain = format!("{a}{b}");
        let blocks = pem_to_der_all(&chain, DerFormat::Hex).unwrap();
        assert_eq!(blocks.len(), 2);
        assert_eq!(blocks[0].1, "010203");
        assert_eq!(blocks[1].1, "040506");
        let out = run(&chain, Direction::PemToDer, DerFormat::Hex, "").unwrap();
        assert!(out.contains("block 1 of 2"));
        assert!(out.contains("block 2 of 2"));
    }

    #[test]
    fn der_to_pem_normalises_pasted_armor_label() {
        let der_bytes: Vec<u8> = vec![1, 2, 3, 4];
        let pem_text =
            der_to_pem(&to_hex(&der_bytes), DerFormat::Hex, "-----BEGIN EC PRIVATE KEY-----").unwrap();
        assert!(pem_text.starts_with("-----BEGIN EC PRIVATE KEY-----"));
    }

    #[test]
    fn der_to_pem_defaults_label_when_blank() {
        let pem_text = der_to_pem("01020304", DerFormat::Hex, "").unwrap();
        assert!(pem_text.starts_with("-----BEGIN CERTIFICATE-----"));
    }

    #[test]
    fn hex_accepts_separators_and_prefix() {
        assert_eq!(from_hex("0xDE:AD BE-EF").unwrap(), vec![0xDE, 0xAD, 0xBE, 0xEF]);
    }

    #[test]
    fn run_pem_to_der_includes_label_header() {
        let der_bytes: Vec<u8> = vec![10, 20, 30];
        let pem_text = der_to_pem(&to_hex(&der_bytes), DerFormat::Hex, "PUBLIC KEY").unwrap();
        let out = run(&pem_text, Direction::PemToDer, DerFormat::Hex, "").unwrap();
        assert!(out.contains("# label: PUBLIC KEY"));
        assert!(out.contains("0a141e"));
    }

    #[test]
    fn pem_to_der_rejects_non_pem() {
        let err = pem_to_der("just some text", DerFormat::Hex).unwrap_err();
        assert!(err.contains("BEGIN"));
    }

    #[test]
    fn der_to_pem_rejects_odd_hex() {
        let err = der_to_pem("abc", DerFormat::Hex, "CERTIFICATE").unwrap_err();
        assert!(err.contains("odd"));
    }

    #[test]
    fn parse_helpers() {
        assert_eq!(parse_direction("pem-to-der").unwrap(), Direction::PemToDer);
        assert_eq!(parse_direction("der2pem").unwrap(), Direction::DerToPem);
        assert!(parse_direction("nope").is_err());
        assert_eq!(parse_der_format("base64").unwrap(), DerFormat::Base64);
        assert_eq!(parse_der_format("hex").unwrap(), DerFormat::Hex);
        assert!(parse_der_format("xyz").is_err());
    }

    #[test]
    fn unused_const_pem_is_referenced() {
        // keep PEM const exercised so it's not dead; it's a realistic shape.
        assert!(PEM.contains("PUBLIC KEY"));
    }
}
