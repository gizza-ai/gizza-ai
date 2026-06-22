//! tweet-thread-splitter core — split long text into numbered, character-limit
//! safe tweet chunks that never break a word in half. No wafer/wasm-bindgen
//! deps. Shared by the chat skill block and the web page.

/// Default per-tweet character budget (X/Twitter's standard limit).
pub const DEFAULT_LIMIT: usize = 280;
/// Smallest limit we accept — must leave room for at least a short numbering
/// suffix plus one character of content.
pub const MIN_LIMIT: usize = 10;
/// Largest limit we accept (X Premium long-post ceiling).
pub const MAX_LIMIT: usize = 25_000;

/// How tweet length is measured.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Counting {
    /// Count Unicode scalar values (`char`s) — the simple, predictable default.
    Chars,
    /// Count UTF-16 code units — matches what most JS-based length checks and
    /// some clients report (a BMP char = 1, an astral char like an emoji = 2).
    Utf16,
}

impl Counting {
    fn parse(s: &str) -> Result<Self, String> {
        match s {
            "" | "chars" => Ok(Counting::Chars),
            "utf16" => Ok(Counting::Utf16),
            other => Err(format!(
                "invalid count {other:?}: expected \"chars\" or \"utf16\""
            )),
        }
    }

    /// Length of `s` under this counting scheme.
    fn len(self, s: &str) -> usize {
        match self {
            Counting::Chars => s.chars().count(),
            Counting::Utf16 => s.chars().map(|c| c.len_utf16()).sum(),
        }
    }
}

/// Where/how a thread counter is attached to each tweet.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Numbering {
    /// No counter at all.
    None,
    /// Appended ` (i/N)` suffix — the most common Twitter convention. Default.
    Parens,
    /// Appended ` i/N` suffix (no parentheses).
    Slash,
    /// Prepended `i. ` prefix (a numbered-list style, e.g. `1. text`).
    Dotted,
}

impl Numbering {
    fn parse(s: &str) -> Result<Self, String> {
        match s {
            "none" => Ok(Numbering::None),
            "" | "parens" => Ok(Numbering::Parens),
            "slash" => Ok(Numbering::Slash),
            "dotted" => Ok(Numbering::Dotted),
            other => Err(format!(
                "invalid numbering {other:?}: expected \"parens\", \"slash\", \"dotted\", or \"none\""
            )),
        }
    }

    /// True if the counter is prepended (a prefix) rather than appended.
    fn is_prefix(self) -> bool {
        matches!(self, Numbering::Dotted)
    }

    /// Render the counter affix text for tweet `idx` of `total`. Includes the
    /// joining space, so the affix is concatenated directly with the content.
    fn affix(self, idx: usize, total: usize) -> String {
        match self {
            Numbering::None => String::new(),
            Numbering::Parens => format!(" ({idx}/{total})"),
            Numbering::Slash => format!(" {idx}/{total}"),
            Numbering::Dotted => format!("{idx}. "),
        }
    }
}

/// A single produced tweet.
#[derive(Debug)]
pub struct Tweet {
    /// 1-based position in the thread.
    pub index: usize,
    /// The tweet text, including any numbering affix.
    pub text: String,
    /// Length of `text` under the requested counting scheme.
    pub len: usize,
}

/// Result of splitting a body of text into a thread.
#[derive(Debug)]
pub struct Thread {
    pub tweets: Vec<Tweet>,
}

impl Thread {
    /// Number of tweets in the thread.
    pub fn count(&self) -> usize {
        self.tweets.len()
    }

    /// Render the thread as plain text: each tweet separated by a blank line.
    pub fn to_plain(&self) -> String {
        self.tweets
            .iter()
            .map(|t| t.text.as_str())
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

/// Split `text` into character-limit-safe tweets that never break a word.
///
/// - `limit`: max characters per tweet (clamped to `MIN_LIMIT..=MAX_LIMIT`;
///   `0` → `DEFAULT_LIMIT`). The numbering affix counts toward this budget, so
///   each emitted tweet (affix included) is `<= limit`.
/// - `numbering` (`"parens"` | `"slash"` | `"dotted"` | `"none"`, blank →
///   `"parens"`): the thread-counter style. `parens` = ` (i/N)`, `slash` =
///   ` i/N`, `dotted` = `i. ` prefix, `none` = no counter.
/// - `count` (`"chars"` | `"utf16"`, blank → `"chars"`): how length is measured.
/// - `prefer_sentences` (default true): start a new tweet on a sentence
///   boundary (`. ! ?`) when it would otherwise fit on the current one, so a
///   tweet rarely ends mid-thought. Falls back to word packing within a long
///   sentence. Never breaks a word.
///
/// Words longer than the available content budget are hard-split (a single
/// "word" that can't fit even alone, such as a long URL, is chunked across
/// tweets so output is never lost).
///
/// Returns `Err` on an invalid `numbering`/`count`, an out-of-range `limit`, or
/// empty input.
pub fn split(
    text: &str,
    limit: usize,
    numbering: &str,
    count: &str,
    prefer_sentences: bool,
) -> Result<Thread, String> {
    let counting = Counting::parse(count)?;
    let numbering = Numbering::parse(numbering)?;

    let limit = if limit == 0 { DEFAULT_LIMIT } else { limit };
    if limit < MIN_LIMIT {
        return Err(format!("limit {limit} is too small: minimum is {MIN_LIMIT}"));
    }
    if limit > MAX_LIMIT {
        return Err(format!("limit {limit} is too large: maximum is {MAX_LIMIT}"));
    }

    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Err("input text is empty".to_string());
    }

    // Tokenize into words; mark which words end a sentence so the packer can
    // prefer to break after them when prefer_sentences is on.
    let words: Vec<Word> = trimmed
        .split_whitespace()
        .map(|w| Word {
            text: w,
            ends_sentence: ends_sentence(w),
        })
        .collect();

    // Two-pass: the numbering affix width depends on the total tweet count,
    // which we don't know until after splitting. Start by assuming the total is
    // 1, split, and if the count changed (so the affix digit-count may grow),
    // re-split with the new assumption until it is stable (converges quickly).
    let mut assumed_total = 1usize;
    loop {
        let chunks = pack(&words, limit, numbering, assumed_total, counting, prefer_sentences)?;
        if chunks.len() == assumed_total {
            return Ok(build(chunks, numbering, counting));
        }
        assumed_total = chunks.len();
    }
}

struct Word<'a> {
    text: &'a str,
    ends_sentence: bool,
}

/// A word ends a sentence if its last non-closing character is `.`, `!`, or `?`
/// (allowing trailing quotes/brackets like `done."` or `right?)`).
fn ends_sentence(word: &str) -> bool {
    word.trim_end_matches(|c| matches!(c, '"' | '\'' | ')' | ']' | '”' | '’'))
        .ends_with(['.', '!', '?'])
}

/// Greedily pack `words` into chunks whose length (content + the numbering
/// affix sized for `assumed_total`) never exceeds `limit`. Returns the raw
/// content strings (without the affix — that is re-applied in `build` with the
/// real total).
fn pack(
    words: &[Word],
    limit: usize,
    numbering: Numbering,
    assumed_total: usize,
    counting: Counting,
    prefer_sentences: bool,
) -> Result<Vec<String>, String> {
    let mut chunks: Vec<String> = Vec::new();
    let mut cur = String::new();

    // Budget left for content after reserving the worst-case affix for this
    // round. Using `assumed_total` keeps every chunk safely under `limit`.
    let reserve = counting.len(&numbering.affix(assumed_total, assumed_total));
    let content_budget = limit
        .checked_sub(reserve)
        .filter(|b| *b >= 1)
        .ok_or_else(|| format!("limit {limit} is too small to fit the thread numbering"))?;

    for word in words {
        let sep = if cur.is_empty() { 0 } else { 1 };
        let word_len = counting.len(word.text);

        if !cur.is_empty() && counting.len(&cur) + sep + word_len <= content_budget {
            cur.push(' ');
            cur.push_str(word.text);
        } else {
            // The word doesn't fit on the current (non-empty) chunk: flush it.
            if !cur.is_empty() {
                chunks.push(std::mem::take(&mut cur));
            }
            if word_len <= content_budget {
                cur.push_str(word.text);
            } else {
                // The word alone is bigger than a whole tweet — hard-split it on
                // char boundaries so nothing is dropped. All but the last piece
                // are full and flushed; the trailing partial stays in `cur`.
                let pieces = hard_split(word.text, content_budget, counting);
                let last = pieces.len().saturating_sub(1);
                for (i, piece) in pieces.into_iter().enumerate() {
                    if i == last {
                        cur = piece;
                    } else {
                        chunks.push(piece);
                    }
                }
            }
        }

        // Prefer to start a fresh tweet after a sentence-ending word, so a tweet
        // rarely ends mid-thought. Only when it leaves the current chunk
        // non-empty (don't emit an empty tweet for a leading sentence end).
        if prefer_sentences && word.ends_sentence && !cur.is_empty() {
            chunks.push(std::mem::take(&mut cur));
        }
    }

    if !cur.is_empty() {
        chunks.push(cur);
    }
    if chunks.is_empty() {
        return Err("input text is empty".to_string());
    }
    Ok(chunks)
}

/// Break a single over-long word into pieces of at most `budget` units each,
/// splitting only on `char` boundaries (never inside a multi-byte char).
fn hard_split(word: &str, budget: usize, counting: Counting) -> Vec<String> {
    let mut pieces = Vec::new();
    let mut cur = String::new();
    let mut cur_len = 0usize;
    for ch in word.chars() {
        let cl = match counting {
            Counting::Chars => 1,
            Counting::Utf16 => ch.len_utf16(),
        };
        if cur_len + cl > budget && !cur.is_empty() {
            pieces.push(std::mem::take(&mut cur));
            cur_len = 0;
        }
        cur.push(ch);
        cur_len += cl;
    }
    if !cur.is_empty() {
        pieces.push(cur);
    }
    pieces
}

/// Attach the numbering affix (sized for the real total) to each content chunk.
fn build(chunks: Vec<String>, numbering: Numbering, counting: Counting) -> Thread {
    let total = chunks.len();
    let tweets = chunks
        .into_iter()
        .enumerate()
        .map(|(i, content)| {
            let idx = i + 1;
            let affix = numbering.affix(idx, total);
            let text = if numbering.is_prefix() {
                format!("{affix}{content}")
            } else {
                format!("{content}{affix}")
            };
            let len = counting.len(&text);
            Tweet { index: idx, text, len }
        })
        .collect();
    Thread { tweets }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper: default settings (parens numbering, chars, word-only packing).
    fn split_words(text: &str, limit: usize) -> Thread {
        split(text, limit, "parens", "chars", false).unwrap()
    }

    #[test]
    fn short_text_is_one_tweet() {
        let t = split_words("hello world", 280);
        assert_eq!(t.count(), 1);
        assert_eq!(t.tweets[0].text, "hello world (1/1)");
    }

    #[test]
    fn splits_into_multiple_numbered_tweets() {
        let body = "aaaaa bbbbb ccccc ddddd eeeee fffff ggggg hhhhh iiiii jjjjj";
        let t = split_words(body, 20);
        assert!(t.count() > 1, "expected multiple tweets, got {}", t.count());
        for tw in &t.tweets {
            assert!(tw.len <= 20, "tweet {} too long: {:?}", tw.index, tw.text);
        }
        let total = t.count();
        assert!(t.tweets[0].text.ends_with(&format!("(1/{total})")));
        assert!(t.tweets[total - 1].text.ends_with(&format!("({total}/{total})")));
    }

    #[test]
    fn never_breaks_a_word_mid_word() {
        let body = "alpha bravo charlie delta echo foxtrot golf hotel india";
        let t = split_words(body, 30);
        let words: Vec<&str> = body.split_whitespace().collect();
        let mut reconstructed: Vec<String> = Vec::new();
        for tw in &t.tweets {
            let content = tw.text.rsplit_once(" (").map(|(c, _)| c).unwrap_or(&tw.text);
            for w in content.split_whitespace() {
                reconstructed.push(w.to_string());
            }
        }
        assert_eq!(reconstructed, words);
    }

    #[test]
    fn no_numbering_when_none() {
        let t = split("just a short note", 280, "none", "chars", false).unwrap();
        assert_eq!(t.count(), 1);
        assert_eq!(t.tweets[0].text, "just a short note");
    }

    #[test]
    fn slash_numbering_omits_parens() {
        let t = split("hello world", 280, "slash", "chars", false).unwrap();
        assert_eq!(t.tweets[0].text, "hello world 1/1");
    }

    #[test]
    fn dotted_numbering_prepends_a_number() {
        let body = "aaaaa bbbbb ccccc ddddd eeeee fffff";
        let t = split(body, 20, "dotted", "chars", false).unwrap();
        assert!(t.tweets[0].text.starts_with("1. "));
        let last = t.count();
        assert!(t.tweets[last - 1].text.starts_with(&format!("{last}. ")));
        for tw in &t.tweets {
            assert!(tw.len <= 20, "tweet {} too long: {:?}", tw.index, tw.text);
        }
    }

    #[test]
    fn rejects_invalid_numbering() {
        let err = split("hi", 280, "fancy", "chars", false).unwrap_err();
        assert!(err.contains("invalid numbering"), "got: {err}");
    }

    #[test]
    fn prefer_sentences_breaks_after_a_sentence() {
        // Two short sentences fit one tweet, but sentence-preference puts each in
        // its own tweet.
        let t = split("First sentence. Second sentence.", 280, "none", "chars", true).unwrap();
        assert_eq!(t.count(), 2);
        assert_eq!(t.tweets[0].text, "First sentence.");
        assert_eq!(t.tweets[1].text, "Second sentence.");
    }

    #[test]
    fn prefer_sentences_falls_back_within_a_long_sentence() {
        // A single sentence longer than the limit is still word-packed (never
        // broken mid-word) across tweets.
        let body = "aaaaa bbbbb ccccc ddddd eeeee fffff ggggg hhhhh.";
        let t = split(body, 20, "none", "chars", true).unwrap();
        assert!(t.count() > 1);
        for tw in &t.tweets {
            assert!(tw.len <= 20);
        }
    }

    #[test]
    fn hard_splits_an_overlong_word() {
        let body = "x".repeat(30);
        let t = split(&body, 12, "none", "chars", false).unwrap();
        assert!(t.count() >= 3);
        for tw in &t.tweets {
            assert!(tw.len <= 12);
        }
        let joined: String = t.tweets.iter().map(|tw| tw.text.as_str()).collect();
        assert_eq!(joined, body);
    }

    #[test]
    fn utf16_counts_astral_chars_as_two() {
        let one = split("😀", 280, "none", "chars", false).unwrap();
        assert_eq!(one.tweets[0].len, 1);
        let two = split("😀", 280, "none", "utf16", false).unwrap();
        assert_eq!(two.tweets[0].len, 2);
    }

    #[test]
    fn collapses_whitespace_and_newlines() {
        let t = split("foo\n\n   bar\tbaz", 280, "none", "chars", false).unwrap();
        assert_eq!(t.tweets[0].text, "foo bar baz");
    }

    #[test]
    fn limit_zero_defaults_to_280() {
        let body = "word ".repeat(100);
        let t = split(&body, 0, "none", "chars", false).unwrap();
        for tw in &t.tweets {
            assert!(tw.len <= DEFAULT_LIMIT);
        }
    }

    #[test]
    fn rejects_empty_input() {
        let err = split("   \n  ", 280, "parens", "chars", false).unwrap_err();
        assert!(err.contains("empty"), "got: {err}");
    }

    #[test]
    fn rejects_invalid_count() {
        let err = split("hi", 280, "parens", "bytes", false).unwrap_err();
        assert!(err.contains("invalid count"), "got: {err}");
    }

    #[test]
    fn rejects_too_small_limit() {
        let err = split("hi", 3, "parens", "chars", false).unwrap_err();
        assert!(err.contains("too small"), "got: {err}");
    }

    #[test]
    fn rejects_too_large_limit() {
        let err = split("hi", 30_000, "parens", "chars", false).unwrap_err();
        assert!(err.contains("too large"), "got: {err}");
    }

    #[test]
    fn to_plain_separates_with_blank_lines() {
        let t = split_words("aaaaa bbbbb ccccc", 14);
        let plain = t.to_plain();
        assert!(plain.contains("\n\n"));
        assert_eq!(plain.split("\n\n").count(), t.count());
    }
}
