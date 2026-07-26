//! gizza-ai/font-subset core — reduce an OpenType font to the glyphs needed
//! for a supplied text sample. Pure Rust; byte-slice in, byte-vector out.

use std::collections::BTreeSet;

use font_subset::Font;

/// Output container for the subset font.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OutputFormat {
    /// Raw OpenType/SFNT bytes (TTF or OTF flavor preserved by the input font).
    OpenType,
    /// WOFF2 webfont container.
    Woff2,
}

impl OutputFormat {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "opentype" | "otf" | "ttf" | "sfnt" => Ok(Self::OpenType),
            "woff2" => Ok(Self::Woff2),
            other => Err(format!(
                "unknown output format {other:?}; expected opentype or woff2"
            )),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::OpenType => "OpenType",
            Self::Woff2 => "WOFF2",
        }
    }

    pub fn mime(self) -> &'static str {
        match self {
            Self::OpenType => "font/otf",
            Self::Woff2 => "font/woff2",
        }
    }

    pub fn ext(self) -> &'static str {
        match self {
            Self::OpenType => "otf",
            Self::Woff2 => "woff2",
        }
    }
}

/// Result metadata for callers.
#[derive(Debug)]
pub struct SubsetResult {
    pub bytes: Vec<u8>,
    pub input_size: usize,
    pub output_size: usize,
    pub input_glyphs: usize,
    pub kept_chars: usize,
    pub missing_chars: Vec<char>,
    pub format: OutputFormat,
}

/// Subset an OpenType (TTF/OTF SFNT) font to the glyphs needed for `text`.
///
/// WOFF/WOFF2 inputs should be converted to OpenType first (for example with
/// the sibling `woff2-convert` tool). Font collections are not supported.
pub fn subset(
    bytes: &[u8],
    text: &str,
    format: OutputFormat,
    drop_variations: bool,
) -> Result<SubsetResult, String> {
    if bytes.is_empty() {
        return Err("font input is empty".to_string());
    }
    let chars: BTreeSet<char> = text.chars().filter(|c| !c.is_control()).collect();
    if chars.is_empty() {
        return Err("text must contain at least one printable character".to_string());
    }

    let mut font =
        Font::opentype(bytes).map_err(|e| format!("could not parse OpenType font: {e:?}"))?;
    if drop_variations {
        font.drop_variation();
    }
    let input_glyphs = font.glyph_count();
    let missing_chars: Vec<char> = chars
        .iter()
        .copied()
        .filter(|ch| !font.contains_char(*ch))
        .collect();
    if missing_chars.len() == chars.len() {
        return Err("none of the requested text characters are covered by this font".to_string());
    }

    let subset = font
        .subset(&chars)
        .map_err(|e| format!("could not subset font: {e:?}"))?;
    let out = match format {
        OutputFormat::OpenType => subset.to_opentype(),
        OutputFormat::Woff2 => subset.to_woff2(),
    };
    if out.is_empty() {
        return Err("subset serialization produced no bytes".to_string());
    }

    Ok(SubsetResult {
        input_size: bytes.len(),
        output_size: out.len(),
        input_glyphs,
        kept_chars: chars.len().saturating_sub(missing_chars.len()),
        missing_chars,
        bytes: out,
        format,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_TTF: &[u8] = include_bytes!("../tests/fixtures/sample.ttf");

    #[test]
    fn subsets_ttf_to_smaller_woff2() {
        let res = subset(SAMPLE_TTF, "Hello web!", OutputFormat::Woff2, false).unwrap();
        assert!(
            res.output_size < res.input_size,
            "{} !< {}",
            res.output_size,
            res.input_size
        );
        assert!(res.bytes.starts_with(b"wOF2"));
        assert!(res.kept_chars >= 1);
        assert!(res.input_glyphs > res.kept_chars);
    }

    #[test]
    fn subsets_to_parseable_opentype() {
        let res = subset(SAMPLE_TTF, "ABC", OutputFormat::OpenType, false).unwrap();
        let parsed = Font::opentype(&res.bytes).unwrap();
        assert!(parsed.contains_char('A'));
        assert!(parsed.glyph_count() < res.input_glyphs);
    }

    #[test]
    fn rejects_empty_text_and_bad_font() {
        assert!(subset(SAMPLE_TTF, "\n\t", OutputFormat::Woff2, false).is_err());
        assert!(subset(b"not a font", "abc", OutputFormat::Woff2, false).is_err());
    }
}
