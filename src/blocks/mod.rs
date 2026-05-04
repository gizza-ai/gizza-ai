//! Native gizza-ai blocks.

#[cfg(any(target_arch = "wasm32", test))]
pub mod agent;
pub mod ffmpeg;
#[cfg(target_arch = "wasm32")]
pub mod ui;
