//! gizza-ai/pdf-compress core — pure PDF size reduction shared by the chat skill
//! block. No wafer/wasm-bindgen deps so it is unit-testable on the host.
//!
//! [`compress`] parses a PDF, drops objects unreachable from the document root,
//! Flate-compresses every stream that isn't already compressed (content streams
//! and image/XObject streams alike), renumbers the object table, and serializes
//! the result. It is lossless: text, vector graphics, and already-compressed
//! image data are preserved byte-for-byte; only the container is made leaner. It
//! does NOT transcode images to a lossier codec (that would need a raster
//! pipeline) — see the tool's notes.

use lopdf::Document;

/// Compress a PDF in place and return the serialized bytes. Errors as a
/// human-readable string if the input doesn't parse as a PDF.
pub fn compress(pdf: &[u8]) -> Result<Vec<u8>, String> {
    let mut doc = Document::load_mem(pdf).map_err(|e| format!("failed to parse PDF: {e}"))?;
    // Drop objects no longer reachable from the trailer Root (orphaned revisions,
    // dead annotations) — pure size win, no content change.
    doc.prune_objects();
    // Flate-compress every stream that permits it and isn't already compressed.
    doc.compress();
    // Compact the object id space after pruning.
    doc.renumber_objects();
    let mut out = Vec::new();
    doc.save_to(&mut out)
        .map_err(|e| format!("failed to serialize PDF: {e}"))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::content::{Content, Operation};
    use lopdf::{dictionary, Document, Object, Stream};

    /// Build a one-page PDF whose content stream is large and uncompressed (many
    /// repeated text operations) so Flate compression has clear headroom.
    fn bulky_uncompressed_pdf() -> Vec<u8> {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let font_id = doc.add_object(dictionary! {
            "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Courier",
        });
        let resources_id = doc.add_object(dictionary! {
            "Font" => dictionary! { "F1" => font_id },
        });
        // ~400 highly repetitive text ops → a big, very compressible stream.
        let mut ops = vec![Operation::new("BT", vec![])];
        for i in 0..400 {
            ops.push(Operation::new("Tf", vec!["F1".into(), 12.into()]));
            ops.push(Operation::new("Td", vec![20.into(), (10 + (i % 80)).into()]));
            ops.push(Operation::new(
                "Tj",
                vec![Object::string_literal("The quick brown fox jumps over the lazy dog.")],
            ));
        }
        ops.push(Operation::new("ET", vec![]));
        let content = Content { operations: ops };
        // A plain Stream (allows_compression defaults true) written uncompressed.
        let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => content_id,
            "Resources" => resources_id,
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
        });
        let pages = dictionary! {
            "Type" => "Pages", "Kids" => vec![page_id.into()], "Count" => 1,
        };
        doc.objects.insert(pages_id, Object::Dictionary(pages));
        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog", "Pages" => pages_id,
        });
        doc.trailer.set("Root", catalog_id);
        let mut out = Vec::new();
        doc.save_to(&mut out).expect("serialize fixture pdf");
        out
    }

    fn page_count(bytes: &[u8]) -> usize {
        Document::load_mem(bytes).expect("output should parse").get_pages().len()
    }

    #[test]
    fn compresses_bulky_pdf_smaller_and_still_valid() {
        let original = bulky_uncompressed_pdf();
        let out = compress(&original).expect("compress should succeed");
        assert!(
            out.len() < original.len(),
            "compressed ({}) should be smaller than original ({})",
            out.len(),
            original.len()
        );
        // Still a valid, single-page PDF.
        assert_eq!(page_count(&out), 1);
    }

    #[test]
    fn compress_is_idempotent_on_already_compressed() {
        let original = bulky_uncompressed_pdf();
        let once = compress(&original).unwrap();
        let twice = compress(&once).unwrap();
        // Re-compressing an already-compressed PDF must not grow it or corrupt it.
        assert!(twice.len() <= once.len() + 64, "second pass should not bloat");
        assert_eq!(page_count(&twice), 1);
    }

    #[test]
    fn rejects_non_pdf_bytes() {
        let err = compress(b"this is not a pdf").unwrap_err();
        assert!(err.contains("failed to parse"), "got: {err}");
    }
}
