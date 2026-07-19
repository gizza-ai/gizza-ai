//! gizza-ai/pdf-diff core — pure PDF comparison shared by the chat skill block.
//! No wafer/wasm-bindgen deps so it compiles natively for unit tests and to
//! `wasm32-wasip1` for the block.
//!
//! Compares an ORIGINAL and a REVISED PDF page by page and reports:
//! - per-page word-level (or line-level) text differences as add/remove/replace
//!   hunks with surrounding context,
//! - object-level "visual" changes per page: page size (MediaBox), rotation,
//!   embedded image XObjects (added/removed/replaced by content hash) and the
//!   set of fonts used,
//! - document metadata (/Info) changes,
//! - a summary with changed/unchanged/added/removed page counts.
//!
//! Pages are aligned either sequentially (page N vs page N) or automatically
//! (`Align::Auto`, the default): a similarity-based monotone alignment over
//! per-page word sets detects inserted/removed pages so one added page does not
//! cascade into "every following page changed". Text comes from the embedded
//! selectable text layer only (no OCR — scanned pages legitimately compare as
//! empty); rasterized pixel comparison is out of scope, the object-level visual
//! diff above is what this tool reports.

use std::collections::BTreeSet;

use lopdf::{Document, Object, ObjectId};
use serde::Serialize;

/// Hard cap on pages per document.
pub const MAX_PAGES: usize = 2000;
/// Auto page alignment is quadratic in page count; beyond this many pages in
/// either document it falls back to sequential alignment (with a warning).
pub const AUTO_ALIGN_MAX_PAGES: usize = 200;
/// Minimum word-set similarity for the auto-aligner to match two pages.
const ALIGN_MIN_SIMILARITY: f64 = 0.3;
/// Cap on the (trimmed) LCS DP table size for one page pair. Beyond it the page
/// is summarized coarsely instead of hunk-by-hunk (keeps the 64 MiB sandbox
/// safe: 1.5M u32 cells = 6 MiB transient).
const MAX_DP_CELLS: usize = 1_500_000;
/// Caps on reported hunks (counts are still exact when truncated).
const MAX_HUNKS_PER_PAGE: usize = 40;
const MAX_HUNKS_TOTAL: usize = 250;
/// Unchanged tokens of context kept on each side of a hunk.
const CONTEXT_TOKENS: usize = 6;
/// Snippet cap per hunk side, in tokens.
const SNIPPET_MAX_TOKENS: usize = 40;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Words,
    Lines,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Auto,
    Sequential,
}

#[derive(Debug, Clone)]
pub struct Options {
    pub mode: Mode,
    pub align: Align,
    pub ignore_case: bool,
    /// 1-based page spec applied to BOTH documents: `"all"`, `"odd"`, `"even"`,
    /// or lists/ranges like `"1,3-5"`. Out-of-range pages are ignored per
    /// document.
    pub pages: String,
    pub include_unchanged: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            mode: Mode::Words,
            align: Align::Auto,
            ignore_case: false,
            pages: "all".to_string(),
            include_unchanged: false,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Report {
    /// True when the compared pages have no text, visual or metadata
    /// differences and no pages were added or removed.
    pub identical: bool,
    pub summary: String,
    pub pages: PagesSummary,
    /// The alignment actually used ("auto" or "sequential").
    pub page_alignment: &'static str,
    pub text_changes: Vec<PageTextChanges>,
    pub visual_changes: Vec<VisualChange>,
    pub metadata_changes: Vec<MetadataChange>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unchanged_pages: Option<Vec<PagePair>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
pub struct PagesSummary {
    /// Total pages in each document (before any `pages` filter).
    pub original: usize,
    pub revised: usize,
    /// Page pairs actually compared after filtering + alignment.
    pub compared_pairs: usize,
    pub changed: usize,
    pub unchanged: usize,
    /// Revised-document page numbers with no counterpart in the original.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub added_in_revised: Vec<u32>,
    /// Original-document page numbers with no counterpart in the revised PDF.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub removed_from_original: Vec<u32>,
}

#[derive(Debug, Serialize)]
pub struct PagePair {
    pub original_page: u32,
    pub revised_page: u32,
}

#[derive(Debug, Serialize)]
pub struct PageTextChanges {
    pub original_page: u32,
    pub revised_page: u32,
    /// Word-set similarity of the pair, 0.0–1.0 (1.0 = same words).
    pub similarity: f64,
    pub words_added: usize,
    pub words_removed: usize,
    /// True when the hunk list below is incomplete (page rewritten beyond the
    /// diff budget, or the report-wide hunk cap was reached). The word counts
    /// above are always exact.
    pub truncated: bool,
    pub changes: Vec<Hunk>,
}

#[derive(Debug, Serialize)]
pub struct Hunk {
    /// "insert" | "delete" | "replace".
    pub op: &'static str,
    /// Text removed from the original ("" for insert).
    pub removed: String,
    /// Text added in the revised PDF ("" for delete).
    pub added: String,
    pub context_before: String,
    pub context_after: String,
}

#[derive(Debug, Serialize)]
pub struct VisualChange {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_page: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revised_page: Option<u32>,
    /// "page_size" | "rotation" | "images" | "fonts".
    pub kind: &'static str,
    pub detail: String,
}

#[derive(Debug, Serialize)]
pub struct MetadataChange {
    pub field: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub revised: Option<String>,
}

// ---------------------------------------------------------------------------
// Page spec parsing (same grammar as pdf-split / pdf-delete-pages, but lenient
// about out-of-range pages: they are ignored per document).
// ---------------------------------------------------------------------------

/// Parse a 1-based page spec (`"all"`, `"odd"`, `"even"`, `"1,3-5"`) into the
/// set of selected pages clamped to `total`. Errors on syntax errors; the
/// caller errors when nothing is selected for a document.
pub fn parse_page_spec(spec: &str, total: u32) -> Result<BTreeSet<u32>, String> {
    let s = spec.trim();
    if s.is_empty() || s.eq_ignore_ascii_case("all") {
        return Ok((1..=total).collect());
    }
    if s.eq_ignore_ascii_case("odd") {
        return Ok((1..=total).filter(|p| p % 2 == 1).collect());
    }
    if s.eq_ignore_ascii_case("even") {
        return Ok((1..=total).filter(|p| p % 2 == 0).collect());
    }
    let mut keep = BTreeSet::new();
    for part in s.split(',') {
        let part = part.trim();
        if part.is_empty() {
            continue;
        }
        if let Some((a, b)) = part.split_once('-') {
            let start: u32 = a
                .trim()
                .parse()
                .map_err(|_| format!("invalid page '{a}' in pages spec '{spec}'"))?;
            let end: u32 = b
                .trim()
                .parse()
                .map_err(|_| format!("invalid page '{b}' in pages spec '{spec}'"))?;
            if start == 0 || end == 0 {
                return Err("page numbers are 1-based (must be >= 1)".into());
            }
            let (lo, hi) = if start <= end { (start, end) } else { (end, start) };
            for p in lo..=hi.min(total) {
                keep.insert(p);
            }
        } else {
            let p: u32 = part
                .parse()
                .map_err(|_| format!("invalid page '{part}' in pages spec '{spec}'"))?;
            if p == 0 {
                return Err("page numbers are 1-based (must be >= 1)".into());
            }
            if p <= total {
                keep.insert(p);
            }
        }
    }
    Ok(keep)
}

// ---------------------------------------------------------------------------
// Per-page extraction
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct PageData {
    number: u32,
    dropped_chunks: usize,
    width: f64,
    height: f64,
    rotation: i64,
    /// Sorted content hashes of image XObjects reachable from this page.
    images: Vec<u64>,
    /// Base font names (subset prefixes like `ABCDEF+` stripped).
    fonts: BTreeSet<String>,
    /// Display tokens (words or normalized lines, per `Mode`).
    tokens: Vec<String>,
    /// Comparison key hash per token (case folding applied per options).
    keys: Vec<u64>,
    /// Sorted, deduplicated word-key hashes (always word-based) for page
    /// similarity, independent of `Mode`.
    word_set: Vec<u64>,
}

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

/// Follow references to the concrete object.
fn deref<'a>(doc: &'a Document, obj: &'a Object) -> Option<&'a Object> {
    match obj {
        Object::Reference(id) => doc.get_object(*id).ok(),
        other => Some(other),
    }
}

fn number(obj: &Object) -> Option<f64> {
    match obj {
        Object::Integer(i) => Some(*i as f64),
        Object::Real(r) => Some(*r as f64),
        _ => None,
    }
}

/// Read an inheritable page attribute, walking `/Parent`.
fn inherited<'a>(doc: &'a Document, page_id: ObjectId, key: &[u8]) -> Option<&'a Object> {
    let mut current = page_id;
    for _ in 0..64 {
        let dict = doc.get_dictionary(current).ok()?;
        if let Ok(raw) = dict.get(key) {
            return Some(raw);
        }
        match dict.get(b"Parent").and_then(|o| o.as_reference()) {
            Ok(parent) => current = parent,
            Err(_) => return None,
        }
    }
    None
}

/// MediaBox as (width, height), normalized. Defaults to US Letter when absent
/// (a malformed but tolerable document).
fn page_size(doc: &Document, page_id: ObjectId) -> (f64, f64) {
    let rect = inherited(doc, page_id, b"MediaBox")
        .and_then(|raw| deref(doc, raw))
        .and_then(|o| o.as_array().ok())
        .and_then(|arr| {
            if arr.len() != 4 {
                return None;
            }
            let mut r = [0.0f64; 4];
            for (i, e) in arr.iter().enumerate() {
                r[i] = deref(doc, e).and_then(number)?;
            }
            Some(r)
        });
    match rect {
        Some([x0, y0, x1, y1]) => ((x1 - x0).abs(), (y1 - y0).abs()),
        None => (612.0, 792.0),
    }
}

fn page_rotation(doc: &Document, page_id: ObjectId) -> i64 {
    let r = inherited(doc, page_id, b"Rotate")
        .and_then(|raw| deref(doc, raw))
        .and_then(|o| o.as_i64().ok())
        .unwrap_or(0);
    r.rem_euclid(360)
}

/// Strip a font subset prefix (`ABCDEF+Arial` -> `Arial`).
fn strip_subset_prefix(name: &str) -> &str {
    match name.split_once('+') {
        Some((prefix, rest))
            if prefix.len() == 6 && prefix.bytes().all(|b| b.is_ascii_uppercase()) =>
        {
            rest
        }
        _ => name,
    }
}

/// Collect image content hashes + font names reachable from a resources dict,
/// recursing into Form XObjects (depth-limited, cycle-safe).
fn collect_resources(
    doc: &Document,
    resources: &Object,
    images: &mut Vec<u64>,
    fonts: &mut BTreeSet<String>,
    visited: &mut BTreeSet<ObjectId>,
    depth: usize,
) {
    if depth > 3 {
        return;
    }
    let Some(res) = deref(doc, resources).and_then(|o| o.as_dict().ok()) else {
        return;
    };
    if let Ok(fonts_raw) = res.get(b"Font") {
        if let Some(fdict) = deref(doc, fonts_raw).and_then(|o| o.as_dict().ok()) {
            for (_, v) in fdict.iter() {
                if let Some(font) = deref(doc, v).and_then(|o| o.as_dict().ok()) {
                    if let Ok(base) = font.get(b"BaseFont").and_then(|o| o.as_name()) {
                        let name = String::from_utf8_lossy(base).to_string();
                        fonts.insert(strip_subset_prefix(&name).to_string());
                    }
                }
            }
        }
    }
    if let Ok(xobj_raw) = res.get(b"XObject") {
        if let Some(xdict) = deref(doc, xobj_raw).and_then(|o| o.as_dict().ok()) {
            for (_, v) in xdict.iter() {
                // Cycle guard on referenced XObjects.
                if let Object::Reference(id) = v {
                    if !visited.insert(*id) {
                        continue;
                    }
                }
                let Some(Object::Stream(stream)) = deref(doc, v) else {
                    continue;
                };
                let subtype = stream
                    .dict
                    .get(b"Subtype")
                    .ok()
                    .and_then(|o| o.as_name().ok())
                    .unwrap_or(b"");
                if subtype == b"Image" {
                    images.push(fnv1a(&stream.content));
                } else if subtype == b"Form" {
                    if let Ok(inner) = stream.dict.get(b"Resources") {
                        collect_resources(doc, inner, images, fonts, visited, depth + 1);
                    }
                }
            }
        }
    }
}

fn token_key(token: &str, ignore_case: bool) -> u64 {
    if ignore_case {
        fnv1a(token.to_lowercase().as_bytes())
    } else {
        fnv1a(token.as_bytes())
    }
}

fn tokenize(text: &str, mode: Mode, ignore_case: bool) -> (Vec<String>, Vec<u64>) {
    let tokens: Vec<String> = match mode {
        Mode::Words => text.split_whitespace().map(|w| w.to_string()).collect(),
        Mode::Lines => text
            .lines()
            .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
            .filter(|l| !l.is_empty())
            .collect(),
    };
    let keys: Vec<u64> = tokens.iter().map(|t| token_key(t, ignore_case)).collect();
    (tokens, keys)
}

/// Parse one document and extract everything the diff needs, labeling errors
/// with `label` ("original" / "revised").
fn load_pages(
    bytes: &[u8],
    label: &str,
    opt: &Options,
) -> Result<(Vec<PageData>, usize, Document), String> {
    if bytes.is_empty() {
        return Err(format!("{label} PDF is empty (0 bytes)"));
    }
    let doc = Document::load_mem(bytes).map_err(|e| {
        if find_bytes(bytes, b"/Encrypt") {
            format!(
                "{label} PDF appears to be password-protected — remove the password first \
                 (this tool cannot open encrypted PDFs)"
            )
        } else {
            format!("{label}: failed to parse PDF: {e}")
        }
    })?;
    if doc.trailer.get(b"Encrypt").is_ok() {
        return Err(format!(
            "{label} PDF is password-protected — remove the password first \
             (this tool cannot open encrypted PDFs)"
        ));
    }

    let mut page_map: Vec<(u32, ObjectId)> = doc.get_pages().into_iter().collect();
    page_map.sort_unstable_by_key(|(n, _)| *n);
    let total = page_map.len();
    if total == 0 {
        return Err(format!("{label} PDF has no pages"));
    }
    if total > MAX_PAGES {
        return Err(format!(
            "{label} PDF has too many pages: {total} (cap {MAX_PAGES})"
        ));
    }

    let selected = parse_page_spec(&opt.pages, total as u32)?;
    if selected.is_empty() {
        return Err(format!(
            "pages spec '{}' selects no pages in the {label} PDF ({total} page{})",
            opt.pages,
            if total == 1 { "" } else { "s" }
        ));
    }

    let mut pages = Vec::with_capacity(selected.len());
    for (num, id) in page_map {
        if !selected.contains(&num) {
            continue;
        }
        let mut text = String::new();
        let mut dropped = 0usize;
        for chunk in doc.extract_text_chunks(&[num]) {
            match chunk {
                Ok(t) => text.push_str(&t),
                Err(_) => dropped += 1,
            }
        }
        let (tokens, keys) = tokenize(&text, opt.mode, opt.ignore_case);
        // Word set for similarity is always word-based, independent of Mode.
        let word_set: Vec<u64> = {
            let mut v: Vec<u64> = text
                .split_whitespace()
                .map(|w| token_key(w, opt.ignore_case))
                .collect();
            v.sort_unstable();
            v.dedup();
            v
        };
        let (width, height) = page_size(&doc, id);
        let rotation = page_rotation(&doc, id);
        let mut images = Vec::new();
        let mut fonts = BTreeSet::new();
        if let Some(res) = inherited(&doc, id, b"Resources") {
            let mut visited = BTreeSet::new();
            collect_resources(&doc, res, &mut images, &mut fonts, &mut visited, 0);
        }
        images.sort_unstable();
        pages.push(PageData {
            number: num,
            dropped_chunks: dropped,
            width,
            height,
            rotation,
            images,
            fonts,
            tokens,
            keys,
            word_set,
        });
    }
    Ok((pages, total, doc))
}

fn find_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

// ---------------------------------------------------------------------------
// Page alignment
// ---------------------------------------------------------------------------

fn jaccard(a: &[u64], b: &[u64]) -> f64 {
    if a.is_empty() && b.is_empty() {
        return 1.0;
    }
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let (mut i, mut j, mut inter) = (0usize, 0usize, 0usize);
    while i < a.len() && j < b.len() {
        match a[i].cmp(&b[j]) {
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
            std::cmp::Ordering::Equal => {
                inter += 1;
                i += 1;
                j += 1;
            }
        }
    }
    let union = a.len() + b.len() - inter;
    inter as f64 / union as f64
}

/// Monotone page alignment: maximize total pair similarity over pairs with
/// similarity >= `ALIGN_MIN_SIMILARITY`. Returns matched index pairs into the
/// filtered page vectors.
fn align_auto(a: &[PageData], b: &[PageData]) -> Vec<(usize, usize)> {
    let (n, m) = (a.len(), b.len());
    let mut sim = vec![0.0f64; n * m];
    for i in 0..n {
        for j in 0..m {
            sim[i * m + j] = jaccard(&a[i].word_set, &b[j].word_set);
        }
    }
    // dp[i][j] = best total similarity aligning a[..i] with b[..j].
    let mut dp = vec![0.0f64; (n + 1) * (m + 1)];
    let idx = |i: usize, j: usize| i * (m + 1) + j;
    for i in 1..=n {
        for j in 1..=m {
            let mut best = dp[idx(i - 1, j)].max(dp[idx(i, j - 1)]);
            let s = sim[(i - 1) * m + (j - 1)];
            if s >= ALIGN_MIN_SIMILARITY {
                best = best.max(dp[idx(i - 1, j - 1)] + s);
            }
            dp[idx(i, j)] = best;
        }
    }
    // Backtrack, preferring diagonal matches.
    let mut pairs = Vec::new();
    let (mut i, mut j) = (n, m);
    while i > 0 && j > 0 {
        let s = sim[(i - 1) * m + (j - 1)];
        if s >= ALIGN_MIN_SIMILARITY
            && (dp[idx(i, j)] - (dp[idx(i - 1, j - 1)] + s)).abs() < 1e-12
        {
            pairs.push((i - 1, j - 1));
            i -= 1;
            j -= 1;
        } else if dp[idx(i, j)] == dp[idx(i - 1, j)] {
            i -= 1;
        } else {
            j -= 1;
        }
    }
    pairs.reverse();
    pairs
}

/// Turn matched pairs into the full ordered pairing: leftover pages inside each
/// gap region are paired index-wise (a fully rewritten page reads as CHANGED,
/// not removed+added); the rest become added/removed.
fn full_pairing(
    matched: &[(usize, usize)],
    n: usize,
    m: usize,
) -> (Vec<(usize, usize)>, Vec<usize>, Vec<usize>) {
    let mut pairs = Vec::new();
    let mut removed = Vec::new();
    let mut added = Vec::new();
    let (mut pa, mut pb) = (0usize, 0usize);
    fn fill_gap(
        a_from: usize,
        a_to: usize,
        b_from: usize,
        b_to: usize,
        pairs: &mut Vec<(usize, usize)>,
        removed: &mut Vec<usize>,
        added: &mut Vec<usize>,
    ) {
        let ga: Vec<usize> = (a_from..a_to).collect();
        let gb: Vec<usize> = (b_from..b_to).collect();
        let k = ga.len().min(gb.len());
        for t in 0..k {
            pairs.push((ga[t], gb[t]));
        }
        removed.extend(ga[k..].iter().copied());
        added.extend(gb[k..].iter().copied());
    }
    for &(i, j) in matched {
        fill_gap(pa, i, pb, j, &mut pairs, &mut removed, &mut added);
        pairs.push((i, j));
        pa = i + 1;
        pb = j + 1;
    }
    fill_gap(pa, n, pb, m, &mut pairs, &mut removed, &mut added);
    pairs.sort_unstable();
    (pairs, removed, added)
}

// ---------------------------------------------------------------------------
// Token diff
// ---------------------------------------------------------------------------

struct PairDiff {
    hunks: Vec<Hunk>,
    added: usize,
    removed: usize,
    truncated: bool,
}

#[derive(Clone, Copy, PartialEq)]
enum Op {
    Equal,
    Delete,
    Insert,
}

fn snippet(tokens: &[String], joiner: &str) -> String {
    if tokens.len() <= SNIPPET_MAX_TOKENS {
        tokens.join(joiner)
    } else {
        let mut s = tokens[..SNIPPET_MAX_TOKENS].join(joiner);
        s.push_str(&format!(" … (+{} more)", tokens.len() - SNIPPET_MAX_TOKENS));
        s
    }
}

fn context_snippet(tokens: &[String], joiner: &str, tail: bool) -> String {
    let k = tokens.len().min(CONTEXT_TOKENS);
    if tail {
        tokens[tokens.len() - k..].join(joiner)
    } else {
        tokens[..k].join(joiner)
    }
}

/// Diff one aligned page pair at token level. `budget` caps hunks emitted
/// across the whole report.
fn diff_pair(a: &PageData, b: &PageData, joiner: &str, budget: &mut usize) -> PairDiff {
    let (ak, bk) = (&a.keys, &b.keys);
    let (n, m) = (ak.len(), bk.len());
    // Common prefix / suffix trim.
    let mut p = 0usize;
    while p < n && p < m && ak[p] == bk[p] {
        p += 1;
    }
    let mut s = 0usize;
    while s < n - p && s < m - p && ak[n - 1 - s] == bk[m - 1 - s] {
        s += 1;
    }
    let (ma, mb) = (n - p - s, m - p - s);

    // Beyond the DP budget: summarize coarsely with one replace hunk.
    if (ma + 1).saturating_mul(mb + 1) > MAX_DP_CELLS {
        let hunk = Hunk {
            op: "replace",
            removed: snippet(&a.tokens[p..n - s], joiner),
            added: snippet(&b.tokens[p..m - s], joiner),
            context_before: context_snippet(&a.tokens[..p], joiner, true),
            context_after: context_snippet(&a.tokens[n - s..], joiner, false),
        };
        let emit = *budget > 0;
        if emit {
            *budget -= 1;
        }
        return PairDiff {
            hunks: if emit { vec![hunk] } else { Vec::new() },
            added: mb,
            removed: ma,
            truncated: true,
        };
    }

    // LCS DP over the middle region (suffix LCS lengths, like text-diff).
    let (aw, bw) = (&ak[p..n - s], &bk[p..m - s]);
    let width = mb + 1;
    let mut dp = vec![0u32; (ma + 1) * width];
    for i in (0..ma).rev() {
        for j in (0..mb).rev() {
            dp[i * width + j] = if aw[i] == bw[j] {
                dp[(i + 1) * width + j + 1] + 1
            } else {
                dp[(i + 1) * width + j].max(dp[i * width + j + 1])
            };
        }
    }
    // Op stream over the FULL token lists (prefix + middle + suffix) so hunk
    // context handling is uniform.
    let mut ops: Vec<(Op, usize, usize)> = Vec::new(); // (op, a_idx, b_idx)
    for t in 0..p {
        ops.push((Op::Equal, t, t));
    }
    let (mut i, mut j) = (0usize, 0usize);
    while i < ma && j < mb {
        if aw[i] == bw[j] {
            ops.push((Op::Equal, p + i, p + j));
            i += 1;
            j += 1;
        } else if dp[(i + 1) * width + j] >= dp[i * width + j + 1] {
            ops.push((Op::Delete, p + i, 0));
            i += 1;
        } else {
            ops.push((Op::Insert, 0, p + j));
            j += 1;
        }
    }
    while i < ma {
        ops.push((Op::Delete, p + i, 0));
        i += 1;
    }
    while j < mb {
        ops.push((Op::Insert, 0, p + j));
        j += 1;
    }
    for t in 0..s {
        ops.push((Op::Equal, n - s + t, m - s + t));
    }

    // Group maximal non-equal runs into hunks with surrounding context.
    let mut hunks = Vec::new();
    let mut added = 0usize;
    let mut removed = 0usize;
    let mut truncated = false;
    let mut k = 0usize;
    while k < ops.len() {
        if ops[k].0 == Op::Equal {
            k += 1;
            continue;
        }
        let start = k;
        let mut rem_toks: Vec<String> = Vec::new();
        let mut add_toks: Vec<String> = Vec::new();
        while k < ops.len() && ops[k].0 != Op::Equal {
            match ops[k].0 {
                Op::Delete => rem_toks.push(a.tokens[ops[k].1].clone()),
                Op::Insert => add_toks.push(b.tokens[ops[k].2].clone()),
                Op::Equal => unreachable!(),
            }
            k += 1;
        }
        removed += rem_toks.len();
        added += add_toks.len();
        if hunks.len() >= MAX_HUNKS_PER_PAGE || *budget == 0 {
            truncated = true;
            continue;
        }
        *budget -= 1;
        // Context: equal tokens before `start` and after `k` (original text).
        let ctx_before: Vec<String> = {
            let mut v: Vec<String> = ops[..start]
                .iter()
                .rev()
                .filter(|(op, _, _)| *op == Op::Equal)
                .take(CONTEXT_TOKENS)
                .map(|(_, ai, _)| a.tokens[*ai].clone())
                .collect();
            v.reverse();
            v
        };
        let ctx_after: Vec<String> = ops[k..]
            .iter()
            .filter(|(op, _, _)| *op == Op::Equal)
            .take(CONTEXT_TOKENS)
            .map(|(_, ai, _)| a.tokens[*ai].clone())
            .collect();
        let op = if rem_toks.is_empty() {
            "insert"
        } else if add_toks.is_empty() {
            "delete"
        } else {
            "replace"
        };
        hunks.push(Hunk {
            op,
            removed: snippet(&rem_toks, joiner),
            added: snippet(&add_toks, joiner),
            context_before: ctx_before.join(joiner),
            context_after: ctx_after.join(joiner),
        });
    }
    PairDiff {
        hunks,
        added,
        removed,
        truncated,
    }
}

// ---------------------------------------------------------------------------
// Visual + metadata comparison
// ---------------------------------------------------------------------------

fn visual_changes_for_pair(a: &PageData, b: &PageData, out: &mut Vec<VisualChange>) {
    let pages = (Some(a.number), Some(b.number));
    if (a.width - b.width).abs() > 0.5 || (a.height - b.height).abs() > 0.5 {
        out.push(VisualChange {
            original_page: pages.0,
            revised_page: pages.1,
            kind: "page_size",
            detail: format!(
                "{:.0}×{:.0} pt → {:.0}×{:.0} pt",
                a.width, a.height, b.width, b.height
            ),
        });
    }
    if a.rotation != b.rotation {
        out.push(VisualChange {
            original_page: pages.0,
            revised_page: pages.1,
            kind: "rotation",
            detail: format!("page rotation {}° → {}°", a.rotation, b.rotation),
        });
    }
    if a.images != b.images {
        // Multiset diff over sorted hash vecs.
        let (mut i, mut j, mut common) = (0usize, 0usize, 0usize);
        while i < a.images.len() && j < b.images.len() {
            match a.images[i].cmp(&b.images[j]) {
                std::cmp::Ordering::Less => i += 1,
                std::cmp::Ordering::Greater => j += 1,
                std::cmp::Ordering::Equal => {
                    common += 1;
                    i += 1;
                    j += 1;
                }
            }
        }
        let only_a = a.images.len() - common;
        let only_b = b.images.len() - common;
        let replaced = only_a.min(only_b);
        let removed = only_a - replaced;
        let added = only_b - replaced;
        let mut parts = Vec::new();
        if replaced > 0 {
            parts.push(format!(
                "{replaced} image{} replaced",
                if replaced == 1 { "" } else { "s" }
            ));
        }
        if added > 0 {
            parts.push(format!(
                "{added} image{} added",
                if added == 1 { "" } else { "s" }
            ));
        }
        if removed > 0 {
            parts.push(format!(
                "{removed} image{} removed",
                if removed == 1 { "" } else { "s" }
            ));
        }
        out.push(VisualChange {
            original_page: pages.0,
            revised_page: pages.1,
            kind: "images",
            detail: parts.join(", "),
        });
    }
    if a.fonts != b.fonts {
        let added: Vec<&str> = b.fonts.difference(&a.fonts).map(|s| s.as_str()).collect();
        let removed: Vec<&str> = a.fonts.difference(&b.fonts).map(|s| s.as_str()).collect();
        let mut parts = Vec::new();
        if !added.is_empty() {
            parts.push(format!("fonts added: {}", added.join(", ")));
        }
        if !removed.is_empty() {
            parts.push(format!("fonts removed: {}", removed.join(", ")));
        }
        out.push(VisualChange {
            original_page: pages.0,
            revised_page: pages.1,
            kind: "fonts",
            detail: parts.join("; "),
        });
    }
}

/// Decode a PDF text string (UTF-16BE with BOM, else PDFDocEncoding treated as
/// Latin-1 — close enough for the ASCII range metadata usually holds).
fn pdf_text(bytes: &[u8]) -> String {
    if bytes.len() >= 2 && bytes[0] == 0xFE && bytes[1] == 0xFF {
        let units: Vec<u16> = bytes[2..]
            .chunks_exact(2)
            .map(|c| u16::from_be_bytes([c[0], c[1]]))
            .collect();
        char::decode_utf16(units)
            .map(|r| r.unwrap_or('\u{FFFD}'))
            .collect()
    } else {
        bytes.iter().map(|&b| b as char).collect()
    }
}

const METADATA_FIELDS: [&str; 8] = [
    "Title",
    "Author",
    "Subject",
    "Keywords",
    "Creator",
    "Producer",
    "CreationDate",
    "ModDate",
];

fn info_field(doc: &Document, field: &str) -> Option<String> {
    let info_raw = doc.trailer.get(b"Info").ok()?;
    let info = deref(doc, info_raw)?.as_dict().ok()?;
    let v = info.get(field.as_bytes()).ok()?;
    match deref(doc, v)? {
        Object::String(bytes, _) => Some(pdf_text(bytes)),
        Object::Name(n) => Some(String::from_utf8_lossy(n).to_string()),
        other => number(other).map(|f| f.to_string()),
    }
}

fn metadata_diff(a: &Document, b: &Document) -> Vec<MetadataChange> {
    let mut out = Vec::new();
    for field in METADATA_FIELDS {
        let va = info_field(a, field);
        let vb = info_field(b, field);
        if va != vb {
            out.push(MetadataChange {
                field: field.to_string(),
                original: va,
                revised: vb,
            });
        }
    }
    out
}

// ---------------------------------------------------------------------------
// Top-level diff
// ---------------------------------------------------------------------------

pub fn diff_pdfs(original: &[u8], revised: &[u8], opt: &Options) -> Result<Report, String> {
    let (a_pages, a_total, a_doc) = load_pages(original, "original", opt)?;
    let (b_pages, b_total, b_doc) = load_pages(revised, "revised", opt)?;

    let mut warnings: Vec<String> = Vec::new();

    // Alignment.
    let mut align_used = opt.align;
    if align_used == Align::Auto
        && (a_pages.len() > AUTO_ALIGN_MAX_PAGES || b_pages.len() > AUTO_ALIGN_MAX_PAGES)
    {
        align_used = Align::Sequential;
        warnings.push(format!(
            "auto page alignment is limited to {AUTO_ALIGN_MAX_PAGES} pages per document — \
             fell back to sequential (page N vs page N) alignment"
        ));
    }
    let (pairs, removed_idx, added_idx) = match align_used {
        Align::Auto => {
            let matched = align_auto(&a_pages, &b_pages);
            full_pairing(&matched, a_pages.len(), b_pages.len())
        }
        Align::Sequential => {
            let k = a_pages.len().min(b_pages.len());
            let pairs: Vec<(usize, usize)> = (0..k).map(|i| (i, i)).collect();
            let removed: Vec<usize> = (k..a_pages.len()).collect();
            let added: Vec<usize> = (k..b_pages.len()).collect();
            (pairs, removed, added)
        }
    };

    // Per-pair diffs.
    let joiner = if opt.mode == Mode::Words { " " } else { "\n" };
    let mut budget = MAX_HUNKS_TOTAL;
    let mut text_changes: Vec<PageTextChanges> = Vec::new();
    let mut visual_changes: Vec<VisualChange> = Vec::new();
    let mut unchanged_pairs: Vec<PagePair> = Vec::new();
    let mut changed_pages = 0usize;
    let mut budget_hit = false;
    for &(i, j) in &pairs {
        let (pa, pb) = (&a_pages[i], &b_pages[j]);
        let before = visual_changes.len();
        visual_changes_for_pair(pa, pb, &mut visual_changes);
        let visual_changed = visual_changes.len() > before;
        let text_changed = pa.keys != pb.keys;
        if text_changed {
            let d = diff_pair(pa, pb, joiner, &mut budget);
            if d.truncated && budget == 0 {
                budget_hit = true;
            }
            text_changes.push(PageTextChanges {
                original_page: pa.number,
                revised_page: pb.number,
                similarity: (jaccard(&pa.word_set, &pb.word_set) * 1000.0).round() / 1000.0,
                words_added: d.added,
                words_removed: d.removed,
                truncated: d.truncated,
                changes: d.hunks,
            });
        }
        if text_changed || visual_changed {
            changed_pages += 1;
        } else {
            unchanged_pairs.push(PagePair {
                original_page: pa.number,
                revised_page: pb.number,
            });
        }
    }
    if budget_hit {
        warnings.push(format!(
            "diff output truncated at {MAX_HUNKS_TOTAL} changes — per-page word counts remain \
             exact"
        ));
    }

    let removed_pages: Vec<u32> = removed_idx.iter().map(|&i| a_pages[i].number).collect();
    let added_pages: Vec<u32> = added_idx.iter().map(|&j| b_pages[j].number).collect();

    let meta = metadata_diff(&a_doc, &b_doc);

    // Warnings about text quality.
    let a_dropped: usize = a_pages.iter().map(|p| p.dropped_chunks).sum();
    let b_dropped: usize = b_pages.iter().map(|p| p.dropped_chunks).sum();
    if a_dropped + b_dropped > 0 {
        warnings.push(format!(
            "{} text run{} could not be decoded (unparseable font encoding) — the text diff may \
             be partial",
            a_dropped + b_dropped,
            if a_dropped + b_dropped == 1 { "" } else { "s" }
        ));
    }
    let a_empty = a_pages.iter().all(|p| p.tokens.is_empty());
    let b_empty = b_pages.iter().all(|p| p.tokens.is_empty());
    if a_empty && b_empty {
        warnings.push(
            "neither PDF has a selectable text layer on the compared pages (scanned/image-only \
             PDFs need OCR, which this tool does not do) — only object-level visual comparison \
             was possible"
                .to_string(),
        );
    } else if a_empty || b_empty {
        warnings.push(format!(
            "the {} PDF has no selectable text layer on the compared pages — its text compares \
             as empty",
            if a_empty { "original" } else { "revised" }
        ));
    }

    let identical = text_changes.is_empty()
        && visual_changes.is_empty()
        && meta.is_empty()
        && removed_pages.is_empty()
        && added_pages.is_empty();

    // Summary.
    let compared = pairs.len();
    let summary = if identical {
        format!(
            "No differences found: the two PDFs have identical text, page objects and metadata \
             across the {compared} compared page pair{}.",
            if compared == 1 { "" } else { "s" }
        )
    } else {
        let mut parts: Vec<String> = Vec::new();
        parts.push(format!(
            "{changed_pages} of {compared} compared page pair{} differ",
            if compared == 1 { "" } else { "s" }
        ));
        if !added_pages.is_empty() {
            parts.push(format!(
                "{} page{} added in the revised PDF",
                added_pages.len(),
                if added_pages.len() == 1 { "" } else { "s" }
            ));
        }
        if !removed_pages.is_empty() {
            parts.push(format!(
                "{} page{} removed from the original",
                removed_pages.len(),
                if removed_pages.len() == 1 { "" } else { "s" }
            ));
        }
        if !meta.is_empty() {
            let fields: Vec<&str> = meta.iter().map(|m| m.field.as_str()).collect();
            parts.push(format!("metadata changed ({})", fields.join(", ")));
        }
        let mut s = parts.join("; ");
        s.push('.');
        s
    };

    Ok(Report {
        identical,
        summary,
        pages: PagesSummary {
            original: a_total,
            revised: b_total,
            compared_pairs: compared,
            changed: changed_pages,
            unchanged: compared - changed_pages,
            added_in_revised: added_pages,
            removed_from_original: removed_pages,
        },
        page_alignment: match align_used {
            Align::Auto => "auto",
            Align::Sequential => "sequential",
        },
        text_changes,
        visual_changes,
        metadata_changes: meta,
        unchanged_pages: opt.include_unchanged.then_some(unchanged_pairs),
        warnings,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::content::{Content, Operation};
    use lopdf::{dictionary, Dictionary, Stream};

    /// Build a PDF; each entry becomes one page drawing that string in
    /// Helvetica. `title` sets /Info /Title when non-empty.
    fn build_pdf(pages_text: &[&str], title: &str) -> Vec<u8> {
        build_pdf_full(pages_text, title, "Helvetica", None)
    }

    fn build_pdf_full(
        pages_text: &[&str],
        title: &str,
        font: &str,
        image_on_page: Option<usize>,
    ) -> Vec<u8> {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let font_id = doc.add_object(dictionary! {
            "Type" => "Font",
            "Subtype" => "Type1",
            "BaseFont" => font,
        });

        let mut kids: Vec<Object> = Vec::new();
        for (idx, text) in pages_text.iter().enumerate() {
            let mut res = Dictionary::new();
            res.set("Font", dictionary! { "F1" => font_id });
            if image_on_page == Some(idx + 1) {
                let img = Stream::new(
                    dictionary! {
                        "Type" => "XObject",
                        "Subtype" => "Image",
                        "Width" => 2,
                        "Height" => 2,
                        "ColorSpace" => "DeviceGray",
                        "BitsPerComponent" => 8,
                    },
                    vec![0u8, 64, 128, 255],
                );
                let img_id = doc.add_object(img);
                res.set("XObject", dictionary! { "Im0" => img_id });
            }
            let resources_id = doc.add_object(Object::Dictionary(res));
            let content = Content {
                operations: vec![
                    Operation::new("BT", vec![]),
                    Operation::new("Tf", vec!["F1".into(), 24.into()]),
                    Operation::new("Td", vec![100.into(), 600.into()]),
                    Operation::new("Tj", vec![Object::string_literal(*text)]),
                    Operation::new("ET", vec![]),
                ],
            };
            let content_id =
                doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
            let page_id = doc.add_object(dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "Contents" => content_id,
                "Resources" => resources_id,
                "MediaBox" => vec![0.into(), 0.into(), 612.into(), 792.into()],
            });
            kids.push(page_id.into());
        }

        let count = kids.len() as i64;
        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! {
                "Type" => "Pages",
                "Kids" => kids,
                "Count" => count,
            }),
        );
        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        doc.trailer.set("Root", catalog_id);
        if !title.is_empty() {
            let info_id =
                doc.add_object(dictionary! { "Title" => Object::string_literal(title) });
            doc.trailer.set("Info", info_id);
        }

        let mut buf = Vec::new();
        doc.save_to(&mut buf).unwrap();
        buf
    }

    #[test]
    fn identical_pdfs_report_identical() {
        let pdf = build_pdf(&["Alpha beta gamma", "Second page here"], "Doc");
        let r = diff_pdfs(&pdf, &pdf, &Options::default()).unwrap();
        assert!(r.identical, "{r:?}");
        assert_eq!(r.pages.changed, 0);
        assert_eq!(r.pages.unchanged, 2);
        assert_eq!(r.pages.compared_pairs, 2);
        assert!(r.summary.contains("No differences"), "{}", r.summary);
    }

    #[test]
    fn word_change_is_reported_as_replace_hunk() {
        let a = build_pdf(&["Payment is due within 30 days of invoice"], "");
        let b = build_pdf(&["Payment is due within 60 days of invoice"], "");
        let r = diff_pdfs(&a, &b, &Options::default()).unwrap();
        assert!(!r.identical);
        assert_eq!(r.pages.changed, 1);
        assert_eq!(r.text_changes.len(), 1);
        let tc = &r.text_changes[0];
        assert_eq!((tc.original_page, tc.revised_page), (1, 1));
        assert_eq!(tc.words_added, 1);
        assert_eq!(tc.words_removed, 1);
        assert_eq!(tc.changes.len(), 1);
        let h = &tc.changes[0];
        assert_eq!(h.op, "replace");
        assert_eq!(h.removed, "30");
        assert_eq!(h.added, "60");
        assert!(h.context_before.contains("within"), "{h:?}");
        assert!(h.context_after.contains("days"), "{h:?}");
    }

    #[test]
    fn inserted_page_is_detected_by_auto_alignment() {
        let a = build_pdf(&["Chapter one text here", "Chapter two text here"], "");
        let b = build_pdf(
            &[
                "Chapter one text here",
                "A brand new inserted page",
                "Chapter two text here",
            ],
            "",
        );
        let r = diff_pdfs(&a, &b, &Options::default()).unwrap();
        assert_eq!(r.page_alignment, "auto");
        assert_eq!(r.pages.added_in_revised, vec![2]);
        assert!(r.pages.removed_from_original.is_empty());
        assert_eq!(r.pages.changed, 0, "existing pages unchanged: {r:?}");
        assert!(!r.identical);
        assert!(r.summary.contains("1 page added"), "{}", r.summary);
    }

    #[test]
    fn sequential_alignment_cascades_on_insertion() {
        let a = build_pdf(&["Chapter one text here", "Chapter two text here"], "");
        let b = build_pdf(
            &[
                "Chapter one text here",
                "A brand new inserted page",
                "Chapter two text here",
            ],
            "",
        );
        let opt = Options {
            align: Align::Sequential,
            ..Options::default()
        };
        let r = diff_pdfs(&a, &b, &opt).unwrap();
        assert_eq!(r.page_alignment, "sequential");
        // Page 2 now differs and page 3 counts as added.
        assert_eq!(r.pages.changed, 1);
        assert_eq!(r.pages.added_in_revised, vec![3]);
    }

    #[test]
    fn rewritten_page_pairs_as_changed_not_removed_plus_added() {
        let a = build_pdf(
            &["Shared first page words", "old body entirely different"],
            "",
        );
        let b = build_pdf(
            &["Shared first page words", "completely new replacement text"],
            "",
        );
        let r = diff_pdfs(&a, &b, &Options::default()).unwrap();
        assert!(r.pages.added_in_revised.is_empty(), "{r:?}");
        assert!(r.pages.removed_from_original.is_empty(), "{r:?}");
        assert_eq!(r.pages.changed, 1);
        assert!(r.text_changes[0].similarity < 0.3);
    }

    #[test]
    fn ignore_case_suppresses_case_only_changes() {
        let a = build_pdf(&["Hello World"], "");
        let b = build_pdf(&["hello world"], "");
        let r = diff_pdfs(&a, &b, &Options::default()).unwrap();
        assert!(!r.identical, "case change is a diff by default");
        let opt = Options {
            ignore_case: true,
            ..Options::default()
        };
        let r = diff_pdfs(&a, &b, &opt).unwrap();
        assert!(r.identical, "{r:?}");
    }

    #[test]
    fn pages_filter_limits_comparison() {
        let a = build_pdf(&["same page one", "different old"], "");
        let b = build_pdf(&["same page one", "different new"], "");
        let opt = Options {
            pages: "1".to_string(),
            ..Options::default()
        };
        let r = diff_pdfs(&a, &b, &opt).unwrap();
        assert!(r.identical, "page 2's change must be filtered out: {r:?}");
        assert_eq!(r.pages.compared_pairs, 1);
        // Totals still report the full documents.
        assert_eq!(r.pages.original, 2);
        assert_eq!(r.pages.revised, 2);
    }

    #[test]
    fn lines_mode_diffs_whole_lines() {
        let a = build_pdf(&["alpha beta"], "");
        let b = build_pdf(&["alpha gamma"], "");
        let opt = Options {
            mode: Mode::Lines,
            ..Options::default()
        };
        let r = diff_pdfs(&a, &b, &opt).unwrap();
        assert_eq!(r.text_changes.len(), 1);
        let h = &r.text_changes[0].changes[0];
        assert_eq!(h.op, "replace");
        assert!(h.removed.contains("alpha beta"), "{h:?}");
        assert!(h.added.contains("alpha gamma"), "{h:?}");
    }

    #[test]
    fn metadata_title_change_is_reported() {
        let a = build_pdf(&["same text"], "Contract v1");
        let b = build_pdf(&["same text"], "Contract v2");
        let r = diff_pdfs(&a, &b, &Options::default()).unwrap();
        assert!(!r.identical);
        assert_eq!(r.pages.changed, 0);
        assert_eq!(r.metadata_changes.len(), 1);
        let m = &r.metadata_changes[0];
        assert_eq!(m.field, "Title");
        assert_eq!(m.original.as_deref(), Some("Contract v1"));
        assert_eq!(m.revised.as_deref(), Some("Contract v2"));
        assert!(
            r.summary.contains("metadata changed (Title)"),
            "{}",
            r.summary
        );
    }

    #[test]
    fn added_image_is_a_visual_change() {
        let a = build_pdf_full(&["look at this"], "", "Helvetica", None);
        let b = build_pdf_full(&["look at this"], "", "Helvetica", Some(1));
        let r = diff_pdfs(&a, &b, &Options::default()).unwrap();
        assert_eq!(r.pages.changed, 1, "{r:?}");
        assert!(r.text_changes.is_empty(), "text is unchanged");
        let img = r.visual_changes.iter().find(|c| c.kind == "images").unwrap();
        assert_eq!(img.detail, "1 image added");
        assert_eq!(img.original_page, Some(1));
    }

    #[test]
    fn font_change_is_a_visual_change() {
        let a = build_pdf_full(&["styled text"], "", "Helvetica", None);
        let b = build_pdf_full(&["styled text"], "", "Courier", None);
        let r = diff_pdfs(&a, &b, &Options::default()).unwrap();
        let f = r.visual_changes.iter().find(|c| c.kind == "fonts").unwrap();
        assert!(f.detail.contains("fonts added: Courier"), "{f:?}");
        assert!(f.detail.contains("fonts removed: Helvetica"), "{f:?}");
    }

    #[test]
    fn include_unchanged_lists_pairs() {
        let pdf = build_pdf(&["one", "two"], "");
        let opt = Options {
            include_unchanged: true,
            ..Options::default()
        };
        let r = diff_pdfs(&pdf, &pdf, &opt).unwrap();
        let u = r.unchanged_pages.unwrap();
        assert_eq!(u.len(), 2);
        assert_eq!((u[0].original_page, u[0].revised_page), (1, 1));
    }

    #[test]
    fn non_pdf_bytes_error_names_the_side() {
        let pdf = build_pdf(&["fine"], "");
        let err = diff_pdfs(b"not a pdf at all", &pdf, &Options::default()).unwrap_err();
        assert!(err.contains("original"), "{err}");
        let err = diff_pdfs(&pdf, b"not a pdf at all", &Options::default()).unwrap_err();
        assert!(err.contains("revised"), "{err}");
    }

    #[test]
    fn encrypted_pdf_gets_a_clear_error() {
        // A trailer /Encrypt entry marks the document password-protected.
        let mut doc = Document::load_mem(&build_pdf(&["secret"], "")).unwrap();
        let enc_id = doc.add_object(dictionary! { "Filter" => "Standard" });
        doc.trailer.set("Encrypt", enc_id);
        let mut buf = Vec::new();
        doc.save_to(&mut buf).unwrap();
        let plain = build_pdf(&["public"], "");
        let err = diff_pdfs(&buf, &plain, &Options::default()).unwrap_err();
        assert!(err.contains("password"), "{err}");
    }

    #[test]
    fn empty_input_errors() {
        let pdf = build_pdf(&["fine"], "");
        let err = diff_pdfs(b"", &pdf, &Options::default()).unwrap_err();
        assert!(err.contains("original") && err.contains("empty"), "{err}");
    }

    #[test]
    fn bad_page_spec_errors() {
        let pdf = build_pdf(&["fine"], "");
        let opt = Options {
            pages: "x-3".to_string(),
            ..Options::default()
        };
        let err = diff_pdfs(&pdf, &pdf, &opt).unwrap_err();
        assert!(err.contains("invalid page"), "{err}");
        let opt = Options {
            pages: "7".to_string(),
            ..Options::default()
        };
        let err = diff_pdfs(&pdf, &pdf, &opt).unwrap_err();
        assert!(err.contains("selects no pages"), "{err}");
    }

    #[test]
    fn parse_page_spec_grammar() {
        assert_eq!(
            parse_page_spec("1,3-5", 10).unwrap(),
            [1, 3, 4, 5].into_iter().collect::<BTreeSet<u32>>()
        );
        assert_eq!(
            parse_page_spec("odd", 5).unwrap(),
            [1, 3, 5].into_iter().collect::<BTreeSet<u32>>()
        );
        assert_eq!(
            parse_page_spec("even", 5).unwrap(),
            [2, 4].into_iter().collect::<BTreeSet<u32>>()
        );
        // Out-of-range pages are clamped/ignored, not an error.
        assert_eq!(
            parse_page_spec("2-99", 4).unwrap(),
            [2, 3, 4].into_iter().collect::<BTreeSet<u32>>()
        );
        assert!(parse_page_spec("0", 4).is_err());
        assert!(parse_page_spec("a", 4).is_err());
    }
}
