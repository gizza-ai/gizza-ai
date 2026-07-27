//! pdf-metadata-edit core — pure PDF document-info manipulation shared by the
//! chat skill block and CLI. No wafer/wasm-bindgen deps, so it is unit-testable
//! on the host.
//!
//! A PDF's document-level metadata lives in the `/Info` dictionary referenced
//! from the trailer (Title, Author, Subject, Keywords, plus read-only Creator /
//! Producer). This module exposes two operations:
//!   * [`read_info`] — parse the PDF and return the current Info fields (view).
//!   * [`edit`] — apply a set of per-field [`FieldUpdate`]s (set / clear /
//!     leave), re-serialize, and report what changed (edit).
//!
//! Text values are decoded/encoded per the PDF text-string convention: a
//! UTF-16BE byte-order mark selects UTF-16, otherwise the bytes are treated as
//! PDFDocEncoding (approximated by Latin-1, which round-trips ASCII exactly).
//! On write, ASCII stays a plain literal string; anything with non-ASCII is
//! emitted as UTF-16BE with a BOM so accented names and CJK titles survive.

use lopdf::{Dictionary, Document, Object, ObjectId, StringFormat};

/// One metadata field's current value (as read from the Info dict).
///
/// `None` means the key is absent (or not a string object).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Info {
    pub title: Option<String>,
    pub author: Option<String>,
    pub subject: Option<String>,
    pub keywords: Option<String>,
    /// Read-only in this tool — surfaced by `view` for context.
    pub creator: Option<String>,
    /// Read-only in this tool — surfaced by `view` for context.
    pub producer: Option<String>,
}

/// What to do with a single editable field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FieldUpdate {
    /// Don't touch the existing value.
    Leave,
    /// Set (or overwrite) the field to this value.
    Set(String),
    /// Remove the key from the Info dictionary.
    Clear,
}

/// The four editable fields, each with its own [`FieldUpdate`].
#[derive(Debug, Clone, Default)]
pub struct Updates {
    pub title: FieldUpdate,
    pub author: FieldUpdate,
    pub subject: FieldUpdate,
    pub keywords: FieldUpdate,
}

impl Default for FieldUpdate {
    fn default() -> Self {
        FieldUpdate::Leave
    }
}

impl Updates {
    /// True when no field would change — the caller should reject this rather
    /// than re-serialize a byte-identical PDF.
    pub fn is_noop(&self) -> bool {
        [&self.title, &self.author, &self.subject, &self.keywords]
            .iter()
            .all(|u| matches!(u, FieldUpdate::Leave))
    }
}

/// Result of an [`edit`]: the new PDF bytes, the human-facing list of changed
/// field names, and the Info dict as it stands after the edit.
#[derive(Debug, Clone)]
pub struct EditResult {
    pub bytes: Vec<u8>,
    /// Field labels that were set or cleared, in field order (e.g. `["Title"]`).
    pub changed: Vec<String>,
    pub info: Info,
}

/// Parse `pdf` and return its current Info-dictionary metadata (view mode).
/// A PDF with no `/Info` dictionary yields an all-`None` [`Info`].
pub fn read_info(pdf: &[u8]) -> Result<Info, String> {
    let doc = Document::load_mem(pdf).map_err(|e| format!("failed to parse PDF: {e}"))?;
    Ok(match doc.trailer.get(b"Info") {
        Ok(Object::Reference(id)) => doc.get_dictionary(*id).map(read_info_dict).unwrap_or_default(),
        Ok(Object::Dictionary(d)) => read_info_dict(d),
        _ => Info::default(),
    })
}

/// Apply `updates` to `pdf`'s Info dictionary (creating one if absent) and
/// re-serialize. Fields left as [`FieldUpdate::Leave`] are preserved exactly.
pub fn edit(pdf: &[u8], updates: &Updates) -> Result<EditResult, String> {
    let mut doc = Document::load_mem(pdf).map_err(|e| format!("failed to parse PDF: {e}"))?;
    let id = get_or_create_info_id(&mut doc);

    let mut changed = Vec::new();
    {
        let dict = match doc.get_object_mut(id) {
            Ok(Object::Dictionary(d)) => d,
            Ok(_) => return Err("document /Info entry is not a dictionary".into()),
            Err(e) => return Err(format!("failed to read Info dictionary: {e}")),
        };
        apply(dict, b"Title", &updates.title, "Title", &mut changed);
        apply(dict, b"Author", &updates.author, "Author", &mut changed);
        apply(dict, b"Subject", &updates.subject, "Subject", &mut changed);
        apply(dict, b"Keywords", &updates.keywords, "Keywords", &mut changed);
    }

    let info = doc.get_dictionary(id).map(read_info_dict).unwrap_or_default();
    let mut out = Vec::new();
    doc.save_to(&mut out)
        .map_err(|e| format!("failed to serialize PDF: {e}"))?;
    Ok(EditResult {
        bytes: out,
        changed,
        info,
    })
}

/// Find the Info dictionary's object id, creating an empty one (and wiring it
/// into the trailer) if the document has none. An inline (non-reference) Info
/// dictionary is promoted to a referenced object so it can be mutated in place.
fn get_or_create_info_id(doc: &mut Document) -> ObjectId {
    let existing = match doc.trailer.get(b"Info") {
        Ok(Object::Reference(id)) => Some(*id),
        Ok(Object::Dictionary(d)) => {
            let d = d.clone();
            let id = doc.add_object(d);
            doc.trailer.set("Info", id);
            return id;
        }
        _ => None,
    };
    if let Some(id) = existing {
        return id;
    }
    let id = doc.add_object(Dictionary::new());
    doc.trailer.set("Info", id);
    id
}

/// Apply one field's update to `dict`, recording `label` in `changed` when it
/// actually mutates the dictionary.
fn apply(dict: &mut Dictionary, key: &[u8], update: &FieldUpdate, label: &str, changed: &mut Vec<String>) {
    match update {
        FieldUpdate::Leave => {}
        FieldUpdate::Set(v) => {
            dict.set(key.to_vec(), encode_pdf_string(v));
            changed.push(label.to_string());
        }
        FieldUpdate::Clear => {
            if dict.has(key) {
                dict.remove(key);
                changed.push(label.to_string());
            }
        }
    }
}

/// Read the six surfaced fields out of an Info dictionary.
fn read_info_dict(dict: &Dictionary) -> Info {
    Info {
        title: read_field(dict, b"Title"),
        author: read_field(dict, b"Author"),
        subject: read_field(dict, b"Subject"),
        keywords: read_field(dict, b"Keywords"),
        creator: read_field(dict, b"Creator"),
        producer: read_field(dict, b"Producer"),
    }
}

/// Read a single string field, decoding the PDF text-string encoding.
fn read_field(dict: &Dictionary, key: &[u8]) -> Option<String> {
    match dict.get(key) {
        Ok(Object::String(bytes, _)) => Some(decode_pdf_string(bytes)),
        _ => None,
    }
}

/// Decode a PDF text string: UTF-16BE if it carries a BOM, else PDFDocEncoding
/// (approximated as Latin-1, exact for ASCII).
fn decode_pdf_string(bytes: &[u8]) -> String {
    if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
        let units: Vec<u16> = bytes[2..]
            .chunks(2)
            .map(|c| if c.len() == 2 { u16::from_be_bytes([c[0], c[1]]) } else { c[0] as u16 })
            .collect();
        String::from_utf16_lossy(&units)
    } else {
        bytes.iter().map(|&b| b as char).collect()
    }
}

/// Encode a Rust string as a PDF text string: a plain literal for ASCII, or
/// UTF-16BE with a leading BOM when any character is non-ASCII.
fn encode_pdf_string(s: &str) -> Object {
    if s.is_ascii() {
        Object::String(s.as_bytes().to_vec(), StringFormat::Literal)
    } else {
        let mut bytes = vec![0xFE, 0xFF];
        for unit in s.encode_utf16() {
            bytes.extend_from_slice(&unit.to_be_bytes());
        }
        Object::String(bytes, StringFormat::Literal)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::{dictionary, Object};

    /// Build a minimal one-page PDF whose Info dictionary holds `fields`.
    fn pdf_with_info(fields: &[(&str, &str)]) -> Vec<u8> {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let page_id = doc.add_object(dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
        });
        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages", "Kids" => vec![page_id.into()], "Count" => 1,
            }),
        );
        let catalog_id = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
        doc.trailer.set("Root", catalog_id);

        if !fields.is_empty() {
            let mut info = Dictionary::new();
            for (k, v) in fields {
                info.set(k.as_bytes().to_vec(), encode_pdf_string(v));
            }
            let info_id = doc.add_object(info);
            doc.trailer.set("Info", info_id);
        }

        let mut out = Vec::new();
        doc.save_to(&mut out).unwrap();
        out
    }

    #[test]
    fn reads_existing_info_fields() {
        let pdf = pdf_with_info(&[("Title", "Quarterly Report"), ("Author", "Ada")]);
        let info = read_info(&pdf).unwrap();
        assert_eq!(info.title.as_deref(), Some("Quarterly Report"));
        assert_eq!(info.author.as_deref(), Some("Ada"));
        assert_eq!(info.subject, None);
        assert_eq!(info.keywords, None);
    }

    #[test]
    fn read_info_on_pdf_without_info_dict_is_all_none() {
        let pdf = pdf_with_info(&[]);
        assert_eq!(read_info(&pdf).unwrap(), Info::default());
    }

    #[test]
    fn edit_sets_clears_and_leaves() {
        let pdf = pdf_with_info(&[("Title", "Old Title"), ("Author", "Ada"), ("Subject", "Keep")]);
        let updates = Updates {
            title: FieldUpdate::Set("New Title".into()),
            author: FieldUpdate::Clear,
            subject: FieldUpdate::Leave,
            keywords: FieldUpdate::Set("alpha, beta".into()),
        };
        let result = edit(&pdf, &updates).unwrap();
        // Reported changes cover set + clear, not the left-alone subject.
        assert_eq!(result.changed, vec!["Title", "Author", "Keywords"]);

        // Re-parse the emitted bytes to confirm the on-disk result.
        let info = read_info(&result.bytes).unwrap();
        assert_eq!(info.title.as_deref(), Some("New Title"));
        assert_eq!(info.author, None); // cleared
        assert_eq!(info.subject.as_deref(), Some("Keep")); // untouched
        assert_eq!(info.keywords.as_deref(), Some("alpha, beta"));
    }

    #[test]
    fn edit_creates_info_dict_when_absent() {
        let pdf = pdf_with_info(&[]);
        assert_eq!(read_info(&pdf).unwrap().title, None);
        let updates = Updates {
            title: FieldUpdate::Set("Fresh".into()),
            ..Updates::default()
        };
        let result = edit(&pdf, &updates).unwrap();
        assert_eq!(read_info(&result.bytes).unwrap().title.as_deref(), Some("Fresh"));
    }

    #[test]
    fn edit_round_trips_non_ascii_via_utf16() {
        let pdf = pdf_with_info(&[]);
        let updates = Updates {
            author: FieldUpdate::Set("José Ünïcode 日本語".into()),
            ..Updates::default()
        };
        let result = edit(&pdf, &updates).unwrap();
        assert_eq!(
            read_info(&result.bytes).unwrap().author.as_deref(),
            Some("José Ünïcode 日本語")
        );
    }

    #[test]
    fn clearing_absent_field_is_not_reported_as_changed() {
        let pdf = pdf_with_info(&[("Title", "Only Title")]);
        let updates = Updates {
            author: FieldUpdate::Clear,
            ..Updates::default()
        };
        let result = edit(&pdf, &updates).unwrap();
        assert!(result.changed.is_empty());
    }

    #[test]
    fn is_noop_detects_all_leave() {
        assert!(Updates::default().is_noop());
        assert!(!Updates {
            title: FieldUpdate::Set("x".into()),
            ..Updates::default()
        }
        .is_noop());
    }

    #[test]
    fn read_info_rejects_non_pdf_bytes() {
        let err = read_info(b"definitely not a pdf").unwrap_err();
        assert!(err.contains("failed to parse"), "got: {err}");
    }

    #[test]
    fn edit_rejects_non_pdf_bytes() {
        let err = edit(b"nope", &Updates {
            title: FieldUpdate::Set("x".into()),
            ..Updates::default()
        })
        .unwrap_err();
        assert!(err.contains("failed to parse"), "got: {err}");
    }
}
