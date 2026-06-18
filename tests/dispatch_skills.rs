//! In-process integration test for gizza-ai/web-fetch.
//!
//! Boots a wafer-run runtime, substitutes a FakeNetworkBlock for
//! wafer-run/network, loads blocks/web-fetch/target/block.wasm into the wasmi
//! runtime, and asserts the cross-block dispatch path (web-fetch → call_block
//! → fake network → response → web-fetch parses → ToolResp) round-trips
//! correctly. No browser, no LLM, no Playwright.

use std::{
    collections::{BTreeMap, HashMap},
    sync::Arc,
};

use base64::{engine::general_purpose::STANDARD as B64_STANDARD, Engine as _};
use serde_json::json;
use wafer_block::{
    codec,
    wire::network::{Request as WireRequest, ResponseHeader},
    Attachment, Block, Context, InputStream, Message, OutputStream, WaferError,
};
use wafer_run::{ErrorCode, Wafer, WasmiBlock};

/// A native test stub for `wafer-run/network`. Decodes the new
/// MessagePack-encoded `wire::network::Request` and emits two-frame responses
/// (header + body chunks) via `OutputStream::from_producer` — mirroring the
/// production handler in `wafer-core::interfaces::network::handler`.
#[derive(Default)]
struct FakeNetworkBlock;

#[async_trait::async_trait]
impl Block for FakeNetworkBlock {
    fn info(&self) -> wafer_block::types::BlockInfo {
        wafer_block::types::BlockInfo::new(
            "wafer-run/network",
            "0.1.0",
            "network@v1",
            "Test stub — emits two-frame codec-encoded responses",
        )
    }

    async fn handle(&self, _ctx: &dyn Context, msg: Message, input: InputStream) -> OutputStream {
        let expected_kind = wafer_block::ServiceOp::NETWORK_DO_REQUEST;
        if msg.kind != expected_kind {
            return OutputStream::error(WaferError::new(
                ErrorCode::InvalidArgument,
                format!("FakeNetworkBlock: unexpected msg.kind {}", msg.kind),
            ));
        }
        let body = input.collect_to_bytes().await;
        let req: WireRequest = match codec::decode(&body) {
            Ok(r) => r,
            Err(e) => {
                return OutputStream::error(WaferError::new(
                    ErrorCode::InvalidArgument,
                    format!("FakeNetworkBlock: invalid wire::network::Request: {e}"),
                ));
            }
        };

        let url = req.url.as_str();

        // Special case: huge.png signals oversize via Content-Length header
        // with an empty body. With the new binary transport this no longer
        // tests an OOM workaround — but the test still asserts that the skill
        // honours the Content-Length pre-check, so keep emitting it.
        let (status, headers, body_bytes): (u16, HashMap<String, Vec<String>>, Vec<u8>) = if url
            == "https://example.test/huge.png"
        {
            let mut h = HashMap::new();
            h.insert("content-type".to_string(), vec!["image/png".to_string()]);
            h.insert("content-length".to_string(), vec!["5242880".to_string()]);
            (200, h, Vec::new())
        } else if url == "https://example.test/huge.mp4" {
            // Special case: huge.mp4 signals oversize via Content-Length header (20 MiB > 10 MiB cap).
            let mut h = HashMap::new();
            h.insert("content-type".to_string(), vec!["video/mp4".to_string()]);
            h.insert("content-length".to_string(), vec!["20971520".to_string()]);
            (200, h, Vec::new())
        } else {
            let (status, content_type, body_bytes): (u16, &str, Vec<u8>) = match url {
                "https://example.test/web-fetch.txt" => {
                    (200, "text/plain", b"WEBFETCH_OK_8f3a2".to_vec())
                }
                // A real 2-page PDF fixture served with the canonical PDF mime.
                "https://example.test/sample.pdf" => {
                    (200, "application/pdf", SAMPLE_PDF.to_vec())
                }
                // The same bytes served as application/octet-stream — the mime
                // raw.githubusercontent.com uses for binary downloads. The
                // pdf-extract-text block's AssetKind::Document must accept it.
                "https://example.test/octet.pdf" => {
                    (200, "application/octet-stream", SAMPLE_PDF.to_vec())
                }
                // Not a PDF, but served with a PDF mime — lopdf must reject the
                // bytes even though the fetch-time mime check passes.
                "https://example.test/fake.pdf" => {
                    (200, "application/pdf", b"this is not a pdf".to_vec())
                }
                "https://example.test/sample.mp4" => (200, "video/mp4", b"FAKE_MP4_BYTES".to_vec()),
                "https://example.test/sample.webm" => {
                    (200, "video/webm", b"FAKE_WEBM_BYTES".to_vec())
                }
                "https://example.test/cat.png" => (200, "image/png", {
                    // Minimal valid-looking PNG header bytes; real validity not required —
                    // the skill doesn't decode the image.
                    let mut v = b"\x89PNG\r\n\x1a\n".to_vec();
                    v.extend_from_slice(&[0u8; 64]);
                    v
                }),
                "https://example.test/not-an-image.html" => {
                    (200, "text/html", b"<html></html>".to_vec())
                }
                "https://example.test/not-a-video.html" => {
                    (200, "text/html", b"<html></html>".to_vec())
                }
                "https://example.test/small.png" => (200, "image/png", {
                    let mut v = b"\x89PNG\r\n\x1a\n".to_vec();
                    v.extend_from_slice(&[0u8; 248]);
                    v
                }),
                "https://example.test/not-an-image-2.html" => {
                    (200, "text/html", b"<html></html>".to_vec())
                }
                _ => (404, "text/plain", Vec::new()),
            };
            let mut h = HashMap::new();
            h.insert("content-type".to_string(), vec![content_type.to_string()]);
            (status, h, body_bytes)
        };

        let header = ResponseHeader {
            status_code: status,
            headers,
        };

        OutputStream::from_producer(move |sink, _cancel| async move {
            let header_bytes = match codec::encode(&header) {
                Ok(b) => b,
                Err(e) => {
                    let _ = sink
                        .error(WaferError::new(
                            ErrorCode::Internal,
                            format!("FakeNetworkBlock: encoding header: {e}"),
                        ))
                        .await;
                    return;
                }
            };
            if sink.send_chunk(header_bytes).await.is_err() {
                return;
            }
            if !body_bytes.is_empty() && sink.send_chunk(body_bytes).await.is_err() {
                return;
            }
            let _ = sink.complete(vec![]).await;
        })
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

// -- pdf-extract-text dispatch tests -----------------------------------------

/// A real 2-page PDF fixture (built with lopdf) whose text layer contains
/// "Gizza PDF Extract Page One" on page 1 and "Second Page Beta Marker" on
/// page 2. Used to prove the wasmi-executed lopdf extraction round-trips.
const SAMPLE_PDF: &[u8] = include_bytes!("fixtures/sample-text.pdf");
const PDF_EXTRACT_WASM: &[u8] = include_bytes!("../blocks/pdf-extract-text/target/block.wasm");

async fn dispatch_pdf_extract(args: serde_json::Value) -> Result<serde_json::Value, String> {
    let mut wafer = Wafer::builder()
        .disable_inventory()
        .disable_lockfile()
        .build()
        .expect("Wafer::build");
    wafer
        .register_block("wafer-run/network", Arc::new(FakeNetworkBlock))
        .expect("register fake network");
    wafer
        .register_block(
            "gizza-ai/pdf-extract-text",
            Arc::new(WasmiBlock::load_from_bytes(PDF_EXTRACT_WASM).expect("load pdf-extract-text")),
        )
        .expect("register pdf-extract-text");
    let wafer = wafer.start().await.expect("start runtime");

    let body = serde_json::to_vec(&args).expect("serialize");
    let output = wafer
        .run_block(
            "gizza-ai/pdf-extract-text",
            Message::new("invoke"),
            InputStream::from_bytes(body),
        )
        .await;
    match output.collect_buffered().await {
        Ok(resp) => Ok(serde_json::from_slice(&resp.body).expect("response JSON")),
        Err(e) => Err(format!("{e:?}")),
    }
}

#[tokio::test]
async fn pdf_extract_all_pages_returns_text() {
    let resp = dispatch_pdf_extract(json!({ "url": "https://example.test/sample.pdf" }))
        .await
        .expect("happy path");
    let text = resp["text"].as_str().expect("text field");
    assert!(text.contains("Gizza"), "should contain page 1 text; got {text:?}");
    assert!(
        text.contains("Beta Marker"),
        "all-pages extraction should include page 2 text; got {text:?}"
    );
    assert_eq!(resp["page"], serde_json::Value::Null, "page is null for all");
    assert_eq!(resp["truncated"], false);
    assert!(resp["chars"].as_u64().unwrap() > 0);
}

#[tokio::test]
async fn pdf_extract_single_page_excludes_other_page() {
    let resp = dispatch_pdf_extract(json!({ "url": "https://example.test/sample.pdf", "page": 2 }))
        .await
        .expect("page 2");
    let text = resp["text"].as_str().expect("text field");
    assert!(text.contains("Beta Marker"), "page 2 text; got {text:?}");
    assert!(
        !text.contains("Page One"),
        "page 2 must not include page 1 text; got {text:?}"
    );
    assert_eq!(resp["page"], 2);
}

#[tokio::test]
async fn pdf_extract_accepts_octet_stream_mime() {
    // raw.githubusercontent.com serves PDFs as application/octet-stream; the
    // Document AssetKind must accept it.
    let resp = dispatch_pdf_extract(json!({ "url": "https://example.test/octet.pdf" }))
        .await
        .expect("octet-stream PDF should be accepted");
    assert!(resp["text"].as_str().unwrap().contains("Gizza"));
}

#[tokio::test]
async fn pdf_extract_rejects_page_out_of_range() {
    let err = dispatch_pdf_extract(json!({ "url": "https://example.test/sample.pdf", "page": 99 }))
        .await
        .expect_err("page 99 of a 2-page PDF must error");
    assert!(
        err.contains("out of range")
            || err.contains("INVALID_ARGUMENT")
            || err.contains("InvalidArgument"),
        "expected out-of-range error; got {err}"
    );
}

#[tokio::test]
async fn pdf_extract_rejects_non_pdf_bytes() {
    let err = dispatch_pdf_extract(json!({ "url": "https://example.test/fake.pdf" }))
        .await
        .expect_err("non-PDF bytes must error");
    assert!(
        err.contains("failed to parse PDF")
            || err.contains("INVALID_ARGUMENT")
            || err.contains("InvalidArgument"),
        "expected parse error; got {err}"
    );
}

#[tokio::test]
async fn pdf_extract_rejects_page_zero() {
    let err = dispatch_pdf_extract(json!({ "url": "https://example.test/sample.pdf", "page": 0 }))
        .await
        .expect_err("page 0 must error (pages are 1-based)");
    assert!(
        err.contains(">= 1") || err.contains("INVALID_ARGUMENT") || err.contains("InvalidArgument"),
        "expected 1-based error; got {err}"
    );
}

// -- ffmpeg-runtime dispatch test ----------------------------------------------

/// Test stub: `FfmpegService` impl that returns canned bytes regardless of args.
/// The call counter lets oversize-input tests assert that ffmpeg-runtime is
/// not called when the skill rejects pre-fetch.
struct FakeFfmpegService {
    canned_exit_code: i32,
    canned_output: Vec<u8>,
    canned_log: String,
    call_count: std::sync::atomic::AtomicUsize,
}

impl FakeFfmpegService {
    fn ok(output: Vec<u8>, log: impl Into<String>) -> Self {
        Self {
            canned_exit_code: 0,
            canned_output: output,
            canned_log: log.into(),
            call_count: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    #[allow(dead_code)]
    fn fail(exit_code: i32, log: impl Into<String>) -> Self {
        Self {
            canned_exit_code: exit_code,
            canned_output: Vec::new(),
            canned_log: log.into(),
            call_count: std::sync::atomic::AtomicUsize::new(0),
        }
    }

    #[allow(dead_code)]
    fn calls(&self) -> usize {
        self.call_count.load(std::sync::atomic::Ordering::SeqCst)
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl gizza_ai::ffmpeg::FfmpegService for FakeFfmpegService {
    async fn exec(
        &self,
        _args: gizza_ai::ffmpeg::ExecArgs,
    ) -> Result<gizza_ai::ffmpeg::ExecResult, gizza_ai::ffmpeg::FfmpegError> {
        self.call_count
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(gizza_ai::ffmpeg::ExecResult {
            exit_code: self.canned_exit_code,
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

    let svc: Arc<dyn gizza_ai::ffmpeg::FfmpegService> = Arc::new(FakeFfmpegService::ok(
        b"FAKE_FFMPEG_OUT".to_vec(),
        "fake ok",
    ));
    wafer
        .register_block(
            "gizza-ai/ffmpeg-runtime",
            Arc::new(gizza_ai::ffmpeg::FfmpegBlock::new(svc)),
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
    let ffmpeg_svc: Arc<dyn gizza_ai::ffmpeg::FfmpegService> = Arc::new(FakeFfmpegService::ok(
        Vec::new(),
        "Stream #0:0: Video: h264, yuv420p, 1920x1080, 30 fps",
    ));
    wafer
        .register_block(
            "gizza-ai/ffmpeg-runtime",
            Arc::new(gizza_ai::ffmpeg::FfmpegBlock::new(ffmpeg_svc)),
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
        parsed["info"].as_str().unwrap_or_default().contains("h264"),
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
        msg.contains("expected image/*")
            || msg.contains("InvalidArgument")
            || msg.contains("INVALID_ARGUMENT"),
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

// -- image-resize / image-crop / image-convert dispatch tests ----------------

async fn dispatch_image_op(
    block_name: &'static str,
    wasm_path_relative_to_target: &'static str,
    wasm: &'static [u8],
    url: &str,
    extra: serde_json::Value,
    ffmpeg_svc: Arc<FakeFfmpegService>,
) -> Result<serde_json::Value, wafer_block::TerminalNotResponse> {
    let _ = wasm_path_relative_to_target; // documentation only

    let mut wafer = Wafer::builder()
        .disable_inventory()
        .disable_lockfile()
        .build()
        .expect("Wafer::build");
    wafer
        .register_block("wafer-run/network", Arc::new(FakeNetworkBlock))
        .expect("register fake network");
    let svc_dyn: Arc<dyn gizza_ai::ffmpeg::FfmpegService> = ffmpeg_svc.clone();
    wafer
        .register_block(
            "gizza-ai/ffmpeg-runtime",
            Arc::new(gizza_ai::ffmpeg::FfmpegBlock::new(svc_dyn)),
        )
        .expect("register ffmpeg-runtime");
    wafer
        .register_block(
            block_name,
            Arc::new(WasmiBlock::load_from_bytes(wasm).expect("load skill wasm")),
        )
        .expect("register skill");
    let wafer = wafer.start().await.expect("start runtime");

    let mut req = serde_json::json!({ "url": url });
    if let serde_json::Value::Object(map) = extra {
        for (k, v) in map {
            req[k] = v;
        }
    }
    let body = serde_json::to_vec(&req).expect("serialize");

    let output = wafer
        .run_block(
            block_name,
            Message::new("invoke"),
            InputStream::from_bytes(body),
        )
        .await;
    match output.collect_buffered().await {
        Ok(resp) => Ok(serde_json::from_slice(&resp.body).expect("envelope JSON")),
        Err(e) => Err(e),
    }
}

const RESIZE_WASM: &[u8] = include_bytes!("../blocks/image-resize/target/block.wasm");
const CROP_WASM: &[u8] = include_bytes!("../blocks/image-crop/target/block.wasm");
const CONVERT_WASM: &[u8] = include_bytes!("../blocks/image-convert/target/block.wasm");

#[tokio::test]
async fn image_resize_happy_path_returns_envelope() {
    let svc = Arc::new(FakeFfmpegService::ok(
        b"\x89PNG\r\n\x1a\n"
            .iter()
            .chain([0u8; 200].iter())
            .copied()
            .collect(),
        "fake resize log",
    ));
    let env = dispatch_image_op(
        "gizza-ai/image-resize",
        "image-resize",
        RESIZE_WASM,
        "https://example.test/small.png",
        serde_json::json!({"width": 256}),
        svc.clone(),
    )
    .await
    .expect("happy path returns Ok terminal");
    let for_llm = env["_for_llm"].as_str().expect("_for_llm string");
    assert!(for_llm.contains("resized"), "got {for_llm:?}");
    assert!(for_llm.contains("256 wide"), "got {for_llm:?}");
    let for_ui = &env["_for_ui"];
    assert!(
        for_ui["data_url"]
            .as_str()
            .unwrap()
            .starts_with("data:image/png;base64,"),
        "data_url shape; got {:?}",
        for_ui["data_url"]
    );
    assert_eq!(for_ui["mime"], "image/png");
    let filename = for_ui["filename"].as_str().expect("filename string");
    assert!(filename.ends_with(".png"));
    assert!(filename.contains("resized") || filename.contains("256"));
    assert_eq!(svc.calls(), 1);
}

#[tokio::test]
async fn image_resize_rejects_missing_dims() {
    let svc = Arc::new(FakeFfmpegService::ok(b"x".to_vec(), ""));
    let result = dispatch_image_op(
        "gizza-ai/image-resize",
        "image-resize",
        RESIZE_WASM,
        "https://example.test/small.png",
        serde_json::json!({}),
        svc.clone(),
    )
    .await;
    assert!(result.is_err());
    let err = format!("{:?}", result.unwrap_err());
    assert!(
        err.contains("INVALID_ARGUMENT")
            || err.contains("InvalidArgument")
            || err.contains("width")
            || err.contains("height"),
        "expected arg error, got {err}"
    );
    assert_eq!(svc.calls(), 0);
}

#[tokio::test]
async fn image_resize_rejects_oversize_input() {
    let svc = Arc::new(FakeFfmpegService::ok(b"x".to_vec(), ""));
    let result = dispatch_image_op(
        "gizza-ai/image-resize",
        "image-resize",
        RESIZE_WASM,
        "https://example.test/huge.png",
        serde_json::json!({"width": 256}),
        svc.clone(),
    )
    .await;
    assert!(result.is_err());
    let err = format!("{:?}", result.unwrap_err());
    assert!(
        err.contains("OUT_OF_RANGE") || err.contains("OutOfRange") || err.contains("too large"),
        "expected size error, got {err}"
    );
    assert_eq!(svc.calls(), 0);
}

#[tokio::test]
async fn image_resize_rejects_non_image_content_type() {
    let svc = Arc::new(FakeFfmpegService::ok(b"x".to_vec(), ""));
    let result = dispatch_image_op(
        "gizza-ai/image-resize",
        "image-resize",
        RESIZE_WASM,
        "https://example.test/not-an-image-2.html",
        serde_json::json!({"width": 256}),
        svc.clone(),
    )
    .await;
    assert!(result.is_err());
    let err = format!("{:?}", result.unwrap_err());
    assert!(
        err.contains("INVALID_ARGUMENT")
            || err.contains("InvalidArgument")
            || err.contains("expected image"),
        "expected mime error, got {err}"
    );
    assert_eq!(svc.calls(), 0);
}

#[tokio::test]
async fn image_resize_surfaces_ffmpeg_failure() {
    let svc = Arc::new(FakeFfmpegService::fail(
        1,
        "Invalid argument: unsupported pixel format",
    ));
    let result = dispatch_image_op(
        "gizza-ai/image-resize",
        "image-resize",
        RESIZE_WASM,
        "https://example.test/small.png",
        serde_json::json!({"width": 256}),
        svc.clone(),
    )
    .await;
    assert!(result.is_err());
    let err = format!("{:?}", result.unwrap_err());
    assert!(
        err.contains("ffmpeg failed") || err.contains("INTERNAL") || err.contains("Internal"),
        "expected internal/ffmpeg error, got {err}"
    );
    assert_eq!(svc.calls(), 1);
}

#[tokio::test]
async fn image_resize_rejects_cover_with_single_dim() {
    let svc = Arc::new(FakeFfmpegService::ok(b"x".to_vec(), ""));
    let result = dispatch_image_op(
        "gizza-ai/image-resize",
        "image-resize",
        RESIZE_WASM,
        "https://example.test/small.png",
        serde_json::json!({"width": 256, "fit": "cover"}),
        svc.clone(),
    )
    .await;
    assert!(result.is_err());
    let err = format!("{:?}", result.unwrap_err());
    assert!(
        err.contains("INVALID_ARGUMENT") || err.contains("cover requires both"),
        "expected arg error, got {err}"
    );
    assert_eq!(svc.calls(), 0);
}

// -- image-crop ---

#[tokio::test]
async fn image_crop_happy_path_returns_envelope() {
    let svc = Arc::new(FakeFfmpegService::ok(
        b"\x89PNG\r\n\x1a\n"
            .iter()
            .chain([0u8; 200].iter())
            .copied()
            .collect(),
        "fake crop log",
    ));
    let env = dispatch_image_op(
        "gizza-ai/image-crop",
        "image-crop",
        CROP_WASM,
        "https://example.test/small.png",
        serde_json::json!({"x": 10, "y": 20, "width": 100, "height": 100}),
        svc.clone(),
    )
    .await
    .expect("happy path");
    assert!(env["_for_llm"].as_str().unwrap().contains("cropped"));
    assert!(env["_for_llm"].as_str().unwrap().contains("(10,20)"));
    assert!(env["_for_ui"]["data_url"]
        .as_str()
        .unwrap()
        .starts_with("data:image/png;base64,"));
    assert!(env["_for_ui"]["filename"]
        .as_str()
        .unwrap()
        .contains("cropped"));
    assert_eq!(svc.calls(), 1);
}

#[tokio::test]
async fn image_crop_rejects_missing_required_args() {
    let svc = Arc::new(FakeFfmpegService::ok(b"x".to_vec(), ""));
    let result = dispatch_image_op(
        "gizza-ai/image-crop",
        "image-crop",
        CROP_WASM,
        "https://example.test/small.png",
        serde_json::json!({"x": 0, "y": 0}),
        svc.clone(),
    )
    .await;
    assert!(result.is_err());
    assert_eq!(svc.calls(), 0);
}

#[tokio::test]
async fn image_crop_rejects_zero_width() {
    let svc = Arc::new(FakeFfmpegService::ok(b"x".to_vec(), ""));
    let result = dispatch_image_op(
        "gizza-ai/image-crop",
        "image-crop",
        CROP_WASM,
        "https://example.test/small.png",
        serde_json::json!({"x": 0, "y": 0, "width": 0, "height": 100}),
        svc.clone(),
    )
    .await;
    assert!(result.is_err());
    assert_eq!(svc.calls(), 0);
}

#[tokio::test]
async fn image_crop_rejects_oversize_input() {
    let svc = Arc::new(FakeFfmpegService::ok(b"x".to_vec(), ""));
    let result = dispatch_image_op(
        "gizza-ai/image-crop",
        "image-crop",
        CROP_WASM,
        "https://example.test/huge.png",
        serde_json::json!({"x": 0, "y": 0, "width": 100, "height": 100}),
        svc.clone(),
    )
    .await;
    assert!(result.is_err());
    assert_eq!(svc.calls(), 0);
}

#[tokio::test]
async fn image_crop_rejects_non_image_content_type() {
    let svc = Arc::new(FakeFfmpegService::ok(b"x".to_vec(), ""));
    let result = dispatch_image_op(
        "gizza-ai/image-crop",
        "image-crop",
        CROP_WASM,
        "https://example.test/not-an-image-2.html",
        serde_json::json!({"x": 0, "y": 0, "width": 100, "height": 100}),
        svc.clone(),
    )
    .await;
    assert!(result.is_err());
    assert_eq!(svc.calls(), 0);
}

#[tokio::test]
async fn image_crop_surfaces_ffmpeg_failure() {
    let svc = Arc::new(FakeFfmpegService::fail(1, "crop region out of bounds"));
    let result = dispatch_image_op(
        "gizza-ai/image-crop",
        "image-crop",
        CROP_WASM,
        "https://example.test/small.png",
        serde_json::json!({"x": 9999, "y": 9999, "width": 100, "height": 100}),
        svc.clone(),
    )
    .await;
    assert!(result.is_err());
    assert_eq!(svc.calls(), 1);
}

// -- image-convert ---

#[tokio::test]
async fn image_convert_happy_path_to_jpeg() {
    let svc = Arc::new(FakeFfmpegService::ok(
        b"\xff\xd8\xff\xe0"
            .iter()
            .chain([0u8; 200].iter())
            .copied()
            .collect(),
        "fake convert log",
    ));
    let env = dispatch_image_op(
        "gizza-ai/image-convert",
        "image-convert",
        CONVERT_WASM,
        "https://example.test/small.png",
        serde_json::json!({"format": "jpeg"}),
        svc.clone(),
    )
    .await
    .expect("happy path");
    assert!(env["_for_llm"].as_str().unwrap().contains("converted"));
    assert!(env["_for_llm"].as_str().unwrap().contains("image/jpeg"));
    assert!(env["_for_ui"]["data_url"]
        .as_str()
        .unwrap()
        .starts_with("data:image/jpeg;base64,"));
    assert_eq!(env["_for_ui"]["mime"], "image/jpeg");
    assert!(env["_for_ui"]["filename"]
        .as_str()
        .unwrap()
        .ends_with(".jpg"));
    assert_eq!(svc.calls(), 1);
}

#[tokio::test]
async fn image_convert_rejects_unknown_format() {
    let svc = Arc::new(FakeFfmpegService::ok(b"x".to_vec(), ""));
    let result = dispatch_image_op(
        "gizza-ai/image-convert",
        "image-convert",
        CONVERT_WASM,
        "https://example.test/small.png",
        serde_json::json!({"format": "tiff"}),
        svc.clone(),
    )
    .await;
    assert!(result.is_err());
    let err = format!("{:?}", result.unwrap_err());
    assert!(err.contains("INVALID_ARGUMENT") || err.contains("not supported"));
    assert_eq!(svc.calls(), 0);
}

#[tokio::test]
async fn image_convert_rejects_quality_out_of_range() {
    let svc = Arc::new(FakeFfmpegService::ok(b"x".to_vec(), ""));
    let result = dispatch_image_op(
        "gizza-ai/image-convert",
        "image-convert",
        CONVERT_WASM,
        "https://example.test/small.png",
        serde_json::json!({"format": "jpeg", "quality": 200}),
        svc.clone(),
    )
    .await;
    assert!(result.is_err());
    assert_eq!(svc.calls(), 0);
}

#[tokio::test]
async fn image_convert_rejects_oversize_input() {
    let svc = Arc::new(FakeFfmpegService::ok(b"x".to_vec(), ""));
    let result = dispatch_image_op(
        "gizza-ai/image-convert",
        "image-convert",
        CONVERT_WASM,
        "https://example.test/huge.png",
        serde_json::json!({"format": "jpeg"}),
        svc.clone(),
    )
    .await;
    assert!(result.is_err());
    assert_eq!(svc.calls(), 0);
}

#[tokio::test]
async fn image_convert_rejects_non_image_content_type() {
    let svc = Arc::new(FakeFfmpegService::ok(b"x".to_vec(), ""));
    let result = dispatch_image_op(
        "gizza-ai/image-convert",
        "image-convert",
        CONVERT_WASM,
        "https://example.test/not-an-image-2.html",
        serde_json::json!({"format": "jpeg"}),
        svc.clone(),
    )
    .await;
    assert!(result.is_err());
    assert_eq!(svc.calls(), 0);
}

#[tokio::test]
async fn image_convert_surfaces_ffmpeg_failure() {
    let svc = Arc::new(FakeFfmpegService::fail(1, "encoder not found"));
    let result = dispatch_image_op(
        "gizza-ai/image-convert",
        "image-convert",
        CONVERT_WASM,
        "https://example.test/small.png",
        serde_json::json!({"format": "webp"}),
        svc.clone(),
    )
    .await;
    assert!(result.is_err());
    assert_eq!(svc.calls(), 1);
}

// -- Skill-output chaining: image-resize → image-crop via ref attachment ----

/// Native caller block: dispatches a target block with a single attachment under
/// the id "call_1". The bytes come from the caller's input stream; the args
/// passed to the test fixture are forwarded verbatim. The target block sees
/// `{ref: "call_1"}` in args and pulls bytes via `wafer_sdk::lookup_attachment`.
#[derive(Clone)]
struct ChainCallerBlock {
    target_block: String,
    args: serde_json::Value,
    attached_bytes: Vec<u8>,
    attachment_id: String,
}

#[async_trait::async_trait]
impl Block for ChainCallerBlock {
    fn info(&self) -> wafer_block::types::BlockInfo {
        wafer_block::types::BlockInfo::new(
            "test/chain-caller",
            "0.0.0",
            "h@v1",
            "Drives a chained call to a configurable target block with attachments.",
        )
    }

    async fn handle(&self, ctx: &dyn Context, _msg: Message, _input: InputStream) -> OutputStream {
        let mut atts = BTreeMap::new();
        atts.insert(
            self.attachment_id.clone(),
            Attachment {
                mime: "image/png".into(),
                bytes: self.attached_bytes.clone(),
                filename: Some("from-resize.png".into()),
            },
        );

        let body = serde_json::to_vec(&self.args).expect("serialize args");

        let mut msg = Message::new("invoke");
        msg.set_meta(wafer_block::meta::META_REQ_ACTION, "create");

        match ctx
            .call_block_buffered_with_attachments(&self.target_block, msg, &body, atts)
            .await
        {
            Ok(resp) => OutputStream::respond(resp.body),
            Err(e) => OutputStream::error(e),
        }
    }
}

#[tokio::test]
async fn dispatch_chains_resize_then_crop_via_ref() {
    // Step 1: resize an image normally to obtain real PNG bytes the crop can use.
    // Use FakeFfmpegService::ok with a deterministic PNG-shaped byte sequence.
    let resize_output_bytes: Vec<u8> = b"\x89PNG\r\n\x1a\n"
        .iter()
        .chain([0u8; 200].iter())
        .copied()
        .collect();
    let resize_svc = Arc::new(FakeFfmpegService::ok(
        resize_output_bytes.clone(),
        "fake resize log",
    ));

    let resize_envelope = dispatch_image_op(
        "gizza-ai/image-resize",
        "image-resize",
        RESIZE_WASM,
        "https://example.test/cat.png",
        json!({ "width": 100 }),
        resize_svc,
    )
    .await
    .expect("resize succeeds");

    // The resize envelope's _for_ui.data_url has the bytes we want to chain.
    let data_url = resize_envelope["_for_ui"]["data_url"]
        .as_str()
        .expect("data_url present");
    let comma = data_url.find(',').expect("data: URL has comma");
    let header = &data_url[..comma];
    let b64 = &data_url[comma + 1..];
    assert!(header.starts_with("data:image/"));
    let chained_bytes = B64_STANDARD.decode(b64).expect("data_url base64 decodes");
    assert!(!chained_bytes.is_empty(), "resize produced no bytes");

    // Step 2: build a runtime with image-crop + chain caller. Drive the caller.
    let mut wafer = Wafer::builder()
        .disable_inventory()
        .disable_lockfile()
        .build()
        .expect("Wafer::build");
    wafer
        .register_block("wafer-run/network", Arc::new(FakeNetworkBlock))
        .expect("register fake network");
    let crop_svc: Arc<dyn gizza_ai::ffmpeg::FfmpegService> = Arc::new(FakeFfmpegService::ok(
        b"\x89PNG\r\n\x1a\n"
            .iter()
            .chain([0u8; 88].iter())
            .copied()
            .collect(),
        "fake crop log",
    ));
    wafer
        .register_block(
            "gizza-ai/ffmpeg-runtime",
            Arc::new(gizza_ai::ffmpeg::FfmpegBlock::new(crop_svc)),
        )
        .expect("register ffmpeg-runtime");
    wafer
        .register_block(
            "gizza-ai/image-crop",
            Arc::new(WasmiBlock::load_from_bytes(CROP_WASM).expect("load image-crop")),
        )
        .expect("register image-crop");
    wafer
        .register_block(
            "test/chain-caller",
            Arc::new(ChainCallerBlock {
                target_block: "gizza-ai/image-crop".into(),
                args: json!({
                    "ref": "call_1",
                    "x": 0,
                    "y": 0,
                    "width": 50,
                    "height": 50,
                }),
                attachment_id: "call_1".into(),
                attached_bytes: chained_bytes,
            }),
        )
        .expect("register chain caller");
    let wafer = wafer.start().await.expect("start runtime");

    let output = wafer
        .run_block(
            "test/chain-caller",
            Message::new("invoke"),
            InputStream::empty(),
        )
        .await;
    let resp = output
        .collect_buffered()
        .await
        .expect("chain-caller returns success");

    let envelope: serde_json::Value =
        serde_json::from_slice(&resp.body).expect("crop returns SP4 envelope");
    let data_url = envelope["_for_ui"]["data_url"]
        .as_str()
        .expect("crop emits data_url");
    assert!(data_url.starts_with("data:image/"));
    assert!(data_url.len() > 100, "cropped data_url suspiciously small");
}

// -- video-transcode dispatch tests ------------------------------------------

const TRANSCODE_WASM: &[u8] = include_bytes!("../blocks/video-transcode/target/block.wasm");
const TRIM_WASM: &[u8] = include_bytes!("../blocks/video-trim/target/block.wasm");
const FRAME_EXTRACT_WASM: &[u8] = include_bytes!("../blocks/video-frame-extract/target/block.wasm");

#[tokio::test]
async fn video_transcode_happy_path_returns_envelope() {
    let svc = Arc::new(FakeFfmpegService::ok(
        b"\x00\x00\x00\x18ftypmp42"
            .iter()
            .chain([0u8; 200].iter())
            .copied()
            .collect(),
        "fake transcode log",
    ));
    let env = dispatch_image_op(
        "gizza-ai/video-transcode",
        "video-transcode",
        TRANSCODE_WASM,
        "https://example.test/sample.mp4",
        json!({ "format": "mp4", "quality": 75 }),
        svc,
    )
    .await
    .expect("transcode succeeds");
    assert_eq!(env["_for_ui"]["mime"], "video/mp4");
    assert!(env["_for_ui"]["data_url"]
        .as_str()
        .unwrap()
        .starts_with("data:video/mp4;base64,"));
}

#[tokio::test]
async fn video_transcode_rejects_oversize_input() {
    let svc = Arc::new(FakeFfmpegService::ok(b"unused".to_vec(), "unused"));
    let result = dispatch_image_op(
        "gizza-ai/video-transcode",
        "video-transcode",
        TRANSCODE_WASM,
        "https://example.test/huge.mp4",
        json!({ "format": "mp4" }),
        svc.clone(),
    )
    .await;
    assert!(result.is_err(), "oversize must error");
    assert_eq!(
        svc.calls(),
        0,
        "ffmpeg-runtime must not be called when input rejected"
    );
}

#[tokio::test]
async fn video_transcode_rejects_non_video_content_type() {
    let svc = Arc::new(FakeFfmpegService::ok(b"unused".to_vec(), "unused"));
    let result = dispatch_image_op(
        "gizza-ai/video-transcode",
        "video-transcode",
        TRANSCODE_WASM,
        "https://example.test/not-a-video.html",
        json!({ "format": "mp4" }),
        svc.clone(),
    )
    .await;
    assert!(result.is_err());
    assert_eq!(svc.calls(), 0);
}

#[tokio::test]
async fn video_transcode_surfaces_ffmpeg_failure() {
    let svc = Arc::new(FakeFfmpegService::fail(127, "ffmpeg crashed"));
    let result = dispatch_image_op(
        "gizza-ai/video-transcode",
        "video-transcode",
        TRANSCODE_WASM,
        "https://example.test/sample.mp4",
        json!({ "format": "mp4" }),
        svc,
    )
    .await;
    assert!(result.is_err());
}

// -- video-trim dispatch tests -----------------------------------------------

#[tokio::test]
async fn video_trim_happy_path_returns_envelope() {
    let svc = Arc::new(FakeFfmpegService::ok(
        b"\x00\x00\x00\x18ftypmp42"
            .iter()
            .chain([0u8; 88].iter())
            .copied()
            .collect(),
        "fake trim log",
    ));
    let env = dispatch_image_op(
        "gizza-ai/video-trim",
        "video-trim",
        TRIM_WASM,
        "https://example.test/sample.mp4",
        json!({ "start": 1.0, "duration": 2.0 }),
        svc,
    )
    .await
    .expect("trim succeeds");
    assert_eq!(env["_for_ui"]["mime"], "video/mp4");
}

#[tokio::test]
async fn video_trim_rejects_oversize_input() {
    let svc = Arc::new(FakeFfmpegService::ok(b"unused".to_vec(), "unused"));
    let result = dispatch_image_op(
        "gizza-ai/video-trim",
        "video-trim",
        TRIM_WASM,
        "https://example.test/huge.mp4",
        json!({ "start": 0.0, "duration": 1.0 }),
        svc.clone(),
    )
    .await;
    assert!(result.is_err());
    assert_eq!(svc.calls(), 0);
}

#[tokio::test]
async fn video_trim_rejects_non_video_content_type() {
    let svc = Arc::new(FakeFfmpegService::ok(b"unused".to_vec(), "unused"));
    let result = dispatch_image_op(
        "gizza-ai/video-trim",
        "video-trim",
        TRIM_WASM,
        "https://example.test/not-a-video.html",
        json!({ "start": 0.0, "duration": 1.0 }),
        svc.clone(),
    )
    .await;
    assert!(result.is_err());
    assert_eq!(svc.calls(), 0);
}

#[tokio::test]
async fn video_trim_surfaces_ffmpeg_failure() {
    let svc = Arc::new(FakeFfmpegService::fail(1, "stream copy failed"));
    let result = dispatch_image_op(
        "gizza-ai/video-trim",
        "video-trim",
        TRIM_WASM,
        "https://example.test/sample.mp4",
        json!({ "start": 0.0, "duration": 1.0 }),
        svc,
    )
    .await;
    assert!(result.is_err());
}

// -- video-frame-extract dispatch tests --------------------------------------

#[tokio::test]
async fn video_frame_extract_happy_path_returns_envelope() {
    let svc = Arc::new(FakeFfmpegService::ok(
        b"\x89PNG\r\n\x1a\n"
            .iter()
            .chain([0u8; 200].iter())
            .copied()
            .collect(),
        "fake frame extract log",
    ));
    let env = dispatch_image_op(
        "gizza-ai/video-frame-extract",
        "video-frame-extract",
        FRAME_EXTRACT_WASM,
        "https://example.test/sample.mp4",
        json!({ "timestamp": 1.5 }),
        svc,
    )
    .await
    .expect("frame-extract succeeds");
    assert_eq!(env["_for_ui"]["mime"], "image/png");
    assert!(env["_for_ui"]["data_url"]
        .as_str()
        .unwrap()
        .starts_with("data:image/png;base64,"));
}

#[tokio::test]
async fn video_frame_extract_rejects_oversize_input() {
    let svc = Arc::new(FakeFfmpegService::ok(b"unused".to_vec(), "unused"));
    let result = dispatch_image_op(
        "gizza-ai/video-frame-extract",
        "video-frame-extract",
        FRAME_EXTRACT_WASM,
        "https://example.test/huge.mp4",
        json!({ "timestamp": 0.0 }),
        svc.clone(),
    )
    .await;
    assert!(result.is_err());
    assert_eq!(svc.calls(), 0);
}

#[tokio::test]
async fn video_frame_extract_rejects_non_video_content_type() {
    let svc = Arc::new(FakeFfmpegService::ok(b"unused".to_vec(), "unused"));
    let result = dispatch_image_op(
        "gizza-ai/video-frame-extract",
        "video-frame-extract",
        FRAME_EXTRACT_WASM,
        "https://example.test/not-a-video.html",
        json!({ "timestamp": 0.0 }),
        svc.clone(),
    )
    .await;
    assert!(result.is_err());
    assert_eq!(svc.calls(), 0);
}

#[tokio::test]
async fn video_frame_extract_surfaces_ffmpeg_failure() {
    let svc = Arc::new(FakeFfmpegService::fail(1, "no decoder for stream"));
    let result = dispatch_image_op(
        "gizza-ai/video-frame-extract",
        "video-frame-extract",
        FRAME_EXTRACT_WASM,
        "https://example.test/sample.mp4",
        json!({ "timestamp": 0.0 }),
        svc,
    )
    .await;
    assert!(result.is_err());
}

// -- Cross-modality chain: video-frame-extract → image-resize via ref --------

#[tokio::test]
async fn dispatch_chains_video_frame_extract_then_image_resize_via_ref() {
    // Step 1: extract a frame from a fake video to obtain real PNG bytes.
    let png_bytes: Vec<u8> = b"\x89PNG\r\n\x1a\n"
        .iter()
        .chain([0u8; 200].iter())
        .copied()
        .collect();
    let extract_svc = Arc::new(FakeFfmpegService::ok(
        png_bytes.clone(),
        "fake frame extract log",
    ));

    let extract_envelope = dispatch_image_op(
        "gizza-ai/video-frame-extract",
        "video-frame-extract",
        FRAME_EXTRACT_WASM,
        "https://example.test/sample.mp4",
        json!({ "timestamp": 1.5 }),
        extract_svc,
    )
    .await
    .expect("frame-extract succeeds");

    let data_url = extract_envelope["_for_ui"]["data_url"]
        .as_str()
        .expect("data_url present");
    assert!(data_url.starts_with("data:image/png;base64,"));
    let comma = data_url.find(',').expect("data: URL has comma");
    let b64 = &data_url[comma + 1..];
    let chained_bytes = base64::engine::general_purpose::STANDARD
        .decode(b64)
        .expect("data_url base64 decodes");
    assert!(!chained_bytes.is_empty());
    let _ = png_bytes; // (kept for clarity; chained_bytes is the verified roundtrip)

    // Step 2: drive image-resize with {ref: "call_1"} + the extracted bytes.
    let mut wafer = Wafer::builder()
        .disable_inventory()
        .disable_lockfile()
        .build()
        .expect("Wafer::build");
    wafer
        .register_block("wafer-run/network", Arc::new(FakeNetworkBlock))
        .expect("register fake network");
    let resize_svc: Arc<dyn gizza_ai::ffmpeg::FfmpegService> = Arc::new(FakeFfmpegService::ok(
        b"\x89PNG\r\n\x1a\n"
            .iter()
            .chain([0u8; 88].iter())
            .copied()
            .collect(),
        "fake resize log",
    ));
    wafer
        .register_block(
            "gizza-ai/ffmpeg-runtime",
            Arc::new(gizza_ai::ffmpeg::FfmpegBlock::new(resize_svc)),
        )
        .expect("register ffmpeg-runtime");
    wafer
        .register_block(
            "gizza-ai/image-resize",
            Arc::new(WasmiBlock::load_from_bytes(RESIZE_WASM).expect("load image-resize")),
        )
        .expect("register image-resize");
    wafer
        .register_block(
            "test/chain-caller",
            Arc::new(ChainCallerBlock {
                target_block: "gizza-ai/image-resize".into(),
                args: json!({
                    "ref": "call_1",
                    "width": 50,
                }),
                attached_bytes: chained_bytes,
                attachment_id: "call_1".into(),
            }),
        )
        .expect("register chain caller");
    let wafer = wafer.start().await.expect("start runtime");

    let output = wafer
        .run_block(
            "test/chain-caller",
            Message::new("invoke"),
            InputStream::empty(),
        )
        .await;
    let resp = output
        .collect_buffered()
        .await
        .expect("chain-caller returns success");

    let envelope: serde_json::Value =
        serde_json::from_slice(&resp.body).expect("resize returns SP4 envelope");
    let resized_data_url = envelope["_for_ui"]["data_url"]
        .as_str()
        .expect("resize emits data_url");
    assert!(resized_data_url.starts_with("data:image/"));
    assert!(resized_data_url.len() > 100);
}

#[tokio::test]
async fn dispatch_uploaded_image_then_resize_via_ref() {
    // Simulate what the agent block does post-decode_uploads: it pre-seeds
    // the per-loop attachment map with bytes keyed by `upload_N`. The LLM
    // then emits {ref:"upload_1"} which routes through dispatch_tool —
    // exactly the path ChainCallerBlock exercises by parameterising
    // `attachment_id`.
    let png_bytes: Vec<u8> = b"\x89PNG\r\n\x1a\n"
        .iter()
        .chain([0u8; 200].iter())
        .copied()
        .collect();

    let mut wafer = Wafer::builder()
        .disable_inventory()
        .disable_lockfile()
        .build()
        .expect("Wafer::build");
    wafer
        .register_block("wafer-run/network", Arc::new(FakeNetworkBlock))
        .expect("register fake network");
    let resize_svc: Arc<dyn gizza_ai::ffmpeg::FfmpegService> = Arc::new(FakeFfmpegService::ok(
        b"\x89PNG\r\n\x1a\n"
            .iter()
            .chain([0u8; 88].iter())
            .copied()
            .collect(),
        "fake resize log",
    ));
    wafer
        .register_block(
            "gizza-ai/ffmpeg-runtime",
            Arc::new(gizza_ai::ffmpeg::FfmpegBlock::new(resize_svc)),
        )
        .expect("register ffmpeg-runtime");
    wafer
        .register_block(
            "gizza-ai/image-resize",
            Arc::new(WasmiBlock::load_from_bytes(RESIZE_WASM).expect("load image-resize")),
        )
        .expect("register image-resize");
    wafer
        .register_block(
            "test/chain-caller",
            Arc::new(ChainCallerBlock {
                target_block: "gizza-ai/image-resize".into(),
                args: json!({
                    "ref": "upload_1",
                    "width": 100,
                }),
                attached_bytes: png_bytes,
                attachment_id: "upload_1".into(),
            }),
        )
        .expect("register chain caller");
    let wafer = wafer.start().await.expect("start runtime");

    let output = wafer
        .run_block(
            "test/chain-caller",
            Message::new("invoke"),
            InputStream::empty(),
        )
        .await;
    let resp = output
        .collect_buffered()
        .await
        .expect("chain-caller returns success");

    let envelope: serde_json::Value =
        serde_json::from_slice(&resp.body).expect("resize returns SP4 envelope");
    let resized_data_url = envelope["_for_ui"]["data_url"]
        .as_str()
        .expect("resize emits data_url");
    assert!(resized_data_url.starts_with("data:image/"));
    assert!(resized_data_url.len() > 100);
}
