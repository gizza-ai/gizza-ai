# `new-tool` Skill (Phase 1) — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A gizza project skill (`/new-tool`) that takes a tool **name + description** and autonomously scaffolds, implements, builds, tests (Playwright page + `gizza` CLI), opens a PR, and code-reviews a new gizza tool.

**Architecture:** A deterministic `scripts/scaffold-tool.sh` generates the name-substituted **boilerplate** (workspace + crate Cargo.tomls, `wafer.toml`, the `core/`+`web/`+`page/` skeleton) from `slug` + `type` (pure|ffmpeg), cloning the *structure* of the reference tools (`calculator` for pure, `image-resize` for ffmpeg — both now exist after Phase 0). The `.claude/skills/new-tool/SKILL.md` playbook then has Claude fill the tool-*specific* files (core logic, the `skill(...)` schema, the web export, `page/meta.toml`+`content.md`) from the description, and run the build → type-aware test → PR → review loop. The skill is proven by using it to build two real tools (a pure one + an ffmpeg one) end-to-end.

**Tech Stack:** Bash (scaffold), Rust (the generated crates), `wasm-bindgen`/`wasm-pack`, `wafer build`, the `gizza-tool-pages` generator, the `gizza` CLI, Playwright. Skill format: a `SKILL.md` with YAML frontmatter (`name`, `description`) + a markdown playbook.

**Spec:** `docs/superpowers/specs/2026-06-17-gizza-new-tool-skill-design.md` (Phase 1).

---

## Reference facts (verified — Phase 0 established these)

- **Reference tools:** `blocks/calculator/` (pure: `core/`+`web/`+`page/`, `core::evaluate`) and `blocks/image-resize/` (ffmpeg: `core::{build_argv,plan_resize,parse_fit,Fit}`, `web::build_argv`, `page/meta.toml` with `runtime="ffmpeg"`, file input, `format="image"`). Each is its own cargo workspace (`[workspace] members=[".","core","web"]`).
- **A new tool is 100% auto-discovered:** `build.rs` scans `blocks/*/target/block.wasm`+`manifest.json`; the generator scans `blocks/*/page/meta.toml`; the CLI reads `block.info().tool`. No registration list anywhere.
- **Build/test commands** (each `blocks/<tool>/` is its own workspace): `cd blocks/<slug> && cargo test --workspace`; `wafer build` is run **from inside** `blocks/<slug>/` (no path arg); `wasm-pack build blocks/<slug>/web --target web --release --out-dir pkg` (from repo root); `cargo run --manifest-path tools/generator/Cargo.toml -- .` renders `pkg/tools/<slug>/`; `solobase build` rebuilds the app+blocks.
- **Playwright** lives in `tests/` (`import from './fixtures'`; `playwright.config.ts` serves `../pkg`; `page.goto('/tools/<slug>/')`).
- **CLI:** `gizza tool <slug> <args>` dispatches the block; ffmpeg tools need system `ffmpeg`; gpu returns `unsupported_in_cli` (exit 3).
- **`gizza-ai-block-utils::ffmpeg`** holds the shared `FfmpegBlock`/`FfmpegService`; ffmpeg skill blocks dispatch via `dispatch_ffmpeg_runtime` (chat) — for the page, the tool's `web/build_argv` (pure, from `core`) is run by `site/tool.js`'s ffmpeg branch.
- **Skill format:** `.claude/skills/<name>/SKILL.md` with frontmatter `name:` + `description:` then the playbook. It is invoked as `/new-tool`. No project skills exist yet — this is the first (`.claude/skills/` must be created).

---

## File structure

```
gizza-ai/
  scripts/scaffold-tool.sh              # CREATE: deterministic boilerplate generator
  scripts/scaffold-tool.test.sh         # CREATE: scaffolds a throwaway slug, asserts files + cargo check, cleans up
  .claude/skills/new-tool/SKILL.md      # CREATE: the autonomous playbook
  .claude/skills/new-tool/reference.md  # CREATE: the per-type file checklist + the exact build/test/PR commands (kept out of SKILL.md to keep it lean)
  # End-to-end proofs create real tools (their own PRs), e.g.:
  blocks/word-count/...                 # CREATE (Task 4 proof, pure)
  blocks/image-grayscale/...            # CREATE (Task 5 proof, ffmpeg)
```

`scaffold-tool.sh` generates only boilerplate; the SKILL.md has Claude write the logic files. Splitting them keeps the error-prone name-substitution deterministic and the judgement-requiring logic with the model.

---

## Task 1: `scaffold-tool.sh` — pure-tool boilerplate

**Files:** Create `scripts/scaffold-tool.sh`

- [ ] **Step 1: Write `scripts/scaffold-tool.sh`** (pure path first; ffmpeg added in Task 2)

```bash
#!/usr/bin/env bash
# Scaffold the name-substituted BOILERPLATE for a new gizza tool. The skill
# (.claude/skills/new-tool/SKILL.md) fills the tool-specific logic afterward.
# Usage: scripts/scaffold-tool.sh <slug> <pure|ffmpeg>
set -euo pipefail

slug="${1:?usage: scaffold-tool.sh <slug> <pure|ffmpeg>}"
type="${2:?usage: scaffold-tool.sh <slug> <pure|ffmpeg>}"
[[ "$slug" =~ ^[a-z][a-z0-9-]*$ ]] || { echo "slug must be kebab-case [a-z0-9-]" >&2; exit 2; }
[[ "$type" == pure || "$type" == ffmpeg ]] || { echo "type must be pure|ffmpeg" >&2; exit 2; }
root="$(cd "$(dirname "$0")/.." && pwd)"
dir="$root/blocks/$slug"
[[ -e "$dir" ]] && { echo "blocks/$slug already exists — refusing to overwrite" >&2; exit 2; }

under="${slug//-/_}"                 # kebab -> snake for crate/wasm idents
crate="gizza-ai-$slug"               # crate name stem
ucrate="gizza_ai_${under}"           # underscore form for wasm export basenames

mkdir -p "$dir/core/src" "$dir/web/src" "$dir/src" "$dir/page" "$dir/tests"

# --- root workspace + block crate ---
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
serde = { version = "1", features = ["derive"] }
serde_json = "1"
EOF

# --- core (pure logic) ---
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

# --- web (wasm-bindgen wrapper) ---
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

# --- chat skill block ---
cat > "$dir/src/lib.rs" <<EOF
//! gizza-ai/$slug — chat skill block (thin wrapper around core). The new-tool
//! skill replaces the skill(description, parameters) schema + Args + delegation.
#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]
use serde::Deserialize;
use wafer_sdk::*;

#[derive(Deserialize)]
struct Args { input: String }

#[cfg(target_arch = "wasm32")]
struct Tool;

#[cfg(target_arch = "wasm32")]
#[wafer_block(
    name = "gizza-ai/$slug",
    version = "0.1.0",
    interface = "handler@v1",
    summary = "$slug skill",
    skill(
        description = "TODO: describe what this tool does and its inputs.",
        parameters = r#"{ "type": "object", "properties": { "input": { "type": "string" } }, "required": ["input"], "additionalProperties": false }"#
    ),
)]
impl Tool {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        let args: Args = match serde_json::from_slice(&body) {
            Ok(a) => a,
            Err(e) => return respond_error(format!("invalid args: {e}")),
        };
        match ${ucrate}_core::run(&args.input) {
            Ok(v) => GuestResult::respond(serde_json::to_vec(&serde_json::json!({ "result": v })).unwrap_or_default()),
            Err(e) => respond_error(e),
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn respond_error(msg: String) -> GuestResult {
    GuestResult::respond(serde_json::to_vec(&serde_json::json!({ "error": msg })).unwrap_or_default())
}
EOF

# --- wafer.toml ---
cat > "$dir/wafer.toml" <<EOF
[package]
org = "gizza-ai"
name = "$slug"
version = "0.1.0"
abi = 1
summary = "TODO: one-line summary."
EOF

# --- page (pure default; ffmpeg overrides in Task 2) ---
cat > "$dir/page/meta.toml" <<EOF
slug          = "$slug"
title         = "TODO — gizza.ai"
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
EOF

echo "scaffolded blocks/$slug ($type). Next: implement core/src/lib.rs, src/lib.rs (skill schema), web/src/lib.rs, page/meta.toml, page/content.md."
```

- [ ] **Step 2: chmod + smoke** — `chmod +x scripts/scaffold-tool.sh && scripts/scaffold-tool.sh zzscratch pure && ls blocks/zzscratch && rm -rf blocks/zzscratch`
Expected: prints the scaffold message; the dir tree exists; cleanup removes it. (The slug must be valid kebab — start with a-z — so use a throwaway like `zzscratch`, not a `_`-prefixed name.)

- [ ] **Step 3: Commit** — `git add scripts/scaffold-tool.sh && git commit -m "feat(skill): scaffold-tool.sh — pure-tool boilerplate generator"`

## Task 2: `scaffold-tool.sh` — ffmpeg branch + the verification test

**Files:** Modify `scripts/scaffold-tool.sh`; Create `scripts/scaffold-tool.test.sh`

- [ ] **Step 1:** In `scaffold-tool.sh`, after the pure `page/meta.toml`/`content.md` heredocs, branch on `$type == ffmpeg` to OVERWRITE the web wrapper + page for the ffmpeg shape (mirroring `image-resize`):

```bash
if [[ "$type" == ffmpeg ]]; then
  cat > "$dir/web/src/lib.rs" <<EOF
//! Browser-facing wasm-bindgen wrapper for /tools/$slug/ (ffmpeg page).
//! Builds the ffmpeg argv (pure, shared with the chat block via core).
use serde::Serialize;
use wasm_bindgen::prelude::*;

#[derive(Serialize)]
struct Plan { argv: Vec<String>, out_name: String }

#[wasm_bindgen]
pub fn build_argv(in_name: &str) -> Result<JsValue, JsValue> {
    // The new-tool skill replaces this with the tool's real param signature.
    let (argv, out_name) = ${ucrate}_core::plan(in_name).map_err(|e| JsValue::from_str(&e))?;
    serde_wasm_bindgen::to_value(&Plan { argv, out_name }).map_err(|e| JsValue::from_str(&e.to_string()))
}
EOF
  # ffmpeg web needs serde + serde-wasm-bindgen
  sed -i 's#wasm-bindgen = "0.2"#wasm-bindgen = "0.2"\nserde = { version = "1", features = ["derive"] }\nserde-wasm-bindgen = "0.6"#' "$dir/web/Cargo.toml"
  # core stub for ffmpeg returns (argv, out_name)
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
title         = "TODO — gizza.ai"
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
```

- [ ] **Step 2: Write `scripts/scaffold-tool.test.sh`** (the verification — scaffold both types, cargo-check, clean up):

```bash
#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "$0")/.." && pwd)"
cleanup() { rm -rf "$root/blocks/zzscratch-pure" "$root/blocks/zzscratch-ff"; }
trap cleanup EXIT
"$root/scripts/scaffold-tool.sh" zzscratch-pure pure
"$root/scripts/scaffold-tool.sh" zzscratch-ff ffmpeg
# The stubs return Err("not implemented") but must COMPILE (workspace check).
(cd "$root/blocks/zzscratch-pure" && cargo check --workspace)
(cd "$root/blocks/zzscratch-ff" && cargo check --workspace)
echo "scaffold-tool.test.sh OK"
```
(Throwaway slugs are valid kebab — `zzscratch-pure`/`zzscratch-ff` — and the `trap` cleans them so `blocks/` stays clean.)

- [ ] **Step 3: Run it** — `chmod +x scripts/scaffold-tool.test.sh && scripts/scaffold-tool.test.sh`
Expected: `scaffold-tool.test.sh OK` — both scaffolds compile (the stubs are valid Rust). Cleanup leaves `blocks/` unchanged (`git status` clean).

- [ ] **Step 4: Commit** — `git add scripts/scaffold-tool.sh scripts/scaffold-tool.test.sh && git commit -m "feat(skill): scaffold ffmpeg tools + a cargo-check verification test"`

## Task 3: the `new-tool` SKILL.md playbook

**Files:** Create `.claude/skills/new-tool/SKILL.md`, `.claude/skills/new-tool/reference.md`

- [ ] **Step 1: Create `.claude/skills/new-tool/SKILL.md`** — frontmatter + the autonomous playbook. Full content:

```markdown
---
name: new-tool
description: "Use when the user wants to add a new gizza tool (a calculator/clock-style pure-compute tool, or an image/video ffmpeg tool). Takes a name + description and autonomously scaffolds, implements, builds, tests (Playwright page + gizza CLI), opens a PR, and code-reviews it."
---

# new-tool — build a gizza tool end to end

Autonomous: from a tool **name + description**, ship a new gizza tool. No mid-way
confirmation — infer the schema/behavior, DOCUMENT assumptions in the PR, and rely
on the build/test loop + the code review as the safety nets. NEVER claim a step
passed that you didn't run; if a tool type can't be headlessly verified, say so.

Read `reference.md` (next to this file) for the exact per-type file contents +
the build/test/PR commands. Follow these steps in order:

1. **Gather** the tool `name` (→ `slug`, kebab-case) and `description`. If not
   supplied with the invocation, ask for them (the ONLY question allowed).
2. **Branch:** `git checkout -b feat/tool-<slug>` from `main`.
3. **Classify** the type from the description:
   - **pure** — typed input → typed output, deterministic, no I/O (math, text, time, conversion).
   - **ffmpeg** — image/video transform (resize, crop, convert, grayscale, trim, transcode).
   - **network** — fetches a URL (treat as a chat-only block; build it, CLI-test it, no page).
   - **gpu** — needs WebGPU/a model (imagine-style). Build the chat block ONLY; do NOT
     build a page (no headless GPU); CLI is expected to be `unsupported_in_cli`.
4. **Derive + record assumptions:** the input schema (param names + JSON-Schema types),
   the output, the behavior. These go verbatim into the PR body.
5. **Scaffold:** `scripts/scaffold-tool.sh <slug> <pure|ffmpeg>` (for network/gpu, copy
   `blocks/web-fetch`/`blocks/imagine` as the reference instead — they have no page).
6. **Implement** (replace the `TODO`/`not implemented` stubs):
   - `core/src/lib.rs` — the pure logic (+ ≥1 happy-path & ≥1 error unit test). Use
     `f64` not `i64` for numeric params (the wasm BigInt gotcha).
   - `src/lib.rs` — the real `skill(description, parameters)` schema + `Args` + delegation.
   - `web/src/lib.rs` — the export (pure: `run`; ffmpeg: `build_argv` returning `{argv,out_name}`).
   - `page/meta.toml` + `page/content.md` — real title/desc/tags/inputs (field ORDER must
     match the `build_argv` param order for ffmpeg); + `tests/*.json` wafer fixtures.
7. **Build:** `wafer build` (from `blocks/<slug>/`); `cargo test --workspace` (from
   `blocks/<slug>/`); `wasm-pack build blocks/<slug>/web --target web --release --out-dir pkg`;
   `cargo run --manifest-path tools/generator/Cargo.toml -- .`; `solobase build`. Fix
   compile/test failures (≤3 attempts) before escalating.
8. **Test (type-aware)** — see reference.md:
   - unit (always) + wafer fixtures (always);
   - **Playwright** the page (pure + ffmpeg): add a spec in `tests/` driving `/tools/<slug>/`;
   - **CLI** (pure/ffmpeg/network): `cargo install --path cli --force` then `gizza tool <slug> …`;
     gpu: assert `unsupported_in_cli` + exit 3.
9. **Ship:** commit, `git push -u origin feat/tool-<slug>`, `gh pr create` with a body that
   states: the tool type, the derived assumptions, what was tested + results, and any
   limitation (e.g. "gpu: no page, no headless verification"; "ffmpeg: chat path is
   non-functional — runs only on the standalone page / CLI, see the SW-ffmpeg note").
10. **Code review:** run `/code-review` on the diff and post the findings as a PR comment.
    Do NOT merge.

**Honesty gate:** if a build/test fails unrecoverably, STOP and report the failure with
the error — do not open a "done" PR for a broken tool. If the description is too vague to
implement correctly, make your best documented guess and flag it prominently in the PR.

**Known constraint (record in the PR for ffmpeg tools):** the CHAT runtime runs in a
Service Worker where ffmpeg cannot run (`import()`/`Worker` are SW-forbidden), so ffmpeg
tools work via their **standalone page** + the **CLI**, not in-chat. The page is the
supported surface.
```

- [ ] **Step 2: Create `.claude/skills/new-tool/reference.md`** — the per-type file checklist + exact commands, copied from this plan's "Reference facts" + the `image-resize`/`calculator` file shapes (so the skill body stays lean). Include: the build/test command sequence, the Playwright spec template (`import from './fixtures'`, `page.goto('/tools/<slug>/')`, the pure assertion `#tool-output` text and the ffmpeg assertion `#tool-output-media` `data:` src), the wafer-fixture byte-array recipe (`python3 -c "import json;print(list(json.dumps({...}).encode()))"`), and the field-order/`f64` gotchas.

- [ ] **Step 3: Commit** — `git add .claude/skills/new-tool && git commit -m "feat(skill): new-tool SKILL.md playbook + reference"`

## Task 4: end-to-end proof — a pure tool (`word-count`)

**Files:** Create `blocks/word-count/...`, `tests/tool-page-word-count.spec.ts` (via the skill)

- [ ] **Step 1: Run the skill** for name="Word Count", description="Count the words, characters, and lines in a block of text." Follow SKILL.md exactly (scaffold pure → implement `core::counts` returning a summary string / JSON → skill schema `{text}` → web `run(text)` → page meta `format="text"`, one field input `text` → content.md → fixtures).
- [ ] **Step 2: Build** per SKILL.md step 7. Expected: green.
- [ ] **Step 3: Unit + Playwright + CLI:**
  - `cd blocks/word-count && cargo test --workspace` → PASS.
  - Add `tests/tool-page-word-count.spec.ts`: `goto('/tools/word-count/')`, fill `#in-text` with "a b c", assert `#tool-output` shows the count. Run via the `tests/` harness → PASS.
  - `gizza tool word-count "one two three"` → the count. 
- [ ] **Step 4: PR + review** per SKILL.md steps 9–10 (its OWN branch/PR, separate from the skill's branch).
- [ ] **Step 5: Record** the result here (the proof that the skill produces a working pure tool end-to-end).

## Task 5: end-to-end proof — an ffmpeg tool (`image-grayscale`)

**Files:** Create `blocks/image-grayscale/...`, `tests/tool-page-image-grayscale.spec.ts` (via the skill)

- [ ] **Step 1: Run the skill** for name="Image Grayscale", description="Convert an image to grayscale." → type ffmpeg → scaffold ffmpeg → `core::plan(in_name)` returns argv `["-i", in, "-vf", "format=gray", out]` + `out_name` (keep ext) → web `build_argv(in_name)` → page meta `runtime="ffmpeg"`, file input, `format="image"`.
- [ ] **Step 2: Build** per SKILL.md. Expected green; `pkg/tools/image-grayscale/` rendered.
- [ ] **Step 3: Playwright + CLI:**
  - `tests/tool-page-image-grayscale.spec.ts`: upload `tests/fixtures/red-2x2.png`, assert `#tool-output-media` gets a `data:image/` src. Run → PASS (needs @ffmpeg CDN network).
  - `gizza tool image-grayscale url=<...>` (with system ffmpeg) — confirm it dispatches cleanly.
- [ ] **Step 4: PR + review** (own branch/PR).
- [ ] **Step 5: Record** the result (the proof the skill produces a working ffmpeg tool, page + CLI).

---

## Self-review notes

- **Spec coverage:** §form/location→Tasks 1–3; §scaffold→Tasks 1–2; §autonomous procedure→Task 3 SKILL.md; §per-type test strategy→SKILL.md step 8 + Tasks 4–5; §honesty rules→SKILL.md "Honesty gate"; §PR+review→SKILL.md steps 9–10. The two proofs (Tasks 4–5) cover the pure + ffmpeg tiers; network/gpu are documented in the SKILL.md but not separately proven (they reuse the chat-only path — acceptable per spec, noted).
- **Reality update folded in:** the SKILL.md records the Phase-0 finding that **chat ffmpeg is non-functional** (SW limitation), so ffmpeg tools are shipped via page + CLI and the PR says so.
- **Scaffold is deterministic + cargo-check-tested** (Task 2 test); the logic is LLM-authored per SKILL.md. No sed-across-prose fragility — the script writes whole files from `slug`.
- **Tasks 4–5 are end-to-end proofs run by following the SKILL.md** — they are the real "tests" of the skill (it produces working tools). Each opens its own PR (the skill's deliverable).
- **Follow-up (separate):** the chat-ffmpeg page-side bridge (its own brainstorm/spec/plan) — out of scope here.
```
