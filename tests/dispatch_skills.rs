//! In-process integration test for gizza-ai/web-fetch.
//!
//! Boots a wafer-run runtime, substitutes a FakeNetworkBlock for
//! wafer-run/network, loads blocks/web-fetch/target/block.wasm into the wasmi
//! runtime, and asserts the cross-block dispatch path (web-fetch → call_block
//! → fake network → response → web-fetch parses → ToolResp) round-trips
//! correctly. No browser, no LLM, no Playwright.

use std::sync::Arc;

use serde_json::json;
use wafer_block::{Block, Context, InputStream, Message, OutputStream, WaferError};
use wafer_run::{ErrorCode, WasmiBlock, Wafer};

/// A native test stub for `wafer-run/network`. Returns canned 200 responses for
/// the URL the test uses, 404 for everything else. Lives inline because no
/// other test consumer needs it yet — promote to `wafer-test-support` if a
/// second skill grows a similar dispatch test.
#[derive(Default)]
struct FakeNetworkBlock;

#[async_trait::async_trait]
impl Block for FakeNetworkBlock {
    fn info(&self) -> wafer_block::types::BlockInfo {
        wafer_block::types::BlockInfo::new(
            "wafer-run/network",
            "0.1.0",
            "network@v1",
            "Test stub — returns canned HTTP responses",
        )
    }

    async fn handle(
        &self,
        _ctx: &dyn Context,
        msg: Message,
        input: InputStream,
    ) -> OutputStream {
        let expected_kind = wafer_block::ServiceOp::NETWORK_DO_REQUEST;
        if msg.kind != expected_kind {
            return OutputStream::error(WaferError::new(
                ErrorCode::INVALID_ARGUMENT,
                format!("FakeNetworkBlock: unexpected msg.kind {}", msg.kind),
            ));
        }
        let body = input.collect_to_bytes().await;
        let req: serde_json::Value = match serde_json::from_slice(&body) {
            Ok(v) => v,
            Err(e) => {
                return OutputStream::error(WaferError::new(
                    ErrorCode::INVALID_ARGUMENT,
                    format!("FakeNetworkBlock: invalid request body: {e}"),
                ));
            }
        };

        let url = req["url"].as_str().unwrap_or("");

        // Special case: huge.png signals oversize via Content-Length header with
        // an empty body. The wasmi JSON transport inflates binary data ~6x, so
        // a 5 MiB body would exceed the 16 MiB WASM memory cap before the skill
        // code even runs. Skills that check Content-Length can pre-reject without
        // receiving the full body.
        if url == "https://example.test/huge.png" {
            // Signal oversize via Content-Length header with an empty body.
            // A 5 MiB body would inflate to ~30 MiB through the wasmi JSON
            // transport, exceeding the 16 MiB WASM memory cap. Skills should
            // check Content-Length first and pre-reject without reading the body.
            let huge_resp = serde_json::json!({
                "status_code": 200u16,
                "headers": {
                    "content-type": ["image/png"],
                    "content-length": ["5242880"]
                },
                "body": serde_json::Value::Array(vec![])
            });
            return OutputStream::respond(
                serde_json::to_vec(&huge_resp).expect("serialize huge response"),
            );
        }

        let (status, content_type, body_bytes): (u16, &str, Vec<u8>) = match url {
            "https://example.test/web-fetch.txt" => (200, "text/plain", b"WEBFETCH_OK_8f3a2".to_vec()),
            "https://example.test/sample.mp4" => (200, "video/mp4", b"FAKE_MP4_BYTES".to_vec()),
            "https://example.test/cat.png" => (200, "image/png", {
                // Minimal valid-looking PNG header bytes; real validity not required —
                // the skill doesn't decode the image.
                let mut v = b"\x89PNG\r\n\x1a\n".to_vec();
                v.extend_from_slice(&[0u8; 64]);
                v
            }),
            "https://example.test/not-an-image.html" => (200, "text/html", b"<html></html>".to_vec()),
            _ => (404, "text/plain", Vec::new()),
        };

        OutputStream::respond(
            serde_json::to_vec(&json!({
                "status_code": status,
                "headers": { "content-type": [content_type] },
                "body": body_bytes,
            }))
            .expect("serialize fake response"),
        )
    }
}

#[tokio::test]
async fn web_fetch_round_trips_through_network_block() {
    let mut wafer = Wafer::builder()
        .disable_inventory()
        .disable_lockfile()
        .build()
        .expect("Wafer::build");

    wafer
        .register_block("wafer-run/network", Arc::new(FakeNetworkBlock))
        .expect("register fake network");

    let wasm: &[u8] = include_bytes!("../blocks/web-fetch/target/block.wasm");
    let block = WasmiBlock::load_from_bytes(wasm).expect("load web-fetch wasm");
    wafer
        .register_block("gizza-ai/web-fetch", Arc::new(block))
        .expect("register web-fetch");

    let wafer = wafer.start().await.expect("start runtime");

    let req_body = serde_json::to_vec(&json!({
        "url": "https://example.test/web-fetch.txt"
    }))
    .expect("serialize request body");

    let output = wafer
        .run_block(
            "gizza-ai/web-fetch",
            Message::new("invoke"),
            InputStream::from_bytes(req_body),
        )
        .await;

    let resp = output
        .collect_buffered()
        .await
        .expect("web-fetch returned a non-success terminal");

    let parsed: serde_json::Value =
        serde_json::from_slice(&resp.body).expect("web-fetch body is JSON");

    assert_eq!(parsed["status"], 200, "status field");
    assert_eq!(
        parsed["url"], "https://example.test/web-fetch.txt",
        "url echoed back"
    );
    assert_eq!(parsed["body"], "WEBFETCH_OK_8f3a2", "body marker");
    assert_eq!(parsed["truncated"], false, "truncated flag");
}

// -- ffmpeg-runtime dispatch test ----------------------------------------------

/// Test stub: `FfmpegService` impl that returns canned bytes regardless of args.
struct FakeFfmpegService {
    canned_output: Vec<u8>,
    canned_log: String,
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl gizza_ai::ffmpeg::FfmpegService for FakeFfmpegService {
    async fn exec(
        &self,
        _args: gizza_ai::ffmpeg::ExecArgs,
    ) -> Result<gizza_ai::ffmpeg::ExecResult, gizza_ai::ffmpeg::FfmpegError> {
        Ok(gizza_ai::ffmpeg::ExecResult {
            exit_code: 0,
            output: self.canned_output.clone(),
            log: self.canned_log.clone(),
        })
    }
}

#[tokio::test]
async fn ffmpeg_block_dispatches_to_service() {
    let mut wafer = Wafer::builder()
        .disable_inventory()
        .disable_lockfile()
        .build()
        .expect("Wafer::builder().build()");

    let svc: Arc<dyn gizza_ai::ffmpeg::FfmpegService> = Arc::new(FakeFfmpegService {
        canned_output: b"FAKE_FFMPEG_OUT".to_vec(),
        canned_log: "fake ok".into(),
    });
    wafer
        .register_block(
            "gizza-ai/ffmpeg-runtime",
            Arc::new(gizza_ai::blocks::ffmpeg::FfmpegBlock::new(svc)),
        )
        .expect("register ffmpeg-runtime");

    let wafer = wafer.start().await.expect("start runtime");

    let body = serde_json::to_vec(&gizza_ai::ffmpeg::ExecArgs {
        args: vec!["-i".into(), "in".into(), "out".into()],
        inputs: vec![("in".into(), b"hello".to_vec())],
        output: "out".into(),
    })
    .expect("serialize args");

    let output = wafer
        .run_block(
            "gizza-ai/ffmpeg-runtime",
            Message::new("ffmpeg.exec"),
            InputStream::from_bytes(body),
        )
        .await;

    let resp = output
        .collect_buffered()
        .await
        .expect("ffmpeg-runtime non-success terminal");

    let result: gizza_ai::ffmpeg::ExecResult =
        serde_json::from_slice(&resp.body).expect("ffmpeg result is JSON");

    assert_eq!(result.exit_code, 0);
    assert_eq!(result.output, b"FAKE_FFMPEG_OUT");
    assert_eq!(result.log, "fake ok");
}

// -- ffmpeg skill (two-hop) dispatch test ------------------------------------

#[tokio::test]
async fn ffmpeg_skill_two_hop_dispatch() {
    let mut wafer = Wafer::builder()
        .disable_inventory()
        .disable_lockfile()
        .build()
        .expect("Wafer::builder().build()");

    // Hop 1 target: fake network block returning canned bytes for the URL.
    wafer
        .register_block("wafer-run/network", Arc::new(FakeNetworkBlock))
        .expect("register fake network");

    // Hop 2 target: fake ffmpeg-runtime returning a canned log.
    let ffmpeg_svc: Arc<dyn gizza_ai::ffmpeg::FfmpegService> = Arc::new(FakeFfmpegService {
        canned_output: Vec::new(),
        canned_log: "Stream #0:0: Video: h264, yuv420p, 1920x1080, 30 fps".into(),
    });
    wafer
        .register_block(
            "gizza-ai/ffmpeg-runtime",
            Arc::new(gizza_ai::blocks::ffmpeg::FfmpegBlock::new(ffmpeg_svc)),
        )
        .expect("register ffmpeg-runtime");

    // The skill itself.
    let wasm: &[u8] = include_bytes!("../blocks/ffmpeg/target/block.wasm");
    wafer
        .register_block(
            "gizza-ai/ffmpeg",
            Arc::new(WasmiBlock::load_from_bytes(wasm).expect("load ffmpeg skill")),
        )
        .expect("register ffmpeg skill");

    let wafer = wafer.start().await.expect("start runtime");

    let body = serde_json::to_vec(&json!({
        "url": "https://example.test/sample.mp4"
    }))
    .expect("serialize request body");

    let output = wafer
        .run_block(
            "gizza-ai/ffmpeg",
            Message::new("invoke"),
            InputStream::from_bytes(body),
        )
        .await;

    let resp = output
        .collect_buffered()
        .await
        .expect("ffmpeg skill non-success terminal");

    let parsed: serde_json::Value =
        serde_json::from_slice(&resp.body).expect("ffmpeg skill body is JSON");

    assert_eq!(parsed["url"], "https://example.test/sample.mp4");
    assert!(
        parsed["info"]
            .as_str()
            .unwrap_or_default()
            .contains("h264"),
        "info field should contain the canned ffmpeg log marker; got: {}",
        parsed["info"]
    );
}

// -- image-fetch dispatch tests ----------------------------------------------

async fn dispatch_image_fetch(url: &str) -> serde_json::Value {
    let mut wafer = Wafer::builder()
        .disable_inventory()
        .disable_lockfile()
        .build()
        .expect("Wafer::build");

    wafer
        .register_block("wafer-run/network", Arc::new(FakeNetworkBlock))
        .expect("register fake network");

    let wasm: &[u8] = include_bytes!("../blocks/image-fetch/target/block.wasm");
    let block = WasmiBlock::load_from_bytes(wasm).expect("load image-fetch wasm");
    wafer
        .register_block("gizza-ai/image-fetch", Arc::new(block))
        .expect("register image-fetch");

    let wafer = wafer.start().await.expect("start runtime");

    let req_body = serde_json::to_vec(&json!({ "url": url })).expect("serialize");

    let output = wafer
        .run_block(
            "gizza-ai/image-fetch",
            Message::new("invoke"),
            InputStream::from_bytes(req_body),
        )
        .await;

    let resp = output
        .collect_buffered()
        .await
        .expect("image-fetch returned a non-success terminal");

    serde_json::from_slice(&resp.body).expect("image-fetch body is JSON")
}

#[tokio::test]
async fn image_fetch_skill_returns_envelope() {
    let parsed = dispatch_image_fetch("https://example.test/cat.png").await;
    let for_llm = parsed["_for_llm"].as_str().expect("_for_llm string");
    assert!(
        for_llm.contains("image/png"),
        "_for_llm should mention mime; got {for_llm:?}"
    );
    let for_ui = &parsed["_for_ui"];
    assert!(for_ui.is_object(), "_for_ui should be an object");
    let data_url = for_ui["data_url"].as_str().expect("data_url string");
    assert!(
        data_url.starts_with("data:image/png;base64,"),
        "data_url should start with data:image/png;base64, got {data_url:?}"
    );
    assert_eq!(for_ui["mime"], "image/png");
    assert_eq!(for_ui["filename"], "cat.png");
}

#[tokio::test]
async fn image_fetch_rejects_non_image_content_type() {
    let mut wafer = Wafer::builder()
        .disable_inventory()
        .disable_lockfile()
        .build()
        .expect("Wafer::build");
    wafer
        .register_block("wafer-run/network", Arc::new(FakeNetworkBlock))
        .expect("register fake network");
    let wasm: &[u8] = include_bytes!("../blocks/image-fetch/target/block.wasm");
    wafer
        .register_block(
            "gizza-ai/image-fetch",
            Arc::new(WasmiBlock::load_from_bytes(wasm).expect("load")),
        )
        .expect("register");
    let wafer = wafer.start().await.expect("start");

    let req_body = serde_json::to_vec(&json!({
        "url": "https://example.test/not-an-image.html"
    }))
    .expect("serialize");
    let output = wafer
        .run_block(
            "gizza-ai/image-fetch",
            Message::new("invoke"),
            InputStream::from_bytes(req_body),
        )
        .await;
    // Expect the terminal to be an Err (skill returned WaferError).
    let result = output.collect_buffered().await;
    assert!(
        result.is_err(),
        "non-image content-type should produce a terminal error, got Ok"
    );
    let err = result.unwrap_err();
    let msg = format!("{err:?}");
    assert!(
        msg.contains("expected image/*") || msg.contains("InvalidArgument") || msg.contains("INVALID_ARGUMENT"),
        "error should mention the mime mismatch; got {msg}"
    );
}

#[tokio::test]
async fn image_fetch_rejects_oversize_body() {
    let mut wafer = Wafer::builder()
        .disable_inventory()
        .disable_lockfile()
        .build()
        .expect("Wafer::build");
    wafer
        .register_block("wafer-run/network", Arc::new(FakeNetworkBlock))
        .expect("register fake network");
    let wasm: &[u8] = include_bytes!("../blocks/image-fetch/target/block.wasm");
    wafer
        .register_block(
            "gizza-ai/image-fetch",
            Arc::new(WasmiBlock::load_from_bytes(wasm).expect("load")),
        )
        .expect("register");
    let wafer = wafer.start().await.expect("start");

    let req_body = serde_json::to_vec(&json!({
        "url": "https://example.test/huge.png"
    }))
    .expect("serialize");
    let output = wafer
        .run_block(
            "gizza-ai/image-fetch",
            Message::new("invoke"),
            InputStream::from_bytes(req_body),
        )
        .await;
    let result = output.collect_buffered().await;
    assert!(
        result.is_err(),
        "oversize body should produce a terminal error, got Ok"
    );
    let err = result.unwrap_err();
    let msg = format!("{err:?}");
    assert!(
        msg.contains("too large") || msg.contains("OutOfRange") || msg.contains("OUT_OF_RANGE"),
        "error should mention oversize; got {msg}"
    );
}
