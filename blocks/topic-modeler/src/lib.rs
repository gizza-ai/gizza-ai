//! gizza-ai/topic-modeler — discover the latent topics across a corpus of
//! pasted documents with LDA (collapsed Gibbs sampling), fitted at run time.
//! The chat schema is single-sourced from `descriptor()` (which also drives the
//! CLI); `handle()` delegates to the pure `core::run`. Pure compute → runs on
//! every backend including the chat Service Worker.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_topic_modeler_core::{run, Options};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    documents: String,
    #[serde(default = "default_separator")]
    separator: String,
    #[serde(default = "default_topics")]
    topics: u32,
    #[serde(default = "default_words_per_topic")]
    words_per_topic: u32,
    #[serde(default = "default_iterations")]
    iterations: u32,
    #[serde(default)]
    alpha: f64,
    #[serde(default = "default_beta")]
    beta: f64,
    #[serde(default = "default_true")]
    remove_stopwords: bool,
    #[serde(default)]
    stopwords: String,
    #[serde(default = "default_min_word_length")]
    min_word_length: u32,
    #[serde(default = "default_seed")]
    seed: u64,
    #[serde(default = "default_output")]
    output: String,
}

fn default_separator() -> String {
    "blank-line".into()
}
fn default_topics() -> u32 {
    5
}
fn default_words_per_topic() -> u32 {
    8
}
fn default_iterations() -> u32 {
    200
}
fn default_beta() -> f64 {
    0.01
}
fn default_true() -> bool {
    true
}
fn default_min_word_length() -> u32 {
    3
}
fn default_seed() -> u64 {
    42
}
fn default_output() -> String {
    "report".into()
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("documents")
                .required()
                .multiline()
                .describe("The corpus to model: two or more documents pasted as one block of text, split by `separator`. Up to 300 documents, 25,000 kept words, and 20,000 distinct terms."),
        )
        .param(
            Param::enumv("separator", ["blank-line", "line", "dashes"])
                .default("blank-line")
                .describe("How `documents` is split into documents: 'blank-line' (one document per paragraph, separated by an empty line), 'line' (one document per non-empty line), or 'dashes' (documents separated by a line of three or more dashes). Default blank-line."),
        )
        .param(
            Param::integer("topics")
                .min(2.0)
                .max(20.0)
                .default(5)
                .describe("Number of topics K to fit (2–20). Fewer topics give broader themes, more topics give finer ones. Default 5."),
        )
        .param(
            Param::integer("words_per_topic")
                .min(1.0)
                .max(25.0)
                .default(8)
                .describe("How many top words to list per topic (1–25). Default 8."),
        )
        .param(
            Param::integer("iterations")
                .min(50.0)
                .max(1000.0)
                .default(200)
                .describe("Gibbs sampling sweeps over the corpus (50–1000). More iterations let the topics settle further at the cost of runtime. Default 200."),
        )
        .param(
            Param::number("alpha")
                .min(0.0)
                .max(100.0)
                .default(0.0)
                .describe("Dirichlet prior on the document–topic mixture. 0 means auto (50/K, the MALLET convention). Higher values make each document a blend of more topics; lower values make documents more single-topic. Default 0."),
        )
        .param(
            Param::number("beta")
                .min(0.001)
                .max(1.0)
                .default(0.01)
                .describe("Dirichlet prior on the topic–word distribution (0.001–1). Higher values make topics use more of the vocabulary; lower values make them sharper. Default 0.01."),
        )
        .param(
            Param::boolean("remove_stopwords")
                .default(true)
                .describe("Drop common English function words (the, and, of, …) before modelling so topics are built from content words. Default true."),
        )
        .param(
            Param::string("stopwords")
                .default("")
                .describe("Extra words to exclude, comma or whitespace separated (e.g. `company,report,q3`). Merged with the built-in list, and applied even when remove_stopwords is off — this is also how to filter a non-English corpus."),
        )
        .param(
            Param::integer("min_word_length")
                .min(1.0)
                .max(12.0)
                .default(3)
                .describe("Shortest word kept, in characters (1–12). Prunes noise like 'a' and 'to' that survive the stopword list. Default 3."),
        )
        .param(
            Param::integer("seed")
                .min(0.0)
                .default(42)
                .describe("Random seed for the sampler. The same corpus, settings, and seed always produce the same topics; change it to see whether a topic is stable. Default 42."),
        )
        .param(
            Param::enumv("output", ["report", "json", "csv"])
                .default("report")
                .describe("Result format: 'report' (ranked topics with their top words and corpus share, plus each document's topic mixture), 'json' (the full model: topics, word weights, and per-document mixtures), or 'csv' (a topic-keys table followed by the document–topic matrix). Default report."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn options(a: &Args) -> Options {
    Options {
        separator: a.separator.clone(),
        topics: a.topics,
        words_per_topic: a.words_per_topic,
        iterations: a.iterations,
        alpha: a.alpha,
        beta: a.beta,
        remove_stopwords: a.remove_stopwords,
        stopwords: a.stopwords.clone(),
        min_word_length: a.min_word_length,
        seed: a.seed,
        output: a.output.clone(),
    }
}

#[cfg(target_arch = "wasm32")]
struct TopicModeler;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/topic-modeler",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Discover the latent topics across a set of documents with LDA",
    skill(
        description = "Discover the latent topics running through a collection of documents with LDA (Latent Dirichlet Allocation), fitted right here from the pasted corpus — no training data, no upload, nothing pretrained. Paste two or more documents separated by blank lines, one per line, or --- fences, and the tool tokenises them, drops stopwords and short words, and runs collapsed Gibbs sampling to learn K topics. You get the ranked topics (their top words with weights and each topic's share of the corpus) and every document's topic mixture. Tune topics (K), words_per_topic, iterations, the Dirichlet priors alpha and beta, stopword removal plus your own stopword list, and min_word_length; the seed makes every run reproducible. Output as a readable report, JSON, or CSV (topic keys plus the document–topic matrix). Runs locally and deterministically.",
        parameters = schema_json()
    ),
)]
impl TopicModeler {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "topic-modeler", |a: Args| {
            run(&a.documents, &options(&a)).map_err(SkillError::InvalidArgs)
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
    /// copy, so an accidental descriptor edit can't silently change the
    /// LLM-facing schema (and the page controls the manifest renders from it).
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r##"{
                "type": "object",
                "properties": {
                    "documents": { "type": "string", "description": "The corpus to model: two or more documents pasted as one block of text, split by `separator`. Up to 300 documents, 25,000 kept words, and 20,000 distinct terms." },
                    "separator": { "type": "string", "enum": ["blank-line", "line", "dashes"], "default": "blank-line", "description": "How `documents` is split into documents: 'blank-line' (one document per paragraph, separated by an empty line), 'line' (one document per non-empty line), or 'dashes' (documents separated by a line of three or more dashes). Default blank-line." },
                    "topics": { "type": "integer", "minimum": 2, "maximum": 20, "default": 5, "description": "Number of topics K to fit (2–20). Fewer topics give broader themes, more topics give finer ones. Default 5." },
                    "words_per_topic": { "type": "integer", "minimum": 1, "maximum": 25, "default": 8, "description": "How many top words to list per topic (1–25). Default 8." },
                    "iterations": { "type": "integer", "minimum": 50, "maximum": 1000, "default": 200, "description": "Gibbs sampling sweeps over the corpus (50–1000). More iterations let the topics settle further at the cost of runtime. Default 200." },
                    "alpha": { "type": "number", "minimum": 0, "maximum": 100, "default": 0.0, "description": "Dirichlet prior on the document–topic mixture. 0 means auto (50/K, the MALLET convention). Higher values make each document a blend of more topics; lower values make documents more single-topic. Default 0." },
                    "beta": { "type": "number", "minimum": 0.001, "maximum": 1, "default": 0.01, "description": "Dirichlet prior on the topic–word distribution (0.001–1). Higher values make topics use more of the vocabulary; lower values make them sharper. Default 0.01." },
                    "remove_stopwords": { "type": "boolean", "default": true, "description": "Drop common English function words (the, and, of, …) before modelling so topics are built from content words. Default true." },
                    "stopwords": { "type": "string", "default": "", "description": "Extra words to exclude, comma or whitespace separated (e.g. `company,report,q3`). Merged with the built-in list, and applied even when remove_stopwords is off — this is also how to filter a non-English corpus." },
                    "min_word_length": { "type": "integer", "minimum": 1, "maximum": 12, "default": 3, "description": "Shortest word kept, in characters (1–12). Prunes noise like 'a' and 'to' that survive the stopword list. Default 3." },
                    "seed": { "type": "integer", "minimum": 0, "default": 42, "description": "Random seed for the sampler. The same corpus, settings, and seed always produce the same topics; change it to see whether a topic is stable. Default 42." },
                    "output": { "type": "string", "enum": ["report", "json", "csv"], "default": "report", "description": "Result format: 'report' (ranked topics with their top words and corpus share, plus each document's topic mixture), 'json' (the full model: topics, word weights, and per-document mixtures), or 'csv' (a topic-keys table followed by the document–topic matrix). Default report." }
                },
                "required": ["documents"],
                "additionalProperties": false
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    /// The handler's serde defaults must match the descriptor's declared
    /// defaults — a chat call that omits every optional param has to behave the
    /// same as one that passes the documented values.
    #[test]
    fn arg_defaults_match_the_descriptor() {
        let a: Args = serde_json::from_str(r#"{"documents":"a\n\nb"}"#).unwrap();
        let o = options(&a);
        assert_eq!(o.separator, "blank-line");
        assert_eq!(o.topics, 5);
        assert_eq!(o.words_per_topic, 8);
        assert_eq!(o.iterations, 200);
        assert_eq!(o.alpha, 0.0);
        assert_eq!(o.beta, 0.01);
        assert!(o.remove_stopwords);
        assert_eq!(o.stopwords, "");
        assert_eq!(o.min_word_length, 3);
        assert_eq!(o.seed, 42);
        assert_eq!(o.output, "report");
    }

    #[test]
    fn runs_a_corpus_through_the_handler_path() {
        let a: Args = serde_json::from_str(
            r#"{"documents":"butter flour sugar oven baking\n\nsugar oven baking dough crust\n\ncompiler module function type\n\nmodule function type returns compiler","topics":2}"#,
        )
        .unwrap();
        let out = run(&a.documents, &options(&a)).expect("run");
        assert!(out.starts_with("LDA topic model"));
        assert!(out.contains("4 documents · 2 topics"));
    }

    #[test]
    fn surfaces_bad_args_as_an_error() {
        let a: Args =
            serde_json::from_str(r#"{"documents":"only one document","topics":2}"#).unwrap();
        let err = run(&a.documents, &options(&a)).unwrap_err();
        assert!(err.contains("at least 2 documents"), "{err}");
    }
}
