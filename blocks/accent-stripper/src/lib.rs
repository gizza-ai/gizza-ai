//! gizza-ai/accent-stripper — chat skill block on the shared tool abstraction.
//! Converts accented and non-ASCII text to plain ASCII, or conservatively drops only
//! combining marks, with explicit policies for characters that still cannot be mapped.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_accent_stripper_core::{
    strip, Mode, Options, Unmapped, MAX_INPUT_CHARS, MAX_REPLACEMENT_CHARS,
};
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default = "default_unmapped")]
    unmapped: String,
    #[serde(default = "default_replacement")]
    replacement: String,
    #[serde(default)]
    keep: String,
    #[serde(default)]
    lowercase: bool,
    #[serde(default)]
    collapse_whitespace: bool,
    #[serde(default)]
    include_report: bool,
}

fn default_mode() -> String {
    "transliterate".into()
}
fn default_unmapped() -> String {
    "keep".into()
}
fn default_replacement() -> String {
    "?".into()
}

fn options_from_args(a: &Args) -> Result<Options, String> {
    Ok(Options {
        mode: Mode::parse(&a.mode)?,
        unmapped: Unmapped::parse(&a.unmapped)?,
        replacement: a.replacement.clone(),
        keep: a.keep.clone(),
        lowercase: a.lowercase,
        collapse_whitespace: a.collapse_whitespace,
    })
}

fn run_tool(a: Args) -> Result<String, String> {
    let opts = options_from_args(&a)?;
    let report = strip(&a.input, &opts)?;
    if a.include_report {
        Ok(format!(
            "{{\n  \"result\": {},\n  \"input_chars\": {},\n  \"output_chars\": {},\n  \"non_ascii_in\": {},\n  \"converted\": {},\n  \"kept\": {},\n  \"unmapped\": {},\n  \"output_is_ascii\": {},\n  \"unmapped_samples\": [{}]\n}}",
            json_string(&report.result),
            report.input_chars,
            report.output_chars,
            report.non_ascii_in,
            report.converted,
            report.kept,
            report.unmapped,
            report.output_is_ascii,
            report.unmapped_samples.iter().map(|s| json_string(s)).collect::<Vec<_>>().join(", ")
        ))
    } else {
        Ok(report.result)
    }
}

fn json_string(s: &str) -> String {
    serde_json::to_string(s).expect("string serialization cannot fail")
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input")
                .required()
                .describe("Text to convert. Accented Latin text, already-decomposed combining marks, symbols, and many non-Latin scripts are accepted. Max 200,000 characters."),
        )
        .param(
            Param::enumv("mode", ["transliterate", "marks-only"])
                .default("transliterate")
                .describe("Conversion strategy: 'transliterate' (default) maps accented letters and many non-Latin characters to their closest ASCII spelling, so Straße becomes Strasse and Москва becomes Moskva; 'marks-only' only removes combining accents after Unicode decomposition, so café becomes cafe but ß, ø, Ж and other letters without removable marks stay non-ASCII unless the unmapped policy handles them."),
        )
        .param(
            Param::enumv("unmapped", ["keep", "remove", "replace"])
                .default("keep")
                .describe("What to do with characters still non-ASCII after the selected conversion: 'keep' leaves them in place, 'remove' deletes them, and 'replace' swaps each one for the replacement string. Use remove or replace when the output must be strict ASCII."),
        )
        .param(
            Param::string("replacement")
                .default("?")
                .describe("ASCII text inserted for each unmapped character when unmapped=replace. Must be plain ASCII and at most 8 characters. Ignored unless unmapped is replace."),
        )
        .param(
            Param::string("keep")
                .default("")
                .describe("Non-ASCII characters to protect from conversion, entered literally as a short string. For example keep='ñ' keeps mañana as mañana while still converting café to cafe. ASCII characters never need listing because they are always preserved."),
        )
        .param(
            Param::boolean("lowercase")
                .default(false)
                .describe("Lowercase the converted text after accent stripping/transliteration. Default false preserves the casing created by the transliteration table."),
        )
        .param(
            Param::boolean("collapse_whitespace")
                .default(false)
                .describe("Trim each line and collapse runs of spaces or tabs inside the line to one plain space after conversion. Line breaks are preserved. Default false keeps spacing exactly as the converter emitted it."),
        )
        .param(
            Param::boolean("include_report")
                .default(false)
                .describe("Return a JSON report instead of only the converted text. The report includes character counts, how many non-ASCII characters were converted/kept/unmapped, whether the result is pure ASCII, and up to 20 unmapped samples."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/accent-stripper",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Strip accents and transliterate text to plain ASCII.",
    skill(
        description = "Remove diacritics from text and optionally transliterate non-ASCII letters/scripts to their closest plain ASCII spelling. Use the default transliterate mode for slugs, search keys, filenames and CSV cleanup — it handles café→cafe, Straße→Strasse, Ærøskøbing→AEroskobing and Москва→Moskva. Use marks-only mode for the conservative Unicode-normalization pass that drops combining accents but leaves characters like ß, ø and Ж alone. Choose what happens to anything still non-ASCII (keep, remove or replace), protect specific characters with keep, lowercase the result, collapse whitespace, and optionally return a JSON audit report. Runs locally in the sandbox; nothing is uploaded.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "accent-stripper", |a: Args| {
            run_tool(a).map_err(SkillError::InvalidArgs)
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
                    "input": { "type": "string", "description": "Text to convert. Accented Latin text, already-decomposed combining marks, symbols, and many non-Latin scripts are accepted. Max 200,000 characters." },
                    "mode": { "type": "string", "enum": ["transliterate", "marks-only"], "default": "transliterate", "description": "Conversion strategy: 'transliterate' (default) maps accented letters and many non-Latin characters to their closest ASCII spelling, so Straße becomes Strasse and Москва becomes Moskva; 'marks-only' only removes combining accents after Unicode decomposition, so café becomes cafe but ß, ø, Ж and other letters without removable marks stay non-ASCII unless the unmapped policy handles them." },
                    "unmapped": { "type": "string", "enum": ["keep", "remove", "replace"], "default": "keep", "description": "What to do with characters still non-ASCII after the selected conversion: 'keep' leaves them in place, 'remove' deletes them, and 'replace' swaps each one for the replacement string. Use remove or replace when the output must be strict ASCII." },
                    "replacement": { "type": "string", "default": "?", "description": "ASCII text inserted for each unmapped character when unmapped=replace. Must be plain ASCII and at most 8 characters. Ignored unless unmapped is replace." },
                    "keep": { "type": "string", "default": "", "description": "Non-ASCII characters to protect from conversion, entered literally as a short string. For example keep='ñ' keeps mañana as mañana while still converting café to cafe. ASCII characters never need listing because they are always preserved." },
                    "lowercase": { "type": "boolean", "default": false, "description": "Lowercase the converted text after accent stripping/transliteration. Default false preserves the casing created by the transliteration table." },
                    "collapse_whitespace": { "type": "boolean", "default": false, "description": "Trim each line and collapse runs of spaces or tabs inside the line to one plain space after conversion. Line breaks are preserved. Default false keeps spacing exactly as the converter emitted it." },
                    "include_report": { "type": "boolean", "default": false, "description": "Return a JSON report instead of only the converted text. The report includes character counts, how many non-ASCII characters were converted/kept/unmapped, whether the result is pure ASCII, and up to 20 unmapped samples." }
                },
                "required": ["input"],
                "additionalProperties": false
            }"#,
        ).unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored);
    }

    #[test]
    fn descriptor_describes_every_param() {
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = derived["properties"].as_object().unwrap();
        assert_eq!(props.len(), 8);
        for (name, prop) in props {
            let d = prop["description"].as_str().unwrap_or("");
            assert!(d.len() > 20, "param '{name}' needs a real .describe()");
        }
    }

    #[test]
    fn run_tool_can_return_report() {
        let got = run_tool(Args {
            input: "café".into(),
            mode: default_mode(),
            unmapped: default_unmapped(),
            replacement: default_replacement(),
            keep: String::new(),
            lowercase: false,
            collapse_whitespace: false,
            include_report: true,
        })
        .unwrap();
        assert!(got.contains("\"result\": \"cafe\""), "{got}");
        assert!(got.contains("\"converted\": 1"), "{got}");
    }
}
