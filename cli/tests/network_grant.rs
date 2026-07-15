//! The full runtime must authorize skill network egress at the WRAP layer.
//!
//! WRAP default-denies typed service resources unless the host grants them;
//! `boot_full` installs a network grant so tools like web-fetch can reach the
//! network service at all. These tests don't need working internet: a DNS
//! failure on an RFC 2606 `.invalid` host proves the request got PAST
//! authorization and into the network stack.

use gizza_cli::runtime;

/// Error text that would indicate the WRAP grant or capability gate fired.
fn is_authorization_failure(text: &str) -> bool {
    text.contains("WRAP") || text.contains("PermissionDenied") || text.contains("permission")
}

#[tokio::test]
async fn web_fetch_egress_is_wrap_authorized() {
    let rt = runtime::boot_full().await.expect("boot");
    let body = rt
        .run_tool(
            "gizza-ai/web-fetch",
            serde_json::json!({"url": "http://gizza-wrap-grant-test.invalid/"}),
        )
        .await
        .expect("call");
    let text = String::from_utf8_lossy(&body);
    // The fetch must fail in the network stack (DNS), not at authorization.
    assert!(
        !is_authorization_failure(&text),
        "network egress was denied at the authorization layer: {text}"
    );
}
