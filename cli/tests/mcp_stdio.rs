//! End-to-end smoke test for `gizza mcp`: drive the real binary over stdio with
//! a scripted MCP session (initialize → initialized → tools/list → tools/call)
//! and check the newline-delimited JSON-RPC framing.

use std::io::Write as _;
use std::process::{Command, Stdio};

use serde_json::Value;

#[test]
fn stdio_session_initialize_list_call() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_gizza"))
        .arg("mcp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn gizza mcp");

    let mut stdin = child.stdin.take().expect("stdin");
    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","id":0,"method":"initialize","params":{{"protocolVersion":"2025-06-18","capabilities":{{}},"clientInfo":{{"name":"smoke","version":"0"}}}}}}"#
    )
    .expect("write initialize");
    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","method":"notifications/initialized"}}"#
    )
    .expect("write initialized");
    writeln!(stdin, r#"{{"jsonrpc":"2.0","id":1,"method":"tools/list"}}"#).expect("write list");
    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{{"name":"calculator","arguments":{{"expr":"6*7"}}}}}}"#
    )
    .expect("write call");
    // Close stdin → EOF → the server must exit cleanly on its own.
    drop(stdin);

    let out = child.wait_with_output().expect("wait");
    assert!(
        out.status.success(),
        "exit={:?} stderr={}",
        out.status.code(),
        String::from_utf8_lossy(&out.stderr)
    );

    let stdout = String::from_utf8_lossy(&out.stdout);
    let lines: Vec<Value> = stdout
        .lines()
        .map(|l| serde_json::from_str(l).unwrap_or_else(|e| panic!("bad json line ({e}): {l}")))
        .collect();
    // Exactly one response per request; none for the notification.
    assert_eq!(lines.len(), 3, "stdout: {stdout}");

    assert_eq!(lines[0]["id"], 0);
    assert_eq!(lines[0]["result"]["protocolVersion"], "2025-06-18");
    assert_eq!(lines[0]["result"]["serverInfo"]["name"], "gizza");

    assert_eq!(lines[1]["id"], 1);
    let tools = lines[1]["result"]["tools"].as_array().expect("tools");
    assert!(
        tools.iter().any(|t| t["name"] == "calculator"),
        "calculator missing from {} listed tools",
        tools.len()
    );

    assert_eq!(lines[2]["id"], 2);
    assert_eq!(lines[2]["result"]["content"][0]["text"], "42");
    assert_eq!(lines[2]["result"]["isError"], false);
}
