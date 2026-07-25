//! gizza-ai/chunk-text core — split a long document into overlapping chunks by
//! token, character, or word count for RAG / embedding pipelines.
//!
//! Pure-Rust (only `serde_json` for output). No wafer/wasm-bindgen deps — shared
//! by the chat skill block and the web page.
//!
//! Each chunk is a **contiguous substring of the original text** (LangChain-style
//! `RecursiveCharacterTextSplitter` semantics), so every record's `start`/`end`
//! character offsets point back into the source and downstream embedding code can
//! cite exact spans.
//!
//! Sizing is measured in the chosen `unit`:
//! - `tokens` (default) — an *estimate* of `chars / chars_per_token`; there is no
//!   real BPE tokenizer in the browser wasm (the `token-counter` tool does real
//!   BPE). Rounded to the nearest whole token.
//! - `characters` — Unicode scalar values.
//! - `words` — whitespace-separated words.
//!
//! Chunks never split a `boundary` unit when it fits: `word` (default) keeps
//! words whole, `sentence` cuts on sentence ends, `paragraph` on blank lines,
//! `character` is a hard cut. When a single boundary unit is larger than the
//! chunk budget the splitter recursively falls back to the next finer boundary so
//! it always makes progress.

use serde_json::json;

/// Chunk size must be at least this many units.
pub const MIN_CHUNK_SIZE: usize = 1;
/// Guard against absurd inputs.
pub const MAX_CHUNK_SIZE: usize = 1_000_000;
/// Cap on emitted chunks — a tiny size with heavy overlap could otherwise
/// produce one chunk per character.
pub const MAX_CHUNKS: usize = 100_000;
/// Bounds on the chars-per-token estimate.
pub const MIN_CHARS_PER_TOKEN: f64 = 1.0;
pub const MAX_CHARS_PER_TOKEN: f64 = 100.0;

/// How `chunk_size` / `overlap` are measured.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Unit {
    Tokens,
    Characters,
    Words,
}

impl Unit {
    pub fn parse(s: &str) -> Result<Unit, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "tokens" | "token" | "tok" | "t" => Ok(Unit::Tokens),
            "characters" | "character" | "chars" | "char" | "c" => Ok(Unit::Characters),
            "words" | "word" | "w" => Ok(Unit::Words),
            other => Err(format!(
                "unit must be \"tokens\", \"characters\", or \"words\" (got {other:?})"
            )),
        }
    }
}

/// The coarsest granularity a chunk boundary may fall on. Ordered finest → coarsest
/// so `as usize` gives the fallback ladder.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Boundary {
    Character = 0,
    Word = 1,
    Sentence = 2,
    Paragraph = 3,
}

impl Boundary {
    pub fn parse(s: &str) -> Result<Boundary, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "word" | "words" => Ok(Boundary::Word),
            "character" | "characters" | "char" | "chars" | "hard" => Ok(Boundary::Character),
            "sentence" | "sentences" => Ok(Boundary::Sentence),
            "paragraph" | "paragraphs" | "para" => Ok(Boundary::Paragraph),
            other => Err(format!(
                "boundary must be \"word\", \"character\", \"sentence\", or \"paragraph\" (got {other:?})"
            )),
        }
    }
}

/// Output serialization.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Format {
    Json,
    Jsonl,
    Plain,
}

impl Format {
    pub fn parse(s: &str) -> Result<Format, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "json" => Ok(Format::Json),
            "jsonl" | "ndjson" => Ok(Format::Jsonl),
            "plain" | "text" => Ok(Format::Plain),
            other => Err(format!(
                "format must be \"json\", \"jsonl\", or \"plain\" (got {other:?})"
            )),
        }
    }
}

/// Separator between chunks in the `plain` output.
pub const PLAIN_SEPARATOR: &str = "\n\n---\n\n";

/// One emitted chunk.
struct Chunk {
    /// Char offset into the source where this chunk's text begins.
    start: usize,
    /// Char offset (exclusive) where it ends.
    end: usize,
    text: String,
}

/// Split `text` into overlapping chunks. See the module docs for semantics.
///
/// `chunk_size` and `overlap` are measured in `unit`. `overlap` must be strictly
/// less than `chunk_size`. `chars_per_token` is the estimate used when
/// `unit = "tokens"`.
pub fn chunk(
    text: &str,
    chunk_size: usize,
    overlap: usize,
    unit: &str,
    boundary: &str,
    chars_per_token: f64,
    format: &str,
    trim: bool,
) -> Result<String, String> {
    let unit = Unit::parse(unit)?;
    let boundary = Boundary::parse(boundary)?;
    let format = Format::parse(format)?;

    if chunk_size < MIN_CHUNK_SIZE || chunk_size > MAX_CHUNK_SIZE {
        return Err(format!(
            "chunk_size must be between {MIN_CHUNK_SIZE} and {MAX_CHUNK_SIZE}"
        ));
    }
    if overlap >= chunk_size {
        return Err(format!(
            "overlap ({overlap}) must be less than chunk_size ({chunk_size})"
        ));
    }
    if !chars_per_token.is_finite()
        || chars_per_token < MIN_CHARS_PER_TOKEN
        || chars_per_token > MAX_CHARS_PER_TOKEN
    {
        return Err(format!(
            "chars_per_token must be between {MIN_CHARS_PER_TOKEN} and {MAX_CHARS_PER_TOKEN}"
        ));
    }

    let chars: Vec<char> = text.chars().collect();
    let n = chars.len();

    let chunks = if n == 0 {
        Vec::new()
    } else {
        split(&chars, chunk_size, overlap, unit, boundary, chars_per_token, trim)?
    };

    Ok(render(&chunks, format, chars_per_token))
}

/// Measure the char span `[a, b)` in `unit`.
fn measure(prefix_words: &[usize], a: usize, b: usize, unit: Unit, cpt: f64) -> usize {
    match unit {
        Unit::Characters => b - a,
        Unit::Words => prefix_words[b] - prefix_words[a],
        Unit::Tokens => (((b - a) as f64) / cpt).round() as usize,
    }
}

fn split(
    chars: &[char],
    chunk_size: usize,
    overlap: usize,
    unit: Unit,
    boundary: Boundary,
    cpt: f64,
    trim: bool,
) -> Result<Vec<Chunk>, String> {
    let n = chars.len();

    // prefix_words[i] = number of word-starts in chars[0..i].
    let mut prefix_words = vec![0usize; n + 1];
    for i in 0..n {
        let is_start = !chars[i].is_whitespace() && (i == 0 || chars[i - 1].is_whitespace());
        prefix_words[i + 1] = prefix_words[i] + usize::from(is_start);
    }

    // Allowed cut positions per boundary level (each sorted, includes 0 and n).
    let positions = boundary_positions(chars);

    let m = |a: usize, b: usize| measure(&prefix_words, a, b, unit, cpt);

    let mut out: Vec<Chunk> = Vec::new();
    let mut start = 0usize;

    loop {
        // Largest `e` in (start, n] whose measured size fits the budget.
        let maxend = max_fit(start, n, chunk_size, &m);
        // Snap the chunk end to the coarsest boundary that fits (recursive fallback).
        let end = pick_end(start, maxend, boundary, &positions);

        push_chunk(chars, start, end, trim, &mut out);
        if out.len() > MAX_CHUNKS {
            return Err(format!(
                "would produce more than {MAX_CHUNKS} chunks — increase chunk_size or reduce overlap"
            ));
        }

        if end >= n {
            break;
        }

        let next = next_start(start, end, overlap, boundary, &positions, &m);
        // `next_start` guarantees strictly-forward progress.
        start = next;
    }

    Ok(out)
}

/// Largest `e` in `(start, n]` with `measure(start, e) <= budget` (measure is
/// monotonic nondecreasing in `e`). Always at least `start + 1`.
fn max_fit(start: usize, n: usize, budget: usize, m: &impl Fn(usize, usize) -> usize) -> usize {
    if m(start, n) <= budget {
        return n;
    }
    // Binary search: invariant lo fits, hi does not.
    let mut lo = start + 1;
    let mut hi = n;
    while lo < hi {
        let mid = lo + (hi - lo + 1) / 2;
        if m(start, mid) <= budget {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    lo.max(start + 1)
}

/// Coarsest boundary position in `(start, maxend]`, falling back to finer levels.
/// Character level always yields `maxend`.
fn pick_end(start: usize, maxend: usize, boundary: Boundary, positions: &[Vec<usize>]) -> usize {
    for lvl in (0..=boundary as usize).rev() {
        let v = &positions[lvl];
        let hi = v.partition_point(|&p| p <= maxend);
        if hi > 0 && v[hi - 1] > start {
            return v[hi - 1];
        }
    }
    maxend
}

/// Where the next chunk begins: back up from `end` by up to `overlap` units,
/// snapped up to a boundary at the requested level so a unit is never split.
/// Always strictly greater than `start`.
fn next_start(
    start: usize,
    end: usize,
    overlap: usize,
    boundary: Boundary,
    positions: &[Vec<usize>],
    m: &impl Fn(usize, usize) -> usize,
) -> usize {
    if overlap == 0 {
        return end;
    }
    // Smallest char q in [start, end] with measure(q, end) <= overlap. As q grows
    // the measured tail shrinks, so this is monotonic.
    let mut lo = start;
    let mut hi = end;
    while lo < hi {
        let mid = lo + (hi - lo) / 2;
        if m(mid, end) <= overlap {
            hi = mid;
        } else {
            lo = mid + 1;
        }
    }
    let q0 = lo;
    // Snap up to the nearest boundary at the requested level that is < end.
    let v = &positions[boundary as usize];
    let idx = v.partition_point(|&p| p < q0);
    if idx < v.len() && v[idx] < end && v[idx] > start {
        return v[idx];
    }
    // No usable boundary in the overlap window — fall back to no overlap for this
    // step (still strictly forward, `end > start`).
    end
}

fn push_chunk(chars: &[char], start: usize, end: usize, trim: bool, out: &mut Vec<Chunk>) {
    let (mut s, mut e) = (start, end);
    if trim {
        while s < e && chars[s].is_whitespace() {
            s += 1;
        }
        while e > s && chars[e - 1].is_whitespace() {
            e -= 1;
        }
        if s == e {
            return; // whitespace-only chunk — drop it
        }
    }
    out.push(Chunk {
        start: s,
        end: e,
        text: chars[s..e].iter().collect(),
    });
}

/// Boundary cut positions for each `Boundary` level, indexed by `level as usize`.
/// Every vector is sorted ascending and contains `0` and `n`.
fn boundary_positions(chars: &[char]) -> Vec<Vec<usize>> {
    let n = chars.len();

    // Character: every index.
    let character: Vec<usize> = (0..=n).collect();

    // Word: the start of every word (a non-space preceded by a space or bos).
    let mut word = vec![0usize];
    for i in 1..n {
        if !chars[i].is_whitespace() && chars[i - 1].is_whitespace() {
            word.push(i);
        }
    }
    if *word.last().unwrap() != n {
        word.push(n);
    }

    // Sentence: the start of the next sentence — the first non-space char whose
    // preceding non-space char is a terminator (. ! ?), optionally through
    // closing quotes/brackets.
    let mut sentence = vec![0usize];
    for i in 1..n {
        if chars[i].is_whitespace() || (i > 0 && !chars[i - 1].is_whitespace()) {
            continue;
        }
        // chars[i] starts a run of non-space; find the last non-space before the
        // whitespace gap.
        let mut j = i - 1;
        while j > 0 && chars[j].is_whitespace() {
            j -= 1;
        }
        // Skip trailing closing punctuation to reach the sentence terminator.
        let mut k = j;
        while k > 0 && matches!(chars[k], '"' | '\'' | ')' | ']' | '}' | '”' | '’') {
            k -= 1;
        }
        if matches!(chars[k], '.' | '!' | '?' | '…') {
            sentence.push(i);
        }
    }
    if *sentence.last().unwrap() != n {
        sentence.push(n);
    }

    // Paragraph: the start of the text after a blank line (a whitespace run with
    // two or more newlines).
    let mut paragraph = vec![0usize];
    let mut i = 0;
    while i < n {
        if chars[i] == '\n' {
            // Scan the whitespace run starting here; count newlines.
            let mut j = i;
            let mut newlines = 0;
            while j < n && chars[j].is_whitespace() {
                if chars[j] == '\n' {
                    newlines += 1;
                }
                j += 1;
            }
            if newlines >= 2 && j < n {
                paragraph.push(j);
            }
            i = j;
        } else {
            i += 1;
        }
    }
    if *paragraph.last().unwrap() != n {
        paragraph.push(n);
    }

    // Indexed by Boundary::Character/Word/Sentence/Paragraph as usize (0..=3).
    vec![character, word, sentence, paragraph]
}

fn token_estimate(chars: usize, cpt: f64) -> usize {
    ((chars as f64) / cpt).round() as usize
}

fn render(chunks: &[Chunk], format: Format, cpt: f64) -> String {
    match format {
        Format::Plain => chunks
            .iter()
            .map(|c| c.text.as_str())
            .collect::<Vec<_>>()
            .join(PLAIN_SEPARATOR),
        Format::Json => {
            let arr: Vec<_> = chunks
                .iter()
                .enumerate()
                .map(|(id, c)| record(id, c, cpt))
                .collect();
            serde_json::to_string_pretty(&arr).unwrap_or_else(|_| "[]".into())
        }
        Format::Jsonl => chunks
            .iter()
            .enumerate()
            .map(|(id, c)| record(id, c, cpt).to_string())
            .collect::<Vec<_>>()
            .join("\n"),
    }
}

fn record(id: usize, c: &Chunk, cpt: f64) -> serde_json::Value {
    let n_chars = c.end - c.start;
    json!({
        "id": id,
        "text": c.text,
        "chars": n_chars,
        "tokens": token_estimate(n_chars, cpt),
        "start": c.start,
        "end": c.end,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(json: &str) -> Vec<serde_json::Value> {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn character_unit_hard_cut_with_overlap() {
        // 10 chars, size 4, overlap 2, hard character boundary → windows of 4
        // stepping by 2: [0,4) [2,6) [4,8) [6,10).
        let out = chunk("abcdefghij", 4, 2, "characters", "character", 4.0, "json", false).unwrap();
        let recs = parse(&out);
        let texts: Vec<&str> = recs.iter().map(|r| r["text"].as_str().unwrap()).collect();
        assert_eq!(texts, vec!["abcd", "cdef", "efgh", "ghij"]);
        assert_eq!(recs[0]["start"], json!(0));
        assert_eq!(recs[0]["end"], json!(4));
        assert_eq!(recs[1]["start"], json!(2));
    }

    #[test]
    fn word_boundary_never_splits_a_word() {
        // Every chunk must be whole words only (no partial word at either edge).
        let text = "the quick brown fox jumps over the lazy dog again";
        let out = chunk(text, 12, 3, "characters", "word", 4.0, "json", true).unwrap();
        let recs = parse(&out);
        assert!(recs.len() > 1);
        for r in &recs {
            let t = r["text"].as_str().unwrap();
            assert!(!t.starts_with(' ') && !t.ends_with(' '), "trimmed: {t:?}");
            for w in t.split_whitespace() {
                assert!(text.contains(w));
            }
            // No chunk exceeds the 12-char budget.
            assert!(r["chars"].as_u64().unwrap() <= 12, "chunk too big: {t:?}");
        }
    }

    #[test]
    fn covers_entire_text_no_overlap() {
        let text = "one two three four five six seven eight";
        let out = chunk(text, 10, 0, "characters", "word", 4.0, "json", false).unwrap();
        let recs = parse(&out);
        // With no overlap the concatenation of spans reconstructs the source.
        let joined: String = recs.iter().map(|r| r["text"].as_str().unwrap()).collect();
        assert_eq!(joined, text);
    }

    #[test]
    fn paragraph_boundary_prefers_blank_lines() {
        let text = "First para line one.\nStill first.\n\nSecond paragraph here.\n\nThird.";
        let out = chunk(text, 40, 0, "characters", "paragraph", 4.0, "json", true).unwrap();
        let recs = parse(&out);
        // The first chunk should end at the paragraph break, not mid-sentence.
        assert_eq!(recs[0]["text"], json!("First para line one.\nStill first."));
    }

    #[test]
    fn token_unit_uses_chars_per_token_estimate() {
        // 40 chars, 4 chars/token → ~10 tokens; size 5 tokens ≈ 20 chars.
        let text = "abcdefghijklmnopqrstuvwxyzabcdefghijklmn"; // 40 chars, no spaces
        let out = chunk(text, 5, 0, "tokens", "character", 4.0, "json", false).unwrap();
        let recs = parse(&out);
        assert_eq!(recs.len(), 2);
        assert_eq!(recs[0]["tokens"], json!(5));
        // round(21/4) = 5, round(22/4) = 6, so the budget fits 21 chars.
        assert_eq!(recs[0]["chars"], json!(21));
    }

    #[test]
    fn jsonl_one_object_per_line() {
        let out = chunk("abcdef", 3, 0, "characters", "character", 4.0, "jsonl", false).unwrap();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 2);
        let first: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(first["text"], json!("abc"));
        assert_eq!(first["id"], json!(0));
    }

    #[test]
    fn plain_joins_with_separator() {
        let out = chunk("abcdef", 3, 0, "characters", "character", 4.0, "plain", false).unwrap();
        assert_eq!(out, format!("abc{PLAIN_SEPARATOR}def"));
    }

    #[test]
    fn empty_text_yields_empty_array() {
        assert_eq!(chunk("", 100, 10, "tokens", "word", 4.0, "json", false).unwrap(), "[]");
        assert_eq!(chunk("", 100, 10, "tokens", "word", 4.0, "jsonl", false).unwrap(), "");
    }

    #[test]
    fn long_word_falls_back_to_hard_cut() {
        // A single 20-char "word" larger than the 8-char budget must still be cut
        // (recursive fallback to character level) rather than looping forever.
        let out = chunk("abcdefghijklmnopqrst", 8, 0, "characters", "word", 4.0, "json", false)
            .unwrap();
        let recs = parse(&out);
        assert!(recs.len() >= 3);
        let joined: String = recs.iter().map(|r| r["text"].as_str().unwrap()).collect();
        assert_eq!(joined, "abcdefghijklmnopqrst");
    }

    #[test]
    fn rejects_overlap_ge_chunk_size() {
        let err = chunk("abc", 4, 4, "characters", "word", 4.0, "json", false).unwrap_err();
        assert!(err.contains("overlap"));
    }

    #[test]
    fn rejects_bad_unit() {
        assert!(chunk("abc", 4, 0, "banana", "word", 4.0, "json", false).is_err());
    }

    #[test]
    fn rejects_zero_chunk_size() {
        assert!(chunk("abc", 0, 0, "characters", "word", 4.0, "json", false).is_err());
    }
}
