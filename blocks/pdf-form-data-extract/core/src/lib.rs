//! gizza-ai/pdf-form-data-extract core — read the filled values of an AcroForm
//! (interactive form) PDF and return each field's name / type / value. No
//! wafer/wasm-bindgen deps, so it compiles natively for the unit tests below and
//! to `wasm32-wasip1` for the block. Pure-Rust `lopdf` (no native libs).
//!
//! Pipeline: parse the PDF → find the catalog `/AcroForm` `/Fields` → walk the
//! field tree (descending `/Kids` that are themselves fields, i.e. carry a `/T`)
//! → for each terminal field compute the fully-qualified dotted name, decode its
//! `/V` value, and classify its type from `/FT` + the `/Ff` flag bits. Names and
//! values that are UTF-16BE strings (as real IRS fillable forms use) are decoded.
//!
//! This reads the AcroForm layer only. It does not parse pure XFA (XML) forms —
//! those need a separate XFA parser; a hybrid AcroForm/XFA form reads its AcroForm
//! fallback. A PDF with no AcroForm is reported as an error (it is not a form).

use lopdf::{Document, Object, ObjectId};

/// Cap on fields walked — guards a pathological/cyclic `/Kids` tree.
const MAX_FIELDS: usize = 100_000;

// PDF field-flag (`/Ff`) bits we key on (spec Table 226/228, 1-based positions).
const FF_RADIO: i64 = 1 << 15; // button field: Radio (bit 16)
const FF_PUSHBUTTON: i64 = 1 << 16; // button field: Pushbutton (bit 17)
const FF_COMBO: i64 = 1 << 17; // choice field: Combo/dropdown (bit 18)

/// One terminal (fillable) AcroForm field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormField {
    /// Fully-qualified name — ancestor partial `/T` names joined with `.`.
    pub name: String,
    /// Friendly type: `text`, `checkbox`, `radio`, `pushbutton`, `dropdown`,
    /// `listbox`, `signature`, or the raw `/FT` / `unknown` when absent.
    pub field_type: String,
    /// Decoded current value (`/V`). Empty string when the field has no value.
    pub value: String,
    /// True when the field carries a meaningful value — non-empty and, for
    /// buttons, not the `Off` (unchecked) state.
    pub filled: bool,
}

/// Decode a PDF text string. A string beginning with the UTF-16BE byte-order mark
/// (`FE FF`) is UTF-16BE; otherwise it is PDFDocEncoding (a Latin-1 superset we
/// approximate with Latin-1, which covers the ASCII names/values PDFs use). Real
/// IRS fillable forms store `/T` names as UTF-16BE, so this makes them readable.
fn decode_pdf_text(bytes: &[u8]) -> String {
    if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
        let units: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect();
        String::from_utf16_lossy(&units)
    } else {
        bytes.iter().map(|&b| b as char).collect()
    }
}

/// Read a field dict's partial `/T` name (used to decide whether a kid is itself a
/// field or just a widget annotation).
fn partial_name(doc: &Document, id: ObjectId) -> Option<String> {
    doc.get_dictionary(id)
        .ok()?
        .get(b"T")
        .ok()?
        .as_str()
        .ok()
        .map(decode_pdf_text)
}

/// Decode a `/V` (or array element) value object into a display string. Text
/// strings are decoded (incl. UTF-16BE); button values are Names (the on-state,
/// e.g. `Yes`/`Off`); multi-select choice values are arrays joined with `; `.
fn object_value(doc: &Document, obj: &Object) -> String {
    match obj {
        Object::String(bytes, _) => decode_pdf_text(bytes),
        Object::Name(n) => String::from_utf8_lossy(n).to_string(),
        Object::Array(items) => items
            .iter()
            .map(|o| object_value(doc, o))
            .collect::<Vec<_>>()
            .join("; "),
        Object::Reference(id) => doc
            .get_object(*id)
            .map(|o| object_value(doc, o))
            .unwrap_or_default(),
        Object::Boolean(b) => b.to_string(),
        Object::Integer(i) => i.to_string(),
        Object::Real(f) => f.to_string(),
        _ => String::new(),
    }
}

/// Map `/FT` + `/Ff` to a friendly field type.
fn classify(ft: Option<&str>, ff: i64) -> String {
    match ft {
        Some("Tx") => "text".to_string(),
        Some("Btn") => {
            if ff & FF_PUSHBUTTON != 0 {
                "pushbutton".to_string()
            } else if ff & FF_RADIO != 0 {
                "radio".to_string()
            } else {
                "checkbox".to_string()
            }
        }
        Some("Ch") => {
            if ff & FF_COMBO != 0 {
                "dropdown".to_string()
            } else {
                "listbox".to_string()
            }
        }
        Some("Sig") => "signature".to_string(),
        Some(other) => other.to_string(),
        None => "unknown".to_string(),
    }
}

/// Collect the AcroForm root field object ids from the catalog. Errors when the
/// PDF has no `/AcroForm` (it is not a fillable form).
fn acroform_field_ids(doc: &Document) -> Result<Vec<ObjectId>, String> {
    let catalog = doc.catalog().map_err(|e| format!("no document catalog: {e}"))?;
    let af = catalog
        .get(b"AcroForm")
        .map_err(|_| "this PDF has no AcroForm (it is not a fillable form)".to_string())?;
    let af_dict = match af {
        Object::Reference(id) => doc
            .get_dictionary(*id)
            .map_err(|e| format!("bad AcroForm reference: {e}"))?,
        Object::Dictionary(d) => d,
        _ => return Err("AcroForm is not a dictionary".to_string()),
    };
    Ok(af_dict
        .get(b"Fields")
        .ok()
        .and_then(|o| o.as_array().ok())
        .map(|a| a.iter().filter_map(|o| o.as_reference().ok()).collect())
        .unwrap_or_default())
}

/// Read every terminal AcroForm field's name / type / value from `pdf`.
///
/// - `bytes` — the raw PDF file.
///
/// Returns the fields in document (tree) order. `/FT` and `/Ff` are inherited from
/// ancestor fields when a terminal doesn't set them. An AcroForm with zero fields
/// returns an empty vec (not an error); a PDF with no AcroForm returns `Err`.
pub fn extract_form_data(bytes: &[u8]) -> Result<Vec<FormField>, String> {
    let doc = Document::load_mem(bytes).map_err(|e| format!("failed to parse PDF: {e}"))?;
    let roots = acroform_field_ids(&doc)?;

    let mut out = Vec::new();
    // (id, name-prefix, inherited /FT, inherited /Ff) — iterative DFS.
    let mut stack: Vec<(ObjectId, String, Option<String>, i64)> = roots
        .iter()
        .rev()
        .map(|id| (*id, String::new(), None, 0i64))
        .collect();
    let mut guard = 0usize;
    while let Some((id, prefix, inh_ft, inh_ff)) = stack.pop() {
        guard += 1;
        if guard > MAX_FIELDS {
            break;
        }
        let Ok(d) = doc.get_dictionary(id) else {
            continue;
        };
        let part = d
            .get(b"T")
            .ok()
            .and_then(|o| o.as_str().ok())
            .map(decode_pdf_text);
        let full = match &part {
            Some(t) if prefix.is_empty() => t.clone(),
            Some(t) => format!("{prefix}.{t}"),
            None => prefix.clone(),
        };
        let ft = d
            .get(b"FT")
            .ok()
            .and_then(|o| o.as_name().ok())
            .map(|s| String::from_utf8_lossy(s).to_string())
            .or_else(|| inh_ft.clone());
        let ff = match d.get(b"Ff") {
            Ok(Object::Integer(i)) => *i,
            _ => inh_ff,
        };

        // Child *fields* = kid refs that carry their own /T. Kids without a /T are
        // widget annotations (the field itself is terminal).
        let child_fields: Vec<ObjectId> = d
            .get(b"Kids")
            .ok()
            .and_then(|o| o.as_array().ok())
            .map(|a| {
                a.iter()
                    .filter_map(|o| o.as_reference().ok())
                    .filter(|kid| partial_name(&doc, *kid).is_some())
                    .collect()
            })
            .unwrap_or_default();

        if child_fields.is_empty() {
            let value = d
                .get(b"V")
                .ok()
                .map(|o| object_value(&doc, o))
                .unwrap_or_default();
            let filled = !value.is_empty() && value != "Off";
            out.push(FormField {
                name: full,
                field_type: classify(ft.as_deref(), ff),
                value,
                filled,
            });
        } else {
            for k in child_fields.into_iter().rev() {
                stack.push((k, full.clone(), ft.clone(), ff));
            }
        }
    }
    Ok(out)
}

/// Serialize fields as a pretty JSON array of `{name, type, value}` objects.
pub fn to_json(fields: &[FormField]) -> String {
    let arr: Vec<serde_json::Value> = fields
        .iter()
        .map(|f| {
            serde_json::json!({
                "name": f.name,
                "type": f.field_type,
                "value": f.value,
            })
        })
        .collect();
    serde_json::to_string_pretty(&serde_json::Value::Array(arr)).unwrap_or_else(|_| "[]".to_string())
}

/// Quote a CSV cell per RFC 4180 when it contains the delimiter, a quote, or a
/// newline; doubling embedded quotes.
fn csv_escape(s: &str, delim: char) -> String {
    if s.contains(delim) || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Serialize fields as CSV/TSV with a `name,type,value` header and CRLF rows.
pub fn to_csv(fields: &[FormField], delim: char) -> String {
    let sep = delim.to_string();
    let row = |cells: &[&str]| -> String {
        cells
            .iter()
            .map(|c| csv_escape(c, delim))
            .collect::<Vec<_>>()
            .join(&sep)
            + "\r\n"
    };
    let mut out = row(&["name", "type", "value"]);
    for f in fields {
        out.push_str(&row(&[&f.name, &f.field_type, &f.value]));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::{dictionary, StringFormat};

    /// Build a fillable PDF: a filled text field, a checked checkbox, an
    /// unchecked checkbox, and a signature field. Returns the bytes.
    fn form_pdf() -> Vec<u8> {
        let mut doc = Document::with_version("1.5");
        let name = doc.add_object(dictionary! {
            "T" => Object::string_literal("full_name"),
            "FT" => Object::Name(b"Tx".to_vec()),
            "V" => Object::string_literal("Ada Lovelace"),
        });
        let agree = doc.add_object(dictionary! {
            "T" => Object::string_literal("agree"),
            "FT" => Object::Name(b"Btn".to_vec()),
            "V" => Object::Name(b"Yes".to_vec()),
            "AS" => Object::Name(b"Yes".to_vec()),
        });
        let subscribe = doc.add_object(dictionary! {
            "T" => Object::string_literal("subscribe"),
            "FT" => Object::Name(b"Btn".to_vec()),
            "V" => Object::Name(b"Off".to_vec()),
        });
        let sig = doc.add_object(dictionary! {
            "T" => Object::string_literal("signature"),
            "FT" => Object::Name(b"Sig".to_vec()),
        });
        let acro = doc.add_object(dictionary! {
            "Fields" => vec![
                Object::Reference(name),
                Object::Reference(agree),
                Object::Reference(subscribe),
                Object::Reference(sig),
            ],
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

    #[test]
    fn extracts_names_types_and_values() {
        let pdf = form_pdf();
        let fields = extract_form_data(&pdf).unwrap();
        assert_eq!(fields.len(), 4);

        let name = &fields[0];
        assert_eq!(name.name, "full_name");
        assert_eq!(name.field_type, "text");
        assert_eq!(name.value, "Ada Lovelace");
        assert!(name.filled);

        let agree = &fields[1];
        assert_eq!(agree.field_type, "checkbox");
        assert_eq!(agree.value, "Yes");
        assert!(agree.filled);

        // An unchecked checkbox reads its Off state and counts as not filled.
        let subscribe = &fields[2];
        assert_eq!(subscribe.value, "Off");
        assert!(!subscribe.filled);

        // A field with no /V has an empty value and is not filled.
        let sig = &fields[3];
        assert_eq!(sig.field_type, "signature");
        assert_eq!(sig.value, "");
        assert!(!sig.filled);
    }

    #[test]
    fn rejects_non_pdf_bytes() {
        let err = extract_form_data(b"this is plainly not a pdf").unwrap_err();
        assert!(err.contains("failed to parse PDF"), "got: {err}");
    }

    #[test]
    fn errors_without_acroform() {
        let mut doc = Document::with_version("1.5");
        let pages = doc.add_object(dictionary! {
            "Type" => "Pages", "Kids" => Vec::<Object>::new(), "Count" => 0i64,
        });
        let catalog = doc.add_object(dictionary! {
            "Type" => "Catalog", "Pages" => Object::Reference(pages),
        });
        doc.trailer.set("Root", Object::Reference(catalog));
        let mut out = Vec::new();
        doc.save_to(&mut out).unwrap();
        let err = extract_form_data(&out).unwrap_err();
        assert!(err.contains("no AcroForm"), "got: {err}");
    }

    #[test]
    fn nested_field_gets_dotted_name() {
        let mut doc = Document::with_version("1.5");
        let leaf = doc.add_object(dictionary! {
            "T" => Object::string_literal("first"),
            "FT" => Object::Name(b"Tx".to_vec()),
            "V" => Object::string_literal("Ada"),
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

        let fields = extract_form_data(&pdf).unwrap();
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].name, "form1.first");
        assert_eq!(fields[0].value, "Ada");
    }

    #[test]
    fn radio_and_dropdown_classified_via_flags() {
        let mut doc = Document::with_version("1.5");
        let radio = doc.add_object(dictionary! {
            "T" => Object::string_literal("plan"),
            "FT" => Object::Name(b"Btn".to_vec()),
            "Ff" => Object::Integer(FF_RADIO),
            "V" => Object::Name(b"pro".to_vec()),
        });
        let combo = doc.add_object(dictionary! {
            "T" => Object::string_literal("country"),
            "FT" => Object::Name(b"Ch".to_vec()),
            "Ff" => Object::Integer(FF_COMBO),
            "V" => Object::string_literal("US"),
        });
        let acro = doc.add_object(dictionary! {
            "Fields" => vec![Object::Reference(radio), Object::Reference(combo)],
        });
        let pages = doc.add_object(dictionary! { "Type" => "Pages", "Kids" => Vec::<Object>::new(), "Count" => 0i64 });
        let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => Object::Reference(pages), "AcroForm" => Object::Reference(acro) });
        doc.trailer.set("Root", Object::Reference(catalog));
        let mut pdf = Vec::new();
        doc.save_to(&mut pdf).unwrap();

        let fields = extract_form_data(&pdf).unwrap();
        assert_eq!(fields[0].field_type, "radio");
        assert_eq!(fields[0].value, "pro");
        assert_eq!(fields[1].field_type, "dropdown");
        assert_eq!(fields[1].value, "US");
    }

    #[test]
    fn decodes_utf16be_names_and_values() {
        let mut doc = Document::with_version("1.5");
        let mut t = vec![0xFE, 0xFF];
        for u in "f1_01".encode_utf16() {
            t.extend_from_slice(&u.to_be_bytes());
        }
        let mut v = vec![0xFE, 0xFF];
        for u in "café".encode_utf16() {
            v.extend_from_slice(&u.to_be_bytes());
        }
        let f = doc.add_object(dictionary! {
            "T" => Object::String(t, StringFormat::Literal),
            "FT" => Object::Name(b"Tx".to_vec()),
            "V" => Object::String(v, StringFormat::Literal),
        });
        let acro = doc.add_object(dictionary! { "Fields" => vec![Object::Reference(f)] });
        let pages = doc.add_object(dictionary! { "Type" => "Pages", "Kids" => Vec::<Object>::new(), "Count" => 0i64 });
        let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => Object::Reference(pages), "AcroForm" => Object::Reference(acro) });
        doc.trailer.set("Root", Object::Reference(catalog));
        let mut pdf = Vec::new();
        doc.save_to(&mut pdf).unwrap();

        let fields = extract_form_data(&pdf).unwrap();
        assert_eq!(fields[0].name, "f1_01");
        assert_eq!(fields[0].value, "café");
    }

    #[test]
    fn to_json_shape() {
        let fields = vec![FormField {
            name: "n".into(),
            field_type: "text".into(),
            value: "v".into(),
            filled: true,
        }];
        let v: serde_json::Value = serde_json::from_str(&to_json(&fields)).unwrap();
        assert_eq!(v[0]["name"], "n");
        assert_eq!(v[0]["type"], "text");
        assert_eq!(v[0]["value"], "v");
    }

    #[test]
    fn to_csv_quotes_and_uses_delimiter() {
        let fields = vec![FormField {
            name: "full name".into(),
            field_type: "text".into(),
            value: "Doe, Jane".into(),
            filled: true,
        }];
        let csv = to_csv(&fields, ',');
        assert_eq!(csv, "name,type,value\r\nfull name,text,\"Doe, Jane\"\r\n");
        // Semicolon delimiter leaves the comma-containing value unquoted.
        let ssv = to_csv(&fields, ';');
        assert_eq!(ssv, "name;type;value\r\nfull name;text;Doe, Jane\r\n");
    }
}
