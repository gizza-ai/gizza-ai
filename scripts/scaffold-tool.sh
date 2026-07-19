#!/usr/bin/env bash
# Scaffold the name-substituted BOILERPLATE for a new gizza tool. The skill
# (.claude/skills/new-tool/SKILL.md) fills the tool-specific logic afterward.
# Usage: scripts/scaffold-tool.sh <slug> <pure|ffmpeg>
set -euo pipefail

# Portable in-place sed (GNU `sed -i` has no BSD/macOS equivalent: `sed -i`
# without a suffix consumes the next arg on BSD). Write to a temp file and move
# it back — identical behavior on GNU and BSD sed.
sed_inplace() {
  local expr="$1" file="$2" tmp
  tmp="$(mktemp)"
  sed "$expr" "$file" > "$tmp" && mv "$tmp" "$file"
}

slug="${1:?usage: scaffold-tool.sh <slug> <pure|ffmpeg>}"
type="${2:?usage: scaffold-tool.sh <slug> <pure|ffmpeg>}"
# kebab-case [a-z0-9-]; may start with a digit (e.g. 7z-extract) but not a hyphen,
# and must not start/end with '-'. Block names allow leading digits (wafer-run
# validate_block_name); all crate/lib identifiers are prefixed gizza-ai-/gizza_ai_.
[[ "$slug" =~ ^[a-z0-9]([a-z0-9-]*[a-z0-9])?$ ]] || { echo "slug must be kebab-case [a-z0-9-], no leading/trailing hyphen" >&2; exit 2; }
[[ "$type" == pure || "$type" == ffmpeg ]] || { echo "type must be pure|ffmpeg" >&2; exit 2; }
root="$(cd "$(dirname "$0")/.." && pwd)"
dir="$root/blocks/$slug"
[[ -e "$dir" ]] && { echo "blocks/$slug already exists — refusing to overwrite" >&2; exit 2; }

under="${slug//-/_}"
crate="gizza-ai-$slug"
ucrate="gizza_ai_${under}"

mkdir -p "$dir/core/src" "$dir/web/src" "$dir/src" "$dir/page" "$dir/tests"

cat > "$dir/Cargo.toml" <<EOF
[workspace]
resolver = "2"
members = [".", "core", "web"]

[package]
name = "${crate}-block"
version = "0.1.0"
edition = "2021"
rust-version = "1.82"

[lib]
crate-type = ["cdylib", "rlib"]

[dependencies]
wafer-sdk = { git = "https://github.com/wafer-run/wafer-run.git", branch = "main" }
wafer-block = { git = "https://github.com/wafer-run/wafer-run.git", branch = "main" }
${crate}-core = { path = "core" }
gizza-ai-block-utils = { path = "../../block-utils" }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
EOF

cat > "$dir/core/Cargo.toml" <<EOF
[package]
name = "${crate}-core"
version = "0.1.0"
edition = "2021"
rust-version = "1.82"

[dependencies]
EOF
cat > "$dir/core/src/lib.rs" <<'EOF'
//! __SLUG__ core — pure compute, shared by the chat skill block and the web page.
//! No wafer/wasm-bindgen deps. The new-tool skill replaces `run` with real logic.

/// Replace with the tool's real signature + logic (see SKILL.md).
pub fn run(input: &str) -> Result<String, String> {
    let _ = input;
    Err("not implemented".into())
}
EOF

cat > "$dir/web/Cargo.toml" <<EOF
[package]
name = "${crate}-web"
version = "0.1.0"
edition = "2021"
rust-version = "1.82"

[lib]
crate-type = ["cdylib"]

[dependencies]
wasm-bindgen = "0.2"
${crate}-core = { path = "../core" }
EOF
cat > "$dir/web/src/lib.rs" <<EOF
//! Browser-facing wasm-bindgen wrapper for /tools/$slug/.
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn run(input: &str) -> Result<String, JsValue> {
    ${ucrate}_core::run(input).map_err(|e| JsValue::from_str(&e))
}
EOF

cat > "$dir/src/lib.rs" <<EOF
//! gizza-ai/$slug — chat skill block on the shared tool abstraction.
//! The chat schema is single-sourced from descriptor() (which also drives the
//! CLI); handle() delegates to block_utils::run_skill. The new-tool skill edits
//! descriptor()'s params + core::run to the tool's real inputs/logic.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{run_skill, Input, Param, SkillError, ToolDescriptor};
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args { input: String }

/// Single source for the chat schema (and CLI). Edit the params to match the
/// tool's real inputs — e.g. \`.param(Param::enumv("mode", ["a","b"]).default("a"))\`,
/// \`.param(Param::integer("n").min(1.0))\`. Use Input::Image/Video/Document/File
/// for tools that take a url/ref media input (see image-resize / web-fetch).
fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("input").required().describe("TODO: describe the input."))
}
fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/$slug",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "TODO: one-line summary.",
    skill(
        description = "TODO: describe what this tool does and its inputs.",
        parameters = schema_json()
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // run_skill wraps the returned value in { "result": ... }. For a media
        // tool, use resolve_source + dispatch_ffmpeg + build_media_envelope
        // instead (see blocks/image-resize/src/lib.rs).
        match run_skill(&body, "$slug", |a: Args| {
            ${ucrate}_core::run(&a.input).map_err(SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}
EOF

cat > "$dir/wafer.toml" <<EOF
[package]
org = "gizza-ai"
name = "$slug"
version = "0.1.0"
abi = 1
summary = "TODO: one-line summary."
EOF

# manifest.json is NOT produced by \`wafer build\` — it is a committed, hand-kept
# file that the repo build.rs requires (alongside target/block.wasm) to include
# the block in the app + CLI. Generate it here; the skill syncs the tool section
# to the src/lib.rs skill() schema (the runtime reads schemas from info(), so the
# tool section is informational, but build.rs needs the file + name to exist).
cat > "$dir/manifest.json" <<EOF
{
  "name": "gizza-ai/$slug",
  "version": "0.1.0",
  "interface": "handler@v1",
  "summary": "TODO: one-line summary.",
  "role": "skill",
  "tool": {
    "description": "TODO: describe what this tool does and its inputs.",
    "parameters": {
      "type": "object",
      "required": ["input"],
      "properties": { "input": { "type": "string" } },
      "additionalProperties": false
    }
  }
}
EOF

cat > "$dir/page/meta.toml" <<EOF
slug          = "$slug"
title         = "TODO"
description   = "TODO."
tags          = []
h1            = "TODO"
hero_subtitle = "TODO."
wasm          = "${ucrate}_web"
export        = "run"
live          = false
output_label  = "Result"
format        = "text"

[[input]]
name        = "input"
label       = "Input"
placeholder = ""
source      = "field"
EOF
cat > "$dir/page/content.md" <<EOF
## About this tool

TODO: SEO copy.

## FAQ

<!-- FAQ MUST be <details>/<summary> accordions: tools/generator/assets/runtime/tool.css styles them and
     scripts/check-tool-hygiene.py fails the build on a plain-markdown FAQ. Keep
     the blank line inside each <details> so the answer's markdown (inline
     \`code\`, **bold**, lists) renders and gets wrapped in <p>. One <details> per
     question; write real Q&A, not these TODOs. -->

<details>
<summary>TODO: a real question users ask about this tool?</summary>

TODO: the answer in plain sentences.

</details>
EOF

if [[ "$type" == ffmpeg ]]; then
  cat > "$dir/web/src/lib.rs" <<EOF
//! Browser-facing wasm-bindgen wrapper for /tools/$slug/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core); returns
//! the shared block_utils::ArgvPlan so the page driver gets { argv, out_name }.
use gizza_ai_block_utils::ArgvPlan;
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn build_argv(in_name: &str) -> Result<JsValue, JsValue> {
    let (argv, out_name) = ${ucrate}_core::plan(in_name).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&ArgvPlan { argv, out_name }).map_err(|e| JsValue::from_str(&e.to_string()))
}
EOF
  cat > "$dir/web/Cargo.toml" <<EOF
[package]
name = "${crate}-web"
version = "0.1.0"
edition = "2021"
rust-version = "1.82"

[lib]
crate-type = ["cdylib"]

[dependencies]
wasm-bindgen = "0.2"
serde-wasm-bindgen = "0.6"
gizza-ai-block-utils = { path = "../../../block-utils" }
${crate}-core = { path = "../core" }
EOF
  cat > "$dir/src/lib.rs" <<EOF
//! gizza-ai/$slug — media (ffmpeg) chat skill on the shared abstraction.
//! Input::Image emits a url⊕ref oneOf; run() uses resolve_source → core::plan →
//! dispatch_ffmpeg → build_media_envelope. The new-tool skill fills the params +
//! core::plan logic. See blocks/image-resize/src/lib.rs for a full example.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use gizza_ai_block_utils::{
    build_media_envelope, mime_to_ext, AssetKind, Input, Param, SkillError, SourceFields,
    ToolDescriptor,
};
// resolve_source / dispatch_ffmpeg call host imports → wasm-only (like run() below).
#[cfg(target_arch = "wasm32")]
use gizza_ai_block_utils::{dispatch_ffmpeg, resolve_source};
use serde::Deserialize;
use wafer_sdk::*;

const MAX_BYTES: usize = 8 * 1024 * 1024;

#[derive(Deserialize)]
struct Args {
    #[serde(flatten)]
    source: SourceFields,
    // TODO: add the tool's scalar params, e.g. \`width: Option<u32>\`.
}

fn descriptor() -> ToolDescriptor {
    // Input::Image → url⊕ref oneOf (use Input::Video for video tools).
    ToolDescriptor::new(Input::Image)
    // TODO: .param(Param::integer("width").min(1.0).describe("..."))
}
fn schema_json() -> String { descriptor().to_schema_json() }

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/$slug",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "TODO: one-line summary.",
    requires = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"],
    capabilities(network, callable_blocks = ["wafer-run/network", "gizza-ai/ffmpeg-runtime"]),
    skill(description = "TODO: describe the tool.", parameters = schema_json()),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body)
        .map_err(|e| SkillError::InvalidArgs(format!("invalid $slug args: {e}")))?;
    let (bytes, mime, in_name) =
        resolve_source(args.source.into_inner(), AssetKind::Image, MAX_BYTES)?;
    let ext = mime_to_ext(&mime)
        .ok_or_else(|| SkillError::InvalidArgs(format!("unsupported mime: {mime}")))?;
    let (argv, out_name) =
        ${ucrate}_core::plan(&format!("in.{ext}")).map_err(SkillError::InvalidArgs)?;
    let output = dispatch_ffmpeg(argv, format!("in.{ext}"), bytes, out_name)?;
    // TODO: refine the output mime/filename/summary for this tool.
    build_media_envelope(&output, &mime, in_name.clone(), format!("processed {in_name}"), MAX_BYTES)
}
EOF
  cat > "$dir/core/src/lib.rs" <<'EOF'
//! __SLUG__ core — pure ffmpeg argv construction shared by the chat block + page.
/// Replace with the tool's real params + ffmpeg argv (see SKILL.md, image-resize core).
pub fn plan(in_name: &str) -> Result<(Vec<String>, String), String> {
    let _ = in_name;
    Err("not implemented".into())
}
EOF
  cat > "$dir/page/meta.toml" <<EOF
slug          = "$slug"
title         = "TODO"
description   = "TODO."
tags          = []
h1            = "TODO"
hero_subtitle = "TODO."
wasm          = "${ucrate}_web"
export        = "build_argv"
runtime       = "ffmpeg"
output_label  = "Result"
format        = "image"

[[input]]
name   = "file"
source = "file"
accept = "image/*"
label  = "File"
EOF
fi

# The quoted ('EOF') heredocs above can't expand $slug, so their templates use a
# literal __SLUG__ token — substitute it everywhere in one pass here.
grep -rl '__SLUG__' "$dir" | while read -r f; do sed_inplace "s/__SLUG__/${slug}/g" "$f"; done

# Pin the fresh block's wafer-run deps to wafer-run-pin.txt so its very first
# build resolves the same runtime rev as the committed CLI — an unlocked crate
# resolves wafer-run at today's main, and that rev drift is what breaks
# `wafer build` validation with `__wafer_info()` JSON parse errors. The lock is
# gitignored (block locks stay uncommitted by policy); this only makes local
# builds deterministic. Best-effort: CI re-pins before building either way.
if command -v cargo >/dev/null 2>&1 && [ -f "$root/wafer-run-pin.txt" ]; then
  pin="$(tr -d '[:space:]' < "$root/wafer-run-pin.txt")"
  for c in wafer-sdk wafer-block wafer-block-macro; do
    (cd "$dir" && cargo update -p "$c" --precise "$pin" >/dev/null 2>&1) || true
  done
fi

echo "scaffolded blocks/$slug ($type). Next: implement core/src/lib.rs, src/lib.rs (skill schema), web/src/lib.rs, page/meta.toml, page/content.md."
