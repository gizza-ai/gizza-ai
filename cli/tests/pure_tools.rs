use gizza_cli::runtime;

#[tokio::test]
async fn boots_and_registers_calculator() {
    let rt = runtime::boot_minimal().await.expect("boot");
    let names = rt.tool_names();
    assert!(
        names.iter().any(|n| n == "gizza-ai/calculator"),
        "got {names:?}"
    );
}

#[tokio::test]
async fn calculator_evaluates() {
    let rt = runtime::boot_minimal().await.expect("boot");
    let body = rt
        .run_tool("gizza-ai/calculator", serde_json::json!({"expr":"2+2"}))
        .await
        .expect("call");
    let v: serde_json::Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(v["result"], 4.0);
}

#[tokio::test]
async fn calculator_div_by_zero_errors() {
    let rt = runtime::boot_minimal().await.expect("boot");
    let body = rt
        .run_tool("gizza-ai/calculator", serde_json::json!({"expr":"1/0"}))
        .await
        .expect("call");
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(v.get("error").is_some(), "expected error envelope, got {v}");
}
