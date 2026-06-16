# gizza tool CLI — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship a native `gizza` binary whose `tool` subcommand runs any gizza skill block headlessly — `gizza tool calculator "2*2"` → `4` — by reusing the exact skill-block wasm + wasmi runtime, with host capabilities injected as native services and unsupported tools stubbed.

**Architecture:** A new `gizza-ai/cli/` crate (lib + `[[bin]] gizza`) boots a minimal raw `Wafer::builder()` (wasmi), loads the embedded skill-block wasm as `WasmiBlock`s (registered under `block.info().name`), and dispatches one tool call via `wafer.run_block(...).collect_buffered()`. Tool schemas come from `block.info().tool` (the `SkillTool` the chat uses) — no manifest dependency. Host capabilities are injected services: `NativeFfmpegService` (shell-out), `HttpNetworkService` (native network block), and a stub image service for `imagine`. The host-agnostic `FfmpegBlock`/`FfmpegService` are relocated out of the browser app crate into a native-compilable shared crate so app and CLI register the identical block.

**Tech Stack:** Rust (native, tokio), `wafer-run` (`features=["wasm"]` → wasmi + reqwest + tokio), `wafer-block`, `wafer-block-network`, `clap` (CLI parsing), `serde_json`. System `ffmpeg` for image/video tools.

**Spec:** `docs/superpowers/specs/2026-06-17-gizza-tool-cli-design.md`

---

## Prerequisites (run once before Task 1)

The CLI's `build.rs` embeds `blocks/<tool>/target/block.wasm`. Build them first:

```bash
cd /home/joris/Programs/suppers-ai/workspace/gizza-ai
solobase build            # builds every blocks/* skill wasm → blocks/<tool>/target/block.wasm
ls blocks/*/target/block.wasm   # expect 12 paths
```

If `solobase` is not installed: `cargo install --path ../solobase/crates/solobase --locked`.

---

## Reference: verified wafer-run APIs (paste-ready)

```rust
// Boot (wafer-run/src/runtime.rs:169, tests/integration_test.rs:232)
let mut wafer = wafer_run::Wafer::builder()
    .disable_inventory()
    .disable_lockfile()
    .build()?;                                  // Result<Wafer, RuntimeError>

// Load + register a wasm block (wasm/wasmi_loader/mod.rs:209, runtime.rs:681)
let block = wafer_run::wasm::WasmiBlock::load_from_bytes(bytes)?;  // Result<_, RuntimeError>
let info = block.info();                        // BlockInfo { name, .. , tool: Option<SkillTool> }
wafer.register_block(&info.name, std::sync::Arc::new(block))?;     // &str, Arc<dyn Block>

wafer.seal().await?;                            // Result<(), RuntimeError>

// Dispatch from host code (runtime.rs run_block; tests/integration_test.rs:395)
let mut msg = wafer_block::core_types::Message::new("http");
msg.set_meta(wafer_block::meta::META_REQ_ACTION, "create");
msg.set_meta(wafer_block::meta::META_REQ_RESOURCE, format!("/b/{name}"));
let input = wafer_block::InputStream::from_bytes(body_bytes);      // Vec<u8>
let out: wafer_block::OutputStream = wafer.run_block(&block_name, msg, input).await;
let resp: wafer_block::streams::output::BufferedResponse = out.collect_buffered().await?;
let response_bytes: Vec<u8> = resp.body;
```

`SkillTool` (`wafer_block::types::SkillTool`): `{ description: String, parameters: serde_json::Value }`.
`BlockInfo` carries `tool: Option<SkillTool>` (chat reads it via `lookup_skill_tool`, `src/blocks/agent/slash.rs:36`).

> Two specifics to confirm at first compile (use `cargo doc -p wafer-block --open` / the cited tests): the exact accessor for the tool on `BlockInfo` (field `.tool` vs method), and `InputStream::from_bytes` vs `InputStream::from(Vec<u8>)`. The cited test files show the real call shapes.

---

## File structure

```
gizza-ai/
  Cargo.toml                       # MODIFY: add "cli" + (Task 10) ffmpeg-runtime crate to [workspace].members if a workspace; else standalone
  src/lib.rs                       # MODIFY (Task 10): import FfmpegBlock/FfmpegService from relocated crate
  src/ffmpeg.rs                    # DELETE (Task 10): trait+DTOs move out; BrowserFfmpegService moves too
  src/blocks/ffmpeg.rs             # DELETE (Task 10): FfmpegBlock moves out
  src/blocks/mod.rs                # MODIFY (Task 10): drop `pub mod ffmpeg`
  block-utils/src/lib.rs           # MODIFY (Task 10): host the relocated FfmpegBlock/FfmpegService/ExecArgs/ExecResult + envelope parser
  cli/
    Cargo.toml                     # CREATE
    build.rs                       # CREATE: embed blocks/*/target/block.wasm → SKILL_WASMS
    src/lib.rs                     # CREATE: public API (run_tool/list_tools/describe_tool) + re-exports
    src/runtime.rs                 # CREATE: boot_minimal() → sealed Wafer with blocks+services
    src/args.rs                    # CREATE: schema-driven arg mapping
    src/render.rs                  # CREATE: envelope → stdout/file + exit-code policy
    src/ffmpeg_native.rs           # CREATE: NativeFfmpegService (shell-out)
    src/stub.rs                    # CREATE: stub image service for imagine
    src/skill_md.rs                # CREATE: SKILL.md generator
    src/bin/gizza.rs               # CREATE: clap entrypoint (thin)
    tests/pure_tools.rs            # CREATE
    tests/arg_mapping.rs           # CREATE
    tests/discovery.rs             # CREATE
    tests/ffmpeg_tool.rs           # CREATE (ignored unless ffmpeg present)
    tests/stub.rs                  # CREATE
  SKILL.md                         # CREATE (Task 14): generated, committed
  .github/workflows/*.yml          # MODIFY (Task 14): SKILL.md drift check
```

---

## Milestone 0 — scaffold + native-compile sanity

### Task 1: Create the `cli` crate (lib + bin) that compiles natively

**Files:**
- Create: `gizza-ai/cli/Cargo.toml`
- Create: `gizza-ai/cli/src/lib.rs`
- Create: `gizza-ai/cli/src/bin/gizza.rs`
- Modify: `gizza-ai/Cargo.toml` (only if it declares a `[workspace]`; otherwise the crate is standalone — check first)

- [ ] **Step 1: Write `cli/Cargo.toml`**

```toml
[package]
name = "gizza-cli"
version = "0.1.0"
edition = "2021"
rust-version = "1.82"
description = "Headless CLI to run gizza skill tools"
license = "MIT"

[lib]
name = "gizza_cli"
path = "src/lib.rs"

[[bin]]
name = "gizza"
path = "src/bin/gizza.rs"

[dependencies]
# Native runtime: the `wasm` feature bundles wasmi + reqwest + tokio (rt-multi-thread).
wafer-run = { git = "https://github.com/wafer-run/wafer-run", branch = "main", default-features = false, features = ["wasm"] }
wafer-block = { git = "https://github.com/wafer-run/wafer-run", branch = "main" }
wafer-core = { git = "https://github.com/wafer-run/wafer-run", branch = "main" }
wafer-block-network = { git = "https://github.com/wafer-run/wafer-run", branch = "main" }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
base64 = "0.22"
clap = { version = "4", features = ["derive"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
anyhow = "1"
# gizza shared native crate that will hold the relocated ffmpeg bridge (wired in Task 10):
# gizza-ai-block-utils = { path = "../block-utils" }

[build-dependencies]
# none yet (build.rs uses std only)
```

> If `gizza-ai/Cargo.toml` has a `[workspace]` table, add `"cli"` to `members`. It currently does **not** (it's a single package), so `cli/` is a standalone crate resolved by its own `Cargo.toml`. Mirror the app's `.cargo/config.toml` git-override convention for local sibling checkouts if present.

- [ ] **Step 2: Write a minimal `cli/src/lib.rs`**

```rust
//! gizza-cli — run gizza skill tools headlessly.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
```

- [ ] **Step 3: Write a minimal `cli/src/bin/gizza.rs`**

```rust
fn main() {
    println!("gizza {}", gizza_cli::version());
}
```

- [ ] **Step 4: Build + run natively**

Run: `cd gizza-ai/cli && cargo run --bin gizza`
Expected: prints `gizza 0.1.0` (first build is slow — it compiles wafer-run native).

- [ ] **Step 5: Commit**

```bash
cd gizza-ai
git add cli/Cargo.toml cli/src/lib.rs cli/src/bin/gizza.rs
git commit -m "feat(cli): scaffold native gizza-cli crate (lib + bin)"
```

### Task 2: Confirm `block-utils` compiles natively (de-risk the Task 10 relocation home)

**Files:** none (verification only)

- [ ] **Step 1: Build block-utils for the host target**

Run: `cd gizza-ai && cargo build -p gizza-ai-block-utils`
Expected: SUCCESS. The wasm-only items (`fetch_from_url`, `load_from_attachment`, `dispatch_ffmpeg_runtime`) are `#[cfg(target_arch="wasm32")]`-gated, so they compile out; the native types (`Envelope`, `ForUi`, `Source`, `SourceFields`, `SkillError`, `ExecArgs`-shaped DTOs) remain.

- [ ] **Step 2: Record the outcome in the Task 10 decision**

If SUCCESS → block-utils is the relocation home (Task 10 uses it).
If FAIL (wafer-sdk pulls wasm-only code into the native build) → Task 10 instead creates a new `gizza-ffmpeg-runtime` crate with zero wafer-sdk dep. Note which path applies; do not commit anything in this task.

---

## Milestone 1 — pure tool end-to-end (offline proof of architecture)

### Task 3: `build.rs` embeds skill wasm bytes

**Files:**
- Create: `gizza-ai/cli/build.rs`

- [ ] **Step 1: Write `cli/build.rs`**

```rust
use std::{env, fs, path::PathBuf};

// Embed every blocks/<tool>/target/block.wasm as raw bytes. Name + schema are
// read from block.info() at runtime, so no manifest.json is needed here.
fn main() {
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let out_dir = env::var("OUT_DIR").expect("OUT_DIR");
    // cli/ is a sibling of blocks/ under gizza-ai/.
    let blocks_dir = PathBuf::from(&manifest_dir).parent().unwrap().join("blocks");

    let mut wasm_paths: Vec<PathBuf> = Vec::new();
    if blocks_dir.is_dir() {
        for entry in fs::read_dir(&blocks_dir).expect("read blocks/") {
            let wasm = entry.expect("entry").path().join("target/block.wasm");
            if wasm.is_file() {
                println!("cargo:rerun-if-changed={}", wasm.display());
                wasm_paths.push(wasm);
            }
        }
    }
    wasm_paths.sort();
    println!("cargo:rerun-if-changed={}", blocks_dir.display());

    let mut out = String::from("// Generated by build.rs — do not edit.\n");
    out.push_str("pub const SKILL_WASMS: &[&[u8]] = &[\n");
    for p in &wasm_paths {
        out.push_str(&format!("    include_bytes!(\"{}\"),\n", p.to_string_lossy().replace('\\', "/")));
    }
    out.push_str("];\n");
    fs::write(PathBuf::from(&out_dir).join("skill_wasms.rs"), out).expect("write skill_wasms.rs");
}
```

- [ ] **Step 2: Include it from `lib.rs`** — add to `cli/src/lib.rs`:

```rust
pub mod runtime;

mod skill_wasms {
    include!(concat!(env!("OUT_DIR"), "/skill_wasms.rs"));
}
pub(crate) use skill_wasms::SKILL_WASMS;
```

- [ ] **Step 3: Build to verify embedding** — `cd gizza-ai/cli && cargo build` (requires the Prerequisites build). Expected: SUCCESS; `SKILL_WASMS` is non-empty (12 entries).

- [ ] **Step 4: Commit** — `git add cli/build.rs cli/src/lib.rs && git commit -m "feat(cli): embed skill block wasm via build.rs"`

### Task 4: `runtime::boot_minimal()` — sealed Wafer with skill blocks

**Files:**
- Create: `gizza-ai/cli/src/runtime.rs`
- Test: `gizza-ai/cli/tests/pure_tools.rs`

- [ ] **Step 1: Write the failing test** (`cli/tests/pure_tools.rs`)

```rust
use gizza_cli::runtime;

#[tokio::test]
async fn boots_and_registers_calculator() {
    let rt = runtime::boot_minimal().await.expect("boot");
    let names = rt.tool_names();
    assert!(names.iter().any(|n| n == "gizza-ai/calculator"), "got {names:?}");
}
```

- [ ] **Step 2: Run it to confirm it fails** — `cargo test -p gizza-cli --test pure_tools boots_and_registers_calculator`
Expected: FAIL (`runtime::boot_minimal` not found).

- [ ] **Step 3: Implement `runtime.rs`**

```rust
//! Boot a minimal native Wafer that hosts the embedded skill blocks.

use std::sync::Arc;
use anyhow::{Context as _, Result};
use wafer_block::core_types::Message;
use wafer_run::Wafer;
use wafer_run::wasm::WasmiBlock;

use crate::SKILL_WASMS;

/// A sealed runtime plus the list of tool block names it hosts.
pub struct ToolRuntime {
    wafer: Wafer,
    names: Vec<String>,
}

impl ToolRuntime {
    pub fn tool_names(&self) -> &[String] {
        &self.names
    }
}

/// Build + seal a Wafer with only the skill blocks registered (no services yet;
/// services are added in Milestone 3). Pure-compute tools need nothing else.
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
```

- [ ] **Step 4: Run the test to confirm it passes** — `cargo test -p gizza-cli --test pure_tools boots_and_registers_calculator`
Expected: PASS. (If `block.info().name` is a method-with-different-shape, adjust per `cargo doc -p wafer-block`; the field/method is what `lookup_skill_tool` reads.)

- [ ] **Step 5: Commit** — `git add cli/src/runtime.rs cli/tests/pure_tools.rs && git commit -m "feat(cli): boot_minimal() registers skill blocks on a native wasmi Wafer"`

### Task 5: `run_tool(name, json)` dispatch → response bytes

**Files:**
- Modify: `gizza-ai/cli/src/runtime.rs`
- Test: `gizza-ai/cli/tests/pure_tools.rs`

- [ ] **Step 1: Add the failing test** (`cli/tests/pure_tools.rs`)

```rust
#[tokio::test]
async fn calculator_evaluates() {
    let rt = runtime::boot_minimal().await.expect("boot");
    let body = rt
        .run_tool("gizza-ai/calculator", serde_json::json!({"expr": "2+2"}))
        .await
        .expect("call");
    let v: serde_json::Value = serde_json::from_slice(&body).expect("json");
    assert_eq!(v["result"], 4.0);
}
```

- [ ] **Step 2: Run it to confirm it fails** — `cargo test -p gizza-cli --test pure_tools calculator_evaluates`
Expected: FAIL (`run_tool` not found).

- [ ] **Step 3: Implement `run_tool` on `ToolRuntime`** (add to `runtime.rs`)

```rust
use wafer_block::InputStream;
use wafer_block::meta::{META_REQ_ACTION, META_REQ_RESOURCE};

impl ToolRuntime {
    /// Dispatch one tool call. `name` is the full block name ("gizza-ai/<tool>").
    /// Returns the raw response body bytes.
    pub async fn run_tool(&self, name: &str, args: serde_json::Value) -> Result<Vec<u8>> {
        let short = name.strip_prefix("gizza-ai/").unwrap_or(name);
        let body = serde_json::to_vec(&args).context("serialize args")?;

        let mut msg = Message::new("http");
        msg.set_meta(META_REQ_ACTION, "create");
        msg.set_meta(META_REQ_RESOURCE, format!("/b/{short}"));

        let out = self
            .wafer
            .run_block(name, msg, InputStream::from_bytes(body))
            .await;
        let resp = out
            .collect_buffered()
            .await
            .map_err(|e| anyhow::anyhow!("tool {name} produced no response: {e:?}"))?;
        Ok(resp.body)
    }
}
```

- [ ] **Step 4: Run the test to confirm it passes** — `cargo test -p gizza-cli --test pure_tools calculator_evaluates`
Expected: PASS.

- [ ] **Step 5: Add an error-path test + run it**

```rust
#[tokio::test]
async fn calculator_div_by_zero_errors() {
    let rt = runtime::boot_minimal().await.expect("boot");
    let body = rt.run_tool("gizza-ai/calculator", serde_json::json!({"expr":"1/0"})).await.expect("call");
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(v.get("error").is_some(), "expected error envelope, got {v}");
}
```

Run: `cargo test -p gizza-cli --test pure_tools` — Expected: all PASS.

- [ ] **Step 6: Commit** — `git add cli/src/runtime.rs cli/tests/pure_tools.rs && git commit -m "feat(cli): run_tool() dispatches a skill block and returns its body"`

### Task 6: minimal bin path — `gizza tool <name> --json '{...}'` prints the result

**Files:**
- Create: `gizza-ai/cli/src/render.rs`
- Modify: `gizza-ai/cli/src/lib.rs`, `gizza-ai/cli/src/bin/gizza.rs`

- [ ] **Step 1: Write the envelope parser + renderer** (`cli/src/render.rs`)

```rust
//! Turn a skill's response body into terminal output + an exit code.
//! Envelope contract mirrors src/blocks/agent/dispatch.rs::parse_skill_response:
//! a JSON object with a string `_for_llm` is an envelope; otherwise the whole
//! body is the human text.

use serde_json::Value;

pub struct Rendered {
    pub stdout: String,
    pub exit_code: i32,
}

pub fn render(body: &[u8], json_mode: bool) -> Rendered {
    let text = String::from_utf8_lossy(body);
    let parsed: Option<Value> = serde_json::from_str(&text).ok();

    // Tool-level error: {"error": "..."} or {"error":"tool_failed",...}
    if let Some(Value::Object(map)) = &parsed {
        if let Some(err) = map.get("error") {
            let msg = map.get("message").and_then(|m| m.as_str())
                .unwrap_or_else(|| err.as_str().unwrap_or("tool error"));
            return Rendered { stdout: msg.to_string(), exit_code: 1 };
        }
    }

    if json_mode {
        return Rendered { stdout: text.into_owned(), exit_code: 0 };
    }

    // Human mode: prefer the envelope's _for_llm; else the raw body; else result.
    if let Some(Value::Object(map)) = &parsed {
        if let Some(s) = map.get("_for_llm").and_then(|v| v.as_str()) {
            return Rendered { stdout: s.to_string(), exit_code: 0 };
        }
        if let Some(r) = map.get("result") {
            return Rendered { stdout: trim_number(r), exit_code: 0 };
        }
    }
    Rendered { stdout: text.into_owned(), exit_code: 0 }
}

// Render 4.0 as "4" but keep 3.5 as "3.5".
fn trim_number(v: &Value) -> String {
    if let Some(f) = v.as_f64() {
        if f.fract() == 0.0 { return format!("{}", f as i64); }
        return format!("{f}");
    }
    v.to_string()
}
```

- [ ] **Step 2: Write the failing renderer test** (append to `render.rs`)

```rust
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn result_number_human() {
        let r = render(br#"{"result":4.0}"#, false);
        assert_eq!(r.stdout, "4");
        assert_eq!(r.exit_code, 0);
    }
    #[test]
    fn error_nonzero() {
        let r = render(br#"{"error":"eval failed: non-finite"}"#, false);
        assert_eq!(r.exit_code, 1);
    }
    #[test]
    fn envelope_for_llm() {
        let r = render(br#"{"_for_llm":"resized cat to 64x64","_for_ui":{}}"#, false);
        assert_eq!(r.stdout, "resized cat to 64x64");
    }
}
```

- [ ] **Step 3: Run the tests to confirm they pass** — `cargo test -p gizza-cli render` — Expected: PASS.

- [ ] **Step 4: Wire the bin** (`cli/src/bin/gizza.rs`)

```rust
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "gizza")]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Run a gizza tool
    Tool {
        /// Tool name, e.g. "calculator"
        name: String,
        /// Full JSON body (escape hatch); positional/key=value mapping added in Task 8.
        #[arg(long)]
        json: Option<String>,
        /// Print the full envelope as JSON instead of human text.
        #[arg(long = "json-out")]
        json_out: bool,
    },
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    let code = match cli.command {
        Cmd::Tool { name, json, json_out } => run(name, json, json_out).await,
    };
    std::process::exit(code);
}

async fn run(name: String, json: Option<String>, json_out: bool) -> i32 {
    let args: serde_json::Value = match json.as_deref() {
        Some(s) => match serde_json::from_str(s) {
            Ok(v) => v,
            Err(e) => { eprintln!("invalid --json: {e}"); return 2; }
        },
        None => serde_json::json!({}),
    };
    let full = format!("gizza-ai/{name}");
    let rt = match gizza_cli::runtime::boot_minimal().await {
        Ok(rt) => rt,
        Err(e) => { eprintln!("boot failed: {e}"); return 1; }
    };
    match rt.run_tool(&full, args).await {
        Ok(body) => {
            let r = gizza_cli::render::render(&body, json_out);
            println!("{}", r.stdout);
            r.exit_code
        }
        Err(e) => { eprintln!("{e}"); 2 }
    }
}
```

- [ ] **Step 5: Add `pub mod render;` to `cli/src/lib.rs`, then run the bin end-to-end**

Run: `cd gizza-ai/cli && cargo run --bin gizza -- tool calculator --json '{"expr":"2*2"}'`
Expected: prints `4`, exit 0.

- [ ] **Step 6: Commit** — `git add cli/src/render.rs cli/src/lib.rs cli/src/bin/gizza.rs && git commit -m "feat(cli): gizza tool <name> --json end-to-end (pure tools live)"`

---

## Milestone 2 — schema-driven args, discovery, output contract

### Task 7: read `block.info().tool` → `list_tools()` / `describe_tool()`

**Files:**
- Modify: `gizza-ai/cli/src/runtime.rs` (capture `SkillTool` per block at boot)
- Create: `gizza-ai/cli/tests/discovery.rs`
- Modify: `gizza-ai/cli/src/bin/gizza.rs` (add `list` / `describe` subcommands)

- [ ] **Step 1: Write the failing test** (`cli/tests/discovery.rs`)

```rust
use gizza_cli::runtime;

#[tokio::test]
async fn lists_calculator_with_schema() {
    let rt = runtime::boot_minimal().await.expect("boot");
    let tools = rt.tools();
    let calc = tools.iter().find(|t| t.name == "gizza-ai/calculator").expect("calc present");
    assert!(calc.description.to_lowercase().contains("expression"));
    assert_eq!(calc.parameters["required"][0], "expr");
}
```

- [ ] **Step 2: Run it to confirm it fails** — `cargo test -p gizza-cli --test discovery` → FAIL (`rt.tools()` missing).

- [ ] **Step 3: Capture tool metadata at boot** — in `runtime.rs`, add a `ToolMeta` struct and populate it from `block.info().tool` before registering:

```rust
#[derive(Clone, Debug)]
pub struct ToolMeta {
    pub name: String,          // "gizza-ai/calculator"
    pub short: String,         // "calculator"
    pub description: String,
    pub parameters: serde_json::Value,
}

// In boot_minimal(), replace the registration loop body with:
let info = block.info();
let name = info.name.clone();
if let Some(tool) = info.tool.clone() {            // Option<SkillTool { description, parameters }>
    metas.push(ToolMeta {
        name: name.clone(),
        short: name.strip_prefix("gizza-ai/").unwrap_or(&name).to_string(),
        description: tool.description,
        parameters: tool.parameters,
    });
}
wafer.register_block(&name, Arc::new(block)).map_err(|e| anyhow::anyhow!("register {name}: {e}"))?;
names.push(name);
```

Store `metas: Vec<ToolMeta>` on `ToolRuntime` (sorted by name) and expose:

```rust
impl ToolRuntime {
    pub fn tools(&self) -> &[ToolMeta] { &self.metas }
    pub fn tool(&self, short_or_full: &str) -> Option<&ToolMeta> {
        let full = if short_or_full.starts_with("gizza-ai/") { short_or_full.to_string() }
                   else { format!("gizza-ai/{short_or_full}") };
        self.metas.iter().find(|m| m.name == full)
    }
}
```

- [ ] **Step 4: Run the test to confirm it passes** — `cargo test -p gizza-cli --test discovery` → PASS.
(If a block has no `tool` in `info()`, it is excluded from `tools()` — pure non-skill blocks like `gizza-ai/ffmpeg-runtime` should not appear. Verify the video-* tools DO appear, proving the `info()` source bypasses their stale manifests.)

- [ ] **Step 5: Add `list` / `describe` subcommands to the bin** (extend the `Cmd` enum + a `match`):

```rust
/// List available tools
List { #[arg(long = "json-out")] json_out: bool },
/// Show a tool's description + JSON schema
Describe { name: String, #[arg(long = "json-out")] json_out: bool },
```
with handlers that boot, then print `rt.tools()` (name + description) or the matched `tool()` (`--json-out` prints `parameters`). Unknown tool in `describe` → eprintln + exit 2.

- [ ] **Step 6: Verify + commit**

Run: `cargo run --bin gizza -- tool list` (expect calculator, clock, image-resize, …, video-trim) and `cargo run --bin gizza -- tool describe calculator --json-out` (expect the JSON schema).
```bash
git add cli/src/runtime.rs cli/src/bin/gizza.rs cli/tests/discovery.rs
git commit -m "feat(cli): tool list/describe from block.info().tool (single source)"
```

### Task 8: schema-driven arg mapping (positional / key=value / --json)

**Files:**
- Create: `gizza-ai/cli/src/args.rs`
- Create: `gizza-ai/cli/tests/arg_mapping.rs`
- Modify: `gizza-ai/cli/src/bin/gizza.rs`

- [ ] **Step 1: Write the failing tests** (`cli/tests/arg_mapping.rs`)

```rust
use gizza_cli::args::map_args;
use serde_json::json;

fn calc_schema() -> serde_json::Value {
    json!({"type":"object","required":["expr"],
           "properties":{"expr":{"type":"string"}}})
}
fn resize_schema() -> serde_json::Value {
    json!({"type":"object",
           "properties":{"url":{"type":"string"},"width":{"type":"integer"},"fit":{"type":"string"}}})
}

#[test]
fn single_required_positional() {
    let body = map_args(&calc_schema(), &["2*2".into()], None).unwrap();
    assert_eq!(body, json!({"expr":"2*2"}));
}
#[test]
fn key_value_with_type_coercion() {
    let body = map_args(&resize_schema(), &["url=http://x/a.png".into(), "width=640".into()], None).unwrap();
    assert_eq!(body, json!({"url":"http://x/a.png","width":640}));
}
#[test]
fn json_escape_hatch() {
    let body = map_args(&resize_schema(), &[], Some(r#"{"url":"http://x","width":10}"#)).unwrap();
    assert_eq!(body["width"], 10);
}
#[test]
fn positional_and_json_is_error() {
    assert!(map_args(&calc_schema(), &["2*2".into()], Some("{}")).is_err());
}
#[test]
fn missing_required_is_error() {
    assert!(map_args(&calc_schema(), &[], None).is_err());
}
```

- [ ] **Step 2: Run them to confirm they fail** — `cargo test -p gizza-cli --test arg_mapping` → FAIL (`map_args` missing).

- [ ] **Step 3: Implement `args.rs`**

```rust
//! Map CLI args to a JSON body using the tool's JSON schema.
//! Precedence: --json (whole body) XOR (positional + key=value).

use anyhow::{bail, Result};
use serde_json::{Map, Value};

pub fn map_args(schema: &Value, positional: &[String], json: Option<&str>) -> Result<Value> {
    if json.is_some() && !positional.is_empty() {
        bail!("pass either positional/key=value args or --json, not both");
    }
    if let Some(s) = json {
        let v: Value = serde_json::from_str(s).map_err(|e| anyhow::anyhow!("invalid --json: {e}"))?;
        validate_required(schema, &v)?;
        return Ok(v);
    }

    let props = schema.get("properties").and_then(|p| p.as_object());
    let required: Vec<&str> = schema.get("required").and_then(|r| r.as_array())
        .map(|a| a.iter().filter_map(|v| v.as_str()).collect()).unwrap_or_default();

    let mut body = Map::new();
    let mut pos_iter = positional.iter();

    for arg in positional.iter() {
        if let Some((k, v)) = arg.split_once('=') {
            // key=value
            let _ = pos_iter.next(); // consumed
            let coerced = coerce(props, k, v);
            body.insert(k.to_string(), coerced);
        }
    }
    // Bare positionals (no '=') fill required scalar props in order.
    let bare: Vec<&String> = positional.iter().filter(|a| !a.contains('=')).collect();
    let scalar_required: Vec<&str> = required.iter().copied()
        .filter(|k| is_scalar(props, k)).collect();
    if !bare.is_empty() {
        if bare.len() > scalar_required.len() {
            bail!("too many positional args; expected at most {} ({:?}). Use key=value.",
                  scalar_required.len(), scalar_required);
        }
        for (k, val) in scalar_required.iter().zip(bare.iter()) {
            body.insert((*k).to_string(), coerce(props, k, val));
        }
    }

    let v = Value::Object(body);
    validate_required(schema, &v)?;
    Ok(v)
}

fn is_scalar(props: Option<&Map<String, Value>>, key: &str) -> bool {
    match props.and_then(|p| p.get(key)).and_then(|s| s.get("type")).and_then(|t| t.as_str()) {
        Some("string") | Some("integer") | Some("number") | Some("boolean") => true,
        _ => false,
    }
}

fn coerce(props: Option<&Map<String, Value>>, key: &str, raw: &str) -> Value {
    let ty = props.and_then(|p| p.get(key)).and_then(|s| s.get("type")).and_then(|t| t.as_str());
    match ty {
        Some("integer") => raw.parse::<i64>().map(Value::from).unwrap_or_else(|_| Value::from(raw)),
        Some("number")  => raw.parse::<f64>().map(Value::from).unwrap_or_else(|_| Value::from(raw)),
        Some("boolean") => match raw { "true" => Value::Bool(true), "false" => Value::Bool(false), _ => Value::from(raw) },
        _ => Value::from(raw),
    }
}

fn validate_required(schema: &Value, body: &Value) -> Result<()> {
    if let Some(req) = schema.get("required").and_then(|r| r.as_array()) {
        for k in req.iter().filter_map(|v| v.as_str()) {
            if body.get(k).is_none() {
                bail!("missing required arg `{k}`");
            }
        }
    }
    Ok(())
}
```

- [ ] **Step 4: Run the tests to confirm they pass** — `cargo test -p gizza-cli --test arg_mapping` → PASS.

- [ ] **Step 5: Wire into the bin** — change `Tool` to take `args: Vec<String>` positional + keep `--json`; in `run`, look up the tool's schema via a booted `rt.tool(&name)`, then `map_args(&meta.parameters, &args, json.as_deref())`. On `Err`, eprintln + exit 2. Boot once, reuse for schema + dispatch.

- [ ] **Step 6: Verify + commit**

Run: `cargo run --bin gizza -- tool calculator "2*2"` → `4`; `cargo run --bin gizza -- tool clock "%H:%M"` → a time.
```bash
git add cli/src/args.rs cli/src/lib.rs cli/src/bin/gizza.rs cli/tests/arg_mapping.rs
git commit -m "feat(cli): schema-driven positional/key=value arg mapping"
```

### Task 9: output contract — binary→file, exit-code policy

**Files:**
- Modify: `gizza-ai/cli/src/render.rs` (handle `_for_ui.data_url`), `gizza-ai/cli/src/bin/gizza.rs` (`--out`, exit codes)

- [ ] **Step 1: Add the failing test** (`render.rs` tests)

```rust
#[test]
fn data_url_extracts_bytes_and_filename() {
    // "AAAA" base64 of [0,0,0]
    let body = br#"{"_for_llm":"made png","_for_ui":{"data_url":"data:image/png;base64,AAAA","filename":"out.png","mime":"image/png"}}"#;
    let bin = super::extract_binary(body).expect("some");
    assert_eq!(bin.filename, "out.png");
    assert_eq!(bin.bytes, vec![0,0,0]);
}
```

- [ ] **Step 2: Implement `extract_binary`** in `render.rs`:

```rust
use base64::{engine::general_purpose::STANDARD as B64, Engine as _};

pub struct BinaryOut { pub filename: String, pub bytes: Vec<u8> }

/// If the envelope's _for_ui.data_url is a base64 data URL, decode it.
pub fn extract_binary(body: &[u8]) -> Option<BinaryOut> {
    let v: Value = serde_json::from_slice(body).ok()?;
    let ui = v.get("_for_ui")?;
    let data_url = ui.get("data_url")?.as_str()?;
    let b64 = data_url.split(";base64,").nth(1)?;
    let bytes = B64.decode(b64).ok()?;
    let filename = ui.get("filename").and_then(|f| f.as_str()).unwrap_or("output.bin").to_string();
    Some(BinaryOut { filename, bytes })
}
```

- [ ] **Step 3: Run the test** — `cargo test -p gizza-cli render` → PASS.

- [ ] **Step 4: Wire `--out` into the bin** — after a successful `run_tool`, if `extract_binary(&body)` is `Some`, write bytes to `--out <path>` (default: the envelope `filename` in cwd), print the `for_llm` summary, and (in `--json-out`) replace `data_url` with the written path before printing. Exit codes: 0 ok, 1 tool error, 2 usage, 3 unsupported (wired in Task 13). Route all errors to stderr, tool output to stdout.

- [ ] **Step 5: Commit** — `git add cli/src/render.rs cli/src/bin/gizza.rs && git commit -m "feat(cli): binary outputs to --out, exit-code policy, stdout/stderr split"`

---

## Milestone 3 — host services (ffmpeg + network) + relocation + stub

### Task 10: relocate `FfmpegBlock`/`FfmpegService`/`ExecArgs`/`ExecResult` to a native-compilable crate

**Files (if Task 2 said block-utils builds native — primary path):**
- Modify: `gizza-ai/block-utils/src/lib.rs` (add the relocated `ffmpeg` module: `FfmpegBlock`, `FfmpegService`, `FfmpegError`, `ExecArgs`, `ExecResult`; keep `BrowserFfmpegService` behind `#[cfg(target_arch="wasm32")]`)
- Modify: `gizza-ai/block-utils/Cargo.toml` (ensure `wafer-block` dep with `Block`/`Context`/`async-trait` available natively)
- Delete: `gizza-ai/src/ffmpeg.rs`, `gizza-ai/src/blocks/ffmpeg.rs`
- Modify: `gizza-ai/src/blocks/mod.rs` (drop `pub mod ffmpeg`), `gizza-ai/src/lib.rs` (import `FfmpegBlock`/`BrowserFfmpegService` from `gizza_ai_block_utils::ffmpeg`)

**(Fallback if Task 2 said block-utils does NOT build native:** create a new crate `gizza-ai/ffmpeg-runtime/` with the same module and zero `wafer-sdk` dep; both app and CLI depend on it.)**

- [ ] **Step 1: Move the code** — cut the three native items (`FfmpegService` trait + `ExecArgs`/`ExecResult`/`FfmpegError` from `src/ffmpeg.rs`, and `FfmpegBlock` from `src/blocks/ffmpeg.rs`) into a new `pub mod ffmpeg` in the chosen crate, verbatim. Keep `BrowserFfmpegService` gated to `#[cfg(target_arch="wasm32")]` in the same module.

- [ ] **Step 2: Update the app's imports** — in `src/lib.rs` change the registration to:

```rust
let ffmpeg_svc: Arc<dyn gizza_ai_block_utils::ffmpeg::FfmpegService> =
    Arc::new(gizza_ai_block_utils::ffmpeg::BrowserFfmpegService);
wafer.register_block(
    "gizza-ai/ffmpeg-runtime",
    Arc::new(gizza_ai_block_utils::ffmpeg::FfmpegBlock::new(ffmpeg_svc)),
)?;
```
and delete `pub mod ffmpeg;` from `src/lib.rs` and `pub mod ffmpeg;` from `src/blocks/mod.rs`.

- [ ] **Step 3: Verify both targets build**

Run: `cd gizza-ai && cargo build -p gizza-ai-block-utils` (native) → SUCCESS.
Run: `cd gizza-ai && cargo build --target wasm32-unknown-unknown` (or `solobase build`) → app still compiles, registration unchanged in behavior.

- [ ] **Step 4: Commit** — `git add -A && git commit -m "refactor: relocate ffmpeg bridge to native-compilable crate (app + cli share it)"`

### Task 11: `NativeFfmpegService` (shell-out) + register `ffmpeg-runtime` in the CLI

**Files:**
- Create: `gizza-ai/cli/src/ffmpeg_native.rs`
- Modify: `gizza-ai/cli/src/runtime.rs` (register `gizza-ai/ffmpeg-runtime`), `cli/Cargo.toml` (add `gizza-ai-block-utils` dep + `tempfile`)
- Create: `gizza-ai/cli/tests/ffmpeg_tool.rs`

- [ ] **Step 1: Implement `NativeFfmpegService`** (`cli/src/ffmpeg_native.rs`)

```rust
//! ffmpeg via the system binary, implementing the shared FfmpegService trait.

use std::sync::Arc;
use gizza_ai_block_utils::ffmpeg::{ExecArgs, ExecResult, FfmpegError, FfmpegService};

pub struct NativeFfmpegService;

impl NativeFfmpegService {
    pub fn arc() -> Arc<dyn FfmpegService> { Arc::new(NativeFfmpegService) }
}

#[async_trait::async_trait]
impl FfmpegService for NativeFfmpegService {
    async fn exec(&self, args: ExecArgs) -> Result<ExecResult, FfmpegError> {
        let dir = tempfile::tempdir().map_err(|e| FfmpegError::Bridge(format!("tempdir: {e}")))?;
        // Write virtual-FS inputs to the temp dir.
        for (name, bytes) in &args.inputs {
            std::fs::write(dir.path().join(name), bytes)
                .map_err(|e| FfmpegError::Bridge(format!("write {name}: {e}")))?;
        }
        let output = std::process::Command::new("ffmpeg")
            .args(&args.args)
            .current_dir(dir.path())
            .output();
        let output = match output {
            Ok(o) => o,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound =>
                return Err(FfmpegError::Bridge("`ffmpeg` not found on PATH — install ffmpeg to use this tool".into())),
            Err(e) => return Err(FfmpegError::Bridge(format!("spawn ffmpeg: {e}"))),
        };
        let out_bytes = std::fs::read(dir.path().join(&args.output)).unwrap_or_default();
        Ok(ExecResult {
            exit_code: output.status.code().unwrap_or(-1),
            output: out_bytes,
            log: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}
```

Add to `cli/Cargo.toml`: `tempfile = "3"`, `async-trait = "0.1"`, and uncomment the `gizza-ai-block-utils` path dep.

- [ ] **Step 2: Register it in the runtime** — add a `boot_full()` (or a `boot(Services)` parameter) in `runtime.rs` that, after registering skill blocks, also registers:

```rust
wafer.register_block(
    "gizza-ai/ffmpeg-runtime",
    Arc::new(gizza_ai_block_utils::ffmpeg::FfmpegBlock::new(crate::ffmpeg_native::NativeFfmpegService::arc())),
).map_err(|e| anyhow::anyhow!("register ffmpeg-runtime: {e}"))?;
```
Keep `boot_minimal()` for pure tools; the bin calls `boot_full()`.

- [ ] **Step 3: Write an ffmpeg integration test, ignored unless ffmpeg present** (`cli/tests/ffmpeg_tool.rs`)

```rust
use gizza_cli::runtime;

fn have_ffmpeg() -> bool {
    std::process::Command::new("ffmpeg").arg("-version").output().map(|o| o.status.success()).unwrap_or(false)
}

#[tokio::test]
async fn image_resize_via_native_ffmpeg() {
    if !have_ffmpeg() { eprintln!("skipping: ffmpeg not installed"); return; }
    let rt = runtime::boot_full().await.expect("boot");
    // 1x1 png fixture as a data: URL is fetched via network; here use a local fixture path
    // mapped through file= once Task 12/ref-input lands. For now assert the tool is callable
    // and returns a structured error (not a panic) when given a bad url.
    let body = rt.run_tool("gizza-ai/image-resize",
        serde_json::json!({"url":"http://127.0.0.1:0/nope.png","width":8})).await;
    assert!(body.is_ok(), "dispatch should not panic: {body:?}");
}
```

- [ ] **Step 4: Run** — `cargo test -p gizza-cli --test ffmpeg_tool` → PASS (or prints skip).

- [ ] **Step 5: Commit** — `git add cli/src/ffmpeg_native.rs cli/src/runtime.rs cli/Cargo.toml cli/tests/ffmpeg_tool.rs && git commit -m "feat(cli): NativeFfmpegService shell-out + ffmpeg-runtime registration"`

### Task 12: register the native network service block

**Files:**
- Modify: `gizza-ai/cli/src/runtime.rs` (register `wafer-run/network` backed by `HttpNetworkService`)

- [ ] **Step 1: Find the exact native registration API** — read `solobase/crates/solobase-native/src/network.rs` (`make_fetch_network_service()` → `Arc<dyn NetworkService>`) and how it is wrapped into a block (search `wafer_core::service_blocks::network` in wafer-run/wafer-core for a `register_with`/`new`-style block constructor). Confirm the block NAME (expected `"wafer-run/network"`).

- [ ] **Step 2: Register it in `boot_full()`** — concrete shape (adjust the constructor to the exact API found in Step 1):

```rust
use wafer_block_network::service::HttpNetworkService;
// e.g. wafer_core::service_blocks::network exposes a Block wrapper:
let net_block = wafer_core::service_blocks::network::block(Arc::new(HttpNetworkService::new()));
wafer.register_block("wafer-run/network", Arc::new(net_block))
    .map_err(|e| anyhow::anyhow!("register network: {e}"))?;
```

> This is the one spot whose exact constructor must be read from source (Agent audit could not pin the `NetworkService → Block` wrapper). The two candidates are a `service_blocks::network` helper in wafer-core, or the pattern solobase-native uses. Acceptance: `web-fetch` returns content for a real URL.

- [ ] **Step 3: Manual verification** (network-dependent)

Run: `cargo run --bin gizza -- tool web-fetch url=https://example.com`
Expected: fetched text/summary, exit 0.

- [ ] **Step 4: Commit** — `git add cli/src/runtime.rs && git commit -m "feat(cli): register native HttpNetworkService block (web-fetch/image tools)"`

### Task 13: stub the `imagine` image capability → typed unsupported error

**Files:**
- Create: `gizza-ai/cli/src/stub.rs`
- Modify: `gizza-ai/cli/src/runtime.rs` (register a stub image service), `gizza-ai/cli/src/render.rs` (map `unsupported_in_cli` → exit 3), `gizza-ai/cli/tests/stub.rs`

- [ ] **Step 1: Implement a stub `ImageService`** (`cli/src/stub.rs`) implementing `wafer_core::interfaces::image::service::ImageService` whose `generate` returns an error carrying `unsupported_in_cli` (read the trait signature; mirror the shape `imagine` calls via `wafer_sdk::clients::image::generate`). The goal: `gizza-ai/imagine` dispatch yields a body `{"error":"unsupported_in_cli","message":"text-to-image needs a browser GPU; use gizza.ai"}`.

> If wiring a stub `ImageService` into the raw Wafer is heavier than a one-liner, the equivalent fallback (same observable behavior) is: in `run_tool`, short-circuit `gizza-ai/imagine` to that exact error body before dispatch, with a comment that this is the single stubbed capability. Prefer the service-injection path for symmetry; use the short-circuit only if the image-service block registration is disproportionate.

- [ ] **Step 2: Map the error to exit 3** — in `render.rs`, when the error object's `error == "unsupported_in_cli"`, return `exit_code: 3`.

- [ ] **Step 3: Write the test** (`cli/tests/stub.rs`)

```rust
use gizza_cli::runtime;
#[tokio::test]
async fn imagine_is_unsupported_in_cli() {
    let rt = runtime::boot_full().await.expect("boot");
    let body = rt.run_tool("gizza-ai/imagine", serde_json::json!({"prompt":"a cat"})).await.expect("dispatch");
    let v: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(v["error"], "unsupported_in_cli");
    assert_eq!(gizza_cli::render::render(&body, false).exit_code, 3);
}
```

- [ ] **Step 4: Run** — `cargo test -p gizza-cli --test stub` → PASS.

- [ ] **Step 5: Commit** — `git add cli/src/stub.rs cli/src/runtime.rs cli/src/render.rs cli/tests/stub.rs && git commit -m "feat(cli): stub imagine with typed unsupported_in_cli error (exit 3)"`

---

## Milestone 4 — SKILL.md generation + CI drift + docs

### Task 14: generate `SKILL.md` from `tool list` + CI drift check

**Files:**
- Create: `gizza-ai/cli/src/skill_md.rs`
- Modify: `gizza-ai/cli/src/bin/gizza.rs` (add hidden `tool gen-skill` subcommand)
- Create: `gizza-ai/SKILL.md` (generated, committed)
- Modify: a `.github/workflows/*.yml` (drift check)

- [ ] **Step 1: Implement the generator** (`cli/src/skill_md.rs`) — `fn render_skill_md(tools: &[ToolMeta]) -> String` producing a Markdown doc: a header describing the `gizza tool <name> [args]` contract (positional/key=value/--json, `--json-out`, exit codes), then one section per tool with its `name`, `description`, and a fenced JSON `parameters` schema. Add a unit test asserting the output contains `## calculator` and the `expr` property.

- [ ] **Step 2: Add the `gen-skill` subcommand** — `gizza tool gen-skill [--check]`: boots, renders, and either writes `SKILL.md` (relative to the repo root) or, with `--check`, exits 1 if the on-disk `SKILL.md` differs from a fresh render.

- [ ] **Step 3: Generate + commit `SKILL.md`**

Run: `cargo run --bin gizza -- tool gen-skill` then `git add SKILL.md`.

- [ ] **Step 4: Add the CI drift check** — in the test/CI workflow, after building blocks: `cargo run -p gizza-cli --bin gizza -- tool gen-skill --check` (fails the build if `SKILL.md` is stale).

- [ ] **Step 5: Run the unit test + the check locally** — `cargo test -p gizza-cli skill_md` and `cargo run --bin gizza -- tool gen-skill --check` (expect exit 0 right after generating).

- [ ] **Step 6: Commit** — `git add cli/src/skill_md.rs cli/src/bin/gizza.rs SKILL.md .github && git commit -m "feat(cli): generated SKILL.md + CI drift check"`

### Task 15: library API surface + README + full test pass

**Files:**
- Modify: `gizza-ai/cli/src/lib.rs` (export `run_tool`/`list_tools`/`describe_tool` as the stable library API), `gizza-ai/cli/README.md` (create)

- [ ] **Step 1: Expose the library API** — in `lib.rs`, add thin free functions over `runtime` so third-party Rust programs can embed the CLI without the binary:

```rust
/// One-shot: boot, run a tool by short name with JSON args, return the envelope body.
pub async fn run_tool(name: &str, args: serde_json::Value) -> anyhow::Result<Vec<u8>> {
    let rt = runtime::boot_full().await?;
    rt.run_tool(&format!("gizza-ai/{}", name.trim_start_matches("gizza-ai/")), args).await
}
```
plus `list_tools()` and `describe_tool(name)` returning `ToolMeta`.

- [ ] **Step 2: Write `cli/README.md`** documenting `gizza tool <name> "<arg>"`, `key=value`, `--json`, `--json-out`, `--out`, `gizza tool list/describe`, exit codes, the ffmpeg dependency, and the `imagine` limitation. Reference `SKILL.md` for the agent contract.

- [ ] **Step 3: Full test run** — `cargo test -p gizza-cli` → all PASS (ffmpeg test self-skips if absent).

- [ ] **Step 4: Commit** — `git add cli/src/lib.rs cli/README.md && git commit -m "feat(cli): public library API + README; full test pass"`

---

## Self-review notes

- **Spec coverage:** §Architecture→Tasks 4–5; §Host services→Tasks 11–13; §arg contract→Task 8; §output→Tasks 6,9; §discovery/SKILL.md→Tasks 7,14; §relocation→Task 10; §library reuse→Task 15; §testing→every task. The `file=<path>`/`ref` input (spec "ref inputs", left open) is **not yet a task** — it is only needed for image/video tools fed by a local file; tracked as a follow-up below, since URL inputs (Task 12) already exercise the host path.
- **Known open spots flagged inline, not hidden:** the exact `NetworkService→Block` wrapper (Task 12 Step 1), the `BlockInfo.tool` accessor shape (Reference note), and the stub `ImageService` vs short-circuit (Task 13 Step 1). Each has a concrete acceptance criterion.
- **Follow-up (post-v1, not in this plan):** `file=<path>` local-file input mapping onto each block's `Source`/`SourceFields`; only relevant once a user wants to resize a local image rather than a URL.
