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

/// Tool metadata extracted from a block's `info().tool` at boot time.
#[derive(Clone, Debug)]
pub struct ToolMeta {
    /// Full block name: `"gizza-ai/calculator"`.
    pub name: String,
    /// Short name without org prefix: `"calculator"`.
    pub short: String,
    /// Natural-language description for the tool.
    pub description: String,
    /// JSON Schema describing the tool's input arguments.
    pub parameters: serde_json::Value,
}

/// A booted runtime with all embedded skill blocks registered.
pub struct ToolRuntime {
    wafer: Wafer,
    names: Vec<String>,
    metas: Vec<ToolMeta>,
}

impl ToolRuntime {
    /// Returns the sorted list of registered block names.
    pub fn tool_names(&self) -> &[String] {
        &self.names
    }

    /// Returns metadata for all blocks that declared a `SkillTool`, sorted by name.
    pub fn tools(&self) -> &[ToolMeta] {
        &self.metas
    }

    /// Look up a tool by short name (e.g. `"calculator"`) or full name
    /// (e.g. `"gizza-ai/calculator"`). Returns `None` if not found.
    pub fn tool(&self, short_or_full: &str) -> Option<&ToolMeta> {
        let full = if short_or_full.starts_with("gizza-ai/") {
            short_or_full.to_string()
        } else {
            format!("gizza-ai/{short_or_full}")
        };
        self.metas.iter().find(|m| m.name == full)
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
    let mut metas = Vec::new();
    for bytes in SKILL_WASMS {
        let block = WasmiBlock::load_from_bytes(bytes).context("load skill wasm")?;
        let info = block.info();
        let name = info.name.clone();
        // Capture SkillTool metadata if the block exposes one.
        if let Some(tool) = &info.tool {
            let short = name
                .strip_prefix("gizza-ai/")
                .unwrap_or(&name)
                .to_string();
            metas.push(ToolMeta {
                name: name.clone(),
                short,
                description: tool.description.clone(),
                parameters: tool.parameters.clone(),
            });
        }
        wafer
            .register_block(&name, Arc::new(block))
            .map_err(|e| anyhow::anyhow!("register {name}: {e}"))?;
        names.push(name);
    }
    wafer.seal().await.context("seal wafer")?;
    names.sort();
    metas.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(ToolRuntime { wafer, names, metas })
}
