//! Boot must stay lazy: registering skill stubs and starting the runtime
//! (which eagerly dispatches `lifecycle(Init)`/`Start` to every block, same
//! as the app's `init_all_blocks()` at SW boot) must NOT fetch any skill
//! wasm. Regression guard for the bug where `LazySkillBlock::lifecycle`
//! force-loaded, so every runtime boot downloaded all ~47 `/blocks/*.wasm`
//! (~10+ MiB) — defeating the lazy-load that keeps the app bundle small.
//!
//! The wasm only loads on the first real `handle()` — and the deferred Init
//! must still let the skill answer correctly.

use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use serde_json::json;
use wafer_block::{types::BlockInfo, wafer_async_trait, InputStream, Message};
use wafer_run::asset_loader::NoopAssetLoader;
use wafer_run::{ResourceLimits, Wafer};

use gizza_ai::lazy_skill::{LazySkillBlock, SkillWasmFetcher};

/// Serves the committed calculator block wasm, counting every fetch. Yields
/// once before returning so concurrent first-use callers genuinely interleave
/// at the fetch await point (as a real network fetch would).
struct CountingFetcher {
    fetches: AtomicUsize,
}

#[wafer_async_trait]
impl SkillWasmFetcher for CountingFetcher {
    async fn fetch(&self, _url: &str) -> Result<Vec<u8>, String> {
        self.fetches.fetch_add(1, Ordering::SeqCst);
        tokio::task::yield_now().await;
        Ok(include_bytes!("../blocks/calculator/target/block.wasm").to_vec())
    }
}

#[tokio::test]
async fn boot_does_not_fetch_skill_wasm_first_use_does() {
    let manifest = include_str!("../blocks/calculator/manifest.json");
    let info: BlockInfo = serde_json::from_str(manifest).expect("calculator manifest");
    let name = info.name.clone();

    let fetcher = Arc::new(CountingFetcher {
        fetches: AtomicUsize::new(0),
    });
    let block = LazySkillBlock::new(
        info,
        "/blocks/calculator.wasm".to_string(),
        ResourceLimits::default(),
        Arc::new(NoopAssetLoader),
        fetcher.clone(),
    );

    let mut wafer = Wafer::builder()
        .disable_inventory()
        .disable_lockfile()
        .build()
        .expect("Wafer::build");
    wafer
        .register_block(&name, Arc::new(block))
        .expect("register lazy calculator stub");

    // start() seals then eagerly dispatches lifecycle(Init) + Start to every
    // registered block — the same eager pass the app runs at SW boot.
    let wafer = wafer.start().await.expect("start runtime");

    assert_eq!(
        fetcher.fetches.load(Ordering::SeqCst),
        0,
        "runtime boot must not fetch skill wasm — lifecycle on an unloaded \
         stub has to defer, not force-load"
    );

    // First real use: NOW the wasm loads (exactly once) and the skill works.
    let body = serde_json::to_vec(&json!({ "expr": "2+2" })).expect("serialize body");
    let output = wafer
        .run_block(&name, Message::new("invoke"), InputStream::from_bytes(body))
        .await;
    let resp = output
        .collect_buffered()
        .await
        .expect("calculator returned a non-success terminal");
    let parsed: serde_json::Value =
        serde_json::from_slice(&resp.body).expect("calculator body is JSON");
    assert_eq!(
        fetcher.fetches.load(Ordering::SeqCst),
        1,
        "first handle() must fetch the block wasm exactly once"
    );
    assert!(
        parsed.to_string().contains('4'),
        "calculator should evaluate 2+2, got: {parsed}"
    );

    // Second use hits the cache — still exactly one fetch.
    let body = serde_json::to_vec(&json!({ "expr": "3*3" })).expect("serialize body");
    let output = wafer
        .run_block(&name, Message::new("invoke"), InputStream::from_bytes(body))
        .await;
    output
        .collect_buffered()
        .await
        .expect("calculator second call succeeds");
    assert_eq!(
        fetcher.fetches.load(Ordering::SeqCst),
        1,
        "subsequent handle() calls must reuse the cached instance"
    );
}

#[tokio::test]
async fn concurrent_first_use_single_flights_the_fetch() {
    // Two invocations racing on the same still-unloaded skill must not each
    // download + compile the wasm — and, worse, without single-flight the
    // loser of the pending-Init take() caches an instance that never received
    // its deferred Init. The fetcher yields, so the two calls genuinely
    // interleave at the fetch await.
    let manifest = include_str!("../blocks/calculator/manifest.json");
    let info: BlockInfo = serde_json::from_str(manifest).expect("calculator manifest");
    let name = info.name.clone();

    let fetcher = Arc::new(CountingFetcher {
        fetches: AtomicUsize::new(0),
    });
    let block = LazySkillBlock::new(
        info,
        "/blocks/calculator.wasm".to_string(),
        ResourceLimits::default(),
        Arc::new(NoopAssetLoader),
        fetcher.clone(),
    );

    let mut wafer = Wafer::builder()
        .disable_inventory()
        .disable_lockfile()
        .build()
        .expect("Wafer::build");
    wafer
        .register_block(&name, Arc::new(block))
        .expect("register lazy calculator stub");
    let wafer = wafer.start().await.expect("start runtime");

    let body1 = serde_json::to_vec(&json!({ "expr": "2+2" })).expect("serialize body");
    let body2 = serde_json::to_vec(&json!({ "expr": "5*5" })).expect("serialize body");
    let (out1, out2) = tokio::join!(
        wafer.run_block(
            &name,
            Message::new("invoke"),
            InputStream::from_bytes(body1)
        ),
        wafer.run_block(
            &name,
            Message::new("invoke"),
            InputStream::from_bytes(body2)
        ),
    );
    out1.collect_buffered()
        .await
        .expect("first concurrent call succeeds");
    out2.collect_buffered()
        .await
        .expect("second concurrent call succeeds");
    assert_eq!(
        fetcher.fetches.load(Ordering::SeqCst),
        1,
        "concurrent first use must single-flight the wasm fetch (no double \
         download, no cached instance that skipped its deferred Init)"
    );
}
