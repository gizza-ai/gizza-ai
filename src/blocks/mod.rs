//! Native gizza-ai blocks.

#[cfg(target_arch = "wasm32")]
pub mod agent;
pub mod ffmpeg;
#[cfg(target_arch = "wasm32")]
pub mod ui;
