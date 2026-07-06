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

use std::cell::RefCell;
use std::sync::Arc;

use async_trait::async_trait;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

use wafer_block::{
    block::Block,
    context::Context,
    core_types::{ErrorCode, LifecycleEvent, Message, WaferError},
    streams::{input::InputStream, output::OutputStream},
    types::BlockInfo,
};
use wafer_run::asset_loader::LoadAssetCallback;
use wafer_run::wasm::WasmiBlock;
use wafer_run::ResourceLimits;

/// A registered skill whose wasm is fetched + instantiated on first use.
///
/// `info()` is served synchronously from the embedded manifest (so the agent
/// can enumerate the tool and the runtime can validate the registry at seal),
/// while the wasm is loaded lazily inside `handle`/`lifecycle`.
pub struct LazySkillBlock {
    info: BlockInfo,
    /// Same-origin URL of the block wasm, e.g. `/blocks/calculator.wasm`.
    url: String,
    limits: ResourceLimits,
    asset_loader: Arc<dyn LoadAssetCallback>,
    inner: RefCell<Option<Arc<WasmiBlock>>>,
}

// SAFETY: mirrors solobase-browser's `BrowserNetworkService`. The gizza app is
// compiled only for wasm32-unknown-unknown, which has no threads, so the
// `Send`/`Sync` bounds required by `Arc<dyn Block>` are satisfied trivially —
// there is no cross-thread aliasing of the `RefCell`.
unsafe impl Send for LazySkillBlock {}
unsafe impl Sync for LazySkillBlock {}

impl LazySkillBlock {
    pub fn new(
        info: BlockInfo,
        url: String,
        limits: ResourceLimits,
        asset_loader: Arc<dyn LoadAssetCallback>,
    ) -> Self {
        Self {
            info,
            url,
            limits,
            asset_loader,
            inner: RefCell::new(None),
        }
    }

    /// Return the instantiated `WasmiBlock`, fetching + compiling its wasm on
    /// the first call and caching it thereafter.
    async fn ensure_loaded(&self) -> Result<Arc<WasmiBlock>, String> {
        // Fast path — already loaded. Scope the borrow so it is dropped before
        // the `.await` below (a RefCell borrow must not be held across await).
        {
            if let Some(block) = self.inner.borrow().as_ref() {
                return Ok(block.clone());
            }
        }

        let bytes = fetch_bytes(&self.url).await?;
        let block = WasmiBlock::load_from_bytes_with_limits(&bytes, self.limits)
            .map_err(|e| format!("instantiate {}: {e}", self.url))?;
        // Registration normally forwards the runtime asset loader to WasmiBlock
        // via a downcast; this block is registered as a stub, so forward it
        // here at load time instead.
        block.set_asset_loader(self.asset_loader.clone());
        let arc = Arc::new(block);
        *self.inner.borrow_mut() = Some(arc.clone());
        web_sys::console::log_1(
            &format!(
                "gizza-ai: skill '{}' loaded ({} bytes)",
                self.info.name,
                bytes.len()
            )
            .into(),
        );
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

#[async_trait(?Send)]
impl Block for LazySkillBlock {
    fn info(&self) -> BlockInfo {
        self.info.clone()
    }

    async fn handle(&self, ctx: &dyn Context, msg: Message, input: InputStream) -> OutputStream {
        match self.ensure_loaded().await {
            Ok(block) => block.handle(ctx, msg, input).await,
            Err(e) => OutputStream::error(self.load_error(e)),
        }
    }

    async fn lifecycle(&self, ctx: &dyn Context, event: LifecycleEvent) -> Result<(), WaferError> {
        let block = self.ensure_loaded().await.map_err(|e| self.load_error(e))?;
        block.lifecycle(ctx, event).await
    }
}

/// Fetch a same-origin asset's bytes from within the Service-Worker global
/// scope. Block wasm lives at `/blocks/<slug>.wasm`; a SW's own `fetch` goes to
/// the network/cache (it does not re-enter the SW's fetch handler), so this
/// needs no postMessage bridge.
async fn fetch_bytes(url: &str) -> Result<Vec<u8>, String> {
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
