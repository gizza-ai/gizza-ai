//! emoji-search core — search a curated, embedded emoji dataset by keyword,
//! shortcode (`:smile:`), or category. No wafer/wasm-bindgen deps. Shared by the
//! chat skill block and the web page.
//!
//! The dataset is a hand-curated table of common emoji (`glyph`, `name`,
//! `shortcode`, `category`, and a small set of search `keywords`). It is fully
//! offline and deterministic — no network, no Unicode data file at runtime.

/// One emoji record in the embedded dataset.
#[derive(Clone, Copy)]
pub struct Emoji {
    /// The emoji glyph itself, e.g. "😀".
    pub glyph: &'static str,
    /// Canonical name, e.g. "grinning face".
    pub name: &'static str,
    /// Short code without surrounding colons, e.g. "grinning".
    pub shortcode: &'static str,
    /// Category bucket, e.g. "smileys".
    pub category: &'static str,
    /// Extra search keywords (the name + shortcode are always searchable too).
    pub keywords: &'static [&'static str],
}

/// Maximum number of matches `search` will return. The LLM-facing `limit`
/// param is clamped to `1..=MAX_LIMIT`.
pub const MAX_LIMIT: u32 = 100;

/// Default match limit when the caller does not specify one.
pub const DEFAULT_LIMIT: u32 = 20;

include!("data.rs");

/// A single match, scored so the best matches sort first.
struct Scored {
    e: &'static Emoji,
    score: u32,
}

/// Normalize a query/token for matching: lowercase, trim, strip surrounding
/// colons (so a shortcode query like `:smile:` matches `smile`).
fn norm(s: &str) -> String {
    s.trim().trim_matches(':').to_ascii_lowercase()
}

/// Score a single emoji against a normalized query. Returns `None` for no match.
/// Higher is better: exact shortcode/name > word-start prefix > substring.
fn score(e: &Emoji, q: &str) -> Option<u32> {
    let name = e.name.to_ascii_lowercase();
    let shortcode = e.shortcode.to_ascii_lowercase();
    let category = e.category.to_ascii_lowercase();

    // Exact shortcode or name is the strongest signal.
    if shortcode == q || name == q {
        return Some(100);
    }
    // Keyword exact match.
    if e.keywords.iter().any(|k| k.eq_ignore_ascii_case(q)) {
        return Some(80);
    }
    // The query matches the whole category — list everything in it.
    if category == q {
        return Some(60);
    }

    let mut best = 0;
    // Prefix of the name or shortcode.
    if name.starts_with(q) || shortcode.starts_with(q) {
        best = best.max(70);
    }
    // Word-start prefix inside the name (e.g. "face" in "grinning face").
    if name.split_whitespace().any(|w| w.starts_with(q)) {
        best = best.max(50);
    }
    // Keyword prefix.
    if e.keywords.iter().any(|k| k.to_ascii_lowercase().starts_with(q)) {
        best = best.max(45);
    }
    // Substring anywhere in name/shortcode/keywords.
    if name.contains(q)
        || shortcode.contains(q)
        || e.keywords.iter().any(|k| k.to_ascii_lowercase().contains(q))
    {
        best = best.max(30);
    }
    if best > 0 {
        Some(best)
    } else {
        None
    }
}

/// Search the embedded dataset for `query`, returning up to `limit` matches
/// (clamped to `1..=MAX_LIMIT`; `0` → `DEFAULT_LIMIT`). Matches are ranked by
/// relevance; ties break alphabetically by name for a stable order. Returns
/// `Err` when `query` is empty/whitespace.
///
/// The query is matched (case-insensitively, colons stripped) against each
/// emoji's name, shortcode, category, and keywords.
pub fn search(query: &str, limit: u32) -> Result<Vec<&'static Emoji>, String> {
    let q = norm(query);
    if q.is_empty() {
        return Err("query is empty: enter a keyword, :shortcode:, or category".into());
    }
    let cap = if limit == 0 {
        DEFAULT_LIMIT
    } else {
        limit.clamp(1, MAX_LIMIT)
    } as usize;

    let mut hits: Vec<Scored> = EMOJI
        .iter()
        .filter_map(|e| score(e, &q).map(|s| Scored { e, score: s }))
        .collect();

    // Sort by score desc, then name asc for a deterministic order.
    hits.sort_by(|a, b| b.score.cmp(&a.score).then_with(|| a.e.name.cmp(b.e.name)));
    Ok(hits.into_iter().take(cap).map(|h| h.e).collect())
}

/// Render search results as a compact, human/LLM-friendly text block — one
/// emoji per line: `<glyph>  :<shortcode>:  <name>  (<category>)`. A no-match
/// result returns a friendly note rather than an empty string.
pub fn run(query: &str, limit: u32) -> Result<String, String> {
    let hits = search(query, limit)?;
    if hits.is_empty() {
        return Ok(format!("No emoji found for {query:?}."));
    }
    let lines: Vec<String> = hits
        .iter()
        .map(|e| {
            format!(
                "{}  :{}:  {}  ({})",
                e.glyph, e.shortcode, e.name, e.category
            )
        })
        .collect();
    Ok(lines.join("\n"))
}

/// Convenience: just the glyphs of the matches, space-separated (e.g. for a
/// quick "give me party emoji" reply). Same matching/limit semantics as `run`.
pub fn glyphs(query: &str, limit: u32) -> Result<String, String> {
    let hits = search(query, limit)?;
    Ok(hits
        .iter()
        .map(|e| e.glyph)
        .collect::<Vec<_>>()
        .join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dataset_is_non_trivial_and_well_formed() {
        assert!(EMOJI.len() >= 150, "dataset should be reasonably large");
        for e in EMOJI {
            assert!(!e.glyph.is_empty(), "empty glyph for {}", e.name);
            assert!(!e.name.is_empty());
            assert!(!e.shortcode.is_empty());
            assert!(!e.category.is_empty());
            assert!(
                !e.shortcode.contains(':'),
                "shortcode must not contain colons: {}",
                e.shortcode
            );
        }
    }

    #[test]
    fn exact_shortcode_match_ranks_first() {
        let hits = search("smile", 5).unwrap();
        assert!(!hits.is_empty());
        // "smile" is an exact shortcode, so it should be the top hit.
        assert_eq!(hits[0].shortcode, "smile");
    }

    #[test]
    fn shortcode_with_colons_is_stripped() {
        let a = search(":heart:", 3).unwrap();
        let b = search("heart", 3).unwrap();
        assert_eq!(a[0].glyph, b[0].glyph);
        assert_eq!(a[0].shortcode, "heart");
    }

    #[test]
    fn keyword_search_finds_emoji() {
        // "happy" is a keyword, not a name/shortcode.
        let hits = search("happy", 10).unwrap();
        assert!(
            hits.iter().any(|e| e.category == "smileys"),
            "expected a smiley for 'happy'"
        );
    }

    #[test]
    fn category_search_lists_members() {
        let hits = search("flags", MAX_LIMIT).unwrap();
        assert!(hits.len() >= 3, "expected several flags, got {}", hits.len());
        assert!(hits.iter().all(|e| e.category == "flags"));
    }

    #[test]
    fn limit_is_respected_and_clamped() {
        // limit 3 returns at most 3.
        assert!(search("face", 3).unwrap().len() <= 3);
        // limit 0 falls back to DEFAULT_LIMIT.
        assert!(search("face", 0).unwrap().len() <= DEFAULT_LIMIT as usize);
        // huge limit is clamped (never panics).
        let _ = search("a", 9_999).unwrap();
    }

    #[test]
    fn empty_query_is_rejected() {
        assert!(search("", 5).is_err());
        assert!(search("   ", 5).is_err());
        assert!(search("::", 5).is_err());
    }

    #[test]
    fn no_match_returns_friendly_note() {
        let out = run("zzzznotanemoji", 5).unwrap();
        assert!(out.contains("No emoji found"), "got: {out}");
    }

    #[test]
    fn run_formats_one_per_line() {
        let out = run("heart", 1).unwrap();
        assert_eq!(out.lines().count(), 1);
        assert!(out.contains(':'), "should include the shortcode: {out}");
    }

    #[test]
    fn glyphs_returns_only_glyphs() {
        let out = glyphs("smile", 1).unwrap();
        // A single glyph, no colon/letters from a shortcode.
        assert!(!out.contains(':'));
        assert!(!out.is_empty());
    }
}
