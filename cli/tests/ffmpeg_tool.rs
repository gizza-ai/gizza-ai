use gizza_cli::runtime;

fn have_ffmpeg() -> bool {
    std::process::Command::new("ffmpeg")
        .arg("-version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[tokio::test]
async fn image_resize_dispatch_does_not_panic() {
    if !have_ffmpeg() {
        eprintln!("skipping: ffmpeg not installed");
        return;
    }
    let rt = runtime::boot_full().await.expect("boot");
    // bad url → structured error, must not panic
    let r = rt
        .run_tool(
            "gizza-ai/image-resize",
            serde_json::json!({"url": "http://127.0.0.1:0/nope.png", "width": 8}),
        )
        .await;
    assert!(
        r.is_ok(),
        "dispatch should return a body or error, not panic: {r:?}"
    );
}
