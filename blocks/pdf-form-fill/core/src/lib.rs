//! gizza-ai/pdf-form-fill core — fill AcroForm (interactive form) fields in a
//! PDF. No wafer/wasm-bindgen deps. Pure-Rust `lopdf`. Sets each named field's
//! `/V` (and `/AS` for buttons), drops stale `/AP` appearance streams, and sets
//! the AcroForm's `/NeedAppearances true` so viewers regenerate the visible
//! value. Walks nested `/Kids` field hierarchies; fields are addressable by full
//! dotted name or leaf name, and UTF-16BE `/T` names (as in IRS forms) are decoded.

use std::collections::HashMap;

use lopdf::{Document, Object, ObjectId};

/// Outcome of a fill.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FillResult {
    pub pdf: Vec<u8>,
    /// Field names that were found and set.
    pub filled: Vec<String>,
    /// Requested names not found in the form.
    pub unknown: Vec<String>,
    /// Total AcroForm fields in the document.
    pub total_fields: usize,
    /// Every fillable field's (full) name found in the form — useful for
    /// discovering exact names to fill.
    pub available: Vec<String>,
}

/// A terminal (fillable) field discovered by walking the AcroForm tree.
struct Terminal {
    full_name: String,
    id: ObjectId,
    ft: Option<String>,
}

/// Decode a PDF text string. Per the PDF spec, a string beginning with the
/// UTF-16BE byte-order mark (`FE FF`) is UTF-16BE; otherwise it is PDFDocEncoding
/// (a Latin-1 superset — we approximate with the Latin-1 subset, which covers the
/// ASCII field names PDFs actually use). Real-world forms (e.g. IRS fillable PDFs)
/// store `/T` names as UTF-16BE, so decoding this correctly is what makes their
/// field names matchable.
fn decode_pdf_text(bytes: &[u8]) -> String {
    if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
        let units: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect();
        String::from_utf16_lossy(&units)
    } else {
        // PDFDocEncoding ≈ Latin-1 for the printable range field names use.
        bytes.iter().map(|&b| b as char).collect()
    }
}

/// Read a field dict's partial `/T` name.
fn partial_name(doc: &Document, id: ObjectId) -> Option<String> {
    doc.get_dictionary(id)
        .ok()?
        .get(b"T")
        .ok()?
        .as_str()
        .ok()
        .map(decode_pdf_text)
}

/// Walk the field tree from `roots`, descending into `/Kids` that are themselves
/// fields (have a `/T`). Widget-only kids (no `/T`) mean the node is terminal.
/// Full names join ancestor partial names with '.'.
fn collect_terminals(doc: &Document, roots: &[ObjectId]) -> Vec<Terminal> {
    let mut out = Vec::new();
    // (id, prefix) — iterative DFS to avoid recursion/borrow issues.
    let mut stack: Vec<(ObjectId, String)> = roots.iter().rev().map(|id| (*id, String::new())).collect();
    let mut guard = 0usize;
    while let Some((id, prefix)) = stack.pop() {
        guard += 1;
        if guard > 100_000 {
            break; // pathological cycle guard
        }
        let Ok(d) = doc.get_dictionary(id) else { continue };
        let part = d.get(b"T").ok().and_then(|o| o.as_str().ok()).map(decode_pdf_text);
        let full = match part {
            Some(t) if prefix.is_empty() => t,
            Some(t) => format!("{prefix}.{t}"),
            None => prefix.clone(),
        };
        let ft = d.get(b"FT").ok().and_then(|o| o.as_name().ok()).map(|s| String::from_utf8_lossy(s).to_string());
        // Child fields = kid refs that themselves carry a /T.
        let child_fields: Vec<ObjectId> = d
            .get(b"Kids")
            .ok()
            .and_then(|o| o.as_array().ok())
            .map(|a| {
                a.iter()
                    .filter_map(|o| o.as_reference().ok())
                    .filter(|kid| partial_name(doc, *kid).is_some())
                    .collect()
            })
            .unwrap_or_default();
        if child_fields.is_empty() {
            out.push(Terminal { full_name: full, id, ft });
        } else {
            for k in child_fields.into_iter().rev() {
                stack.push((k, full.clone()));
            }
        }
    }
    out
}

/// Fill `fields` (name → value) into the AcroForm of `pdf`. Field names may be the
/// full dotted name or the leaf partial name. Errors if the PDF can't be parsed
/// or has no AcroForm.
pub fn fill(pdf: &[u8], fields: &[(String, String)]) -> Result<FillResult, String> {
    let mut doc = Document::load_mem(pdf).map_err(|e| format!("failed to parse PDF: {e}"))?;
    let map: HashMap<String, String> = fields.iter().cloned().collect();

    // Collect the AcroForm id (if a reference) and the root field object ids.
    let (acroform_id, root_ids): (Option<ObjectId>, Vec<ObjectId>) = {
        let catalog = doc.catalog().map_err(|e| format!("no document catalog: {e}"))?;
        let af = catalog
            .get(b"AcroForm")
            .map_err(|_| "this PDF has no AcroForm (it is not a fillable form)".to_string())?;
        let af_id = af.as_reference().ok();
        let af_dict = match af {
            Object::Reference(id) => doc
                .get_dictionary(*id)
                .map_err(|e| format!("bad AcroForm reference: {e}"))?,
            Object::Dictionary(d) => d,
            _ => return Err("AcroForm is not a dictionary".into()),
        };
        let ids = af_dict
            .get(b"Fields")
            .ok()
            .and_then(|o| o.as_array().ok())
            .map(|a| a.iter().filter_map(|o| o.as_reference().ok()).collect())
            .unwrap_or_default();
        (af_id, ids)
    };

    let terminals = collect_terminals(&doc, &root_ids);
    let total_fields = terminals.len();
    let available: Vec<String> = terminals.iter().map(|t| t.full_name.clone()).collect();

    let mut filled = Vec::new();
    let mut filled_set = std::collections::HashSet::new();
    for t in &terminals {
        let leaf = t.full_name.rsplit('.').next().unwrap_or(&t.full_name);
        // Match by full dotted name first, then by the leaf partial name.
        let (matched_key, val) = if let Some(v) = map.get(&t.full_name) {
            (t.full_name.clone(), v.clone())
        } else if let Some(v) = map.get(leaf) {
            (leaf.to_string(), v.clone())
        } else {
            continue;
        };

        if let Ok(d) = doc.get_dictionary_mut(t.id) {
            match t.ft.as_deref() {
                Some("Btn") => {
                    d.set("V", Object::Name(val.clone().into_bytes()));
                    d.set("AS", Object::Name(val.into_bytes()));
                }
                _ => {
                    d.set("V", Object::string_literal(val));
                }
            }
            d.remove(b"AP"); // force the viewer to rebuild the appearance
            filled_set.insert(matched_key);
            filled.push(t.full_name.clone());
        }
    }

    // Ask viewers to regenerate appearances for the new values.
    if let Some(id) = acroform_id {
        if let Ok(d) = doc.get_dictionary_mut(id) {
            d.set("NeedAppearances", true);
        }
    } else if let Ok(cat) = doc.catalog_mut() {
        if let Ok(Object::Dictionary(d)) = cat.get_mut(b"AcroForm") {
            d.set("NeedAppearances", true);
        }
    }

    let unknown: Vec<String> = map
        .keys()
        .filter(|k| !filled_set.contains(*k))
        .cloned()
        .collect();

    let mut out = Vec::new();
    doc.save_to(&mut out).map_err(|e| format!("failed to write PDF: {e}"))?;
    Ok(FillResult { pdf: out, filled, unknown, total_fields, available })
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::dictionary;

    /// Build a minimal fillable PDF with two AcroForm text fields.
    fn form_pdf() -> Vec<u8> {
        let mut doc = Document::with_version("1.5");
        let f1 = doc.add_object(dictionary! {
            "T" => Object::string_literal("name"),
            "FT" => Object::Name(b"Tx".to_vec()),
        });
        let f2 = doc.add_object(dictionary! {
            "T" => Object::string_literal("email"),
            "FT" => Object::Name(b"Tx".to_vec()),
        });
        let acro = doc.add_object(dictionary! {
            "Fields" => vec![Object::Reference(f1), Object::Reference(f2)],
        });
        let pages = doc.add_object(dictionary! {
            "Type" => "Pages",
            "Kids" => Vec::<Object>::new(),
            "Count" => 0i64,
        });
        let catalog = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => Object::Reference(pages),
            "AcroForm" => Object::Reference(acro),
        });
        doc.trailer.set("Root", Object::Reference(catalog));
        let mut out = Vec::new();
        doc.save_to(&mut out).unwrap();
        out
    }

    fn field_value(pdf: &[u8], name: &str) -> Option<String> {
        let doc = Document::load_mem(pdf).unwrap();
        for (_id, obj) in doc.objects.iter() {
            if let Object::Dictionary(d) = obj {
                let is_named = d
                    .get(b"T")
                    .ok()
                    .and_then(|o| o.as_str().ok())
                    .map(|s| s == name.as_bytes())
                    .unwrap_or(false);
                if is_named {
                    return d
                        .get(b"V")
                        .ok()
                        .and_then(|o| o.as_str().ok())
                        .map(|s| String::from_utf8_lossy(s).to_string());
                }
            }
        }
        None
    }

    #[test]
    fn fills_named_fields() {
        let pdf = form_pdf();
        let r = fill(&pdf, &[("name".into(), "Ada Lovelace".into())]).unwrap();
        assert_eq!(r.total_fields, 2);
        assert_eq!(r.filled, vec!["name".to_string()]);
        assert!(r.unknown.is_empty());
        assert_eq!(field_value(&r.pdf, "name").as_deref(), Some("Ada Lovelace"));
        // untouched field has no value
        assert_eq!(field_value(&r.pdf, "email"), None);
    }

    #[test]
    fn fills_nested_field_by_full_and_leaf_name() {
        // A subform "form1" with a leaf text field "first" → full name "form1.first".
        let mut doc = Document::with_version("1.5");
        let leaf = doc.add_object(dictionary! {
            "T" => Object::string_literal("first"),
            "FT" => Object::Name(b"Tx".to_vec()),
        });
        let subform = doc.add_object(dictionary! {
            "T" => Object::string_literal("form1"),
            "Kids" => vec![Object::Reference(leaf)],
        });
        let acro = doc.add_object(dictionary! { "Fields" => vec![Object::Reference(subform)] });
        let pages = doc.add_object(dictionary! { "Type" => "Pages", "Kids" => Vec::<Object>::new(), "Count" => 0i64 });
        let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => Object::Reference(pages), "AcroForm" => Object::Reference(acro) });
        doc.trailer.set("Root", Object::Reference(catalog));
        let mut pdf = Vec::new();
        doc.save_to(&mut pdf).unwrap();

        // Only the leaf is a terminal field; its full name is dotted.
        let r = fill(&pdf, &[("form1.first".into(), "Ada".into())]).unwrap();
        assert_eq!(r.total_fields, 1);
        assert_eq!(r.available, vec!["form1.first".to_string()]);
        assert_eq!(r.filled, vec!["form1.first".to_string()]);
        assert_eq!(field_value(&r.pdf, "first").as_deref(), Some("Ada"));

        // Leaf-name convenience also works.
        let r2 = fill(&pdf, &[("first".into(), "Bo".into())]).unwrap();
        assert_eq!(r2.filled, vec!["form1.first".to_string()]);
        assert_eq!(field_value(&r2.pdf, "first").as_deref(), Some("Bo"));
    }

    #[test]
    fn decodes_utf16be_field_names() {
        // A field whose /T is a UTF-16BE string with BOM, as real IRS forms use.
        let mut doc = Document::with_version("1.5");
        let mut t_bytes = vec![0xFE, 0xFF];
        for u in "f1_05".encode_utf16() {
            t_bytes.extend_from_slice(&u.to_be_bytes());
        }
        let f = doc.add_object(dictionary! {
            "T" => Object::String(t_bytes, lopdf::StringFormat::Literal),
            "FT" => Object::Name(b"Tx".to_vec()),
        });
        let acro = doc.add_object(dictionary! { "Fields" => vec![Object::Reference(f)] });
        let pages = doc.add_object(dictionary! { "Type" => "Pages", "Kids" => Vec::<Object>::new(), "Count" => 0i64 });
        let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => Object::Reference(pages), "AcroForm" => Object::Reference(acro) });
        doc.trailer.set("Root", Object::Reference(catalog));
        let mut pdf = Vec::new();
        doc.save_to(&mut pdf).unwrap();

        let r = fill(&pdf, &[("f1_05".into(), "Ada".into())]).unwrap();
        assert_eq!(r.available, vec!["f1_05".to_string()]);
        assert_eq!(r.filled, vec!["f1_05".to_string()]);
        assert!(r.unknown.is_empty());
    }

    #[test]
    fn reports_unknown_fields() {
        let pdf = form_pdf();
        let r = fill(&pdf, &[("nope".into(), "x".into())]).unwrap();
        assert!(r.filled.is_empty());
        assert_eq!(r.unknown, vec!["nope".to_string()]);
    }

    #[test]
    fn sets_need_appearances() {
        let pdf = form_pdf();
        let r = fill(&pdf, &[("name".into(), "X".into())]).unwrap();
        let doc = Document::load_mem(&r.pdf).unwrap();
        let na = doc
            .objects
            .values()
            .filter_map(|o| if let Object::Dictionary(d) = o { Some(d) } else { None })
            .any(|d| matches!(d.get(b"NeedAppearances"), Ok(Object::Boolean(true))));
        assert!(na, "AcroForm should have NeedAppearances true");
    }

    #[test]
    fn errors_without_acroform() {
        let mut doc = Document::with_version("1.5");
        let pages = doc.add_object(dictionary! { "Type" => "Pages", "Kids" => Vec::<Object>::new(), "Count" => 0i64 });
        let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => Object::Reference(pages) });
        doc.trailer.set("Root", Object::Reference(catalog));
        let mut out = Vec::new();
        doc.save_to(&mut out).unwrap();
        assert!(fill(&out, &[("a".into(), "b".into())]).is_err());
    }

    #[test]
    fn errors_on_garbage() {
        assert!(fill(b"not a pdf", &[]).is_err());
    }
}
