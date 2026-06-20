//! gizza-ai/csv-formula-eval — add/transform CSV columns via arithmetic formulas.
//! Thin wrapper around the core (meval); chat schema single-sourced from
//! descriptor(); handler delegates to run_skill. Pure.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_csv_formula_eval_core::eval;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    formulas: String,
    #[serde(default)]
    delimiter: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("data").required().describe("The CSV text (first row is a header)."))
        .param(Param::string("formulas").required().describe("One or more '<column> = <expression>' formulas, separated by ';' or newlines. Expressions reference other columns by their (identifier-like) header name and support + - * / %, ^, parentheses, and functions like sqrt/abs/min/max/round. A target naming an existing column replaces it; a new name appends a column. Formulas run left-to-right so later ones can use earlier results."))
        .param(Param::string("delimiter").default(",").describe("Field separator: a single char or 'comma'/'tab'/'semicolon'/'pipe'. Default ','."))
}

fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct CsvFormulaEval;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/csv-formula-eval",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Add/transform CSV columns with formulas",
    skill(
        description = "Evaluate spreadsheet-style arithmetic formulas to add or transform CSV columns. `formulas` is one or more '<column> = <expression>' (separated by ';' or newlines); expressions reference other columns by header name and support + - * / %, ^, parentheses, and functions (sqrt, abs, min, max, round, etc.). A target naming an existing column replaces it; a new name appends one. Formulas run left-to-right. Non-numeric cells make a referencing formula blank for that row. First row must be a header with identifier-like column names.",
        parameters = schema_json()
    )
)]
impl CsvFormulaEval {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "csv-formula-eval", |a: Args| {
            let delim = if a.delimiter.is_empty() { ",".to_string() } else { a.delimiter };
            eval(&a.data, &a.formulas, &delim).map_err(SkillError::InvalidArgs)
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
                    "data":      { "type": "string", "description": "The CSV text (first row is a header)." },
                    "formulas":  { "type": "string", "description": "One or more '<column> = <expression>' formulas, separated by ';' or newlines. Expressions reference other columns by their (identifier-like) header name and support + - * / %, ^, parentheses, and functions like sqrt/abs/min/max/round. A target naming an existing column replaces it; a new name appends a column. Formulas run left-to-right so later ones can use earlier results." },
                    "delimiter": { "type": "string", "default": ",", "description": "Field separator: a single char or 'comma'/'tab'/'semicolon'/'pipe'. Default ','." }
                },
                "required": ["data", "formulas"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
