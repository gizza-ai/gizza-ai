//! Lazy on-demand skill-block loading.
//!
//! Each skill block's wasm used to be `include_bytes!`d into the app cdylib
//! (`gizza_ai_bg.wasm`), which grew ~0.5 MiB per tool and pushed the single
//! file past Cloudflare Pages' 25 MiB/file limit once ~47 blocks accumulated
//! (26 MiB of embedded wasm). Instead we register a lightweight stub per skill
//! whose `info()` comes from the embedded manifest.json; the ~0.6 MiB block
//! wasm is fetched from `/blocks/<slug>.wasm` and instantiated only on the
//! first call to that skill, then cached. The app wasm now stays flat
//! regardless of tool count.
//!
//! The module compiles on native targets too (fetching is injected via
//! [`SkillWasmFetcher`]) so the boot-must-stay-lazy invariant is covered by a
//! native integration test (`tests/lazy_skill_lifecycle.rs`) — the regression
//! it guards against was `lifecycle(Init)` force-loading every block, which
//! made `init_all_blocks()` at boot download all ~47 wasm files.

use std::sync::{Arc, Mutex};

use wafer_block::{
    block::Block,
    context::Context,
    core_types::{ErrorCode, LifecycleEvent, Message, WaferError},
    streams::{input::InputStream, output::OutputStream},
    types::BlockInfo,
    wafer_async_trait, MaybeSend, MaybeSync,
};
use wafer_run::asset_loader::LoadAssetCallback;
use wafer_run::wasm::WasmiBlock;
use wafer_run::ResourceLimits;

/// Source of a skill block's wasm bytes. In the browser this is a fetch of
/// `/blocks/<slug>.wasm` from the Service-Worker global scope
/// ([`SwSkillWasmFetcher`]); native tests inject a filesystem/counting fake.
#[wafer_async_trait]
pub trait SkillWasmFetcher: MaybeSend + MaybeSync {
    async fn fetch(&self, url: &str) -> Result<Vec<u8>, String>;
}

/// Where a lazy skill is in its load lifecycle. One enum instead of separate
/// `Option<Arc<WasmiBlock>>` + pending-event fields, so "loaded but still has
/// pending events" is unrepresentable.
enum LoadState {
    /// Wasm not fetched yet. Lifecycle events received in this state are
    /// stashed in order and replayed on the real block at first load, so the
    /// block sees the same sequence it would have seen loaded eagerly.
    Unloaded { pending: Vec<LifecycleEvent> },
    /// Fetched, instantiated, pending events replayed — serving traffic.
    Loaded(Arc<WasmiBlock>),
    /// A deferred lifecycle replay failed. Cached permanently — the runtime's
    /// init slot already recorded success for the stub, so retrying here would
    /// re-download the wasm on every call forever. Mirrors the eager-boot
    /// behaviour where a failed Init left the block permanently erroring.
    Failed(String),
}

/// A registered skill whose wasm is fetched + instantiated on first use.
///
/// `info()` is served synchronously from the embedded manifest (so the agent
/// can enumerate the tool and the runtime can validate the registry at seal),
/// while the wasm is loaded lazily on the first `handle()`.
pub struct LazySkillBlock {
    info: BlockInfo,
    /// Same-origin URL of the block wasm, e.g. `/blocks/calculator.wasm`.
    url: String,
    limits: ResourceLimits,
    asset_loader: Arc<dyn LoadAssetCallback>,
    fetcher: Arc<dyn SkillWasmFetcher>,
    state: Mutex<LoadState>,
    /// Single-flight gate for the load path: concurrent first-use callers
    /// queue here so exactly one fetches + compiles + replays the pending
    /// lifecycle events; the rest then read the cached result. Without it the
    /// losers would re-download the wasm AND cache an instance that never
    /// received its deferred Init. Async (`futures::lock`) because it is held
    /// across the fetch await.
    load_gate: futures::lock::Mutex<()>,
}

// SAFETY: mirrors impresspress-browser's `BrowserNetworkService`. On
// wasm32-unknown-unknown there are no threads, so the `Send`/`Sync` bounds
// required by `Arc<dyn Block>` are satisfied trivially — there is no
// cross-thread aliasing. On native targets these impls are NOT emitted: all
// fields are genuinely Send + Sync there (`MaybeSend`/`MaybeSync` resolve to
// the real bounds), so the auto impls apply.
#[cfg(target_arch = "wasm32")]
unsafe impl Send for LazySkillBlock {}
#[cfg(target_arch = "wasm32")]
unsafe impl Sync for LazySkillBlock {}

impl LazySkillBlock {
    pub fn new(
        info: BlockInfo,
        url: String,
        limits: ResourceLimits,
        asset_loader: Arc<dyn LoadAssetCallback>,
        fetcher: Arc<dyn SkillWasmFetcher>,
    ) -> Self {
        Self {
            info,
            url,
            limits,
            asset_loader,
            fetcher,
            state: Mutex::new(LoadState::Unloaded {
                pending: Vec::new(),
            }),
            load_gate: futures::lock::Mutex::new(()),
        }
    }

    fn state(&self) -> std::sync::MutexGuard<'_, LoadState> {
        self.state.lock().expect("LazySkillBlock state poisoned")
    }

    /// `Some(result)` once the load has settled (loaded or permanently
    /// failed), `None` while still unloaded.
    fn settled(&self) -> Option<Result<Arc<WasmiBlock>, String>> {
        match &*self.state() {
            LoadState::Loaded(block) => Some(Ok(block.clone())),
            LoadState::Failed(e) => Some(Err(e.clone())),
            LoadState::Unloaded { .. } => None,
        }
    }

    /// Return the instantiated `WasmiBlock`, fetching + compiling its wasm on
    /// the first call and caching it thereafter. Lifecycle events stashed
    /// while unloaded are replayed in order before the instance is published.
    /// A fetch/instantiate failure leaves the state Unloaded (retried on the
    /// next call — e.g. a transient network error); a replay failure is cached
    /// as permanent (see [`LoadState::Failed`]).
    async fn ensure_loaded(&self, ctx: &dyn Context) -> Result<Arc<WasmiBlock>, String> {
        if let Some(settled) = self.settled() {
            return settled;
        }
        // Single-flight: first caller loads, the rest await and reuse.
        let _flight = self.load_gate.lock().await;
        if let Some(settled) = self.settled() {
            return settled;
        }

        let bytes = self.fetcher.fetch(&self.url).await?;
        let block = WasmiBlock::load_from_bytes_with_limits(&bytes, self.limits)
            .map_err(|e| format!("instantiate {}: {e}", self.url))?;
        // Registration normally forwards the runtime asset loader to WasmiBlock
        // via a downcast; this block is registered as a stub, so forward it
        // here at load time instead.
        block.set_asset_loader(self.asset_loader.clone());
        let arc = Arc::new(block);

        let pending = match &mut *self.state() {
            LoadState::Unloaded { pending } => std::mem::take(pending),
            // Unreachable while holding the gate; harmless if it ever isn't.
            _ => Vec::new(),
        };
        for event in pending {
            if let Err(e) = arc.lifecycle(ctx, event).await {
                let msg = format!("deferred lifecycle replay: {}", e.message);
                *self.state() = LoadState::Failed(msg.clone());
                return Err(msg);
            }
        }

        *self.state() = LoadState::Loaded(arc.clone());
        log_loaded(&self.info.name, bytes.len());
        Ok(arc)
    }

    fn load_error(&self, e: String) -> WaferError {
        WaferError {
            code: ErrorCode::Internal,
            message: format!("lazy-load skill '{}': {e}", self.info.name),
            meta: vec![],
        }
    }
}

#[wafer_async_trait]
impl Block for LazySkillBlock {
    fn info(&self) -> BlockInfo {
        self.info.clone()
    }

    async fn handle(&self, ctx: &dyn Context, msg: Message, input: InputStream) -> OutputStream {
        match self.ensure_loaded(ctx).await {
            Ok(block) => block.handle(ctx, msg, input).await,
            Err(e) => OutputStream::error(self.load_error(e)),
        }
    }

    /// Lifecycle events must NOT force-load the wasm: the runtime's boot pass
    /// (`init_all_blocks()` / `start()`) dispatches `Init` (and `Start`) to
    /// every registered block, and loading here meant every boot downloaded
    /// all ~47 skill wasm files — defeating the entire lazy-load design.
    /// While unloaded, every event is stashed in order and replayed on first
    /// use (note the replay runs under the first request's ctx, not the
    /// runtime's dedicated init ctx — acceptable for gizza's pure-compute
    /// skills); once loaded, events forward to the real block as usual.
    async fn lifecycle(&self, ctx: &dyn Context, event: LifecycleEvent) -> Result<(), WaferError> {
        let block = {
            match &mut *self.state() {
                LoadState::Loaded(block) => block.clone(),
                // Permanently failed: the block will never serve; swallow.
                LoadState::Failed(_) => return Ok(()),
                LoadState::Unloaded { pending } => {
                    pending.push(event);
                    return Ok(());
                }
            }
        };
        block.lifecycle(ctx, event).await
    }
}

#[cfg(target_arch = "wasm32")]
fn log_loaded(name: &str, len: usize) {
    web_sys::console::log_1(&format!("gizza-ai: skill '{name}' loaded ({len} bytes)").into());
}

#[cfg(not(target_arch = "wasm32"))]
fn log_loaded(_name: &str, _len: usize) {}

/// Fetch a same-origin asset's bytes from within the Service-Worker global
/// scope. Block wasm lives at `/blocks/<slug>.wasm`; a SW's own `fetch` goes to
/// the network/cache (it does not re-enter the SW's fetch handler), so this
/// needs no postMessage bridge.
#[cfg(target_arch = "wasm32")]
pub struct SwSkillWasmFetcher;

#[cfg(target_arch = "wasm32")]
#[wafer_async_trait]
impl SkillWasmFetcher for SwSkillWasmFetcher {
    async fn fetch(&self, url: &str) -> Result<Vec<u8>, String> {
        use wasm_bindgen::JsCast;
        use wasm_bindgen_futures::JsFuture;

        let global: web_sys::WorkerGlobalScope = js_sys::global().unchecked_into();
        let resp_val = JsFuture::from(global.fetch_with_str(url))
            .await
            .map_err(|e| format!("fetch {url}: {e:?}"))?;
        let resp: web_sys::Response = resp_val
            .dyn_into()
            .map_err(|_| format!("fetch {url}: response was not a Response"))?;
        if !resp.ok() {
            return Err(format!("fetch {url}: HTTP {}", resp.status()));
        }
        let ab_promise = resp
            .array_buffer()
            .map_err(|e| format!("array_buffer {url}: {e:?}"))?;
        let ab_val = JsFuture::from(ab_promise)
            .await
            .map_err(|e| format!("array_buffer {url}: {e:?}"))?;
        let ab: js_sys::ArrayBuffer = ab_val.unchecked_into();
        Ok(js_sys::Uint8Array::new(&ab).to_vec())
    }
}
