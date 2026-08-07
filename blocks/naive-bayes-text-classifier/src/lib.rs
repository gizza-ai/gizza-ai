//! gizza-ai/naive-bayes-text-classifier — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI + page query-params); handle() delegates to block_utils::run_skill. No
//! host calls — runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_naive_bayes_text_classifier_core::{
    classify, Options, MAX_ALPHA, MAX_MIN_COUNT, MAX_NGRAM, MAX_TOP_K,
};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    /// Labeled examples, one per line, as `label<separator>text`.
    training_data: String,
    /// The text to classify with the trained model.
    text: String,
    /// How label and text are split on each training line.
    #[serde(default = "default_separator")]
    separator: String,
    /// Treat `text` as one document or as one document per line.
    #[serde(default = "default_input_mode")]
    input_mode: String,
    /// Which naive Bayes variant to train.
    #[serde(default = "default_model")]
    model: String,
    /// Additive smoothing constant.
    #[serde(default = "default_alpha")]
    alpha: f64,
    /// Longest word n-gram used as a feature.
    #[serde(default = "default_ngram_max")]
    ngram_max: usize,
    /// Fold everything to lower case before tokenizing.
    #[serde(default = "default_true")]
    lowercase: bool,
    /// Drop common English stop words.
    #[serde(default)]
    remove_stopwords: bool,
    /// Minimum corpus-wide occurrences for a token to stay in the vocabulary.
    #[serde(default = "default_min_count")]
    min_count: usize,
    /// Class prior style.
    #[serde(default = "default_priors")]
    priors: String,
    /// How many classes to list.
    #[serde(default = "default_top_k")]
    top_k: usize,
    /// Include the tokens that drove the decision.
    #[serde(default = "default_true")]
    explain: bool,
    /// Output format: report or json.
    #[serde(default = "default_output")]
    output: String,
}

fn default_separator() -> String {
    "auto".into()
}
fn default_input_mode() -> String {
    "single".into()
}
fn default_model() -> String {
    "multinomial".into()
}
fn default_alpha() -> f64 {
    1.0
}
fn default_ngram_max() -> usize {
    1
}
fn default_true() -> bool {
    true
}
fn default_min_count() -> usize {
    1
}
fn default_priors() -> String {
    "empirical".into()
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
            model: a.model.clone(),
            alpha: a.alpha,
            ngram_max: a.ngram_max,
            lowercase: a.lowercase,
            remove_stopwords: a.remove_stopwords,
            min_count: a.min_count,
            priors: a.priors.clone(),
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
            Param::string("training_data")
                .required()
                .describe("The labeled training set: one example per line, written as label<separator>text, for example `spam,win a free prize now`. Use at least 2 distinct labels and ideally 10+ examples per label. The first separator on a line splits the label from the text, so the text itself may contain more of them. Surrounding double quotes are stripped, so a pasted two-column CSV works. Capped at 1 MiB, 20000 examples and 200 labels."),
        )
        .param(
            Param::string("text")
                .required()
                .describe("The text to classify with the freshly trained model. With input_mode=single the whole value is one document; with input_mode=lines each non-blank line is classified separately. Capped at 256 KiB and 1000 batch lines."),
        )
        .param(
            Param::enumv("separator", ["auto", "tab", "comma", "pipe", "colon"])
                .default("auto")
                .describe("How each training line is split into label and text: auto (default — picks whichever of tab, comma, pipe or colon appears on the most lines), or force one of tab, comma, pipe, colon. Force it when your example text contains the auto-detected character more often than the labels do."),
        )
        .param(
            Param::enumv("input_mode", ["single", "lines"])
                .default("single")
                .describe("What text holds: single (default) classifies the whole value as one document; lines classifies every non-blank line separately and returns one row per line, for batch-labelling a list."),
        )
        .param(
            Param::enumv("model", ["multinomial", "bernoulli", "complement"])
                .default("multinomial")
                .describe("Which naive Bayes variant to train. multinomial (default) counts how often each token occurs and is the usual choice for topic and spam classification. bernoulli uses presence/absence per token and also scores the words that are missing, which suits short texts. complement uses complement-class statistics and holds up better when one label has many more examples than the others."),
        )
        .param(
            Param::number("alpha")
                .default(1.0)
                .min(0.0)
                .max(MAX_ALPHA)
                .describe("Additive (Lidstone) smoothing added to every token count so unseen words do not force a zero probability. 1.0 (default) is Laplace smoothing; lower values such as 0.1 trust the training counts more and give sharper confidences; higher values flatten them. 0 is clipped to 1e-10 so the logs stay finite. Maximum 10."),
        )
        .param(
            Param::integer("ngram_max")
                .default(1)
                .min(1.0)
                .max(MAX_NGRAM as f64)
                .describe("Longest word n-gram used as a feature. 1 (default) is a plain bag of words; 2 also learns adjacent word pairs such as `free money`, which helps on short texts at the cost of a much larger vocabulary; 3 adds triples. Maximum 3."),
        )
        .param(
            Param::boolean("lowercase")
                .default(true)
                .describe("Fold the training data and the input to lower case before tokenizing, so `Free` and `free` are the same feature. Default true. Turn it off when capitalisation itself is a signal, for example ALL-CAPS shouting."),
        )
        .param(
            Param::boolean("remove_stopwords")
                .default(false)
                .describe("Drop common English function words (the, and, is, to, …) before features are formed. Default false, because naive Bayes usually handles them fine and they matter in some domains. Turn it on for topic classification of longer English documents."),
        )
        .param(
            Param::integer("min_count")
                .default(1)
                .min(1.0)
                .max(MAX_MIN_COUNT as f64)
                .describe("Minimum number of times a token must occur across the whole training set to stay in the vocabulary. 1 (default) keeps everything; 2 or 3 removes one-off typos and names and shrinks the model. Maximum 100. If it would empty the vocabulary you get an error instead."),
        )
        .param(
            Param::enumv("priors", ["empirical", "uniform"])
                .default("empirical")
                .describe("Class prior probabilities: empirical (default) uses each label's share of the training examples, so a label with more examples starts ahead; uniform gives every label the same starting probability, which is what you want when the example counts do not reflect real-world frequencies. Ignored by the complement model, which scores from complement-class weights only."),
        )
        .param(
            Param::integer("top_k")
                .default(3)
                .min(0.0)
                .max(MAX_TOP_K as f64)
                .describe("How many of the highest-scoring labels to list with their probabilities. Default 3; 0 lists every label. Maximum 50. The prediction itself is always the top label regardless of this setting."),
        )
        .param(
            Param::boolean("explain")
                .default(true)
                .describe("Include the tokens that pushed the decision towards the winning label, scored as the weight gap between the top label and the runner-up. Default true. In batch mode this becomes a compact top-tokens column per row."),
        )
        .param(
            Param::enumv("output", ["report", "json"])
                .default("report")
                .describe("Output format: report (default) is an aligned human-readable summary with the prediction, class probabilities, explanation, model settings and training statistics; json is the same information as a machine-readable object (prediction, confidence, classes, explanation, model, training, notes). Default report."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/naive-bayes-text-classifier",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Train a naive Bayes text classifier on labeled examples and classify new text.",
    skill(
        description = "Train a naive Bayes text classifier from pasted labeled examples and immediately classify new text with it. Training data is one example per line as label<separator>text, with the separator auto-detected from tab, comma, pipe or colon. Supports the multinomial, Bernoulli and complement variants, Lidstone/Laplace smoothing, word n-grams up to 3, case folding, English stop-word removal, a minimum token count, and empirical or uniform class priors. Returns the predicted label, per-class probabilities, the tokens that decided it, the model settings and training statistics as an aligned report or JSON. Can classify one document or batch-label every line of a list. Trains from scratch on each call and runs entirely locally — nothing is uploaded and no pre-trained model is downloaded.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "naive-bayes-text-classifier", |a: Args| {
            let opts: Options = (&a).into();
            classify(&a.training_data, &a.text, &opts).map_err(SkillError::InvalidArgs)
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
            serde_json::from_str(r#"{"training_data":"a,x\nb,y","text":"x"}"#).unwrap();
        let o: Options = (&a).into();
        assert_eq!(o.separator, "auto");
        assert_eq!(o.input_mode, "single");
        assert_eq!(o.model, "multinomial");
        assert_eq!(o.alpha, 1.0);
        assert_eq!(o.ngram_max, 1);
        assert!(o.lowercase);
        assert!(!o.remove_stopwords);
        assert_eq!(o.min_count, 1);
        assert_eq!(o.priors, "empirical");
        assert_eq!(o.top_k, 3);
        assert!(o.explain);
        assert_eq!(o.output, "report");
    }

    #[test]
    fn args_flow_through_to_a_real_classification() {
        let a: Args = serde_json::from_str(
            r#"{"training_data":"spam|free money now\nham|team lunch today","text":"free money","output":"json","separator":"pipe"}"#,
        )
        .unwrap();
        let o: Options = (&a).into();
        let out = classify(&a.training_data, &a.text, &o).unwrap();
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["prediction"], "spam");
        assert_eq!(v["training"]["separator"], "pipe");
    }

    const AUTHORED: &str = r#"{
        "type": "object",
        "properties": {
            "training_data": { "type": "string", "description": "The labeled training set: one example per line, written as label<separator>text, for example `spam,win a free prize now`. Use at least 2 distinct labels and ideally 10+ examples per label. The first separator on a line splits the label from the text, so the text itself may contain more of them. Surrounding double quotes are stripped, so a pasted two-column CSV works. Capped at 1 MiB, 20000 examples and 200 labels." },
            "text": { "type": "string", "description": "The text to classify with the freshly trained model. With input_mode=single the whole value is one document; with input_mode=lines each non-blank line is classified separately. Capped at 256 KiB and 1000 batch lines." },
            "separator": { "type": "string", "enum": ["auto", "tab", "comma", "pipe", "colon"], "default": "auto", "description": "How each training line is split into label and text: auto (default — picks whichever of tab, comma, pipe or colon appears on the most lines), or force one of tab, comma, pipe, colon. Force it when your example text contains the auto-detected character more often than the labels do." },
            "input_mode": { "type": "string", "enum": ["single", "lines"], "default": "single", "description": "What text holds: single (default) classifies the whole value as one document; lines classifies every non-blank line separately and returns one row per line, for batch-labelling a list." },
            "model": { "type": "string", "enum": ["multinomial", "bernoulli", "complement"], "default": "multinomial", "description": "Which naive Bayes variant to train. multinomial (default) counts how often each token occurs and is the usual choice for topic and spam classification. bernoulli uses presence/absence per token and also scores the words that are missing, which suits short texts. complement uses complement-class statistics and holds up better when one label has many more examples than the others." },
            "alpha": { "type": "number", "minimum": 0, "maximum": 10, "default": 1.0, "description": "Additive (Lidstone) smoothing added to every token count so unseen words do not force a zero probability. 1.0 (default) is Laplace smoothing; lower values such as 0.1 trust the training counts more and give sharper confidences; higher values flatten them. 0 is clipped to 1e-10 so the logs stay finite. Maximum 10." },
            "ngram_max": { "type": "integer", "minimum": 1, "maximum": 3, "default": 1, "description": "Longest word n-gram used as a feature. 1 (default) is a plain bag of words; 2 also learns adjacent word pairs such as `free money`, which helps on short texts at the cost of a much larger vocabulary; 3 adds triples. Maximum 3." },
            "lowercase": { "type": "boolean", "default": true, "description": "Fold the training data and the input to lower case before tokenizing, so `Free` and `free` are the same feature. Default true. Turn it off when capitalisation itself is a signal, for example ALL-CAPS shouting." },
            "remove_stopwords": { "type": "boolean", "default": false, "description": "Drop common English function words (the, and, is, to, …) before features are formed. Default false, because naive Bayes usually handles them fine and they matter in some domains. Turn it on for topic classification of longer English documents." },
            "min_count": { "type": "integer", "minimum": 1, "maximum": 100, "default": 1, "description": "Minimum number of times a token must occur across the whole training set to stay in the vocabulary. 1 (default) keeps everything; 2 or 3 removes one-off typos and names and shrinks the model. Maximum 100. If it would empty the vocabulary you get an error instead." },
            "priors": { "type": "string", "enum": ["empirical", "uniform"], "default": "empirical", "description": "Class prior probabilities: empirical (default) uses each label's share of the training examples, so a label with more examples starts ahead; uniform gives every label the same starting probability, which is what you want when the example counts do not reflect real-world frequencies. Ignored by the complement model, which scores from complement-class weights only." },
            "top_k": { "type": "integer", "minimum": 0, "maximum": 50, "default": 3, "description": "How many of the highest-scoring labels to list with their probabilities. Default 3; 0 lists every label. Maximum 50. The prediction itself is always the top label regardless of this setting." },
            "explain": { "type": "boolean", "default": true, "description": "Include the tokens that pushed the decision towards the winning label, scored as the weight gap between the top label and the runner-up. Default true. In batch mode this becomes a compact top-tokens column per row." },
            "output": { "type": "string", "enum": ["report", "json"], "default": "report", "description": "Output format: report (default) is an aligned human-readable summary with the prediction, class probabilities, explanation, model settings and training statistics; json is the same information as a machine-readable object (prediction, confidence, classes, explanation, model, training, notes). Default report." }
        },
        "required": ["training_data", "text"],
        "additionalProperties": false
    }"#;
}
