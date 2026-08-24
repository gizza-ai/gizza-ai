//! gizza-ai/env-var-reference-extractor — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the CLI);
//! handle() delegates to block_utils::run_skill. Pure → all backends.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use gizza_ai_env_var_reference_extractor_core::extract;
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    text: String,
    #[serde(default = "default_auto")]
    syntax: String,
    #[serde(default = "default_names")]
    output: String,
    #[serde(default)]
    defined: String,
    #[serde(default = "default_true")]
    include_defined_in_source: bool,
    #[serde(default = "default_true")]
    skip_comments: bool,
    #[serde(default)]
    ignore: String,
    #[serde(default)]
    only_undefined: bool,
    #[serde(default = "default_name")]
    sort: String,
}
fn default_auto() -> String {
    "auto".to_string()
}
fn default_names() -> String {
    "names".to_string()
}
fn default_name() -> String {
    "name".to_string()
}
fn default_true() -> bool {
    true
}

fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("text")
                .required()
                .describe("The shell script, Dockerfile, docker-compose/CI YAML, batch file or source file to scan, pasted as text. Example: 'PORT=${PORT:-8080}\\ncurl \"$API_URL/health\"'."),
        )
        .param(
            Param::enumv(
                "syntax",
                ["auto", "shell", "dockerfile", "windows", "code", "all"],
            )
            .default("auto")
            .describe("Which reference family to look for. 'auto' (default) guesses from the input's shape. 'shell' = $VAR, ${VAR}, ${VAR:-default} and the other parameter expansions. 'dockerfile' = the same plus ENV/ARG lines counted as definitions. 'windows' = %VAR% and delayed-expansion !VAR!, plus set/setx definitions. 'code' = library accessors such as process.env.VAR, import.meta.env.VAR, os.environ[\"VAR\"], os.getenv(\"VAR\"), System.getenv(\"VAR\"), env::var(\"VAR\"), getenv(\"VAR\"). 'all' scans every family at once — useful for mixed inputs, at the cost of false positives such as a printf '%d%' being read as a Windows reference."),
        )
        .param(
            Param::enumv(
                "output",
                ["names", "table", "json", "markdown", "csv", "env-template", "stats"],
            )
            .default("names")
            .describe("What to return. 'names' (default) = one variable name per line. 'table' = an aligned VARIABLE/USES/LINES/DEFAULT/STATUS table. 'json' = an array of {name, count, lines, forms, default, defined, defined_in, defined_at_line}. 'markdown' = the same columns as a Markdown table. 'csv' = a name,uses,lines,default,status sheet. 'env-template' = a ready-to-edit .env.example with a usage comment above each key. 'stats' = a summary of the detected syntax and reference counts."),
        )
        .param(
            Param::string("defined")
                .default("")
                .describe("Optional list of variables you already provide, used to mark each reference defined or undefined. Accepts a .env body ('DB_HOST=db\\nAPI_KEY=secret'), export lines, or bare names separated by newlines, commas or spaces. Values are ignored — only the names matter. Default empty, in which case only definitions found inside the pasted text count."),
        )
        .param(
            Param::boolean("include_defined_in_source")
                .default(true)
                .describe("Count assignments inside the pasted text as definitions, so a variable the script sets itself is reported as 'defined'. Recognises shell 'NAME=value' (with export/declare/readonly/local), Dockerfile ENV and ARG, and Windows set/setx. Default true; set false to treat every reference as external and list everything the input consumes."),
        )
        .param(
            Param::boolean("skip_comments")
                .default(true)
                .describe("Ignore references inside comments — a '#' at the start of a line or after whitespace, '//' line comments in code, and REM/:: lines in batch files. Default true. The '#' in an expansion such as ${PREFIX#/opt} is never treated as a comment. Set false to include commented-out references."),
        )
        .param(
            Param::string("ignore")
                .default("")
                .describe("Variable names to leave out of the report, separated by commas, spaces or newlines. A trailing '*' matches a prefix. Matching is case-sensitive. Example: 'PATH, HOME, LC_*' drops PATH, HOME and every LC_ locale variable. Default empty (report everything)."),
        )
        .param(
            Param::boolean("only_undefined")
                .default(false)
                .describe("Report only the variables nothing defines — neither the pasted text nor the 'defined' list. Default false. Turn this on to get the exact set of values a script still needs before it can run; the stats output always reports both totals."),
        )
        .param(
            Param::enumv("sort", ["name", "occurrences", "first-seen"])
                .default("name")
                .describe("Row order. 'name' (default) = alphabetical. 'occurrences' = most-referenced first, ties broken alphabetically. 'first-seen' = the order the variables first appear in the input, which keeps a script's reading order."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

#[cfg(target_arch = "wasm32")]
struct EnvVarReferenceExtractor;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/env-var-reference-extractor",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Find every environment-variable reference in a script, Dockerfile or config",
    skill(
        description = "Scan a pasted shell script, Dockerfile, docker-compose/CI YAML, batch file or source file and report every environment variable it references, where each one is used, whether it carries a fallback default, and whether anything defines it. Recognises shell $VAR, ${VAR} and the parameter expansions ${VAR:-default}, ${VAR:=default}, ${VAR:?error}, ${VAR:+alt}, ${VAR#pattern}, ${#VAR} and ${!VAR}; Windows %VAR% and delayed-expansion !VAR!; and library accessors such as process.env.VAR, import.meta.env.VAR, os.environ[\"VAR\"], os.getenv(\"VAR\"), System.getenv(\"VAR\"), env::var(\"VAR\") and getenv(\"VAR\"). Escaped \\$ and $$ are skipped, and positional or special parameters ($1, $@, $?, $$) are never reported as variables. Definitions are picked up from shell assignments, Dockerfile ENV/ARG and Windows set/setx, and you can paste a .env body or name list to mark the rest as defined. Options: syntax (auto/shell/dockerfile/windows/code/all), output (names, table, json, markdown, csv, env-template that generates a .env.example, or stats), defined, include_defined_in_source, skip_comments, ignore with prefix wildcards, only_undefined, sort. This is a deterministic scanner, not a shell parser: single-quoted strings and here-doc bodies are still scanned even though a real shell would not expand them, bare names inside $(( )) arithmetic are not references, and a run is capped at 20,000 references.",
        parameters = schema_json()
    ),
)]
impl EnvVarReferenceExtractor {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "env-var-reference-extractor", |a: Args| {
            extract(
                &a.text,
                &a.syntax,
                &a.output,
                &a.defined,
                a.include_defined_in_source,
                a.skip_comments,
                &a.ignore,
                a.only_undefined,
                &a.sort,
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
                    "text":     { "type": "string", "description": "The shell script, Dockerfile, docker-compose/CI YAML, batch file or source file to scan, pasted as text. Example: 'PORT=${PORT:-8080}\\ncurl \"$API_URL/health\"'." },
                    "syntax":   { "type": "string", "enum": ["auto", "shell", "dockerfile", "windows", "code", "all"], "default": "auto", "description": "Which reference family to look for. 'auto' (default) guesses from the input's shape. 'shell' = $VAR, ${VAR}, ${VAR:-default} and the other parameter expansions. 'dockerfile' = the same plus ENV/ARG lines counted as definitions. 'windows' = %VAR% and delayed-expansion !VAR!, plus set/setx definitions. 'code' = library accessors such as process.env.VAR, import.meta.env.VAR, os.environ[\"VAR\"], os.getenv(\"VAR\"), System.getenv(\"VAR\"), env::var(\"VAR\"), getenv(\"VAR\"). 'all' scans every family at once — useful for mixed inputs, at the cost of false positives such as a printf '%d%' being read as a Windows reference." },
                    "output":   { "type": "string", "enum": ["names", "table", "json", "markdown", "csv", "env-template", "stats"], "default": "names", "description": "What to return. 'names' (default) = one variable name per line. 'table' = an aligned VARIABLE/USES/LINES/DEFAULT/STATUS table. 'json' = an array of {name, count, lines, forms, default, defined, defined_in, defined_at_line}. 'markdown' = the same columns as a Markdown table. 'csv' = a name,uses,lines,default,status sheet. 'env-template' = a ready-to-edit .env.example with a usage comment above each key. 'stats' = a summary of the detected syntax and reference counts." },
                    "defined":  { "type": "string", "default": "", "description": "Optional list of variables you already provide, used to mark each reference defined or undefined. Accepts a .env body ('DB_HOST=db\\nAPI_KEY=secret'), export lines, or bare names separated by newlines, commas or spaces. Values are ignored — only the names matter. Default empty, in which case only definitions found inside the pasted text count." },
                    "include_defined_in_source": { "type": "boolean", "default": true, "description": "Count assignments inside the pasted text as definitions, so a variable the script sets itself is reported as 'defined'. Recognises shell 'NAME=value' (with export/declare/readonly/local), Dockerfile ENV and ARG, and Windows set/setx. Default true; set false to treat every reference as external and list everything the input consumes." },
                    "skip_comments": { "type": "boolean", "default": true, "description": "Ignore references inside comments — a '#' at the start of a line or after whitespace, '//' line comments in code, and REM/:: lines in batch files. Default true. The '#' in an expansion such as ${PREFIX#/opt} is never treated as a comment. Set false to include commented-out references." },
                    "ignore":   { "type": "string", "default": "", "description": "Variable names to leave out of the report, separated by commas, spaces or newlines. A trailing '*' matches a prefix. Matching is case-sensitive. Example: 'PATH, HOME, LC_*' drops PATH, HOME and every LC_ locale variable. Default empty (report everything)." },
                    "only_undefined": { "type": "boolean", "default": false, "description": "Report only the variables nothing defines — neither the pasted text nor the 'defined' list. Default false. Turn this on to get the exact set of values a script still needs before it can run; the stats output always reports both totals." },
                    "sort":     { "type": "string", "enum": ["name", "occurrences", "first-seen"], "default": "name", "description": "Row order. 'name' (default) = alphabetical. 'occurrences' = most-referenced first, ties broken alphabetically. 'first-seen' = the order the variables first appear in the input, which keeps a script's reading order." }
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
