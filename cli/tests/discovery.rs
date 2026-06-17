use gizza_cli::runtime;

#[tokio::test]
async fn lists_calculator_with_schema() {
    let rt = runtime::boot_minimal().await.expect("boot");
    let tools = rt.tools();
    let calc = tools
        .iter()
        .find(|t| t.name == "gizza-ai/calculator")
        .expect("calc present");
    assert!(
        calc.description.to_lowercase().contains("expression"),
        "description should mention 'expression', got: {}",
        calc.description
    );
    assert_eq!(calc.parameters["required"][0], "expr");
}

#[tokio::test]
async fn video_tools_present_despite_stale_manifest() {
    let rt = runtime::boot_minimal().await.expect("boot");
    assert!(
        rt.tools().iter().any(|t| t.name == "gizza-ai/video-trim"),
        "video-trim must come from info(), not manifest"
    );
}
