//! gizza-ai/xml-formatter core — pretty-print or minify XML, with a
//! well-formedness check. Pure-Rust (`quick-xml`). No wafer/wasm-bindgen deps.

use quick_xml::events::Event;
use quick_xml::reader::Reader;
use quick_xml::writer::Writer;

/// Output mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Pretty,
    Minify,
}

impl Mode {
    pub fn parse(s: &str) -> Result<Mode, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "pretty" | "" => Ok(Mode::Pretty),
            "minify" | "min" | "compact" => Ok(Mode::Minify),
            other => Err(format!("unknown mode '{other}' (use 'pretty' or 'minify')")),
        }
    }
}

/// Format `xml`. `indent` is the number of spaces per level (pretty mode only).
/// Returns an error describing the position if the XML is not well-formed.
pub fn format(xml: &str, mode: Mode, indent: usize) -> Result<String, String> {
    if xml.trim().is_empty() {
        return Err("no XML input".into());
    }
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut writer = match mode {
        Mode::Pretty => Writer::new_with_indent(Vec::new(), b' ', indent.clamp(0, 16)),
        Mode::Minify => Writer::new(Vec::new()),
    };

    loop {
        match reader.read_event() {
            Ok(Event::Eof) => break,
            Ok(event) => {
                writer
                    .write_event(event)
                    .map_err(|e| format!("failed to write XML: {e}"))?;
            }
            Err(e) => {
                return Err(format!(
                    "XML is not well-formed at byte {}: {e}",
                    reader.error_position()
                ));
            }
        }
    }

    String::from_utf8(writer.into_inner())
        .map_err(|e| format!("output is not valid UTF-8: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pretty_prints() {
        let out = format("<a><b>hi</b><c/></a>", Mode::Pretty, 2).unwrap();
        assert_eq!(out, "<a>\n  <b>hi</b>\n  <c/>\n</a>");
    }

    #[test]
    fn custom_indent() {
        let out = format("<a><b>x</b></a>", Mode::Pretty, 4).unwrap();
        assert_eq!(out, "<a>\n    <b>x</b>\n</a>");
    }

    #[test]
    fn minifies() {
        let pretty = "<a>\n  <b>hi</b>\n  <c/>\n</a>";
        let out = format(pretty, Mode::Minify, 2).unwrap();
        assert_eq!(out, "<a><b>hi</b><c/></a>");
    }

    #[test]
    fn preserves_attributes() {
        let out = format(r#"<root id="1"><item k="v"/></root>"#, Mode::Pretty, 2).unwrap();
        assert!(out.contains(r#"<root id="1">"#));
        assert!(out.contains(r#"<item k="v"/>"#));
    }

    #[test]
    fn rejects_malformed() {
        assert!(format("<a><b></a>", Mode::Pretty, 2).is_err()); // mismatched tags
        assert!(format("not xml at all <", Mode::Pretty, 2).is_err());
        assert!(format("", Mode::Pretty, 2).is_err());
    }
}
