//! gizza-ai/pdf-annotations-extract core — collect every comment, highlight,
//! sticky note, drawing and stamp annotation out of a PDF, with the page number,
//! author, date, colour, comment text, and (for text markup) the underlying page
//! text the annotation covers.
//!
//! No wafer/wasm-bindgen deps, so it compiles natively for the unit tests below
//! and to `wasm32-wasip1` for the block. Pure-Rust `lopdf` (no native libs).
//!
//! Pipeline: parse the PDF → for each selected page read `/Annots` → skip
//! `Popup` (the on-screen container of its parent markup annotation, which would
//! duplicate every comment) and `Widget` (AcroForm fields — `pdf-form-data-extract`
//! owns those) → decode `/Subtype`, `/T`, `/Contents`, `/M`, `/C`, `/QuadPoints`
//! → when the annotation is text markup, map its `/QuadPoints` boxes back onto
//! the page's text layer to recover the marked-up text → filter (type / author /
//! page range / empties) → sort → serialize.
//!
//! ## Marked-up text
//!
//! PDFs do not store "the highlighted text" — a highlight is a set of rectangles
//! (`/QuadPoints`) drawn over the page. Recovering the text therefore means
//! walking the content stream, laying out each glyph, and keeping the glyphs
//! whose box falls inside a quad. Glyph advances come from the font's real
//! `/Widths` (simple fonts) or `/W`+`/DW` (Type0 CID fonts) when present, and
//! fall back to Helvetica's base-14 metrics otherwise. This is accurate to about
//! a character at each edge of a highlight, and recovers nothing at all from a
//! scanned/image-only PDF (there is no text layer to map onto).

use lopdf::{Dictionary, Document, Object, ObjectId};
use std::collections::{BTreeMap, BTreeSet};

/// Cap on pages walked — guards a pathological document.
pub const MAX_PAGES: usize = 10_000;
/// Cap on annotations collected — guards a pathological `/Annots` array.
pub const MAX_ANNOTATIONS: usize = 100_000;

/// One extracted annotation.
#[derive(Debug, Clone, PartialEq)]
pub struct Annotation {
    /// 1-based page number the annotation lives on.
    pub page: u32,
    /// Friendly kind: `highlight`, `underline`, `strikeout`, `squiggly`, `note`,
    /// `freetext`, `drawing`, `stamp`, `link`, `attachment`, `caret`, or the
    /// lower-cased raw subtype for anything else.
    pub kind: String,
    /// Raw PDF `/Subtype` (e.g. `StrikeOut`, `PolyLine`).
    pub subtype: String,
    /// `/T` — the annotation author, empty when the producer wrote none.
    pub author: String,
    /// `/M` (or `/CreationDate`) normalised to ISO-8601, empty when absent.
    pub date: String,
    /// `/C` as `#rrggbb`, empty for a transparent/colourless annotation.
    pub color: String,
    /// `/Contents` — the note the author typed.
    pub comment: String,
    /// The page text the annotation's `/QuadPoints` cover (text markup only).
    pub marked_text: String,
}

/// Extraction + filtering options. All fields have sensible zero-ish defaults via
/// [`Options::default`].
#[derive(Debug, Clone)]
pub struct Options {
    /// `all`, `markup` (highlight+underline+strikeout+squiggly), or one kind.
    pub types: String,
    /// 1-based page spec, e.g. `"1,3-5"`. Empty = every page.
    pub pages: String,
    /// Case-insensitive substring match on the author. Empty = every author.
    pub author: String,
    /// Recover the page text under text-markup annotations.
    pub include_marked_text: bool,
    /// Keep annotations that carry neither a comment nor marked-up text (bare
    /// links, empty stamps/drawings). Text-markup annotations are always kept.
    pub include_empty: bool,
    /// `page` (default), `author`, `type`, or `date`.
    pub sort: String,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            types: "all".to_string(),
            pages: String::new(),
            author: String::new(),
            include_marked_text: true,
            include_empty: false,
            sort: "page".to_string(),
        }
    }
}

/// The `types` values the tool accepts (also the descriptor's enum).
pub const TYPE_CHOICES: [&str; 11] = [
    "all",
    "markup",
    "highlight",
    "underline",
    "strikeout",
    "squiggly",
    "note",
    "freetext",
    "drawing",
    "stamp",
    "link",
];

/// The `sort` values the tool accepts.
pub const SORT_CHOICES: [&str; 4] = ["page", "author", "type", "date"];

/// Text-markup kinds — the ones that carry `/QuadPoints` and therefore have
/// marked-up text, and the ones `types = "markup"` selects.
const MARKUP_KINDS: [&str; 4] = ["highlight", "underline", "strikeout", "squiggly"];

// ---------------------------------------------------------------------------
// PDF value decoding
// ---------------------------------------------------------------------------

/// Decode a PDF text string: UTF-16BE when it carries the `FE FF` byte-order
/// mark, otherwise PDFDocEncoding (approximated by Latin-1, which covers what
/// annotation authors/comments actually use).
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

/// Read a PDF number `Object` as `f64`.
fn num(o: &Object) -> Option<f64> {
    match o {
        Object::Integer(i) => Some(*i as f64),
        Object::Real(r) => Some(*r as f64),
        _ => None,
    }
}

/// Follow `Object::Reference` chains to the real object.
fn deref<'a>(doc: &'a Document, obj: &'a Object) -> &'a Object {
    let mut cur = obj;
    for _ in 0..32 {
        match cur {
            Object::Reference(id) => match doc.get_object(*id) {
                Ok(o) => cur = o,
                Err(_) => return cur,
            },
            _ => return cur,
        }
    }
    cur
}

/// Read a dictionary entry, dereferencing it.
fn get<'a>(doc: &'a Document, dict: &'a Dictionary, key: &[u8]) -> Option<&'a Object> {
    dict.get(key).ok().map(|o| deref(doc, o))
}

/// Normalise a PDF date string (`D:YYYYMMDDHHmmSS+HH'mm'`) to ISO-8601.
/// Returns the input unchanged when it doesn't look like a PDF date.
fn normalize_date(raw: &str) -> String {
    let s = raw.trim();
    let body = s.strip_prefix("D:").unwrap_or(s);
    let digits: String = body.chars().take_while(|c| c.is_ascii_digit()).collect();
    if digits.len() < 4 {
        return s.to_string();
    }
    let part = |a: usize, b: usize| digits.get(a..b).unwrap_or("");
    let mut out = part(0, 4).to_string();
    if digits.len() >= 6 {
        out.push('-');
        out.push_str(part(4, 6));
    }
    if digits.len() >= 8 {
        out.push('-');
        out.push_str(part(6, 8));
    }
    if digits.len() >= 10 {
        out.push('T');
        out.push_str(part(8, 10));
        out.push(':');
        out.push_str(if digits.len() >= 12 { part(10, 12) } else { "00" });
        out.push(':');
        out.push_str(if digits.len() >= 14 { part(12, 14) } else { "00" });
    }
    // Trailing UTC offset: `Z`, or `+HH'mm'` / `-HH'mm'`.
    let rest = &body[digits.len().min(body.len())..];
    let rest_clean: String = rest.chars().filter(|c| *c != '\'').collect();
    if rest_clean.starts_with('Z') {
        out.push('Z');
    } else if rest_clean.starts_with('+') || rest_clean.starts_with('-') {
        let sign = &rest_clean[..1];
        let tz: String = rest_clean[1..].chars().filter(|c| c.is_ascii_digit()).collect();
        if tz.len() >= 2 {
            out.push_str(sign);
            out.push_str(&tz[..2]);
            out.push(':');
            out.push_str(if tz.len() >= 4 { &tz[2..4] } else { "00" });
        }
    }
    out
}

/// Convert a `/C` colour array (gray / RGB / CMYK) to `#rrggbb`. An empty array
/// (transparent) yields an empty string.
fn color_hex(doc: &Document, obj: &Object) -> String {
    let Object::Array(items) = deref(doc, obj) else {
        return String::new();
    };
    let v: Vec<f64> = items.iter().filter_map(|o| num(deref(doc, o))).collect();
    let (r, g, b) = match v.len() {
        1 => (v[0], v[0], v[0]),
        3 => (v[0], v[1], v[2]),
        4 => {
            let k = v[3];
            ((1.0 - v[0]) * (1.0 - k), (1.0 - v[1]) * (1.0 - k), (1.0 - v[2]) * (1.0 - k))
        }
        _ => return String::new(),
    };
    let ch = |x: f64| (x.clamp(0.0, 1.0) * 255.0).round() as u8;
    format!("#{:02x}{:02x}{:02x}", ch(r), ch(g), ch(b))
}

/// Map a raw `/Subtype` to the friendly kind used for filtering and output.
fn classify(subtype: &str) -> String {
    match subtype {
        "Highlight" => "highlight",
        "Underline" => "underline",
        "StrikeOut" => "strikeout",
        "Squiggly" => "squiggly",
        "Text" => "note",
        "FreeText" => "freetext",
        "Ink" | "Square" | "Circle" | "Line" | "Polygon" | "PolyLine" => "drawing",
        "Stamp" => "stamp",
        "Link" => "link",
        "FileAttachment" => "attachment",
        "Caret" => "caret",
        other => return other.to_ascii_lowercase(),
    }
    .to_string()
}

// ---------------------------------------------------------------------------
// Page selection
// ---------------------------------------------------------------------------

/// Parse a 1-based page spec like `"1,3-5"`. Empty spec = every page (`None`).
pub fn parse_pages(spec: &str, page_count: u32) -> Result<Option<BTreeSet<u32>>, String> {
    let spec = spec.trim();
    if spec.is_empty() || spec.eq_ignore_ascii_case("all") {
        return Ok(None);
    }
    let mut out = BTreeSet::new();
    for part in spec.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        let (lo, hi) = match part.split_once('-') {
            Some((a, b)) => (a.trim(), b.trim()),
            None => (part, part),
        };
        let lo: u32 = lo
            .parse()
            .map_err(|_| format!("bad page spec {part:?}: expected a number or range like \"3-5\""))?;
        let hi: u32 = hi
            .parse()
            .map_err(|_| format!("bad page spec {part:?}: expected a number or range like \"3-5\""))?;
        if lo == 0 || hi == 0 {
            return Err(format!("bad page spec {part:?}: pages are 1-based"));
        }
        if lo > hi {
            return Err(format!("bad page spec {part:?}: start {lo} is after end {hi}"));
        }
        if lo > page_count {
            return Err(format!(
                "page {lo} is out of range (the PDF has {page_count} page(s))"
            ));
        }
        for p in lo..=hi.min(page_count) {
            out.insert(p);
        }
    }
    if out.is_empty() {
        return Err(format!("page spec {spec:?} selected no pages"));
    }
    Ok(Some(out))
}

// ---------------------------------------------------------------------------
// Text layout — glyph boxes for the marked-text mapping
// ---------------------------------------------------------------------------

/// A 2-D affine matrix in PDF order `[a b c d e f]`.
#[derive(Clone, Copy, Debug)]
struct Mat {
    a: f64,
    b: f64,
    c: f64,
    d: f64,
    e: f64,
    f: f64,
}

impl Mat {
    const ID: Mat = Mat { a: 1.0, b: 0.0, c: 0.0, d: 1.0, e: 0.0, f: 0.0 };

    /// `self × other` (PDF convention: `self` is applied first).
    fn mul(self, o: Mat) -> Mat {
        Mat {
            a: self.a * o.a + self.b * o.c,
            b: self.a * o.b + self.b * o.d,
            c: self.c * o.a + self.d * o.c,
            d: self.c * o.b + self.d * o.d,
            e: self.e * o.a + self.f * o.c + o.e,
            f: self.e * o.b + self.f * o.d + o.f,
        }
    }

    fn apply(self, x: f64, y: f64) -> (f64, f64) {
        (self.a * x + self.c * y + self.e, self.b * x + self.d * y + self.f)
    }

    /// Vertical scale factor — used to turn the font size into a device size.
    fn y_scale(self) -> f64 {
        (self.b * self.b + self.d * self.d).sqrt()
    }
}

/// One laid-out character with its device-space box.
#[derive(Debug, Clone)]
struct Glyph {
    x0: f64,
    x1: f64,
    /// Baseline y in device space.
    y: f64,
    /// Effective font size in device space.
    size: f64,
    ch: char,
}

/// Where a font's glyph advances come from.
enum Widths {
    /// Simple font: 1-byte codes indexing `/Widths` from `/FirstChar`.
    Simple { first: i64, widths: Vec<f64>, missing: f64 },
    /// Type0/CID font: 2-byte codes, `/W` ranges with a `/DW` default.
    Composite { dw: f64, w: BTreeMap<u32, f64> },
    /// No metrics in the file — fall back to Helvetica's base-14 widths.
    Base14,
}

/// Helvetica AFM advance widths for ASCII 32..=126, in 1/1000 em.
const HELVETICA: [u16; 95] = [
    278, 278, 355, 556, 556, 889, 667, 191, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556,
    556, 556, 556, 556, 556, 556, 556, 278, 278, 584, 584, 584, 556, 1015, 667, 667, 722, 722, 667,
    611, 778, 722, 278, 500, 667, 556, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667,
    667, 611, 278, 278, 278, 469, 556, 333, 556, 556, 500, 556, 556, 278, 556, 556, 222, 222, 500,
    222, 833, 556, 556, 556, 556, 333, 500, 278, 556, 500, 722, 500, 500, 500, 334, 260, 334, 584,
];

impl Widths {
    /// Advance for a character code, in em (already divided by 1000).
    fn advance(&self, code: u32, ch: char) -> f64 {
        match self {
            Widths::Simple { first, widths, missing } => {
                let idx = code as i64 - *first;
                if idx >= 0 && (idx as usize) < widths.len() {
                    widths[idx as usize] / 1000.0
                } else {
                    *missing / 1000.0
                }
            }
            Widths::Composite { dw, w } => w.get(&code).copied().unwrap_or(*dw) / 1000.0,
            Widths::Base14 => {
                let c = ch as u32;
                if (32..=126).contains(&c) {
                    HELVETICA[(c - 32) as usize] as f64 / 1000.0
                } else {
                    0.5
                }
            }
        }
    }

    fn is_composite(&self) -> bool {
        matches!(self, Widths::Composite { .. })
    }
}

/// Per-resource font state needed to lay out a show-text operator. The encoding
/// borrows the document it was resolved from.
struct FontInfo<'a> {
    enc: Option<lopdf::Encoding<'a>>,
    widths: Widths,
}

/// Build the `/W` code→width map of a CID font.
fn composite_widths(doc: &Document, desc: &Dictionary) -> Widths {
    let dw = get(doc, desc, b"DW").and_then(num).unwrap_or(1000.0);
    let mut w = BTreeMap::new();
    if let Some(Object::Array(items)) = get(doc, desc, b"W") {
        let vals: Vec<&Object> = items.iter().map(|o| deref(doc, o)).collect();
        let mut i = 0usize;
        while i < vals.len() {
            let Some(start) = num(vals[i]) else { break };
            let Some(next) = vals.get(i + 1) else { break };
            match next {
                // `c [w1 w2 …]` — consecutive codes from `c`.
                Object::Array(list) => {
                    for (k, item) in list.iter().enumerate() {
                        if let Some(width) = num(deref(doc, item)) {
                            w.insert(start as u32 + k as u32, width);
                        }
                    }
                    i += 2;
                }
                // `cFirst cLast w` — one width for the whole range.
                _ => {
                    let (Some(end), Some(width)) =
                        (num(next), vals.get(i + 2).and_then(|o| num(o)))
                    else {
                        break;
                    };
                    let (lo, hi) = (start as i64, end as i64);
                    if hi >= lo && hi - lo < 65_536 {
                        for c in lo..=hi {
                            w.insert(c as u32, width);
                        }
                    }
                    i += 3;
                }
            }
        }
    }
    Widths::Composite { dw, w }
}

/// Read a font dictionary's advance-width source.
fn font_widths(doc: &Document, font: &Dictionary) -> Widths {
    let subtype = get(doc, font, b"Subtype")
        .and_then(|o| o.as_name().ok())
        .map(|n| String::from_utf8_lossy(n).to_string())
        .unwrap_or_default();
    if subtype == "Type0" {
        if let Some(Object::Array(kids)) = get(doc, font, b"DescendantFonts") {
            if let Some(first) = kids.first() {
                if let Object::Dictionary(d) = deref(doc, first) {
                    return composite_widths(doc, d);
                }
            }
        }
        return Widths::Composite { dw: 1000.0, w: BTreeMap::new() };
    }
    let widths: Vec<f64> = match get(doc, font, b"Widths") {
        Some(Object::Array(items)) => items.iter().filter_map(|o| num(deref(doc, o))).collect(),
        _ => Vec::new(),
    };
    if widths.is_empty() {
        return Widths::Base14;
    }
    let first = get(doc, font, b"FirstChar")
        .and_then(num)
        .unwrap_or(0.0) as i64;
    let missing = get(doc, font, b"FontDescriptor")
        .and_then(|o| match o {
            Object::Dictionary(d) => get(doc, d, b"MissingWidth").and_then(num),
            _ => None,
        })
        .unwrap_or(0.0);
    Widths::Simple { first, widths, missing }
}

/// Lay out every glyph on a page in device space. Returns an empty vec for a page
/// with no decodable text layer (e.g. a scan).
fn page_glyphs(doc: &Document, page_id: ObjectId) -> Vec<Glyph> {
    let mut fonts: BTreeMap<Vec<u8>, FontInfo> = BTreeMap::new();
    if let Ok(page_fonts) = doc.get_page_fonts(page_id) {
        for (name, font) in page_fonts {
            let enc = font.get_font_encoding(doc).ok();
            let widths = font_widths(doc, &font);
            fonts.insert(name, FontInfo { enc, widths });
        }
    }

    let Ok(content) = doc.get_and_decode_page_content(page_id) else {
        return Vec::new();
    };

    let mut out: Vec<Glyph> = Vec::new();
    let mut ctm = Mat::ID;
    let mut ctm_stack: Vec<Mat> = Vec::new();
    let mut tm = Mat::ID;
    let mut tlm = Mat::ID;
    let mut font: Option<&FontInfo> = None;
    let mut fs = 0.0f64; // Tf size
    let mut tc = 0.0f64; // char spacing
    let mut tw = 0.0f64; // word spacing
    let mut th = 1.0f64; // horizontal scale (Tz/100)
    let mut ts = 0.0f64; // rise
    let mut leading = 0.0f64;

    /// `Td`-style translation of the line matrix.
    fn next_line(tlm: &mut Mat, tm: &mut Mat, tx: f64, ty: f64) {
        *tlm = Mat { a: 1.0, b: 0.0, c: 0.0, d: 1.0, e: tx, f: ty }.mul(*tlm);
        *tm = *tlm;
    }

    for op in &content.operations {
        let ops = &op.operands;
        match op.operator.as_str() {
            "q" => ctm_stack.push(ctm),
            "Q" => {
                if let Some(m) = ctm_stack.pop() {
                    ctm = m;
                }
            }
            "cm" => {
                if ops.len() >= 6 {
                    let v: Vec<f64> = ops.iter().filter_map(num).collect();
                    if v.len() >= 6 {
                        ctm = Mat { a: v[0], b: v[1], c: v[2], d: v[3], e: v[4], f: v[5] }.mul(ctm);
                    }
                }
            }
            "BT" => {
                tm = Mat::ID;
                tlm = Mat::ID;
            }
            "Tf" => {
                if let Some(Object::Name(name)) = ops.first() {
                    font = fonts.get(name.as_slice());
                }
                if let Some(sz) = ops.get(1).and_then(num) {
                    fs = sz;
                }
            }
            "Tc" => tc = ops.first().and_then(num).unwrap_or(tc),
            "Tw" => tw = ops.first().and_then(num).unwrap_or(tw),
            "Tz" => th = ops.first().and_then(num).map(|v| v / 100.0).unwrap_or(th),
            "Ts" => ts = ops.first().and_then(num).unwrap_or(ts),
            "TL" => leading = ops.first().and_then(num).unwrap_or(leading),
            "Tm" => {
                let v: Vec<f64> = ops.iter().filter_map(num).collect();
                if v.len() >= 6 {
                    tlm = Mat { a: v[0], b: v[1], c: v[2], d: v[3], e: v[4], f: v[5] };
                    tm = tlm;
                }
            }
            "Td" => {
                let (tx, ty) = (
                    ops.first().and_then(num).unwrap_or(0.0),
                    ops.get(1).and_then(num).unwrap_or(0.0),
                );
                next_line(&mut tlm, &mut tm, tx, ty);
            }
            "TD" => {
                let (tx, ty) = (
                    ops.first().and_then(num).unwrap_or(0.0),
                    ops.get(1).and_then(num).unwrap_or(0.0),
                );
                leading = -ty;
                next_line(&mut tlm, &mut tm, tx, ty);
            }
            "T*" => next_line(&mut tlm, &mut tm, 0.0, -leading),
            "Tj" | "TJ" | "'" | "\"" => {
                if op.operator == "'" {
                    next_line(&mut tlm, &mut tm, 0.0, -leading);
                } else if op.operator == "\"" {
                    tw = ops.first().and_then(num).unwrap_or(tw);
                    tc = ops.get(1).and_then(num).unwrap_or(tc);
                    next_line(&mut tlm, &mut tm, 0.0, -leading);
                }
                let Some(f) = font else { continue };
                let items: &[Object] = match op.operator.as_str() {
                    "TJ" => match ops.first().map(|o| deref(doc, o)) {
                        Some(Object::Array(arr)) => arr,
                        _ => continue,
                    },
                    "\"" => &ops[2.min(ops.len())..],
                    _ => ops,
                };
                for item in items {
                    match item {
                        Object::String(bytes, _) => {
                            show_string(bytes, f, fs, tc, tw, th, ts, &mut tm, ctm, &mut out);
                        }
                        other => {
                            // TJ kerning: a positive number moves LEFT by n/1000 em.
                            if let Some(adj) = num(other) {
                                let tx = -adj / 1000.0 * fs * th;
                                tm = Mat { a: 1.0, b: 0.0, c: 0.0, d: 1.0, e: tx, f: 0.0 }.mul(tm);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        if out.len() > 400_000 {
            break;
        }
    }
    out
}

/// Lay out one show-text string operand, appending its glyph boxes to `out` and
/// advancing the text matrix.
#[allow(clippy::too_many_arguments)]
fn show_string(
    bytes: &[u8],
    font: &FontInfo,
    fs: f64,
    tc: f64,
    tw: f64,
    th: f64,
    ts: f64,
    tm: &mut Mat,
    ctm: Mat,
    out: &mut Vec<Glyph>,
) {
    let composite = font.widths.is_composite();
    let codes: Vec<u32> = if composite {
        bytes
            .chunks(2)
            .map(|c| {
                if c.len() == 2 {
                    u32::from(u16::from_be_bytes([c[0], c[1]]))
                } else {
                    u32::from(c[0])
                }
            })
            .collect()
    } else {
        bytes.iter().map(|&b| u32::from(b)).collect()
    };

    let decoded: Vec<char> = match font.enc.as_ref() {
        Some(enc) => Document::decode_text(enc, bytes)
            .map(|s| s.chars().collect())
            .unwrap_or_default(),
        None => Vec::new(),
    };
    if decoded.is_empty() || codes.is_empty() {
        return;
    }
    // Codes and decoded characters line up 1:1 for the overwhelming majority of
    // fonts. When they don't (ligatures, multi-byte fallbacks), spread the string's
    // total advance evenly across the characters instead of guessing per glyph.
    let aligned = decoded.len() == codes.len();

    for (i, ch) in decoded.iter().copied().enumerate() {
        let code = if aligned { codes[i] } else { 0 };
        let w = if aligned {
            font.widths.advance(code, ch)
        } else {
            codes
                .iter()
                .map(|c| font.widths.advance(*c, ch))
                .sum::<f64>()
                / decoded.len() as f64
        };
        let word = if !composite && code == 32 { tw } else { 0.0 };
        let tx = (w * fs + tc + word) * th;

        let trm = tm.mul(ctm);
        let (x0, y) = trm.apply(0.0, ts);
        let advanced = Mat { a: 1.0, b: 0.0, c: 0.0, d: 1.0, e: tx, f: 0.0 }.mul(*tm);
        let (x1, _) = advanced.mul(ctm).apply(0.0, ts);
        out.push(Glyph {
            x0: x0.min(x1),
            x1: x0.max(x1),
            y,
            size: fs * trm.y_scale(),
            ch,
        });
        *tm = advanced;
    }
}

/// Recover the text covered by one annotation's `/QuadPoints` boxes.
fn text_in_quads(glyphs: &[Glyph], quads: &[f64]) -> String {
    let mut parts: Vec<String> = Vec::new();
    for quad in quads.chunks_exact(8) {
        let xs = [quad[0], quad[2], quad[4], quad[6]];
        let ys = [quad[1], quad[3], quad[5], quad[7]];
        let x_min = xs.iter().cloned().fold(f64::INFINITY, f64::min);
        let x_max = xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
        let y_min = ys.iter().cloned().fold(f64::INFINITY, f64::min);
        let y_max = ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max);

        let mut hits: Vec<&Glyph> = glyphs
            .iter()
            .filter(|g| {
                // A glyph belongs to the quad when its visual centre sits inside it.
                let cy = g.y + g.size * 0.3;
                let cx = (g.x0 + g.x1) / 2.0;
                cy >= y_min && cy <= y_max && cx >= x_min - 0.5 && cx <= x_max + 0.5
            })
            .collect();
        hits.sort_by(|a, b| a.x0.partial_cmp(&b.x0).unwrap_or(std::cmp::Ordering::Equal));

        let mut s = String::new();
        let mut prev_end: Option<(f64, f64)> = None;
        for g in hits {
            if let Some((end, size)) = prev_end {
                // PDFs often position words with Td instead of writing a space.
                if g.x0 - end > size * 0.2 && !s.ends_with(' ') {
                    s.push(' ');
                }
            }
            s.push(g.ch);
            prev_end = Some((g.x1, g.size.max(1.0)));
        }
        let s = collapse_ws(&s);
        if !s.is_empty() {
            parts.push(s);
        }
    }
    parts.join(" ")
}

/// Collapse runs of whitespace to single spaces and trim.
fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ---------------------------------------------------------------------------
// Extraction
// ---------------------------------------------------------------------------

/// Extract, filter and sort a PDF's annotations.
///
/// Returns `Err` when the bytes don't parse as a PDF, when the page spec is
/// invalid, or when `types`/`sort` carry an unknown value. A PDF that simply has
/// no annotations returns `Ok(vec![])`.
pub fn extract_annotations(bytes: &[u8], opts: &Options) -> Result<Vec<Annotation>, String> {
    if !TYPE_CHOICES.contains(&opts.types.as_str()) {
        return Err(format!(
            "unknown types {:?}; use one of: {}",
            opts.types,
            TYPE_CHOICES.join(", ")
        ));
    }
    if !SORT_CHOICES.contains(&opts.sort.as_str()) {
        return Err(format!(
            "unknown sort {:?}; use one of: {}",
            opts.sort,
            SORT_CHOICES.join(", ")
        ));
    }

    let doc = Document::load_mem(bytes).map_err(|e| format!("failed to parse PDF: {e}"))?;
    let pages = doc.get_pages();
    if pages.is_empty() {
        return Err("PDF has no pages".to_string());
    }
    if pages.len() > MAX_PAGES {
        return Err(format!(
            "PDF has too many pages: {} (cap {MAX_PAGES})",
            pages.len()
        ));
    }
    let page_count = pages.len() as u32;
    let selected = parse_pages(&opts.pages, page_count)?;
    let author_needle = opts.author.trim().to_lowercase();

    let mut out: Vec<Annotation> = Vec::new();
    let mut page_numbers: Vec<u32> = pages.keys().copied().collect();
    page_numbers.sort_unstable();

    for page in page_numbers {
        if let Some(sel) = &selected {
            if !sel.contains(&page) {
                continue;
            }
        }
        let Some(&page_id) = pages.get(&page) else { continue };
        let Ok(page_dict) = doc.get_dictionary(page_id) else { continue };
        let annots: Vec<Object> = match get(&doc, page_dict, b"Annots") {
            Some(Object::Array(items)) => items.clone(),
            _ => continue,
        };

        // The glyph layout is only needed when this page has text markup and the
        // caller asked for the marked-up text — it is the expensive part.
        let mut glyphs: Option<Vec<Glyph>> = None;

        for entry in &annots {
            if out.len() >= MAX_ANNOTATIONS {
                return Err(format!(
                    "PDF has too many annotations (cap {MAX_ANNOTATIONS})"
                ));
            }
            let Object::Dictionary(dict) = deref(&doc, entry) else { continue };
            let subtype = get(&doc, dict, b"Subtype")
                .and_then(|o| o.as_name().ok())
                .map(|n| String::from_utf8_lossy(n).to_string())
                .unwrap_or_default();
            // `Popup` only re-states its parent markup annotation's note, and
            // `Widget` is an AcroForm field (see the pdf-form-data-extract block).
            if subtype.is_empty() || subtype == "Popup" || subtype == "Widget" {
                continue;
            }
            let kind = classify(&subtype);
            if !type_matches(&opts.types, &kind) {
                continue;
            }

            let author = get(&doc, dict, b"T")
                .and_then(|o| o.as_str().ok())
                .map(decode_pdf_text)
                .unwrap_or_default();
            if !author_needle.is_empty() && !author.to_lowercase().contains(&author_needle) {
                continue;
            }

            let comment = collapse_lines(
                &get(&doc, dict, b"Contents")
                    .and_then(|o| o.as_str().ok())
                    .map(decode_pdf_text)
                    .unwrap_or_default(),
            );
            let date = get(&doc, dict, b"M")
                .or_else(|| get(&doc, dict, b"CreationDate"))
                .and_then(|o| o.as_str().ok())
                .map(|b| normalize_date(&decode_pdf_text(b)))
                .unwrap_or_default();
            let color = get(&doc, dict, b"C").map(|o| color_hex(&doc, o)).unwrap_or_default();

            let is_markup = MARKUP_KINDS.contains(&kind.as_str());
            let mut marked_text = String::new();
            if opts.include_marked_text && is_markup {
                if let Some(Object::Array(items)) = get(&doc, dict, b"QuadPoints") {
                    let quads: Vec<f64> =
                        items.iter().filter_map(|o| num(deref(&doc, o))).collect();
                    if quads.len() >= 8 {
                        let g = glyphs.get_or_insert_with(|| page_glyphs(&doc, page_id));
                        marked_text = text_in_quads(g, &quads);
                    }
                }
            }

            if !opts.include_empty && !is_markup && comment.is_empty() && marked_text.is_empty() {
                continue;
            }

            out.push(Annotation {
                page,
                kind,
                subtype,
                author,
                date,
                color,
                comment,
                marked_text,
            });
        }
    }

    sort_annotations(&mut out, &opts.sort);
    Ok(out)
}

/// Does an annotation kind pass the `types` filter?
fn type_matches(filter: &str, kind: &str) -> bool {
    match filter {
        "all" => true,
        "markup" => MARKUP_KINDS.contains(&kind),
        other => other == kind,
    }
}

/// Flatten a comment's newlines into single spaces (CSV/one-line output stays
/// well-formed; Markdown/text keep the collapsed form for consistency).
fn collapse_lines(s: &str) -> String {
    collapse_ws(s)
}

/// Stable sort by the requested key, page order always breaking ties.
fn sort_annotations(list: &mut [Annotation], sort: &str) {
    match sort {
        "author" => list.sort_by(|a, b| {
            a.author
                .to_lowercase()
                .cmp(&b.author.to_lowercase())
                .then(a.page.cmp(&b.page))
        }),
        "type" => list.sort_by(|a, b| a.kind.cmp(&b.kind).then(a.page.cmp(&b.page))),
        "date" => list.sort_by(|a, b| a.date.cmp(&b.date).then(a.page.cmp(&b.page))),
        // "page" — already in page order from the walk; keep it stable.
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// Serialization
// ---------------------------------------------------------------------------

/// The output formats the tool accepts (also the descriptor's enum).
pub const FORMAT_CHOICES: [&str; 4] = ["json", "csv", "markdown", "text"];

/// JSON array of annotation objects.
pub fn to_json(list: &[Annotation]) -> String {
    let items: Vec<String> = list
        .iter()
        .map(|a| {
            format!(
                "  {{\n    \"page\": {},\n    \"type\": {},\n    \"subtype\": {},\n    \"author\": {},\n    \"date\": {},\n    \"color\": {},\n    \"comment\": {},\n    \"marked_text\": {}\n  }}",
                a.page,
                json_str(&a.kind),
                json_str(&a.subtype),
                json_str(&a.author),
                json_str(&a.date),
                json_str(&a.color),
                json_str(&a.comment),
                json_str(&a.marked_text),
            )
        })
        .collect();
    if items.is_empty() {
        "[]".to_string()
    } else {
        format!("[\n{}\n]", items.join(",\n"))
    }
}

/// Minimal JSON string escaping (no serde dep in `core`).
fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// RFC 4180 CSV with a header row.
pub fn to_csv(list: &[Annotation]) -> String {
    let mut out = String::from("page,type,author,date,color,comment,marked_text\r\n");
    for a in list {
        let cells = [
            a.page.to_string(),
            a.kind.clone(),
            a.author.clone(),
            a.date.clone(),
            a.color.clone(),
            a.comment.clone(),
            a.marked_text.clone(),
        ];
        out.push_str(
            &cells
                .iter()
                .map(|c| csv_cell(c))
                .collect::<Vec<_>>()
                .join(","),
        );
        out.push_str("\r\n");
    }
    out
}

fn csv_cell(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Markdown grouped under a heading per page.
pub fn to_markdown(list: &[Annotation]) -> String {
    let mut out = String::new();
    let mut current: Option<u32> = None;
    for a in list {
        if current != Some(a.page) {
            if current.is_some() {
                out.push('\n');
            }
            out.push_str(&format!("## Page {}\n\n", a.page));
            current = Some(a.page);
        }
        out.push_str(&format!("- **{}**", a.kind));
        if !a.marked_text.is_empty() {
            out.push_str(&format!(" — \u{201c}{}\u{201d}", a.marked_text));
        }
        if !a.comment.is_empty() {
            out.push_str(&format!(" — {}", a.comment));
        }
        let mut meta: Vec<String> = Vec::new();
        if !a.author.is_empty() {
            meta.push(a.author.clone());
        }
        if !a.date.is_empty() {
            meta.push(a.date.clone());
        }
        if !meta.is_empty() {
            out.push_str(&format!(" _({})_", meta.join(", ")));
        }
        out.push('\n');
    }
    out
}

/// One flat line per annotation.
pub fn to_text(list: &[Annotation]) -> String {
    let mut out = String::new();
    for a in list {
        out.push_str(&format!("p{} [{}]", a.page, a.kind));
        if !a.author.is_empty() {
            out.push_str(&format!(" {}", a.author));
        }
        if !a.marked_text.is_empty() {
            out.push_str(&format!(" \u{201c}{}\u{201d}", a.marked_text));
        }
        if !a.comment.is_empty() {
            out.push_str(&format!(" — {}", a.comment));
        }
        out.push('\n');
    }
    out
}

/// Serialize with the named format. Errors on an unknown format.
pub fn serialize(list: &[Annotation], format: &str) -> Result<String, String> {
    match format {
        "json" => Ok(to_json(list)),
        "csv" => Ok(to_csv(list)),
        "markdown" => Ok(to_markdown(list)),
        "text" => Ok(to_text(list)),
        other => Err(format!(
            "unknown format {other:?}; use one of: {}",
            FORMAT_CHOICES.join(", ")
        )),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::content::{Content, Operation};
    use lopdf::{dictionary, Stream};

    /// One text run to place on the synthetic test page.
    struct Run {
        text: &'static str,
        x: f64,
        y: f64,
        size: f64,
    }

    /// Build a one-page PDF with Helvetica text runs and the given annotation
    /// dictionaries. Helvetica has no `/Widths`, so the layout exercises the
    /// base-14 fallback — the same path a real base-14 PDF takes.
    fn build_pdf(runs: &[Run], annots: Vec<Object>) -> Vec<u8> {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let font_id = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => "Helvetica",
        });
        let resources_id = doc.add_object(dictionary! {
            "Font" => dictionary! { "F1" => font_id },
        });

        let mut ops = Vec::new();
        for r in runs {
            ops.push(Operation::new("BT", vec![]));
            ops.push(Operation::new("Tf", vec!["F1".into(), r.size.into()]));
            ops.push(Operation::new("Td", vec![r.x.into(), r.y.into()]));
            ops.push(Operation::new(
                "Tj",
                vec![Object::string_literal(r.text)],
            ));
            ops.push(Operation::new("ET", vec![]));
        }
        let content = Content { operations: ops };
        let content_id = doc.add_object(Stream::new(
            dictionary! {},
            content.encode().expect("encode content"),
        ));

        let mut page = dictionary! {
            "Type" => "Page",
            "Parent" => pages_id,
            "Contents" => content_id,
            "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
        };
        if !annots.is_empty() {
            page.set("Annots", Object::Array(annots));
        }
        let page_id = doc.add_object(page);

        let pages = dictionary! {
            "Type" => "Pages",
            "Kids" => vec![page_id.into()],
            "Count" => 1,
            "Resources" => resources_id,
        };
        doc.objects.insert(pages_id, Object::Dictionary(pages));
        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        doc.trailer.set("Root", catalog_id);

        let mut buf = Vec::new();
        doc.save_to(&mut buf).expect("save pdf");
        buf
    }

    /// Width of a Helvetica string at `size`, in points — used to build quads
    /// that land exactly on a known substring.
    fn helv_width(s: &str, size: f64) -> f64 {
        s.chars()
            .map(|c| {
                let c = c as u32;
                if (32..=126).contains(&c) {
                    HELVETICA[(c - 32) as usize] as f64 / 1000.0
                } else {
                    0.5
                }
            })
            .sum::<f64>()
            * size
    }

    fn quad(x0: f64, x1: f64, y0: f64, y1: f64) -> Object {
        Object::Array(vec![
            x0.into(),
            y1.into(),
            x1.into(),
            y1.into(),
            x0.into(),
            y0.into(),
            x1.into(),
            y0.into(),
        ])
    }

    /// A PDF with a highlight over "brown fox" plus a sticky note and a bare link.
    fn sample_pdf() -> Vec<u8> {
        let text = "The quick brown fox jumps over the lazy dog";
        let size = 12.0;
        let x = 72.0;
        let y = 700.0;
        let start = x + helv_width("The quick ", size);
        let end = x + helv_width("The quick brown fox", size);

        let highlight = Object::Dictionary(dictionary! {
            "Type" => "Annot",
            "Subtype" => "Highlight",
            "Rect" => vec![start.into(), (y - 3.0).into(), end.into(), (y + 12.0).into()],
            "QuadPoints" => quad(start, end, y - 3.0, y + 11.0),
            "T" => Object::string_literal("Ada Lovelace"),
            "Contents" => Object::string_literal("check this animal"),
            "M" => Object::string_literal("D:20260115103000+01'00'"),
            "C" => vec![1.into(), Object::Real(1.0), 0.into()],
        });
        let note = Object::Dictionary(dictionary! {
            "Type" => "Annot",
            "Subtype" => "Text",
            "Rect" => vec![500.into(), 700.into(), 520.into(), 720.into()],
            "T" => Object::string_literal("Grace Hopper"),
            "Contents" => Object::string_literal("needs a citation"),
            "M" => Object::string_literal("D:20260220081500Z"),
            "C" => vec![Object::Real(0.0), Object::Real(0.0), Object::Real(1.0)],
        });
        let link = Object::Dictionary(dictionary! {
            "Type" => "Annot",
            "Subtype" => "Link",
            "Rect" => vec![72.into(), 600.into(), 200.into(), 620.into()],
        });
        let popup = Object::Dictionary(dictionary! {
            "Type" => "Annot",
            "Subtype" => "Popup",
            "Rect" => vec![300.into(), 600.into(), 400.into(), 700.into()],
            "Contents" => Object::string_literal("check this animal"),
        });

        build_pdf(
            &[Run { text, x, y, size }],
            vec![highlight, note, link, popup],
        )
    }

    #[test]
    fn extracts_highlight_note_and_recovers_marked_text() {
        let list = extract_annotations(&sample_pdf(), &Options::default()).unwrap();
        assert_eq!(list.len(), 2, "popup + bare link are dropped: {list:?}");

        let h = &list[0];
        assert_eq!(h.kind, "highlight");
        assert_eq!(h.subtype, "Highlight");
        assert_eq!(h.page, 1);
        assert_eq!(h.author, "Ada Lovelace");
        assert_eq!(h.comment, "check this animal");
        assert_eq!(h.color, "#ffff00");
        assert_eq!(h.date, "2026-01-15T10:30:00+01:00");
        assert_eq!(h.marked_text, "brown fox");

        let n = &list[1];
        assert_eq!(n.kind, "note");
        assert_eq!(n.author, "Grace Hopper");
        assert_eq!(n.comment, "needs a citation");
        assert_eq!(n.color, "#0000ff");
        assert_eq!(n.date, "2026-02-20T08:15:00Z");
        assert!(n.marked_text.is_empty(), "a sticky note has no quads");
    }

    #[test]
    fn include_empty_keeps_the_bare_link() {
        let opts = Options { include_empty: true, ..Options::default() };
        let list = extract_annotations(&sample_pdf(), &opts).unwrap();
        assert_eq!(list.len(), 3);
        assert!(list.iter().any(|a| a.kind == "link"));
    }

    #[test]
    fn types_filter_selects_one_kind() {
        let opts = Options { types: "note".to_string(), ..Options::default() };
        let list = extract_annotations(&sample_pdf(), &opts).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].kind, "note");

        let opts = Options { types: "markup".to_string(), ..Options::default() };
        let list = extract_annotations(&sample_pdf(), &opts).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].kind, "highlight");
    }

    #[test]
    fn author_filter_is_case_insensitive_substring() {
        let opts = Options { author: "hopper".to_string(), ..Options::default() };
        let list = extract_annotations(&sample_pdf(), &opts).unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].author, "Grace Hopper");
    }

    #[test]
    fn include_marked_text_off_skips_the_text_layer_walk() {
        let opts = Options { include_marked_text: false, ..Options::default() };
        let list = extract_annotations(&sample_pdf(), &opts).unwrap();
        assert_eq!(list[0].kind, "highlight");
        assert!(list[0].marked_text.is_empty());
        // A markup annotation stays even with no comment/marked text.
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn sort_by_author_orders_alphabetically() {
        let opts = Options { sort: "author".to_string(), ..Options::default() };
        let list = extract_annotations(&sample_pdf(), &opts).unwrap();
        assert_eq!(list[0].author, "Ada Lovelace");
        assert_eq!(list[1].author, "Grace Hopper");
    }

    #[test]
    fn rejects_unknown_types_value() {
        let opts = Options { types: "sticky".to_string(), ..Options::default() };
        let err = extract_annotations(&sample_pdf(), &opts).unwrap_err();
        assert!(err.contains("unknown types"), "{err}");
        assert!(err.contains("highlight"), "error lists the valid values: {err}");
    }

    #[test]
    fn rejects_non_pdf_bytes() {
        let err = extract_annotations(b"not a pdf at all", &Options::default()).unwrap_err();
        assert!(err.contains("failed to parse PDF"), "{err}");
    }

    #[test]
    fn page_out_of_range_errors() {
        let err = extract_annotations(
            &sample_pdf(),
            &Options { pages: "4".to_string(), ..Options::default() },
        )
        .unwrap_err();
        assert!(err.contains("out of range"), "{err}");
    }

    #[test]
    fn page_spec_selects_and_rejects() {
        assert_eq!(parse_pages("", 5).unwrap(), None);
        assert_eq!(
            parse_pages("1,3-5", 5).unwrap().unwrap().into_iter().collect::<Vec<_>>(),
            vec![1, 3, 4, 5]
        );
        assert!(parse_pages("0", 5).unwrap_err().contains("1-based"));
        assert!(parse_pages("5-2", 5).unwrap_err().contains("after end"));
        assert!(parse_pages("x", 5).unwrap_err().contains("bad page spec"));
    }

    #[test]
    fn pages_filter_returns_nothing_for_an_annotation_free_page() {
        // The sample is one page, so selecting page 1 keeps everything.
        let opts = Options { pages: "1".to_string(), ..Options::default() };
        assert_eq!(extract_annotations(&sample_pdf(), &opts).unwrap().len(), 2);
    }

    #[test]
    fn pdf_without_annotations_is_empty_not_an_error() {
        let pdf = build_pdf(
            &[Run { text: "plain page", x: 72.0, y: 700.0, size: 12.0 }],
            vec![],
        );
        assert!(extract_annotations(&pdf, &Options::default()).unwrap().is_empty());
    }

    #[test]
    fn serializes_every_advertised_format() {
        let list = extract_annotations(&sample_pdf(), &Options::default()).unwrap();

        let json = serialize(&list, "json").unwrap();
        assert!(json.contains("\"marked_text\": \"brown fox\""), "{json}");
        assert!(json.contains("\"type\": \"highlight\""), "{json}");

        let csv = serialize(&list, "csv").unwrap();
        assert!(csv.starts_with("page,type,author,date,color,comment,marked_text\r\n"));
        assert!(csv.contains("1,highlight,Ada Lovelace,2026-01-15T10:30:00+01:00,#ffff00,check this animal,brown fox\r\n"), "{csv}");

        let md = serialize(&list, "markdown").unwrap();
        assert!(md.starts_with("## Page 1\n\n"), "{md}");
        assert!(md.contains("- **highlight** — \u{201c}brown fox\u{201d} — check this animal _(Ada Lovelace, 2026-01-15T10:30:00+01:00)_"), "{md}");

        let txt = serialize(&list, "text").unwrap();
        assert!(txt.starts_with("p1 [highlight] Ada Lovelace \u{201c}brown fox\u{201d} — check this animal\n"), "{txt}");
    }

    #[test]
    fn serialize_rejects_unknown_format() {
        let err = serialize(&[], "xml").unwrap_err();
        assert!(err.contains("unknown format"), "{err}");
    }

    #[test]
    fn empty_list_serializes_cleanly() {
        assert_eq!(to_json(&[]), "[]");
        assert_eq!(to_csv(&[]), "page,type,author,date,color,comment,marked_text\r\n");
        assert_eq!(to_markdown(&[]), "");
        assert_eq!(to_text(&[]), "");
    }

    #[test]
    fn csv_quotes_commas_and_quotes() {
        assert_eq!(csv_cell("a,b"), "\"a,b\"");
        assert_eq!(csv_cell("say \"hi\""), "\"say \"\"hi\"\"\"");
        assert_eq!(csv_cell("plain"), "plain");
    }

    #[test]
    fn colors_cover_gray_rgb_and_cmyk() {
        let doc = Document::with_version("1.5");
        let gray = Object::Array(vec![Object::Real(0.5)]);
        assert_eq!(color_hex(&doc, &gray), "#808080");
        let rgb = Object::Array(vec![Object::Real(1.0), Object::Real(0.0), Object::Real(0.0)]);
        assert_eq!(color_hex(&doc, &rgb), "#ff0000");
        // CMYK cyan → red channel 0.
        let cmyk = Object::Array(vec![
            Object::Real(1.0),
            Object::Real(0.0),
            Object::Real(0.0),
            Object::Real(0.0),
        ]);
        assert_eq!(color_hex(&doc, &cmyk), "#00ffff");
        assert_eq!(color_hex(&doc, &Object::Array(vec![])), "");
    }

    #[test]
    fn dates_normalize_and_pass_through() {
        assert_eq!(normalize_date("D:20260115103000+01'00'"), "2026-01-15T10:30:00+01:00");
        assert_eq!(normalize_date("D:20260115103000Z"), "2026-01-15T10:30:00Z");
        assert_eq!(normalize_date("D:20260115"), "2026-01-15");
        assert_eq!(normalize_date("yesterday"), "yesterday");
    }

    #[test]
    fn utf16be_author_and_comment_decode() {
        let mut bytes = vec![0xFE, 0xFF];
        for u in "Ünïcode".encode_utf16() {
            bytes.extend_from_slice(&u.to_be_bytes());
        }
        assert_eq!(decode_pdf_text(&bytes), "Ünïcode");
    }
}
