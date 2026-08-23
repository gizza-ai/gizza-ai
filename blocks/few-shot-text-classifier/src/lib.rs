//! gizza-ai/few-shot-text-classifier — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI + page query-params); handle() delegates to block_utils::run_skill. No
//! host calls — runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_few_shot_text_classifier_core::{
    classify, Options, MAX_K, MAX_MIN_DF, MAX_NGRAM, MAX_TOP_K,
};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    /// The labeled support set, one example per line as `label<separator>text`.
    examples: String,
    /// The text to classify against the support set.
    text: String,
    /// How label and text are split on each example line.
    #[serde(default = "default_separator")]
    separator: String,
    /// Treat `text` as one document or as one document per line.
    #[serde(default = "default_input_mode")]
    input_mode: String,
    /// How example similarities become a label decision.
    #[serde(default = "default_method")]
    method: String,
    /// Neighbours inspected by the knn method.
    #[serde(default = "default_k")]
    k: usize,
    /// Which similarity metric compares two vectors.
    #[serde(default = "default_similarity")]
    similarity: String,
    /// Feature weighting applied before comparing.
    #[serde(default = "default_weighting")]
    weighting: String,
    /// Word or character features.
    #[serde(default = "default_analyzer")]
    analyzer: String,
    /// Longest n-gram used as a feature.
    #[serde(default = "default_ngram_max")]
    ngram_max: usize,
    /// Fold everything to lower case before tokenizing.
    #[serde(default = "default_true")]
    lowercase: bool,
    /// Fold accented Latin letters onto their ASCII base.
    #[serde(default)]
    strip_accents: bool,
    /// Drop common English stop words.
    #[serde(default)]
    remove_stopwords: bool,
    /// Use `1 + ln(tf)` instead of the raw term frequency.
    #[serde(default)]
    sublinear_tf: bool,
    /// Minimum number of examples a term must appear in.
    #[serde(default = "default_min_df")]
    min_df: usize,
    /// Confidence the winner must reach before it is reported as the prediction.
    #[serde(default)]
    min_confidence: f64,
    /// How many labels to list.
    #[serde(default = "default_top_k")]
    top_k: usize,
    /// Include the shared terms and nearest example behind the decision.
    #[serde(default = "default_true")]
    explain: bool,
    /// Output format: report, json or csv.
    #[serde(default = "default_output")]
    output: String,
}

fn default_separator() -> String {
    "auto".into()
}
fn default_input_mode() -> String {
    "single".into()
}
fn default_method() -> String {
    "centroid".into()
}
fn default_k() -> usize {
    3
}
fn default_similarity() -> String {
    "cosine".into()
}
fn default_weighting() -> String {
    "tfidf".into()
}
fn default_analyzer() -> String {
    "word".into()
}
fn default_ngram_max() -> usize {
    1
}
fn default_true() -> bool {
    true
}
fn default_min_df() -> usize {
    1
}
fn default_top_k() -> usize {
    3
}
fn default_output() -> String {
    "report".into()
}

impl From<&Args> for Options {
    fn from(a: &Args) -> Self {
        Options {
            separator: a.separator.clone(),
            input_mode: a.input_mode.clone(),
            method: a.method.clone(),
            k: a.k,
            similarity: a.similarity.clone(),
            weighting: a.weighting.clone(),
            analyzer: a.analyzer.clone(),
            ngram_max: a.ngram_max,
            lowercase: a.lowercase,
            strip_accents: a.strip_accents,
            remove_stopwords: a.remove_stopwords,
            sublinear_tf: a.sublinear_tf,
            min_df: a.min_df,
            min_confidence: a.min_confidence,
            top_k: a.top_k,
            explain: a.explain,
            output: a.output.clone(),
        }
    }
}

/// Single-source param descriptor → chat schema (and CLI + page query-params).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("examples")
                .required()
                .describe("The labeled support set: one example per line, written as label<separator>text, for example `billing,invoice charge refund`. Two examples per label is the practical minimum and 3-8 per label is the sweet spot — this is few-shot, so a handful is enough. The first separator on a line splits the label from the text, so the text itself may contain more of them; surrounding double quotes are stripped, so a pasted two-column CSV works, and lines starting with # are treated as comments. Needs at least 2 distinct labels. Capped at 1 MiB, 5000 examples and 200 labels."),
        )
        .param(
            Param::string("text")
                .required()
                .describe("The text to classify against the support set. With input_mode=single the whole value is one document; with input_mode=lines each non-blank line is classified separately. Capped at 256 KiB and 1000 batch lines."),
        )
        .param(
            Param::enumv("separator", ["auto", "tab", "comma", "pipe", "colon"])
                .default("auto")
                .describe("How each example line is split into label and text: auto (default — picks whichever of tab, comma, pipe or colon appears on the most lines), or force one of tab, comma, pipe, colon. Force it when your example text contains the auto-detected character more often than the labels do."),
        )
        .param(
            Param::enumv("input_mode", ["single", "lines"])
                .default("single")
                .describe("What text holds: single (default) classifies the whole value as one document; lines classifies every non-blank line separately and returns one row per line, for batch-labelling a list."),
        )
        .param(
            Param::enumv("method", ["centroid", "knn", "best-match"])
                .default("centroid")
                .describe("How example similarities become a label decision. centroid (default) averages each label's examples into one prototype vector and compares against those — the steadiest choice with only a few examples per label. knn scores every example, keeps the k most similar overall and lets them vote weighted by similarity, which handles labels that cover several distinct topics. best-match takes each label's single closest example, so one strongly matching example is enough to win."),
        )
        .param(
            Param::integer("k")
                .default(3)
                .min(1.0)
                .max(MAX_K as f64)
                .describe("How many nearest examples vote when method=knn. 3 (default) is a good starting point; 1 makes it a pure nearest-neighbour rule and larger values smooth out one-off matches. Never exceeds the number of examples you supplied. Maximum 50. Ignored by the centroid and best-match methods."),
        )
        .param(
            Param::enumv("similarity", ["cosine", "dot", "euclidean", "jaccard"])
                .default("cosine")
                .describe("How two feature vectors are compared. cosine (default) measures angle only, so long and short texts compare fairly. dot is the raw dot product, which rewards longer texts with more matching terms. euclidean is straight-line distance reported as 1/(1+distance), so it is always above zero even with nothing in common. jaccard compares term sets only — shared terms over total distinct terms — and ignores the weighting setting. Every metric is reported so that higher means more similar."),
        )
        .param(
            Param::enumv("weighting", ["tfidf", "tf", "binary"])
                .default("tfidf")
                .describe("How term counts become feature weights. tfidf (default) multiplies the term frequency by a smoothed inverse document frequency, so words common to every example count for less and distinctive words dominate. tf uses the raw counts. binary uses presence/absence only, which suits very short texts such as titles or search queries."),
        )
        .param(
            Param::enumv("analyzer", ["word", "char"])
                .default("word")
                .describe("What a feature is. word (default) splits on non-alphanumeric characters and uses whole words. char slides a window over the text and uses character n-grams instead, which tolerates typos, inflection and languages that do not space-separate words — set ngram_max to 3-5 when you pick it."),
        )
        .param(
            Param::integer("ngram_max")
                .default(1)
                .min(1.0)
                .max(MAX_NGRAM as f64)
                .describe("Longest n-gram used as a feature. With analyzer=word this includes every length from 1 up to this value, so 1 (default) is a plain bag of words and 2 also learns adjacent pairs such as `password reset`. With analyzer=char it is the exact character-window length, where 3-5 works best. Maximum 6."),
        )
        .param(
            Param::boolean("lowercase")
                .default(true)
                .describe("Fold the examples and the input to lower case before tokenizing, so `Refund` and `refund` are the same feature. Default true. Turn it off when capitalisation itself is a signal, for example ALL-CAPS shouting."),
        )
        .param(
            Param::boolean("strip_accents")
                .default(false)
                .describe("Fold accented Latin letters onto their ASCII base (café → cafe, straße → strasse) before tokenizing, so accented and unaccented spellings match. Default false. Turn it on for European-language text that is typed inconsistently."),
        )
        .param(
            Param::boolean("remove_stopwords")
                .default(false)
                .describe("Drop common English function words (the, and, is, to, …) before features are formed. Default false, because with few examples every word carries signal. Turn it on for topic classification of longer English documents."),
        )
        .param(
            Param::boolean("sublinear_tf")
                .default(false)
                .describe("Replace the raw term frequency with 1 + ln(count), so a word repeated ten times counts a little more than one repeated once instead of ten times as much. Default false. Turn it on for longer documents with repetitive vocabulary."),
        )
        .param(
            Param::integer("min_df")
                .default(1)
                .min(1.0)
                .max(MAX_MIN_DF as f64)
                .describe("Minimum number of examples a term must appear in to stay in the vocabulary. 1 (default) keeps everything, which is usually right for a few-shot support set; 2 drops terms that appear in only one example and shrinks the vocabulary. Maximum 100. If it would empty the vocabulary you get an error instead."),
        )
        .param(
            Param::number("min_confidence")
                .default(0.0)
                .min(0.0)
                .max(1.0)
                .describe("Confidence the winning label must reach, from 0 to 1, before it is reported as the prediction. 0 (default) always commits to the top label. Set 0.5 or 0.6 to have weak, ambiguous inputs come back as `uncertain` instead, with a note explaining which label was leading. Confidence is each label's share of the total vote weight, so it always sums to 100% across labels."),
        )
        .param(
            Param::integer("top_k")
                .default(3)
                .min(0.0)
                .max(MAX_TOP_K as f64)
                .describe("How many of the highest-scoring labels to list with their similarity and confidence. Default 3; 0 lists every label. Maximum 50. The prediction itself is always the top label regardless of this setting."),
        )
        .param(
            Param::boolean("explain")
                .default(true)
                .describe("Include the terms shared between the input and the winning label, ranked by how much they contributed, plus the single nearest training example and its similarity. Default true. In batch mode this becomes a compact top-terms column per row."),
        )
        .param(
            Param::enumv("output", ["report", "json", "csv"])
                .default("report")
                .describe("Output format: report (default) is an aligned human-readable summary with the prediction, label scores, explanation, settings and training statistics; json is the same information as a machine-readable object; csv is one row per classified text with text, prediction, confidence, similarity and top terms, for pasting into a spreadsheet."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/few-shot-text-classifier",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Classify text by similarity to a handful of labeled examples, with no training or model.",
    skill(
        description = "Classify text by similarity to a handful of user-provided labeled examples — few-shot classification with no model download, no API key and no training run. Paste a support set of one example per line as label<separator>text (separator auto-detected from tab, comma, pipe or colon), then classify one document or batch-label every line of a list. Features are word or character n-grams weighted by tf-idf, plain term frequency or presence; labels are decided by nearest centroid, similarity-weighted k nearest neighbours, or the single best-matching example, compared with cosine, dot product, euclidean distance or Jaccard overlap. Options cover case folding, accent stripping, English stop words, sublinear term frequency, a minimum document frequency, and a confidence floor below which the answer comes back as `uncertain`. Returns the predicted label, per-label similarity and confidence, the shared terms that drove the match and the nearest example, as an aligned report, JSON or CSV. Fully deterministic and runs entirely locally.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "few-shot-text-classifier", |a: Args| {
            let opts: Options = (&a).into();
            classify(&a.examples, &a.text, &opts).map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let authored: serde_json::Value = serde_json::from_str(AUTHORED).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn args_defaults_match_the_descriptor() {
        let a: Args =
            serde_json::from_str(r#"{"examples":"a,x\nb,y","text":"x"}"#).unwrap();
        let o: Options = (&a).into();
        assert_eq!(o.separator, "auto");
        assert_eq!(o.input_mode, "single");
        assert_eq!(o.method, "centroid");
        assert_eq!(o.k, 3);
        assert_eq!(o.similarity, "cosine");
        assert_eq!(o.weighting, "tfidf");
        assert_eq!(o.analyzer, "word");
        assert_eq!(o.ngram_max, 1);
        assert!(o.lowercase);
        assert!(!o.strip_accents);
        assert!(!o.remove_stopwords);
        assert!(!o.sublinear_tf);
        assert_eq!(o.min_df, 1);
        assert_eq!(o.min_confidence, 0.0);
        assert_eq!(o.top_k, 3);
        assert!(o.explain);
        assert_eq!(o.output, "report");
    }

    #[test]
    fn args_flow_through_to_a_real_classification() {
        let a: Args = serde_json::from_str(
            r#"{"examples":"spam|free money now\nham|team lunch today","text":"free money","output":"json","separator":"pipe","method":"best-match"}"#,
        )
        .unwrap();
        let o: Options = (&a).into();
        let out = classify(&a.examples, &a.text, &o).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["prediction"], "spam");
        assert_eq!(v["training"]["separator"], "pipe");
        assert_eq!(v["settings"]["method"], "best-match");
    }

    const AUTHORED: &str = r#"{
        "type": "object",
        "properties": {
            "examples": { "type": "string", "description": "The labeled support set: one example per line, written as label<separator>text, for example `billing,invoice charge refund`. Two examples per label is the practical minimum and 3-8 per label is the sweet spot — this is few-shot, so a handful is enough. The first separator on a line splits the label from the text, so the text itself may contain more of them; surrounding double quotes are stripped, so a pasted two-column CSV works, and lines starting with # are treated as comments. Needs at least 2 distinct labels. Capped at 1 MiB, 5000 examples and 200 labels." },
            "text": { "type": "string", "description": "The text to classify against the support set. With input_mode=single the whole value is one document; with input_mode=lines each non-blank line is classified separately. Capped at 256 KiB and 1000 batch lines." },
            "separator": { "type": "string", "enum": ["auto", "tab", "comma", "pipe", "colon"], "default": "auto", "description": "How each example line is split into label and text: auto (default — picks whichever of tab, comma, pipe or colon appears on the most lines), or force one of tab, comma, pipe, colon. Force it when your example text contains the auto-detected character more often than the labels do." },
            "input_mode": { "type": "string", "enum": ["single", "lines"], "default": "single", "description": "What text holds: single (default) classifies the whole value as one document; lines classifies every non-blank line separately and returns one row per line, for batch-labelling a list." },
            "method": { "type": "string", "enum": ["centroid", "knn", "best-match"], "default": "centroid", "description": "How example similarities become a label decision. centroid (default) averages each label's examples into one prototype vector and compares against those — the steadiest choice with only a few examples per label. knn scores every example, keeps the k most similar overall and lets them vote weighted by similarity, which handles labels that cover several distinct topics. best-match takes each label's single closest example, so one strongly matching example is enough to win." },
            "k": { "type": "integer", "minimum": 1, "maximum": 50, "default": 3, "description": "How many nearest examples vote when method=knn. 3 (default) is a good starting point; 1 makes it a pure nearest-neighbour rule and larger values smooth out one-off matches. Never exceeds the number of examples you supplied. Maximum 50. Ignored by the centroid and best-match methods." },
            "similarity": { "type": "string", "enum": ["cosine", "dot", "euclidean", "jaccard"], "default": "cosine", "description": "How two feature vectors are compared. cosine (default) measures angle only, so long and short texts compare fairly. dot is the raw dot product, which rewards longer texts with more matching terms. euclidean is straight-line distance reported as 1/(1+distance), so it is always above zero even with nothing in common. jaccard compares term sets only — shared terms over total distinct terms — and ignores the weighting setting. Every metric is reported so that higher means more similar." },
            "weighting": { "type": "string", "enum": ["tfidf", "tf", "binary"], "default": "tfidf", "description": "How term counts become feature weights. tfidf (default) multiplies the term frequency by a smoothed inverse document frequency, so words common to every example count for less and distinctive words dominate. tf uses the raw counts. binary uses presence/absence only, which suits very short texts such as titles or search queries." },
            "analyzer": { "type": "string", "enum": ["word", "char"], "default": "word", "description": "What a feature is. word (default) splits on non-alphanumeric characters and uses whole words. char slides a window over the text and uses character n-grams instead, which tolerates typos, inflection and languages that do not space-separate words — set ngram_max to 3-5 when you pick it." },
            "ngram_max": { "type": "integer", "minimum": 1, "maximum": 6, "default": 1, "description": "Longest n-gram used as a feature. With analyzer=word this includes every length from 1 up to this value, so 1 (default) is a plain bag of words and 2 also learns adjacent pairs such as `password reset`. With analyzer=char it is the exact character-window length, where 3-5 works best. Maximum 6." },
            "lowercase": { "type": "boolean", "default": true, "description": "Fold the examples and the input to lower case before tokenizing, so `Refund` and `refund` are the same feature. Default true. Turn it off when capitalisation itself is a signal, for example ALL-CAPS shouting." },
            "strip_accents": { "type": "boolean", "default": false, "description": "Fold accented Latin letters onto their ASCII base (café → cafe, straße → strasse) before tokenizing, so accented and unaccented spellings match. Default false. Turn it on for European-language text that is typed inconsistently." },
            "remove_stopwords": { "type": "boolean", "default": false, "description": "Drop common English function words (the, and, is, to, …) before features are formed. Default false, because with few examples every word carries signal. Turn it on for topic classification of longer English documents." },
            "sublinear_tf": { "type": "boolean", "default": false, "description": "Replace the raw term frequency with 1 + ln(count), so a word repeated ten times counts a little more than one repeated once instead of ten times as much. Default false. Turn it on for longer documents with repetitive vocabulary." },
            "min_df": { "type": "integer", "minimum": 1, "maximum": 100, "default": 1, "description": "Minimum number of examples a term must appear in to stay in the vocabulary. 1 (default) keeps everything, which is usually right for a few-shot support set; 2 drops terms that appear in only one example and shrinks the vocabulary. Maximum 100. If it would empty the vocabulary you get an error instead." },
            "min_confidence": { "type": "number", "minimum": 0, "maximum": 1, "default": 0.0, "description": "Confidence the winning label must reach, from 0 to 1, before it is reported as the prediction. 0 (default) always commits to the top label. Set 0.5 or 0.6 to have weak, ambiguous inputs come back as `uncertain` instead, with a note explaining which label was leading. Confidence is each label's share of the total vote weight, so it always sums to 100% across labels." },
            "top_k": { "type": "integer", "minimum": 0, "maximum": 50, "default": 3, "description": "How many of the highest-scoring labels to list with their similarity and confidence. Default 3; 0 lists every label. Maximum 50. The prediction itself is always the top label regardless of this setting." },
            "explain": { "type": "boolean", "default": true, "description": "Include the terms shared between the input and the winning label, ranked by how much they contributed, plus the single nearest training example and its similarity. Default true. In batch mode this becomes a compact top-terms column per row." },
            "output": { "type": "string", "enum": ["report", "json", "csv"], "default": "report", "description": "Output format: report (default) is an aligned human-readable summary with the prediction, label scores, explanation, settings and training statistics; json is the same information as a machine-readable object; csv is one row per classified text with text, prediction, confidence, similarity and top terms, for pasting into a spreadsheet." }
        },
        "required": ["examples", "text"],
        "additionalProperties": false
    }"#;
}
