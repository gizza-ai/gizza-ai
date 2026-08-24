//! gizza-ai/docstring-stub-generator — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the CLI);
//! handle() delegates to block_utils::run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_docstring_stub_generator_core::generate;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    signature: String,
    #[serde(default = "default_auto")]
    language: String,
    #[serde(default = "default_auto")]
    style: String,
    #[serde(default = "default_annotated")]
    output: String,
    #[serde(default = "default_guess")]
    types: String,
    #[serde(default = "default_placeholder")]
    placeholder: String,
    #[serde(default)]
    raises: String,
    #[serde(default = "default_double")]
    quote_style: String,
    #[serde(default)]
    extended_summary: bool,
    #[serde(default)]
    examples: bool,
    #[serde(default)]
    align_tags: bool,
    #[serde(default = "default_indent")]
    indent_size: i64,
}
fn default_auto() -> String {
    "auto".to_string()
}
fn default_annotated() -> String {
    "annotated".to_string()
}
fn default_guess() -> String {
    "guess".to_string()
}
fn default_placeholder() -> String {
    "_description_".to_string()
}
fn default_double() -> String {
    "double".to_string()
}
fn default_indent() -> i64 {
    4
}

/// Every value the `language` param accepts — shared by the descriptor so the
/// chat schema, the CLI and the page `<select>` can never drift apart.
const LANGUAGES: [&str; 10] = [
    "auto",
    "python",
    "javascript",
    "typescript",
    "php",
    "java",
    "csharp",
    "go",
    "rust",
    "ruby",
];

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("signature")
                .required()
                .describe("The function signature(s) to document, pasted as text. One per run or several at once; a signature may span multiple lines. Pasting a whole function works too — the body is passed through untouched. Example: 'def fetch(url: str, timeout: int = 30) -> dict:'."),
        )
        .param(
            Param::enumv("language", LANGUAGES)
                .default("auto")
                .describe("Signature syntax and documentation convention. 'auto' (default) guesses from the code's shape. Named values: python (docstring), javascript/typescript (JSDoc), php (PHPDoc), java (Javadoc), csharp (XML doc comments), go (godoc), rust (rustdoc), ruby (YARD). Set it explicitly when auto-detection guesses wrong — e.g. force 'ruby' so `def f(a)` is not read as Python."),
        )
        .param(
            Param::enumv("style", ["auto", "google", "numpy", "sphinx", "epytext", "pep257"])
                .default("auto")
                .describe("Python docstring convention: 'auto' (default) = google. 'google' = 'Args:'/'Returns:' sections; 'numpy' = underlined 'Parameters'/'Returns' sections; 'sphinx' = ':param x:'/':type x:'/':rtype:' fields; 'epytext' = '@param'/'@type'/'@rtype' fields; 'pep257' = plain 'Arguments:' with 'name -- description'. Ignored for the other languages, which each have exactly one native convention (JSDoc, PHPDoc, Javadoc, XML doc, godoc, rustdoc, YARD)."),
        )
        .param(
            Param::enumv("output", ["annotated", "docstring", "json"])
                .default("annotated")
                .describe("What to return. 'annotated' (default) = the pasted input with each stub inserted in place (inside the def for Python, above the signature otherwise) — copy-paste ready. 'docstring' = only the generated stub blocks, blank-line separated. 'json' = the parsed signatures as {name, async, params:[{name,type,default,optional,variadic}], returns, raises}."),
        )
        .param(
            Param::enumv("types", ["guess", "annotated", "none"])
                .default("guess")
                .describe("Where parameter and return types come from. 'guess' (default) = the declared annotation, else inferred from the default value (timeout=30 → int, name=\"x\" → str), else a placeholder such as _type_. 'annotated' = only types actually declared in the signature; nothing is written when there is none. 'none' = omit type slots entirely (the '-notypes' shape)."),
        )
        .param(
            Param::string("placeholder")
                .default("_description_")
                .describe("Filler text written into every description slot for you to replace. Default '_description_' (the editor-extension convention). Try 'FIXME' to make unfilled stubs easy to grep for. Blank falls back to the default."),
        )
        .param(
            Param::string("raises")
                .default("")
                .describe("Exception/error names to document, comma- or space-separated — e.g. 'ValueError, KeyError'. Default empty (no raises section). A signature alone does not say what it throws, so these are declared by you; Java `throws` clauses are picked up automatically and merged in. Capped at 20 names."),
        )
        .param(
            Param::enumv("quote_style", ["double", "single"])
                .default("double")
                .describe("Python docstring quotes: 'double' (default) = \"\"\", 'single' = '''. Ignored for every other language, which use comment blocks rather than string literals."),
        )
        .param(
            Param::boolean("extended_summary")
                .default(false)
                .describe("Add a second placeholder paragraph under the summary line for a longer description. Default false."),
        )
        .param(
            Param::boolean("examples")
                .default(false)
                .describe("Add an examples section in the language's convention — 'Examples:' with a >>> line for Python, '@example' for JSDoc/PHPDoc/YARD, a '# Examples' fenced block for rustdoc, '<example>' for XML doc, 'Example:' for godoc. Default false."),
        )
        .param(
            Param::boolean("align_tags")
                .default(false)
                .describe("Pad the type and name columns of tag-style blocks so every description starts at the same column (JSDoc, PHPDoc, Javadoc). Default false, which leaves the tags ragged. Has no effect on Python, godoc, rustdoc or XML doc comments."),
        )
        .param(
            Param::integer("indent_size")
                .default(4)
                .min(0.0)
                .max(8.0)
                .describe("Spaces used for indentation inside the generated stub, and for the Python docstring's offset from the `def` line. Default 4; use 2 for 2-space codebases. Range 0-8."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct DocstringStubGenerator;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/docstring-stub-generator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Generate docstring, JSDoc and Javadoc stubs from a pasted function signature",
    skill(
        description = "Turn a pasted function signature into a ready-to-fill documentation stub with parameter, return and raises slots. Covers Python (Google, NumPy, Sphinx/reST, Epytext and PEP 257 conventions), JavaScript/TypeScript (JSDoc), PHP (PHPDoc), Java (Javadoc), C# (XML doc comments), Go (godoc), Rust (rustdoc) and Ruby (YARD), with 'auto' language detection. Parameter types come from annotations, or are inferred from default values (timeout=30 -> int), or fall back to a placeholder; 'types' can restrict this to declared annotations only or drop type slots entirely. Handles *args/**kwargs, ...rest, params/varargs, optional and defaulted parameters, decorators and annotations, generics, Go grouped parameters and receivers, and Rust lifetimes. Options: language, style, output (annotated source, stub only, or a JSON parse of the signature), types, placeholder text, raises, quote_style, extended_summary, examples, align_tags, indent_size. Descriptions are placeholders, never invented prose: this is a stub generator, not a summarizer. It reads signatures only, so raised errors are declared via 'raises' (Java `throws` is read automatically); input is capped at 200000 bytes and 200 signatures per run.",
        parameters = schema_json()
    ),
)]
impl DocstringStubGenerator {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "docstring-stub-generator", |a: Args| {
            generate(
                &a.signature,
                &a.language,
                &a.style,
                &a.output,
                &a.types,
                &a.placeholder,
                &a.raises,
                &a.quote_style,
                a.extended_summary,
                a.examples,
                a.align_tags,
                a.indent_size,
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
            r#"{
                "type": "object",
                "properties": {
                    "signature":        { "type": "string", "description": "The function signature(s) to document, pasted as text. One per run or several at once; a signature may span multiple lines. Pasting a whole function works too — the body is passed through untouched. Example: 'def fetch(url: str, timeout: int = 30) -> dict:'." },
                    "language":         { "type": "string", "enum": ["auto", "python", "javascript", "typescript", "php", "java", "csharp", "go", "rust", "ruby"], "default": "auto", "description": "Signature syntax and documentation convention. 'auto' (default) guesses from the code's shape. Named values: python (docstring), javascript/typescript (JSDoc), php (PHPDoc), java (Javadoc), csharp (XML doc comments), go (godoc), rust (rustdoc), ruby (YARD). Set it explicitly when auto-detection guesses wrong — e.g. force 'ruby' so `def f(a)` is not read as Python." },
                    "style":            { "type": "string", "enum": ["auto", "google", "numpy", "sphinx", "epytext", "pep257"], "default": "auto", "description": "Python docstring convention: 'auto' (default) = google. 'google' = 'Args:'/'Returns:' sections; 'numpy' = underlined 'Parameters'/'Returns' sections; 'sphinx' = ':param x:'/':type x:'/':rtype:' fields; 'epytext' = '@param'/'@type'/'@rtype' fields; 'pep257' = plain 'Arguments:' with 'name -- description'. Ignored for the other languages, which each have exactly one native convention (JSDoc, PHPDoc, Javadoc, XML doc, godoc, rustdoc, YARD)." },
                    "output":           { "type": "string", "enum": ["annotated", "docstring", "json"], "default": "annotated", "description": "What to return. 'annotated' (default) = the pasted input with each stub inserted in place (inside the def for Python, above the signature otherwise) — copy-paste ready. 'docstring' = only the generated stub blocks, blank-line separated. 'json' = the parsed signatures as {name, async, params:[{name,type,default,optional,variadic}], returns, raises}." },
                    "types":            { "type": "string", "enum": ["guess", "annotated", "none"], "default": "guess", "description": "Where parameter and return types come from. 'guess' (default) = the declared annotation, else inferred from the default value (timeout=30 → int, name=\"x\" → str), else a placeholder such as _type_. 'annotated' = only types actually declared in the signature; nothing is written when there is none. 'none' = omit type slots entirely (the '-notypes' shape)." },
                    "placeholder": { "type": "string", "default": "_description_", "description": "Filler text written into every description slot for you to replace. Default '_description_' (the editor-extension convention). Try 'FIXME' to make unfilled stubs easy to grep for. Blank falls back to the default." },
                    "raises":           { "type": "string", "default": "", "description": "Exception/error names to document, comma- or space-separated — e.g. 'ValueError, KeyError'. Default empty (no raises section). A signature alone does not say what it throws, so these are declared by you; Java `throws` clauses are picked up automatically and merged in. Capped at 20 names." },
                    "quote_style":      { "type": "string", "enum": ["double", "single"], "default": "double", "description": "Python docstring quotes: 'double' (default) = \"\"\", 'single' = '''. Ignored for every other language, which use comment blocks rather than string literals." },
                    "extended_summary": { "type": "boolean", "default": false, "description": "Add a second placeholder paragraph under the summary line for a longer description. Default false." },
                    "examples":         { "type": "boolean", "default": false, "description": "Add an examples section in the language's convention — 'Examples:' with a >>> line for Python, '@example' for JSDoc/PHPDoc/YARD, a '# Examples' fenced block for rustdoc, '<example>' for XML doc, 'Example:' for godoc. Default false." },
                    "align_tags":       { "type": "boolean", "default": false, "description": "Pad the type and name columns of tag-style blocks so every description starts at the same column (JSDoc, PHPDoc, Javadoc). Default false, which leaves the tags ragged. Has no effect on Python, godoc, rustdoc or XML doc comments." },
                    "indent_size":      { "type": "integer", "default": 4, "minimum": 0, "maximum": 8, "description": "Spaces used for indentation inside the generated stub, and for the Python docstring's offset from the `def` line. Default 4; use 2 for 2-space codebases. Range 0-8." }
                },
                "required": ["signature"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
