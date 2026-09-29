//! A skill that takes its file from a `url` reaches `wafer-run/network`. The CLI
//! bounds each skill to the capabilities its `#[wafer_block]` declares, so the
//! fetch is only admitted when the block declares the `network` capability and
//! lists the service in `callable_blocks`. This needs no working internet: a
//! DNS failure on an RFC 2606 `.invalid` host proves the request got past
//! authorization and into the network stack.

use gizza_cli::runtime;

#[tokio::test]
async fn file_compressor_url_input_is_admitted_to_the_network() {
    let rt = runtime::boot().await.expect("boot");
    let body = rt
        .run_tool(
            "gizza-ai/file-compressor",
            serde_json::json!({"url": "http://gizza-file-compressor-test.invalid/data.txt"}),
        )
        .await
        .expect("call");
    let text = String::from_utf8_lossy(&body);
    assert!(
        !(text.contains("WRAP") || text.contains("permission")),
        "the url fetch was refused at the authorization layer: {text}"
    );
}
