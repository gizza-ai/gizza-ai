//! `gizza mcp` — a stdio MCP (Model Context Protocol) server exposing every tool.
//!
//! Framing is the MCP stdio transport: newline-delimited JSON-RPC 2.0, one JSON
//! message per line — NOT LSP `Content-Length` headers. Requests get exactly one
//! response line; notifications get none. EOF on stdin ends the session.

use std::io::Write as _;

use anyhow::Result;
use serde_json::{json, Value};
use tokio::io::AsyncBufReadExt as _;

use crate::{render, runtime::ToolRuntime};

/// Protocol revisions this server can speak. A tools-only server behaves
/// identically under all three, so we echo whichever the client asked for.
const SUPPORTED_PROTOCOL_VERSIONS: &[&str] = &["2024-11-05", "2025-03-26", "2025-06-18"];
const LATEST_PROTOCOL_VERSION: &str = "2025-06-18";

/// JSON-RPC dispatch for one MCP session over a booted [`ToolRuntime`].
pub struct McpServer {
    rt: ToolRuntime,
}

impl McpServer {
    pub fn new(rt: ToolRuntime) -> Self {
        Self { rt }
    }

    /// Handle one raw line from the client. Returns the serialized response for
    /// requests, `None` for notifications and blank lines.
    pub async fn handle_line(&self, line: &str) -> Option<String> {
        let line = line.trim();
        if line.is_empty() {
            return None;
        }
        let msg: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            // Parse errors have no recoverable id → id null per JSON-RPC 2.0.
            Err(_) => return Some(error_response(Value::Null, -32700, "Parse error").to_string()),
        };
        self.handle_message(msg).await.map(|v| v.to_string())
    }

    /// Handle one parsed JSON-RPC message. Requests yield `Some(response)`;
    /// notifications (no `id`) are processed silently and yield `None`.
    pub async fn handle_message(&self, msg: Value) -> Option<Value> {
        let Value::Object(obj) = msg else {
            return Some(error_response(Value::Null, -32600, "Invalid Request"));
        };
        let method = obj
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let params = obj.get("params").cloned().unwrap_or(Value::Null);
        // No id → notification (notifications/initialized, notifications/cancelled, …).
        // Nothing here needs action, and notifications must never be answered.
        let id = obj.get("id").cloned()?;
        Some(match self.dispatch(method, &params).await {
            Ok(result) => json!({"jsonrpc": "2.0", "id": id, "result": result}),
            Err((code, message)) => error_response(id, code, &message),
        })
    }

    async fn dispatch(&self, method: &str, params: &Value) -> Result<Value, (i64, String)> {
        match method {
            "initialize" => Ok(self.initialize(params)),
            "ping" => Ok(json!({})),
            "tools/list" => Ok(self.tools_list()),
            "tools/call" => self.tools_call(params).await,
            other => Err((-32601, format!("Method not found: {other}"))),
        }
    }

    fn initialize(&self, params: &Value) -> Value {
        let requested = params
            .get("protocolVersion")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let version = if SUPPORTED_PROTOCOL_VERSIONS.contains(&requested) {
            requested
        } else {
            LATEST_PROTOCOL_VERSION
        };
        json!({
            "protocolVersion": version,
            "capabilities": {"tools": {}},
            "serverInfo": {"name": "gizza", "version": crate::version()},
            "instructions": "Local gizza compute tools (math, text, image/video/audio via \
                system ffmpeg, web fetch). Tools that produce a file write it to a temp \
                path and return that path in the text content.",
        })
    }

    fn tools_list(&self) -> Value {
        let tools: Vec<Value> = self
            .rt
            .tools()
            .iter()
            .map(|t| {
                json!({
                    "name": t.short,
                    "description": t.description,
                    "inputSchema": t.parameters,
                })
            })
            .collect();
        json!({"tools": tools})
    }

    async fn tools_call(&self, params: &Value) -> Result<Value, (i64, String)> {
        let name = params
            .get("name")
            .and_then(Value::as_str)
            .ok_or_else(|| (-32602, "Missing required param: name".to_string()))?;
        let meta = self
            .rt
            .tool(name)
            .ok_or_else(|| (-32602, format!("Unknown tool: {name}")))?;
        let args = params
            .get("arguments")
            .cloned()
            .unwrap_or_else(|| json!({}));
        let full_name = meta.name.clone();
        let body = match self.rt.run_tool(&full_name, args).await {
            Ok(b) => b,
            // Execution failures are tool-level errors (isError), not protocol errors.
            Err(e) => return Ok(tool_result(format!("Dispatch error: {e}"), true)),
        };
        // Binary envelope (image/video/audio): write the file, return its path.
        if let Some(bin) = render::extract_binary(&body) {
            return Ok(match write_binary_output(&bin) {
                Ok(path) => {
                    let summary = render::render(&body, false).stdout;
                    tool_result(
                        format!("{summary}\nOutput written to {}", path.display()),
                        false,
                    )
                }
                Err(e) => tool_result(format!("Failed to write output file: {e}"), true),
            });
        }
        let rendered = render::render(&body, false);
        Ok(tool_result(rendered.stdout, rendered.exit_code != 0))
    }
}

/// Write a tool's binary output to a unique temp file and return its path.
///
/// Same traversal-safe basename rule as `gizza tool` (`render::safe_default_out`),
/// but placed in the OS temp dir: the MCP server's cwd belongs to the client
/// (often `/`), so outputs must not land there.
pub fn write_binary_output(bin: &render::BinaryOut) -> std::io::Result<std::path::PathBuf> {
    let safe = render::safe_default_out(&bin.filename);
    let name = safe.to_str().unwrap_or("output.bin");
    let file = tempfile::Builder::new()
        .prefix("gizza-mcp-")
        .suffix(&format!("-{name}"))
        .tempfile()?;
    std::fs::write(file.path(), &bin.bytes)?;
    // keep(): the file is handed to the MCP client, so it must outlive us.
    let (_f, path) = file.keep().map_err(|e| e.error)?;
    Ok(path)
}

fn tool_result(text: String, is_error: bool) -> Value {
    json!({"content": [{"type": "text", "text": text}], "isError": is_error})
}

fn error_response(id: Value, code: i64, message: &str) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "error": {"code": code, "message": message}})
}

/// Run the stdio server: read newline-delimited JSON-RPC from stdin, write one
/// response per line to stdout. Returns cleanly on stdin EOF.
pub async fn serve(rt: ToolRuntime) -> Result<()> {
    let server = McpServer::new(rt);
    let mut lines = tokio::io::BufReader::new(tokio::io::stdin()).lines();
    while let Some(line) = lines.next_line().await? {
        if let Some(resp) = server.handle_line(&line).await {
            let mut out = std::io::stdout().lock();
            // Flush per message: stdout is block-buffered on a pipe, and the
            // client is waiting on this exact line.
            writeln!(out, "{resp}")?;
            out.flush()?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn binary_output_lands_in_temp_dir_with_safe_name() {
        let bin = render::BinaryOut {
            filename: "../../evil/cat.png".to_string(),
            bytes: vec![1, 2, 3],
        };
        let path = write_binary_output(&bin).expect("write");
        assert!(path.starts_with(std::env::temp_dir()), "got {path:?}");
        let name = path.file_name().unwrap().to_str().unwrap();
        assert!(name.starts_with("gizza-mcp-"), "got {name}");
        assert!(name.ends_with("-cat.png"), "got {name}");
        assert_eq!(std::fs::read(&path).unwrap(), vec![1, 2, 3]);
        std::fs::remove_file(&path).unwrap();
    }
}
