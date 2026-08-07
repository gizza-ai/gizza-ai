//! naive-bayes-text-classifier core — pure compute, shared by the chat skill block and the web page.
//! Trains a multinomial / Bernoulli / complement naive Bayes classifier on pasted labeled
//! examples and classifies new text. No I/O, no randomness, fully deterministic.

use std::collections::{BTreeMap, HashMap};

/// Maximum size of the training block, in bytes.
pub const MAX_TRAINING_BYTES: usize = 1_048_576;
/// Maximum size of the text being classified, in bytes.
pub const MAX_TEXT_BYTES: usize = 262_144;
/// Maximum number of labeled training examples.
pub const MAX_EXAMPLES: usize = 20_000;
/// Maximum number of distinct labels.
pub const MAX_CLASSES: usize = 200;
/// Maximum vocabulary size after `min_count` filtering.
pub const MAX_VOCAB: usize = 200_000;
/// Maximum lines classified in one run when `input_mode=lines`.
pub const MAX_BATCH_LINES: usize = 1_000;
/// Maximum n-gram length.
pub const MAX_NGRAM: usize = 3;
/// Maximum accepted `min_count`.
pub const MAX_MIN_COUNT: usize = 100;
/// Maximum accepted `top_k`.
pub const MAX_TOP_K: usize = 50;
/// Maximum accepted smoothing alpha.
pub const MAX_ALPHA: f64 = 10.0;

/// Tokens listed in a single-document explanation.
const EXPLAIN_TOKENS: usize = 10;
/// Tokens listed per row in a batch explanation.
const EXPLAIN_TOKENS_BATCH: usize = 3;
/// Alpha is clipped up to this to keep every log finite (mirrors scikit-learn).
const ALPHA_FLOOR: f64 = 1e-10;
/// Batch rows show at most this many characters of the source line.
const BATCH_TEXT_WIDTH: usize = 60;

/// A conservative English stop-word list, applied before n-grams are formed.
const STOPWORDS: &[&str] = &[
    "a", "about", "above", "after", "again", "against", "all", "am", "an", "and", "any", "are",
    "aren't", "as", "at", "be", "because", "been", "before", "being", "below", "between", "both",
    "but", "by", "can", "can't", "cannot", "could", "couldn't", "did", "didn't", "do", "does",
    "doesn't", "doing", "don't", "down", "during", "each", "few", "for", "from", "further", "had",
    "hadn't", "has", "hasn't", "have", "haven't", "having", "he", "her", "here", "hers", "herself",
    "him", "himself", "his", "how", "i", "if", "in", "into", "is", "isn't", "it", "it's", "its",
    "itself", "just", "me", "more", "most", "must", "my", "myself", "no", "nor", "not", "now",
    "of", "off", "on", "once", "only", "or", "other", "ought", "our", "ours", "ourselves", "out",
    "over", "own", "same", "shan't", "she", "should", "shouldn't", "so", "some", "such", "than",
    "that", "the", "their", "theirs", "them", "themselves", "then", "there", "these", "they",
    "this", "those", "through", "to", "too", "under", "until", "up", "very", "was", "wasn't", "we",
    "were", "weren't", "what", "when", "where", "which", "while", "who", "whom", "why", "will",
    "with", "won't", "would", "wouldn't", "you", "your", "yours", "yourself", "yourselves",
];

/// Every knob the classifier exposes; mirrors the block descriptor 1:1.
#[derive(Clone, Debug)]
pub struct Options {
    /// `auto`, `tab`, `comma`, `pipe` or `colon` — how label and text are split.
    pub separator: String,
    /// `single` (one document) or `lines` (one document per non-empty line).
    pub input_mode: String,
    /// `multinomial`, `bernoulli` or `complement`.
    pub model: String,
    /// Additive (Lidstone/Laplace) smoothing.
    pub alpha: f64,
    /// Longest n-gram formed from the word sequence.
    pub ngram_max: usize,
    /// Lower-case everything before tokenizing.
    pub lowercase: bool,
    /// Drop English stop words before forming n-grams.
    pub remove_stopwords: bool,
    /// Drop vocabulary entries seen fewer than this many times in total.
    pub min_count: usize,
    /// `empirical` or `uniform` class priors.
    pub priors: String,
    /// How many classes to list (0 = all).
    pub top_k: usize,
    /// Include the tokens that drove the decision.
    pub explain: bool,
    /// `report` or `json`.
    pub output: String,
}

impl Default for Options {
    fn default() -> Self {
        Options {
            separator: "auto".into(),
            input_mode: "single".into(),
            model: "multinomial".into(),
            alpha: 1.0,
            ngram_max: 1,
            lowercase: true,
            remove_stopwords: false,
            min_count: 1,
            priors: "empirical".into(),
            top_k: 3,
            explain: true,
            output: "report".into(),
        }
    }
}

/// Train on `training_data` and classify `text`. Returns the rendered report or JSON.
pub fn classify(training_data: &str, text: &str, opts: &Options) -> Result<String, String> {
    let opts = validate(opts)?;
    if training_data.len() > MAX_TRAINING_BYTES {
        return Err(format!(
            "training data is {} bytes, over the {MAX_TRAINING_BYTES}-byte limit; trim the example set",
            training_data.len()
        ));
    }
    if text.len() > MAX_TEXT_BYTES {
        return Err(format!(
            "text is {} bytes, over the {MAX_TEXT_BYTES}-byte limit; classify it in smaller pieces",
            text.len()
        ));
    }

    let model = Model::train(training_data, &opts)?;

    let documents: Vec<String> = if opts.input_mode == "lines" {
        let rows: Vec<String> = text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(str::to_string)
            .collect();
        if rows.is_empty() {
            return Err(
                "text is empty; with input_mode=lines every non-blank line is classified separately"
                    .into(),
            );
        }
        if rows.len() > MAX_BATCH_LINES {
            return Err(format!(
                "text has {} non-blank lines, over the {MAX_BATCH_LINES}-line batch limit",
                rows.len()
            ));
        }
        rows
    } else {
        if text.trim().is_empty() {
            return Err("text is empty; paste the text you want to classify".into());
        }
        vec![text.to_string()]
    };

    let scored: Vec<Scored> = documents
        .iter()
        .map(|d| model.predict(d, &opts))
        .collect::<Vec<_>>();

    if opts.output == "json" {
        Ok(render_json(&model, &opts, &documents, &scored))
    } else if opts.input_mode == "lines" {
        Ok(render_batch_report(&model, &opts, &documents, &scored))
    } else {
        Ok(render_report(&model, &opts, &scored[0]))
    }
}

fn validate(o: &Options) -> Result<Options, String> {
    let mut out = o.clone();
    out.separator = enum_of(
        &o.separator,
        "separator",
        &["auto", "tab", "comma", "pipe", "colon"],
    )?;
    out.input_mode = enum_of(&o.input_mode, "input_mode", &["single", "lines"])?;
    out.model = enum_of(
        &o.model,
        "model",
        &["multinomial", "bernoulli", "complement"],
    )?;
    out.priors = enum_of(&o.priors, "priors", &["empirical", "uniform"])?;
    out.output = enum_of(&o.output, "output", &["report", "json"])?;
    if !o.alpha.is_finite() || o.alpha < 0.0 || o.alpha > MAX_ALPHA {
        return Err(format!(
            "alpha must be between 0 and {MAX_ALPHA}, got {}",
            o.alpha
        ));
    }
    if o.ngram_max < 1 || o.ngram_max > MAX_NGRAM {
        return Err(format!(
            "ngram_max must be between 1 and {MAX_NGRAM}, got {}",
            o.ngram_max
        ));
    }
    if o.min_count < 1 || o.min_count > MAX_MIN_COUNT {
        return Err(format!(
            "min_count must be between 1 and {MAX_MIN_COUNT}, got {}",
            o.min_count
        ));
    }
    if o.top_k > MAX_TOP_K {
        return Err(format!(
            "top_k must be between 0 and {MAX_TOP_K}, got {}",
            o.top_k
        ));
    }
    Ok(out)
}

fn enum_of(value: &str, name: &str, allowed: &[&str]) -> Result<String, String> {
    let v = value.trim().to_ascii_lowercase();
    if v.is_empty() {
        return Ok(allowed[0].to_string());
    }
    if allowed.contains(&v.as_str()) {
        Ok(v)
    } else {
        Err(format!(
            "{name} must be one of {}, got \"{value}\"",
            allowed.join(", ")
        ))
    }
}

// ---------------------------------------------------------------------------
// tokenizing
// ---------------------------------------------------------------------------

fn words(text: &str, lowercase: bool) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let flush = |cur: &mut String, out: &mut Vec<String>| {
        let w = cur.trim_matches(|c| c == '\'' || c == '_');
        if !w.is_empty() {
            out.push(w.to_string());
        }
        cur.clear();
    };
    for ch in text.chars() {
        if ch.is_alphanumeric() || ch == '\'' || ch == '_' {
            if lowercase {
                cur.extend(ch.to_lowercase());
            } else {
                cur.push(ch);
            }
        } else {
            flush(&mut cur, &mut out);
        }
    }
    flush(&mut cur, &mut out);
    out
}

fn tokenize(text: &str, o: &Options) -> Vec<String> {
    let mut ws = words(text, o.lowercase);
    if o.remove_stopwords {
        ws.retain(|w| !STOPWORDS.contains(&w.to_lowercase().as_str()));
    }
    let n = o.ngram_max;
    let mut out = Vec::new();
    for k in 1..=n {
        if ws.len() < k {
            break;
        }
        for i in 0..=(ws.len() - k) {
            out.push(ws[i..i + k].join(" "));
        }
    }
    out
}

// ---------------------------------------------------------------------------
// training
// ---------------------------------------------------------------------------

struct Model {
    classes: Vec<String>,
    doc_counts: Vec<usize>,
    /// per class: token index -> occurrences
    counts: Vec<HashMap<usize, f64>>,
    /// per class: token index -> number of examples containing it
    doc_freq: Vec<HashMap<usize, f64>>,
    /// per class: total token occurrences
    class_tokens: Vec<f64>,
    /// per class: constant part of the score (prior, plus Bernoulli absent-term mass)
    base: Vec<f64>,
    vocab: Vec<String>,
    index: HashMap<String, usize>,
    /// corpus-wide occurrences per token index (used by complement NB)
    global_counts: Vec<f64>,
    global_tokens: f64,
    total_docs: usize,
    separator: char,
    separator_auto: bool,
    empty_examples: usize,
    dropped_vocab: usize,
    alpha: f64,
    kind: String,
}

impl Model {
    fn train(training_data: &str, o: &Options) -> Result<Model, String> {
        let raw: Vec<(usize, &str)> = training_data
            .lines()
            .enumerate()
            .map(|(i, l)| (i + 1, l))
            .filter(|(_, l)| !l.trim().is_empty())
            .collect();
        if raw.is_empty() {
            return Err(
                "training data is empty; add labeled examples, one per line, as `label<TAB>text`"
                    .into(),
            );
        }
        if raw.len() > MAX_EXAMPLES {
            return Err(format!(
                "training data has {} examples, over the {MAX_EXAMPLES}-example limit",
                raw.len()
            ));
        }

        let (sep, sep_auto) = resolve_separator(&o.separator, &raw)?;
        let sep_name = separator_name(sep);

        let mut examples: Vec<(String, Vec<String>)> = Vec::with_capacity(raw.len());
        let mut empty_examples = 0usize;
        for (n, line) in &raw {
            let (label_raw, text_raw) = line.split_once(sep).ok_or_else(|| {
                format!(
                    "training line {n} has no {sep_name} separator: \"{}\" — expected `label{}text`",
                    truncate(line.trim(), 60),
                    sep_display(sep)
                )
            })?;
            let label = unquote(label_raw.trim());
            if label.is_empty() {
                return Err(format!(
                    "training line {n} has an empty label before the {sep_name}: \"{}\"",
                    truncate(line.trim(), 60)
                ));
            }
            let body = unquote(text_raw.trim());
            if body.is_empty() {
                return Err(format!(
                    "training line {n} has a label (\"{label}\") but no example text after the {sep_name}"
                ));
            }
            let toks = tokenize(&body, o);
            if toks.is_empty() {
                empty_examples += 1;
            }
            examples.push((label.to_string(), toks));
        }

        let mut labels: Vec<String> = examples.iter().map(|(l, _)| l.clone()).collect();
        labels.sort();
        labels.dedup();
        if labels.len() < 2 {
            return Err(format!(
                "training data needs at least 2 distinct labels; found only 1 (\"{}\") across {} examples",
                labels[0],
                examples.len()
            ));
        }
        if labels.len() > MAX_CLASSES {
            return Err(format!(
                "training data has {} distinct labels, over the {MAX_CLASSES}-class limit",
                labels.len()
            ));
        }
        let class_of: HashMap<String, usize> = labels
            .iter()
            .enumerate()
            .map(|(i, l)| (l.clone(), i))
            .collect();

        // Corpus-wide totals first, so min_count can prune before the vocabulary is fixed.
        let mut totals: BTreeMap<&str, f64> = BTreeMap::new();
        for (_, toks) in &examples {
            for t in toks {
                *totals.entry(t.as_str()).or_insert(0.0) += 1.0;
            }
        }
        let before = totals.len();
        let mut vocab: Vec<String> = totals
            .iter()
            .filter(|(_, c)| **c >= o.min_count as f64)
            .map(|(t, _)| (*t).to_string())
            .collect();
        vocab.sort();
        if vocab.is_empty() {
            return Err(format!(
                "no vocabulary left: every one of the {before} distinct tokens occurs fewer than min_count={} times; lower min_count",
                o.min_count
            ));
        }
        if vocab.len() > MAX_VOCAB {
            return Err(format!(
                "vocabulary is {} tokens, over the {MAX_VOCAB}-token limit; raise min_count or lower ngram_max",
                vocab.len()
            ));
        }
        let dropped_vocab = before - vocab.len();
        let index: HashMap<String, usize> = vocab
            .iter()
            .enumerate()
            .map(|(i, t)| (t.clone(), i))
            .collect();

        let k = labels.len();
        let mut m = Model {
            classes: labels,
            doc_counts: vec![0; k],
            counts: vec![HashMap::new(); k],
            doc_freq: vec![HashMap::new(); k],
            class_tokens: vec![0.0; k],
            base: vec![0.0; k],
            global_counts: vec![0.0; vocab.len()],
            global_tokens: 0.0,
            vocab,
            index,
            total_docs: examples.len(),
            separator: sep,
            separator_auto: sep_auto,
            empty_examples,
            dropped_vocab,
            alpha: o.alpha.max(ALPHA_FLOOR),
            kind: o.model.clone(),
        };

        for (label, toks) in &examples {
            let c = class_of[label];
            m.doc_counts[c] += 1;
            let mut seen: Vec<usize> = Vec::new();
            for t in toks {
                if let Some(&ti) = m.index.get(t) {
                    *m.counts[c].entry(ti).or_insert(0.0) += 1.0;
                    m.class_tokens[c] += 1.0;
                    m.global_counts[ti] += 1.0;
                    m.global_tokens += 1.0;
                    seen.push(ti);
                }
            }
            seen.sort_unstable();
            seen.dedup();
            for ti in seen {
                *m.doc_freq[c].entry(ti).or_insert(0.0) += 1.0;
            }
        }

        m.compute_base(o);
        Ok(m)
    }

    fn log_prior(&self, c: usize, o: &Options) -> f64 {
        match o.priors.as_str() {
            "uniform" => -(self.classes.len() as f64).ln(),
            _ => (self.doc_counts[c] as f64 / self.total_docs as f64).ln(),
        }
    }

    fn compute_base(&mut self, o: &Options) {
        let a = self.alpha;
        for c in 0..self.classes.len() {
            self.base[c] = match self.kind.as_str() {
                // Complement NB scores from complement-class weights only; scikit-learn
                // likewise drops the prior for more than one class.
                "complement" => 0.0,
                "bernoulli" => {
                    let n = self.doc_counts[c] as f64;
                    let mut absent = 0.0;
                    for t in 0..self.vocab.len() {
                        let df = self.doc_freq[c].get(&t).copied().unwrap_or(0.0);
                        let theta = (df + a) / (n + 2.0 * a);
                        absent += (1.0 - theta).ln();
                    }
                    self.log_prior(c, o) + absent
                }
                _ => self.log_prior(c, o),
            };
        }
    }

    /// Per-class weight of one vocabulary token, in the units the model scores in.
    fn weight(&self, c: usize, t: usize) -> f64 {
        let a = self.alpha;
        match self.kind.as_str() {
            "bernoulli" => {
                let n = self.doc_counts[c] as f64;
                let df = self.doc_freq[c].get(&t).copied().unwrap_or(0.0);
                let theta = (df + a) / (n + 2.0 * a);
                theta.ln() - (1.0 - theta).ln()
            }
            "complement" => {
                let comp = a + (self.global_counts[t] - self.counts[c].get(&t).copied().unwrap_or(0.0));
                let denom = a * self.vocab.len() as f64 + (self.global_tokens - self.class_tokens[c]);
                -(comp / denom).ln()
            }
            _ => {
                let n = self.counts[c].get(&t).copied().unwrap_or(0.0);
                ((n + a) / (self.class_tokens[c] + a * self.vocab.len() as f64)).ln()
            }
        }
    }

    fn predict(&self, doc: &str, o: &Options) -> Scored {
        let toks = tokenize(doc, o);
        let binary = self.kind == "bernoulli";
        let mut feats: BTreeMap<usize, f64> = BTreeMap::new();
        let mut known = 0usize;
        for t in &toks {
            if let Some(&ti) = self.index.get(t) {
                known += 1;
                let e = feats.entry(ti).or_insert(0.0);
                *e = if binary { 1.0 } else { *e + 1.0 };
            }
        }

        let mut scores = self.base.clone();
        for (&ti, &x) in &feats {
            for (c, s) in scores.iter_mut().enumerate() {
                *s += x * self.weight(c, ti);
            }
        }

        let lse = log_sum_exp(&scores);
        let probs: Vec<f64> = scores.iter().map(|s| (s - lse).exp()).collect();

        let mut order: Vec<usize> = (0..self.classes.len()).collect();
        order.sort_by(|&a, &b| {
            scores[b]
                .partial_cmp(&scores[a])
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| self.classes[a].cmp(&self.classes[b]))
        });

        let top = order[0];
        let runner = order[1];
        let mut explain: Vec<(String, f64)> = feats
            .iter()
            .map(|(&ti, &x)| {
                (
                    self.vocab[ti].clone(),
                    x * (self.weight(top, ti) - self.weight(runner, ti)),
                )
            })
            .filter(|(_, w)| *w > 0.0)
            .collect();
        explain.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.0.cmp(&b.0))
        });

        Scored {
            scores,
            probs,
            order,
            tokens_total: toks.len(),
            tokens_known: known,
            explain,
        }
    }
}

struct Scored {
    scores: Vec<f64>,
    probs: Vec<f64>,
    order: Vec<usize>,
    tokens_total: usize,
    tokens_known: usize,
    explain: Vec<(String, f64)>,
}

fn log_sum_exp(v: &[f64]) -> f64 {
    let max = v.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
    if !max.is_finite() {
        return max;
    }
    max + v.iter().map(|s| (s - max).exp()).sum::<f64>().ln()
}

// ---------------------------------------------------------------------------
// separator handling
// ---------------------------------------------------------------------------

const SEPARATORS: [(&str, char); 4] = [("tab", '\t'), ("comma", ','), ("pipe", '|'), ("colon", ':')];

fn resolve_separator(spec: &str, lines: &[(usize, &str)]) -> Result<(char, bool), String> {
    if spec != "auto" {
        let ch = SEPARATORS
            .iter()
            .find(|(n, _)| *n == spec)
            .map(|(_, c)| *c)
            .ok_or_else(|| format!("separator must be auto, tab, comma, pipe or colon, got \"{spec}\""))?;
        return Ok((ch, false));
    }
    let mut best: Option<(usize, char)> = None;
    for (_, ch) in SEPARATORS {
        let hits = lines.iter().filter(|(_, l)| l.contains(ch)).count();
        if hits > best.map(|(h, _)| h).unwrap_or(0) {
            best = Some((hits, ch));
        }
    }
    match best {
        Some((_, ch)) => Ok((ch, true)),
        None => Err(format!(
            "could not find a label separator in the training data; each line must look like `label<TAB>text` (a comma, pipe or colon also works). First line was: \"{}\"",
            truncate(lines[0].1.trim(), 60)
        )),
    }
}

fn separator_name(ch: char) -> &'static str {
    SEPARATORS
        .iter()
        .find(|(_, c)| *c == ch)
        .map(|(n, _)| *n)
        .unwrap_or("separator")
}

fn sep_display(ch: char) -> &'static str {
    match ch {
        '\t' => "<TAB>",
        ',' => ",",
        '|' => "|",
        _ => ":",
    }
}

fn unquote(s: &str) -> &str {
    let b = s.as_bytes();
    if b.len() >= 2 && b[0] == b'"' && b[b.len() - 1] == b'"' {
        &s[1..s.len() - 1]
    } else {
        s
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let head: String = s.chars().take(max.saturating_sub(3)).collect();
        format!("{head}...")
    }
}

// ---------------------------------------------------------------------------
// rendering
// ---------------------------------------------------------------------------

fn pad(s: &str, w: usize) -> String {
    let n = s.chars().count();
    format!("{s}{}", " ".repeat(w.saturating_sub(n)))
}

fn lpad(s: &str, w: usize) -> String {
    let n = s.chars().count();
    format!("{}{s}", " ".repeat(w.saturating_sub(n)))
}

fn pct(p: f64) -> String {
    format!("{:.1}%", p * 100.0)
}

fn on_off(b: bool) -> &'static str {
    if b {
        "on"
    } else {
        "off"
    }
}

fn model_name(kind: &str) -> &'static str {
    match kind {
        "bernoulli" => "Bernoulli naive Bayes",
        "complement" => "complement naive Bayes",
        _ => "multinomial naive Bayes",
    }
}

fn shown_classes<'a>(m: &Model, s: &'a Scored, o: &Options) -> &'a [usize] {
    let k = if o.top_k == 0 {
        m.classes.len()
    } else {
        o.top_k.min(m.classes.len())
    };
    &s.order[..k]
}

fn notes(m: &Model, o: &Options, scored: &[Scored]) -> Vec<String> {
    let mut out = Vec::new();
    let unknown: usize = scored
        .iter()
        .map(|s| s.tokens_total - s.tokens_known)
        .sum();
    let total: usize = scored.iter().map(|s| s.tokens_total).sum();
    if unknown > 0 {
        out.push(format!(
            "{unknown} of {total} input tokens were never seen in training and were ignored."
        ));
    }
    if scored.iter().any(|s| s.tokens_known == 0) {
        out.push(
            "At least one document shares no vocabulary with the training set, so it was decided by the class priors alone.".into(),
        );
    }
    if m.dropped_vocab > 0 {
        out.push(format!(
            "min_count={} removed {} rare tokens from the vocabulary.",
            o.min_count, m.dropped_vocab
        ));
    }
    if m.empty_examples > 0 {
        out.push(format!(
            "{} training example(s) produced no tokens (they still count towards the class priors).",
            m.empty_examples
        ));
    }
    if o.alpha < ALPHA_FLOOR {
        out.push(format!(
            "alpha was raised to {ALPHA_FLOOR} so no probability becomes exactly zero."
        ));
    }
    if m.kind == "complement" {
        out.push(
            "Complement naive Bayes scores from complement-class weights and ignores class priors; the percentages are normalised scores, not calibrated probabilities.".into(),
        );
    }
    if m.total_docs < 2 * m.classes.len() {
        out.push(format!(
            "Only {} examples for {} classes — add more examples per class for a stable model.",
            m.total_docs,
            m.classes.len()
        ));
    }
    out
}

fn model_section(m: &Model, o: &Options) -> String {
    let rows = [
        ("algorithm", model_name(&m.kind).to_string()),
        ("smoothing alpha", format!("{}", o.alpha)),
        ("n-grams", format!("1..{}", o.ngram_max)),
        ("lowercase", on_off(o.lowercase).to_string()),
        ("stopwords removed", on_off(o.remove_stopwords).to_string()),
        ("min token count", format!("{}", o.min_count)),
        ("class priors", o.priors.clone()),
    ];
    let w = rows.iter().map(|(k, _)| k.len()).max().unwrap_or(0);
    let mut s = String::from("Model\n");
    for (k, v) in rows {
        s.push_str(&format!("  {}  {v}\n", pad(k, w)));
    }
    s
}

fn training_section(m: &Model) -> String {
    let sep = format!(
        "{}{}",
        separator_name(m.separator),
        if m.separator_auto {
            " (auto-detected)"
        } else {
            ""
        }
    );
    let mut rows = vec![
        ("separator".to_string(), sep),
        ("examples".to_string(), m.total_docs.to_string()),
        ("classes".to_string(), m.classes.len().to_string()),
        (
            "vocabulary".to_string(),
            format!("{} tokens", m.vocab.len()),
        ),
    ];
    for (c, label) in m.classes.iter().enumerate() {
        rows.push((
            label.clone(),
            format!(
                "{} examples, {} tokens",
                m.doc_counts[c], m.class_tokens[c] as u64
            ),
        ));
    }
    let w = rows.iter().map(|(k, _)| k.chars().count()).max().unwrap_or(0);
    let mut s = String::from("Training data\n");
    for (k, v) in rows {
        s.push_str(&format!("  {}  {v}\n", pad(&k, w)));
    }
    s
}

fn notes_section(list: &[String]) -> String {
    if list.is_empty() {
        return String::new();
    }
    let mut s = String::from("\nNotes\n");
    for n in list {
        s.push_str(&format!("  - {n}\n"));
    }
    s
}

fn render_report(m: &Model, o: &Options, s: &Scored) -> String {
    let top = s.order[0];
    let mut out = format!(
        "Prediction: {}\nConfidence: {}\n\n",
        m.classes[top],
        pct(s.probs[top])
    );

    let shown = shown_classes(m, s, o);
    let lw = shown
        .iter()
        .map(|&c| m.classes[c].chars().count())
        .max()
        .unwrap_or(5)
        .max("class".len());
    out.push_str(&format!(
        "Class scores\n  {}  probability   log score\n",
        pad("class", lw)
    ));
    for &c in shown {
        out.push_str(&format!(
            "  {}  {}  {}\n",
            pad(&m.classes[c], lw),
            lpad(&pct(s.probs[c]), 11),
            lpad(&format!("{:.4}", s.scores[c]), 10)
        ));
    }
    if shown.len() < m.classes.len() {
        out.push_str(&format!(
            "  ({} more class(es) not shown; raise top_k)\n",
            m.classes.len() - shown.len()
        ));
    }

    if o.explain {
        let runner = s.order[1];
        out.push_str(&format!(
            "\nTop tokens for \"{}\" over \"{}\"\n",
            m.classes[top], m.classes[runner]
        ));
        if s.explain.is_empty() {
            out.push_str("  (no token favours the prediction; it came from the class priors)\n");
        } else {
            let tw = s
                .explain
                .iter()
                .take(EXPLAIN_TOKENS)
                .map(|(t, _)| t.chars().count())
                .max()
                .unwrap_or(0);
            for (t, w) in s.explain.iter().take(EXPLAIN_TOKENS) {
                out.push_str(&format!("  {}  {}\n", pad(t, tw), lpad(&format!("+{w:.4}"), 9)));
            }
        }
    }

    out.push_str(&format!(
        "\nInput\n  tokens            {}\n  seen in training  {}\n\n",
        s.tokens_total, s.tokens_known
    ));
    out.push_str(&model_section(m, o));
    out.push('\n');
    out.push_str(&training_section(m));
    out.push_str(&notes_section(&notes(m, o, std::slice::from_ref(s))));
    out
}

fn render_batch_report(m: &Model, o: &Options, docs: &[String], scored: &[Scored]) -> String {
    let idx_w = docs.len().to_string().len().max(1);
    let lw = scored
        .iter()
        .map(|s| m.classes[s.order[0]].chars().count())
        .max()
        .unwrap_or(10)
        .max("prediction".len());
    let tokens: Vec<String> = scored
        .iter()
        .map(|s| {
            s.explain
                .iter()
                .take(EXPLAIN_TOKENS_BATCH)
                .map(|(t, _)| t.clone())
                .collect::<Vec<_>>()
                .join(", ")
        })
        .collect();
    let tw = tokens
        .iter()
        .map(|t| t.chars().count())
        .max()
        .unwrap_or(0)
        .max("top tokens".len());

    let mut out = format!(
        "Classified {} line(s) with {}.\n\n",
        docs.len(),
        model_name(&m.kind)
    );
    out.push_str(&format!("  {}  {}  confidence", lpad("#", idx_w), pad("prediction", lw)));
    if o.explain {
        out.push_str(&format!("  {}", pad("top tokens", tw)));
    }
    out.push_str("  text\n");
    for (i, (d, s)) in docs.iter().zip(scored).enumerate() {
        let top = s.order[0];
        out.push_str(&format!(
            "  {}  {}  {}",
            lpad(&(i + 1).to_string(), idx_w),
            pad(&m.classes[top], lw),
            lpad(&pct(s.probs[top]), 10)
        ));
        if o.explain {
            out.push_str(&format!("  {}", pad(&tokens[i], tw)));
        }
        out.push_str(&format!("  {}\n", truncate(d.trim(), BATCH_TEXT_WIDTH)));
    }

    out.push('\n');
    out.push_str(&model_section(m, o));
    out.push('\n');
    out.push_str(&training_section(m));
    out.push_str(&notes_section(&notes(m, o, scored)));
    out
}

fn render_json(m: &Model, o: &Options, docs: &[String], scored: &[Scored]) -> String {
    let round = |v: f64, places: i32| {
        let f = 10f64.powi(places);
        (v * f).round() / f
    };
    let result_of = |s: &Scored| {
        let top = s.order[0];
        let runner = s.order[1];
        let classes: Vec<serde_json::Value> = shown_classes(m, s, o)
            .iter()
            .map(|&c| {
                serde_json::json!({
                    "label": m.classes[c],
                    "probability": round(s.probs[c], 6),
                    "score": round(s.scores[c], 6),
                })
            })
            .collect();
        let mut v = serde_json::json!({
            "prediction": m.classes[top],
            "confidence": round(s.probs[top], 6),
            "classes": classes,
            "tokens": s.tokens_total,
            "tokens_seen_in_training": s.tokens_known,
        });
        if o.explain {
            v["explanation"] = serde_json::json!({
                "against": m.classes[runner],
                "tokens": s.explain.iter().take(EXPLAIN_TOKENS).map(|(t, w)| serde_json::json!({
                    "token": t, "weight": round(*w, 6)
                })).collect::<Vec<_>>(),
            });
        }
        v
    };

    let per_class: Vec<serde_json::Value> = m
        .classes
        .iter()
        .enumerate()
        .map(|(c, label)| {
            serde_json::json!({
                "label": label,
                "examples": m.doc_counts[c],
                "tokens": m.class_tokens[c] as u64,
            })
        })
        .collect();

    let mut root = serde_json::json!({
        "model": {
            "algorithm": m.kind,
            "alpha": o.alpha,
            "ngram_max": o.ngram_max,
            "lowercase": o.lowercase,
            "remove_stopwords": o.remove_stopwords,
            "min_count": o.min_count,
            "priors": o.priors,
        },
        "training": {
            "separator": separator_name(m.separator),
            "separator_auto_detected": m.separator_auto,
            "examples": m.total_docs,
            "classes": m.classes.len(),
            "vocabulary": m.vocab.len(),
            "per_class": per_class,
        },
        "notes": notes(m, o, scored),
    });

    if o.input_mode == "lines" {
        root["results"] = serde_json::Value::Array(
            docs.iter()
                .zip(scored)
                .map(|(d, s)| {
                    let mut v = result_of(s);
                    v["text"] = serde_json::Value::String(d.trim().to_string());
                    v
                })
                .collect(),
        );
    } else {
        let v = result_of(&scored[0]);
        for (k, val) in v.as_object().unwrap() {
            root[k] = val.clone();
        }
    }
    serde_json::to_string_pretty(&root).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
}

// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    const TRAIN: &str = "spam,win a free prize now\nspam,free money click here\nspam,claim your free gift\nham,meeting at ten tomorrow\nham,lunch with the team\nham,project update attached";

    fn opts() -> Options {
        Options::default()
    }

    #[test]
    fn classifies_a_spam_like_message() {
        let out = classify(TRAIN, "claim your free money now", &opts()).unwrap();
        assert!(out.starts_with("Prediction: spam\n"), "{out}");
        assert!(out.contains("Class scores"));
        assert!(out.contains("multinomial naive Bayes"));
    }

    #[test]
    fn classifies_a_ham_like_message() {
        let out = classify(TRAIN, "team lunch meeting tomorrow", &opts()).unwrap();
        assert!(out.starts_with("Prediction: ham\n"), "{out}");
    }

    #[test]
    fn rejects_a_single_label() {
        let err = classify("spam,buy now\nspam,free stuff", "hello", &opts()).unwrap_err();
        assert!(err.contains("at least 2 distinct labels"), "{err}");
        assert!(err.contains("spam"), "{err}");
    }

    #[test]
    fn rejects_empty_training_data() {
        let err = classify("   \n\n", "hello", &opts()).unwrap_err();
        assert!(err.contains("training data is empty"), "{err}");
    }

    #[test]
    fn rejects_empty_text() {
        let err = classify(TRAIN, "   ", &opts()).unwrap_err();
        assert!(err.contains("text is empty"), "{err}");
    }

    #[test]
    fn reports_a_line_without_a_separator() {
        let train = "spam\tbuy now\nham lunch today\nspam\tfree gift";
        let err = classify(train, "hello", &opts()).unwrap_err();
        assert!(err.contains("training line 2"), "{err}");
        assert!(err.contains("no tab separator"), "{err}");
    }

    #[test]
    fn reports_an_empty_label() {
        let err = classify("spam,buy now\n,lunch today", "hi", &opts()).unwrap_err();
        assert!(err.contains("training line 2"), "{err}");
        assert!(err.contains("empty label"), "{err}");
    }

    #[test]
    fn reports_a_missing_example_body() {
        let err = classify("spam,buy now\nham,", "hi", &opts()).unwrap_err();
        assert!(err.contains("training line 2"), "{err}");
        assert!(err.contains("no example text"), "{err}");
    }

    #[test]
    fn rejects_a_bad_enum_value() {
        let mut o = opts();
        o.model = "svm".into();
        let err = classify(TRAIN, "hello", &o).unwrap_err();
        assert!(err.contains("model must be one of"), "{err}");
    }

    #[test]
    fn rejects_alpha_out_of_range() {
        let mut o = opts();
        o.alpha = 25.0;
        let err = classify(TRAIN, "hello", &o).unwrap_err();
        assert!(err.contains("alpha must be between 0 and 10"), "{err}");
    }

    #[test]
    fn auto_detects_tabs_and_pipes() {
        for (sep, name) in [("\t", "tab"), ("|", "pipe"), (":", "colon")] {
            let train = TRAIN.replace(',', sep);
            let out = classify(&train, "free money", &opts()).unwrap();
            assert!(out.contains(&format!("{name} (auto-detected)")), "{out}");
            assert!(out.starts_with("Prediction: spam"), "{out}");
        }
    }

    #[test]
    fn explicit_separator_is_not_marked_auto() {
        let mut o = opts();
        o.separator = "comma".into();
        let out = classify(TRAIN, "free gift", &o).unwrap();
        assert!(out.contains("separator   comma\n"), "{out}");
        assert!(!out.contains("auto-detected"), "{out}");
    }

    #[test]
    fn every_model_runs_and_agrees_on_an_obvious_case() {
        for kind in ["multinomial", "bernoulli", "complement"] {
            let mut o = opts();
            o.model = kind.into();
            let out = classify(TRAIN, "free free free money", &o).unwrap();
            assert!(out.starts_with("Prediction: spam"), "{kind}: {out}");
            assert!(out.contains(model_name(kind)), "{kind}: {out}");
        }
    }

    #[test]
    fn multinomial_probabilities_match_the_closed_form() {
        // Two classes, one token each, alpha = 1, empirical priors 1/2.
        // vocab = {a, b}; class A: "a" (n_a=1, total=1); class B: "b".
        // P(a|A) = (1+1)/(1+2) = 2/3 ; P(a|B) = (0+1)/(1+2) = 1/3.
        // score_A = ln(0.5) + ln(2/3), score_B = ln(0.5) + ln(1/3) → P(A) = 2/3.
        let mut o = opts();
        o.output = "json".into();
        o.explain = false;
        let out = classify("A,a\nB,b", "a", &o).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["prediction"], "A");
        let p = v["confidence"].as_f64().unwrap();
        assert!((p - 2.0 / 3.0).abs() < 1e-6, "{p}");
    }

    #[test]
    fn alpha_changes_the_confidence() {
        let mut o = opts();
        o.output = "json".into();
        o.alpha = 0.1;
        let sharp: serde_json::Value =
            serde_json::from_str(&classify("A,a\nB,b", "a", &o).unwrap()).unwrap();
        o.alpha = 5.0;
        let soft: serde_json::Value =
            serde_json::from_str(&classify("A,a\nB,b", "a", &o).unwrap()).unwrap();
        assert!(
            sharp["confidence"].as_f64().unwrap() > soft["confidence"].as_f64().unwrap(),
            "less smoothing must be more confident"
        );
    }

    #[test]
    fn uniform_priors_ignore_class_imbalance() {
        let train = "ham,alpha\nham,alpha\nham,alpha\nham,alpha\nspam,beta";
        let mut o = opts();
        o.output = "json".into();
        o.priors = "empirical".into();
        let a: serde_json::Value =
            serde_json::from_str(&classify(train, "gamma", &o).unwrap()).unwrap();
        assert_eq!(a["prediction"], "ham");
        assert!(a["confidence"].as_f64().unwrap() > 0.6);
        o.priors = "uniform".into();
        let b: serde_json::Value =
            serde_json::from_str(&classify(train, "gamma", &o).unwrap()).unwrap();
        assert!(
            (b["confidence"].as_f64().unwrap() - 0.5).abs() < 1e-9,
            "unseen text under uniform priors must be a tie: {b}"
        );
    }

    #[test]
    fn bigrams_enter_the_vocabulary() {
        let mut o = opts();
        o.ngram_max = 2;
        o.output = "json".into();
        let v: serde_json::Value =
            serde_json::from_str(&classify("A,red car\nB,blue sky", "x", &o).unwrap()).unwrap();
        // unigrams red, car, blue, sky + bigrams "red car", "blue sky"
        assert_eq!(v["training"]["vocabulary"], 6);
    }

    #[test]
    fn stopword_removal_shrinks_the_vocabulary() {
        let mut o = opts();
        o.output = "json".into();
        let with: serde_json::Value =
            serde_json::from_str(&classify("A,the red car\nB,the blue sky", "x", &o).unwrap())
                .unwrap();
        o.remove_stopwords = true;
        let without: serde_json::Value =
            serde_json::from_str(&classify("A,the red car\nB,the blue sky", "x", &o).unwrap())
                .unwrap();
        assert_eq!(with["training"]["vocabulary"], 5);
        assert_eq!(without["training"]["vocabulary"], 4);
    }

    #[test]
    fn min_count_prunes_rare_tokens() {
        let mut o = opts();
        o.output = "json".into();
        o.min_count = 2;
        let v: serde_json::Value =
            serde_json::from_str(&classify("A,red red car\nB,blue sky", "red", &o).unwrap())
                .unwrap();
        assert_eq!(v["training"]["vocabulary"], 1);
        assert!(v["notes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|n| n.as_str().unwrap().contains("min_count=2")));
    }

    #[test]
    fn min_count_that_removes_everything_is_an_error() {
        let mut o = opts();
        o.min_count = 9;
        let err = classify("A,red\nB,blue", "red", &o).unwrap_err();
        assert!(err.contains("no vocabulary left"), "{err}");
        assert!(err.contains("min_count=9"), "{err}");
    }

    #[test]
    fn case_folding_can_be_turned_off() {
        let mut o = opts();
        o.output = "json".into();
        o.lowercase = false;
        let v: serde_json::Value =
            serde_json::from_str(&classify("A,Red red\nB,blue", "Red", &o).unwrap()).unwrap();
        assert_eq!(v["training"]["vocabulary"], 3);
    }

    #[test]
    fn top_k_limits_the_class_list() {
        let train = "A,alpha\nB,beta\nC,gamma\nD,delta";
        let mut o = opts();
        o.top_k = 2;
        let out = classify(train, "alpha", &o).unwrap();
        assert!(out.contains("2 more class(es) not shown"), "{out}");
        o.top_k = 0;
        let all = classify(train, "alpha", &o).unwrap();
        assert!(!all.contains("not shown"), "{all}");
    }

    #[test]
    fn batch_mode_classifies_each_line() {
        let mut o = opts();
        o.input_mode = "lines".into();
        let out = classify(TRAIN, "free money now\nteam meeting tomorrow", &o).unwrap();
        assert!(out.starts_with("Classified 2 line(s) with multinomial naive Bayes.\n"), "{out}");
        assert!(out.contains("1  spam"), "{out}");
        assert!(out.contains("2  ham"), "{out}");
    }

    #[test]
    fn batch_json_returns_one_result_per_line() {
        let mut o = opts();
        o.input_mode = "lines".into();
        o.output = "json".into();
        let v: serde_json::Value =
            serde_json::from_str(&classify(TRAIN, "free gift\nproject update", &o).unwrap())
                .unwrap();
        let rows = v["results"].as_array().unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["prediction"], "spam");
        assert_eq!(rows[0]["text"], "free gift");
        assert_eq!(rows[1]["prediction"], "ham");
    }

    #[test]
    fn batch_mode_needs_at_least_one_line() {
        let mut o = opts();
        o.input_mode = "lines".into();
        let err = classify(TRAIN, "\n  \n", &o).unwrap_err();
        assert!(err.contains("input_mode=lines"), "{err}");
    }

    #[test]
    fn explanation_lists_the_deciding_tokens() {
        let out = classify(TRAIN, "free free prize", &opts()).unwrap();
        assert!(out.contains("Top tokens for \"spam\" over \"ham\""), "{out}");
        assert!(out.contains("free"), "{out}");
    }

    #[test]
    fn explanation_can_be_switched_off() {
        let mut o = opts();
        o.explain = false;
        let out = classify(TRAIN, "free prize", &o).unwrap();
        assert!(!out.contains("Top tokens"), "{out}");
    }

    #[test]
    fn unknown_tokens_are_reported() {
        let out = classify(TRAIN, "zzz qqq", &opts()).unwrap();
        assert!(out.contains("never seen in training"), "{out}");
        assert!(out.contains("class priors alone"), "{out}");
    }

    #[test]
    fn quoted_csv_fields_are_unwrapped() {
        let out = classify("\"spam\",\"free money\"\n\"ham\",\"team lunch\"", "free", &opts())
            .unwrap();
        assert!(out.starts_with("Prediction: spam"), "{out}");
        assert!(out.contains("  spam       "), "{out}");
    }

    #[test]
    fn the_first_separator_wins_so_text_may_contain_more() {
        let mut o = opts();
        o.separator = "comma".into();
        o.output = "json".into();
        let v: serde_json::Value =
            serde_json::from_str(&classify("A,one, two, three\nB,four", "two", &o).unwrap())
                .unwrap();
        assert_eq!(v["training"]["classes"], 2);
        assert_eq!(v["prediction"], "A");
    }

    #[test]
    fn training_data_over_the_byte_cap_is_rejected() {
        let big = "A,".to_string() + &"x".repeat(MAX_TRAINING_BYTES);
        let err = classify(&big, "x", &opts()).unwrap_err();
        assert!(err.contains("over the"), "{err}");
        assert!(err.contains("byte limit"), "{err}");
    }

    #[test]
    fn unicode_text_tokenizes() {
        let out = classify("de,guten tag freund\nfr,bonjour mon ami", "bonjour ami", &opts())
            .unwrap();
        assert!(out.starts_with("Prediction: fr"), "{out}");
    }

    #[test]
    fn json_output_carries_the_full_model_description() {
        let mut o = opts();
        o.output = "json".into();
        let v: serde_json::Value = serde_json::from_str(&classify(TRAIN, "free gift", &o).unwrap())
            .unwrap();
        assert_eq!(v["prediction"], "spam");
        assert_eq!(v["model"]["algorithm"], "multinomial");
        assert_eq!(v["model"]["alpha"], 1.0);
        assert_eq!(v["training"]["separator"], "comma");
        assert_eq!(v["training"]["separator_auto_detected"], true);
        assert_eq!(v["training"]["examples"], 6);
        assert_eq!(v["training"]["per_class"][0]["label"], "ham");
        assert_eq!(v["explanation"]["against"], "ham");
    }
}
