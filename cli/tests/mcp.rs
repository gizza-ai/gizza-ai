//! Dispatch-logic tests for the `gizza mcp` stdio server: handshake, tool
//! listing/calling, and JSON-RPC error/notification semantics.

use gizza_cli::{mcp::McpServer, runtime};
use serde_json::{json, Value};

async fn server() -> McpServer {
    McpServer::new(runtime::boot_minimal().await.expect("boot"))
}

#[tokio::test]
async fn initialize_echoes_supported_protocol_version() {
    let s = server().await;
    let resp = s
        .handle_message(json!({
            "jsonrpc": "2.0", "id": 0, "method": "initialize",
            "params": {
                "protocolVersion": "2025-03-26",
                "capabilities": {},
                "clientInfo": {"name": "test", "version": "0"}
            }
        }))
        .await
        .expect("response");
    assert_eq!(resp["id"], 0);
    let result = &resp["result"];
    assert_eq!(result["protocolVersion"], "2025-03-26");
    assert!(result["capabilities"]["tools"].is_object(), "got {result}");
    assert_eq!(result["serverInfo"]["name"], "gizza");
    assert_eq!(result["serverInfo"]["version"], gizza_cli::version());
}

#[tokio::test]
async fn initialize_with_unknown_version_offers_latest() {
    let s = server().await;
    let resp = s
        .handle_message(json!({
            "jsonrpc": "2.0", "id": 1, "method": "initialize",
            "params": {"protocolVersion": "1999-01-01"}
        }))
        .await
        .expect("response");
    assert_eq!(resp["result"]["protocolVersion"], "2025-06-18");
}

#[tokio::test]
async fn tools_list_has_calculator_with_schema() {
    let s = server().await;
    let resp = s
        .handle_message(json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list"}))
        .await
        .expect("response");
    let tools = resp["result"]["tools"].as_array().expect("tools array");
    assert!(!tools.is_empty());
    let calc = tools
        .iter()
        .find(|t| t["name"] == "calculator")
        .unwrap_or_else(|| panic!("calculator not listed in {tools:?}"));
    assert!(
        !calc["description"].as_str().unwrap_or_default().is_empty(),
        "got {calc}"
    );
    // inputSchema is the manifest's parameters JSON Schema, passed through verbatim.
    assert_eq!(calc["inputSchema"]["type"], "object");
    assert!(
        calc["inputSchema"]["properties"]["expr"].is_object(),
        "got {calc}"
    );
}

#[tokio::test]
async fn tools_call_calculator_returns_text_content() {
    let s = server().await;
    let resp = s
        .handle_message(json!({
            "jsonrpc": "2.0", "id": 3, "method": "tools/call",
            "params": {"name": "calculator", "arguments": {"expr": "6*7"}}
        }))
        .await
        .expect("response");
    let result = &resp["result"];
    assert_eq!(result["isError"], false);
    assert_eq!(result["content"][0]["type"], "text");
    assert_eq!(result["content"][0]["text"], "42");
}

#[tokio::test]
async fn tools_call_tool_error_sets_is_error_not_rpc_error() {
    let s = server().await;
    let resp = s
        .handle_message(json!({
            "jsonrpc": "2.0", "id": 4, "method": "tools/call",
            "params": {"name": "calculator", "arguments": {"expr": "1/0"}}
        }))
        .await
        .expect("response");
    assert!(
        resp.get("error").is_none(),
        "tool failure must not be a JSON-RPC error: {resp}"
    );
    assert_eq!(resp["result"]["isError"], true);
    assert!(resp["result"]["content"][0]["text"].is_string());
}

#[tokio::test]
async fn tools_call_unknown_tool_is_invalid_params() {
    let s = server().await;
    let resp = s
        .handle_message(json!({
            "jsonrpc": "2.0", "id": 5, "method": "tools/call",
            "params": {"name": "no-such-tool", "arguments": {}}
        }))
        .await
        .expect("response");
    assert_eq!(resp["error"]["code"], -32602);
}

#[tokio::test]
async fn ping_returns_empty_result() {
    let s = server().await;
    let resp = s
        .handle_message(json!({"jsonrpc": "2.0", "id": 6, "method": "ping"}))
        .await
        .expect("response");
    assert_eq!(resp["result"], json!({}));
}

#[tokio::test]
async fn unknown_method_is_method_not_found() {
    let s = server().await;
    let resp = s
        .handle_message(json!({"jsonrpc": "2.0", "id": 7, "method": "resources/list"}))
        .await
        .expect("response");
    assert_eq!(resp["error"]["code"], -32601);
    assert_eq!(resp["id"], 7);
}

#[tokio::test]
async fn malformed_json_is_parse_error_with_null_id() {
    let s = server().await;
    let resp = s.handle_line("{not json").await.expect("response");
    let v: Value = serde_json::from_str(&resp).expect("valid json response");
    assert_eq!(v["error"]["code"], -32700);
    assert_eq!(v["id"], Value::Null);
}

#[tokio::test]
async fn notification_gets_no_reply() {
    let s = server().await;
    let resp = s
        .handle_message(json!({"jsonrpc": "2.0", "method": "notifications/initialized"}))
        .await;
    assert!(
        resp.is_none(),
        "notifications must never be answered: {resp:?}"
    );
    // Even an unknown-method notification stays silent.
    let resp = s
        .handle_message(json!({"jsonrpc": "2.0", "method": "notifications/whatever"}))
        .await;
    assert!(resp.is_none());
}

#[tokio::test]
async fn blank_line_is_ignored() {
    let s = server().await;
    assert!(s.handle_line("   ").await.is_none());
}
