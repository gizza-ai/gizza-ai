//! gizza-ai/vscode-snippets-generator — generate VS Code user-snippet JSON.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    template: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    prefix: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    scope: String,
    #[serde(default = "default_output")]
    output: String,
    #[serde(default = "default_dollars")]
    dollars: String,
    #[serde(default = "default_indent")]
    indent: String,
    #[serde(default = "default_tab_size")]
    tab_size: f64,
    #[serde(default)]
    final_tabstop: bool,
    #[serde(default)]
    is_file_template: bool,
    #[serde(default = "default_json_indent")]
    json_indent: f64,
}

fn default_output() -> String {
    "snippets-file".into()
}
fn default_dollars() -> String {
    "auto".into()
}
fn default_indent() -> String {
    "keep".into()
}
fn default_tab_size() -> f64 {
    2.0
}
fn default_json_indent() -> f64 {
    2.0
}

const TEMPLATE_DESC: &str = "The code template that should become the VS Code snippet body. Paste one or more lines. Existing VS Code snippet constructs such as $1, ${1:name}, ${1|red,green|}, $0, $TM_FILENAME, and ${TM_FILENAME/(.*)\\..+$/$1/} are preserved in auto dollar mode, while stray literal dollar signs are escaped. Up to 200000 bytes.";
const NAME_DESC: &str = "Snippet name, used as the JSON key in the snippets file. If omitted, the first prefix is used. Use a readable name such as React function component or Rust test module.";
const PREFIX_DESC: &str = "Trigger word or words you type in VS Code IntelliSense. Separate multiple triggers with commas or new lines. One trigger is emitted as a string; multiple triggers are emitted as an array.";
const DESCRIPTION_DESC: &str = "Optional description shown in VS Code's IntelliSense details. Leave blank to omit the description field.";
const SCOPE_DESC: &str = "Optional comma-separated VS Code language identifiers such as javascript,typescriptreact,rust. Scope matters in a global .code-snippets file; language-specific files are already scoped by filename.";
const OUTPUT_DESC: &str = "Output shape. snippets-file returns a complete JSON object ready to save as a user snippets file. entry returns only the quoted snippet entry, useful when pasting into an existing JSON object.";
const DOLLARS_DESC: &str = "How to treat dollar signs in the template. auto preserves valid VS Code tabstops, placeholders, choices, variables, and transforms, but escapes stray literal dollars. literal escapes every dollar and backslash for plain text snippets. raw leaves the template untouched except for JSON escaping.";
const INDENT_DESC: &str = "Leading indentation normalization. keep preserves pasted whitespace. tabs converts leading whitespace to tabs using tab_size. spaces converts leading tabs to spaces using tab_size.";
const TAB_SIZE_DESC: &str = "Tab width used when converting indentation, from 1 to 16. Default 2. Has no effect when indent=keep.";
const FINAL_DESC: &str = "Append $0 to the last body line if the template does not already contain a final tabstop. This is a stateless alternative to generator pages that provide an insert-final-cursor button.";
const FILE_TEMPLATE_DESC: &str = "Include VS Code's isFileTemplate flag when true. File-template snippets can be used with VS Code's 'Snippets: Fill File with Snippet' command.";
const JSON_INDENT_DESC: &str =
    "Spaces for formatting the output JSON, 0 to 8. Default 2. Set 0 for compact one-line JSON.";

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("template").required().describe(TEMPLATE_DESC))
        .param(Param::string("name").default("").describe(NAME_DESC))
        .param(Param::string("prefix").default("").describe(PREFIX_DESC))
        .param(
            Param::string("description")
                .default("")
                .describe(DESCRIPTION_DESC),
        )
        .param(Param::string("scope").default("").describe(SCOPE_DESC))
        .param(
            Param::enumv("output", ["snippets-file", "entry"])
                .default("snippets-file")
                .describe(OUTPUT_DESC),
        )
        .param(
            Param::enumv("dollars", ["auto", "literal", "raw"])
                .default("auto")
                .describe(DOLLARS_DESC),
        )
        .param(
            Param::enumv("indent", ["keep", "tabs", "spaces"])
                .default("keep")
                .describe(INDENT_DESC),
        )
        .param(
            Param::integer("tab_size")
                .min(1.0)
                .max(16.0)
                .default(2)
                .describe(TAB_SIZE_DESC),
        )
        .param(
            Param::boolean("final_tabstop")
                .default(false)
                .describe(FINAL_DESC),
        )
        .param(
            Param::boolean("is_file_template")
                .default(false)
                .describe(FILE_TEMPLATE_DESC),
        )
        .param(
            Param::integer("json_indent")
                .min(0.0)
                .max(8.0)
                .default(2)
                .describe(JSON_INDENT_DESC),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/vscode-snippets-generator",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert a code template into VS Code user-snippet JSON with safe escaping",
    skill(
        description = "Convert a code template with placeholders into a VS Code user-snippet JSON entry or complete snippets-file object. The tool accepts name, prefix, description, scope, output shape, dollar escaping mode, indentation normalization, tab size, final-tabstop insertion, file-template flag, and JSON indentation. It preserves valid VS Code constructs like $1, ${1:name}, ${1|a,b|}, $0, $TM_FILENAME, and transforms while escaping stray literal dollars and JSON-special characters.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "vscode-snippets-generator", |a: Args| {
            gizza_ai_vscode_snippets_generator_core::run(
                &a.template,
                &a.name,
                &a.prefix,
                &a.description,
                &a.scope,
                &a.output,
                &a.dollars,
                &a.indent,
                a.tab_size,
                a.final_tabstop,
                a.is_file_template,
                a.json_indent,
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
    fn dump_schema() {
        println!("SCHEMA_BEGIN{}SCHEMA_END", schema_json());
    }

    #[test]
    fn schema_matches_authored_contract() {
        let actual: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        let authored: serde_json::Value = serde_json::json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["template"],
            "properties": {
                "template": { "type": "string", "description": TEMPLATE_DESC },
                "name": { "type": "string", "default": "", "description": NAME_DESC },
                "prefix": { "type": "string", "default": "", "description": PREFIX_DESC },
                "description": { "type": "string", "default": "", "description": DESCRIPTION_DESC },
                "scope": { "type": "string", "default": "", "description": SCOPE_DESC },
                "output": { "type": "string", "enum": ["snippets-file", "entry"], "default": "snippets-file", "description": OUTPUT_DESC },
                "dollars": { "type": "string", "enum": ["auto", "literal", "raw"], "default": "auto", "description": DOLLARS_DESC },
                "indent": { "type": "string", "enum": ["keep", "tabs", "spaces"], "default": "keep", "description": INDENT_DESC },
                "tab_size": { "type": "integer", "minimum": 1, "maximum": 16, "default": 2, "description": TAB_SIZE_DESC },
                "final_tabstop": { "type": "boolean", "default": false, "description": FINAL_DESC },
                "is_file_template": { "type": "boolean", "default": false, "description": FILE_TEMPLATE_DESC },
                "json_indent": { "type": "integer", "minimum": 0, "maximum": 8, "default": 2, "description": JSON_INDENT_DESC }
            }
        });
        assert_eq!(actual, authored);
    }

    #[test]
    fn defaults_run_through_core() {
        let a: Args =
            serde_json::from_str(r#"{"template":"console.log($1);","name":"Log","prefix":"log"}"#)
                .unwrap();
        let out = gizza_ai_vscode_snippets_generator_core::run(
            &a.template,
            &a.name,
            &a.prefix,
            &a.description,
            &a.scope,
            &a.output,
            &a.dollars,
            &a.indent,
            a.tab_size,
            a.final_tabstop,
            a.is_file_template,
            a.json_indent,
        )
        .unwrap();
        assert!(out.contains("\"Log\""), "{out}");
        assert!(out.contains("console.log($1);"), "{out}");
    }
}
