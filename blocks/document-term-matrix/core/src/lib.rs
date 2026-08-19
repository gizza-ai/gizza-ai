//! gizza-ai/document-term-matrix core — turn a collection of documents into a
//! document-term matrix (bag-of-words counts, or binary presence) and export it
//! as CSV, TSV or JSON. Pure Rust, deterministic, no I/O.
//!
//! Documents arrive either as a JSON array of strings or as one document per
//! line. Each document is tokenized into words (Unicode alphanumerics, with
//! apostrophes and hyphens kept *inside* a term), optionally case-folded, and
//! expanded into n-grams. The vocabulary is ordered by descending document
//! frequency then term ascending, trimmed by `min_df` and `max_features`, and
//! becomes the matrix columns; rows are `doc_1`..`doc_n`.

use std::collections::{BTreeMap, HashMap};

use serde_json::{json, Value};

/// Most documents accepted in one run.
pub const MAX_DOCUMENTS: usize = 10_000;
/// Hard ceiling on matrix columns — also the `max_features` maximum.
pub const MAX_TERMS: usize = 5_000;

/// A vocabulary entry: the term, how many documents contain it, and how many
/// times it occurs across the whole corpus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Term {
    pub term: String,
    pub document_frequency: usize,
    pub total_count: usize,
}

/// The built matrix, before rendering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Matrix {
    /// Row labels, `doc_1`..`doc_n`.
    pub documents: Vec<String>,
    /// Column terms, ordered by descending document frequency then term ascending.
    pub terms: Vec<Term>,
    /// One row per document, one cell per term (count, or 0/1 when binary).
    pub rows: Vec<Vec<usize>>,
}

impl Matrix {
    /// Sum of a row's cells. With `binary` weighting this is the number of
    /// distinct kept terms present in the document.
    pub fn row_total(&self, row: usize) -> usize {
        self.rows[row].iter().sum()
    }
}

/// Split `text` into words. A word is a run of Unicode alphanumerics; a
/// straight apostrophe, a curly apostrophe or a hyphen is kept when it sits
/// *between* two word characters ("don't", "state-of-the-art" stay single
/// terms). Curly apostrophes are normalized to straight ones. Everything else
/// — punctuation, symbols, whitespace — separates.
pub fn tokenize(text: &str, case_sensitive: bool) -> Vec<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut out: Vec<String> = Vec::new();
    let mut cur = String::new();
    for (i, &c) in chars.iter().enumerate() {
        if c.is_alphanumeric() {
            cur.push(c);
        } else if matches!(c, '\'' | '\u{2019}' | '-')
            && !cur.is_empty()
            && chars.get(i + 1).is_some_and(|n| n.is_alphanumeric())
        {
            cur.push(if c == '\u{2019}' { '\'' } else { c });
        } else if !cur.is_empty() {
            out.push(std::mem::take(&mut cur));
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    if case_sensitive {
        out
    } else {
        out.iter().map(|t| t.to_lowercase()).collect()
    }
}

/// Contiguous n-grams of every length in `ngram_min..=ngram_max`, joined by a
/// single space. Shorter documents simply contribute fewer n-grams.
fn ngrams(tokens: &[String], ngram_min: usize, ngram_max: usize) -> Vec<String> {
    let mut out = Vec::new();
    for n in ngram_min..=ngram_max {
        if tokens.len() < n {
            continue;
        }
        for w in tokens.windows(n) {
            out.push(w.join(" "));
        }
    }
    out
}

/// Split the raw input into documents. `input_format` is `auto`, `json` or
/// `lines`; `auto` treats input starting with `[` as JSON and anything else as
/// one document per line (blank lines are skipped).
pub fn split_documents(input: &str, input_format: &str) -> Result<Vec<String>, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err("expected at least one document, got empty input".into());
    }
    let format = match input_format {
        "auto" => {
            if trimmed.starts_with('[') {
                "json"
            } else {
                "lines"
            }
        }
        other => other,
    };
    let docs = match format {
        "json" => {
            let parsed: Value = serde_json::from_str(trimmed).map_err(|e| {
                format!(
                    "expected a JSON array of strings, but the input is not valid JSON ({e}); \
                     use input_format='lines' for one document per line"
                )
            })?;
            let items = parsed.as_array().ok_or_else(|| {
                format!(
                    "expected a JSON array of strings, got a JSON {}",
                    json_type_name(&parsed)
                )
            })?;
            let mut docs = Vec::with_capacity(items.len());
            for (i, item) in items.iter().enumerate() {
                let s = item.as_str().ok_or_else(|| {
                    format!(
                        "expected a JSON array of strings, but element {} is a {}",
                        i + 1,
                        json_type_name(item)
                    )
                })?;
                docs.push(s.to_string());
            }
            docs
        }
        "lines" => input
            .lines()
            .map(|l| l.trim().to_string())
            .filter(|l| !l.is_empty())
            .collect(),
        other => {
            return Err(format!(
                "unknown input_format '{other}'; expected auto, json or lines"
            ))
        }
    };
    if docs.is_empty() {
        return Err("expected at least one document, got none".into());
    }
    if docs.len() > MAX_DOCUMENTS {
        return Err(format!(
            "expected at most {MAX_DOCUMENTS} documents, got {}",
            docs.len()
        ));
    }
    Ok(docs)
}

fn json_type_name(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

/// Build the document-term matrix from already-split documents.
#[allow(clippy::too_many_arguments)]
pub fn build(
    docs: &[String],
    weighting: &str,
    case_sensitive: bool,
    ngram_min: u32,
    ngram_max: u32,
    min_df: u32,
    max_features: u32,
) -> Result<Matrix, String> {
    let binary = match weighting {
        "count" => false,
        "binary" => true,
        other => {
            return Err(format!(
                "unknown weighting '{other}'; expected count or binary"
            ))
        }
    };
    if !(1..=3).contains(&ngram_min) {
        return Err(format!(
            "ngram_min must be between 1 and 3, got {ngram_min}"
        ));
    }
    if !(1..=3).contains(&ngram_max) {
        return Err(format!(
            "ngram_max must be between 1 and 3, got {ngram_max}"
        ));
    }
    if ngram_min > ngram_max {
        return Err(format!(
            "ngram_min ({ngram_min}) must not be greater than ngram_max ({ngram_max})"
        ));
    }
    if !(1..=100_000).contains(&min_df) {
        return Err(format!("min_df must be between 1 and 100000, got {min_df}"));
    }
    if max_features as usize > MAX_TERMS {
        return Err(format!(
            "max_features must be between 0 and {MAX_TERMS}, got {max_features}"
        ));
    }

    // Per-document term counts + corpus-wide document frequency.
    let mut per_doc: Vec<BTreeMap<String, usize>> = Vec::with_capacity(docs.len());
    let mut df: HashMap<String, usize> = HashMap::new();
    let mut total: HashMap<String, usize> = HashMap::new();
    for doc in docs {
        let tokens = tokenize(doc, case_sensitive);
        let mut counts: BTreeMap<String, usize> = BTreeMap::new();
        for gram in ngrams(&tokens, ngram_min as usize, ngram_max as usize) {
            *counts.entry(gram).or_insert(0) += 1;
        }
        for (term, n) in &counts {
            *df.entry(term.clone()).or_insert(0) += 1;
            *total.entry(term.clone()).or_insert(0) += n;
        }
        per_doc.push(counts);
    }

    let max_df_seen = df.values().copied().max().unwrap_or(0);
    let mut vocab: Vec<Term> = df
        .iter()
        .filter(|(_, &n)| n >= min_df as usize)
        .map(|(term, &n)| Term {
            term: term.clone(),
            document_frequency: n,
            total_count: total[term],
        })
        .collect();
    // Descending document frequency, then term ascending — a stable, useful
    // column order (the terms that tie the corpus together come first).
    vocab.sort_by(|a, b| {
        b.document_frequency
            .cmp(&a.document_frequency)
            .then_with(|| a.term.cmp(&b.term))
    });

    if vocab.is_empty() {
        return Err(if max_df_seen == 0 {
            "no terms found — the documents contain no words".to_string()
        } else {
            format!(
                "no terms left after min_df={min_df} (the most common term appears in only \
                 {max_df_seen} document(s)); lower min_df"
            )
        });
    }
    if max_features > 0 {
        vocab.truncate(max_features as usize);
    } else if vocab.len() > MAX_TERMS {
        return Err(format!(
            "the vocabulary is {} terms, over the {MAX_TERMS}-column limit; \
             set max_features (1-{MAX_TERMS}) or raise min_df",
            vocab.len()
        ));
    }

    let rows = per_doc
        .iter()
        .map(|counts| {
            vocab
                .iter()
                .map(|t| {
                    let n = counts.get(&t.term).copied().unwrap_or(0);
                    if binary && n > 0 {
                        1
                    } else {
                        n
                    }
                })
                .collect()
        })
        .collect();

    Ok(Matrix {
        documents: (1..=docs.len()).map(|i| format!("doc_{i}")).collect(),
        terms: vocab,
        rows,
    })
}

/// Quote a delimited field when it contains the delimiter, a quote or a newline.
fn csv_field(s: &str, delim: char) -> String {
    if s.contains(delim) || s.contains('"') || s.contains('\n') || s.contains('\r') {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn render_delimited(m: &Matrix, delim: char, include_totals: bool) -> String {
    let mut out = String::new();
    let mut header: Vec<String> = vec!["document".to_string()];
    header.extend(m.terms.iter().map(|t| csv_field(&t.term, delim)));
    if include_totals {
        header.push("__total_terms".to_string());
    }
    out.push_str(&header.join(&delim.to_string()));
    for (i, row) in m.rows.iter().enumerate() {
        let mut cells: Vec<String> = vec![m.documents[i].clone()];
        cells.extend(row.iter().map(|v| v.to_string()));
        if include_totals {
            cells.push(m.row_total(i).to_string());
        }
        out.push('\n');
        out.push_str(&cells.join(&delim.to_string()));
    }
    out
}

#[allow(clippy::too_many_arguments)]
fn render_json(
    m: &Matrix,
    weighting: &str,
    case_sensitive: bool,
    ngram_min: u32,
    ngram_max: u32,
    min_df: u32,
    max_features: u32,
    include_totals: bool,
) -> String {
    let mut body = json!({
        "weighting": weighting,
        "case_sensitive": case_sensitive,
        "ngram_min": ngram_min,
        "ngram_max": ngram_max,
        "min_df": min_df,
        "max_features": max_features,
        "document_count": m.documents.len(),
        "documents": m.documents,
        "term_count": m.terms.len(),
        "terms": m.terms.iter().map(|t| t.term.clone()).collect::<Vec<_>>(),
        "document_frequency": m.terms.iter().map(|t| t.document_frequency).collect::<Vec<_>>(),
        "total_count": m.terms.iter().map(|t| t.total_count).collect::<Vec<_>>(),
        "matrix": m.rows,
    });
    if include_totals {
        let totals: Vec<usize> = (0..m.rows.len()).map(|i| m.row_total(i)).collect();
        body["total_terms"] = json!(totals);
    }
    serde_json::to_string_pretty(&body).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
}

/// Build a document-term matrix and render it.
///
/// * `documents` — a JSON array of strings, or one document per line.
/// * `input_format` — `auto` | `json` | `lines`.
/// * `weighting` — `count` (occurrences) | `binary` (0/1 presence).
/// * `case_sensitive` — when false (default) terms are case-folded.
/// * `ngram_min` / `ngram_max` — n-gram sizes to include, each 1..=3.
/// * `min_df` — keep only terms appearing in at least this many documents.
/// * `max_features` — keep at most this many columns (0 = no cap).
/// * `output` — `csv` | `json` | `tsv`.
/// * `include_totals` — add the `__total_terms` column / `total_terms` array.
#[allow(clippy::too_many_arguments)]
pub fn run(
    documents: &str,
    input_format: &str,
    weighting: &str,
    case_sensitive: bool,
    ngram_min: u32,
    ngram_max: u32,
    min_df: u32,
    max_features: u32,
    output: &str,
    include_totals: bool,
) -> Result<String, String> {
    if !matches!(output, "csv" | "json" | "tsv") {
        return Err(format!(
            "unknown output '{output}'; expected csv, json or tsv"
        ));
    }
    let docs = split_documents(documents, input_format)?;
    let matrix = build(
        &docs,
        weighting,
        case_sensitive,
        ngram_min,
        ngram_max,
        min_df,
        max_features,
    )?;
    Ok(match output {
        "csv" => render_delimited(&matrix, ',', include_totals),
        "tsv" => render_delimited(&matrix, '\t', include_totals),
        _ => render_json(
            &matrix,
            weighting,
            case_sensitive,
            ngram_min,
            ngram_max,
            min_df,
            max_features,
            include_totals,
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_two_documents_as_csv() {
        let out = run(
            "the cat sat\nthe dog sat",
            "auto",
            "count",
            false,
            1,
            1,
            1,
            0,
            "csv",
            true,
        )
        .unwrap();
        // sat/the appear in both docs (df 2), cat/dog in one (df 1);
        // ties break on the term ascending.
        assert_eq!(
            out,
            "document,sat,the,cat,dog,__total_terms\n\
             doc_1,1,1,1,0,3\n\
             doc_2,1,1,0,1,3"
        );
    }

    #[test]
    fn lines_and_json_input_agree() {
        let lines = run("a b\nb c", "lines", "count", false, 1, 1, 1, 0, "csv", true).unwrap();
        let js = run(
            r#"["a b", "b c"]"#,
            "json",
            "count",
            false,
            1,
            1,
            1,
            0,
            "csv",
            true,
        )
        .unwrap();
        assert_eq!(lines, js);
        assert_eq!(
            lines,
            "document,b,a,c,__total_terms\ndoc_1,1,1,0,2\ndoc_2,1,0,1,2"
        );
    }

    #[test]
    fn binary_weighting_clamps_repeats() {
        let out = run(
            "cat cat cat\ncat dog",
            "lines",
            "binary",
            false,
            1,
            1,
            1,
            0,
            "csv",
            true,
        )
        .unwrap();
        assert_eq!(
            out,
            "document,cat,dog,__total_terms\ndoc_1,1,0,1\ndoc_2,1,1,2"
        );
    }

    #[test]
    fn case_folding_is_default_and_optional() {
        let folded = run("The the", "lines", "count", false, 1, 1, 1, 0, "csv", false).unwrap();
        assert_eq!(folded, "document,the\ndoc_1,2");
        let kept = run("The the", "lines", "count", true, 1, 1, 1, 0, "csv", false).unwrap();
        assert_eq!(kept, "document,The,the\ndoc_1,1,1");
    }

    #[test]
    fn punctuation_is_stripped_but_apostrophes_and_hyphens_survive() {
        assert_eq!(
            tokenize("Don't stop -- state-of-the-art, really!", false),
            vec!["don't", "stop", "state-of-the-art", "really"]
        );
        // A curly apostrophe folds to a straight one; a trailing hyphen drops.
        assert_eq!(tokenize("it\u{2019}s over-", false), vec!["it's", "over"]);
    }

    #[test]
    fn ngrams_include_every_size_in_range() {
        let out = run(
            "red car\nred car",
            "lines",
            "count",
            false,
            1,
            2,
            2,
            0,
            "csv",
            false,
        )
        .unwrap();
        assert_eq!(out, "document,car,red,red car\ndoc_1,1,1,1\ndoc_2,1,1,1");
    }

    #[test]
    fn min_df_and_max_features_trim_the_vocabulary() {
        let corpus = "apple pear\napple plum\napple pear";
        let min_df = run(corpus, "lines", "count", false, 1, 1, 2, 0, "csv", false).unwrap();
        assert_eq!(
            min_df,
            "document,apple,pear\ndoc_1,1,1\ndoc_2,1,0\ndoc_3,1,1"
        );
        let capped = run(corpus, "lines", "count", false, 1, 1, 1, 1, "csv", false).unwrap();
        assert_eq!(capped, "document,apple\ndoc_1,1\ndoc_2,1\ndoc_3,1");
    }

    #[test]
    fn tsv_and_json_render_the_same_matrix() {
        let tsv = run("a b\nb", "lines", "count", false, 1, 1, 1, 0, "tsv", true).unwrap();
        assert_eq!(
            tsv,
            "document\tb\ta\t__total_terms\ndoc_1\t1\t1\t2\ndoc_2\t1\t0\t1"
        );

        let js: Value = serde_json::from_str(
            &run("a b\nb", "lines", "count", false, 1, 1, 1, 0, "json", true).unwrap(),
        )
        .unwrap();
        assert_eq!(js["terms"], json!(["b", "a"]));
        assert_eq!(js["document_frequency"], json!([2, 1]));
        assert_eq!(js["matrix"], json!([[1, 1], [1, 0]]));
        assert_eq!(js["total_terms"], json!([2, 1]));
        assert_eq!(js["documents"], json!(["doc_1", "doc_2"]));
    }

    #[test]
    fn totals_column_is_optional() {
        let with = run("a b", "lines", "count", false, 1, 1, 1, 0, "csv", true).unwrap();
        assert!(with.starts_with("document,a,b,__total_terms"));
        let without = run("a b", "lines", "count", false, 1, 1, 1, 0, "csv", false).unwrap();
        assert_eq!(without, "document,a,b\ndoc_1,1,1");
    }

    #[test]
    fn empty_input_is_an_error() {
        let err = run("   ", "auto", "count", false, 1, 1, 1, 0, "csv", true).unwrap_err();
        assert!(err.contains("at least one document"), "got: {err}");
    }

    #[test]
    fn non_string_json_element_is_an_error() {
        let err = run(
            r#"["ok", 42]"#,
            "json",
            "count",
            false,
            1,
            1,
            1,
            0,
            "csv",
            true,
        )
        .unwrap_err();
        assert_eq!(
            err,
            "expected a JSON array of strings, but element 2 is a number"
        );
    }

    #[test]
    fn malformed_json_says_how_to_recover() {
        let err = run("[\"a\", ", "json", "count", false, 1, 1, 1, 0, "csv", true).unwrap_err();
        assert!(err.contains("not valid JSON"), "got: {err}");
        assert!(err.contains("input_format='lines'"), "got: {err}");
    }

    #[test]
    fn bad_option_values_are_rejected() {
        let err = run("a", "lines", "count", false, 3, 1, 1, 0, "csv", true).unwrap_err();
        assert_eq!(err, "ngram_min (3) must not be greater than ngram_max (1)");
        let err = run("a", "lines", "tfidf", false, 1, 1, 1, 0, "csv", true).unwrap_err();
        assert_eq!(err, "unknown weighting 'tfidf'; expected count or binary");
        let err = run("a", "lines", "count", false, 1, 1, 1, 0, "xlsx", true).unwrap_err();
        assert_eq!(err, "unknown output 'xlsx'; expected csv, json or tsv");
        let err = run("a", "lines", "count", false, 1, 4, 1, 0, "csv", true).unwrap_err();
        assert_eq!(err, "ngram_max must be between 1 and 3, got 4");
        let err = run("a", "lines", "count", false, 1, 1, 0, 0, "csv", true).unwrap_err();
        assert_eq!(err, "min_df must be between 1 and 100000, got 0");
        let err = run("a", "lines", "count", false, 1, 1, 1, 9000, "csv", true).unwrap_err();
        assert_eq!(err, "max_features must be between 0 and 5000, got 9000");
    }

    #[test]
    fn min_df_that_removes_everything_explains_itself() {
        let err = run("a b\nc d", "lines", "count", false, 1, 1, 2, 0, "csv", true).unwrap_err();
        assert!(err.contains("no terms left after min_df=2"), "got: {err}");
        assert!(err.contains("only 1 document"), "got: {err}");
    }

    #[test]
    fn documents_without_words_still_get_a_row() {
        let out = run(
            "cat\n!!!\ncat",
            "lines",
            "count",
            false,
            1,
            1,
            1,
            0,
            "csv",
            true,
        )
        .unwrap();
        assert_eq!(
            out,
            "document,cat,__total_terms\ndoc_1,1,1\ndoc_2,0,0\ndoc_3,1,1"
        );
    }

    #[test]
    fn too_many_documents_is_an_error() {
        let many = (0..MAX_DOCUMENTS + 1)
            .map(|i| format!("doc {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let err = run(&many, "lines", "count", false, 1, 1, 1, 0, "csv", true).unwrap_err();
        assert_eq!(err, "expected at most 10000 documents, got 10001");
    }
}
