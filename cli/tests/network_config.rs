//! The CLI's block config comes from the process environment: the
//! `wafer-run/network` block reads its declared `WAFER_RUN__NETWORK__*` limits
//! through `EnvConfigSource` at its Init, so a value the user sets in their
//! shell reaches the block. An invalid value fails the network block's Init,
//! naming the key, instead of being ignored.
//!
//! Its own test binary: it sets a process-wide environment variable.

use gizza_cli::runtime;

#[tokio::test]
async fn invalid_network_limit_from_env_fails_the_network_block_naming_the_key() {
    // SAFETY: this test binary runs this single test, so no other thread
    // reads the environment concurrently.
    unsafe { std::env::set_var("WAFER_RUN__NETWORK__MAX_RESPONSE_BYTES", "not-a-number") };
    let rt = runtime::boot().await.expect("boot");
    let body = rt
        .run_tool(
            "gizza-ai/web-fetch",
            serde_json::json!({"url": "http://gizza-network-config-test.invalid/"}),
        )
        .await
        .expect("call");
    let text = String::from_utf8_lossy(&body);
    assert!(
        text.contains("WAFER_RUN__NETWORK__MAX_RESPONSE_BYTES"),
        "the network block's Init must reject the env value, naming the key: {text}"
    );
}
