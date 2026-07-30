use gizza_cli::runtime;

#[tokio::test]
async fn imagine_is_unsupported_in_cli() {
    let rt = runtime::boot_full().await.expect("boot");
    let body = rt
        .run_tool("gizza-ai/imagine", serde_json::json!({"prompt": "a cat"}))
        .await
        .expect("dispatch");
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["error"], "unsupported_in_cli");
    assert_eq!(gizza_cli::render::render(&body, false).exit_code, 3);
}

#[tokio::test]
async fn background_removal_is_unsupported_in_cli() {
    let rt = runtime::boot_full().await.expect("boot");
    let body = rt
        .run_tool(
            "gizza-ai/image-background-remove-ai",
            serde_json::json!({"url": "https://example.com/product.jpg"}),
        )
        .await
        .expect("dispatch");
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["error"], "unsupported_in_cli");
    assert_eq!(gizza_cli::render::render(&body, false).exit_code, 3);
}
