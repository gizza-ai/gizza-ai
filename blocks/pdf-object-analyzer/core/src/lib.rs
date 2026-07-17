//! gizza-ai/pdf-object-analyzer core — walk a PDF's object tree and surface the
//! structural indicators used in malicious-document triage: automatic actions
//! that fire on open (`/OpenAction`, `/AA`), embedded JavaScript (`/JS`,
//! `/JavaScript`), `/Launch` actions that can start an external program,
//! embedded files, form/obfuscation features (`/AcroForm`, `/XFA`, `/ObjStm`,
//! `/JBIG2Decode`), and outbound links (`/URI`, `/SubmitForm`, `/GoToR`).
//!
//! This is *static structural* analysis — it reports what the document contains
//! and assigns a coarse risk level from the presence of auto-executing scripts
//! and launch triggers. It does NOT execute anything, sandbox the PDF, or make a
//! malware verdict; every indicator is reported with the object ids it was found
//! in so an analyst can dig in with a dedicated tool.
//!
//! No wafer/wasm-bindgen deps: compiles natively for unit tests and to
//! `wasm32-wasip1` (the `wafer build` / `cargo build --target wasm32-wasip1`
//! target). `lopdf` (default-features=false) is the only engine dep.

use lopdf::{Dictionary, Document, Object};
use serde::Serialize;
use std::collections::BTreeSet;

/// One structural indicator (a `/Key` we flagged), with the object ids that
/// carried it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Indicator {
    /// The PDF name, e.g. `/OpenAction`.
    pub key: String,
    /// Risk category: `code-execution`, `auto-execution`, `scripting`,
    /// `embedded-file`, `network`, `form`, `obfuscation`, or `encryption`.
    pub category: String,
    /// Number of distinct objects the indicator was found in.
    pub count: usize,
    /// Plain-language explanation of why this matters.
    pub note: String,
    /// Object references (`"12 0"`) that carried the indicator (capped at 32).
    pub object_ids: Vec<String>,
}

/// The full structural analysis of one PDF.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct Analysis {
    /// The PDF header version, e.g. `1.7`.
    pub pdf_version: String,
    /// Total number of indirect objects.
    pub object_count: usize,
    /// Number of stream objects.
    pub stream_count: usize,
    /// Number of pages.
    pub page_count: usize,
    /// True when the document is encrypted (`/Encrypt` in the trailer).
    pub encrypted: bool,
    /// Number of `/ObjStm` object streams (compressed object containers, a
    /// common obfuscation vehicle).
    pub object_stream_count: usize,
    /// True when a document-level `/OpenAction` is present.
    pub open_action: bool,
    /// What the `/OpenAction` triggers, if determinable (e.g. `JavaScript`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open_action_kind: Option<String>,
    /// True when any `/AA` (additional-actions) dictionary is present.
    pub additional_actions: bool,
    /// True when the document runs something automatically on open
    /// (`/OpenAction` or `/AA`).
    pub auto_execution: bool,
    /// Every flagged indicator, ordered most-severe first.
    pub indicators: Vec<Indicator>,
    /// Extracted JavaScript source snippets (each capped at 800 chars).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub javascript: Vec<String>,
    /// Names of embedded files.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub embedded_files: Vec<String>,
    /// Targets of `/Launch` actions (program/file names).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub launch_targets: Vec<String>,
    /// URLs referenced by `/URI` actions.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub uri_targets: Vec<String>,
    /// Coarse risk level: `none`, `low`, `medium`, or `high`.
    pub risk_level: String,
    /// Human-readable reasons behind the risk level.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub risk_reasons: Vec<String>,
}

impl Analysis {
    /// Drop the bulky evidence fields (extracted JavaScript source and the
    /// per-indicator object-id lists), keeping only the structural summary,
    /// indicator keys/counts, risk level, and target lists. Backs the
    /// `detail=summary` report mode.
    pub fn summarized(mut self) -> Analysis {
        self.javascript.clear();
        for ind in &mut self.indicators {
            ind.object_ids.clear();
        }
        self
    }
}

// --- indicator notes (kept as constants so the JSON copy is consistent) ---
const OPENACTION_NOTE: &str = "runs an action automatically when the document is opened";
const AA_NOTE: &str = "additional actions fire automatically on document/page/field events";
const JS_NOTE: &str = "holds JavaScript source the viewer may execute";
const JAVASCRIPT_NOTE: &str = "an action or names entry that executes JavaScript";
const LAUNCH_NOTE: &str = "launches an external application or file — high abuse potential";
const URI_NOTE: &str = "an action that opens an external URL";
const SUBMIT_NOTE: &str = "submits form data to a remote URL";
const GOTOR_NOTE: &str = "navigates to a remote (external) document";
const GOTOE_NOTE: &str = "navigates to a document embedded inside this PDF";
const IMPORTDATA_NOTE: &str = "imports form data, possibly from an external file";
const ACROFORM_NOTE: &str = "interactive form; can carry scripts or auto-submit actions";
const XFA_NOTE: &str = "XML Forms Architecture — a historically exploited attack surface";
const EMBEDDED_NOTE: &str = "a file stored inside the PDF (a potential dropped payload)";
const OBJSTM_NOTE: &str =
    "object stream — packs many objects into one compressed stream, often used to hide content";
const JBIG2_NOTE: &str = "JBIG2 image filter associated with historical decoder exploits";
const RICHMEDIA_NOTE: &str = "embedded rich media (Flash/3D/video) annotation";
const ENCRYPT_NOTE: &str = "the document is encrypted; string and stream contents are obscured";

/// One accumulating indicator during the walk.
struct IndTmp {
    category: &'static str,
    note: &'static str,
    seen: BTreeSet<String>,
}

#[derive(Default)]
struct Acc {
    map: std::collections::BTreeMap<String, IndTmp>,
}

impl Acc {
    fn hit(&mut self, key: &str, category: &'static str, note: &'static str, idstr: &str) {
        let e = self.map.entry(key.to_string()).or_insert(IndTmp {
            category,
            note,
            seen: BTreeSet::new(),
        });
        e.seen.insert(idstr.to_string());
    }
    fn has(&self, key: &str) -> bool {
        self.map.get(key).is_some_and(|e| !e.seen.is_empty())
    }
}

fn cat_rank(c: &str) -> u8 {
    match c {
        "code-execution" => 0,
        "auto-execution" => 1,
        "scripting" => 2,
        "embedded-file" => 3,
        "network" => 4,
        "form" => 5,
        "obfuscation" => 6,
        "encryption" => 7,
        _ => 8,
    }
}

/// Analyze a PDF's structure.
///
/// Returns `Err` only when the bytes don't parse as a PDF. A perfectly benign
/// PDF parses fine and yields an empty `indicators` list with `risk_level` =
/// `none`.
pub fn analyze(bytes: &[u8]) -> Result<Analysis, String> {
    let doc = Document::load_mem(bytes).map_err(|e| format!("failed to parse PDF: {e}"))?;

    let mut a = Analysis {
        pdf_version: doc.version.clone(),
        object_count: doc.objects.len(),
        page_count: doc.get_pages().len(),
        encrypted: doc.trailer.has(b"Encrypt"),
        ..Default::default()
    };

    let mut acc = Acc::default();

    for (id, obj) in &doc.objects {
        let idstr = format!("{} {}", id.0, id.1);
        let d = match obj {
            Object::Dictionary(d) => Some(d),
            Object::Stream(s) => {
                a.stream_count += 1;
                Some(&s.dict)
            }
            _ => None,
        };
        let Some(d) = d else { continue };

        // --- key-based indicators ---
        if d.has(b"OpenAction") {
            acc.hit("/OpenAction", "auto-execution", OPENACTION_NOTE, &idstr);
        }
        if d.has(b"AA") {
            acc.hit("/AA", "auto-execution", AA_NOTE, &idstr);
            a.additional_actions = true;
        }
        if d.has(b"JS") {
            acc.hit("/JS", "scripting", JS_NOTE, &idstr);
            if let Some(js) = read_js(&doc, d) {
                push_unique(&mut a.javascript, snippet(&js));
            }
        }
        if d.has(b"JavaScript") {
            acc.hit("/JavaScript", "scripting", JAVASCRIPT_NOTE, &idstr);
        }
        if d.has(b"AcroForm") {
            acc.hit("/AcroForm", "form", ACROFORM_NOTE, &idstr);
        }
        if d.has(b"XFA") {
            acc.hit("/XFA", "form", XFA_NOTE, &idstr);
        }

        // --- action subtype via /S ---
        if let Some(s) = name_val(d, b"S") {
            match s.as_slice() {
                b"JavaScript" => {
                    acc.hit("/JavaScript", "scripting", JAVASCRIPT_NOTE, &idstr);
                    if let Some(js) = read_js(&doc, d) {
                        push_unique(&mut a.javascript, snippet(&js));
                    }
                }
                b"Launch" => {
                    acc.hit("/Launch", "code-execution", LAUNCH_NOTE, &idstr);
                    if let Some(t) = read_launch_target(&doc, d) {
                        push_unique(&mut a.launch_targets, t);
                    }
                }
                b"URI" => {
                    acc.hit("/URI", "network", URI_NOTE, &idstr);
                    if let Some(u) = dict_string(d, b"URI") {
                        push_unique(&mut a.uri_targets, u);
                    }
                }
                b"SubmitForm" => acc.hit("/SubmitForm", "network", SUBMIT_NOTE, &idstr),
                b"GoToR" => acc.hit("/GoToR", "network", GOTOR_NOTE, &idstr),
                b"GoToE" => acc.hit("/GoToE", "embedded-file", GOTOE_NOTE, &idstr),
                b"ImportData" => acc.hit("/ImportData", "form", IMPORTDATA_NOTE, &idstr),
                _ => {}
            }
        }

        // --- type value ---
        if let Some(t) = name_val(d, b"Type") {
            match t.as_slice() {
                b"EmbeddedFile" => acc.hit("/EmbeddedFile", "embedded-file", EMBEDDED_NOTE, &idstr),
                b"ObjStm" => {
                    acc.hit("/ObjStm", "obfuscation", OBJSTM_NOTE, &idstr);
                    a.object_stream_count += 1;
                }
                b"Filespec" => {
                    if let Some(f) = filespec_name(d) {
                        push_unique(&mut a.embedded_files, f);
                    }
                }
                _ => {}
            }
        }
        // A file specification may omit /Type but always carries /EF when it
        // wraps an embedded stream.
        if d.has(b"EF") {
            if let Some(f) = filespec_name(d) {
                push_unique(&mut a.embedded_files, f);
            }
        }

        // --- subtype value (annotations) ---
        if let Some(st) = name_val(d, b"Subtype") {
            if st.as_slice() == b"RichMedia" {
                acc.hit("/RichMedia", "embedded-file", RICHMEDIA_NOTE, &idstr);
            }
        }

        // --- stream filters ---
        for f in filter_names(d) {
            if f == b"JBIG2Decode" {
                acc.hit("/JBIG2Decode", "obfuscation", JBIG2_NOTE, &idstr);
            }
        }
    }

    if a.encrypted {
        acc.hit("/Encrypt", "encryption", ENCRYPT_NOTE, "trailer");
    }

    // --- /OpenAction kind (from the catalog) ---
    if let Ok(cat) = doc.catalog() {
        if let Ok(oa) = cat.get(b"OpenAction") {
            a.open_action = true;
            a.open_action_kind = match deref(&doc, oa) {
                Some(Object::Array(_)) => Some("GoTo (navigate to a destination)".to_string()),
                Some(Object::Dictionary(d)) => name_val(d, b"S").map(|s| action_kind_label(&s)),
                _ => None,
            };
        }
    }

    a.auto_execution = a.open_action || a.additional_actions;

    // --- build the ordered indicator list ---
    let mut indicators: Vec<Indicator> = acc
        .map
        .iter()
        .map(|(key, t)| Indicator {
            key: key.clone(),
            category: t.category.to_string(),
            count: t.seen.len(),
            note: t.note.to_string(),
            object_ids: t.seen.iter().take(32).cloned().collect(),
        })
        .collect();
    indicators.sort_by(|x, y| {
        cat_rank(&x.category)
            .cmp(&cat_rank(&y.category))
            .then_with(|| x.key.cmp(&y.key))
    });

    // --- risk scoring ---
    let has_js = acc.has("/JS") || acc.has("/JavaScript");
    let has_launch = acc.has("/Launch");
    let has_embed = acc.has("/EmbeddedFile");
    let has_submit = acc.has("/SubmitForm");
    let has_uri = !a.uri_targets.is_empty();

    let mut reasons: Vec<String> = Vec::new();
    if has_launch {
        reasons.push("contains a /Launch action that can start an external program or file".into());
    }
    if has_js && a.auto_execution {
        reasons.push(
            "embedded JavaScript is wired to run automatically on open (/OpenAction or /AA)".into(),
        );
    } else if has_js {
        reasons.push("contains embedded JavaScript".into());
    } else if a.auto_execution {
        reasons.push("runs an automatic action on open (/OpenAction or /AA)".into());
    }
    if has_embed {
        reasons.push("carries one or more embedded files".into());
    }
    if has_submit {
        reasons.push("can submit form data to a remote URL".into());
    } else if has_uri {
        reasons.push("links out to external URLs".into());
    }

    let level = if has_launch || (has_js && a.auto_execution) {
        "high"
    } else if has_js || has_embed || a.auto_execution || has_submit {
        "medium"
    } else if !indicators.is_empty() {
        "low"
    } else {
        "none"
    };
    if level != "none" && reasons.is_empty() {
        reasons.push(
            "contains interactive or structural features worth noting (forms, object streams, or encryption)"
                .into(),
        );
    }

    a.indicators = indicators;
    a.risk_level = level.to_string();
    a.risk_reasons = reasons;
    Ok(a)
}

fn action_kind_label(s: &[u8]) -> String {
    String::from_utf8_lossy(s).into_owned()
}

/// Follow one level of indirection.
fn deref<'a>(doc: &'a Document, o: &'a Object) -> Option<&'a Object> {
    match o {
        Object::Reference(id) => doc.get_object(*id).ok(),
        other => Some(other),
    }
}

/// Read the name value of a dictionary key (e.g. `/S`).
fn name_val(d: &Dictionary, key: &[u8]) -> Option<Vec<u8>> {
    match d.get(key) {
        Ok(Object::Name(n)) => Some(n.clone()),
        _ => None,
    }
}

/// `/Filter` may be a single name or an array of names.
fn filter_names(d: &Dictionary) -> Vec<Vec<u8>> {
    match d.get(b"Filter") {
        Ok(Object::Name(n)) => vec![n.clone()],
        Ok(Object::Array(a)) => a
            .iter()
            .filter_map(|o| match o {
                Object::Name(n) => Some(n.clone()),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Read a PDF text/name value as a String.
fn dict_string(d: &Dictionary, key: &[u8]) -> Option<String> {
    match d.get(key) {
        Ok(Object::String(bytes, _)) => Some(decode_pdf_text(bytes)),
        Ok(Object::Name(bytes)) => Some(String::from_utf8_lossy(bytes).into_owned()),
        _ => None,
    }
}

/// A file specification's display name (`/UF` preferred, else `/F`).
fn filespec_name(d: &Dictionary) -> Option<String> {
    dict_string(d, b"UF").or_else(|| dict_string(d, b"F"))
}

/// The target of a `/Launch` action: a bare `/F` string, a `/F` file-spec
/// dictionary, or the Windows-specific `/Win << /F … >>`.
fn read_launch_target(doc: &Document, d: &Dictionary) -> Option<String> {
    if let Some(f) = dict_string(d, b"F") {
        return Some(f);
    }
    if let Ok(f) = d.get(b"F") {
        if let Some(Object::Dictionary(fd)) = deref(doc, f) {
            if let Some(n) = filespec_name(fd) {
                return Some(n);
            }
        }
    }
    if let Ok(w) = d.get(b"Win") {
        if let Some(Object::Dictionary(wd)) = deref(doc, w) {
            return dict_string(wd, b"F");
        }
    }
    None
}

/// Read the JavaScript source held under `/JS` (a literal string or a stream).
fn read_js(doc: &Document, d: &Dictionary) -> Option<String> {
    let v = deref(doc, d.get(b"JS").ok()?)?;
    match v {
        Object::String(bytes, _) => Some(decode_pdf_text(bytes)),
        Object::Stream(s) => {
            let content = s
                .decompressed_content()
                .unwrap_or_else(|_| s.content.clone());
            Some(String::from_utf8_lossy(&content).into_owned())
        }
        _ => None,
    }
}

/// Decode a PDF text string: UTF-16BE with a BOM, else Latin-1/PDFDoc.
fn decode_pdf_text(bytes: &[u8]) -> String {
    if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
        let u16s: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect();
        String::from_utf16_lossy(&u16s)
    } else {
        bytes.iter().map(|&b| b as char).collect()
    }
}

/// Trim + cap a JavaScript snippet for the report.
fn snippet(s: &str) -> String {
    let s = s.trim();
    if s.chars().count() > 800 {
        let out: String = s.chars().take(800).collect();
        format!("{out} … (truncated)")
    } else {
        s.to_string()
    }
}

/// Push a value into a capped, de-duplicated list.
fn push_unique(v: &mut Vec<String>, s: String) {
    if s.is_empty() || v.len() >= 64 {
        return;
    }
    if !v.iter().any(|e| e == &s) {
        v.push(s);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::StringFormat;

    fn name(s: &str) -> Object {
        Object::Name(s.as_bytes().to_vec())
    }
    fn lit(s: &str) -> Object {
        Object::String(s.as_bytes().to_vec(), StringFormat::Literal)
    }
    fn dict(pairs: Vec<(&str, Object)>) -> Dictionary {
        let mut d = Dictionary::new();
        for (k, v) in pairs {
            d.set(k.as_bytes().to_vec(), v);
        }
        d
    }

    /// A minimal but complete PDF carrying an auto-run JavaScript OpenAction,
    /// an /AA Launch action, and an embedded file.
    fn malicious_pdf() -> Vec<u8> {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let page_id = doc.add_object(Object::Dictionary(dict(vec![
            ("Type", name("Page")),
            ("Parent", Object::Reference(pages_id)),
            (
                "MediaBox",
                Object::Array(vec![
                    Object::Integer(0),
                    Object::Integer(0),
                    Object::Integer(612),
                    Object::Integer(792),
                ]),
            ),
        ])));
        doc.objects.insert(
            pages_id,
            Object::Dictionary(dict(vec![
                ("Type", name("Pages")),
                ("Kids", Object::Array(vec![Object::Reference(page_id)])),
                ("Count", Object::Integer(1)),
            ])),
        );
        let js_action = doc.add_object(Object::Dictionary(dict(vec![
            ("Type", name("Action")),
            ("S", name("JavaScript")),
            ("JS", lit("app.alert('pwn-acme');")),
        ])));
        let launch_action = doc.add_object(Object::Dictionary(dict(vec![
            ("Type", name("Action")),
            ("S", name("Launch")),
            ("F", lit("cmd.exe")),
        ])));
        let ef_stream = doc.add_object(Object::Stream(lopdf::Stream::new(
            dict(vec![("Type", name("EmbeddedFile"))]),
            b"payload-bytes".to_vec(),
        )));
        doc.add_object(Object::Dictionary(dict(vec![
            ("Type", name("Filespec")),
            ("F", lit("evil.exe")),
            (
                "EF",
                Object::Dictionary(dict(vec![("F", Object::Reference(ef_stream))])),
            ),
        ])));
        let catalog_id = doc.add_object(Object::Dictionary(dict(vec![
            ("Type", name("Catalog")),
            ("Pages", Object::Reference(pages_id)),
            ("OpenAction", Object::Reference(js_action)),
            (
                "AA",
                Object::Dictionary(dict(vec![("WC", Object::Reference(launch_action))])),
            ),
        ])));
        doc.trailer.set(b"Root".to_vec(), Object::Reference(catalog_id));
        let mut buf = Vec::new();
        doc.save_to(&mut buf).unwrap();
        buf
    }

    /// A plain, benign one-page PDF with no actions.
    fn benign_pdf() -> Vec<u8> {
        let mut doc = Document::with_version("1.4");
        let pages_id = doc.new_object_id();
        let page_id = doc.add_object(Object::Dictionary(dict(vec![
            ("Type", name("Page")),
            ("Parent", Object::Reference(pages_id)),
        ])));
        doc.objects.insert(
            pages_id,
            Object::Dictionary(dict(vec![
                ("Type", name("Pages")),
                ("Kids", Object::Array(vec![Object::Reference(page_id)])),
                ("Count", Object::Integer(1)),
            ])),
        );
        let catalog_id = doc.add_object(Object::Dictionary(dict(vec![
            ("Type", name("Catalog")),
            ("Pages", Object::Reference(pages_id)),
        ])));
        doc.trailer.set(b"Root".to_vec(), Object::Reference(catalog_id));
        let mut buf = Vec::new();
        doc.save_to(&mut buf).unwrap();
        buf
    }

    #[test]
    fn flags_auto_run_javascript_launch_and_embedded_file() {
        let a = analyze(&malicious_pdf()).expect("valid pdf");
        assert_eq!(a.risk_level, "high", "auto-run JS + /Launch ⇒ high");
        assert!(a.auto_execution);
        assert!(a.open_action);
        assert_eq!(a.open_action_kind.as_deref(), Some("JavaScript"));
        assert!(a.additional_actions);

        let keys: Vec<&str> = a.indicators.iter().map(|i| i.key.as_str()).collect();
        for want in [
            "/OpenAction",
            "/AA",
            "/JS",
            "/JavaScript",
            "/Launch",
            "/EmbeddedFile",
        ] {
            assert!(keys.contains(&want), "missing indicator {want}; got {keys:?}");
        }
        // most-severe (code-execution) sorts first
        assert_eq!(a.indicators[0].key, "/Launch");

        assert!(a.javascript.iter().any(|j| j.contains("pwn-acme")));
        assert!(a.launch_targets.iter().any(|t| t == "cmd.exe"));
        assert!(a.embedded_files.iter().any(|f| f == "evil.exe"));
        assert!(a.page_count >= 1);
    }

    #[test]
    fn benign_pdf_is_clean() {
        let a = analyze(&benign_pdf()).expect("valid pdf");
        assert_eq!(a.risk_level, "none");
        assert!(a.indicators.is_empty());
        assert!(!a.auto_execution);
        assert!(a.javascript.is_empty());
        assert_eq!(a.page_count, 1);
    }

    #[test]
    fn non_pdf_bytes_error() {
        let err = analyze(b"this is definitely not a pdf file").unwrap_err();
        assert!(err.contains("failed to parse PDF"), "got: {err}");
    }

    #[test]
    fn summarized_drops_evidence_but_keeps_indicators() {
        let a = analyze(&malicious_pdf()).expect("valid pdf").summarized();
        // structural summary + risk survive
        assert_eq!(a.risk_level, "high");
        assert!(!a.indicators.is_empty());
        // bulky evidence is stripped
        assert!(a.javascript.is_empty(), "js snippets dropped in summary mode");
        assert!(
            a.indicators.iter().all(|i| i.object_ids.is_empty()),
            "object-id lists dropped in summary mode"
        );
        // counts are retained (they came from the object-id sets, not the list)
        assert!(a.indicators.iter().any(|i| i.count >= 1));
        // launch/embedded target lists are NOT evidence-stripped
        assert!(a.launch_targets.iter().any(|t| t == "cmd.exe"));
    }
}
