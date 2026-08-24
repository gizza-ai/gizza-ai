//! gizza-ai/stl-format-converter — chat skill block on the shared tool abstraction.
//! Converts an STL mesh between binary and ASCII encodings in EITHER direction:
//! binary STL bytes (pasted as base64 or hex) become readable ASCII STL text,
//! and ASCII STL text becomes a compact binary STL. The chat schema is
//! single-sourced from descriptor() (which also drives the CLI); handle()
//! delegates to block_utils::run_skill. Pure → runs on all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_stl_format_converter_core::{
    convert, InputFormat, NumberFormat, Normals, Options, Output, OutputEncoding, Target,
};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    stl: String,
    #[serde(default = "default_input_format")]
    input_format: String,
    #[serde(default = "default_to")]
    to: String,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default)]
    solid_name: String,
    #[serde(default = "default_normals")]
    normals: String,
    #[serde(default = "default_number_format")]
    number_format: String,
    #[serde(default = "default_precision")]
    precision: f64,
    #[serde(default = "default_output_encoding")]
    output_encoding: String,
}

fn default_input_format() -> String {
    "auto".to_string()
}
fn default_to() -> String {
    "auto".to_string()
}
fn default_output() -> String {
    "stl".to_string()
}
fn default_normals() -> String {
    "keep".to_string()
}
fn default_number_format() -> String {
    "scientific".to_string()
}
fn default_precision() -> f64 {
    6.0
}
fn default_output_encoding() -> String {
    "data-url".to_string()
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("stl")
                .required()
                .describe(
                    "The STL mesh to re-encode. Paste an ASCII STL as text (solid / facet normal \
                     / outer loop / vertex lines), or a BINARY STL's bytes encoded as base64 or \
                     hex (hex may include spaces, colons or dashes; a data:model/stl;base64 URL \
                     is accepted too). Binary STL is not text, so it cannot be pasted directly. \
                     Up to 100000 triangles.",
                ),
        )
        .param(
            Param::enumv("input_format", ["auto", "ascii", "base64", "hex"])
                .default("auto")
                .describe(
                    "How the pasted value is encoded. 'auto' (default) detects ASCII STL text, \
                     hex bytes or base64 bytes; binary STL is recognised by its 84 + 50 x \
                     triangle-count byte length rather than the leading 'solid' keyword, which \
                     many exporters also write into a binary header. Set 'ascii', 'base64' or \
                     'hex' to force one.",
                ),
        )
        .param(
            Param::enumv("to", ["auto", "ascii", "binary"])
                .default("auto")
                .describe(
                    "Which encoding to write. 'auto' (default) flips to the other one — binary \
                     in gives ASCII out, ASCII in gives binary out. 'ascii' always returns \
                     readable STL text; 'binary' always returns binary STL bytes (see \
                     output_encoding). Choosing the encoding the input already uses re-writes \
                     the file in place, which is how you renormalise the solid name, facet \
                     normals or number formatting.",
                ),
        )
        .param(
            Param::enumv("output", ["stl", "summary"])
                .default("stl")
                .describe(
                    "What to return. 'stl' (default) is the converted mesh itself. 'summary' is \
                     a short conversion report instead: the detected input encoding, triangle \
                     count, solid name, input and output sizes with the size change, how normals \
                     were handled, and whether the file carried per-triangle attribute bytes \
                     (VisCAM/SolidView colour) that ASCII STL cannot store.",
                ),
        )
        .param(
            Param::string("solid_name")
                .default("")
                .describe(
                    "Name written into the output: the ASCII 'solid'/'endsolid' name, or the \
                     binary 80-byte header. Leave blank (default) to carry the source file's own \
                     name through unchanged. A binary STL must never start with the word \
                     'solid', so a name beginning with it is written as 'STL <name>' in binary \
                     headers.",
                ),
        )
        .param(
            Param::enumv("normals", ["keep", "recompute", "zero"])
                .default("keep")
                .describe(
                    "What to do with each facet's stored normal vector. 'keep' (default) copies \
                     it through untouched, so the conversion is lossless. 'recompute' derives it \
                     from the triangle's own winding by the right-hand rule, which fixes normals \
                     an exporter got wrong. 'zero' writes 0 0 0, the convention meaning 'no \
                     normal declared — use the vertex order'.",
                ),
        )
        .param(
            Param::enumv("number_format", ["scientific", "decimal"])
                .default("scientific")
                .describe(
                    "How ASCII coordinates are written (ignored for binary output). \
                     'scientific' (default) uses the STL specification's \
                     sign-mantissa-e-sign-exponent shape, e.g. 2.648000e-002, which is what CAD \
                     exporters write. 'decimal' writes plain numbers such as 0.002648 with \
                     trailing zeros trimmed — easier to read and diff, but it rounds values far \
                     from 1 to the chosen number of decimal places.",
                ),
        )
        .param(
            Param::integer("precision")
                .default(6)
                .min(0.0)
                .max(17.0)
                .describe(
                    "Decimal places used for ASCII coordinates (default 6, matching common CAD \
                     exporters; ignored for binary output). STL coordinates are 32-bit floats, \
                     so precision 9 with number_format=scientific is the smallest setting that \
                     reproduces every value exactly — use it when you plan to convert back to \
                     binary and want a byte-identical file.",
                ),
        )
        .param(
            Param::enumv("output_encoding", ["data-url", "base64", "hex"])
                .default("data-url")
                .describe(
                    "How binary STL output is handed back, since binary is not text (ignored for \
                     ASCII output). 'data-url' (default) returns a data:model/stl;base64,… URL \
                     you can save straight to a .stl file. 'base64' and 'hex' return the raw \
                     encoded bytes, which is what you want when piping the result into another \
                     tool.",
                ),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

fn run_convert(a: Args) -> Result<String, SkillError> {
    if !(0.0..=17.0).contains(&a.precision) || a.precision.fract() != 0.0 {
        return Err(SkillError::InvalidArgs(format!(
            "precision must be a whole number from 0 to 17, got {}",
            a.precision
        )));
    }
    let opt = Options {
        input_format: InputFormat::parse(&a.input_format).map_err(SkillError::InvalidArgs)?,
        to: Target::parse(&a.to).map_err(SkillError::InvalidArgs)?,
        output_encoding: OutputEncoding::parse(&a.output_encoding)
            .map_err(SkillError::InvalidArgs)?,
        solid_name: a.solid_name,
        normals: Normals::parse(&a.normals).map_err(SkillError::InvalidArgs)?,
        precision: a.precision as u32,
        number_format: NumberFormat::parse(&a.number_format).map_err(SkillError::InvalidArgs)?,
        output: Output::parse(&a.output).map_err(SkillError::InvalidArgs)?,
    };
    convert(&a.stl, &opt).map_err(SkillError::InvalidArgs)
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/stl-format-converter",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert an STL mesh between binary and ASCII encodings, in either direction",
    skill(
        description = "Convert an STL mesh between its binary and ASCII encodings, in either direction. Binary STL is not text, so paste its bytes as base64 or hex (a data:model/stl;base64 URL works too) and get readable ASCII STL back; paste ASCII STL text and get a compact binary STL back as a data:model/stl;base64 URL (or raw base64/hex via output_encoding). The input encoding is auto-detected by its 84 + 50 x triangle-count byte length rather than the leading 'solid' keyword, which binary headers also carry. to=auto (default) flips to the other encoding; set to=ascii or to=binary to force one, including re-writing a file in its own encoding to renormalise it. Geometry is carried as the 32-bit floats binary STL stores, so nothing is welded, repaired, scaled or re-ordered; normals=keep (default) copies stored facet normals through untouched, and number_format=scientific with precision=9 makes a binary -> ASCII -> binary round trip byte-identical. solid_name overwrites the ASCII solid name / binary 80-byte header (blank keeps the source's own), and output=summary returns a conversion report — encoding, triangle count, sizes and size change, and any per-triangle VisCAM/SolidView colour attribute bytes that ASCII STL cannot store. Up to 100000 triangles. Runs fully locally, no network access.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "stl-format-converter", run_convert) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(stl: &str) -> Args {
        Args {
            stl: stl.to_string(),
            input_format: default_input_format(),
            to: default_to(),
            output: default_output(),
            solid_name: String::new(),
            normals: default_normals(),
            number_format: default_number_format(),
            precision: default_precision(),
            output_encoding: default_output_encoding(),
        }
    }

    const TRI: &str = "solid demo\n  facet normal 0 0 1\n    outer loop\n      vertex 0 0 0\n      \
                       vertex 1 0 0\n      vertex 0 1 0\n    endloop\n  endfacet\nendsolid demo\n";

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r##"{
                "type": "object",
                "properties": {
                    "stl": { "type": "string", "description": "The STL mesh to re-encode. Paste an ASCII STL as text (solid / facet normal / outer loop / vertex lines), or a BINARY STL's bytes encoded as base64 or hex (hex may include spaces, colons or dashes; a data:model/stl;base64 URL is accepted too). Binary STL is not text, so it cannot be pasted directly. Up to 100000 triangles." },
                    "input_format": { "type": "string", "enum": ["auto", "ascii", "base64", "hex"], "default": "auto", "description": "How the pasted value is encoded. 'auto' (default) detects ASCII STL text, hex bytes or base64 bytes; binary STL is recognised by its 84 + 50 x triangle-count byte length rather than the leading 'solid' keyword, which many exporters also write into a binary header. Set 'ascii', 'base64' or 'hex' to force one." },
                    "to": { "type": "string", "enum": ["auto", "ascii", "binary"], "default": "auto", "description": "Which encoding to write. 'auto' (default) flips to the other one — binary in gives ASCII out, ASCII in gives binary out. 'ascii' always returns readable STL text; 'binary' always returns binary STL bytes (see output_encoding). Choosing the encoding the input already uses re-writes the file in place, which is how you renormalise the solid name, facet normals or number formatting." },
                    "output": { "type": "string", "enum": ["stl", "summary"], "default": "stl", "description": "What to return. 'stl' (default) is the converted mesh itself. 'summary' is a short conversion report instead: the detected input encoding, triangle count, solid name, input and output sizes with the size change, how normals were handled, and whether the file carried per-triangle attribute bytes (VisCAM/SolidView colour) that ASCII STL cannot store." },
                    "solid_name": { "type": "string", "default": "", "description": "Name written into the output: the ASCII 'solid'/'endsolid' name, or the binary 80-byte header. Leave blank (default) to carry the source file's own name through unchanged. A binary STL must never start with the word 'solid', so a name beginning with it is written as 'STL <name>' in binary headers." },
                    "normals": { "type": "string", "enum": ["keep", "recompute", "zero"], "default": "keep", "description": "What to do with each facet's stored normal vector. 'keep' (default) copies it through untouched, so the conversion is lossless. 'recompute' derives it from the triangle's own winding by the right-hand rule, which fixes normals an exporter got wrong. 'zero' writes 0 0 0, the convention meaning 'no normal declared — use the vertex order'." },
                    "number_format": { "type": "string", "enum": ["scientific", "decimal"], "default": "scientific", "description": "How ASCII coordinates are written (ignored for binary output). 'scientific' (default) uses the STL specification's sign-mantissa-e-sign-exponent shape, e.g. 2.648000e-002, which is what CAD exporters write. 'decimal' writes plain numbers such as 0.002648 with trailing zeros trimmed — easier to read and diff, but it rounds values far from 1 to the chosen number of decimal places." },
                    "precision": { "type": "integer", "default": 6, "minimum": 0, "maximum": 17, "description": "Decimal places used for ASCII coordinates (default 6, matching common CAD exporters; ignored for binary output). STL coordinates are 32-bit floats, so precision 9 with number_format=scientific is the smallest setting that reproduces every value exactly — use it when you plan to convert back to binary and want a byte-identical file." },
                    "output_encoding": { "type": "string", "enum": ["data-url", "base64", "hex"], "default": "data-url", "description": "How binary STL output is handed back, since binary is not text (ignored for ASCII output). 'data-url' (default) returns a data:model/stl;base64,… URL you can save straight to a .stl file. 'base64' and 'hex' return the raw encoded bytes, which is what you want when piping the result into another tool." }
                },
                "required": ["stl"],
                "additionalProperties": false
            }"##,
        )
        .unwrap();
        let live: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(live, authored);
    }

    #[test]
    fn every_param_is_described() {
        let live: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let props = live["properties"].as_object().unwrap();
        assert_eq!(props.len(), 9);
        for (name, p) in props {
            let d = p["description"].as_str().unwrap_or("");
            assert!(d.len() > 40, "param '{name}' needs a real description");
        }
    }

    #[test]
    fn run_convert_flips_ascii_to_binary_by_default() {
        let out = run_convert(args(TRI)).unwrap();
        assert!(out.starts_with("data:model/stl;base64,"), "got {out}");
    }

    #[test]
    fn run_convert_flips_binary_back_to_ascii() {
        let binary = run_convert(Args {
            output_encoding: "base64".to_string(),
            ..args(TRI)
        })
        .unwrap();
        let ascii = run_convert(args(&binary)).unwrap();
        assert!(ascii.starts_with("solid demo\n"), "got {ascii}");
        assert!(ascii.contains("facet normal 0.000000e+000 0.000000e+000 1.000000e+000"));
    }

    #[test]
    fn run_convert_surfaces_parse_errors() {
        let err = run_convert(Args {
            input_format: "ascii".to_string(),
            ..args("not a mesh")
        })
        .unwrap_err();
        assert!(format!("{err:?}").contains("no facets found"), "got {err:?}");
    }

    #[test]
    fn run_convert_rejects_a_fractional_precision() {
        let err = run_convert(Args {
            precision: 6.5,
            ..args(TRI)
        })
        .unwrap_err();
        assert!(format!("{err:?}").contains("whole number"), "got {err:?}");
    }
}
