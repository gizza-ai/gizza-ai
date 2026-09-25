//! harmony-format-renderer core — pure compute, shared by the chat skill block
//! and the web page. No wafer/wasm-bindgen deps.
//!
//! Renders a system/developer/user/assistant/tool conversation into the Harmony
//! response format used by the gpt-oss open-weight models: a flat token string
//! built from `<|start|>`, `<|channel|>`, `<|constrain|>`, `<|message|>`,
//! `<|end|>`, `<|call|>` and `<|return|>` control tokens.
//!
//! Layout of a rendered prompt:
//!
//! ```text
//! <|start|>system<|message|>{identity}
//! Knowledge cutoff: {cutoff}
//! Current date: {date}
//!
//! Reasoning: {effort}
//!
//! # Valid channels: analysis, commentary, final. Channel must be included for every message.<|end|>
//! <|start|>developer<|message|># Instructions
//!
//! {instructions}
//!
//! # Tools
//!
//! ## functions
//!
//! namespace functions {
//! ...
//! } // namespace functions<|end|>
//! <|start|>user<|message|>{text}<|end|>
//! <|start|>assistant
//! ```
//!
//! Everything is deterministic string assembly — no tokenizer, no network, no LLM.

use serde::{Deserialize, Serialize};

// ---- Limits (stated on the page, enforced here) -----------------------------

/// Largest accepted `messages` payload, in characters.
pub const MAX_INPUT_CHARS: usize = 200_000;
/// Largest accepted `tools` payload, in characters.
pub const MAX_TOOLS_CHARS: usize = 50_000;
/// Largest accepted number of conversation turns.
pub const MAX_MESSAGES: usize = 500;

// ---- Control tokens ---------------------------------------------------------

const T_START: &str = "<|start|>";
const T_MESSAGE: &str = "<|message|>";
const T_END: &str = "<|end|>";
const T_CHANNEL: &str = "<|channel|>";
const T_CONSTRAIN: &str = "<|constrain|>";
const T_CALL: &str = "<|call|>";
const T_RETURN: &str = "<|return|>";

const DEFAULT_IDENTITY: &str = "You are ChatGPT, a large language model trained by OpenAI.";
const DEFAULT_CUTOFF: &str = "2024-06";
const CHANNELS: &str = "analysis, commentary, final";

const INPUT_FORMATS: &[&str] = &["auto", "json", "lines"];
const REASONING_EFFORTS: &[&str] = &["low", "medium", "high", "none"];
const RENDER_TARGETS: &[&str] = &["completion", "conversation"];
const OUTPUT_FORMATS: &[&str] = &["text", "json"];
const ROLES: &[&str] = &["system", "developer", "user", "assistant", "tool"];
const VALID_CHANNELS: &[&str] = &["analysis", "commentary", "final"];

// ---- Input model ------------------------------------------------------------

/// One conversation turn as supplied by the caller.
#[derive(Debug, Clone, Deserialize)]
struct InMessage {
    role: String,
    #[serde(default)]
    content: String,
    /// `analysis` | `commentary` | `final` — assistant turns only.
    #[serde(default)]
    channel: Option<String>,
    /// Tool being called, e.g. `get_weather` or `functions.get_weather`.
    #[serde(default)]
    recipient: Option<String>,
    /// Tool name for a `tool` turn, e.g. `get_weather`.
    #[serde(default)]
    name: Option<String>,
}

/// One turn after normalization.
#[derive(Debug, Clone)]
struct Turn {
    role: String,
    content: String,
    channel: Option<String>,
    recipient: Option<String>,
    name: Option<String>,
}

/// A function tool declaration, as accepted in the `tools` JSON.
#[derive(Debug, Clone, Deserialize)]
struct ToolDef {
    name: String,
    #[serde(default)]
    description: Option<String>,
    /// A JSON-Schema object describing the call arguments.
    #[serde(default)]
    parameters: Option<serde_json::Value>,
}

/// The `output_format = "json"` payload.
#[derive(Debug, Serialize)]
struct JsonOut {
    prompt: String,
    message_count: usize,
    rendered_message_count: usize,
    dropped_analysis_count: usize,
    tool_count: usize,
    char_count: usize,
    stop_tokens: Vec<String>,
}

// ---- Entry point ------------------------------------------------------------

/// Render `messages` (plus the system/developer knobs) into a Harmony prompt.
///
/// Returns the rendered prompt (`output_format = "text"`) or a pretty-printed
/// JSON report containing it (`output_format = "json"`).
#[allow(clippy::too_many_arguments)]
pub fn run(
    messages: &str,
    input_format: &str,
    instructions: &str,
    tools: &str,
    model_identity: &str,
    reasoning_effort: &str,
    knowledge_cutoff: &str,
    current_date: &str,
    include_system: bool,
    render_target: &str,
    auto_drop_analysis: bool,
    output_format: &str,
) -> Result<String, String> {
    let input_format = pick(input_format, INPUT_FORMATS, "auto", "input_format")?;
    let reasoning_effort = pick(reasoning_effort, REASONING_EFFORTS, "medium", "reasoning_effort")?;
    let render_target = pick(render_target, RENDER_TARGETS, "completion", "render_target")?;
    let output_format = pick(output_format, OUTPUT_FORMATS, "text", "output_format")?;

    if messages.chars().count() > MAX_INPUT_CHARS {
        return Err(format!(
            "messages is too large: expected at most {MAX_INPUT_CHARS} characters, got {}",
            messages.chars().count()
        ));
    }
    if tools.chars().count() > MAX_TOOLS_CHARS {
        return Err(format!(
            "tools is too large: expected at most {MAX_TOOLS_CHARS} characters, got {}",
            tools.chars().count()
        ));
    }

    let raw_turns = parse_messages(messages, input_format)?;
    if raw_turns.is_empty() {
        return Err(
            "no messages found: expected at least one turn, got an empty conversation. \
             Provide a JSON array like [{\"role\":\"user\",\"content\":\"hi\"}] or lines like `user: hi`."
                .to_string(),
        );
    }
    if raw_turns.len() > MAX_MESSAGES {
        return Err(format!(
            "too many messages: expected at most {MAX_MESSAGES} turns, got {}",
            raw_turns.len()
        ));
    }

    let tool_defs = parse_tools(tools)?;

    // Harmony reserves the system message for metadata, so a caller-supplied
    // `system` turn becomes developer Instructions (the mapping every gpt-oss
    // chat template uses). `developer` turns fold in the same way.
    let mut instruction_parts: Vec<String> = Vec::new();
    if !instructions.trim().is_empty() {
        instruction_parts.push(instructions.trim().to_string());
    }
    let mut convo: Vec<Turn> = Vec::new();
    for t in raw_turns.iter() {
        match t.role.as_str() {
            "system" | "developer" => {
                if !t.content.trim().is_empty() {
                    instruction_parts.push(t.content.trim().to_string());
                }
            }
            _ => convo.push(t.clone()),
        }
    }
    let merged_instructions = instruction_parts.join("\n\n");

    let total_in = raw_turns.len();
    let mut dropped = 0usize;
    if auto_drop_analysis {
        let before = convo.len();
        convo = drop_superseded_analysis(convo);
        dropped = before - convo.len();
    }

    let mut out = String::new();
    if include_system {
        out.push_str(&render_system(
            model_identity,
            reasoning_effort,
            knowledge_cutoff,
            current_date,
            !tool_defs.is_empty(),
        ));
    }
    if !merged_instructions.is_empty() || !tool_defs.is_empty() {
        out.push_str(&render_developer(&merged_instructions, &tool_defs));
    }
    for t in &convo {
        out.push_str(&render_turn(t)?);
    }
    if render_target == "completion" {
        out.push_str(T_START);
        out.push_str("assistant");
    }

    if output_format == "json" {
        let payload = JsonOut {
            message_count: total_in,
            rendered_message_count: convo.len(),
            dropped_analysis_count: dropped,
            tool_count: tool_defs.len(),
            char_count: out.chars().count(),
            stop_tokens: vec![T_RETURN.to_string(), T_CALL.to_string()],
            prompt: out,
        };
        return serde_json::to_string_pretty(&payload)
            .map_err(|e| format!("could not serialize the JSON report: {e}"));
    }
    Ok(out)
}

/// Validate a fixed-choice value, treating blank as the default.
fn pick<'a>(v: &'a str, allowed: &[&str], default: &'a str, field: &str) -> Result<&'a str, String> {
    let v = v.trim();
    if v.is_empty() {
        return Ok(default);
    }
    if allowed.contains(&v) {
        return Ok(v);
    }
    Err(format!(
        "invalid {field}: expected one of {}, got '{v}'",
        allowed.join(", ")
    ))
}

// ---- Message parsing --------------------------------------------------------

fn parse_messages(src: &str, input_format: &str) -> Result<Vec<Turn>, String> {
    let trimmed = src.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    let treat_as_json = match input_format {
        "json" => true,
        "lines" => false,
        // auto: a leading `[` or `{` means the caller pasted JSON.
        _ => trimmed.starts_with('[') || trimmed.starts_with('{'),
    };
    if treat_as_json {
        parse_json_messages(trimmed)
    } else {
        parse_line_messages(trimmed)
    }
}

fn parse_json_messages(src: &str) -> Result<Vec<Turn>, String> {
    let value: serde_json::Value = serde_json::from_str(src).map_err(|e| {
        format!(
            "messages is not valid JSON: {e}. Expected a JSON array of \
             {{\"role\":…,\"content\":…}} objects, or switch the input format to 'lines'."
        )
    })?;
    // Accept a bare array, a single object, or {"messages": [...]}.
    let arr = match value {
        serde_json::Value::Array(a) => a,
        serde_json::Value::Object(ref o) => {
            if let Some(serde_json::Value::Array(a)) = o.get("messages") {
                a.clone()
            } else {
                vec![value.clone()]
            }
        }
        other => {
            return Err(format!(
                "messages must be a JSON array of message objects, got {}",
                json_kind(&other)
            ))
        }
    };
    let mut out = Vec::with_capacity(arr.len());
    for (i, item) in arr.iter().enumerate() {
        if !item.is_object() {
            return Err(format!(
                "message #{} must be an object with a 'role', got {}",
                i + 1,
                json_kind(item)
            ));
        }
        let m: InMessage = serde_json::from_value(normalize_content(item.clone())).map_err(|e| {
            format!(
                "message #{} could not be read: {e}. Expected fields: role, content, \
                 and optionally channel / recipient / name.",
                i + 1
            )
        })?;
        out.push(normalize_turn(m, i + 1)?);
    }
    Ok(out)
}

/// The Responses/Chat APIs allow `content` to be an array of content parts and
/// allow tool calls to be carried in sibling fields; flatten what we can so a
/// copy-pasted API payload still renders.
fn normalize_content(mut item: serde_json::Value) -> serde_json::Value {
    let obj = match item.as_object_mut() {
        Some(o) => o,
        None => return item,
    };
    if let Some(parts) = obj.get("content").and_then(|c| c.as_array()).cloned() {
        let joined: Vec<String> = parts
            .iter()
            .map(|p| match p {
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Object(o) => o
                    .get("text")
                    .and_then(|t| t.as_str())
                    .unwrap_or_default()
                    .to_string(),
                _ => String::new(),
            })
            .filter(|s| !s.is_empty())
            .collect();
        obj.insert(
            "content".to_string(),
            serde_json::Value::String(joined.join("\n")),
        );
    }
    if obj.get("content").map(|c| c.is_null()).unwrap_or(false) {
        obj.insert("content".to_string(), serde_json::Value::String(String::new()));
    }
    // Numbers/bools in `content` are stringified rather than rejected.
    match obj.get("content") {
        Some(serde_json::Value::Number(n)) => {
            let s = n.to_string();
            obj.insert("content".to_string(), serde_json::Value::String(s));
        }
        Some(serde_json::Value::Bool(b)) => {
            let s = b.to_string();
            obj.insert("content".to_string(), serde_json::Value::String(s));
        }
        _ => {}
    }
    item
}

/// `role: content` lines, with an optional `[channel]` and `to=tool` qualifier:
/// `assistant[analysis]: thinking…`, `assistant[commentary] to=get_weather: {…}`,
/// `tool:get_weather: {…}`. A line that doesn't start a new turn is appended to
/// the previous one, so pasted multi-line answers survive.
fn parse_line_messages(src: &str) -> Result<Vec<Turn>, String> {
    let mut out: Vec<Turn> = Vec::new();
    for (lineno, line) in src.lines().enumerate() {
        match split_line_header(line) {
            Some((m, rest)) => {
                let mut turn = normalize_turn(m, lineno + 1)?;
                turn.content = rest.trim_start().to_string();
                out.push(turn);
            }
            None => match out.last_mut() {
                Some(prev) => {
                    prev.content.push('\n');
                    prev.content.push_str(line);
                }
                None => {
                    if line.trim().is_empty() {
                        continue;
                    }
                    return Err(format!(
                        "line {}: expected a turn like `user: hello` (roles: {}), got '{}'",
                        lineno + 1,
                        ROLES.join(", "),
                        line.trim()
                    ));
                }
            },
        }
    }
    // Trailing blank lines from a textarea shouldn't ride along in the content.
    for t in out.iter_mut() {
        t.content = t.content.trim_end().to_string();
    }
    Ok(out)
}

/// Recognize `role[channel] to=target:` / `tool:name:` headers at the start of a
/// line. Returns the parsed header plus the remainder, or `None` if the line is a
/// continuation.
fn split_line_header(line: &str) -> Option<(InMessage, &str)> {
    let t = line.trim_start();
    let role = ROLES.iter().find(|r| {
        t.len() > r.len()
            && t.get(..r.len()) == Some(**r)
            && matches!(t.as_bytes()[r.len()], b':' | b'[' | b' ')
    })?;
    let mut rest = &t[role.len()..];

    let mut channel = None;
    if let Some(stripped) = rest.strip_prefix('[') {
        let close = stripped.find(']')?;
        channel = Some(stripped[..close].trim().to_string());
        rest = &stripped[close + 1..];
    }

    let mut recipient = None;
    let mut name = None;
    let rest_trimmed = rest.trim_start();
    if let Some(after) = rest_trimmed.strip_prefix("to=") {
        let end = after.find(':')?;
        recipient = Some(after[..end].trim().to_string());
        rest = &after[end..];
    } else if *role == "tool" {
        // `tool:get_weather: {…}` — the first segment names the tool.
        if let Some(after) = rest_trimmed.strip_prefix(':') {
            if let Some(end) = after.find(':') {
                let candidate = after[..end].trim();
                if !candidate.is_empty() && !candidate.contains(' ') {
                    name = Some(candidate.to_string());
                    rest = &after[end..];
                }
            }
        }
    }

    let body = rest.trim_start().strip_prefix(':')?;
    Some((
        InMessage {
            role: (*role).to_string(),
            content: String::new(),
            channel,
            recipient,
            name,
        },
        body,
    ))
}

fn normalize_turn(m: InMessage, index: usize) -> Result<Turn, String> {
    let role = m.role.trim().to_ascii_lowercase();
    // The Chat Completions wire format calls tool results "function"/"tool".
    let role = match role.as_str() {
        "function" => "tool".to_string(),
        other => other.to_string(),
    };
    if !ROLES.contains(&role.as_str()) {
        return Err(format!(
            "message #{index}: invalid role — expected one of {}, got '{}'",
            ROLES.join(", "),
            m.role.trim()
        ));
    }
    let channel = match m.channel.as_deref().map(str::trim).filter(|c| !c.is_empty()) {
        Some(c) => {
            if !VALID_CHANNELS.contains(&c) {
                return Err(format!(
                    "message #{index}: invalid channel — expected one of {}, got '{c}'",
                    VALID_CHANNELS.join(", ")
                ));
            }
            Some(c.to_string())
        }
        None => None,
    };
    Ok(Turn {
        role,
        content: m.content,
        channel,
        recipient: m
            .recipient
            .map(|r| r.trim().to_string())
            .filter(|r| !r.is_empty()),
        name: m.name.map(|n| n.trim().to_string()).filter(|n| !n.is_empty()),
    })
}

fn json_kind(v: &serde_json::Value) -> &'static str {
    match v {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "a boolean",
        serde_json::Value::Number(_) => "a number",
        serde_json::Value::String(_) => "a string",
        serde_json::Value::Array(_) => "an array",
        serde_json::Value::Object(_) => "an object",
    }
}

// ---- Chain-of-thought drop rule --------------------------------------------

/// Harmony's guidance: once an assistant turn has landed in the `final` channel,
/// the analysis (chain-of-thought) that preceded it is dropped from history.
/// Analysis after the last `final` turn belongs to an in-flight tool-calling
/// chain and is kept.
fn drop_superseded_analysis(turns: Vec<Turn>) -> Vec<Turn> {
    let last_final = turns
        .iter()
        .rposition(|t| t.role == "assistant" && effective_channel(t) == "final");
    let Some(cut) = last_final else {
        return turns;
    };
    turns
        .into_iter()
        .enumerate()
        .filter(|(i, t)| {
            !(*i < cut && t.role == "assistant" && effective_channel(t) == "analysis")
        })
        .map(|(_, t)| t)
        .collect()
}

/// The channel a turn actually renders on: an assistant turn with a `recipient`
/// is a tool call (commentary), anything else assistant-side defaults to `final`.
fn effective_channel(t: &Turn) -> &str {
    match t.channel.as_deref() {
        Some(c) => c,
        None if t.recipient.is_some() => "commentary",
        None => "final",
    }
}

// ---- Rendering --------------------------------------------------------------

fn render_system(
    identity: &str,
    effort: &str,
    cutoff: &str,
    date: &str,
    has_tools: bool,
) -> String {
    let identity = if identity.trim().is_empty() {
        DEFAULT_IDENTITY
    } else {
        identity.trim()
    };
    let cutoff = if cutoff.trim().is_empty() {
        DEFAULT_CUTOFF
    } else {
        cutoff.trim()
    };

    let mut head = String::from(identity);
    head.push_str(&format!("\nKnowledge cutoff: {cutoff}"));
    if !date.trim().is_empty() {
        head.push_str(&format!("\nCurrent date: {}", date.trim()));
    }

    let mut sections = vec![head];
    if effort != "none" {
        sections.push(format!("Reasoning: {effort}"));
    }
    let mut channels = format!(
        "# Valid channels: {CHANNELS}. Channel must be included for every message."
    );
    if has_tools {
        channels.push_str("\nCalls to these tools must go to the commentary channel: 'functions'.");
    }
    sections.push(channels);

    format!(
        "{T_START}system{T_MESSAGE}{}{T_END}",
        sections.join("\n\n")
    )
}

fn render_developer(instructions: &str, tools: &[ToolDef]) -> String {
    let mut sections: Vec<String> = Vec::new();
    if !instructions.is_empty() {
        sections.push(format!("# Instructions\n\n{instructions}"));
    }
    if !tools.is_empty() {
        let mut block = String::from("# Tools\n\n## functions\n\nnamespace functions {\n\n");
        for t in tools {
            block.push_str(&render_tool(t));
        }
        block.push_str("} // namespace functions");
        sections.push(block);
    }
    format!(
        "{T_START}developer{T_MESSAGE}{}{T_END}",
        sections.join("\n\n")
    )
}

/// One `type name = (_: {…}) => any;` declaration, with the description as a
/// leading `//` comment — the shape gpt-oss was trained to read.
fn render_tool(t: &ToolDef) -> String {
    let mut s = String::new();
    if let Some(d) = t.description.as_deref().map(str::trim).filter(|d| !d.is_empty()) {
        for line in d.lines() {
            s.push_str(&format!("// {line}\n"));
        }
    }
    let props = t
        .parameters
        .as_ref()
        .and_then(|p| p.get("properties"))
        .and_then(|p| p.as_object());
    let required: Vec<&str> = t
        .parameters
        .as_ref()
        .and_then(|p| p.get("required"))
        .and_then(|r| r.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_str()).collect())
        .unwrap_or_default();

    match props {
        Some(props) if !props.is_empty() => {
            s.push_str(&format!("type {} = (_: {{\n", t.name));
            for (key, spec) in props {
                if let Some(desc) = spec.get("description").and_then(|d| d.as_str()) {
                    for line in desc.trim().lines() {
                        s.push_str(&format!("// {line}\n"));
                    }
                }
                let opt = if required.contains(&key.as_str()) { "" } else { "?" };
                let mut line = format!("{key}{opt}: {},", ts_type(spec));
                if let Some(d) = spec.get("default") {
                    line.push_str(&format!(" // default: {d}"));
                }
                s.push_str(&line);
                s.push('\n');
            }
            s.push_str("}) => any;\n\n");
        }
        _ => {
            s.push_str(&format!("type {} = () => any;\n\n", t.name));
        }
    }
    s
}

/// JSON Schema → the TypeScript-ish type text Harmony's tool namespace uses.
fn ts_type(spec: &serde_json::Value) -> String {
    if let Some(vals) = spec.get("enum").and_then(|e| e.as_array()) {
        let parts: Vec<String> = vals
            .iter()
            .map(|v| match v {
                serde_json::Value::String(s) => format!("\"{s}\""),
                other => other.to_string(),
            })
            .collect();
        if !parts.is_empty() {
            return parts.join(" | ");
        }
    }
    match spec.get("type").and_then(|t| t.as_str()) {
        Some("string") => "string".to_string(),
        Some("number") | Some("integer") => "number".to_string(),
        Some("boolean") => "boolean".to_string(),
        Some("array") => {
            let inner = spec
                .get("items")
                .map(ts_type)
                .unwrap_or_else(|| "any".to_string());
            format!("{inner}[]")
        }
        Some("object") => "object".to_string(),
        _ => "any".to_string(),
    }
}

fn render_turn(t: &Turn) -> Result<String, String> {
    match t.role.as_str() {
        "user" => Ok(format!("{T_START}user{T_MESSAGE}{}{T_END}", t.content)),
        "tool" => {
            let name = t.name.as_deref().ok_or_else(|| {
                "a tool message needs a 'name' (the function that produced the output), \
                 e.g. {\"role\":\"tool\",\"name\":\"get_weather\",\"content\":\"…\"}"
                    .to_string()
            })?;
            let author = qualify(name);
            let channel = t.channel.as_deref().unwrap_or("commentary");
            Ok(format!(
                "{T_START}{author} to=assistant{T_CHANNEL}{channel}{T_MESSAGE}{}{T_END}",
                t.content
            ))
        }
        "assistant" => {
            if let Some(recipient) = t.recipient.as_deref() {
                // A tool call: commentary channel, JSON-constrained, `<|call|>` stop.
                let channel = t.channel.as_deref().unwrap_or("commentary");
                Ok(format!(
                    "{T_START}assistant{T_CHANNEL}{channel} to={} {T_CONSTRAIN}json{T_MESSAGE}{}{T_CALL}",
                    qualify(recipient),
                    t.content
                ))
            } else {
                let channel = t.channel.as_deref().unwrap_or("final");
                Ok(format!(
                    "{T_START}assistant{T_CHANNEL}{channel}{T_MESSAGE}{}{T_END}",
                    t.content
                ))
            }
        }
        other => Err(format!(
            "internal: role '{other}' should have been folded into the developer message"
        )),
    }
}

/// Tool authors/recipients are namespaced: `get_weather` → `functions.get_weather`.
/// An already-qualified name (any name containing a `.`) is left alone.
fn qualify(name: &str) -> String {
    if name.contains('.') {
        name.to_string()
    } else {
        format!("functions.{name}")
    }
}

// ---- Tools parsing ----------------------------------------------------------

fn parse_tools(src: &str) -> Result<Vec<ToolDef>, String> {
    let trimmed = src.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }
    let value: serde_json::Value = serde_json::from_str(trimmed).map_err(|e| {
        format!(
            "tools is not valid JSON: {e}. Expected a JSON array like \
             [{{\"name\":\"get_weather\",\"description\":\"…\",\"parameters\":{{…}}}}]."
        )
    })?;
    let arr = match value {
        serde_json::Value::Array(a) => a,
        serde_json::Value::Object(_) => vec![value],
        other => {
            return Err(format!(
                "tools must be a JSON array of function definitions, got {}",
                json_kind(&other)
            ))
        }
    };
    let mut out = Vec::with_capacity(arr.len());
    for (i, item) in arr.iter().enumerate() {
        // Accept the Chat Completions wrapper `{"type":"function","function":{…}}`.
        let inner = item
            .get("function")
            .filter(|f| f.is_object())
            .unwrap_or(item);
        let def: ToolDef = serde_json::from_value(inner.clone()).map_err(|e| {
            format!(
                "tool #{} could not be read: {e}. Expected \
                 {{\"name\":…,\"description\":…,\"parameters\":{{JSON Schema}}}}.",
                i + 1
            )
        })?;
        if def.name.trim().is_empty() {
            return Err(format!("tool #{}: 'name' must not be empty", i + 1));
        }
        out.push(def);
    }
    Ok(out)
}

// ---- Tests ------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn render(messages: &str) -> String {
        run(
            messages, "auto", "", "", "", "medium", "", "", true, "completion", true, "text",
        )
        .unwrap()
    }

    #[test]
    fn renders_a_minimal_system_and_user_prompt() {
        let got = render(r#"[{"role":"user","content":"What is 2 + 2?"}]"#);
        assert_eq!(
            got,
            "<|start|>system<|message|>You are ChatGPT, a large language model trained by OpenAI.\n\
             Knowledge cutoff: 2024-06\n\n\
             Reasoning: medium\n\n\
             # Valid channels: analysis, commentary, final. Channel must be included for every message.<|end|>\
             <|start|>user<|message|>What is 2 + 2?<|end|>\
             <|start|>assistant"
        );
    }

    #[test]
    fn rejects_an_unknown_role() {
        let err = run(
            r#"[{"role":"robot","content":"beep"}]"#,
            "auto",
            "",
            "",
            "",
            "medium",
            "",
            "",
            true,
            "completion",
            true,
            "text",
        )
        .unwrap_err();
        assert!(err.contains("invalid role"), "got: {err}");
        assert!(err.contains("robot"), "got: {err}");
    }

    #[test]
    fn rejects_an_unknown_enum_value() {
        let err = run(
            r#"[{"role":"user","content":"hi"}]"#,
            "auto",
            "",
            "",
            "",
            "extreme",
            "",
            "",
            true,
            "completion",
            true,
            "text",
        )
        .unwrap_err();
        assert!(err.contains("invalid reasoning_effort"), "got: {err}");
    }

    #[test]
    fn rejects_empty_and_invalid_json() {
        assert!(render_err("").contains("no messages found"));
        assert!(render_err("[{\"role\":").contains("not valid JSON"));
        assert!(render_err("[\"hello\"]").contains("must be an object"));
    }

    fn render_err(messages: &str) -> String {
        run(
            messages, "auto", "", "", "", "medium", "", "", true, "completion", true, "text",
        )
        .unwrap_err()
    }

    #[test]
    fn current_date_and_reasoning_none_are_honored() {
        let got = run(
            "user: hi",
            "lines",
            "",
            "",
            "",
            "none",
            "2024-06",
            "2025-06-28",
            true,
            "conversation",
            true,
            "text",
        )
        .unwrap();
        assert!(got.contains("Current date: 2025-06-28"), "got: {got}");
        assert!(!got.contains("Reasoning:"), "got: {got}");
        assert!(!got.ends_with("<|start|>assistant"), "got: {got}");
    }

    #[test]
    fn system_turn_becomes_developer_instructions() {
        let got = render(
            r#"[{"role":"system","content":"Talk like a pirate."},{"role":"user","content":"hi"}]"#,
        );
        assert!(
            got.contains("<|start|>developer<|message|># Instructions\n\nTalk like a pirate.<|end|>"),
            "got: {got}"
        );
        assert!(!got.contains("<|start|>system<|message|>Talk"), "got: {got}");
    }

    #[test]
    fn instructions_param_merges_before_conversation_system_turns() {
        let got = run(
            r#"[{"role":"system","content":"Second."},{"role":"user","content":"hi"}]"#,
            "auto",
            "First.",
            "",
            "",
            "medium",
            "",
            "",
            true,
            "completion",
            true,
            "text",
        )
        .unwrap();
        assert!(got.contains("# Instructions\n\nFirst.\n\nSecond."), "got: {got}");
    }

    #[test]
    fn line_format_parses_channels_recipients_and_continuations() {
        let got = run(
            "user: what is the weather in Oslo?\n\
             assistant[analysis]: the user wants weather\n\
             need to call the tool\n\
             assistant[commentary] to=get_weather: {\"city\":\"Oslo\"}\n\
             tool:get_weather: {\"c\":21}\n\
             assistant: It is 21 C in Oslo.",
            "lines",
            "",
            "",
            "",
            "medium",
            "",
            "",
            false,
            "conversation",
            false,
            "text",
        )
        .unwrap();
        assert_eq!(
            got,
            "<|start|>user<|message|>what is the weather in Oslo?<|end|>\
             <|start|>assistant<|channel|>analysis<|message|>the user wants weather\nneed to call the tool<|end|>\
             <|start|>assistant<|channel|>commentary to=functions.get_weather <|constrain|>json<|message|>{\"city\":\"Oslo\"}<|call|>\
             <|start|>functions.get_weather to=assistant<|channel|>commentary<|message|>{\"c\":21}<|end|>\
             <|start|>assistant<|channel|>final<|message|>It is 21 C in Oslo.<|end|>"
        );
    }

    #[test]
    fn auto_drop_analysis_drops_only_superseded_chain_of_thought() {
        let convo = r#"[
            {"role":"user","content":"a"},
            {"role":"assistant","channel":"analysis","content":"old thinking"},
            {"role":"assistant","channel":"final","content":"answer a"},
            {"role":"user","content":"b"},
            {"role":"assistant","channel":"analysis","content":"live thinking"},
            {"role":"assistant","channel":"commentary","recipient":"get_weather","content":"{}"}
        ]"#;
        let dropped = render(convo);
        assert!(!dropped.contains("old thinking"), "got: {dropped}");
        assert!(dropped.contains("live thinking"), "got: {dropped}");

        let kept = run(
            convo, "auto", "", "", "", "medium", "", "", true, "completion", false, "text",
        )
        .unwrap();
        assert!(kept.contains("old thinking"), "got: {kept}");
    }

    #[test]
    fn tools_render_a_typescript_namespace_and_the_commentary_clause() {
        let tools = r#"[{
            "name": "get_weather",
            "description": "Get the current weather.",
            "parameters": {
              "type": "object",
              "properties": {
                "location": {"type": "string", "description": "City and country."},
                "unit": {"type": "string", "enum": ["celsius", "fahrenheit"], "default": "celsius"}
              },
              "required": ["location"]
            }
        }]"#;
        let got = run(
            r#"[{"role":"user","content":"weather?"}]"#,
            "auto",
            "Be brief.",
            tools,
            "",
            "medium",
            "",
            "",
            true,
            "completion",
            true,
            "text",
        )
        .unwrap();
        assert!(
            got.contains("Calls to these tools must go to the commentary channel: 'functions'."),
            "got: {got}"
        );
        assert!(got.contains("namespace functions {"), "got: {got}");
        assert!(got.contains("// Get the current weather."), "got: {got}");
        assert!(got.contains("location: string,"), "got: {got}");
        assert!(
            got.contains("unit?: \"celsius\" | \"fahrenheit\", // default: \"celsius\""),
            "got: {got}"
        );
        assert!(got.contains("} // namespace functions"), "got: {got}");
    }

    #[test]
    fn chat_completions_tool_wrapper_and_content_parts_are_accepted() {
        let got = run(
            r#"[{"role":"user","content":[{"type":"input_text","text":"hi there"}]}]"#,
            "json",
            "",
            r#"[{"type":"function","function":{"name":"ping","description":"Ping."}}]"#,
            "",
            "medium",
            "",
            "",
            true,
            "completion",
            true,
            "text",
        )
        .unwrap();
        assert!(got.contains("<|start|>user<|message|>hi there<|end|>"), "got: {got}");
        assert!(got.contains("type ping = () => any;"), "got: {got}");
    }

    #[test]
    fn json_output_reports_counts_and_stop_tokens() {
        let got = run(
            r#"[{"role":"user","content":"a"},{"role":"assistant","channel":"analysis","content":"t"},{"role":"assistant","content":"b"}]"#,
            "auto",
            "",
            "",
            "",
            "medium",
            "",
            "",
            true,
            "completion",
            true,
            "json",
        )
        .unwrap();
        let v: serde_json::Value = serde_json::from_str(&got).unwrap();
        assert_eq!(v["message_count"], 3);
        assert_eq!(v["rendered_message_count"], 2);
        assert_eq!(v["dropped_analysis_count"], 1);
        assert_eq!(v["stop_tokens"][0], "<|return|>");
        assert_eq!(v["stop_tokens"][1], "<|call|>");
        assert!(v["prompt"].as_str().unwrap().ends_with("<|start|>assistant"));
    }

    #[test]
    fn custom_identity_and_cutoff_are_used() {
        let got = run(
            "user: hi",
            "lines",
            "",
            "",
            "You are Nemo, a helpful assistant.",
            "high",
            "2025-01",
            "",
            true,
            "completion",
            true,
            "text",
        )
        .unwrap();
        assert!(got.contains("You are Nemo, a helpful assistant.\nKnowledge cutoff: 2025-01"), "got: {got}");
        assert!(got.contains("Reasoning: high"), "got: {got}");
    }

    #[test]
    fn include_system_off_omits_the_metadata_message() {
        let got = run(
            "user: hi", "lines", "", "", "", "medium", "", "", false, "completion", true, "text",
        )
        .unwrap();
        assert_eq!(got, "<|start|>user<|message|>hi<|end|><|start|>assistant");
    }

    #[test]
    fn tool_message_without_a_name_explains_what_is_missing() {
        let err = render_err(r#"[{"role":"tool","content":"{}"}]"#);
        assert!(err.contains("needs a 'name'"), "got: {err}");
    }

    #[test]
    fn caps_are_enforced_at_the_boundary() {
        // One character over the messages cap.
        let big = format!("user: {}", "x".repeat(MAX_INPUT_CHARS));
        let err = run(
            &big, "lines", "", "", "", "medium", "", "", true, "completion", true, "text",
        )
        .unwrap_err();
        assert!(err.contains("messages is too large"), "got: {err}");

        // Exactly at the cap still renders.
        let exact = format!("user: {}", "x".repeat(MAX_INPUT_CHARS - 6));
        assert_eq!(exact.chars().count(), MAX_INPUT_CHARS);
        assert!(run(
            &exact, "lines", "", "", "", "medium", "", "", true, "completion", true, "text",
        )
        .is_ok());

        // Too many turns.
        let many = "user: hi\n".repeat(MAX_MESSAGES + 1);
        let err = run(
            &many, "lines", "", "", "", "medium", "", "", true, "completion", true, "text",
        )
        .unwrap_err();
        assert!(err.contains("too many messages"), "got: {err}");
    }

    #[test]
    fn invalid_channel_is_rejected_with_the_valid_set() {
        let err = render_err(r#"[{"role":"assistant","channel":"secret","content":"x"}]"#);
        assert!(err.contains("invalid channel"), "got: {err}");
        assert!(err.contains("analysis, commentary, final"), "got: {err}");
    }

    #[test]
    fn already_qualified_tool_names_are_not_double_prefixed() {
        let got = render(
            r#"[{"role":"tool","name":"functions.get_weather","content":"{}"}]"#,
        );
        assert!(got.contains("<|start|>functions.get_weather to=assistant"), "got: {got}");
        assert!(!got.contains("functions.functions"), "got: {got}");
    }
}
