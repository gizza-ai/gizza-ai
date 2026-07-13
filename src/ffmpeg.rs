//! gizza-ai ffmpeg invocation primitive.
//!
//! The shared types (`ExecArgs`, `ExecResult`, `FfmpegError`, `FfmpegService`
//! trait, and `FfmpegBlock`) now live in `gizza-ai-block-utils::ffmpeg` so
//! they can be used by both the browser app (wasm32) and the native CLI.
//!
//! This file keeps only the browser-side implementation: `BrowserFfmpegService`
//! and its `BridgeInput`/`BridgeResponse` helpers. `BrowserFfmpegService` uses
//! `#[wasm_bindgen(module = "/js/ffmpeg-bridge.js")]`, which posts the work to a
//! window client (ffmpeg can't run in the Service Worker) and is resolved
//! relative to THIS crate's root, so it must stay here.

// Re-export the shared types for convenience so callers within the app crate
// can still write `ffmpeg::FfmpegService` without changing every reference.
pub use gizza_ai_block_utils::ffmpeg::{
    ExecArgs, ExecResult, FfmpegBlock, FfmpegError, FfmpegService,
};

#[cfg(target_arch = "wasm32")]
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
#[cfg(target_arch = "wasm32")]
use serde::{Deserialize, Serialize};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(module = "/js/ffmpeg-bridge.js")]
extern "C" {
    #[wasm_bindgen(js_name = ffmpegExec)]
    pub async fn ffmpeg_exec(args_json: &str, inputs_json: &str, output_name: &str) -> JsValue;
}

/// Browser-side ffmpeg service. Delegates to a window client via the
/// wasm-bindgen bridge at `js/ffmpeg-bridge.js`, which postMessages a page
/// running `@ffmpeg/ffmpeg` (the Service Worker can't run ffmpeg itself).
/// wasm32-only — native tests substitute their own `FfmpegService` impl.
///
/// Note: no explicit `unsafe impl Send + Sync` is needed. `FfmpegService`'s
/// `MaybeSend + MaybeSync` bound is a blanket no-op on wasm32 (any `?Sized`
/// type satisfies it — see `wafer_block::compat`), and this unit struct
/// holds no fields. The FFmpeg instance lives in the JS module.
#[cfg(target_arch = "wasm32")]
pub struct BrowserFfmpegService;

#[cfg(target_arch = "wasm32")]
#[derive(Serialize)]
struct BridgeInput<'a> {
    name: &'a str,
    bytes_b64: String,
}

#[cfg(target_arch = "wasm32")]
#[derive(Deserialize)]
struct BridgeResponse {
    exit_code: i32,
    output_b64: String,
    log: String,
}

#[cfg(target_arch = "wasm32")]
#[async_trait::async_trait(?Send)]
impl FfmpegService for BrowserFfmpegService {
    async fn exec(&self, args: ExecArgs) -> Result<ExecResult, FfmpegError> {
        let args_json = serde_json::to_string(&args.args)
            .map_err(|e| FfmpegError::Serialize(format!("args: {e}")))?;

        let inputs_b64: Vec<BridgeInput> = args
            .inputs
            .iter()
            .map(|(name, bytes)| BridgeInput {
                name,
                bytes_b64: B64.encode(bytes),
            })
            .collect();
        let inputs_json = serde_json::to_string(&inputs_b64)
            .map_err(|e| FfmpegError::Serialize(format!("inputs: {e}")))?;

        let js_val = ffmpeg_exec(&args_json, &inputs_json, &args.output).await;

        let json_str = js_sys::JSON::stringify(&js_val)
            .map(|s| s.as_string().unwrap_or_default())
            .unwrap_or_default();

        let resp: BridgeResponse = serde_json::from_str(&json_str)
            .map_err(|e| FfmpegError::Bridge(format!("parse response: {e}")))?;

        let output = if resp.output_b64.is_empty() {
            Vec::new()
        } else {
            B64.decode(&resp.output_b64)
                .map_err(|e| FfmpegError::Bridge(format!("decode output_b64: {e}")))?
        };

        Ok(ExecResult {
            exit_code: resp.exit_code,
            output,
            log: resp.log,
        })
    }
}
