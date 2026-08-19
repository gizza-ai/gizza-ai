//! gizza-ai/entropy-calculator — Shannon entropy of a string, in bits (or nats,
//! dits, trits) per symbol. Thin wrapper: the chat schema is single-sourced from
//! descriptor() (which also drives the CLI) and handle() delegates to
//! block_utils::run_skill; all the math lives in the pure core crate.
//!
//! Pure Rust, no I/O → runs on every backend including the chat Service Worker.
//! Binary FILES are covered by the sibling byte-entropy block, which adds a
//! per-block entropy series over fetched bytes; this block is the text/symbol
//! calculator and is the one with a standalone page.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_entropy_calculator_core::{
    analyze, render, Basis, Options, Scope, Unit, DEFAULT_PRECISION, DEFAULT_TOP_SYMBOLS,
    MAX_PRECISION, MAX_TOP_SYMBOLS,
};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    basis: Option<String>,
    unit: Option<String>,
    scope: Option<String>,
    ignore_case: Option<bool>,
    ignore_whitespace: Option<bool>,
    precision: Option<u32>,
    show_frequencies: Option<bool>,
    top_symbols: Option<u32>,
}

#[derive(Serialize)]
struct FrequencyOut {
    /// Printable form of the symbol (control characters escaped).
    symbol: String,
    count: usize,
    share_percent: f64,
}

#[derive(Serialize)]
struct PartOut {
    label: String,
    symbols: usize,
    distinct_symbols: usize,
    entropy: f64,
    total_information: f64,
}

#[derive(Serialize)]
struct Resp {
    /// Shannon entropy per symbol, in the requested unit.
    entropy: f64,
    /// The unit the entropy figures are in.
    unit: String,
    /// What one symbol was.
    basis: String,
    /// `entropy × symbols`.
    total_information: f64,
    /// Number of symbols counted.
    symbols: usize,
    /// Distinct symbol values seen.
    distinct_symbols: usize,
    /// Entropy a uniform distribution over the same alphabet would have.
    max_entropy: f64,
    /// `entropy / max_entropy × 100`.
    efficiency_percent: f64,
    /// `100 − efficiency_percent`.
    redundancy_percent: f64,
    /// Effective number of equally likely symbols (`base^entropy`).
    perplexity: f64,
    /// Most frequent symbols, highest first.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    frequencies: Vec<FrequencyOut>,
    /// Distinct symbols not listed in `frequencies`.
    #[serde(skip_serializing_if = "is_zero")]
    frequencies_omitted: usize,
    /// Per-line / per-paragraph breakdown (empty unless `scope` asked for one).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    parts: Vec<PartOut>,
    /// The same figures rendered as the plain-text report the page shows.
    report: String,
}

fn is_zero(n: &usize) -> bool {
    *n == 0
}

/// Single source for the chat schema and the CLI.
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text").required().describe(
                "The string, key, password, or passage to measure. Analyzed locally and never stored or sent anywhere. Maximum 1 MiB.",
            ),
        )
        .param(
            Param::enumv("basis", ["characters", "bytes", "words", "lines"])
                .default("characters")
                .describe(
                    "What counts as one symbol: 'characters' (Unicode scalars, the usual choice for keys and passwords), 'bytes' (UTF-8 bytes, the 0-8 bits/byte convention used for binary data), 'words' (whitespace-separated), or 'lines'. Default 'characters'.",
                ),
        )
        .param(
            Param::enumv("unit", ["bits", "nats", "dits", "trits"])
                .default("bits")
                .describe(
                    "Logarithm base for the entropy figures: 'bits' (base 2, shannons - the default), 'nats' (base e), 'dits' (base 10, also called hartleys or bans), or 'trits' (base 3).",
                ),
        )
        .param(
            Param::enumv("scope", ["whole", "line", "paragraph"])
                .default("whole")
                .describe(
                    "Score the input as one sequence ('whole', the default), or report one entropy per 'line' or per blank-line-separated 'paragraph' plus a combined figure. Maximum 20000 parts.",
                ),
        )
        .param(
            Param::boolean("ignore_case")
                .default(false)
                .describe(
                    "Fold upper- and lower-case together before counting, so 'A' and 'a' are the same symbol. Default false.",
                ),
        )
        .param(
            Param::boolean("ignore_whitespace")
                .default(false)
                .describe(
                    "Drop spaces, tabs, and newlines before counting (blank lines when basis='lines'; no effect when basis='words'). Default false.",
                ),
        )
        .param(
            Param::integer("precision")
                .min(0.0)
                .max(MAX_PRECISION as f64)
                .default(DEFAULT_PRECISION as i64)
                .describe("Decimal places for every non-integer figure, 0 to 10. Default 4."),
        )
        .param(
            Param::boolean("show_frequencies")
                .default(true)
                .describe(
                    "Include the symbol-frequency table (symbol, count, share, bar). Default true.",
                ),
        )
        .param(
            Param::integer("top_symbols")
                .min(0.0)
                .max(MAX_TOP_SYMBOLS as f64)
                .default(DEFAULT_TOP_SYMBOLS as i64)
                .describe(
                    "How many rows the frequency table shows, most frequent first, 0 to 64. Default 12.",
                ),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

/// Build the core options from the parsed args, mapping any bad enum value to an
/// invalid-args error that names the accepted values.
fn options_from(a: &Args) -> Result<Options, SkillError> {
    let d = Options::default();
    Ok(Options {
        basis: match &a.basis {
            Some(s) => Basis::parse(s).map_err(SkillError::InvalidArgs)?,
            None => d.basis,
        },
        unit: match &a.unit {
            Some(s) => Unit::parse(s).map_err(SkillError::InvalidArgs)?,
            None => d.unit,
        },
        scope: match &a.scope {
            Some(s) => Scope::parse(s).map_err(SkillError::InvalidArgs)?,
            None => d.scope,
        },
        ignore_case: a.ignore_case.unwrap_or(d.ignore_case),
        ignore_whitespace: a.ignore_whitespace.unwrap_or(d.ignore_whitespace),
        precision: a.precision.map(|n| n as usize).unwrap_or(d.precision),
        show_frequencies: a.show_frequencies.unwrap_or(d.show_frequencies),
        top_symbols: a.top_symbols.map(|n| n as usize).unwrap_or(d.top_symbols),
    })
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/entropy-calculator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Shannon entropy of a string, in bits per character",
    skill(
        description = "Calculate the Shannon entropy of a string in bits per character (or nats, dits, trits), to gauge how random a key, password, token, or passage is. Reports entropy per symbol, total information, distinct and total symbol counts, maximum entropy, efficiency, redundancy, perplexity, and a symbol-frequency table. Symbols can be characters, UTF-8 bytes, words, or lines, and the text can be scored as a whole or per line/paragraph. Pure local math on the text you pass; nothing is stored or transmitted. For binary files use the byte-entropy tool instead.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "entropy-calculator", |a: Args| {
            let opts = options_from(&a)?;
            let report = analyze(&a.text, &opts).map_err(SkillError::InvalidArgs)?;
            let o = &report.overall;
            Ok(Resp {
                entropy: o.entropy,
                unit: opts.unit.name().to_string(),
                basis: opts.basis.name().to_string(),
                total_information: o.total_information,
                symbols: o.symbols,
                distinct_symbols: o.distinct_symbols,
                max_entropy: o.max_entropy,
                efficiency_percent: o.efficiency_percent,
                redundancy_percent: o.redundancy_percent,
                perplexity: o.perplexity,
                frequencies: o
                    .frequencies
                    .iter()
                    .map(|f| FrequencyOut {
                        symbol: f.symbol.clone(),
                        count: f.count,
                        share_percent: f.share_percent,
                    })
                    .collect(),
                frequencies_omitted: o.frequencies_omitted,
                parts: report
                    .parts
                    .iter()
                    .map(|p| PartOut {
                        label: p.label.clone(),
                        symbols: p.analysis.symbols,
                        distinct_symbols: p.analysis.distinct_symbols,
                        entropy: p.analysis.entropy,
                        total_information: p.analysis.total_information,
                    })
                    .collect(),
                report: render(&report),
            })
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "text": {
                        "type": "string",
                        "description": "The string, key, password, or passage to measure. Analyzed locally and never stored or sent anywhere. Maximum 1 MiB."
                    },
                    "basis": {
                        "type": "string",
                        "enum": ["characters", "bytes", "words", "lines"],
                        "default": "characters",
                        "description": "What counts as one symbol: 'characters' (Unicode scalars, the usual choice for keys and passwords), 'bytes' (UTF-8 bytes, the 0-8 bits/byte convention used for binary data), 'words' (whitespace-separated), or 'lines'. Default 'characters'."
                    },
                    "unit": {
                        "type": "string",
                        "enum": ["bits", "nats", "dits", "trits"],
                        "default": "bits",
                        "description": "Logarithm base for the entropy figures: 'bits' (base 2, shannons - the default), 'nats' (base e), 'dits' (base 10, also called hartleys or bans), or 'trits' (base 3)."
                    },
                    "scope": {
                        "type": "string",
                        "enum": ["whole", "line", "paragraph"],
                        "default": "whole",
                        "description": "Score the input as one sequence ('whole', the default), or report one entropy per 'line' or per blank-line-separated 'paragraph' plus a combined figure. Maximum 20000 parts."
                    },
                    "ignore_case": {
                        "type": "boolean",
                        "default": false,
                        "description": "Fold upper- and lower-case together before counting, so 'A' and 'a' are the same symbol. Default false."
                    },
                    "ignore_whitespace": {
                        "type": "boolean",
                        "default": false,
                        "description": "Drop spaces, tabs, and newlines before counting (blank lines when basis='lines'; no effect when basis='words'). Default false."
                    },
                    "precision": {
                        "type": "integer",
                        "minimum": 0,
                        "maximum": 10,
                        "default": 4,
                        "description": "Decimal places for every non-integer figure, 0 to 10. Default 4."
                    },
                    "show_frequencies": {
                        "type": "boolean",
                        "default": true,
                        "description": "Include the symbol-frequency table (symbol, count, share, bar). Default true."
                    },
                    "top_symbols": {
                        "type": "integer",
                        "minimum": 0,
                        "maximum": 64,
                        "default": 12,
                        "description": "How many rows the frequency table shows, most frequent first, 0 to 64. Default 12."
                    }
                },
                "required": ["text"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn options_default_when_unset_and_reject_bad_enums() {
        let a: Args = serde_json::from_str(r#"{"text":"abc"}"#).unwrap();
        assert_eq!(options_from(&a).unwrap(), Options::default());

        let a: Args = serde_json::from_str(r#"{"text":"abc","unit":"bytes"}"#).unwrap();
        let err = options_from(&a).unwrap_err();
        assert!(
            format!("{err:?}").contains("bits, nats, dits, trits"),
            "{err:?}"
        );
    }
}
