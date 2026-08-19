//! gizza-ai/rest-file-to-curl — chat skill block on the shared tool abstraction.
//!
//! Expands a `.http` / `.rest` / `.ain` request file (with `{{variables}}` and an
//! environment) into runnable `curl` commands. Pure compute — the requests are
//! parsed and rendered as shell text, never sent. The chat schema is
//! single-sourced from descriptor() (which also drives the CLI); handle()
//! delegates to block_utils::run_skill. The only host capability used is the
//! clock, to resolve the `{{$timestamp}}` / `{{$datetime}}` system variables.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args {
    data: String,
    #[serde(default)]
    env: String,
    #[serde(default)]
    environment: String,
    #[serde(default)]
    request: String,
    #[serde(default)]
    format: String,
    #[serde(default)]
    shell: String,
    #[serde(default)]
    flag_style: String,
    #[serde(default = "yes")]
    multiline: bool,
    #[serde(default)]
    unresolved: String,
    #[serde(default)]
    follow_redirects: bool,
    #[serde(default)]
    compressed: bool,
    #[serde(default)]
    insecure: bool,
    #[serde(default = "yes")]
    include_comments: bool,
}

fn yes() -> bool {
    true
}

/// Single source for the chat schema (and CLI).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(
            Param::string("data")
                .required()
                .describe(
                    "Contents of the request file. A .http/.rest file looks like \"GET https://api.example.com/users HTTP/1.1\" followed by header lines, a blank line, then the body; '###' separates several requests in one file, '@name = value' declares a variable and '{{name}}' references it. An .ain file uses [Method] [Host] [Query] [Headers] [Body] sections with $VAR substitution. Maximum 1000000 bytes.",
                ),
        )
        .param(
            Param::string("env")
                .default("")
                .describe(
                    "Variable values to substitute, as either a JSON object ({\"host\":\"https://api.example.com\",\"token\":\"abc\"}), a JSON document of NAMED environments ({\"dev\":{...},\"prod\":{...}} — then set 'environment'), or plain KEY=VALUE lines like a .env file ('export ' prefixes, # comments and surrounding quotes are stripped). Leave blank to keep every variable unresolved.",
                ),
        )
        .param(
            Param::string("environment")
                .default("")
                .describe(
                    "Which named environment to take variables from when 'env' is a JSON document of environments, e.g. 'dev' or 'prod'. Leave blank for a flat variable object.",
                ),
        )
        .param(
            Param::string("request")
                .default("all")
                .describe(
                    "Which request in the file to convert: 'all' (default) for every request, a 1-based index like '2', or a request name from a '# @name login' comment or a '### login' separator.",
                ),
        )
        .param(
            Param::enumv("format", ["auto", "http", "ain"])
                .default("auto")
                .describe(
                    "Input file format: 'auto' (default) detects .ain by its [Section] headers and otherwise parses .http/.rest; 'http' or 'ain' force one.",
                ),
        )
        .param(
            Param::enumv("shell", ["bash", "cmd", "powershell"])
                .default("bash")
                .describe(
                    "Shell the generated command must run in — it sets the quoting and line-continuation style: 'bash' (default, also sh/zsh: single quotes, backslash continuations), 'cmd' (double quotes, ^ continuations), 'powershell' (single quotes with doubled escapes, ` continuations, and 'curl.exe' so it does not hit the Invoke-WebRequest alias).",
                ),
        )
        .param(
            Param::enumv("flag_style", ["short", "long"])
                .default("short")
                .describe(
                    "Curl flag spelling: 'short' (default) uses -X/-H/-d/-L/-k; 'long' uses --request/--header/--data-raw/--location/--insecure.",
                ),
        )
        .param(
            Param::boolean("multiline")
                .default(true)
                .describe(
                    "true (default) spreads the command over several lines with continuation characters, one flag per line; false emits a single line.",
                ),
        )
        .param(
            Param::enumv("unresolved", ["keep", "empty", "error"])
                .default("keep")
                .describe(
                    "What to do with a variable that has no value: 'keep' (default) leaves the {{placeholder}} in the command so you can fill it in, 'empty' substitutes an empty string, 'error' fails and lists the missing names.",
                ),
        )
        .param(
            Param::boolean("follow_redirects")
                .default(false)
                .describe("Add curl's -L / --location flag so redirects are followed. Default false."),
        )
        .param(
            Param::boolean("compressed")
                .default(false)
                .describe("Add curl's --compressed flag to request a gzip/deflate response. Default false."),
        )
        .param(
            Param::boolean("insecure")
                .default(false)
                .describe("Add curl's -k / --insecure flag to skip TLS certificate verification (useful against a local dev server with a self-signed certificate). Default false."),
        )
        .param(
            Param::boolean("include_comments")
                .default(true)
                .describe("true (default) prefixes each command with a '# name' comment line when the file has several requests or a named one; false emits only the commands."),
        )
}
fn schema_json() -> String {
    descriptor().to_schema_json()
}

/// Milliseconds since the Unix epoch, for the `{{$timestamp}}` / `{{$datetime}}`
/// / `{{$guid}}` system variables. On wasm (chat block + CLI runtime) the host
/// provides the clock import; the page supplies its own via `js_sys::Date`.
fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/rest-file-to-curl",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "Convert a .http, .rest or .ain request file into curl commands",
    skill(
        description = "Convert a REST client request file into ready-to-run curl commands. Set 'data' to the contents of a .http/.rest file (the VS Code REST Client / JetBrains HTTP Client format: a 'GET https://api.example.com/users HTTP/1.1' request line, header lines, a blank line, then the body, with '###' separating multiple requests, '# @name id' naming one, '@name = value' declaring file variables and '{{name}}' referencing them) or an .ain file ([Method] [Host] [Query] [Headers] [Body] sections with $VAR substitution). Supply variable values in 'env' as a JSON object, a JSON document of named environments (then pick one with 'environment'), or KEY=VALUE lines; system variables {{$guid}}, {{$timestamp}}, {{$datetime}}, {{$randomInt}}, {{$processEnv}} and {{$dotenv}} are resolved too. 'request' picks one request by index or name (default all). Control the output with 'shell' (bash, cmd, powershell quoting), 'flag_style' (short or long flags), 'multiline', 'follow_redirects', 'compressed', 'insecure' and 'include_comments'; 'unresolved' decides whether missing variables are kept as placeholders, blanked, or reported as an error. Multi-line query continuations, form-body continuation lines and '< file' body references (rendered as --data-binary '@file') are handled. Nothing is ever sent — the file is parsed locally and returned as command text.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run_skill(&body, "rest-file-to-curl", |a: Args| {
            gizza_ai_rest_file_to_curl_core::convert(
                &a.data,
                &a.env,
                &a.environment,
                &a.request,
                &a.format,
                &a.shell,
                &a.flag_style,
                a.multiline,
                &a.unresolved,
                a.follow_redirects,
                a.compressed,
                a.insecure,
                a.include_comments,
                now_ms(),
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

    /// Drift guard: the descriptor-derived chat schema must match this authored
    /// schema, so any future change to the LLM-facing API is intentional.
    #[test]
    fn schema_json_matches_authored_chat_schema() {
        let authored: serde_json::Value = serde_json::from_str(
            r#"{
                "type": "object",
                "properties": {
                    "data": { "type": "string", "description": "Contents of the request file. A .http/.rest file looks like \"GET https://api.example.com/users HTTP/1.1\" followed by header lines, a blank line, then the body; '###' separates several requests in one file, '@name = value' declares a variable and '{{name}}' references it. An .ain file uses [Method] [Host] [Query] [Headers] [Body] sections with $VAR substitution. Maximum 1000000 bytes." },
                    "env": { "type": "string", "default": "", "description": "Variable values to substitute, as either a JSON object ({\"host\":\"https://api.example.com\",\"token\":\"abc\"}), a JSON document of NAMED environments ({\"dev\":{...},\"prod\":{...}} — then set 'environment'), or plain KEY=VALUE lines like a .env file ('export ' prefixes, # comments and surrounding quotes are stripped). Leave blank to keep every variable unresolved." },
                    "environment": { "type": "string", "default": "", "description": "Which named environment to take variables from when 'env' is a JSON document of environments, e.g. 'dev' or 'prod'. Leave blank for a flat variable object." },
                    "request": { "type": "string", "default": "all", "description": "Which request in the file to convert: 'all' (default) for every request, a 1-based index like '2', or a request name from a '# @name login' comment or a '### login' separator." },
                    "format": { "type": "string", "enum": ["auto", "http", "ain"], "default": "auto", "description": "Input file format: 'auto' (default) detects .ain by its [Section] headers and otherwise parses .http/.rest; 'http' or 'ain' force one." },
                    "shell": { "type": "string", "enum": ["bash", "cmd", "powershell"], "default": "bash", "description": "Shell the generated command must run in — it sets the quoting and line-continuation style: 'bash' (default, also sh/zsh: single quotes, backslash continuations), 'cmd' (double quotes, ^ continuations), 'powershell' (single quotes with doubled escapes, ` continuations, and 'curl.exe' so it does not hit the Invoke-WebRequest alias)." },
                    "flag_style": { "type": "string", "enum": ["short", "long"], "default": "short", "description": "Curl flag spelling: 'short' (default) uses -X/-H/-d/-L/-k; 'long' uses --request/--header/--data-raw/--location/--insecure." },
                    "multiline": { "type": "boolean", "default": true, "description": "true (default) spreads the command over several lines with continuation characters, one flag per line; false emits a single line." },
                    "unresolved": { "type": "string", "enum": ["keep", "empty", "error"], "default": "keep", "description": "What to do with a variable that has no value: 'keep' (default) leaves the {{placeholder}} in the command so you can fill it in, 'empty' substitutes an empty string, 'error' fails and lists the missing names." },
                    "follow_redirects": { "type": "boolean", "default": false, "description": "Add curl's -L / --location flag so redirects are followed. Default false." },
                    "compressed": { "type": "boolean", "default": false, "description": "Add curl's --compressed flag to request a gzip/deflate response. Default false." },
                    "insecure": { "type": "boolean", "default": false, "description": "Add curl's -k / --insecure flag to skip TLS certificate verification (useful against a local dev server with a self-signed certificate). Default false." },
                    "include_comments": { "type": "boolean", "default": true, "description": "true (default) prefixes each command with a '# name' comment line when the file has several requests or a named one; false emits only the commands." }
                },
                "required": ["data"],
                "additionalProperties": false
            }"#,
        )
        .unwrap();
        let derived: serde_json::Value = serde_json::from_str(&schema_json()).unwrap();
        assert_eq!(derived, authored, "no LLM-facing chat-schema drift");
    }
}
