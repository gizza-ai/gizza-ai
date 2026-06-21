//! gizza-ai/text-to-pdf core — generate a clean, paginated PDF from plain text.
//! Pure-Rust (`lopdf`), no font embedding: uses the built-in Courier (monospace)
//! Type1 font so layout is deterministic and the PDF stays tiny.
//!
//! Letter-size pages (612x792 pt). Long lines are wrapped to the text width and
//! the text flows across as many pages as needed. `font_size` and `margin` (both
//! in points) are configurable.

use lopdf::content::{Content, Operation};
use lopdf::{dictionary, Document, Object, Stream};

const PAGE_W: f64 = 612.0; // US Letter
const PAGE_H: f64 = 792.0;
// Courier advance width is 600/1000 em; line height 1.4x the font size.
const CHAR_EM: f64 = 0.6;
const LINE_FACTOR: f64 = 1.4;
const TAB_WIDTH: usize = 4;

/// Escape a line for a PDF literal string and fold to Latin-1 bytes (Courier's
/// built-in encoding covers ASCII/Latin-1; other code points become '?').
fn pdf_escape(line: &str) -> Vec<u8> {
    let mut out = Vec::with_capacity(line.len() + 2);
    for ch in line.chars() {
        let b = if (ch as u32) <= 0xFF { ch as u8 } else { b'?' };
        match b {
            b'\\' => out.extend_from_slice(b"\\\\"),
            b'(' => out.extend_from_slice(b"\\("),
            b')' => out.extend_from_slice(b"\\)"),
            _ => out.push(b),
        }
    }
    out
}

/// Wrap `text` into display lines: explicit newlines are preserved; lines longer
/// than `max_chars` are hard-wrapped at word boundaries (or mid-word if a single
/// word exceeds the width). Tabs expand to spaces.
fn wrap_lines(text: &str, max_chars: usize) -> Vec<String> {
    let max_chars = max_chars.max(1);
    let mut out = Vec::new();
    for raw in text.replace('\t', &" ".repeat(TAB_WIDTH)).split('\n') {
        let line = raw.trim_end_matches('\r');
        if line.is_empty() {
            out.push(String::new());
            continue;
        }
        let mut current = String::new();
        for word in line.split(' ') {
            // A single word longer than the line: hard-split it.
            if word.chars().count() > max_chars {
                if !current.is_empty() {
                    out.push(std::mem::take(&mut current));
                }
                let mut chunk = String::new();
                for c in word.chars() {
                    if chunk.chars().count() == max_chars {
                        out.push(std::mem::take(&mut chunk));
                    }
                    chunk.push(c);
                }
                current = chunk;
                continue;
            }
            let extra = if current.is_empty() { 0 } else { 1 };
            if current.chars().count() + extra + word.chars().count() > max_chars {
                out.push(std::mem::take(&mut current));
                current.push_str(word);
            } else {
                if extra == 1 {
                    current.push(' ');
                }
                current.push_str(word);
            }
        }
        out.push(current);
    }
    out
}

/// Render `text` to a paginated PDF. `font_size` and `margin` are in points.
pub fn text_to_pdf(text: &str, font_size: f64, margin: f64) -> Result<Vec<u8>, String> {
    if !font_size.is_finite() || font_size < 4.0 || font_size > 96.0 {
        return Err("font_size must be between 4 and 96 points".into());
    }
    if !margin.is_finite() || margin < 0.0 || margin * 2.0 >= PAGE_H.min(PAGE_W) {
        return Err("margin is too large for the page".into());
    }

    let char_w = font_size * CHAR_EM;
    let line_h = font_size * LINE_FACTOR;
    let text_w = PAGE_W - 2.0 * margin;
    let text_h = PAGE_H - 2.0 * margin;
    let max_chars = (text_w / char_w).floor().max(1.0) as usize;
    let lines_per_page = (text_h / line_h).floor().max(1.0) as usize;

    let lines = wrap_lines(text, max_chars);
    let lines = if lines.is_empty() { vec![String::new()] } else { lines };

    let mut doc = Document::with_version("1.5");
    let pages_id = doc.new_object_id();
    let font_id = doc.add_object(dictionary! {
        "Type" => "Font",
        "Subtype" => "Type1",
        "BaseFont" => "Courier",
    });
    let resources_id = doc.add_object(dictionary! {
        "Font" => dictionary! { "F1" => font_id },
    });

    let mut page_ids: Vec<Object> = Vec::new();
    for chunk in lines.chunks(lines_per_page) {
        let mut ops = Vec::new();
        ops.push(Operation::new("BT", vec![]));
        ops.push(Operation::new("Tf", vec!["F1".into(), font_size.into()]));
        ops.push(Operation::new("TL", vec![line_h.into()]));
        // Start at the top-left of the text area (baseline one line down).
        let start_y = PAGE_H - margin - font_size;
        ops.push(Operation::new("Td", vec![margin.into(), start_y.into()]));
        for (i, line) in chunk.iter().enumerate() {
            if i > 0 {
                ops.push(Operation::new("T*", vec![]));
            }
            ops.push(Operation::new(
                "Tj",
                vec![Object::String(pdf_escape(line), lopdf::StringFormat::Literal)],
            ));
        }
        ops.push(Operation::new("ET", vec![]));

        let content = Content { operations: ops };
        let content_id = doc.add_object(Stream::new(
            dictionary! {},
            content.encode().map_err(|e| format!("content encode: {e}"))?,
        ));
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => content_id,
            "Resources" => resources_id,
            "MediaBox" => vec![0.into(), 0.into(), PAGE_W.into(), PAGE_H.into()],
        });
        page_ids.push(page_id.into());
    }

    let count = page_ids.len() as i64;
    let pages = dictionary! {
        "Type" => "Pages",
        "Kids" => page_ids,
        "Count" => count,
    };
    doc.objects.insert(pages_id, Object::Dictionary(pages));
    let catalog_id = doc.add_object(dictionary! {
        "Type" => "Catalog",
        "Pages" => pages_id,
    });
    doc.trailer.set("Root", catalog_id);

    let mut out = Vec::new();
    doc.save_to(&mut out).map_err(|e| format!("failed to write PDF: {e}"))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn page_count(pdf: &[u8]) -> usize {
        let doc = Document::load_mem(pdf).unwrap();
        doc.page_iter().count()
    }

    #[test]
    fn makes_a_valid_one_page_pdf() {
        let pdf = text_to_pdf("Hello, world!\nSecond line.", 12.0, 72.0).unwrap();
        assert_eq!(&pdf[..5], b"%PDF-");
        assert_eq!(page_count(&pdf), 1);
    }

    #[test]
    fn paginates_long_text() {
        // Many lines force multiple pages.
        let text = (0..500).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n");
        let pdf = text_to_pdf(&text, 12.0, 72.0).unwrap();
        assert!(page_count(&pdf) > 1, "500 lines should span multiple pages");
    }

    #[test]
    fn wraps_long_lines() {
        let max = (612.0 - 144.0) / (12.0 * 0.6); // ~65 chars at size 12, 1in margins
        let lines = wrap_lines(&"a".repeat(200), max as usize);
        assert!(lines.len() >= 3, "a 200-char word should wrap onto >= 3 lines");
        assert!(lines.iter().all(|l| l.chars().count() <= max as usize));
    }

    #[test]
    fn wraps_at_word_boundaries() {
        let lines = wrap_lines("the quick brown fox jumps", 10);
        assert!(lines.iter().all(|l| l.chars().count() <= 10));
        // words stay intact
        assert!(lines.iter().any(|l| l.contains("quick")));
    }

    #[test]
    fn errors_on_bad_params() {
        assert!(text_to_pdf("x", 2.0, 72.0).is_err()); // font too small
        assert!(text_to_pdf("x", 12.0, 400.0).is_err()); // margin too big
    }
}
