//! few-shot-text-classifier core — pure compute, shared by the chat skill block and the web page.
//! Classifies text by similarity to a handful of user-provided labeled examples: it vectorizes the
//! support set with tf-idf (or plain tf / binary) features, then scores the query against label
//! centroids, k nearest neighbours, or the single best-matching example. No I/O, no randomness, no
//! pre-trained model — fully deterministic and reproducible.

use std::collections::{BTreeMap, HashMap};

/// Maximum size of the labeled example block, in bytes.
pub const MAX_EXAMPLES_BYTES: usize = 1_048_576;
/// Maximum size of the text being classified, in bytes.
pub const MAX_TEXT_BYTES: usize = 262_144;
/// Maximum number of labeled examples in the support set.
pub const MAX_EXAMPLES: usize = 5_000;
/// Maximum number of distinct labels.
pub const MAX_LABELS: usize = 200;
/// Maximum lines classified in one run when `input_mode=lines`.
pub const MAX_BATCH_LINES: usize = 1_000;
/// Maximum n-gram length.
pub const MAX_NGRAM: usize = 6;
/// Maximum accepted `k` for the knn method.
pub const MAX_K: usize = 50;
/// Maximum accepted `top_k`.
pub const MAX_TOP_K: usize = 50;
/// Maximum accepted `min_df`.
pub const MAX_MIN_DF: usize = 100;
/// Maximum vocabulary size after `min_df` filtering.
pub const MAX_VOCAB: usize = 200_000;
/// Label reported when the winner does not clear `min_confidence` (or nothing matched at all).
pub const UNCERTAIN: &str = "uncertain";

/// Terms listed in a single-document explanation.
const EXPLAIN_TERMS: usize = 8;
/// Terms listed per row in a batch explanation.
const EXPLAIN_TERMS_BATCH: usize = 3;
/// Batch rows and the nearest-example line show at most this many characters of the source text.
const SNIPPET_WIDTH: usize = 60;

/// A conservative English stop-word list, applied before n-grams are formed.
const STOPWORDS: &[&str] = &[
    "a", "about", "above", "after", "again", "against", "all", "am", "an", "and", "any", "are",
    "as", "at", "be", "because", "been", "before", "being", "below", "between", "both", "but",
    "by", "can", "cannot", "could", "did", "do", "does", "doing", "down", "during", "each", "few",
    "for", "from", "further", "had", "has", "have", "having", "he", "her", "here", "hers",
    "herself", "him", "himself", "his", "how", "i", "if", "in", "into", "is", "it", "its",
    "itself", "just", "me", "more", "most", "must", "my", "myself", "no", "nor", "not", "now",
    "of", "off", "on", "once", "only", "or", "other", "ought", "our", "ours", "ourselves", "out",
    "over", "own", "same", "she", "should", "so", "some", "such", "than", "that", "the", "their",
    "theirs", "them", "themselves", "then", "there", "these", "they", "this", "those", "through",
    "to", "too", "under", "until", "up", "very", "was", "we", "were", "what", "when", "where",
    "which", "while", "who", "whom", "why", "will", "with", "would", "you", "your", "yours",
    "yourself", "yourselves",
];

/// Every knob the classifier exposes; mirrors the block descriptor 1:1.
#[derive(Clone, Debug)]
pub struct Options {
    /// `auto`, `tab`, `comma`, `pipe` or `colon` — how label and text are split.
    pub separator: String,
    /// `single` (one document) or `lines` (one document per non-empty line).
    pub input_mode: String,
    /// `centroid`, `knn` or `best-match`.
    pub method: String,
    /// Neighbours inspected by the `knn` method.
    pub k: usize,
    /// `cosine`, `dot`, `euclidean` or `jaccard`.
    pub similarity: String,
    /// `tfidf`, `tf` or `binary` feature weighting.
    pub weighting: String,
    /// `word` or `char` features.
    pub analyzer: String,
    /// Longest n-gram formed from the token sequence.
    pub ngram_max: usize,
    /// Lower-case everything before tokenizing.
    pub lowercase: bool,
    /// Fold accented Latin letters onto their ASCII base.
    pub strip_accents: bool,
    /// Drop English stop words before forming n-grams.
    pub remove_stopwords: bool,
    /// Replace the raw term frequency with `1 + ln(tf)`.
    pub sublinear_tf: bool,
    /// Drop vocabulary entries that appear in fewer than this many examples.
    pub min_df: usize,
    /// Confidence the winner must reach, else the prediction is reported as `uncertain`.
    pub min_confidence: f64,
    /// How many labels to list (0 = all).
    pub top_k: usize,
    /// Include the shared terms and nearest example that drove the decision.
    pub explain: bool,
    /// `report`, `json` or `csv`.
    pub output: String,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            separator: "auto".into(),
            input_mode: "single".into(),
            method: "centroid".into(),
            k: 3,
            similarity: "cosine".into(),
            weighting: "tfidf".into(),
            analyzer: "word".into(),
            ngram_max: 1,
            lowercase: true,
            strip_accents: false,
            remove_stopwords: false,
            sublinear_tf: false,
            min_df: 1,
            min_confidence: 0.0,
            top_k: 3,
            explain: true,
            output: "report".into(),
        }
    }
}

/// A sparse feature vector: `(term id, weight)` pairs sorted by term id.
type Vector = Vec<(u32, f64)>;

/// One parsed labeled example plus its feature vector.
struct Example {
    label: String,
    text: String,
    vector: Vector,
}

/// A scored label in the final ranking.
struct LabelScore {
    label: String,
    /// The metric value for this label (cosine/jaccard 0–1, dot ≥ 0, euclidean mapped to 1/(1+d)).
    similarity: f64,
    /// Non-negative vote weight, normalized into `confidence` across all labels.
    weight: f64,
    confidence: f64,
    examples: usize,
}

/// The result of classifying one document.
struct Outcome {
    prediction: String,
    confidence: f64,
    similarity: f64,
    scores: Vec<LabelScore>,
    /// Shared terms that drove the winner, most influential first.
    terms: Vec<(String, f64)>,
    /// The single closest training example: `(label, text, similarity)`.
    nearest: Option<(String, String, f64)>,
    oov: usize,
    features: usize,
}

/// Classify `text` against the labeled `examples` support set.
///
/// Returns the rendered report / JSON / CSV, or a human-readable error message.
pub fn classify(examples: &str, text: &str, opts: &Options) -> Result<String, String> {
    let opts = validate(opts)?;
    if examples.len() > MAX_EXAMPLES_BYTES {
        return Err(format!(
            "examples is {} bytes, over the {MAX_EXAMPLES_BYTES}-byte limit",
            examples.len()
        ));
    }
    if text.len() > MAX_TEXT_BYTES {
        return Err(format!(
            "text is {} bytes, over the {MAX_TEXT_BYTES}-byte limit",
            text.len()
        ));
    }

    let (pairs, separator) = parse_examples(examples, &opts.separator)?;
    let model = Model::train(&pairs, &opts)?;

    let queries: Vec<String> = if opts.input_mode == "lines" {
        let lines: Vec<String> = text
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect();
        if lines.is_empty() {
            return Err("text has no non-blank lines to classify".into());
        }
        if lines.len() > MAX_BATCH_LINES {
            return Err(format!(
                "text has {} non-blank lines, over the {MAX_BATCH_LINES}-line batch limit",
                lines.len()
            ));
        }
        lines
    } else {
        if text.trim().is_empty() {
            return Err("text is empty — provide the text you want classified".into());
        }
        vec![text.trim().to_string()]
    };

    let outcomes: Vec<Outcome> = queries.iter().map(|q| model.classify_one(q, &opts)).collect();

    let stats = Stats {
        examples: model.examples.len(),
        labels: model.label_counts.len(),
        vocabulary: model.vocab_size,
        separator: separator.to_string(),
    };

    Ok(match opts.output.as_str() {
        "json" => render_json(&queries, &outcomes, &opts, &stats),
        "csv" => render_csv(&queries, &outcomes, &opts),
        _ => render_report(&queries, &outcomes, &opts, &stats),
    })
}

/// Training-set facts echoed back with every result.
struct Stats {
    examples: usize,
    labels: usize,
    vocabulary: usize,
    separator: String,
}

/// Reject unknown enum values and out-of-range numbers up front, with a hint each time.
fn validate(o: &Options) -> Result<Options, String> {
    let mut out = o.clone();
    out.separator = check_enum(
        "separator",
        &o.separator,
        &["auto", "tab", "comma", "pipe", "colon"],
    )?;
    out.input_mode = check_enum("input_mode", &o.input_mode, &["single", "lines"])?;
    out.method = check_enum("method", &o.method, &["centroid", "knn", "best-match"])?;
    out.similarity = check_enum(
        "similarity",
        &o.similarity,
        &["cosine", "dot", "euclidean", "jaccard"],
    )?;
    out.weighting = check_enum("weighting", &o.weighting, &["tfidf", "tf", "binary"])?;
    out.analyzer = check_enum("analyzer", &o.analyzer, &["word", "char"])?;
    out.output = check_enum("output", &o.output, &["report", "json", "csv"])?;

    if o.k < 1 || o.k > MAX_K {
        return Err(format!("k must be between 1 and {MAX_K}, got {}", o.k));
    }
    if o.ngram_max < 1 || o.ngram_max > MAX_NGRAM {
        return Err(format!(
            "ngram_max must be between 1 and {MAX_NGRAM}, got {}",
            o.ngram_max
        ));
    }
    if o.min_df < 1 || o.min_df > MAX_MIN_DF {
        return Err(format!(
            "min_df must be between 1 and {MAX_MIN_DF}, got {}",
            o.min_df
        ));
    }
    if o.top_k > MAX_TOP_K {
        return Err(format!(
            "top_k must be between 0 and {MAX_TOP_K}, got {}",
            o.top_k
        ));
    }
    if !(0.0..=1.0).contains(&o.min_confidence) || o.min_confidence.is_nan() {
        return Err(format!(
            "min_confidence must be between 0 and 1, got {}",
            o.min_confidence
        ));
    }
    Ok(out)
}

fn check_enum(name: &str, value: &str, allowed: &[&str]) -> Result<String, String> {
    let v = value.trim();
    if allowed.contains(&v) {
        Ok(v.to_string())
    } else {
        Err(format!(
            "{name} must be one of {}, got \"{value}\"",
            allowed.join(", ")
        ))
    }
}

/// Split the pasted support set into `(label, text)` pairs, resolving `auto` separators.
fn parse_examples(raw: &str, separator: &str) -> Result<(Vec<(String, String)>, &'static str), String> {
    let lines: Vec<&str> = raw
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .collect();
    if lines.is_empty() {
        return Err(
            "examples is empty — paste labeled examples, one per line, as label<separator>text"
                .into(),
        );
    }
    if lines.len() > MAX_EXAMPLES {
        return Err(format!(
            "examples has {} lines, over the {MAX_EXAMPLES}-example limit",
            lines.len()
        ));
    }

    let chosen: &'static str = if separator == "auto" {
        let mut best = ("comma", 0usize);
        for (name, ch) in [("tab", '\t'), ("comma", ','), ("pipe", '|'), ("colon", ':')] {
            let hits = lines.iter().filter(|l| l.contains(ch)).count();
            if hits > best.1 {
                best = (name, hits);
            }
        }
        if best.1 == 0 {
            return Err("could not detect a separator — every example line must contain a tab, comma, pipe or colon between the label and the text".into());
        }
        best.0
    } else {
        match separator {
            "tab" => "tab",
            "comma" => "comma",
            "pipe" => "pipe",
            _ => "colon",
        }
    };
    let ch = separator_char(chosen);

    let mut pairs = Vec::with_capacity(lines.len());
    for (i, line) in lines.iter().enumerate() {
        let Some(idx) = line.find(ch) else {
            return Err(format!(
                "example line {} has no \"{}\" separator: {}",
                i + 1,
                chosen,
                snippet(line, SNIPPET_WIDTH)
            ));
        };
        let label = unquote(line[..idx].trim());
        let body = unquote(line[idx + ch.len_utf8()..].trim());
        if label.is_empty() {
            return Err(format!("example line {} has an empty label", i + 1));
        }
        if body.is_empty() {
            return Err(format!(
                "example line {} has an empty text for label \"{label}\"",
                i + 1
            ));
        }
        pairs.push((label, body));
    }

    let distinct: std::collections::BTreeSet<&str> =
        pairs.iter().map(|(l, _)| l.as_str()).collect();
    if distinct.len() < 2 {
        return Err(format!(
            "need at least 2 distinct labels to classify between, found {} (\"{}\")",
            distinct.len(),
            distinct.iter().copied().collect::<Vec<_>>().join("\", \"")
        ));
    }
    if distinct.len() > MAX_LABELS {
        return Err(format!(
            "examples has {} distinct labels, over the {MAX_LABELS}-label limit",
            distinct.len()
        ));
    }
    Ok((pairs, chosen))
}

fn separator_char(name: &str) -> char {
    match name {
        "tab" => '\t',
        "comma" => ',',
        "pipe" => '|',
        _ => ':',
    }
}

/// Strip one layer of surrounding double quotes so a pasted two-column CSV works.
fn unquote(s: &str) -> String {
    let t = s.trim();
    if t.len() >= 2 && t.starts_with('"') && t.ends_with('"') {
        t[1..t.len() - 1].replace("\"\"", "\"")
    } else {
        t.to_string()
    }
}

/// The trained support set: vocabulary, idf weights, example vectors and label centroids.
struct Model {
    examples: Vec<Example>,
    label_counts: BTreeMap<String, usize>,
    centroids: BTreeMap<String, Vector>,
    vocab: HashMap<String, u32>,
    terms: Vec<String>,
    idf: Vec<f64>,
    vocab_size: usize,
}

impl Model {
    fn train(pairs: &[(String, String)], o: &Options) -> Result<Model, String> {
        let counted: Vec<HashMap<String, usize>> =
            pairs.iter().map(|(_, t)| tokenize(t, o)).collect();

        let mut df: HashMap<&str, usize> = HashMap::new();
        for doc in &counted {
            for term in doc.keys() {
                *df.entry(term.as_str()).or_insert(0) += 1;
            }
        }
        let mut kept: Vec<&str> = df
            .iter()
            .filter(|(_, &n)| n >= o.min_df)
            .map(|(t, _)| *t)
            .collect();
        kept.sort_unstable();
        if kept.is_empty() {
            return Err(if o.min_df > 1 {
                format!(
                    "no terms survived min_df={} — lower it or add more examples",
                    o.min_df
                )
            } else {
                "the examples produced no usable terms — check the separator and n-gram settings"
                    .into()
            });
        }
        if kept.len() > MAX_VOCAB {
            return Err(format!(
                "vocabulary is {} terms, over the {MAX_VOCAB}-term limit — raise min_df or lower ngram_max",
                kept.len()
            ));
        }

        let n = pairs.len() as f64;
        let mut vocab = HashMap::with_capacity(kept.len());
        let mut terms = Vec::with_capacity(kept.len());
        let mut idf = Vec::with_capacity(kept.len());
        for (id, term) in kept.iter().enumerate() {
            vocab.insert((*term).to_string(), id as u32);
            terms.push((*term).to_string());
            // Smoothed idf, as if one extra document contained every term.
            let d = df[*term] as f64;
            idf.push(((1.0 + n) / (1.0 + d)).ln() + 1.0);
        }

        let mut examples = Vec::with_capacity(pairs.len());
        let mut label_counts: BTreeMap<String, usize> = BTreeMap::new();
        let mut sums: BTreeMap<String, BTreeMap<u32, f64>> = BTreeMap::new();
        for ((label, text), doc) in pairs.iter().zip(counted.iter()) {
            let vector = weigh(doc, &vocab, &idf, o);
            *label_counts.entry(label.clone()).or_insert(0) += 1;
            let acc = sums.entry(label.clone()).or_default();
            for &(id, w) in &vector {
                *acc.entry(id).or_insert(0.0) += w;
            }
            examples.push(Example {
                label: label.clone(),
                text: text.clone(),
                vector,
            });
        }

        let centroids = sums
            .into_iter()
            .map(|(label, acc)| {
                let count = label_counts[&label] as f64;
                let v: Vector = acc.into_iter().map(|(id, w)| (id, w / count)).collect();
                (label, v)
            })
            .collect();

        let vocab_size = terms.len();
        Ok(Model {
            examples,
            label_counts,
            centroids,
            vocab,
            terms,
            idf,
            vocab_size,
        })
    }

    /// Score one query document against the support set.
    fn classify_one(&self, text: &str, o: &Options) -> Outcome {
        let doc = tokenize(text, o);
        let features = doc.len();
        let oov = doc.keys().filter(|t| !self.vocab.contains_key(*t)).count();
        let query = weigh(&doc, &self.vocab, &self.idf, o);

        // Similarity to every single example — used for knn, best-match and the nearest line.
        let mut per_example: Vec<f64> = Vec::with_capacity(self.examples.len());
        for ex in &self.examples {
            per_example.push(similarity(&o.similarity, &query, &ex.vector));
        }

        let mut weights: BTreeMap<&str, f64> = BTreeMap::new();
        let mut sims: BTreeMap<&str, f64> = BTreeMap::new();
        for label in self.label_counts.keys() {
            weights.insert(label.as_str(), 0.0);
            sims.insert(label.as_str(), 0.0);
        }

        match o.method.as_str() {
            "centroid" => {
                for (label, centroid) in &self.centroids {
                    let s = similarity(&o.similarity, &query, centroid);
                    weights.insert(label.as_str(), s.max(0.0));
                    sims.insert(label.as_str(), s);
                }
            }
            "best-match" => {
                for (ex, &s) in self.examples.iter().zip(per_example.iter()) {
                    let slot = sims.get_mut(ex.label.as_str()).unwrap();
                    if s > *slot {
                        *slot = s;
                    }
                }
                for (label, s) in &sims {
                    weights.insert(label, s.max(0.0));
                }
            }
            _ => {
                // knn: rank every example, then let the k nearest vote with their similarity.
                let mut order: Vec<usize> = (0..self.examples.len()).collect();
                order.sort_by(|&a, &b| {
                    per_example[b]
                        .partial_cmp(&per_example[a])
                        .unwrap_or(std::cmp::Ordering::Equal)
                        .then(a.cmp(&b))
                });
                let take = o.k.min(order.len());
                let neighbours = &order[..take];
                let total: f64 = neighbours.iter().map(|&i| per_example[i].max(0.0)).sum();
                for &i in neighbours {
                    let label = self.examples[i].label.as_str();
                    let s = per_example[i];
                    // With no similarity anywhere, fall back to a plain one-vote-each count.
                    let vote = if total > 0.0 { s.max(0.0) } else { 1.0 };
                    *weights.get_mut(label).unwrap() += vote;
                    let slot = sims.get_mut(label).unwrap();
                    if s > *slot {
                        *slot = s;
                    }
                }
            }
        }

        let total: f64 = weights.values().sum();
        let mut scores: Vec<LabelScore> = weights
            .iter()
            .map(|(&label, &weight)| LabelScore {
                label: label.to_string(),
                similarity: sims[label],
                weight,
                confidence: if total > 0.0 { weight / total } else { 0.0 },
                examples: self.label_counts[label],
            })
            .collect();
        scores.sort_by(|a, b| {
            b.weight
                .partial_cmp(&a.weight)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.label.cmp(&b.label))
        });

        let top = &scores[0];
        let (prediction, confidence, sim) = if total <= 0.0 || top.confidence < o.min_confidence {
            (UNCERTAIN.to_string(), top.confidence, top.similarity)
        } else {
            (top.label.clone(), top.confidence, top.similarity)
        };

        let nearest = per_example
            .iter()
            .enumerate()
            .max_by(|(ai, a), (bi, b)| {
                a.partial_cmp(b)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(bi.cmp(ai))
            })
            .map(|(i, &s)| {
                (
                    self.examples[i].label.clone(),
                    self.examples[i].text.clone(),
                    s,
                )
            });

        // Explain against the winning label's reference vector (its centroid, or the best example).
        let terms = if o.explain && total > 0.0 {
            let reference: Option<&Vector> = if o.method == "centroid" {
                self.centroids.get(&top.label)
            } else {
                self.examples
                    .iter()
                    .zip(per_example.iter())
                    .filter(|(ex, _)| ex.label == top.label)
                    .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
                    .map(|(ex, _)| &ex.vector)
            };
            reference.map(|r| shared_terms(&query, r, &self.terms)).unwrap_or_default()
        } else {
            Vec::new()
        };

        Outcome {
            prediction,
            confidence,
            similarity: sim,
            scores,
            terms,
            nearest,
            oov,
            features,
        }
    }
}

/// Terms present in both vectors, ranked by how much they contributed to the match.
fn shared_terms(query: &Vector, reference: &Vector, names: &[String]) -> Vec<(String, f64)> {
    let mut out: Vec<(String, f64)> = Vec::new();
    let (mut i, mut j) = (0usize, 0usize);
    while i < query.len() && j < reference.len() {
        match query[i].0.cmp(&reference[j].0) {
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
            std::cmp::Ordering::Equal => {
                let c = query[i].1 * reference[j].1;
                if c > 0.0 {
                    out.push((names[query[i].0 as usize].clone(), c));
                }
                i += 1;
                j += 1;
            }
        }
    }
    out.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
    });
    out.truncate(EXPLAIN_TERMS);
    out
}

/// Turn per-term counts into a sorted sparse weight vector under the chosen weighting.
fn weigh(doc: &HashMap<String, usize>, vocab: &HashMap<String, u32>, idf: &[f64], o: &Options) -> Vector {
    let mut v: Vector = Vec::with_capacity(doc.len());
    for (term, &count) in doc {
        let Some(&id) = vocab.get(term) else { continue };
        let w = match o.weighting.as_str() {
            "binary" => 1.0,
            "tf" => term_frequency(count, o),
            _ => term_frequency(count, o) * idf[id as usize],
        };
        if w != 0.0 {
            v.push((id, w));
        }
    }
    v.sort_unstable_by_key(|&(id, _)| id);
    v
}

fn term_frequency(count: usize, o: &Options) -> f64 {
    if o.sublinear_tf {
        1.0 + (count as f64).ln()
    } else {
        count as f64
    }
}

/// Similarity between two sparse vectors; always "higher is more similar".
fn similarity(metric: &str, a: &Vector, b: &Vector) -> f64 {
    match metric {
        "dot" => dot(a, b),
        // Distance folded into a bounded (0, 1] similarity so every metric ranks the same way.
        "euclidean" => 1.0 / (1.0 + euclidean(a, b)),
        "jaccard" => jaccard(a, b),
        _ => {
            let (na, nb) = (norm(a), norm(b));
            if na == 0.0 || nb == 0.0 {
                0.0
            } else {
                (dot(a, b) / (na * nb)).clamp(-1.0, 1.0)
            }
        }
    }
}

fn dot(a: &Vector, b: &Vector) -> f64 {
    let (mut i, mut j, mut acc) = (0usize, 0usize, 0.0);
    while i < a.len() && j < b.len() {
        match a[i].0.cmp(&b[j].0) {
            std::cmp::Ordering::Less => i += 1,
            std::cmp::Ordering::Greater => j += 1,
            std::cmp::Ordering::Equal => {
                acc += a[i].1 * b[j].1;
                i += 1;
                j += 1;
            }
        }
    }
    acc
}

fn norm(a: &Vector) -> f64 {
    a.iter().map(|&(_, w)| w * w).sum::<f64>().sqrt()
}

fn euclidean(a: &Vector, b: &Vector) -> f64 {
    let (mut i, mut j, mut acc) = (0usize, 0usize, 0.0);
    while i < a.len() || j < b.len() {
        let d = if i == a.len() {
            let d = b[j].1;
            j += 1;
            d
        } else if j == b.len() {
            let d = a[i].1;
            i += 1;
            d
        } else {
            match a[i].0.cmp(&b[j].0) {
                std::cmp::Ordering::Less => {
                    let d = a[i].1;
                    i += 1;
                    d
                }
                std::cmp::Ordering::Greater => {
                    let d = b[j].1;
                    j += 1;
                    d
                }
                std::cmp::Ordering::Equal => {
                    let d = a[i].1 - b[j].1;
                    i += 1;
                    j += 1;
                    d
                }
            }
        };
        acc += d * d;
    }
    acc.sqrt()
}

fn jaccard(a: &Vector, b: &Vector) -> f64 {
    let (mut i, mut j, mut both, mut union) = (0usize, 0usize, 0usize, 0usize);
    while i < a.len() || j < b.len() {
        union += 1;
        if i == a.len() {
            j += 1;
        } else if j == b.len() {
            i += 1;
        } else {
            match a[i].0.cmp(&b[j].0) {
                std::cmp::Ordering::Less => i += 1,
                std::cmp::Ordering::Greater => j += 1,
                std::cmp::Ordering::Equal => {
                    both += 1;
                    i += 1;
                    j += 1;
                }
            }
        }
    }
    if union == 0 {
        0.0
    } else {
        both as f64 / union as f64
    }
}

/// Normalize, split and n-gram one document into term counts.
fn tokenize(text: &str, o: &Options) -> HashMap<String, usize> {
    let normalized = normalize(text, o);
    let mut words: Vec<&str> = normalized
        .split(|c: char| !c.is_alphanumeric() && c != '\'')
        .filter(|w| !w.is_empty())
        .collect();
    if o.remove_stopwords {
        words.retain(|w| !STOPWORDS.contains(w));
    }

    let mut counts: HashMap<String, usize> = HashMap::new();
    if o.analyzer == "char" {
        // Character n-grams of exactly ngram_max, over the words rejoined by single spaces.
        let joined = words.join(" ");
        let chars: Vec<char> = joined.chars().collect();
        if chars.len() >= o.ngram_max {
            for w in chars.windows(o.ngram_max) {
                *counts.entry(w.iter().collect::<String>()).or_insert(0) += 1;
            }
        }
    } else {
        for n in 1..=o.ngram_max {
            if words.len() < n {
                break;
            }
            for w in words.windows(n) {
                *counts.entry(w.join(" ")).or_insert(0) += 1;
            }
        }
    }
    counts
}

fn normalize(text: &str, o: &Options) -> String {
    let lowered = if o.lowercase {
        text.to_lowercase()
    } else {
        text.to_string()
    };
    if !o.strip_accents {
        return lowered;
    }
    let mut out = String::with_capacity(lowered.len());
    for c in lowered.chars() {
        match deaccent(c) {
            Some(s) => out.push_str(s),
            None => out.push(c),
        }
    }
    out
}

/// Fold the common accented Latin letters onto their ASCII base (no unicode crate needed).
fn deaccent(c: char) -> Option<&'static str> {
    Some(match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' | 'ā' | 'ă' | 'ą' => "a",
        'À' | 'Á' | 'Â' | 'Ã' | 'Ä' | 'Å' | 'Ā' | 'Ă' | 'Ą' => "A",
        'è' | 'é' | 'ê' | 'ë' | 'ē' | 'ĕ' | 'ė' | 'ę' | 'ě' => "e",
        'È' | 'É' | 'Ê' | 'Ë' | 'Ē' | 'Ĕ' | 'Ė' | 'Ę' | 'Ě' => "E",
        'ì' | 'í' | 'î' | 'ï' | 'ī' | 'ĭ' | 'į' | 'ı' => "i",
        'Ì' | 'Í' | 'Î' | 'Ï' | 'Ī' | 'Ĭ' | 'İ' => "I",
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' | 'ø' | 'ō' | 'ŏ' | 'ő' => "o",
        'Ò' | 'Ó' | 'Ô' | 'Õ' | 'Ö' | 'Ø' | 'Ō' | 'Ŏ' | 'Ő' => "O",
        'ù' | 'ú' | 'û' | 'ü' | 'ū' | 'ŭ' | 'ů' | 'ű' | 'ų' => "u",
        'Ù' | 'Ú' | 'Û' | 'Ü' | 'Ū' | 'Ŭ' | 'Ů' | 'Ű' | 'Ų' => "U",
        'ç' | 'ć' | 'ĉ' | 'ċ' | 'č' => "c",
        'Ç' | 'Ć' | 'Ĉ' | 'Ċ' | 'Č' => "C",
        'ñ' | 'ń' | 'ņ' | 'ň' => "n",
        'Ñ' | 'Ń' | 'Ņ' | 'Ň' => "N",
        'ý' | 'ÿ' => "y",
        'Ý' | 'Ÿ' => "Y",
        'š' | 'ś' | 'ŝ' | 'ş' => "s",
        'Š' | 'Ś' | 'Ŝ' | 'Ş' => "S",
        'ž' | 'ź' | 'ż' => "z",
        'Ž' | 'Ź' | 'Ż' => "Z",
        'ğ' | 'ĝ' | 'ġ' | 'ģ' => "g",
        'Ğ' | 'Ĝ' | 'Ġ' | 'Ģ' => "G",
        'ł' => "l",
        'Ł' => "L",
        'ř' | 'ŕ' => "r",
        'Ř' | 'Ŕ' => "R",
        'ť' | 'ţ' => "t",
        'Ť' | 'Ţ' => "T",
        'ď' | 'đ' | 'ð' => "d",
        'Ď' | 'Đ' | 'Ð' => "D",
        'þ' => "th",
        'Þ' => "Th",
        'ß' => "ss",
        'æ' => "ae",
        'Æ' => "AE",
        'œ' => "oe",
        'Œ' => "OE",
        _ => return None,
    })
}

/// Truncate to `width` characters, adding an ellipsis when something was cut.
fn snippet(s: &str, width: usize) -> String {
    let flat: String = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= width {
        flat
    } else {
        let head: String = flat.chars().take(width.saturating_sub(1)).collect();
        format!("{head}…")
    }
}

fn pct(v: f64) -> String {
    format!("{:.1}%", v * 100.0)
}

fn num(v: f64) -> String {
    format!("{v:.4}")
}

fn settings_line(o: &Options) -> String {
    let mut s = format!(
        "method={}  similarity={}  weighting={}  analyzer={}  ngram_max={}",
        o.method, o.similarity, o.weighting, o.analyzer, o.ngram_max
    );
    if o.method == "knn" {
        s.push_str(&format!("  k={}", o.k));
    }
    s.push_str(&format!(
        "\n  lowercase={}  strip_accents={}  stopwords={}  sublinear_tf={}  min_df={}  min_confidence={:.2}",
        on_off(o.lowercase),
        on_off(o.strip_accents),
        on_off(o.remove_stopwords),
        on_off(o.sublinear_tf),
        o.min_df,
        o.min_confidence
    ));
    s
}

fn on_off(b: bool) -> &'static str {
    if b {
        "on"
    } else {
        "off"
    }
}

/// Human-readable notes about anything surprising in this run.
fn notes(outcome: &Outcome, o: &Options) -> Vec<String> {
    let mut n = Vec::new();
    if outcome.prediction == UNCERTAIN {
        if outcome.scores.iter().all(|s| s.weight <= 0.0) {
            n.push(
                "Nothing matched: the input shares no vocabulary with any example.".to_string(),
            );
        } else {
            n.push(format!(
                "Top label \"{}\" reached {}, below min_confidence {:.2} — reported as {UNCERTAIN}.",
                outcome.scores[0].label,
                pct(outcome.scores[0].confidence),
                o.min_confidence
            ));
        }
    }
    if outcome.oov > 0 {
        n.push(format!(
            "{} of {} input terms are not in the training vocabulary and were ignored.",
            outcome.oov, outcome.features
        ));
    }
    if o.similarity == "jaccard" && o.weighting != "binary" {
        n.push(
            "jaccard compares term sets only, so the weighting setting does not affect it."
                .to_string(),
        );
    }
    if o.analyzer == "char" && o.ngram_max < 3 {
        n.push(format!(
            "analyzer=char with ngram_max={} uses very short character n-grams; 3–5 usually works better.",
            o.ngram_max
        ));
    }
    n
}

fn render_report(queries: &[String], outcomes: &[Outcome], o: &Options, stats: &Stats) -> String {
    let mut out = String::new();
    if o.input_mode == "lines" {
        out.push_str(&render_batch_table(queries, outcomes, o));
    } else {
        let r = &outcomes[0];
        out.push_str(&format!("Prediction:  {}\n", r.prediction));
        out.push_str(&format!("Confidence:  {}\n", pct(r.confidence)));
        out.push_str(&format!(
            "Similarity:  {} ({})\n",
            num(r.similarity),
            o.similarity
        ));

        out.push_str("\nLabel scores\n");
        let shown = if o.top_k == 0 {
            r.scores.len()
        } else {
            o.top_k.min(r.scores.len())
        };
        let width = r.scores[..shown]
            .iter()
            .map(|s| s.label.chars().count())
            .max()
            .unwrap_or(5)
            .max(5);
        out.push_str(&format!(
            "  {:<width$}  {:>10}  {:>10}  {:>8}\n",
            "label",
            "similarity",
            "confidence",
            "examples",
            width = width
        ));
        for s in &r.scores[..shown] {
            out.push_str(&format!(
                "  {:<width$}  {:>10}  {:>10}  {:>8}\n",
                s.label,
                num(s.similarity),
                pct(s.confidence),
                s.examples,
                width = width
            ));
        }
        if shown < r.scores.len() {
            out.push_str(&format!("  … {} more\n", r.scores.len() - shown));
        }

        if o.explain {
            if r.terms.is_empty() {
                out.push_str("\nTop matching terms\n  (none — no shared vocabulary)\n");
            } else {
                out.push_str("\nTop matching terms\n");
                let tw = r
                    .terms
                    .iter()
                    .map(|(t, _)| t.chars().count())
                    .max()
                    .unwrap_or(4);
                for (t, w) in &r.terms {
                    out.push_str(&format!("  {t:<tw$}  {}\n", num(*w), tw = tw));
                }
            }
            if let Some((label, text, sim)) = &r.nearest {
                out.push_str(&format!(
                    "\nNearest example\n  [{}] {}  (similarity {})\n",
                    label,
                    snippet(text, SNIPPET_WIDTH),
                    num(*sim)
                ));
            }
        }
    }

    out.push_str(&format!("\nSettings\n  {}\n", settings_line(o)));
    out.push_str(&format!(
        "\nTraining\n  examples={}  labels={}  vocabulary={}  separator={}\n",
        stats.examples, stats.labels, stats.vocabulary, stats.separator
    ));

    let mut seen: Vec<String> = Vec::new();
    for r in outcomes {
        for n in notes(r, o) {
            if !seen.contains(&n) {
                seen.push(n);
            }
        }
    }
    if !seen.is_empty() {
        out.push_str("\nNotes\n");
        for n in seen {
            out.push_str(&format!("  - {n}\n"));
        }
    }
    out
}

fn render_batch_table(queries: &[String], outcomes: &[Outcome], o: &Options) -> String {
    let texts: Vec<String> = queries.iter().map(|q| snippet(q, SNIPPET_WIDTH)).collect();
    let tw = texts
        .iter()
        .map(|t| t.chars().count())
        .max()
        .unwrap_or(4)
        .max(4);
    let lw = outcomes
        .iter()
        .map(|r| r.prediction.chars().count())
        .max()
        .unwrap_or(10)
        .max(10);

    let mut out = format!(
        "{:<4}  {:<tw$}  {:<lw$}  {:>10}  {:>10}",
        "#",
        "text",
        "prediction",
        "confidence",
        "similarity",
        tw = tw,
        lw = lw
    );
    if o.explain {
        out.push_str("  top terms");
    }
    out.push('\n');

    for (i, (t, r)) in texts.iter().zip(outcomes.iter()).enumerate() {
        out.push_str(&format!(
            "{:<4}  {:<tw$}  {:<lw$}  {:>10}  {:>10}",
            i + 1,
            t,
            r.prediction,
            pct(r.confidence),
            num(r.similarity),
            tw = tw,
            lw = lw
        ));
        if o.explain {
            let terms: Vec<&str> = r
                .terms
                .iter()
                .take(EXPLAIN_TERMS_BATCH)
                .map(|(t, _)| t.as_str())
                .collect();
            out.push_str(&format!(
                "  {}",
                if terms.is_empty() {
                    "-".to_string()
                } else {
                    terms.join(", ")
                }
            ));
        }
        out.push('\n');
    }
    out
}

fn render_csv(queries: &[String], outcomes: &[Outcome], o: &Options) -> String {
    let mut out = String::from("text,prediction,confidence,similarity");
    if o.explain {
        out.push_str(",top_terms");
    }
    out.push('\n');
    for (q, r) in queries.iter().zip(outcomes.iter()) {
        out.push_str(&csv_field(q));
        out.push(',');
        out.push_str(&csv_field(&r.prediction));
        out.push_str(&format!(",{:.4},{:.4}", r.confidence, r.similarity));
        if o.explain {
            let terms: Vec<&str> = r
                .terms
                .iter()
                .take(EXPLAIN_TERMS_BATCH)
                .map(|(t, _)| t.as_str())
                .collect();
            out.push(',');
            out.push_str(&csv_field(&terms.join(" ")));
        }
        out.push('\n');
    }
    out
}

fn csv_field(s: &str) -> String {
    let flat = s.replace(['\r', '\n'], " ");
    format!("\"{}\"", flat.replace('"', "\"\""))
}

fn json_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
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
    out
}

fn jstr(s: &str) -> String {
    format!("\"{}\"", json_escape(s))
}

fn render_json(queries: &[String], outcomes: &[Outcome], o: &Options, stats: &Stats) -> String {
    // Returns the object's fields WITHOUT the enclosing braces, so single mode can splice the
    // settings/training/notes in alongside them without any brace-trimming guesswork.
    let one_body = |q: &str, r: &Outcome| -> String {
        let shown = if o.top_k == 0 {
            r.scores.len()
        } else {
            o.top_k.min(r.scores.len())
        };
        let labels: Vec<String> = r.scores[..shown]
            .iter()
            .map(|s| {
                format!(
                    "{{\"label\":{},\"similarity\":{:.6},\"confidence\":{:.6},\"examples\":{}}}",
                    jstr(&s.label),
                    s.similarity,
                    s.confidence,
                    s.examples
                )
            })
            .collect();
        let mut body = format!(
            "\"text\":{},\"prediction\":{},\"confidence\":{:.6},\"similarity\":{:.6},\"labels\":[{}]",
            jstr(q),
            jstr(&r.prediction),
            r.confidence,
            r.similarity,
            labels.join(",")
        );
        if o.explain {
            let terms: Vec<String> = r
                .terms
                .iter()
                .map(|(t, w)| format!("{{\"term\":{},\"weight\":{:.6}}}", jstr(t), w))
                .collect();
            body.push_str(&format!(",\"terms\":[{}]", terms.join(",")));
            body.push_str(&match &r.nearest {
                Some((label, text, sim)) => format!(
                    ",\"nearest_example\":{{\"label\":{},\"text\":{},\"similarity\":{:.6}}}",
                    jstr(label),
                    jstr(text),
                    sim
                ),
                None => ",\"nearest_example\":null".to_string(),
            });
        }
        body
    };

    let bodies: Vec<String> = queries
        .iter()
        .zip(outcomes.iter())
        .map(|(q, r)| one_body(q, r))
        .collect();

    let mut seen: Vec<String> = Vec::new();
    for r in outcomes {
        for n in notes(r, o) {
            if !seen.contains(&n) {
                seen.push(n);
            }
        }
    }
    let notes_json: Vec<String> = seen.iter().map(|n| jstr(n)).collect();

    let settings = format!(
        "{{\"method\":{},\"k\":{},\"similarity\":{},\"weighting\":{},\"analyzer\":{},\"ngram_max\":{},\"lowercase\":{},\"strip_accents\":{},\"remove_stopwords\":{},\"sublinear_tf\":{},\"min_df\":{},\"min_confidence\":{}}}",
        jstr(&o.method),
        o.k,
        jstr(&o.similarity),
        jstr(&o.weighting),
        jstr(&o.analyzer),
        o.ngram_max,
        o.lowercase,
        o.strip_accents,
        o.remove_stopwords,
        o.sublinear_tf,
        o.min_df,
        o.min_confidence
    );
    let training = format!(
        "{{\"examples\":{},\"labels\":{},\"vocabulary\":{},\"separator\":{}}}",
        stats.examples,
        stats.labels,
        stats.vocabulary,
        jstr(&stats.separator)
    );

    if o.input_mode == "lines" {
        let results: Vec<String> = bodies.iter().map(|b| format!("{{{b}}}")).collect();
        format!(
            "{{\"results\":[{}],\"settings\":{},\"training\":{},\"notes\":[{}]}}",
            results.join(","),
            settings,
            training,
            notes_json.join(",")
        )
    } else {
        format!(
            "{{{},\"settings\":{},\"training\":{},\"notes\":[{}]}}",
            bodies[0],
            settings,
            training,
            notes_json.join(",")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SUPPORT: &str = "billing,invoice charge refund payment\nbilling,payment failed on my card\ntech,error login password reset\ntech,server timeout and a crash bug\nsales,pricing demo quote contract\nsales,renewal discount for the contract";

    fn opts() -> Options {
        Options::default()
    }

    #[test]
    fn classifies_a_single_document_with_the_centroid_method() {
        let out = classify(SUPPORT, "my card payment was refused, need a refund", &opts()).unwrap();
        assert!(out.starts_with("Prediction:  billing"), "{out}");
        assert!(out.contains("Label scores"), "{out}");
        assert!(out.contains("separator=comma"), "{out}");
    }

    #[test]
    fn json_output_is_parseable_and_carries_the_settings() {
        let mut o = opts();
        o.output = "json".into();
        let out = classify(SUPPORT, "password reset error", &o).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["prediction"], "tech");
        assert_eq!(v["settings"]["method"], "centroid");
        assert_eq!(v["settings"]["similarity"], "cosine");
        assert_eq!(v["training"]["labels"], 3);
        assert!(v["confidence"].as_f64().unwrap() > 0.5);
    }

    #[test]
    fn empty_examples_is_an_error() {
        let err = classify("   ", "hello", &opts()).unwrap_err();
        assert!(err.contains("examples is empty"), "{err}");
    }

    #[test]
    fn a_single_label_is_rejected() {
        let err = classify("spam,buy now\nspam,free money", "hi", &opts()).unwrap_err();
        assert!(err.contains("at least 2 distinct labels"), "{err}");
    }

    #[test]
    fn unknown_enum_values_are_rejected_with_the_allowed_list() {
        let mut o = opts();
        o.similarity = "manhattan".into();
        let err = classify(SUPPORT, "x", &o).unwrap_err();
        assert!(err.contains("similarity must be one of"), "{err}");
        assert!(err.contains("jaccard"), "{err}");
    }

    #[test]
    fn out_of_range_k_is_rejected() {
        let mut o = opts();
        o.k = MAX_K + 1;
        let err = classify(SUPPORT, "x", &o).unwrap_err();
        assert!(err.contains("k must be between 1 and 50"), "{err}");
    }

    #[test]
    fn every_method_agrees_on_an_obvious_case() {
        for method in ["centroid", "knn", "best-match"] {
            let mut o = opts();
            o.method = method.into();
            o.output = "json".into();
            let out = classify(SUPPORT, "renewal discount quote", &o).unwrap();
            let v: serde_json::Value = serde_json::from_str(&out).unwrap();
            assert_eq!(v["prediction"], "sales", "method={method}: {out}");
        }
    }

    #[test]
    fn every_similarity_metric_runs_and_ranks_the_right_label() {
        for metric in ["cosine", "dot", "euclidean", "jaccard"] {
            let mut o = opts();
            o.similarity = metric.into();
            o.output = "json".into();
            let out = classify(SUPPORT, "invoice refund charge", &o).unwrap();
            let v: serde_json::Value = serde_json::from_str(&out).unwrap();
            assert_eq!(v["prediction"], "billing", "metric={metric}: {out}");
            assert!(v["similarity"].as_f64().unwrap() > 0.0, "metric={metric}");
        }
    }

    #[test]
    fn every_weighting_runs() {
        for w in ["tfidf", "tf", "binary"] {
            let mut o = opts();
            o.weighting = w.into();
            o.output = "json".into();
            let out = classify(SUPPORT, "server crash timeout", &o).unwrap();
            let v: serde_json::Value = serde_json::from_str(&out).unwrap();
            assert_eq!(v["prediction"], "tech", "weighting={w}: {out}");
        }
    }

    #[test]
    fn char_analyzer_survives_a_typo_that_word_features_miss() {
        let mut word = opts();
        word.output = "json".into();
        let typo = "invoicce chargge";
        let v: serde_json::Value =
            serde_json::from_str(&classify(SUPPORT, typo, &word).unwrap()).unwrap();
        assert_eq!(v["prediction"], UNCERTAIN, "word features should whiff");

        let mut chars = opts();
        chars.analyzer = "char".into();
        chars.ngram_max = 4;
        chars.output = "json".into();
        let v: serde_json::Value =
            serde_json::from_str(&classify(SUPPORT, typo, &chars).unwrap()).unwrap();
        assert_eq!(v["prediction"], "billing");
    }

    #[test]
    fn batch_mode_labels_every_line() {
        let mut o = opts();
        o.input_mode = "lines".into();
        o.output = "csv".into();
        let out = classify(SUPPORT, "card charged twice\npassword reset\nquote please\n", &o)
            .unwrap();
        let lines: Vec<&str> = out.trim_end().lines().collect();
        assert_eq!(lines.len(), 4, "header + 3 rows: {out}");
        assert!(lines[0].starts_with("text,prediction,confidence,similarity"));
        assert!(lines[1].contains("\"billing\""), "{out}");
        assert!(lines[2].contains("\"tech\""), "{out}");
        assert!(lines[3].contains("\"sales\""), "{out}");
    }

    #[test]
    fn no_shared_vocabulary_reports_uncertain() {
        let mut o = opts();
        o.output = "json".into();
        let out = classify(SUPPORT, "zzzz qqqq wwww", &o).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["prediction"], UNCERTAIN);
        assert_eq!(v["confidence"], 0.0);
        assert!(v["notes"][0].as_str().unwrap().contains("Nothing matched"));
    }

    #[test]
    fn min_confidence_downgrades_a_weak_winner() {
        // "payment" pulls towards billing and "contract" towards sales, so no label runs away
        // with the vote — exactly the ambiguous case a confidence floor is meant to catch.
        let mut o = opts();
        o.output = "json".into();
        let committed: serde_json::Value =
            serde_json::from_str(&classify(SUPPORT, "payment contract", &o).unwrap()).unwrap();
        assert_eq!(committed["prediction"], "billing");
        assert!(committed["confidence"].as_f64().unwrap() < 0.9);

        o.min_confidence = 0.9;
        let v: serde_json::Value =
            serde_json::from_str(&classify(SUPPORT, "payment contract", &o).unwrap()).unwrap();
        assert_eq!(v["prediction"], UNCERTAIN);
        assert!(v["notes"][0]
            .as_str()
            .unwrap()
            .contains("below min_confidence"));
    }

    #[test]
    fn separators_and_quoted_csv_columns_all_parse() {
        for (name, sep) in [("tab", "\t"), ("comma", ","), ("pipe", "|"), ("colon", ":")] {
            let support = format!("a{sep}alpha beta\nb{sep}gamma delta");
            let mut o = opts();
            o.separator = name.into();
            o.output = "json".into();
            let v: serde_json::Value =
                serde_json::from_str(&classify(&support, "alpha", &o).unwrap()).unwrap();
            assert_eq!(v["prediction"], "a", "separator={name}");
            assert_eq!(v["training"]["separator"], name);
        }
        let mut o = opts();
        o.output = "json".into();
        let v: serde_json::Value = serde_json::from_str(
            &classify("\"a\",\"alpha, beta\"\n\"b\",\"gamma, delta\"", "beta", &o).unwrap(),
        )
        .unwrap();
        assert_eq!(v["prediction"], "a");
    }

    #[test]
    fn strip_accents_matches_an_unaccented_query() {
        let support = "fr,café résumé\nde,straße gebäude";
        let mut plain = opts();
        plain.output = "json".into();
        let v: serde_json::Value =
            serde_json::from_str(&classify(support, "cafe resume", &plain).unwrap()).unwrap();
        assert_eq!(v["prediction"], UNCERTAIN);

        let mut folded = opts();
        folded.strip_accents = true;
        folded.output = "json".into();
        let v: serde_json::Value =
            serde_json::from_str(&classify(support, "cafe resume", &folded).unwrap()).unwrap();
        assert_eq!(v["prediction"], "fr");
    }

    #[test]
    fn min_df_that_empties_the_vocabulary_is_an_error() {
        let mut o = opts();
        o.min_df = 50;
        let err = classify(SUPPORT, "invoice", &o).unwrap_err();
        assert!(err.contains("no terms survived min_df=50"), "{err}");
    }

    #[test]
    fn caps_are_enforced_at_the_boundary() {
        let mut o = opts();
        o.input_mode = "lines".into();
        let ok = "invoice\n".repeat(MAX_BATCH_LINES);
        assert!(classify(SUPPORT, &ok, &o).is_ok());
        let too_many = "invoice\n".repeat(MAX_BATCH_LINES + 1);
        let err = classify(SUPPORT, &too_many, &o).unwrap_err();
        assert!(err.contains("over the 1000-line batch limit"), "{err}");
    }

    #[test]
    fn top_k_zero_lists_every_label() {
        let mut o = opts();
        o.top_k = 0;
        let out = classify(SUPPORT, "invoice", &o).unwrap();
        for label in ["billing", "tech", "sales"] {
            assert!(out.contains(label), "{label} missing from {out}");
        }
    }

    #[test]
    fn ngrams_and_stopwords_and_sublinear_tf_compose() {
        let mut o = opts();
        o.ngram_max = 2;
        o.remove_stopwords = true;
        o.sublinear_tf = true;
        o.output = "json".into();
        let v: serde_json::Value =
            serde_json::from_str(&classify(SUPPORT, "the server timeout", &o).unwrap()).unwrap();
        assert_eq!(v["prediction"], "tech");
        assert_eq!(v["settings"]["ngram_max"], 2);
        assert_eq!(v["settings"]["sublinear_tf"], true);
    }

    #[test]
    fn output_is_deterministic() {
        let a = classify(SUPPORT, "invoice refund", &opts()).unwrap();
        let b = classify(SUPPORT, "invoice refund", &opts()).unwrap();
        assert_eq!(a, b);
    }
}
