//! qr-paper-backup core — split text or encoded bytes into numbered QR-code
//! payloads and render a deterministic printable SVG sheet.

use base64::{engine::general_purpose::STANDARD, Engine as _};
use qrcode::render::svg;
use qrcode::{EcLevel, QrCode};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputEncoding {
    Text,
    Base64,
    Hex,
}

impl InputEncoding {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "text" | "utf8" | "utf-8" | "" => Ok(Self::Text),
            "base64" | "b64" => Ok(Self::Base64),
            "hex" | "base16" => Ok(Self::Hex),
            other => Err(format!(
                "unknown input_encoding '{other}' (use text, base64, or hex)"
            )),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ecc {
    Low,
    Medium,
    Quartile,
    High,
}

impl Ecc {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "l" | "low" => Ok(Self::Low),
            "m" | "medium" | "" => Ok(Self::Medium),
            "q" | "quartile" => Ok(Self::Quartile),
            "h" | "high" => Ok(Self::High),
            other => Err(format!(
                "unknown error_correction '{other}' (use L, M, Q, or H)"
            )),
        }
    }

    fn level(self) -> EcLevel {
        match self {
            Self::Low => EcLevel::L,
            Self::Medium => EcLevel::M,
            Self::Quartile => EcLevel::Q,
            Self::High => EcLevel::H,
        }
    }
}

pub struct Options {
    pub input_encoding: InputEncoding,
    pub chunk_bytes: usize,
    pub columns: usize,
    pub error_correction: Ecc,
    pub show_text: bool,
}

pub fn decode_input(input: &str, enc: InputEncoding) -> Result<Vec<u8>, String> {
    match enc {
        InputEncoding::Text => Ok(input.as_bytes().to_vec()),
        InputEncoding::Base64 => STANDARD
            .decode(input.split_whitespace().collect::<String>())
            .map_err(|e| format!("invalid base64 input: {e}")),
        InputEncoding::Hex => decode_hex(input),
    }
}

fn decode_hex(input: &str) -> Result<Vec<u8>, String> {
    let clean: String = input.chars().filter(|c| !c.is_whitespace()).collect();
    if clean.len() % 2 != 0 {
        return Err("hex input must have an even number of digits".into());
    }
    let mut out = Vec::with_capacity(clean.len() / 2);
    for i in (0..clean.len()).step_by(2) {
        let byte = u8::from_str_radix(&clean[i..i + 2], 16)
            .map_err(|_| format!("invalid hex byte '{}': use 0-9/a-f", &clean[i..i + 2]))?;
        out.push(byte);
    }
    Ok(out)
}

fn escape_text(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn svg_data_url(svg: &str) -> String {
    format!(
        "data:image/svg+xml;base64,{}",
        STANDARD.encode(svg.as_bytes())
    )
}

pub fn build_lines(data: &[u8], chunk_bytes: usize) -> Result<Vec<String>, String> {
    if data.is_empty() {
        return Err("input is empty".into());
    }
    let chunk_bytes = chunk_bytes.clamp(50, 1200);
    let mut hasher = Sha256::new();
    hasher.update(data);
    let hash = hasher.finalize();
    let id = hash[..4]
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let total = data.len().div_ceil(chunk_bytes);
    Ok(data
        .chunks(chunk_bytes)
        .enumerate()
        .map(|(i, chunk)| format!("QRB1|{}|{}|{}|{}", i + 1, total, id, STANDARD.encode(chunk)))
        .collect())
}

pub fn render_sheet(input: &str, opts: Options) -> Result<String, String> {
    let data = decode_input(input, opts.input_encoding)?;
    let lines = build_lines(&data, opts.chunk_bytes)?;
    let columns = opts.columns.clamp(1, 5);
    let cell_w = 260usize;
    let qr = 180usize;
    let caption_h = if opts.show_text { 74 } else { 36 };
    let header_h = 98usize;
    let rows = lines.len().div_ceil(columns);
    let width = columns * cell_w;
    let height = header_h + rows * (qr + caption_h) + 24;

    let mut out = String::new();
    out.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\">\n<rect width=\"100%\" height=\"100%\" fill=\"white\"/>\n"
    ));
    out.push_str("<style>text{font-family:ui-monospace,SFMono-Regular,Menlo,Consolas,monospace;fill:#111}.small{font-size:10px}.title{font-size:18px;font-weight:700}.body{font-size:12px}</style>\n");
    out.push_str(&format!(
        "<text x=\"18\" y=\"28\" class=\"title\">QR paper backup</text>\n<text x=\"18\" y=\"50\" class=\"body\">{} bytes split into {} QR codes. Format: QRB1|index|total|id|base64-chunk.</text>\n<text x=\"18\" y=\"70\" class=\"body\">Restore by scanning all parts, sorting by index, concatenating base64 chunks, then Base64-decoding.</text>\n",
        data.len(),
        lines.len()
    ));

    for (i, line) in lines.iter().enumerate() {
        let col = i % columns;
        let row = i / columns;
        let x = col * cell_w + 20;
        let y = header_h + row * (qr + caption_h);
        let code =
            QrCode::with_error_correction_level(line.as_bytes(), opts.error_correction.level())
                .map_err(|e| format!("failed to encode QR part {}: {e}", i + 1))?;
        let qsvg = code
            .render::<svg::Color>()
            .min_dimensions(qr as u32, qr as u32)
            .quiet_zone(true)
            .dark_color(svg::Color("#000000"))
            .light_color(svg::Color("#ffffff"))
            .build();
        out.push_str(&format!(
            "<g transform=\"translate({x},{y})\"><image href=\"{}\" width=\"{qr}\" height=\"{qr}\"/>\n<text x=\"0\" y=\"{}\" class=\"body\">Part {} / {}</text>\n",
            svg_data_url(&qsvg),
            qr + 18,
            i + 1,
            lines.len()
        ));
        if opts.show_text {
            out.push_str(&format!(
                "<text x=\"0\" y=\"{}\" class=\"small\">{}</text>\n",
                qr + 36,
                escape_text(line)
            ));
        }
        out.push_str("</g>\n");
    }
    out.push_str("</svg>");
    Ok(out)
}

pub fn run(
    input: &str,
    input_encoding: &str,
    chunk_bytes: u32,
    columns: u32,
    error_correction: &str,
    show_text: bool,
) -> Result<String, String> {
    render_sheet(
        input,
        Options {
            input_encoding: InputEncoding::parse(input_encoding)?,
            chunk_bytes: if chunk_bytes == 0 {
                300
            } else {
                chunk_bytes as usize
            },
            columns: if columns == 0 { 3 } else { columns as usize },
            error_correction: Ecc::parse(error_correction)?,
            show_text,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_numbered_payload_lines() {
        let data = vec![b'a'; 101];
        let lines = build_lines(&data, 50).unwrap();
        assert_eq!(lines.len(), 3);
        assert!(lines[0].starts_with("QRB1|1|3|"));
        assert!(lines[2].ends_with("|YQ=="));
    }

    #[test]
    fn decodes_hex_and_base64() {
        assert_eq!(decode_input("6869", InputEncoding::Hex).unwrap(), b"hi");
        assert_eq!(decode_input("aGk=", InputEncoding::Base64).unwrap(), b"hi");
        assert!(decode_input("abc", InputEncoding::Hex).is_err());
    }

    #[test]
    fn renders_svg_sheet() {
        let svg = run("hello", "text", 300, 2, "m", true).unwrap();
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("QR paper backup"));
        assert!(svg.contains("Part 1 / 1"));
        assert!(svg.contains("QRB1|1|1|"));
    }

    #[test]
    fn chunks_and_hides_payload_text() {
        let input = "a".repeat(101);
        let svg = run(&input, "text", 50, 1, "h", false).unwrap();
        assert!(svg.contains("Part 3 / 3"));
        assert!(!svg.contains("QRB1|1|3|"));
    }
}
