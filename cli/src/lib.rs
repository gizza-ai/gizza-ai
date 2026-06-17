//! gizza-cli — run gizza skill tools headlessly.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

pub mod args;
pub mod ffmpeg_native;
pub mod render;
pub mod runtime;
pub mod skill_md;

use anyhow::Result;

/// Boot the full runtime, run a tool by short or full name with JSON args,
/// return the response body bytes.
pub async fn run_tool(name: &str, args: serde_json::Value) -> Result<Vec<u8>> {
    let rt = runtime::boot_full().await?;
    let full = if name.starts_with("gizza-ai/") {
        name.to_string()
    } else {
        format!("gizza-ai/{name}")
    };
    rt.run_tool(&full, args).await
}

/// List all available tools (name, description, parameters).
pub async fn list_tools() -> Result<Vec<runtime::ToolMeta>> {
    Ok(runtime::boot_minimal().await?.tools().to_vec())
}

/// Describe one tool by short or full name.
pub async fn describe_tool(name: &str) -> Result<Option<runtime::ToolMeta>> {
    Ok(runtime::boot_minimal().await?.tool(name).cloned())
}

mod skill_wasms {
    include!(concat!(env!("OUT_DIR"), "/skill_wasms.rs"));
}
pub(crate) use skill_wasms::SKILL_WASMS;
