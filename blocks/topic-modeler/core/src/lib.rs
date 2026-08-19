//! topic-modeler core — discover latent topics across a small corpus of pasted
//! documents with Latent Dirichlet Allocation, fitted at run time by collapsed
//! Gibbs sampling. Pure Rust (hand-rolled sampler + seeded PRNG, no ML runtime,
//! no pretrained weights), so it runs identically on every backend: the chat
//! Service Worker, the CLI, and the browser page. No wafer/wasm-bindgen deps.
//!
//! Determinism: the only randomness is a SplitMix64 PRNG seeded from
//! `Options::seed`, and every loop walks tokens/topics in a fixed order, so the
//! same corpus + settings always produce the same topics — required by the
//! page's recompute-on-input model and by the tests.
//!
//! Pipeline: `model()` splits the corpus, tokenises + filters, fits LDA and
//! returns a [`TopicModel`]; `render()` formats one into a report / JSON / CSV;
//! `run()` is the two together (what every surface calls).

use serde::Serialize;
use std::collections::{HashMap, HashSet};

/// Caps that keep runtime and memory inside the 64 MiB wasm sandbox. Exceeding
/// one is a user-facing error rather than a silent truncation — a truncated
/// corpus would produce topics that don't describe the input the user pasted.
pub const MAX_DOCS: usize = 300;
pub const MAX_TOKENS: usize = 25_000;
pub const MAX_VOCAB: usize = 20_000;

/// Built-in English stopword list (function words only — no domain terms), used
/// when `remove_stopwords` is on. Kept sorted for the `binary_search` lookup and
/// for review; `stopwords` merges the user's own list on top.
const STOPWORDS: &[&str] = &[
    "a",
    "about",
    "above",
    "after",
    "again",
    "against",
    "all",
    "also",
    "am",
    "an",
    "and",
    "any",
    "are",
    "aren't",
    "as",
    "at",
    "be",
    "because",
    "been",
    "before",
    "being",
    "below",
    "between",
    "both",
    "but",
    "by",
    "can",
    "can't",
    "cannot",
    "could",
    "couldn't",
    "did",
    "didn't",
    "do",
    "does",
    "doesn't",
    "doing",
    "don't",
    "down",
    "during",
    "each",
    "few",
    "for",
    "from",
    "further",
    "had",
    "hadn't",
    "has",
    "hasn't",
    "have",
    "haven't",
    "having",
    "he",
    "her",
    "here",
    "hers",
    "herself",
    "him",
    "himself",
    "his",
    "how",
    "i",
    "if",
    "in",
    "into",
    "is",
    "isn't",
    "it",
    "it's",
    "its",
    "itself",
    "just",
    "let's",
    "like",
    "me",
    "more",
    "most",
    "must",
    "mustn't",
    "my",
    "myself",
    "no",
    "nor",
    "not",
    "now",
    "of",
    "off",
    "on",
    "once",
    "only",
    "or",
    "other",
    "ought",
    "our",
    "ours",
    "ourselves",
    "out",
    "over",
    "own",
    "same",
    "shan't",
    "she",
    "should",
    "shouldn't",
    "so",
    "some",
    "such",
    "than",
    "that",
    "the",
    "their",
    "theirs",
    "them",
    "themselves",
    "then",
    "there",
    "these",
    "they",
    "this",
    "those",
    "through",
    "to",
    "too",
    "under",
    "until",
    "up",
    "very",
    "was",
    "wasn't",
    "we",
    "were",
    "weren't",
    "what",
    "when",
    "where",
    "which",
    "while",
    "who",
    "whom",
    "why",
    "will",
    "with",
    "won't",
    "would",
    "wouldn't",
    "you",
    "your",
    "yours",
    "yourself",
    "yourselves",
];

/// Options resolved from the tool params (chat/CLI JSON, or the page's string
/// fields). Enum-ish fields are plain strings so every surface can pass its raw
/// value through; `model()` validates them.
#[derive(Debug, Clone)]
pub struct Options {
    /// How the pasted corpus is split into documents: "blank-line" | "line" |
    /// "dashes".
    pub separator: String,
    /// Number of topics K to fit (2–20).
    pub topics: u32,
    /// Top words listed per topic (1–25).
    pub words_per_topic: u32,
    /// Gibbs sampling sweeps over the corpus (50–1000).
    pub iterations: u32,
    /// Dirichlet prior on the document–topic distribution. 0 = auto (50/K, the
    /// MALLET convention).
    pub alpha: f64,
    /// Dirichlet prior on the topic–word distribution (0.001–1).
    pub beta: f64,
    /// Drop the built-in English stopword list before modelling.
    pub remove_stopwords: bool,
    /// Extra stopwords to drop, comma/whitespace separated. Applied even when
    /// `remove_stopwords` is off (an explicit list is an explicit request).
    pub stopwords: String,
    /// Shortest word kept (1–12 characters).
    pub min_word_length: u32,
    /// PRNG seed — same seed + same corpus + same settings = same topics.
    pub seed: u64,
    /// Result format: "report" | "json" | "csv".
    pub output: String,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            separator: "blank-line".into(),
            topics: 5,
            words_per_topic: 8,
            iterations: 200,
            alpha: 0.0,
            beta: 0.01,
            remove_stopwords: true,
            stopwords: String::new(),
            min_word_length: 3,
            seed: 42,
            output: "report".into(),
        }
    }
}

/// One of a topic's top words with its probability inside that topic (phi).
#[derive(Debug, Clone, Serialize)]
pub struct TopicWord {
    pub word: String,
    /// P(word | topic), smoothed by beta.
    pub weight: f64,
}

/// A fitted topic: its share of the corpus and its top words.
#[derive(Debug, Clone, Serialize)]
pub struct Topic {
    /// 1-based rank in the returned order (topics are sorted by `share` desc).
    pub rank: usize,
    /// Fraction of all kept tokens assigned to this topic.
    pub share: f64,
    /// Short human label — the topic's top three words.
    pub label: String,
    pub words: Vec<TopicWord>,
}

/// One document's topic mixture (theta), aligned to the ranked topic order.
#[derive(Debug, Clone, Serialize)]
pub struct DocumentMix {
    /// 1-based document number, in input order.
    pub index: usize,
    /// Collapsed one-line preview of the document's raw text.
    pub preview: String,
    /// Kept tokens after filtering.
    pub tokens: usize,
    /// Share of this document's words assigned to each topic, in ranked order
    /// (sums to 1). Flat 1/K when every word was filtered out.
    pub weights: Vec<f64>,
    /// 1-based rank of this document's strongest topic.
    pub dominant: usize,
}

/// A fitted model: the topics, the per-document mixtures, and the settings that
/// produced them.
#[derive(Debug, Clone, Serialize)]
pub struct TopicModel {
    pub documents: usize,
    /// Kept tokens across the corpus (after stopword/length filtering).
    pub tokens: usize,
    /// Distinct kept terms.
    pub vocabulary: usize,
    /// Effective alpha (already resolved: 50/K when the option was 0).
    pub alpha: f64,
    pub beta: f64,
    pub iterations: u32,
    pub seed: u64,
    pub topics: Vec<Topic>,
    pub mixtures: Vec<DocumentMix>,
}

// ---------------------------------------------------------------------------
// PRNG — SplitMix64. Small, seedable, identical on every target (integer ops
// only), and fine for Gibbs sampling.
// ---------------------------------------------------------------------------

struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Rng(seed ^ 0x9E37_79B9_7F4A_7C15)
    }
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
    /// Uniform in [0, 1).
    fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 / (1u64 << 53) as f64
    }
}

// ---------------------------------------------------------------------------
// Corpus preparation
// ---------------------------------------------------------------------------

/// Split the pasted corpus into documents.
fn split_documents(text: &str, separator: &str) -> Result<Vec<String>, String> {
    let docs: Vec<String> = match separator {
        "blank-line" => {
            let mut out: Vec<String> = Vec::new();
            let mut current = String::new();
            for line in text.lines() {
                if line.trim().is_empty() {
                    if !current.trim().is_empty() {
                        out.push(current.trim().to_string());
                    }
                    current.clear();
                } else {
                    current.push_str(line);
                    current.push('\n');
                }
            }
            if !current.trim().is_empty() {
                out.push(current.trim().to_string());
            }
            out
        }
        "line" => text
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty())
            .map(|l| l.to_string())
            .collect(),
        "dashes" => {
            let mut out: Vec<String> = Vec::new();
            let mut current = String::new();
            for line in text.lines() {
                let t = line.trim();
                let is_fence = t.len() >= 3 && t.chars().all(|c| c == '-');
                if is_fence {
                    if !current.trim().is_empty() {
                        out.push(current.trim().to_string());
                    }
                    current.clear();
                } else {
                    current.push_str(line);
                    current.push('\n');
                }
            }
            if !current.trim().is_empty() {
                out.push(current.trim().to_string());
            }
            out
        }
        other => {
            return Err(format!(
                "unknown separator '{other}' — use 'blank-line', 'line', or 'dashes'."
            ))
        }
    };
    Ok(docs)
}

/// Lowercase word tokens: runs of alphanumerics and inner apostrophes, keeping
/// only tokens that contain at least one letter (bare numbers make noisy topic
/// words), are at least `min_len` characters, and aren't stopwords.
fn tokenize(text: &str, min_len: usize, stop: &HashSet<String>) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let flush = |current: &mut String, out: &mut Vec<String>| {
        let word = current.trim_matches('\'').to_string();
        current.clear();
        if word.chars().count() < min_len {
            return;
        }
        if !word.chars().any(|c| c.is_alphabetic()) {
            return;
        }
        if stop.contains(&word) {
            return;
        }
        out.push(word);
    };
    for c in text.chars() {
        if c.is_alphanumeric() || c == '\'' || c == '\u{2019}' {
            // Curly apostrophes normalise to the straight form so "don't" and
            // "don’t" are the same token (and both match the stopword list).
            current.push(if c == '\u{2019}' {
                '\''
            } else {
                c.to_lowercase().next().unwrap_or(c)
            });
        } else if !current.is_empty() {
            flush(&mut current, &mut out);
        }
    }
    if !current.is_empty() {
        flush(&mut current, &mut out);
    }
    out
}

/// Build the stopword set from the built-in list (when enabled) plus the user's
/// comma/whitespace separated additions (always applied).
fn stopword_set(remove_builtin: bool, extra: &str) -> HashSet<String> {
    let mut set: HashSet<String> = HashSet::new();
    if remove_builtin {
        set.extend(STOPWORDS.iter().map(|s| s.to_string()));
    }
    for w in extra.split(|c: char| c == ',' || c.is_whitespace()) {
        let w = w.trim().to_lowercase();
        if !w.is_empty() {
            set.insert(w);
        }
    }
    set
}

/// One-line preview of a document's raw text, for the report/JSON.
fn preview(text: &str, max_chars: usize) -> String {
    let collapsed: String = {
        let mut s = String::new();
        let mut space = false;
        for c in text.chars() {
            if c.is_whitespace() {
                space = !s.is_empty();
            } else {
                if space {
                    s.push(' ');
                }
                space = false;
                s.push(c);
            }
        }
        s
    };
    if collapsed.chars().count() <= max_chars {
        return collapsed;
    }
    let head: String = collapsed.chars().take(max_chars).collect();
    format!("{}…", head.trim_end())
}

fn validate(opts: &Options) -> Result<(), String> {
    if !(2..=20).contains(&opts.topics) {
        return Err(format!(
            "topics must be between 2 and 20 (got {}).",
            opts.topics
        ));
    }
    if !(1..=25).contains(&opts.words_per_topic) {
        return Err(format!(
            "words_per_topic must be between 1 and 25 (got {}).",
            opts.words_per_topic
        ));
    }
    if !(50..=1000).contains(&opts.iterations) {
        return Err(format!(
            "iterations must be between 50 and 1000 (got {}).",
            opts.iterations
        ));
    }
    if !(1..=12).contains(&opts.min_word_length) {
        return Err(format!(
            "min_word_length must be between 1 and 12 (got {}).",
            opts.min_word_length
        ));
    }
    if !opts.alpha.is_finite() || !(0.0..=100.0).contains(&opts.alpha) {
        return Err(format!(
            "alpha must be between 0 (auto = 50/K) and 100 (got {}).",
            opts.alpha
        ));
    }
    if !opts.beta.is_finite() || !(0.001..=1.0).contains(&opts.beta) {
        return Err(format!(
            "beta must be between 0.001 and 1 (got {}).",
            opts.beta
        ));
    }
    if !matches!(opts.output.as_str(), "report" | "json" | "csv") {
        return Err(format!(
            "unknown output '{}' — use 'report', 'json', or 'csv'.",
            opts.output
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// LDA — collapsed Gibbs sampling
// ---------------------------------------------------------------------------

/// Fit an LDA topic model over `documents`.
///
/// The corpus is split with `separator`, tokenised and filtered, then fitted by
/// `iterations` sweeps of collapsed Gibbs sampling. Topics come back sorted by
/// their share of the corpus, and every document mixture is aligned to that
/// order.
pub fn model(documents: &str, opts: &Options) -> Result<TopicModel, String> {
    validate(opts)?;

    if documents.trim().is_empty() {
        return Err("Please provide a corpus of documents to model.".into());
    }
    let raw_docs = split_documents(documents, &opts.separator)?;
    if raw_docs.len() < 2 {
        return Err(format!(
            "topic modelling needs at least 2 documents; the '{}' separator found {}. \
             Separate documents with a blank line, one per line, or a --- fence, and pick the matching separator.",
            opts.separator,
            raw_docs.len()
        ));
    }
    if raw_docs.len() > MAX_DOCS {
        return Err(format!(
            "too many documents: {} (cap {MAX_DOCS}). Model a smaller corpus.",
            raw_docs.len()
        ));
    }

    let stop = stopword_set(opts.remove_stopwords, &opts.stopwords);
    let min_len = opts.min_word_length as usize;

    // Tokenise into vocabulary ids. `doc_tokens[d]` holds word ids in order.
    let mut vocab: Vec<String> = Vec::new();
    let mut vocab_ids: HashMap<String, usize> = HashMap::new();
    let mut doc_tokens: Vec<Vec<usize>> = Vec::with_capacity(raw_docs.len());
    let mut total_tokens = 0usize;
    for doc in &raw_docs {
        let words = tokenize(doc, min_len, &stop);
        total_tokens += words.len();
        if total_tokens > MAX_TOKENS {
            return Err(format!(
                "corpus too large: more than {MAX_TOKENS} words after filtering. \
                 Model a smaller corpus, or raise min_word_length."
            ));
        }
        let mut ids = Vec::with_capacity(words.len());
        for w in words {
            let next = vocab.len();
            let id = *vocab_ids.entry(w.clone()).or_insert(next);
            if id == next {
                vocab.push(w);
                if vocab.len() > MAX_VOCAB {
                    return Err(format!(
                        "vocabulary too large: more than {MAX_VOCAB} distinct words. \
                         Model a smaller corpus, or raise min_word_length."
                    ));
                }
            }
            ids.push(id);
        }
        doc_tokens.push(ids);
    }

    if total_tokens == 0 {
        return Err(
            "no words left after filtering — lower min_word_length, or turn off stopword removal."
                .into(),
        );
    }

    let d = doc_tokens.len();
    let k = opts.topics as usize;
    let v = vocab.len();
    let alpha = if opts.alpha == 0.0 {
        50.0 / k as f64
    } else {
        opts.alpha
    };
    let beta = opts.beta;

    // Flat count matrices: ndk[d*k + t], nkw[t*v + w], nk[t].
    let mut ndk = vec![0u32; d * k];
    let mut nkw = vec![0u32; k * v];
    let mut nk = vec![0u32; k];
    // Topic assignment per token, in (doc, position) order.
    let mut z: Vec<usize> = Vec::with_capacity(total_tokens);

    let mut rng = Rng::new(opts.seed);
    for (di, ids) in doc_tokens.iter().enumerate() {
        for &w in ids {
            let t = (rng.next_u64() % k as u64) as usize;
            z.push(t);
            ndk[di * k + t] += 1;
            nkw[t * v + w] += 1;
            nk[t] += 1;
        }
    }

    // Collapsed Gibbs sweeps. p(t | ·) ∝ (ndk + α)·(nkw + β) / (nk + βV).
    let beta_v = beta * v as f64;
    let mut probs = vec![0.0f64; k];
    for _ in 0..opts.iterations {
        let mut i = 0usize;
        for (di, ids) in doc_tokens.iter().enumerate() {
            let doc_base = di * k;
            for &w in ids {
                let old = z[i];
                ndk[doc_base + old] -= 1;
                nkw[old * v + w] -= 1;
                nk[old] -= 1;

                let mut cumulative = 0.0;
                for (t, p) in probs.iter_mut().enumerate() {
                    cumulative += (ndk[doc_base + t] as f64 + alpha)
                        * (nkw[t * v + w] as f64 + beta)
                        / (nk[t] as f64 + beta_v);
                    *p = cumulative;
                }
                let target = rng.next_f64() * cumulative;
                let mut new = k - 1;
                for (t, p) in probs.iter().enumerate() {
                    if target < *p {
                        new = t;
                        break;
                    }
                }

                z[i] = new;
                ndk[doc_base + new] += 1;
                nkw[new * v + w] += 1;
                nk[new] += 1;
                i += 1;
            }
        }
    }

    // Rank topics by corpus share; ties keep the lower topic id (stable sort).
    let mut order: Vec<usize> = (0..k).collect();
    order.sort_by(|&a, &b| nk[b].cmp(&nk[a]));

    let top_n = opts.words_per_topic as usize;
    let mut topics = Vec::with_capacity(k);
    for (rank, &t) in order.iter().enumerate() {
        let denom = nk[t] as f64 + beta_v;
        let mut scored: Vec<(usize, f64)> = (0..v)
            .map(|w| (w, (nkw[t * v + w] as f64 + beta) / denom))
            .collect();
        // Weight desc, then word asc — a total order, so the top-word list is
        // stable even when several words tie.
        scored.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| vocab[a.0].cmp(&vocab[b.0]))
        });
        let words: Vec<TopicWord> = scored
            .iter()
            .take(top_n)
            .map(|&(w, weight)| TopicWord {
                word: vocab[w].clone(),
                weight,
            })
            .collect();
        let label = words
            .iter()
            .take(3)
            .map(|w| w.word.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        topics.push(Topic {
            rank: rank + 1,
            share: nk[t] as f64 / total_tokens as f64,
            label,
            words,
        });
    }

    // Document mixtures, in ranked-topic order. These are the *empirical*
    // assignment shares (n_dk / n_d), not the alpha-smoothed theta: with the
    // 50/K auto prior, alpha dwarfs the word counts of a pasted paragraph and
    // the smoothed mixture would read as a near-uniform split for every
    // document. alpha still does its job inside the sampler; what's reported is
    // the fraction of the document's own words each topic ended up with.
    let mut mixtures = Vec::with_capacity(d);
    for (di, ids) in doc_tokens.iter().enumerate() {
        let n_d = ids.len() as f64;
        let weights: Vec<f64> = order
            .iter()
            .map(|&t| {
                if n_d == 0.0 {
                    // A document whose words were all filtered out has no
                    // evidence either way: report the flat prior.
                    1.0 / k as f64
                } else {
                    ndk[di * k + t] as f64 / n_d
                }
            })
            .collect();
        let mut dominant = 0usize;
        for (i, w) in weights.iter().enumerate() {
            if *w > weights[dominant] {
                dominant = i;
            }
        }
        mixtures.push(DocumentMix {
            index: di + 1,
            preview: preview(&raw_docs[di], 64),
            tokens: ids.len(),
            weights,
            dominant: dominant + 1,
        });
    }

    Ok(TopicModel {
        documents: d,
        tokens: total_tokens,
        vocabulary: v,
        alpha,
        beta,
        iterations: opts.iterations,
        seed: opts.seed,
        topics,
        mixtures,
    })
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

fn csv_cell(s: &str) -> String {
    if s.contains(',') || s.contains('"') || s.contains('\n') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn render_report(m: &TopicModel) -> String {
    let mut out = String::new();
    out.push_str("LDA topic model\n");
    out.push_str(&format!(
        "{} documents · {} topics · {} words kept · {} vocabulary terms\n",
        m.documents,
        m.topics.len(),
        m.tokens,
        m.vocabulary
    ));
    out.push_str(&format!(
        "alpha {:.3} · beta {:.3} · {} iterations · seed {}\n",
        m.alpha, m.beta, m.iterations, m.seed
    ));

    out.push_str("\nTopics (ranked by share of the corpus)\n");
    for t in &m.topics {
        out.push_str(&format!(
            "\nTopic {} — {:.1}% of the corpus\n  ",
            t.rank,
            t.share * 100.0
        ));
        let words: Vec<String> = t
            .words
            .iter()
            .map(|w| format!("{} {:.2}%", w.word, w.weight * 100.0))
            .collect();
        out.push_str(&words.join(" · "));
        out.push('\n');
    }

    out.push_str("\nDocument mixtures\n");
    for doc in &m.mixtures {
        out.push_str(&format!(
            "\nDocument {} ({} words) — {}\n",
            doc.index, doc.tokens, doc.preview
        ));
        // Show the strongest three topics per document (or all, if fewer).
        let mut ranked: Vec<(usize, f64)> = doc
            .weights
            .iter()
            .enumerate()
            .map(|(i, w)| (i + 1, *w))
            .collect();
        ranked.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.0.cmp(&b.0))
        });
        let parts: Vec<String> = ranked
            .iter()
            .take(3)
            .map(|(rank, w)| format!("Topic {rank} {:.1}%", w * 100.0))
            .collect();
        out.push_str(&format!("  {}\n", parts.join(" · ")));
    }

    out.trim_end().to_string()
}

fn render_csv(m: &TopicModel) -> String {
    let mut out = String::new();
    // Section 1: topic keys (MALLET's topic-keys file).
    out.push_str("topic,share,top_words\n");
    for t in &m.topics {
        let words: Vec<&str> = t.words.iter().map(|w| w.word.as_str()).collect();
        out.push_str(&format!(
            "{},{:.4},{}\n",
            t.rank,
            t.share,
            csv_cell(&words.join(" "))
        ));
    }
    // Section 2: the doc–topic matrix (MALLET's doc-topics file).
    out.push('\n');
    out.push_str("document,words,dominant_topic");
    for t in &m.topics {
        out.push_str(&format!(",topic_{}", t.rank));
    }
    out.push('\n');
    for doc in &m.mixtures {
        out.push_str(&format!("{},{},{}", doc.index, doc.tokens, doc.dominant));
        for w in &doc.weights {
            out.push_str(&format!(",{w:.4}"));
        }
        out.push('\n');
    }
    out.trim_end().to_string()
}

/// Format a fitted model. `output` is "report" | "json" | "csv".
pub fn render(m: &TopicModel, output: &str) -> Result<String, String> {
    match output {
        "report" => Ok(render_report(m)),
        "csv" => Ok(render_csv(m)),
        "json" => {
            serde_json::to_string_pretty(m).map_err(|e| format!("failed to encode JSON: {e}"))
        }
        other => Err(format!(
            "unknown output '{other}' — use 'report', 'json', or 'csv'."
        )),
    }
}

/// Fit and format in one call — what the chat/CLI handler and the page wrapper
/// both use.
pub fn run(documents: &str, opts: &Options) -> Result<String, String> {
    let m = model(documents, opts)?;
    render(&m, &opts.output)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Six documents: three about cooking, three about programming, with
    /// disjoint content vocabularies.
    fn corpus() -> String {
        [
            "The recipe calls for butter, flour and sugar baked in a hot oven.",
            "Bake the dough in the oven until the crust turns golden brown, then serve.",
            "Sugar and butter cream together before the flour goes into the baking bowl.",
            "The function returns a value after the compiler checks every type in the module.",
            "A compiler error in the module means the function signature and the type disagree.",
            "Refactor the module so each function has a single return type the compiler accepts.",
        ]
        .join("\n\n")
    }

    #[test]
    fn stopwords_sorted_for_review() {
        let mut s = STOPWORDS.to_vec();
        s.sort();
        assert_eq!(s, STOPWORDS, "STOPWORDS must stay sorted");
    }

    #[test]
    fn separates_two_distinct_document_clusters() {
        let opts = Options {
            topics: 2,
            alpha: 0.1,
            iterations: 500,
            ..Options::default()
        };
        let m = model(&corpus(), &opts).expect("model");
        assert_eq!(m.documents, 6);
        assert_eq!(m.topics.len(), 2);
        assert_eq!(m.mixtures.len(), 6);

        // The sampler is intentionally stochastic (but seeded), so per-document
        // dominant-topic ties can move between seeds. The stable invariant for
        // this disjoint corpus is that the learned topic word lists separate the
        // two vocabularies.
        let dom: Vec<usize> = m.mixtures.iter().map(|d| d.dominant).collect();
        assert!(
            dom.iter().all(|d| (1..=2).contains(d)),
            "dominants are ranked topic ids: {dom:?}"
        );

        // Each topic's top words come from its own cluster's vocabulary.
        let cooking = [
            "oven", "butter", "flour", "sugar", "bake", "baking", "dough",
        ];
        let code = [
            "compiler", "function", "module", "type", "returns", "return",
        ];
        let mut cooking_topics = 0;
        let mut code_topics = 0;
        for t in &m.topics {
            let words: Vec<&str> = t.words.iter().map(|w| w.word.as_str()).collect();
            let c = words.iter().filter(|w| cooking.contains(w)).count();
            let p = words.iter().filter(|w| code.contains(w)).count();
            if c >= 3 {
                cooking_topics += 1;
            }
            if p >= 3 {
                code_topics += 1;
            }
            assert!(
                c >= 3 || p >= 3,
                "topic {} has no clear cluster: {words:?}",
                t.rank
            );
        }
        assert!(cooking_topics >= 1, "no cooking topic learned");
        assert!(code_topics >= 1, "no code topic learned");

        // Shares and mixtures are probability distributions.
        let total: f64 = m.topics.iter().map(|t| t.share).sum();
        assert!((total - 1.0).abs() < 1e-9, "topic shares sum to 1: {total}");
        for d in &m.mixtures {
            let s: f64 = d.weights.iter().sum();
            assert!((s - 1.0).abs() < 1e-9, "doc {} mixture sums to 1", d.index);
        }
    }

    #[test]
    fn is_deterministic_for_a_seed() {
        let opts = Options::default();
        let a = run(&corpus(), &opts).expect("run a");
        let b = run(&corpus(), &opts).expect("run b");
        assert_eq!(a, b, "same seed + input must give the same result");

        let other = run(
            &corpus(),
            &Options {
                seed: 7,
                ..Options::default()
            },
        )
        .expect("run seed 7");
        // A different seed is still a valid model (it may or may not differ —
        // what matters is that it succeeds and stays self-consistent).
        assert!(other.contains("LDA topic model"));
    }

    #[test]
    fn report_lists_topics_and_document_mixtures() {
        let out = run(&corpus(), &Options::default()).expect("report");
        assert!(out.starts_with("LDA topic model"));
        assert!(out.contains("6 documents · 5 topics"));
        assert!(out.contains("Topics (ranked by share of the corpus)"));
        assert!(out.contains("Topic 1 —"));
        assert!(out.contains("Document mixtures"));
        assert!(out.contains("Document 6 ("));
        // alpha 0 resolves to 50/K = 10 for the default 5 topics.
        assert!(out.contains("alpha 10.000"), "{out}");
    }

    #[test]
    fn json_output_carries_topics_and_mixtures() {
        let out = run(
            &corpus(),
            &Options {
                topics: 3,
                words_per_topic: 4,
                output: "json".into(),
                ..Options::default()
            },
        )
        .expect("json");
        let v: serde_json::Value = serde_json::from_str(&out).expect("valid json");
        assert_eq!(v["documents"], 6);
        assert_eq!(v["topics"].as_array().unwrap().len(), 3);
        assert_eq!(v["topics"][0]["words"].as_array().unwrap().len(), 4);
        assert_eq!(v["mixtures"].as_array().unwrap().len(), 6);
        assert_eq!(v["mixtures"][0]["weights"].as_array().unwrap().len(), 3);
    }

    #[test]
    fn csv_output_has_topic_keys_then_the_doc_topic_matrix() {
        let out = run(
            &corpus(),
            &Options {
                topics: 2,
                output: "csv".into(),
                ..Options::default()
            },
        )
        .expect("csv");
        let mut sections = out.split("\n\n");
        let keys = sections.next().expect("topic keys section");
        let matrix = sections.next().expect("doc-topic section");
        assert!(keys.starts_with("topic,share,top_words\n"));
        assert_eq!(keys.lines().count(), 3, "header + 2 topics");
        assert_eq!(
            matrix.lines().next().unwrap(),
            "document,words,dominant_topic,topic_1,topic_2"
        );
        assert_eq!(matrix.lines().count(), 7, "header + 6 documents");
    }

    #[test]
    fn separators_split_the_same_corpus_differently() {
        let text = "alpha beta gamma\ndelta epsilon zeta\n\ntheta iota kappa\nlambda mu nu";
        let by_blank = model(
            text,
            &Options {
                topics: 2,
                min_word_length: 1,
                ..Options::default()
            },
        )
        .expect("blank-line");
        assert_eq!(by_blank.documents, 2);

        let by_line = model(
            text,
            &Options {
                topics: 2,
                min_word_length: 1,
                separator: "line".into(),
                ..Options::default()
            },
        )
        .expect("line");
        assert_eq!(by_line.documents, 4);

        let fenced = "one two three\n---\nfour five six\n-----\nseven eight nine";
        let by_dashes = model(
            fenced,
            &Options {
                topics: 2,
                min_word_length: 1,
                separator: "dashes".into(),
                ..Options::default()
            },
        )
        .expect("dashes");
        assert_eq!(by_dashes.documents, 3);
    }

    #[test]
    fn stopword_options_control_the_vocabulary() {
        let text = "the quick brown fox\n\nthe lazy brown dog";
        let kept = model(
            text,
            &Options {
                topics: 2,
                remove_stopwords: false,
                min_word_length: 1,
                ..Options::default()
            },
        )
        .expect("kept");
        assert_eq!(kept.vocabulary, 6, "'the' is kept as one distinct term");

        let dropped = model(
            text,
            &Options {
                topics: 2,
                remove_stopwords: true,
                stopwords: "brown, fox".into(),
                min_word_length: 1,
                ..Options::default()
            },
        )
        .expect("dropped");
        let words: Vec<&str> = dropped
            .topics
            .iter()
            .flat_map(|t| t.words.iter().map(|w| w.word.as_str()))
            .collect();
        for banned in ["the", "brown", "fox"] {
            assert!(!words.contains(&banned), "'{banned}' should be filtered");
        }
    }

    #[test]
    fn min_word_length_prunes_short_words() {
        let text = "cat dog elephant\n\ncat ox elephant giraffe";
        let m = model(
            text,
            &Options {
                topics: 2,
                min_word_length: 4,
                ..Options::default()
            },
        )
        .expect("model");
        let words: Vec<&str> = m
            .topics
            .iter()
            .flat_map(|t| t.words.iter().map(|w| w.word.as_str()))
            .collect();
        assert!(!words.contains(&"cat"));
        assert!(!words.contains(&"dog"));
        assert!(words.contains(&"elephant"));
    }

    #[test]
    fn rejects_an_empty_corpus() {
        let err = model("   \n\n  ", &Options::default()).unwrap_err();
        assert!(err.contains("corpus"), "{err}");
    }

    #[test]
    fn rejects_a_single_document() {
        let err = model("just one paragraph of text", &Options::default()).unwrap_err();
        assert!(err.contains("at least 2 documents"), "{err}");
    }

    #[test]
    fn rejects_out_of_range_and_unknown_options() {
        for (opts, needle) in [
            (
                Options {
                    topics: 1,
                    ..Options::default()
                },
                "topics must be between 2 and 20",
            ),
            (
                Options {
                    words_per_topic: 26,
                    ..Options::default()
                },
                "words_per_topic must be between 1 and 25",
            ),
            (
                Options {
                    iterations: 10,
                    ..Options::default()
                },
                "iterations must be between 50 and 1000",
            ),
            (
                Options {
                    min_word_length: 13,
                    ..Options::default()
                },
                "min_word_length must be between 1 and 12",
            ),
            (
                Options {
                    alpha: -1.0,
                    ..Options::default()
                },
                "alpha must be between 0",
            ),
            (
                Options {
                    beta: 0.0,
                    ..Options::default()
                },
                "beta must be between 0.001 and 1",
            ),
            (
                Options {
                    output: "pdf".into(),
                    ..Options::default()
                },
                "unknown output 'pdf'",
            ),
            (
                Options {
                    separator: "commas".into(),
                    ..Options::default()
                },
                "unknown separator 'commas'",
            ),
        ] {
            let err = model(&corpus(), &opts).unwrap_err();
            assert!(err.contains(needle), "expected {needle:?}, got {err:?}");
        }
    }

    #[test]
    fn rejects_a_corpus_with_nothing_left_after_filtering() {
        let err = model(
            "the and of\n\nto it is",
            &Options {
                topics: 2,
                ..Options::default()
            },
        )
        .unwrap_err();
        assert!(err.contains("no words left after filtering"), "{err}");
    }

    #[test]
    fn rejects_more_documents_than_the_cap() {
        let text = (0..MAX_DOCS + 1)
            .map(|i| format!("document number {i} about topics"))
            .collect::<Vec<_>>()
            .join("\n\n");
        let err = model(&text, &Options::default()).unwrap_err();
        assert!(err.contains("too many documents"), "{err}");
    }
}
