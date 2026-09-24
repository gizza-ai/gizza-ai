//! gizza-ai/matrix-text-parser — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the CLI);
//! handle() delegates to block_utils::run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::{Deserialize, Serialize};
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    matrix: String,
    #[serde(default = "default_auto")]
    input_format: String,
    #[serde(default = "default_auto")]
    delimiter: String,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default = "default_auto")]
    cells: String,
    #[serde(default = "default_true")]
    fractions: bool,
    #[serde(default)]
    header: bool,
    #[serde(default = "default_ragged")]
    ragged: String,
    #[serde(default = "default_fill")]
    fill: String,
    #[serde(default = "default_indent")]
    indent: f64,
}

fn default_auto() -> String {
    "auto".into()
}
fn default_output() -> String {
    "json".into()
}
fn default_ragged() -> String {
    "error".into()
}
fn default_fill() -> String {
    "0".into()
}
fn default_true() -> bool {
    true
}
fn default_indent() -> f64 {
    2.0
}

#[derive(Serialize)]
struct Resp {
    result: String,
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("matrix").required().describe("The matrix text to parse, in any common form: delimited rows such as '1 2 3' or '1,2,3' one per line, a Python/NumPy nested list such as [[1,2],[3,4]] (np.array(...) wrappers are stripped), MATLAB/Octave '[1 2; 3 4]', or a LaTeX bmatrix/pmatrix/array environment. Up to 2,000,000 bytes, 10,000 rows, 2,000 columns and 1,000,000 cells."))
        .param(Param::enumv("input_format", ["auto", "delimited", "python", "matlab", "latex"]).default("auto").describe("Input syntax. auto (default) detects LaTeX environments, nested Python/NumPy lists, MATLAB semicolon rows and plain delimited lines. Set it explicitly when a paste is ambiguous, for example forcing delimited on a single line that contains semicolons."))
        .param(Param::enumv("delimiter", ["auto", "comma", "space", "tab", "semicolon", "pipe"]).default("auto").describe("Cell separator for delimited rows, ignored by the python/matlab/latex syntaxes. auto (default) picks the first of tab, comma, semicolon or pipe that appears, else splits on whitespace."))
        .param(Param::enumv("output", ["json", "array", "csv", "tsv", "matlab", "numpy", "latex", "aligned"]).default("json").describe("Output format. json (default) returns the 2D array plus shape, rows, columns, square, numeric and the detected syntax; array returns the bare 2D JSON array; csv/tsv re-emit delimited rows; matlab gives '[1 2; 3 4]'; numpy gives np.array([[1, 2], [3, 4]]); latex gives a bmatrix environment; aligned gives column-aligned plain text."))
        .param(Param::enumv("cells", ["auto", "number", "text"]).default("auto").describe("Cell typing. auto (default) makes every parseable cell a JSON number and keeps the rest as strings; number fails with the row and column of the first non-numeric cell; text keeps every cell as a string so IDs with leading zeros survive."))
        .param(Param::boolean("fractions").default(true).describe("Evaluate plain fractions such as 3/4 or -1/8 into decimal numbers. Default true. Turn it off to keep them as text. Arithmetic expressions are never evaluated."))
        .param(Param::boolean("header").default(false).describe("Treat the first row as column labels instead of data. Default false. When true the labels are returned in the json output's 'header' field, re-emitted as the first csv/tsv/aligned row, and written as a comment line above matlab/numpy/latex output."))
        .param(Param::enumv("ragged", ["error", "pad", "trim"]).default("error").describe("What to do when rows have different lengths. error (default) reports the first mismatching row; pad extends short rows with the fill value; trim cuts every row to the shortest length."))
        .param(Param::string("fill").default("0").describe("Cell written into short rows when ragged=pad, for example 0, null or n/a. Default '0'. Ignored for ragged=error and ragged=trim."))
        .param(Param::number("indent").min(0.0).max(8.0).default(2.0).describe("JSON indentation in spaces for the json and array outputs, from 0 for minified to 8. Default 2. Ignored by the text output formats."))
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/matrix-text-parser",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Parse a pasted matrix into a normalized 2D array",
    skill(
        description = "Parse a matrix pasted in any common form into a normalized 2D array with its shape. Accepts space/comma/tab/semicolon/pipe delimited rows, Python and NumPy nested lists (including np.array(...) wrappers), MATLAB/Octave semicolon syntax, and LaTeX bmatrix/pmatrix/array environments, auto-detecting which one was pasted. Reports rows, columns, whether the matrix is square and whether every cell is numeric; evaluates fractions and scientific notation; can pad or trim ragged rows, split off a header row, and re-emit the result as JSON, CSV, TSV, MATLAB, NumPy, LaTeX or column-aligned text.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "matrix-text-parser", |a: Args| {
            let result = gizza_ai_matrix_text_parser_core::parse_matrix(
                &a.matrix,
                &a.input_format,
                &a.delimiter,
                &a.output,
                &a.cells,
                a.fractions,
                a.header,
                &a.ragged,
                &a.fill,
                a.indent,
            )
            .map_err(SkillError::InvalidArgs)?;
            Ok(Resp { result })
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
                    "matrix": { "type": "string", "description": "The matrix text to parse, in any common form: delimited rows such as '1 2 3' or '1,2,3' one per line, a Python/NumPy nested list such as [[1,2],[3,4]] (np.array(...) wrappers are stripped), MATLAB/Octave '[1 2; 3 4]', or a LaTeX bmatrix/pmatrix/array environment. Up to 2,000,000 bytes, 10,000 rows, 2,000 columns and 1,000,000 cells." },
                    "input_format": { "type": "string", "enum": ["auto","delimited","python","matlab","latex"], "default": "auto", "description": "Input syntax. auto (default) detects LaTeX environments, nested Python/NumPy lists, MATLAB semicolon rows and plain delimited lines. Set it explicitly when a paste is ambiguous, for example forcing delimited on a single line that contains semicolons." },
                    "delimiter": { "type": "string", "enum": ["auto","comma","space","tab","semicolon","pipe"], "default": "auto", "description": "Cell separator for delimited rows, ignored by the python/matlab/latex syntaxes. auto (default) picks the first of tab, comma, semicolon or pipe that appears, else splits on whitespace." },
                    "output": { "type": "string", "enum": ["json","array","csv","tsv","matlab","numpy","latex","aligned"], "default": "json", "description": "Output format. json (default) returns the 2D array plus shape, rows, columns, square, numeric and the detected syntax; array returns the bare 2D JSON array; csv/tsv re-emit delimited rows; matlab gives '[1 2; 3 4]'; numpy gives np.array([[1, 2], [3, 4]]); latex gives a bmatrix environment; aligned gives column-aligned plain text." },
                    "cells": { "type": "string", "enum": ["auto","number","text"], "default": "auto", "description": "Cell typing. auto (default) makes every parseable cell a JSON number and keeps the rest as strings; number fails with the row and column of the first non-numeric cell; text keeps every cell as a string so IDs with leading zeros survive." },
                    "fractions": { "type": "boolean", "default": true, "description": "Evaluate plain fractions such as 3/4 or -1/8 into decimal numbers. Default true. Turn it off to keep them as text. Arithmetic expressions are never evaluated." },
                    "header": { "type": "boolean", "default": false, "description": "Treat the first row as column labels instead of data. Default false. When true the labels are returned in the json output's 'header' field, re-emitted as the first csv/tsv/aligned row, and written as a comment line above matlab/numpy/latex output." },
                    "ragged": { "type": "string", "enum": ["error","pad","trim"], "default": "error", "description": "What to do when rows have different lengths. error (default) reports the first mismatching row; pad extends short rows with the fill value; trim cuts every row to the shortest length." },
                    "fill": { "type": "string", "default": "0", "description": "Cell written into short rows when ragged=pad, for example 0, null or n/a. Default '0'. Ignored for ragged=error and ragged=trim." },
                    "indent": { "type": "number", "minimum": 0, "maximum": 8, "default": 2.0, "description": "JSON indentation in spaces for the json and array outputs, from 0 for minified to 8. Default 2. Ignored by the text output formats." }
                },
                "required": ["matrix"],
                "additionalProperties": false
            }"##,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
