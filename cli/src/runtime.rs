//! Boot a minimal native Wafer that hosts the embedded skill blocks.
use std::sync::Arc;

use anyhow::{Context as _, Result};
use gizza_ai_block_utils::GIZZA_MAX_WASM_MEMORY_PAGES;
use wafer_block::{
    core_types::Message,
    meta::{META_REQ_ACTION, META_REQ_RESOURCE},
    streams::{input::InputStream, output::TerminalNotResponse},
};
use wafer_block::{Block, ConfigVar};
use wafer_core::interfaces::network::service::NetworkLimits;
use wafer_run::{
    resolve_declared, ConfigError, ConfigSource, EnvBlockConfig, FuelLimit, Wafer, WasmiBlock,
};

use crate::SKILL_WASMS;

/// Block config read from the process environment.
///
/// A block's declared config keys (e.g. `wafer-run/network`'s
/// `WAFER_RUN__NETWORK__MAX_RESPONSE_BYTES`) are what a user of the CLI sets
/// in their shell. The runtime resolves them through this source at each
/// block's lazy `lifecycle(Init)`; an invalid value fails that block's Init,
/// naming the key. A value that is not valid UTF-8 is passed on lossily, so
/// the block rejects it as invalid instead of it reading as unset.
struct EnvConfigSource;

#[wafer_block::wafer_async_trait]
impl ConfigSource for EnvConfigSource {
    async fn load_for_block(
        &self,
        block: &str,
        declared_keys: &[ConfigVar],
    ) -> Result<EnvBlockConfig, ConfigError> {
        resolve_declared(block, declared_keys, |key| {
            std::env::var_os(key).map(|v| v.to_string_lossy().into_owned())
        })
    }
}

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

/// A booted runtime with the embedded skill blocks registered.
pub struct ToolRuntime {
    wafer: Wafer,
    names: Vec<String>,
    metas: Vec<ToolMeta>,
    /// Skills listed but not registered, each with the block it `requires`
    /// that the CLI does not host (e.g. `gizza-ai/imagine` → `wafer-run/image`).
    unhosted: Vec<(String, String)>,
}

/// The host service blocks [`boot`] registers. A skill whose `requires` names
/// any other block is listed but not registered: `seal()` refuses to boot a
/// block whose required dependency is missing.
const HOSTED_SERVICES: &[&str] = &["gizza-ai/ffmpeg-runtime", "wafer-run/network"];

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
        // Browser-only capabilities the CLI cannot provide: a skill that
        // requires a service block the CLI does not host, and background
        // removal, whose model runs only on its browser tool page.
        let unsupported = if let Some((_, service)) =
            self.unhosted.iter().find(|(skill, _)| skill == name)
        {
            Some(format!(
                "{name} needs the `{service}` service, which only the browser app provides"
            ))
        } else if name == "gizza-ai/image-background-remove-ai" {
            Some(
                "AI image background removal currently runs on its standalone browser tool page"
                    .to_string(),
            )
        } else {
            None
        };
        if let Some(message) = unsupported {
            let body = serde_json::json!({
                "error": "unsupported_in_cli",
                "message": message
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

/// Boot a native `Wafer` hosting every embedded skill WASM plus the host
/// service blocks the skills call (`gizza-ai/ffmpeg-runtime`, backed by the
/// system `ffmpeg`, and `wafer-run/network`). Both services are lazy: nothing
/// is spawned or connected until a skill calls them.
pub async fn boot() -> Result<ToolRuntime> {
    let mut wafer = Wafer::builder()
        .disable_inventory()
        .disable_lockfile()
        .config_source(Arc::new(EnvConfigSource))
        // Trusted single-user CLI: skill calls run unmetered with a raised
        // memory cap, so heavy tools run to completion instead of trapping
        // with `all fuel consumed` (fuel) or `unreachable`/OOM (memory).
        .fuel_per_call(FuelLimit::Unmetered)
        .max_wasm_memory_pages(GIZZA_MAX_WASM_MEMORY_PAGES)
        .build()
        .context("build wafer")?;

    // --- Host service blocks (the names in HOSTED_SERVICES) ---

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
    // The service starts under the default limits; the block's Init replaces
    // them with the `WAFER_RUN__NETWORK__*` limits it declares, resolved from
    // the environment by `EnvConfigSource`.
    let network_service =
        wafer_block_network::service::HttpNetworkService::new(NetworkLimits::default());
    wafer
        .register_block(
            "wafer-run/network",
            Arc::new(wafer_core::service_blocks::network::NetworkBlock::new(
                Arc::new(network_service),
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
    wafer
        .add_wrap_grants(vec![wafer_block::types::ResourceGrant::read_write(
            "*", "*",
        )
        .typed(wafer_block::types::ResourceType::Network)])
        .map_err(|e| anyhow::anyhow!("add network grant: {e}"))?;

    // --- Skill WASMs ---
    let mut names = Vec::new();
    let mut metas = Vec::new();
    let mut unhosted = Vec::new();
    for bytes in SKILL_WASMS {
        // The embedded skills are built from this repository, so the CLI
        // approves exactly the capabilities each one declares (e.g. network +
        // `callable_blocks = ["wafer-run/network"]`): the declaration is the
        // bound the runtime enforces. Fuel and memory limits come from the
        // builder settings above.
        let block = WasmiBlock::load_approving_declaration(bytes, wafer.resource_limits())
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
        if let Some(missing) = info
            .requires
            .iter()
            .find(|required| !HOSTED_SERVICES.contains(&required.as_str()))
        {
            unhosted.push((name, missing.clone()));
            continue;
        }
        wafer
            .register_block(&name, Arc::new(block))
            .map_err(|e| anyhow::anyhow!("register {name}: {e}"))?;
        names.push(name);
    }

    wafer.seal().await.context("seal wafer")?;
    names.sort();
    metas.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(ToolRuntime {
        wafer,
        names,
        metas,
        unhosted,
    })
}
