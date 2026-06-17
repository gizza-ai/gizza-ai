//! Boot a minimal native Wafer that hosts the embedded skill blocks.
use std::sync::Arc;

use anyhow::{Context as _, Result};
use wafer_block::{
    core_types::Message,
    meta::{META_REQ_ACTION, META_REQ_RESOURCE},
    streams::{input::InputStream, output::TerminalNotResponse},
};
use wafer_block::Block;
use wafer_run::{Wafer, WasmiBlock};

use crate::SKILL_WASMS;

/// A booted runtime with all embedded skill blocks registered.
pub struct ToolRuntime {
    wafer: Wafer,
    names: Vec<String>,
}

impl ToolRuntime {
    /// Returns the sorted list of registered block names.
    pub fn tool_names(&self) -> &[String] {
        &self.names
    }

    /// Dispatch a skill block by full name (e.g. `"gizza-ai/calculator"`).
    ///
    /// Returns the raw response body bytes, or an error if dispatch failed.
    pub async fn run_tool(&self, name: &str, args: serde_json::Value) -> Result<Vec<u8>> {
        let short = name.strip_prefix("gizza-ai/").unwrap_or(name);
        let body = serde_json::to_vec(&args).context("serialize args")?;
        let mut msg = Message::new("http");
        msg.set_meta(META_REQ_ACTION, "create");
        msg.set_meta(META_REQ_RESOURCE, format!("/b/{short}"));
        let out = self.wafer.run_block(name, msg, InputStream::from_bytes(body)).await;
        match out.collect_buffered().await {
            Ok(resp) => Ok(resp.body),
            Err(TerminalNotResponse::Halt(buf)) => Ok(buf.body),
            Err(e) => Err(anyhow::anyhow!("tool {name} produced no response: {e}")),
        }
    }
}

/// Boot a minimal native `Wafer` with all embedded skill WASMs registered.
pub async fn boot_minimal() -> Result<ToolRuntime> {
    let mut wafer = Wafer::builder()
        .disable_inventory()
        .disable_lockfile()
        .build()
        .context("build wafer")?;
    let mut names = Vec::new();
    for bytes in SKILL_WASMS {
        let block = WasmiBlock::load_from_bytes(bytes).context("load skill wasm")?;
        let name = block.info().name.clone();
        wafer
            .register_block(&name, Arc::new(block))
            .map_err(|e| anyhow::anyhow!("register {name}: {e}"))?;
        names.push(name);
    }
    wafer.seal().await.context("seal wafer")?;
    names.sort();
    Ok(ToolRuntime { wafer, names })
}
