//! gizza-ai/text-normalizer — cleans and normalizes text (chat skill block).
//!
//! Thin chat-skill wrapper around `gizza-ai-text-normalizer-core`. The chat
//! schema is derived from `descriptor()` (single source — shared shape across
//! chat + CLI); the handler delegates to `block_utils::run_skill`. No host
//! calls — runs entirely inside the WASM sandbox.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
#[cfg(test)]
use gizza_ai_block_utils::{Input, Param, ToolDescriptor};
use gizza_ai_block_utils::{run_skill, SkillError};
use gizza_ai_text_normalizer_core::{normalize, Case, Form, Options, Whitespace};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default = "default_form")]
    form: String,
    #[serde(default)]
    case: String,
    #[serde(default)]
    strip_accents: bool,
    #[serde(default = "default_whitespace")]
    whitespace: String,
    #[serde(default)]
    punctuation: bool,
}

fn default_form() -> String {
    "nfc".to_string()
}
fn default_whitespace() -> String {
    "collapse".to_string()
}

/// Single-source param descriptor → chat schema (and CLI).
#[cfg(test)]
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The text to clean and normalize."),
        )
        .param(
            Param::enumv("form", ["none", "nfc", "nfd", "nfkc", "nfkd"])
                .default("nfc")
                .describe(
                    "Unicode normalization form. 'nfc' (default) canonical composition, the \
web-recommended form; 'nfd' canonical decomposition; 'nfkc' compatibility composition, which \
also folds ligatures (ﬁ -> fi), full-width and circled/styled letters, and fractions to their \
plain equivalents; 'nfkd' compatibility decomposition; 'none' leaves code points untouched.",
                ),
        )
        .param(
            Param::enumv("case", ["none", "lower", "upper", "fold"])
                .default("none")
                .describe(
                    "Letter-case transform applied after normalization. 'none' (default) keeps \
case; 'lower' lowercases; 'upper' UPPERCASES; 'fold' case-folds for caseless matching (like \
lowercase but also maps ß -> ss).",
                ),
        )
        .param(
            Param::boolean("strip_accents")
                .default(false)
                .describe(
                    "When true, strip accents/diacritics by decomposing letters and removing \
combining marks (café -> cafe, naïve -> naive). Default false.",
                ),
        )
        .param(
            Param::enumv("whitespace", ["keep", "trim", "collapse", "collapse-lines"])
                .default("collapse")
                .describe(
                    "Whitespace handling. 'collapse' (default) collapses every run of \
whitespace — spaces, tabs, newlines, and Unicode spaces — to a single space and trims the \
ends; 'trim' only trims leading/trailing whitespace; 'collapse-lines' collapses spaces within \
each line and drops blank lines but keeps line breaks; 'keep' leaves whitespace untouched.",
                ),
        )
        .param(
            Param::boolean("punctuation")
                .default(false)
                .describe(
                    "When true, normalize fancy Unicode punctuation to plain ASCII: curly \
quotes and guillemets -> straight quotes, en/em dashes -> -, the ellipsis glyph -> ..., prime \
marks -> ' and \". Default false.",
                ),
        )
}

#[cfg(test)]
fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn options_from(a: &Args) -> Result<Options, SkillError> {
    Ok(Options {
        form: Form::parse(&a.form).map_err(SkillError::InvalidArgs)?,
        case: Case::parse(&a.case).map_err(SkillError::InvalidArgs)?,
        strip_accents: a.strip_accents,
        whitespace: Whitespace::parse(&a.whitespace).map_err(SkillError::InvalidArgs)?,
        punctuation: a.punctuation,
    })
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/text-normalizer",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Clean and normalize text: Unicode form, case, accents, whitespace, punctuation.",
    skill(
        description = "Clean and normalize text in one pass. Apply a Unicode normalization form (form = none/nfc/nfd/nfkc/nfkd; default nfc — nfkc also folds ligatures like ﬁ->fi, full-width, and circled/styled letters), transform case (case = none/lower/upper/fold; 'fold' is caseless matching, mapping ß->ss), strip accents/diacritics (strip_accents = true turns café into cafe), normalize whitespace (whitespace = keep/trim/collapse/collapse-lines; default collapse squeezes every run of whitespace to one space and trims), and fold fancy punctuation to ASCII (punctuation = true turns curly quotes, em dashes, and ellipsis glyphs into \", -, and ...). Steps run in the order punctuation -> form -> strip accents -> case -> whitespace.",
        parameters = r#"{
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "The text to clean and normalize." },
                    "form": { "type": "string", "enum": ["none", "nfc", "nfd", "nfkc", "nfkd"], "default": "nfc", "description": "Unicode normalization form. 'nfc' (default) canonical composition, the web-recommended form; 'nfd' canonical decomposition; 'nfkc' compatibility composition, which also folds ligatures (ﬁ -> fi), full-width and circled/styled letters, and fractions to their plain equivalents; 'nfkd' compatibility decomposition; 'none' leaves code points untouched." },
                    "case": { "type": "string", "enum": ["none", "lower", "upper", "fold"], "default": "none", "description": "Letter-case transform applied after normalization. 'none' (default) keeps case; 'lower' lowercases; 'upper' UPPERCASES; 'fold' case-folds for caseless matching (like lowercase but also maps ß -> ss)." },
                    "strip_accents": { "type": "boolean", "default": false, "description": "When true, strip accents/diacritics by decomposing letters and removing combining marks (café -> cafe, naïve -> naive). Default false." },
                    "whitespace": { "type": "string", "enum": ["keep", "trim", "collapse", "collapse-lines"], "default": "collapse", "description": "Whitespace handling. 'collapse' (default) collapses every run of whitespace — spaces, tabs, newlines, and Unicode spaces — to a single space and trims the ends; 'trim' only trims leading/trailing whitespace; 'collapse-lines' collapses spaces within each line and drops blank lines but keeps line breaks; 'keep' leaves whitespace untouched." },
                    "punctuation": { "type": "boolean", "default": false, "description": "When true, normalize fancy Unicode punctuation to plain ASCII: curly quotes and guillemets -> straight quotes, en/em dashes -> -, the ellipsis glyph -> ..., prime marks -> ' and \". Default false." }
                },
                "required": ["text"],
                "additionalProperties": false
            }"#
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "text-normalizer", |a: Args| {
            let opts = options_from(&a)?;
            Ok::<String, SkillError>(normalize(&a.text, opts))
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
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "text": { "type": "string", "description": "The text to clean and normalize." },
                    "form": {
                        "type": "string",
                        "enum": ["none", "nfc", "nfd", "nfkc", "nfkd"],
                        "default": "nfc",
                        "description": "Unicode normalization form. 'nfc' (default) canonical composition, the web-recommended form; 'nfd' canonical decomposition; 'nfkc' compatibility composition, which also folds ligatures (ﬁ -> fi), full-width and circled/styled letters, and fractions to their plain equivalents; 'nfkd' compatibility decomposition; 'none' leaves code points untouched."
                    },
                    "case": {
                        "type": "string",
                        "enum": ["none", "lower", "upper", "fold"],
                        "default": "none",
                        "description": "Letter-case transform applied after normalization. 'none' (default) keeps case; 'lower' lowercases; 'upper' UPPERCASES; 'fold' case-folds for caseless matching (like lowercase but also maps ß -> ss)."
                    },
                    "strip_accents": {
                        "type": "boolean",
                        "default": false,
                        "description": "When true, strip accents/diacritics by decomposing letters and removing combining marks (café -> cafe, naïve -> naive). Default false."
                    },
                    "whitespace": {
                        "type": "string",
                        "enum": ["keep", "trim", "collapse", "collapse-lines"],
                        "default": "collapse",
                        "description": "Whitespace handling. 'collapse' (default) collapses every run of whitespace — spaces, tabs, newlines, and Unicode spaces — to a single space and trims the ends; 'trim' only trims leading/trailing whitespace; 'collapse-lines' collapses spaces within each line and drops blank lines but keeps line breaks; 'keep' leaves whitespace untouched."
                    },
                    "punctuation": {
                        "type": "boolean",
                        "default": false,
                        "description": "When true, normalize fancy Unicode punctuation to plain ASCII: curly quotes and guillemets -> straight quotes, en/em dashes -> -, the ellipsis glyph -> ..., prime marks -> ' and \". Default false."
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
}
