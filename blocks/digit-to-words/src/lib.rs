//! gizza-ai/digit-to-words — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the CLI);
//! handle() delegates to block_utils::run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    input: String,
    #[serde(default = "default_style")]
    style: String,
    #[serde(default = "default_scale")]
    scale: String,
    #[serde(default = "default_case")]
    letter_case: String,
    #[serde(default = "default_currency")]
    currency: String,
    #[serde(default)]
    use_and: bool,
    #[serde(default = "default_true")]
    hyphenate: bool,
    #[serde(default = "default_decimals")]
    decimals: String,
    #[serde(default)]
    only_suffix: bool,
}

fn default_style() -> String {
    "cardinal".into()
}
fn default_scale() -> String {
    "short".into()
}
fn default_case() -> String {
    "lower".into()
}
fn default_currency() -> String {
    "USD".into()
}
fn default_decimals() -> String {
    "point".into()
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("input").required().describe("The number or numbers to spell out, one per line (up to 1,000 lines and 100,000 bytes). Digits are read exactly as written, so a 66-digit integer or a money amount never picks up floating-point noise. Accepts thousands separators (1,234 / 1 234 / 1_234), a comma decimal point (1234,5), currency symbols, a leading minus, 'minus'/'negative' words, accounting parentheses such as (1234.50) for negatives, and scientific notation such as 1.5e3."))
        .param(Param::enumv("style", ["cardinal", "ordinal", "ordinal_num", "year", "currency", "check"]).default("cardinal").describe("How to read the number. cardinal (default) gives 'one thousand two hundred thirty-four'; ordinal gives 'one thousand two hundred thirty-fourth'; ordinal_num keeps the digits and spells only the suffix, '1234th'; year reads four digits the spoken way, 1984 → 'nineteen eighty-four'; currency gives 'twenty-five dollars and forty cents'; check gives the cheque/invoice form 'twenty-five and 40/100 dollars'."))
        .param(Param::enumv("scale", ["short", "long", "indian"]).default("short").describe("Naming scale for large numbers. short (default, US/modern UK) makes 10^9 'one billion'; long (continental European) makes 10^9 'one milliard' and 10^12 'one billion'; indian uses thousand/lakh/crore/arab grouping, so 100000 becomes 'one lakh'. Maximum size: 66 digits on short/long, 21 digits on indian."))
        .param(Param::enumv("letter_case", ["lower", "upper", "title", "sentence"]).default("lower").describe("Letter case of the result. lower (default) 'twenty-one'; upper 'TWENTY-ONE'; title 'Twenty-One'; sentence 'Twenty-one'. Use sentence or title for cheques and invoices."))
        .param(Param::enumv("currency", gizza_ai_digit_to_words_core::CURRENCY_CODES).default("USD").describe("ISO currency code used by the currency and check styles; ignored by every other style. Default USD (dollars and cents). Each code carries its own unit and sub-unit words — GBP gives pounds and pence, INR rupees and paise, SEK kronor and öre — and the zero-decimal currencies JPY and KRW have no sub-unit, so amounts are rounded to whole units."))
        .param(Param::boolean("use_and").default(false).describe("Insert the British 'and' before the last group under one hundred: 1023 reads 'one thousand and twenty-three' and 1234 reads 'one thousand two hundred and thirty-four'. Default false (American style, no 'and')."))
        .param(Param::boolean("hyphenate").default(true).describe("Hyphenate compound tens, so 21 reads 'twenty-one'. Default true, which is standard English spelling. Set false for 'twenty one' when the result feeds a system that cannot take hyphens."))
        .param(Param::enumv("decimals", ["point", "ignore", "round"]).default("point").describe("What to do with a fractional part on the cardinal, ordinal, ordinal_num and year styles. point (default) reads it digit by digit, 1.50 → 'one point five zero'; ignore truncates toward zero, 2.9 → 'two'; round rounds half away from zero to a whole number, 2.5 → 'three'. The currency and check styles always round to the currency's sub-unit instead."))
        .param(Param::boolean("only_suffix").default(false).describe("Append the word 'only' to the result, the usual terminator on cheques and invoices: 'one lakh twenty-five thousand and 00/100 rupees only'. Default false."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/digit-to-words",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Spell numbers out in English words",
    skill(
        description = "Spell numbers out as English words — cardinals ('one thousand two hundred thirty-four'), ordinals ('twenty-first' or '21st'), spoken years ('nineteen eighty-four'), money ('twenty-five dollars and forty cents') and the cheque form ('twenty-five and 40/100 dollars'). Reads thousands separators, comma decimal points, currency symbols, accounting parentheses and scientific notation, and works on digit strings rather than floats so a 66-digit integer spells exactly. Supports short, long and Indian (lakh/crore) scales, 25 currencies, British 'and', optional hyphens, lower/UPPER/Title/Sentence case, an 'only' suffix, and batches of up to 1,000 numbers one per line.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "digit-to-words", |a: Args| {
            gizza_ai_digit_to_words_core::convert(
                &a.input,
                &a.style,
                &a.scale,
                &a.letter_case,
                &a.currency,
                a.use_and,
                a.hyphenate,
                &a.decimals,
                a.only_suffix,
            )
            .map_err(SkillError::InvalidArgs)
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
            r##"{
                "type": "object",
                "properties": {
                    "input": { "type": "string", "description": "The number or numbers to spell out, one per line (up to 1,000 lines and 100,000 bytes). Digits are read exactly as written, so a 66-digit integer or a money amount never picks up floating-point noise. Accepts thousands separators (1,234 / 1 234 / 1_234), a comma decimal point (1234,5), currency symbols, a leading minus, 'minus'/'negative' words, accounting parentheses such as (1234.50) for negatives, and scientific notation such as 1.5e3." },
                    "style": { "type": "string", "enum": ["cardinal","ordinal","ordinal_num","year","currency","check"], "default": "cardinal", "description": "How to read the number. cardinal (default) gives 'one thousand two hundred thirty-four'; ordinal gives 'one thousand two hundred thirty-fourth'; ordinal_num keeps the digits and spells only the suffix, '1234th'; year reads four digits the spoken way, 1984 → 'nineteen eighty-four'; currency gives 'twenty-five dollars and forty cents'; check gives the cheque/invoice form 'twenty-five and 40/100 dollars'." },
                    "scale": { "type": "string", "enum": ["short","long","indian"], "default": "short", "description": "Naming scale for large numbers. short (default, US/modern UK) makes 10^9 'one billion'; long (continental European) makes 10^9 'one milliard' and 10^12 'one billion'; indian uses thousand/lakh/crore/arab grouping, so 100000 becomes 'one lakh'. Maximum size: 66 digits on short/long, 21 digits on indian." },
                    "letter_case": { "type": "string", "enum": ["lower","upper","title","sentence"], "default": "lower", "description": "Letter case of the result. lower (default) 'twenty-one'; upper 'TWENTY-ONE'; title 'Twenty-One'; sentence 'Twenty-one'. Use sentence or title for cheques and invoices." },
                    "currency": { "type": "string", "enum": ["USD","EUR","GBP","JPY","INR","CAD","AUD","CHF","CNY","RUB","BRL","MXN","ZAR","NGN","KRW","NZD","SGD","HKD","SEK","NOK","DKK","PLN","TRY","PHP","THB"], "default": "USD", "description": "ISO currency code used by the currency and check styles; ignored by every other style. Default USD (dollars and cents). Each code carries its own unit and sub-unit words — GBP gives pounds and pence, INR rupees and paise, SEK kronor and öre — and the zero-decimal currencies JPY and KRW have no sub-unit, so amounts are rounded to whole units." },
                    "use_and": { "type": "boolean", "default": false, "description": "Insert the British 'and' before the last group under one hundred: 1023 reads 'one thousand and twenty-three' and 1234 reads 'one thousand two hundred and thirty-four'. Default false (American style, no 'and')." },
                    "hyphenate": { "type": "boolean", "default": true, "description": "Hyphenate compound tens, so 21 reads 'twenty-one'. Default true, which is standard English spelling. Set false for 'twenty one' when the result feeds a system that cannot take hyphens." },
                    "decimals": { "type": "string", "enum": ["point","ignore","round"], "default": "point", "description": "What to do with a fractional part on the cardinal, ordinal, ordinal_num and year styles. point (default) reads it digit by digit, 1.50 → 'one point five zero'; ignore truncates toward zero, 2.9 → 'two'; round rounds half away from zero to a whole number, 2.5 → 'three'. The currency and check styles always round to the currency's sub-unit instead." },
                    "only_suffix": { "type": "boolean", "default": false, "description": "Append the word 'only' to the result, the usual terminator on cheques and invoices: 'one lakh twenty-five thousand and 00/100 rupees only'. Default false." }
                },
                "required": ["input"],
                "additionalProperties": false
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
