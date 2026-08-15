//! autocomplete-trie core — pure, dependency-free prefix trie + ranked autocomplete.
//! Shared by the chat skill block and the browser page.
//!
//! Pipeline:
//!
//! 1. **Parse** the pasted wordlist into `(term, weight)` pairs. A line is either a bare
//!    term, `term<sep>weight`, or `weight<sep>term` (the classic weighted-autocomplete
//!    dataset layout), where `<sep>` is a tab, pipe, or comma. Repeated terms merge and
//!    their weights sum, so an unweighted list repeated N times behaves as a frequency
//!    count.
//! 2. **Build** a character trie over the (optionally case-folded) terms. Shared prefixes
//!    reuse nodes, which is what makes the structure worth reporting.
//! 3. **Query** the trie with the typed prefix. With `max_typos = 0` this is a plain
//!    descent; above 0 it is a Levenshtein DP row carried down the trie, pruned as soon as
//!    the row's minimum exceeds the budget, so a prefix within N edits still matches.
//! 4. **Rank** the matches (exact-prefix hits always before fuzzy ones) by weight,
//!    alphabetically, or shortest-first, and cut to `limit`.
//! 5. **Render** as a ranked text list, structured JSON, or an ASCII drawing of the trie
//!    subtree under the prefix.
//!
//! No `wafer`/`wasm-bindgen`/regex deps: it compiles natively for unit tests, to
//! `wasm32-wasip1` (the chat block target), and to `wasm32-unknown-unknown` (the
//! wasm-pack page target).

use serde::Serialize;
use std::collections::BTreeMap;

/// Maximum number of terms accepted in one wordlist (also the documented page limit).
pub const MAX_TERMS: usize = 20_000;
/// Maximum characters in a single term.
pub const MAX_TERM_LEN: usize = 200;
/// Upper bound on `limit` (also the schema max).
pub const LIMIT_MAX: usize = 100;
/// Upper bound on `max_typos` (also the schema max).
pub const MAX_TYPOS_CAP: usize = 2;
/// Maximum node lines drawn by `output = "trie"` before the drawing is truncated.
pub const TRIE_DRAW_CAP: usize = 2_000;

/// A merged wordlist entry: the first-seen display spelling, its match key, its weight.
#[derive(Debug, Clone)]
struct Term {
    display: String,
    key: Vec<char>,
    weight: f64,
}

#[derive(Debug, Clone, Default)]
struct Node {
    children: BTreeMap<char, usize>,
    term: Option<usize>,
}

/// A prefix trie built over a merged wordlist.
#[derive(Debug, Clone)]
pub struct Trie {
    nodes: Vec<Node>,
    terms: Vec<Term>,
    chars_pasted: usize,
    max_depth: usize,
}

/// One ranked suggestion.
#[derive(Debug, Clone, Serialize)]
pub struct Suggestion {
    pub term: String,
    pub weight: f64,
    /// Edit distance between the typed prefix and the term's own prefix. 0 = exact prefix.
    pub typos: usize,
}

/// Structural facts about the built trie.
#[derive(Debug, Clone, Serialize)]
pub struct Stats {
    pub terms: usize,
    pub nodes: usize,
    pub max_depth: usize,
    pub characters_pasted: usize,
    pub characters_stored: usize,
    pub saved_percent: f64,
}

/// The full structured result (what `output = "json"` serializes).
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub prefix: String,
    pub matches: usize,
    pub returned: usize,
    pub rank: String,
    pub max_typos: usize,
    pub case_sensitive: bool,
    pub suggestions: Vec<Suggestion>,
    pub stats: Stats,
}

/// How suggestions are ordered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rank {
    /// Highest weight first, ties broken alphabetically. The default.
    Weight,
    /// Alphabetical by term.
    Alphabetical,
    /// Fewest characters first, then by weight.
    Shortest,
}

impl Rank {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "weight" => Ok(Rank::Weight),
            "alphabetical" | "alpha" => Ok(Rank::Alphabetical),
            "shortest" => Ok(Rank::Shortest),
            other => Err(format!(
                "unknown rank '{other}': expected weight, alphabetical, or shortest"
            )),
        }
    }
    fn as_str(self) -> &'static str {
        match self {
            Rank::Weight => "weight",
            Rank::Alphabetical => "alphabetical",
            Rank::Shortest => "shortest",
        }
    }
}

/// Output format.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Output {
    Text,
    Json,
    TrieDrawing,
}

impl Output {
    pub fn parse(s: &str) -> Result<Self, String> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "text" => Ok(Output::Text),
            "json" => Ok(Output::Json),
            "trie" => Ok(Output::TrieDrawing),
            other => Err(format!(
                "unknown output '{other}': expected text, json, or trie"
            )),
        }
    }
}

/// Separators that may introduce a weight on a wordlist line, strongest signal first.
const SEPARATORS: [char; 3] = ['\t', '|', ','];

/// Parse one wordlist line into `(term, weight)`.
///
/// `term`, `term<sep>weight` and `weight<sep>term` are all accepted; a line with no usable
/// numeric field is taken verbatim as a term with weight 1.
fn parse_line(line: &str) -> (String, f64) {
    let sep = SEPARATORS.iter().copied().find(|c| line.contains(*c));
    let Some(sep) = sep else {
        return (line.trim().to_string(), 1.0);
    };
    // term<sep>weight — split on the LAST separator so a term may contain the separator.
    if let Some(pos) = line.rfind(sep) {
        let (head, tail) = line.split_at(pos);
        let tail = &tail[sep.len_utf8()..];
        if !head.trim().is_empty() {
            if let Some(w) = parse_weight(tail) {
                return (head.trim().to_string(), w);
            }
        }
    }
    // weight<sep>term — split on the FIRST separator.
    if let Some(pos) = line.find(sep) {
        let (head, tail) = line.split_at(pos);
        let tail = &tail[sep.len_utf8()..];
        if !tail.trim().is_empty() {
            if let Some(w) = parse_weight(head) {
                return (tail.trim().to_string(), w);
            }
        }
    }
    (line.trim().to_string(), 1.0)
}

fn parse_weight(s: &str) -> Option<f64> {
    let t = s.trim();
    if t.is_empty() {
        return None;
    }
    match t.parse::<f64>() {
        Ok(w) if w.is_finite() => Some(w),
        _ => None,
    }
}

impl Trie {
    /// Build a trie from a pasted wordlist. Blank lines and `#` comment lines are skipped.
    pub fn build(wordlist: &str, case_sensitive: bool) -> Result<Self, String> {
        let mut terms: Vec<Term> = Vec::new();
        let mut index: BTreeMap<String, usize> = BTreeMap::new();
        let mut seen_lines = 0usize;

        for (n, raw) in wordlist.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let (display, weight) = parse_line(line);
            if display.is_empty() {
                continue;
            }
            let chars = display.chars().count();
            if chars > MAX_TERM_LEN {
                return Err(format!(
                    "the term on line {} is {chars} characters long; the maximum is {MAX_TERM_LEN}",
                    n + 1
                ));
            }
            seen_lines += 1;
            if seen_lines > MAX_TERMS {
                return Err(format!(
                    "wordlist has more than {MAX_TERMS} terms; trim the list and run again"
                ));
            }
            let key_string = if case_sensitive {
                display.clone()
            } else {
                display.to_lowercase()
            };
            match index.get(&key_string) {
                Some(&i) => terms[i].weight += weight,
                None => {
                    index.insert(key_string.clone(), terms.len());
                    terms.push(Term {
                        display,
                        key: key_string.chars().collect(),
                        weight,
                    });
                }
            }
        }

        if terms.is_empty() {
            return Err(
                "wordlist is empty: paste at least one term, one per line (an optional weight may follow a tab, pipe, or comma)"
                    .to_string(),
            );
        }

        let mut nodes: Vec<Node> = vec![Node::default()];
        let mut chars_pasted = 0usize;
        let mut max_depth = 0usize;
        for (i, term) in terms.iter().enumerate() {
            chars_pasted += term.key.len();
            max_depth = max_depth.max(term.key.len());
            let mut cur = 0usize;
            for &c in &term.key {
                cur = match nodes[cur].children.get(&c) {
                    Some(&next) => next,
                    None => {
                        let next = nodes.len();
                        nodes.push(Node::default());
                        nodes[cur].children.insert(c, next);
                        next
                    }
                };
            }
            nodes[cur].term = Some(i);
        }

        Ok(Trie {
            nodes,
            terms,
            chars_pasted,
            max_depth,
        })
    }

    /// Structural facts about the built trie.
    pub fn stats(&self) -> Stats {
        let stored = self.nodes.len().saturating_sub(1);
        let saved = if self.chars_pasted == 0 {
            0.0
        } else {
            let raw = (1.0 - stored as f64 / self.chars_pasted as f64) * 100.0;
            (raw * 10.0).round() / 10.0
        };
        Stats {
            terms: self.terms.len(),
            nodes: self.nodes.len(),
            max_depth: self.max_depth,
            characters_pasted: self.chars_pasted,
            characters_stored: stored,
            saved_percent: saved,
        }
    }

    /// Walk the exact prefix path, returning the node it ends at (`None` when absent).
    fn exact_node(&self, prefix: &[char]) -> Option<usize> {
        let mut cur = 0usize;
        for c in prefix {
            cur = *self.nodes[cur].children.get(c)?;
        }
        Some(cur)
    }

    fn mark_subtree(&self, node: usize, cost: usize, best: &mut BTreeMap<usize, usize>) {
        if let Some(t) = self.nodes[node].term {
            record(best, t, cost);
        }
        for &child in self.nodes[node].children.values() {
            self.mark_subtree(child, cost, best);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn walk(
        &self,
        node: usize,
        ch: char,
        query: &[char],
        prev: &[usize],
        max_typos: usize,
        inherited: usize,
        best: &mut BTreeMap<usize, usize>,
    ) {
        let qlen = query.len();
        let mut row = Vec::with_capacity(qlen + 1);
        row.push(prev[0] + 1);
        for j in 1..=qlen {
            let sub = prev[j - 1] + usize::from(query[j - 1] != ch);
            row.push((row[j - 1] + 1).min(prev[j] + 1).min(sub));
        }
        let mut inh = inherited;
        if row[qlen] <= max_typos {
            inh = inh.min(row[qlen]);
        }
        // Row minima never decrease as the walk descends, so once the budget is blown no
        // descendant can match — but an ancestor may already have matched the whole subtree.
        let can_descend = row.iter().copied().min().unwrap_or(usize::MAX) <= max_typos;
        if !can_descend {
            if inh <= max_typos {
                self.mark_subtree(node, inh, best);
            }
            return;
        }
        if inh <= max_typos {
            if let Some(t) = self.nodes[node].term {
                record(best, t, inh);
            }
        }
        let kids: Vec<(char, usize)> = self.nodes[node]
            .children
            .iter()
            .map(|(c, &i)| (*c, i))
            .collect();
        for (c, i) in kids {
            self.walk(i, c, query, &row, max_typos, inh, best);
        }
    }

    /// Every term whose own prefix is within `max_typos` edits of `query`, with its cost.
    fn matches(&self, query: &[char], max_typos: usize) -> BTreeMap<usize, usize> {
        let mut best: BTreeMap<usize, usize> = BTreeMap::new();
        let qlen = query.len();
        let row0: Vec<usize> = (0..=qlen).collect();
        let inh = if row0[qlen] <= max_typos {
            row0[qlen]
        } else {
            usize::MAX
        };
        if inh <= max_typos {
            if let Some(t) = self.nodes[0].term {
                record(&mut best, t, inh);
            }
        }
        let kids: Vec<(char, usize)> = self.nodes[0]
            .children
            .iter()
            .map(|(c, &i)| (*c, i))
            .collect();
        for (c, i) in kids {
            self.walk(i, c, query, &row0, max_typos, inh, &mut best);
        }
        best
    }

    /// Rank the matches for `prefix` and return the top `limit` suggestions plus the total.
    pub fn suggest(
        &self,
        prefix: &str,
        limit: usize,
        rank: Rank,
        max_typos: usize,
        case_sensitive: bool,
    ) -> (Vec<Suggestion>, usize) {
        let folded = if case_sensitive {
            prefix.trim().to_string()
        } else {
            prefix.trim().to_lowercase()
        };
        let query: Vec<char> = folded.chars().collect();
        let best = self.matches(&query, max_typos);
        let total = best.len();

        let mut hits: Vec<(usize, usize)> = best.into_iter().collect();
        hits.sort_by(|a, b| {
            let (ia, ca) = *a;
            let (ib, cb) = *b;
            let ta = &self.terms[ia];
            let tb = &self.terms[ib];
            ca.cmp(&cb)
                .then_with(|| match rank {
                    Rank::Weight => tb
                        .weight
                        .partial_cmp(&ta.weight)
                        .unwrap_or(std::cmp::Ordering::Equal),
                    Rank::Alphabetical => std::cmp::Ordering::Equal,
                    Rank::Shortest => ta.key.len().cmp(&tb.key.len()).then_with(|| {
                        tb.weight
                            .partial_cmp(&ta.weight)
                            .unwrap_or(std::cmp::Ordering::Equal)
                    }),
                })
                .then_with(|| ta.key.cmp(&tb.key))
        });

        let suggestions = hits
            .into_iter()
            .take(limit)
            .map(|(i, cost)| Suggestion {
                term: self.terms[i].display.clone(),
                weight: self.terms[i].weight,
                typos: cost,
            })
            .collect();
        (suggestions, total)
    }

    /// Number of terms stored at or below the exact-prefix node.
    fn subtree_terms(&self, node: usize) -> usize {
        let mut n = usize::from(self.nodes[node].term.is_some());
        for &child in self.nodes[node].children.values() {
            n += self.subtree_terms(child);
        }
        n
    }

    fn draw(&self, node: usize, depth: usize, out: &mut Vec<String>, truncated: &mut bool) {
        for (c, &child) in self.nodes[node].children.iter() {
            if out.len() >= TRIE_DRAW_CAP {
                *truncated = true;
                return;
            }
            let mut line = format!("{}{}", "  ".repeat(depth + 1), c);
            if let Some(t) = self.nodes[child].term {
                line.push('*');
                line.push_str(&format!(
                    "  -> {} (weight {})",
                    self.terms[t].display,
                    fmt_weight(self.terms[t].weight)
                ));
            }
            out.push(line);
            self.draw(child, depth + 1, out, truncated);
            if *truncated {
                return;
            }
        }
    }
}

fn record(best: &mut BTreeMap<usize, usize>, term: usize, cost: usize) {
    best.entry(term)
        .and_modify(|c| {
            if cost < *c {
                *c = cost;
            }
        })
        .or_insert(cost);
}

/// Render a weight without a trailing `.0` for whole numbers.
fn fmt_weight(w: f64) -> String {
    if w.is_finite() && w.fract() == 0.0 && w.abs() < 1e15 {
        format!("{}", w as i64)
    } else {
        let s = format!("{w:.6}");
        let s = s.trim_end_matches('0').trim_end_matches('.').to_string();
        s
    }
}

fn stats_line(s: &Stats) -> String {
    format!(
        "Trie: {} terms, {} nodes, depth {}, {} of {} characters stored ({}% saved)",
        s.terms,
        s.nodes,
        s.max_depth,
        s.characters_stored,
        s.characters_pasted,
        fmt_weight(s.saved_percent),
    )
}

fn typo_note(n: usize) -> String {
    match n {
        0 => String::new(),
        1 => "  (1 typo)".to_string(),
        other => format!("  ({other} typos)"),
    }
}

/// Run the tool: build the trie, answer the prefix query, render `output`.
pub fn run(
    wordlist: &str,
    prefix: &str,
    limit: usize,
    rank: &str,
    max_typos: usize,
    case_sensitive: bool,
    output: &str,
) -> Result<String, String> {
    if limit == 0 || limit > LIMIT_MAX {
        return Err(format!("limit must be between 1 and {LIMIT_MAX}"));
    }
    if max_typos > MAX_TYPOS_CAP {
        return Err(format!("max_typos must be between 0 and {MAX_TYPOS_CAP}"));
    }
    let rank_v = Rank::parse(rank)?;
    let output_v = Output::parse(output)?;
    let trie = Trie::build(wordlist, case_sensitive)?;
    let stats = trie.stats();
    let shown_prefix = prefix.trim().to_string();

    if output_v == Output::TrieDrawing {
        let folded: Vec<char> = if case_sensitive {
            shown_prefix.chars().collect()
        } else {
            shown_prefix.to_lowercase().chars().collect()
        };
        let mut lines = Vec::new();
        match trie.exact_node(&folded) {
            None => {
                lines.push(format!(
                    "Trie for \"{shown_prefix}\": no node matches this prefix"
                ));
                lines.push(stats_line(&stats));
            }
            Some(node) => {
                let below = trie.subtree_terms(node);
                if shown_prefix.is_empty() {
                    lines.push(format!("Trie: all {below} terms"));
                } else {
                    lines.push(format!(
                        "Trie for \"{shown_prefix}\": {below} {} below this prefix",
                        if below == 1 { "term" } else { "terms" }
                    ));
                }
                lines.push(stats_line(&stats));
                lines.push(String::new());
                let mut root = if shown_prefix.is_empty() {
                    "(root)".to_string()
                } else {
                    shown_prefix.clone()
                };
                if let Some(t) = trie.nodes[node].term {
                    root.push('*');
                    root.push_str(&format!(
                        "  -> {} (weight {})",
                        trie.terms[t].display,
                        fmt_weight(trie.terms[t].weight)
                    ));
                }
                lines.push(root);
                let mut drawn = Vec::new();
                let mut truncated = false;
                trie.draw(node, 0, &mut drawn, &mut truncated);
                lines.extend(drawn);
                if truncated {
                    lines.push(format!(
                        "  ... (drawing truncated at {TRIE_DRAW_CAP} nodes)"
                    ));
                }
                lines.push(String::new());
                lines.push("* marks a node where a stored term ends.".to_string());
            }
        }
        return Ok(lines.join("\n"));
    }

    let (suggestions, total) =
        trie.suggest(&shown_prefix, limit, rank_v, max_typos, case_sensitive);

    if output_v == Output::Json {
        let report = Report {
            prefix: shown_prefix,
            matches: total,
            returned: suggestions.len(),
            rank: rank_v.as_str().to_string(),
            max_typos,
            case_sensitive,
            suggestions,
            stats,
        };
        return serde_json::to_string_pretty(&report).map_err(|e| e.to_string());
    }

    let mut lines = Vec::new();
    let head = if shown_prefix.is_empty() {
        "Top terms (no prefix)".to_string()
    } else {
        format!("Suggestions for \"{shown_prefix}\"")
    };
    if total == 0 {
        lines.push(format!("{head}: no matching terms"));
        lines.push(stats_line(&stats));
        if max_typos == 0 {
            lines.push(String::new());
            lines.push(
                "Nothing starts with that prefix. Raise \"max typos\" to allow near matches."
                    .to_string(),
            );
        }
        return Ok(lines.join("\n"));
    }

    lines.push(format!(
        "{head}: {} of {} {}",
        suggestions.len(),
        total,
        if total == 1 { "match" } else { "matches" }
    ));
    lines.push(stats_line(&stats));
    lines.push(String::new());

    let idx_w = suggestions.len().to_string().len();
    let term_w = suggestions
        .iter()
        .map(|s| s.term.chars().count())
        .max()
        .unwrap_or(0);
    for (i, s) in suggestions.iter().enumerate() {
        let pad = " ".repeat(term_w - s.term.chars().count());
        lines.push(format!(
            "{:>idx_w$}. {}{}  weight {}{}",
            i + 1,
            s.term,
            pad,
            fmt_weight(s.weight),
            typo_note(s.typos),
        ));
    }
    Ok(lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIST: &str = "apple\t12\napplication\t7\napply\t3\napricot\t5\nbanana\t9\nband\t4";

    #[test]
    fn ranks_prefix_matches_by_weight() {
        let out = run(LIST, "app", 10, "weight", 0, false, "text").unwrap();
        assert!(
            out.starts_with("Suggestions for \"app\": 3 of 3 matches\n"),
            "{out}"
        );
        let body: Vec<&str> = out.lines().skip(3).collect();
        assert_eq!(body[0], "1. apple        weight 12");
        assert_eq!(body[1], "2. application  weight 7");
        assert_eq!(body[2], "3. apply        weight 3");
    }

    #[test]
    fn empty_prefix_returns_top_terms_overall() {
        let out = run(LIST, "", 2, "weight", 0, false, "text").unwrap();
        assert!(
            out.starts_with("Top terms (no prefix): 2 of 6 matches\n"),
            "{out}"
        );
        assert!(out.contains("1. apple   weight 12"), "{out}");
        assert!(out.contains("2. banana  weight 9"), "{out}");
    }

    #[test]
    fn alphabetical_and_shortest_ranks() {
        let alpha = run(LIST, "a", 10, "alphabetical", 0, false, "text").unwrap();
        let first: Vec<&str> = alpha.lines().skip(3).take(4).collect();
        assert!(first[0].contains("apple"), "{alpha}");
        assert!(first[1].contains("application"), "{alpha}");
        assert!(first[2].contains("apply"), "{alpha}");
        assert!(first[3].contains("apricot"), "{alpha}");

        let short = run(LIST, "app", 10, "shortest", 0, false, "text").unwrap();
        let body: Vec<&str> = short.lines().skip(3).collect();
        assert!(body[0].contains("apple"), "{short}");
        assert!(body[1].contains("apply"), "{short}");
        assert!(body[2].contains("application"), "{short}");
    }

    #[test]
    fn repeated_terms_sum_into_a_frequency_weight() {
        let out = run("red\nblue\nred\nred", "", 5, "weight", 0, false, "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["suggestions"][0]["term"], "red");
        assert_eq!(v["suggestions"][0]["weight"], 3.0);
        assert_eq!(v["suggestions"][1]["weight"], 1.0);
    }

    #[test]
    fn accepts_weight_first_and_comma_and_pipe_layouts() {
        let (t, w) = parse_line("12\tapple");
        assert_eq!((t.as_str(), w), ("apple", 12.0));
        let (t, w) = parse_line("apple,12");
        assert_eq!((t.as_str(), w), ("apple", 12.0));
        let (t, w) = parse_line("apple pie | 2.5");
        assert_eq!((t.as_str(), w), ("apple pie", 2.5));
        let (t, w) = parse_line("new york, ny, 8");
        assert_eq!((t.as_str(), w), ("new york, ny", 8.0));
        let (t, w) = parse_line("plain term");
        assert_eq!((t.as_str(), w), ("plain term", 1.0));
    }

    #[test]
    fn typo_tolerance_finds_near_prefixes_after_exact_ones() {
        let out = run(LIST, "aple", 10, "weight", 1, false, "text").unwrap();
        assert!(out.contains("apple"), "{out}");
        assert!(out.contains("(1 typo)"), "{out}");
        // with no budget the same prefix matches nothing
        let strict = run(LIST, "aple", 10, "weight", 0, false, "text").unwrap();
        assert!(strict.contains("no matching terms"), "{strict}");
        assert!(strict.contains("Raise \"max typos\""), "{strict}");
    }

    #[test]
    fn exact_prefix_matches_outrank_fuzzy_ones() {
        let out = run("band\t1\nbend\t100", "ban", 10, "weight", 1, false, "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["suggestions"][0]["term"], "band");
        assert_eq!(v["suggestions"][0]["typos"], 0);
        assert_eq!(v["suggestions"][1]["term"], "bend");
        assert_eq!(v["suggestions"][1]["typos"], 1);
    }

    #[test]
    fn case_insensitive_by_default_and_case_sensitive_on_request() {
        let out = run("Apple\napple\nAPPLE", "app", 10, "weight", 0, false, "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["matches"], 1);
        assert_eq!(v["suggestions"][0]["term"], "Apple");
        assert_eq!(v["suggestions"][0]["weight"], 3.0);

        let cs = run("Apple\napple\nAPPLE", "app", 10, "weight", 0, true, "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&cs).unwrap();
        assert_eq!(v["matches"], 1);
        assert_eq!(v["suggestions"][0]["term"], "apple");
    }

    #[test]
    fn stats_report_shared_prefix_savings() {
        let out = run("car\ncart\ncarton", "", 10, "weight", 0, false, "json").unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["stats"]["terms"], 3);
        assert_eq!(v["stats"]["characters_pasted"], 13);
        assert_eq!(v["stats"]["characters_stored"], 6);
        assert_eq!(v["stats"]["nodes"], 7);
        assert_eq!(v["stats"]["max_depth"], 6);
    }

    #[test]
    fn trie_drawing_marks_terminal_nodes() {
        let out = run("car\ncart", "car", 10, "weight", 0, false, "trie").unwrap();
        assert!(
            out.starts_with("Trie for \"car\": 2 terms below this prefix\n"),
            "{out}"
        );
        assert!(out.contains("car*  -> car (weight 1)"), "{out}");
        assert!(out.contains("  t*  -> cart (weight 1)"), "{out}");
        assert!(
            out.contains("* marks a node where a stored term ends."),
            "{out}"
        );
    }

    #[test]
    fn trie_drawing_reports_a_missing_prefix() {
        let out = run("car\ncart", "zzz", 10, "weight", 0, false, "trie").unwrap();
        assert!(out.contains("no node matches this prefix"), "{out}");
    }

    #[test]
    fn limit_caps_the_list_but_not_the_match_count() {
        let out = run(LIST, "a", 2, "weight", 0, false, "text").unwrap();
        assert!(
            out.starts_with("Suggestions for \"a\": 2 of 4 matches\n"),
            "{out}"
        );
        assert_eq!(out.lines().skip(3).count(), 2);
    }

    #[test]
    fn empty_wordlist_is_an_error() {
        let err = run(
            "   \n# only a comment\n",
            "a",
            10,
            "weight",
            0,
            false,
            "text",
        )
        .unwrap_err();
        assert!(err.contains("wordlist is empty"), "{err}");
    }

    #[test]
    fn bad_enum_and_range_values_are_errors() {
        assert!(run(LIST, "a", 0, "weight", 0, false, "text")
            .unwrap_err()
            .contains("limit must be between 1 and 100"));
        assert!(run(LIST, "a", 10, "weight", 5, false, "text")
            .unwrap_err()
            .contains("max_typos must be between 0 and 2"));
        assert!(run(LIST, "a", 10, "nonsense", 0, false, "text")
            .unwrap_err()
            .contains("unknown rank 'nonsense'"));
        assert!(run(LIST, "a", 10, "weight", 0, false, "xml")
            .unwrap_err()
            .contains("unknown output 'xml'"));
    }

    #[test]
    fn over_long_term_is_rejected_with_the_line_number() {
        let long = "x".repeat(MAX_TERM_LEN + 1);
        let err = run(&format!("ok\n{long}"), "o", 10, "weight", 0, false, "text").unwrap_err();
        assert!(err.contains("line 2"), "{err}");
        assert!(err.contains("201 characters long"), "{err}");
    }

    #[test]
    fn too_many_terms_is_rejected() {
        let mut list = String::new();
        for i in 0..(MAX_TERMS + 1) {
            list.push_str(&format!("term{i}\n"));
        }
        let err = run(&list, "term", 10, "weight", 0, false, "text").unwrap_err();
        assert!(err.contains("more than 20000 terms"), "{err}");
    }

    #[test]
    fn unicode_terms_survive_the_trie() {
        let out = run(
            "café\t2\ncafeteria\t1\ncafé au lait\t5",
            "caf",
            10,
            "weight",
            0,
            false,
            "json",
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["matches"], 3);
        assert_eq!(v["suggestions"][0]["term"], "café au lait");
    }
}
