//! gizza-ai/number-words-to-digits — turn spelled-out English numbers into digits.
//! Pure text in, text out: no network, no floating point (exact 128-bit decimals).
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]

use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_number_words_to_digits_core::convert;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize, Debug)]
struct Args {
    input: String,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default = "default_separator")]
    separator: String,
    #[serde(default = "default_scale")]
    scale: String,
    #[serde(default = "default_ordinals")]
    ordinals: String,
    #[serde(default = "default_true")]
    fractions: bool,
    #[serde(default)]
    digit_sequences: bool,
}

fn default_mode() -> String {
    "replace".to_string()
}
fn default_separator() -> String {
    "none".to_string()
}
fn default_scale() -> String {
    "short".to_string()
}
fn default_ordinals() -> String {
    "cardinal".to_string()
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("input").required().describe(
            "Text containing numbers written in English words, e.g. 'two hundred forty-three', \
             'one point five million', 'minus one and a half', or a whole paragraph. Multi-line \
             input is supported (one phrase per line in value mode). Maximum 200000 characters.",
        ))
        .param(
            Param::enumv("mode", ["replace", "value", "extract"])
                .default("replace")
                .describe(
                    "What to return: replace (default) rewrites the text in place, keeping every \
                     non-number word; value treats each non-empty line as one number phrase and \
                     errors on stray words; extract returns only the numbers found, one per line.",
                ),
        )
        .param(
            Param::enumv("separator", ["none", "comma", "space", "underscore"])
                .default("none")
                .describe(
                    "Thousands separator in the digits produced: none (default, 1250000), comma \
                     (1,250,000), space (1 250 000), or underscore (1_250_000).",
                ),
        )
        .param(
            Param::enumv("scale", ["short", "long"])
                .default("short")
                .describe(
                    "Reading of billion/trillion: short (default, US/modern UK — billion = 10^9) \
                     or long (continental European — billion = 10^12, trillion = 10^18). 'milliard' \
                     is always 10^9 and 'lakh'/'crore' are always accepted in both.",
                ),
        )
        .param(
            Param::enumv("ordinals", ["cardinal", "suffix", "ignore"])
                .default("cardinal")
                .describe(
                    "How ordinal words such as 'twenty-first' are handled: cardinal (default, -> 21), \
                     suffix (-> 21st), or ignore (left as words, so 'twenty-first' becomes '20-first' \
                     in replace mode because only the cardinal part converts).",
                ),
        )
        .param(Param::boolean("fractions").default(true).describe(
            "Read 'half' and 'quarter' as fraction words, so 'one and a half' -> 1.5, \
             'six and a quarter' -> 6.25, 'three quarters' -> 0.75, 'half a million' -> 500000. \
             Default true; set false to leave those words untouched.",
        ))
        .param(Param::boolean("digit_sequences").default(false).describe(
            "Read runs of single-digit words as a spoken digit string: 'nine one one' -> 911, \
             'one two three' -> 123. Default false, because it changes the meaning of ordinary \
             prose; grammar still wins when a scale word follows ('one hundred' stays 100).",
        ))
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn run_args(a: &Args) -> Result<String, String> {
    convert(
        &a.input,
        &a.mode,
        &a.separator,
        &a.scale,
        &a.ordinals,
        a.fractions,
        a.digit_sequences,
    )
}

#[cfg(target_arch = "wasm32")]
struct NumberWordsToDigits;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/number-words-to-digits",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert numbers written in English words into digits",
    skill(
        description = "Convert numbers spelled out in English words into digits — cardinals, hundreds and scale words up to decillion, Indian lakh/crore, decimal words ('five point forty-seven'), fraction words ('one and a half'), negatives, and ordinals. Works in place inside prose (default), line by line as strict values, or as a list of every number found. Options cover the thousands separator, the short/long reading of billion, ordinal output, fraction words, and spoken digit strings such as 'nine one one'. Exact 128-bit decimal arithmetic, no floating point, no network.",
        parameters = schema_json()
    ),
)]
impl NumberWordsToDigits {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "number-words-to-digits", |a: Args| {
            run_args(&a).map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(input: &str) -> Args {
        Args {
            input: input.to_string(),
            mode: default_mode(),
            separator: default_separator(),
            scale: default_scale(),
            ordinals: default_ordinals(),
            fractions: true,
            digit_sequences: false,
        }
    }

    #[test]
    fn schema_json_has_no_drift_in_required_shape() {
        let schema: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = schema["properties"].as_object().unwrap();
        for key in [
            "input",
            "mode",
            "separator",
            "scale",
            "ordinals",
            "fractions",
            "digit_sequences",
        ] {
            assert!(props.contains_key(key), "missing schema property {key}");
            assert!(
                props[key].get("description").is_some(),
                "{key} needs a description"
            );
        }
        assert_eq!(schema["required"], serde_json::json!(["input"]));
        assert_eq!(
            props["mode"]["enum"],
            serde_json::json!(["replace", "value", "extract"])
        );
        assert_eq!(
            props["separator"]["enum"],
            serde_json::json!(["none", "comma", "space", "underscore"])
        );
        assert_eq!(props["scale"]["enum"], serde_json::json!(["short", "long"]));
        assert_eq!(
            props["ordinals"]["enum"],
            serde_json::json!(["cardinal", "suffix", "ignore"])
        );
        assert_eq!(props["fractions"]["default"], serde_json::json!(true));
        assert_eq!(props["digit_sequences"]["default"], serde_json::json!(false));
    }

    #[test]
    fn defaults_deserialize_and_replace_in_prose() {
        let a: Args =
            serde_json::from_str(r#"{"input":"We shipped twenty-five units."}"#).unwrap();
        assert_eq!(run_args(&a).unwrap(), "We shipped 25 units.");
    }

    #[test]
    fn all_params_are_wired() {
        let mut a = args("one million two hundred fifty thousand");
        a.mode = "value".to_string();
        a.separator = "comma".to_string();
        assert_eq!(run_args(&a).unwrap(), "1,250,000");

        let mut a = args("one billion");
        a.mode = "value".to_string();
        a.scale = "long".to_string();
        assert_eq!(run_args(&a).unwrap(), "1000000000000");

        let mut a = args("the twenty-first of June");
        a.ordinals = "suffix".to_string();
        assert_eq!(run_args(&a).unwrap(), "the 21st of June");

        let mut a = args("one and a half");
        a.fractions = false;
        assert_eq!(run_args(&a).unwrap(), "1 and a half");

        let mut a = args("nine one one");
        a.digit_sequences = true;
        assert_eq!(run_args(&a).unwrap(), "911");

        let mut a = args("order twelve widgets and thirty-four bolts");
        a.mode = "extract".to_string();
        assert_eq!(run_args(&a).unwrap(), "12\n34");
    }

    #[test]
    fn bad_enum_values_error_clearly() {
        let mut a = args("twelve");
        a.mode = "nope".to_string();
        assert!(run_args(&a).unwrap_err().contains("unknown mode"));
    }
}
