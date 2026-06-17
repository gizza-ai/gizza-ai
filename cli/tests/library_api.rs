#[tokio::test]
async fn run_tool_calculator() {
    let body = gizza_cli::run_tool("calculator", serde_json::json!({"expr":"6*7"}))
        .await
        .expect("run");
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["result"], 42.0);
}

#[tokio::test]
async fn list_and_describe() {
    let tools = gizza_cli::list_tools().await.expect("list");
    assert!(
        tools.iter().any(|t| t.short == "calculator"),
        "calculator not found in list; tools = {:?}",
        tools.iter().map(|t| &t.short).collect::<Vec<_>>()
    );
    let d = gizza_cli::describe_tool("calculator").await.expect("desc");
    assert!(d.is_some(), "describe_tool returned None for calculator");
}
