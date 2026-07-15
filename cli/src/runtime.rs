//! Boot a minimal native Wafer that hosts the embedded skill blocks.
use std::sync::Arc;

use anyhow::{Context as _, Result};
use gizza_ai_block_utils::GIZZA_MAX_WASM_MEMORY_PAGES;
use wafer_block::{
    core_types::Message,
    meta::{META_REQ_ACTION, META_REQ_RESOURCE},
    streams::{input::InputStream, output::TerminalNotResponse},
};
use wafer_block::Block;
use wafer_run::{FuelLimit, Wafer, WasmiBlock};

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
        // the one capability the CLI cannot provide
        if name == "gizza-ai/imagine" {
            let body = serde_json::json!({
                "error": "unsupported_in_cli",
                "message": "text-to-image needs a browser GPU; use gizza.ai"
            });
            return serde_json::to_vec(&body).context("serialize unsupported_in_cli body");
        }

        let short = name.strip_prefix("gizza-ai/").unwrap_or(name);
        let body = serde_json::to_vec(&args).context("serialize args")?;
        let mut msg = Message::new("http");
        msg.set_meta(META_REQ_ACTION, "create");
        msg.set_meta(META_REQ_RESOURCE, format!("/b/{short}"));
        let out = self.wafer.run_block(name, msg, InputStream::from_bytes(body)).await;
        match out.collect_buffered().await {
            Ok(resp) => Ok(resp.body),
            Err(TerminalNotResponse::Halt(buf)) => Ok(buf.body),
            // Blocks surface runtime errors (network failure, service unavailable, etc.)
            // as Error terminals. Convert these to structured JSON error bodies so callers
            // always receive a parseable payload — matching how the HTTP codec handles them.
            Err(TerminalNotResponse::Error(e)) => {
                let body = serde_json::json!({
                    "error": e.code.to_string().to_lowercase().replace(' ', "_"),
                    "message": e.message,
                });
                serde_json::to_vec(&body).context("serialize error body")
            }
            Err(e) => Err(anyhow::anyhow!("tool {name} produced no response: {e}")),
        }
    }
}

/// Register all embedded skill WASMs into a pre-built `Wafer`, collecting
/// block names and tool metadata. Called by both `boot_minimal` and `boot_full`
/// so the loop lives in exactly one place.
fn register_skills(
    wafer: &mut Wafer,
    names: &mut Vec<String>,
    metas: &mut Vec<ToolMeta>,
) -> Result<()> {
    for bytes in SKILL_WASMS {
        // gizza is a single-user, trusted CLI: opt skill calls out of the
        // default 100M fuel cap AND raise the 256-page / 16 MiB memory cap so
        // heavy tools run to completion instead of trapping with `all fuel
        // consumed` (fuel) or `unreachable`/OOM (memory). Both bounds are set
        // on the builder (`fuel_per_call` + `max_wasm_memory_pages`) and read
        // back here so every load site expresses the same policy.
        let block = WasmiBlock::load_from_bytes_with_limits(bytes, wafer.resource_limits())
            .context("load skill wasm")?;
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
    Ok(())
}

/// Boot a minimal native `Wafer` with all embedded skill WASMs registered.
///
/// No host service blocks are registered — suitable for pure-compute tools
/// (calculator, clock) and for fast unit tests that don't need network or
/// ffmpeg.
pub async fn boot_minimal() -> Result<ToolRuntime> {
    let mut wafer = Wafer::builder()
        .disable_inventory()
        .disable_lockfile()
        // Trusted single-user CLI: skill calls run unmetered with a raised
        // memory cap (see register_skills).
        .fuel_per_call(FuelLimit::Unmetered)
        .max_wasm_memory_pages(GIZZA_MAX_WASM_MEMORY_PAGES)
        .build()
        .context("build wafer")?;
    let mut names = Vec::new();
    let mut metas = Vec::new();
    register_skills(&mut wafer, &mut names, &mut metas)?;
    wafer.seal().await.context("seal wafer")?;
    names.sort();
    metas.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(ToolRuntime { wafer, names, metas })
}

/// Boot a full native `Wafer` with skill WASMs plus host service blocks
/// (ffmpeg-runtime, wafer-run/network). Use this for the CLI binary so that
/// image/video/web-fetch tools work. Pure tools still function under boot_full
/// — the extra services are harmless when not called.
pub async fn boot_full() -> Result<ToolRuntime> {
    let mut wafer = Wafer::builder()
        .disable_inventory()
        .disable_lockfile()
        // Trusted single-user CLI: skill calls run unmetered with a raised
        // memory cap (see register_skills).
        .fuel_per_call(FuelLimit::Unmetered)
        .max_wasm_memory_pages(GIZZA_MAX_WASM_MEMORY_PAGES)
        .build()
        .context("build wafer")?;

    // --- Host service blocks ---

    // ffmpeg-runtime: delegates to the system `ffmpeg` binary on PATH.
    wafer
        .register_block(
            "gizza-ai/ffmpeg-runtime",
            Arc::new(gizza_ai_block_utils::ffmpeg::FfmpegBlock::new(
                crate::ffmpeg_native::NativeFfmpegService::arc(),
            )),
        )
        .map_err(|e| anyhow::anyhow!("register ffmpeg-runtime: {e}"))?;

    // wafer-run/network: HTTP client backed by reqwest with SSRF protection.
    // Block name comes from service_blocks/network.rs ("wafer-run/network").
    // Constructor: NetworkBlock::new(Arc<dyn NetworkService>).
    // Source: wafer-run/crates/wafer-core/src/service_blocks/network.rs:10.
    wafer
        .register_block(
            "wafer-run/network",
            Arc::new(wafer_core::service_blocks::network::NetworkBlock::new(
                Arc::new(wafer_block_network::service::HttpNetworkService::new()),
            )),
        )
        .map_err(|e| anyhow::anyhow!("register network: {e}"))?;

    // WRAP host grant for network egress. WRAP default-denies typed service
    // resources unless the HOST grants them — block-declared capabilities are
    // a separate, second gate. Without this grant every network-using tool
    // dies with `WRAP: access denied (type: Network)` before its capability
    // is even consulted. The CLI is a local single-user tool: invoking
    // `gizza tool web-fetch url=…` IS the user's authorization for that
    // egress, so grant network to all blocks; per-block capability
    // declarations still decide which blocks may use it.
    wafer.add_wrap_grants(vec![wafer_block::types::ResourceGrant::read_write(
        "*", "*",
    )
    .typed(wafer_block::types::ResourceType::Network)]);

    // --- Skill WASMs (same loop as boot_minimal) ---
    let mut names = Vec::new();
    let mut metas = Vec::new();
    register_skills(&mut wafer, &mut names, &mut metas)?;

    wafer.seal().await.context("seal wafer")?;
    names.sort();
    metas.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(ToolRuntime { wafer, names, metas })
}
