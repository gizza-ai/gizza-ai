//! gizza-ai/merge-pdf core — pure PDF concatenation shared by the chat skill
//! block. No wafer/wasm-bindgen deps so it is unit-testable on the host.
//!
//! Given a list of PDF byte buffers, [`merge`] concatenates every page (in the
//! given order) into one document and serializes it back to bytes. The
//! algorithm follows lopdf's canonical `merge.rs` example, minus bookmark /
//! outline handling: each input's object ids are renumbered into a disjoint
//! range, all `Page` objects are reparented under a single `Pages` tree, and a
//! single `Catalog` points at that tree.

use std::collections::BTreeMap;

use lopdf::{Document, Object, ObjectId};

/// Concatenate the pages of every input PDF, in order, into a single PDF.
///
/// Returns the serialized merged document. Errors as a human-readable string
/// if `pdfs` has fewer than two entries, if any buffer fails to parse as a
/// PDF, or if the merged result has no `Pages`/`Catalog` root.
pub fn merge(pdfs: &[Vec<u8>]) -> Result<Vec<u8>, String> {
    if pdfs.len() < 2 {
        return Err(format!(
            "merge needs at least 2 PDFs, got {}",
            pdfs.len()
        ));
    }

    // Parse every input up front so a bad buffer fails the whole operation
    // with a clear index, rather than silently dropping a document.
    let mut documents: Vec<Document> = Vec::with_capacity(pdfs.len());
    for (i, bytes) in pdfs.iter().enumerate() {
        let doc = Document::load_mem(bytes)
            .map_err(|e| format!("PDF #{} failed to parse: {e}", i + 1))?;
        documents.push(doc);
    }

    let mut max_id: u32 = 1;
    let mut documents_pages: BTreeMap<ObjectId, Object> = BTreeMap::new();
    let mut documents_objects: BTreeMap<ObjectId, Object> = BTreeMap::new();
    let mut document = Document::with_version("1.5");

    for mut doc in documents {
        // Shift this input's object ids past everything merged so far so ids
        // from different inputs never collide.
        doc.renumber_objects_with(max_id);
        max_id = doc.max_id + 1;

        // Collect this input's pages (object id -> page object).
        for (_page_num, object_id) in doc.get_pages() {
            if let Ok(obj) = doc.get_object(object_id) {
                documents_pages.insert(object_id, obj.to_owned());
            }
        }

        documents_objects.extend(doc.objects);
    }

    // First pass: copy every non-page object into the output, collecting the
    // first Catalog and merging all Pages dictionaries into one.
    let mut catalog_object: Option<(ObjectId, Object)> = None;
    let mut pages_object: Option<(ObjectId, Object)> = None;

    for (object_id, object) in documents_objects {
        match object.type_name().unwrap_or(b"") {
            b"Catalog" => {
                let id = catalog_object.as_ref().map(|(id, _)| *id).unwrap_or(object_id);
                catalog_object = Some((id, object));
            }
            b"Pages" => {
                if let Ok(dictionary) = object.as_dict() {
                    let mut dictionary = dictionary.clone();
                    if let Some((_, ref existing)) = pages_object {
                        if let Ok(old) = existing.as_dict() {
                            dictionary.extend(old);
                        }
                    }
                    let id = pages_object.as_ref().map(|(id, _)| *id).unwrap_or(object_id);
                    pages_object = Some((id, Object::Dictionary(dictionary)));
                }
            }
            // Pages are reparented in the second pass; outline objects are
            // dropped (bookmarks are not preserved across a merge).
            b"Page" | b"Outlines" | b"Outline" => {}
            _ => {
                document.objects.insert(object_id, object);
            }
        }
    }

    let (pages_id, pages_obj) = pages_object
        .ok_or_else(|| "no Pages root found in any input PDF".to_string())?;
    let (catalog_id, catalog_obj) = catalog_object
        .ok_or_else(|| "no Catalog root found in any input PDF".to_string())?;

    // Second pass: reparent every page under the single merged Pages tree.
    for (object_id, object) in &documents_pages {
        if let Ok(dictionary) = object.as_dict() {
            let mut dictionary = dictionary.clone();
            dictionary.set("Parent", pages_id);
            document
                .objects
                .insert(*object_id, Object::Dictionary(dictionary));
        }
    }

    // Rebuild the Pages node with the full Kids list + total page count.
    if let Ok(dictionary) = pages_obj.as_dict() {
        let mut dictionary = dictionary.clone();
        dictionary.set("Count", documents_pages.len() as u32);
        dictionary.set(
            "Kids",
            documents_pages
                .keys()
                .map(|id| Object::Reference(*id))
                .collect::<Vec<_>>(),
        );
        document.objects.insert(pages_id, Object::Dictionary(dictionary));
    }

    // Rebuild the Catalog pointing at the merged Pages node.
    if let Ok(dictionary) = catalog_obj.as_dict() {
        let mut dictionary = dictionary.clone();
        dictionary.set("Pages", pages_id);
        dictionary.remove(b"Outlines"); // outlines are not carried over
        document.objects.insert(catalog_id, Object::Dictionary(dictionary));
    }

    document.trailer.set("Root", catalog_id);
    document.max_id = document.objects.len() as u32;
    document.renumber_objects();
    document.adjust_zero_pages();

    let mut out = Vec::new();
    document
        .save_to(&mut out)
        .map_err(|e| format!("failed to serialize merged PDF: {e}"))?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::{dictionary, Document, Object, Stream};
    use lopdf::content::{Content, Operation};

    /// Build a minimal valid single-page PDF carrying `text`, serialized to
    /// bytes. Mirrors lopdf's `generate_fake_document` example, trimmed to one
    /// page so tests can assert exact page counts.
    fn one_page_pdf(text: &str) -> Vec<u8> {
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
        let content = Content {
            operations: vec![
                Operation::new("BT", vec![]),
                Operation::new("Tf", vec!["F1".into(), 48.into()]),
                Operation::new("Td", vec![100.into(), 600.into()]),
                Operation::new("Tj", vec![Object::string_literal(text)]),
                Operation::new("ET", vec![]),
            ],
        };
        let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => content_id,
            "Resources" => resources_id,
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
        });
        let pages = dictionary! {
            "Type" => "Pages",
            "Kids" => vec![page_id.into()],
            "Count" => 1,
        };
        doc.objects.insert(pages_id, Object::Dictionary(pages));
        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        doc.trailer.set("Root", catalog_id);

        let mut out = Vec::new();
        doc.save_to(&mut out).expect("serialize fixture pdf");
        out
    }

    fn page_count(bytes: &[u8]) -> usize {
        Document::load_mem(bytes)
            .expect("merged output should parse")
            .get_pages()
            .len()
    }

    #[test]
    fn merges_two_single_page_pdfs_into_two_pages() {
        let a = one_page_pdf("Alpha");
        let b = one_page_pdf("Beta");
        assert_eq!(page_count(&a), 1);
        assert_eq!(page_count(&b), 1);

        let merged = merge(&[a, b]).expect("merge should succeed");
        assert_eq!(page_count(&merged), 2, "merged page count = sum of inputs");
    }

    #[test]
    fn merges_three_pdfs_page_count_is_sum() {
        let docs = vec![
            one_page_pdf("One"),
            one_page_pdf("Two"),
            one_page_pdf("Three"),
        ];
        let merged = merge(&docs).expect("merge should succeed");
        assert_eq!(page_count(&merged), 3);
    }

    #[test]
    fn rejects_fewer_than_two_inputs() {
        let only = vec![one_page_pdf("Solo")];
        let err = merge(&only).unwrap_err();
        assert!(err.contains("at least 2"), "got: {err}");

        let none: Vec<Vec<u8>> = Vec::new();
        let err = merge(&none).unwrap_err();
        assert!(err.contains("at least 2"), "got: {err}");
    }

    #[test]
    fn rejects_invalid_pdf_bytes() {
        let good = one_page_pdf("Good");
        let garbage = b"this is not a pdf at all".to_vec();
        let err = merge(&[good, garbage]).unwrap_err();
        assert!(err.contains("failed to parse"), "got: {err}");
        assert!(err.contains("#2"), "error should point at the bad input: {err}");
    }
}
