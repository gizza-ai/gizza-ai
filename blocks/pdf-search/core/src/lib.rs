//! gizza-ai/pdf-search core — literal word/phrase search over a PDF's embedded
//! selectable text layer. No wafer/wasm-bindgen deps so it compiles natively
//! for unit tests and to `wasm32-wasip1` (the `wafer build` target) for the
//! block.
//!
//! Backed by `lopdf` (pure-Rust, no native libs) with `default-features =
//! false` — the same read-only PDF path as `blocks/pdf-extract-text`: `rayon`
//! (threads) and the date crates are disabled so the tree links cleanly on
//! wasm.
//!
//! ## What it does
//!
//! Extracts each page's text, normalizes whitespace (so a phrase query matches
//! across line breaks), and finds every literal occurrence of the query. Each
//! hit is returned with its 1-based page number and a surrounding-context
//! snippet in which the matched span is wrapped in guillemets `«…»`.
//!
//! ## Limits (stated, not worked around)
//!
//! - **Text-layer PDFs only.** Reads the embedded selectable text; it does not
//!   OCR scanned/image-only PDFs (those legitimately return zero matches). Same
//!   limit as `pdf-extract-text` — gizza is pure-Rust with no OCR model.
//! - **Literal search, not regex.** The query is a literal word/phrase.
//! - **ASCII-accurate case folding.** Case-insensitive matching folds each
//!   character to the first `char` of its lowercase mapping (1:1, so snippet
//!   indices stay aligned); it does not do diacritic-insensitive folding.
//!
//! ## Per-chunk failure isolation
//!
//! `lopdf` cannot parse every font's `ToUnicode` CMap. Rather than fail the
//! whole document, we use `extract_text_chunks`, keep every `Ok` run, and count
//! the `Err` ones — the search runs over the text we *can* decode plus a count
//! of skipped runs (see [`SearchOutput::dropped_chunks`]).

use lopdf::Document;

/// Maximum number of pages we will iterate. A guard against a pathological PDF
/// claiming an absurd page count.
const MAX_PAGES: usize = 10_000;

/// A single search hit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchMatch {
    /// 1-based page number the match was found on.
    pub page: usize,
    /// Surrounding-context snippet with the matched span wrapped in `«…»`.
    /// A leading/trailing `…` marks context clipped at the snippet boundary.
    pub snippet: String,
}

/// Options controlling how the query is matched. See the block's descriptor for
/// the user-facing parameter names/defaults.
#[derive(Debug, Clone)]
pub struct SearchOptions {
    /// Match case exactly (`true`) or case-insensitively (`false`, default).
    pub case_sensitive: bool,
    /// Require alphanumeric word boundaries on both sides of the match.
    pub whole_word: bool,
    /// Number of characters of context to show on each side of a match.
    pub context: usize,
    /// Maximum number of match snippets to return. `total_matches` still counts
    /// every occurrence; `truncated` flags when the returned list was capped.
    pub max_matches: usize,
}

/// Result of a search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchOutput {
    /// Matches in document order, capped at `max_matches`.
    pub matches: Vec<SearchMatch>,
    /// Total occurrences found across the whole document (may exceed
    /// `matches.len()` when the list was capped).
    pub total_matches: usize,
    /// Number of distinct pages that contained at least one match.
    pub pages_matched: usize,
    /// True when `matches` was capped at `max_matches` (i.e. `total_matches`
    /// exceeded the cap).
    pub truncated: bool,
    /// Count of text runs skipped because their font encoding could not be
    /// decoded, so the searched text is partial. Zero means a clean extraction.
    pub dropped_chunks: usize,
}

/// Fold one character for matching. Case-insensitive folding takes the first
/// `char` of the lowercase mapping so the folded string stays 1:1 with the
/// original (snippet slicing indexes into the original chars).
fn fold_char(c: char, case_sensitive: bool) -> char {
    if case_sensitive {
        c
    } else {
        c.to_lowercase().next().unwrap_or(c)
    }
}

/// Collapse every run of Unicode whitespace to a single space and trim the
/// ends. This lets a phrase query match across line breaks (a PDF's text layer
/// often splits words/lines arbitrarily).
fn normalize_ws(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_space = false;
    for c in s.chars() {
        if c.is_whitespace() {
            if !prev_space {
                out.push(' ');
                prev_space = true;
            }
        } else {
            out.push(c);
            prev_space = false;
        }
    }
    out.trim().to_string()
}

/// Extract each page's normalized text, in ascending page order, alongside the
/// count of text runs that could not be decoded.
fn page_texts(bytes: &[u8]) -> Result<(Vec<String>, usize), String> {
    let doc = Document::load_mem(bytes).map_err(|e| format!("failed to parse PDF: {e}"))?;

    let mut page_numbers: Vec<u32> = doc.get_pages().keys().copied().collect();
    page_numbers.sort_unstable();

    if page_numbers.is_empty() {
        return Err("PDF has no pages".to_string());
    }
    if page_numbers.len() > MAX_PAGES {
        return Err(format!(
            "PDF has too many pages: {} (cap {MAX_PAGES})",
            page_numbers.len()
        ));
    }

    let mut texts: Vec<String> = Vec::with_capacity(page_numbers.len());
    let mut dropped_chunks = 0usize;
    for n in page_numbers {
        let mut page_text = String::new();
        for chunk in doc.extract_text_chunks(&[n]) {
            match chunk {
                Ok(t) => page_text.push_str(&t),
                Err(_) => dropped_chunks += 1,
            }
        }
        texts.push(normalize_ws(&page_text));
    }
    Ok((texts, dropped_chunks))
}

/// Build a `«…»`-wrapped snippet for a match at char range `start..end` within
/// `chars` (the page's normalized characters), showing up to `context`
/// characters on each side.
fn build_snippet(chars: &[char], start: usize, end: usize, context: usize) -> String {
    let ctx_start = start.saturating_sub(context);
    let ctx_end = (end + context).min(chars.len());

    let mut s = String::new();
    if ctx_start > 0 {
        s.push('…');
    }
    s.extend(&chars[ctx_start..start]);
    s.push('«');
    s.extend(&chars[start..end]);
    s.push('»');
    s.extend(&chars[end..ctx_end]);
    if ctx_end < chars.len() {
        s.push('…');
    }
    s
}

/// Find every literal occurrence of `query` on one page's normalized `chars`,
/// pushing `(start, end)` char ranges (non-overlapping) into `out`.
fn find_on_page(
    chars: &[char],
    needle: &[char],
    case_sensitive: bool,
    whole_word: bool,
    out: &mut Vec<(usize, usize)>,
) {
    if needle.is_empty() || needle.len() > chars.len() {
        return;
    }
    // Fold the haystack once (1:1 with `chars`).
    let folded: Vec<char> = chars.iter().map(|&c| fold_char(c, case_sensitive)).collect();

    let mut i = 0;
    while i + needle.len() <= folded.len() {
        if folded[i..i + needle.len()] == *needle {
            let end = i + needle.len();
            let boundary_ok = !whole_word || {
                let before_ok = i == 0 || !chars[i - 1].is_alphanumeric();
                let after_ok = end == chars.len() || !chars[end].is_alphanumeric();
                before_ok && after_ok
            };
            if boundary_ok {
                out.push((i, end));
                i = end; // non-overlapping
                continue;
            }
        }
        i += 1;
    }
}

/// Search a PDF's embedded text for a literal word/phrase.
///
/// - `bytes` — the raw PDF file.
/// - `query` — the literal word/phrase to find. Whitespace is normalized, so a
///   multi-word phrase matches even where the PDF split it across lines.
/// - `opts` — matching + output options.
///
/// Returns the matches (page-numbered snippets) plus totals, or `Err` when the
/// bytes don't parse as a PDF, the PDF has no pages, or `query` is empty after
/// whitespace normalization.
pub fn search(bytes: &[u8], query: &str, opts: &SearchOptions) -> Result<SearchOutput, String> {
    let needle_str = normalize_ws(query);
    if needle_str.is_empty() {
        return Err("query is empty".to_string());
    }
    let needle: Vec<char> = needle_str
        .chars()
        .map(|c| fold_char(c, opts.case_sensitive))
        .collect();

    let (texts, dropped_chunks) = page_texts(bytes)?;

    let mut matches: Vec<SearchMatch> = Vec::new();
    let mut total_matches = 0usize;
    let mut pages_matched = 0usize;

    for (idx, text) in texts.iter().enumerate() {
        let chars: Vec<char> = text.chars().collect();
        let mut ranges: Vec<(usize, usize)> = Vec::new();
        find_on_page(
            &chars,
            &needle,
            opts.case_sensitive,
            opts.whole_word,
            &mut ranges,
        );
        if ranges.is_empty() {
            continue;
        }
        pages_matched += 1;
        total_matches += ranges.len();
        let page = idx + 1;
        for (start, end) in ranges {
            if matches.len() < opts.max_matches {
                matches.push(SearchMatch {
                    page,
                    snippet: build_snippet(&chars, start, end, opts.context),
                });
            }
        }
    }

    let truncated = total_matches > matches.len();

    Ok(SearchOutput {
        matches,
        total_matches,
        pages_matched,
        truncated,
        dropped_chunks,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::content::{Content, Operation};
    use lopdf::{dictionary, Document, Object, Stream};

    /// Build a minimal multi-page PDF; each entry in `pages_text` becomes one
    /// page whose content stream draws that string.
    fn build_pdf(pages_text: &[&str]) -> Vec<u8> {
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

        let mut kids: Vec<Object> = Vec::new();
        for text in pages_text {
            let content = Content {
                operations: vec![
                    Operation::new("BT", vec![]),
                    Operation::new("Tf", vec!["F1".into(), 24.into()]),
                    Operation::new("Td", vec![100.into(), 600.into()]),
                    Operation::new("Tj", vec![Object::string_literal(*text)]),
                    Operation::new("ET", vec![]),
                ],
            };
            let content_id = doc.add_object(Stream::new(dictionary! {}, content.encode().unwrap()));
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
        let pages = dictionary! {
            "Type" => "Pages",
            "Kids" => kids,
            "Count" => count,
        };
        doc.objects.insert(pages_id, Object::Dictionary(pages));
        let catalog_id = doc.add_object(dictionary! {
            "Type" => "Catalog",
            "Pages" => pages_id,
        });
        doc.trailer.set("Root", catalog_id);

        let mut buf = Vec::new();
        doc.save_to(&mut buf).unwrap();
        buf
    }

    fn opts(context: usize, max_matches: usize) -> SearchOptions {
        SearchOptions {
            case_sensitive: false,
            whole_word: false,
            context,
            max_matches,
        }
    }

    #[test]
    fn happy_path_finds_word_with_page_and_guillemets() {
        let pdf = build_pdf(&["The quick brown fox", "jumps over the lazy dog"]);
        let out = search(&pdf, "fox", &opts(60, 100)).unwrap();
        assert_eq!(out.total_matches, 1);
        assert_eq!(out.pages_matched, 1);
        assert!(!out.truncated);
        assert_eq!(out.matches[0].page, 1);
        assert!(
            out.matches[0].snippet.contains("«fox»"),
            "snippet: {}",
            out.matches[0].snippet
        );
    }

    #[test]
    fn rejects_non_pdf_bytes() {
        let err = search(b"definitely not a pdf", "x", &opts(60, 100)).unwrap_err();
        assert!(err.contains("failed to parse PDF"), "got: {err}");
    }

    #[test]
    fn empty_query_errors() {
        let pdf = build_pdf(&["something"]);
        let err = search(&pdf, "   ", &opts(60, 100)).unwrap_err();
        assert!(err.contains("query is empty"), "got: {err}");
    }

    #[test]
    fn case_insensitive_by_default_and_case_sensitive_opt_in() {
        let pdf = build_pdf(&["Hello hello HELLO"]);
        let ci = search(&pdf, "hello", &opts(60, 100)).unwrap();
        assert_eq!(ci.total_matches, 3, "case-insensitive matches all three");

        let cs = SearchOptions {
            case_sensitive: true,
            ..opts(60, 100)
        };
        let out = search(&pdf, "hello", &cs).unwrap();
        assert_eq!(out.total_matches, 1, "only the exact-case 'hello'");
    }

    #[test]
    fn whole_word_excludes_substrings() {
        let pdf = build_pdf(&["cat category concatenate cat"]);
        let loose = search(&pdf, "cat", &opts(60, 100)).unwrap();
        assert_eq!(loose.total_matches, 4, "substring hits count without whole_word");

        let ww = SearchOptions {
            whole_word: true,
            ..opts(60, 100)
        };
        let out = search(&pdf, "cat", &ww).unwrap();
        assert_eq!(out.total_matches, 2, "only the two standalone 'cat' tokens");
    }

    #[test]
    fn phrase_matches_across_normalized_whitespace() {
        // Two words drawn as separate runs collapse to a single space, so the
        // phrase query spans what was a line break.
        let pdf = build_pdf(&["quick brown fox"]);
        let out = search(&pdf, "quick  brown", &opts(60, 100)).unwrap();
        assert_eq!(out.total_matches, 1);
        assert!(out.matches[0].snippet.contains("«quick brown»"));
    }

    #[test]
    fn context_limits_snippet_width() {
        let pdf = build_pdf(&["aaaaaaaaaa TARGET bbbbbbbbbb"]);
        let out = search(&pdf, "TARGET", &opts(3, 100)).unwrap();
        let snip = &out.matches[0].snippet;
        // 3 chars of context each side, including the spaces adjacent to TARGET;
        // ellipses appear on both clipped ends.
        assert!(snip.contains("aa «TARGET» bb"), "snippet: {snip}");
        assert!(snip.starts_with('…') && snip.ends_with('…'), "snippet: {snip}");
    }

    #[test]
    fn max_matches_caps_and_flags_truncation() {
        let pdf = build_pdf(&["x x x x x"]);
        let out = search(&pdf, "x", &opts(60, 2)).unwrap();
        assert_eq!(out.total_matches, 5, "total counts every occurrence");
        assert_eq!(out.matches.len(), 2, "returned list is capped");
        assert!(out.truncated);
    }

    #[test]
    fn no_match_returns_empty() {
        let pdf = build_pdf(&["nothing to see here"]);
        let out = search(&pdf, "absent", &opts(60, 100)).unwrap();
        assert_eq!(out.total_matches, 0);
        assert_eq!(out.pages_matched, 0);
        assert!(out.matches.is_empty());
        assert!(!out.truncated);
    }

    #[test]
    fn counts_pages_matched_across_pages() {
        let pdf = build_pdf(&["alpha beam", "beam gamma", "delta"]);
        let out = search(&pdf, "beam", &opts(60, 100)).unwrap();
        assert_eq!(out.total_matches, 2);
        assert_eq!(out.pages_matched, 2);
        assert_eq!(out.matches[0].page, 1);
        assert_eq!(out.matches[1].page, 2);
    }
}
