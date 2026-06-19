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

#[tokio::test]
async fn video_silence_cut_is_registered_and_dispatches() {
    if !have_ffmpeg() {
        eprintln!("skipping: ffmpeg not installed");
        return;
    }
    let rt = runtime::boot_full().await.expect("boot");
    // The two-pass tool must be registered (embedded skill wasm).
    assert!(
        rt.tool("gizza-ai/video-silence-cut").is_some(),
        "video-silence-cut should be a registered tool"
    );
    // bad url → structured error (fails at source-resolution, before pass 1),
    // must not panic. Exercises arg-parsing + descriptor + dispatch wiring.
    let r = rt
        .run_tool(
            "gizza-ai/video-silence-cut",
            serde_json::json!({"url": "http://127.0.0.1:0/nope.mp4"}),
        )
        .await;
    assert!(
        r.is_ok(),
        "dispatch should return a body or error, not panic: {r:?}"
    );
}
