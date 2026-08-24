//! gizza-ai/bson-extended-json-converter — convert MongoDB Extended JSON (the
//! `$oid` / `$date` / `$numberLong` wrapper dialect) to and from plain JSON.
//! Chat schema single-sourced from descriptor() (which also drives the CLI);
//! handle() delegates to block_utils::run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_bson_extended_json_converter_core::{convert, DateFormat, Direction, Mode, Options};
use serde::Deserialize;
use wafer_sdk::*;

fn default_direction() -> String {
    "auto".to_string()
}
fn default_mode() -> String {
    "relaxed".to_string()
}
fn default_date_format() -> String {
    "iso".to_string()
}
fn default_true() -> bool {
    true
}

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_direction")]
    direction: String,
    #[serde(default = "default_mode")]
    mode: String,
    #[serde(default = "default_date_format")]
    date_format: String,
    #[serde(default)]
    detect_types: bool,
    #[serde(default)]
    big_numbers_as_strings: bool,
    #[serde(default = "default_true")]
    pretty: bool,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("input").required().multiline().describe(
                "The JSON to convert. Either MongoDB Extended JSON (e.g. \
                 {\"_id\":{\"$oid\":\"507f1f77bcf86cd799439011\"}}) or plain JSON (e.g. \
                 {\"_id\":\"507f1f77bcf86cd799439011\"}). Objects, arrays and scalars are all accepted.",
            ),
        )
        .param(
            Param::enumv("direction", ["auto", "to-plain", "to-extended"])
                .default("auto")
                .describe(
                    "auto picks the direction from the input shape (any recognised $-wrapper means \
                     unwrap, otherwise wrap); to-plain strips the typed wrappers down to plain JSON; \
                     to-extended wraps values in Extended JSON, which also re-normalises Extended \
                     JSON input into the chosen mode.",
                ),
        )
        .param(
            Param::enumv("mode", ["relaxed", "canonical"])
                .default("relaxed")
                .describe(
                    "Which Extended JSON dialect to emit when converting to Extended JSON. relaxed \
                     keeps plain numbers and ISO-8601 dates for readability; canonical wraps every \
                     number and date ($numberInt, $numberLong, $numberDouble, $date/$numberLong) so \
                     the exact BSON type survives. Ignored when converting to plain JSON.",
                ),
        )
        .param(
            Param::enumv("date_format", ["iso", "epoch-millis"])
                .default("iso")
                .describe(
                    "How a BSON date renders when converting to plain JSON: iso gives an ISO-8601 \
                     UTC string like \"2024-07-20T14:30:00Z\"; epoch-millis gives the raw \
                     millisecond count like 1721485800000. Ignored when converting to Extended JSON.",
                ),
        )
        .param(
            Param::boolean("detect_types")
                .default(false)
                .describe(
                    "When converting plain JSON to Extended JSON, promote strings that look like \
                     values: a 24-character hex string becomes {\"$oid\": ...} and a full ISO-8601 \
                     date-time becomes {\"$date\": ...}. Off by default because the guess can be \
                     wrong for ordinary hex or date-like strings.",
                ),
        )
        .param(
            Param::boolean("big_numbers_as_strings")
                .default(false)
                .describe(
                    "When converting to plain JSON, emit 64-bit integers ($numberLong) and \
                     Decimal128 values ($numberDecimal) as JSON strings instead of numbers, so \
                     values beyond JavaScript's safe-integer range keep full precision. Infinity \
                     and NaN are always strings because JSON has no literal for them.",
                ),
        )
        .param(
            Param::boolean("pretty")
                .default(true)
                .describe("Pretty-print (indent) the JSON output. Set false for compact single-line output."),
        )
}

fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn build_options(a: &Args) -> Result<(Direction, Options), String> {
    Ok((
        Direction::parse(&a.direction)?,
        Options {
            mode: Mode::parse(&a.mode)?,
            date_format: DateFormat::parse(&a.date_format)?,
            detect_types: a.detect_types,
            big_numbers_as_strings: a.big_numbers_as_strings,
            pretty: a.pretty,
        },
    ))
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/bson-extended-json-converter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert MongoDB Extended JSON to and from plain JSON",
    skill(
        description = "Convert MongoDB Extended JSON (the BSON wrapper dialect with $oid, $date, $numberLong, $numberDecimal, $binary, $regularExpression, $timestamp, $minKey, $maxKey, $undefined, $symbol, $code/$scope and $dbPointer) to and from plain JSON. direction=auto detects which way to convert; to-plain unwraps every typed value into ordinary JSON; to-extended wraps plain JSON and also re-normalises Extended JSON between the relaxed and canonical dialects (mode). Canonical v2, relaxed v2 and the legacy v1 spellings ({\"$date\": 1721485800000}, {\"$binary\": \"...\", \"$type\": \"00\"}, {\"$regex\": \"...\", \"$options\": \"i\"}) are all accepted on input. date_format picks ISO-8601 or epoch millis for unwrapped dates; big_numbers_as_strings keeps 64-bit and Decimal128 values exact; detect_types optionally promotes 24-hex and ISO-8601 strings to $oid and $date when wrapping; pretty toggles indented vs compact output. Document key order is preserved, and DBRef ($ref/$id/$db) round-trips as the ordinary subdocument it is. Errors name the offending field path. Runs locally; no MongoDB driver or network.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "bson-extended-json-converter", |a: Args| {
            let (dir, opts) = build_options(&a).map_err(SkillError::InvalidArgs)?;
            convert(&a.input, dir, opts).map_err(SkillError::InvalidArgs)
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
                    "input": { "type": "string", "description": "The JSON to convert. Either MongoDB Extended JSON (e.g. {\"_id\":{\"$oid\":\"507f1f77bcf86cd799439011\"}}) or plain JSON (e.g. {\"_id\":\"507f1f77bcf86cd799439011\"}). Objects, arrays and scalars are all accepted." },
                    "direction": { "type": "string", "enum": ["auto", "to-plain", "to-extended"], "default": "auto", "description": "auto picks the direction from the input shape (any recognised $-wrapper means unwrap, otherwise wrap); to-plain strips the typed wrappers down to plain JSON; to-extended wraps values in Extended JSON, which also re-normalises Extended JSON input into the chosen mode." },
                    "mode": { "type": "string", "enum": ["relaxed", "canonical"], "default": "relaxed", "description": "Which Extended JSON dialect to emit when converting to Extended JSON. relaxed keeps plain numbers and ISO-8601 dates for readability; canonical wraps every number and date ($numberInt, $numberLong, $numberDouble, $date/$numberLong) so the exact BSON type survives. Ignored when converting to plain JSON." },
                    "date_format": { "type": "string", "enum": ["iso", "epoch-millis"], "default": "iso", "description": "How a BSON date renders when converting to plain JSON: iso gives an ISO-8601 UTC string like \"2024-07-20T14:30:00Z\"; epoch-millis gives the raw millisecond count like 1721485800000. Ignored when converting to Extended JSON." },
                    "detect_types": { "type": "boolean", "default": false, "description": "When converting plain JSON to Extended JSON, promote strings that look like values: a 24-character hex string becomes {\"$oid\": ...} and a full ISO-8601 date-time becomes {\"$date\": ...}. Off by default because the guess can be wrong for ordinary hex or date-like strings." },
                    "big_numbers_as_strings": { "type": "boolean", "default": false, "description": "When converting to plain JSON, emit 64-bit integers ($numberLong) and Decimal128 values ($numberDecimal) as JSON strings instead of numbers, so values beyond JavaScript's safe-integer range keep full precision. Infinity and NaN are always strings because JSON has no literal for them." },
                    "pretty": { "type": "boolean", "default": true, "description": "Pretty-print (indent) the JSON output. Set false for compact single-line output." }
                },
                "required": ["input"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }

    #[test]
    fn build_options_rejects_a_bad_enum_value() {
        let a = Args {
            input: "{}".to_string(),
            direction: "auto".to_string(),
            mode: "strict".to_string(),
            date_format: "iso".to_string(),
            detect_types: false,
            big_numbers_as_strings: false,
            pretty: true,
        };
        assert!(build_options(&a).unwrap_err().contains("invalid mode"));
    }
}
