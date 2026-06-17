//! Native gizza-ai blocks.

#[cfg(any(target_arch = "wasm32", test))]
pub mod agent;
#[cfg(any(target_arch = "wasm32", test))]
pub mod ui;

/// Default WebLLM model id used by the agent dispatcher and the UI template
/// fallback. Picked for small size + tool-call support. Plan C makes this
/// user-pickable.
pub(crate) const DEFAULT_MODEL_ID: &str = "Qwen2.5-1.5B-Instruct-q4f32_1-MLC";
