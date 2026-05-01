//! gizza-ai ffmpeg invocation primitive.
//!
//! Bridges `gizza-ai/ffmpeg-runtime` block requests into JS-side
//! `@ffmpeg/ffmpeg`. Mirrors the `BrowserNetworkService` pattern from
//! `solobase-browser/src/network.rs`.

#[cfg(target_arch = "wasm32")]
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use serde::{Deserialize, Serialize};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(module = "/js/ffmpeg.js")]
extern "C" {
    #[wasm_bindgen(js_name = ffmpegExec)]
    pub async fn ffmpeg_exec(
        args_json: &str,
        inputs_json: &str,
        output_name: &str,
    ) -> JsValue;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecArgs {
    pub args: Vec<String>,
    /// `(filename, bytes)` pairs — written to ffmpeg's virtual FS before exec.
    pub inputs: Vec<(String, Vec<u8>)>,
    /// Filename to read back after exec.
    pub output: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecResult {
    pub exit_code: i32,
    pub output: Vec<u8>,
    pub log: String,
}

#[derive(Debug, thiserror::Error)]
pub enum FfmpegError {
    #[error("bridge serialization: {0}")]
    Serialize(String),
    #[error("bridge call returned malformed response: {0}")]
    Bridge(String),
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
pub trait FfmpegService: wafer_block::MaybeSend + wafer_block::MaybeSync + 'static {
    async fn exec(&self, args: ExecArgs) -> Result<ExecResult, FfmpegError>;
}

/// Browser-side ffmpeg service. Uses `@ffmpeg/ffmpeg` from jsdelivr via
/// the wasm-bindgen module at `js/ffmpeg.js`. wasm32-only — native tests
/// substitute their own `FfmpegService` impl.
#[cfg(target_arch = "wasm32")]
pub struct BrowserFfmpegService;

// wasm32 is single-threaded; the service holds no JS state of its own
// (the FFmpeg instance lives in the JS module). Mirrors BrowserNetworkService.
#[cfg(target_arch = "wasm32")]
unsafe impl Send for BrowserFfmpegService {}
#[cfg(target_arch = "wasm32")]
unsafe impl Sync for BrowserFfmpegService {}

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
