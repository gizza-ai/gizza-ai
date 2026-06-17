# Design: `new-tool` skill — scaffold, build, test, and ship a new gizza tool

**Date:** 2026-06-17
**Repo:** gizza-ai
**Status:** approved design, pending implementation plan

## Motivation

Adding a gizza tool today is a multi-file, multi-step ritual: a 3-crate block
(`core/` + chat-skill + `web/`), a `page/` (meta + content), test fixtures, then
build + page-generation + manual browser checks + a PR. It's mechanical but
error-prone, and the steps differ by tool type. We want a **skill** that takes a
**name + description** and autonomously: scaffolds the block, implements the
logic, builds it, tests it (Playwright on the page + the `gizza` CLI), opens a PR,
and runs a code review — so shipping a new tool is one invocation.

The user chose **fully autonomous** (no mid-way checkpoints) and **any tool**
(including ffmpeg/image/video). Because there is no confirmation step, the skill
must infer the input schema + behavior from the description, **document its
assumptions in the PR**, and lean on the build/test loop + the final code review
as the safety nets — never claiming success a test didn't prove.

## Tool-type taxonomy (drives everything)

gizza tools fall into four types. The skill classifies the tool from its
description, then picks the template, host wiring, page, and test strategy:

| Type | Example | Block structure | Standalone page | Playwright | CLI test |
|---|---|---|---|---|---|
| **pure-compute** | calculator, unit-convert | `core/` + chat-skill + `web/` | ✅ | ✅ input→output | ✅ |
| **ffmpeg** | image-resize, video-trim | `core/` (argv builder) + chat-skill + `web/` | ✅ (after Phase 0) | ✅ upload→output | ✅ (needs system `ffmpeg`) |
| **network** | web-fetch | chat-skill (`requires` network) | ⚠️ optional | ⚠️ if page | ✅ |
| **gpu** | imagine | chat-skill (`requires` image svc) | ❌ default off | ❌ no WebGPU headless | ❌ `unsupported_in_cli` |

**pure-compute** and **ffmpeg** are the two fully-supported, fully-tested tiers.
**network** is supported (CLI-tested; a page is optional). **gpu** builds the chat
block only by default; a page is buildable but cannot be verified headlessly (no
WebGPU in CI), so the skill does not auto-build one — it records the limitation.

This requires a one-time **Phase 0** (media-I/O page extension) so ffmpeg tools
can have real, Playwright-testable pages. Phase 1 is the skill itself.

---

## Phase 0 — media-I/O page extension (prerequisite)

Today the standalone-page system (`tools/generator/`, `site/tool.js`, `page/meta.toml`)
only handles **pure text→number/text compute**: a `web/` wasm exposes
`fn(args) -> T`, and `tool.js` wires text fields / a clock source to it. ffmpeg
tools need **file input** and **media output**, plus the browser ffmpeg bridge.

The enabling fact: the chat's browser ffmpeg (`js/ffmpeg.js` → `ffmpegExec(argsJson,
inputsJson, outputName)`) loads **`@ffmpeg/core@0.12.10`'s single-threaded UMD
build** — **no `SharedArrayBuffer`, no COOP/COEP headers** (none exist in
`pkg/_headers`), and it **runs in headless Chromium**. So an ffmpeg page is
header-free and Playwright-testable.

### Phase 0 change set

1. **`page/meta.toml` schema** (parsed by `tools/generator/src/meta.rs`):
   - `runtime = "wasm" | "ffmpeg"` (default `"wasm"` — the current pure path).
   - `[[input]] source = "file"` with `accept = "image/*"` (or `"video/*"`) — a file picker.
   - `format = "number" | "text" | "image" | "video"` — output rendering.

2. **`web/src/lib.rs` for ffmpeg tools** exposes a **pure argv builder**, not the
   ffmpeg run: `#[wasm_bindgen] pub fn build_argv(<params>, in_name: &str) -> JsValue`
   returning `{ argv: string[], out_name: string }`. The argv logic lives in
   `core/` (shared with the chat block — single source of truth). The page never
   re-implements the ffmpeg invocation.

3. **`site/tool.js`** gains an ffmpeg path (gated on `cfg.runtime === "ffmpeg"`):
   - read the `file` input bytes + the `field` params,
   - call the web-wasm `build_argv(params, inName)` → `{argv, outName}`,
   - call `ffmpegExec(JSON.stringify(argv), JSON.stringify([{name: inName, bytes_b64}]), outName)`
     (the existing bridge; inputs are base64),
   - decode `output_b64` → render as `<img>`/`<video>` (a data URL) + a download link.
   Errors (non-zero exit, bad file) render in the output area like the pure path.

4. **The generator** (`tools/generator/`) copies `js/ffmpeg.js` into the page dir
   for `runtime="ffmpeg"` tools, and the template emits a `<input type="file">`
   for `source="file"` inputs and an `<img id="tool-output">`/download for media
   `format`. `window.GIZZA_TOOL` carries `runtime`, the file input ids, and the
   output format.

5. **Proof of Phase 0 — give `image-resize` a real page.** As the reference
   ffmpeg tool: extract its `build_argv`/validation into `blocks/image-resize/core/`,
   add `blocks/image-resize/web/` (exposing `build_argv`) + `blocks/image-resize/page/`
   (meta with a `file` input + `image` output + `runtime="ffmpeg"`). The chat
   block keeps working (it now calls `core::build_argv`); the new page works via
   the file path. A Playwright test uploads a tiny PNG and asserts a resized
   `<img>` appears. This validates the extension end-to-end and is the template
   the skill clones for new ffmpeg tools.

> Phase 0 is independently shippable (it adds a feature + gives image-resize a
> page) and is verified before Phase 1 depends on it.

---

## Phase 1 — the `new-tool` skill

### Form & location

- **`gizza-ai/.claude/skills/new-tool/SKILL.md`** — the playbook Claude follows.
- **`scripts/scaffold-tool.sh`** (or a small Rust bin under `tools/`) — deterministic
  boilerplate generation: given `slug`, `type`, it copies the matching template and
  substitutes names, so the ~8 near-identical files are never hand-retyped.
- **`blocks/_template/{pure,ffmpeg}/`** — reference skeletons the scaffold copies
  (the boilerplate parts of calculator / image-resize with `__SLUG__` placeholders).

The skill is a **gizza project skill** (it builds gizza tools); the user invokes
it and supplies **name + description**.

### The autonomous procedure (the SKILL.md steps)

1. **Gather** the tool `name` (→ `slug`, kebab-case) + `description`. (Prompt if
   not supplied with the invocation.)
2. **Classify** the type (pure-compute / ffmpeg / network / gpu) from the
   description, and **derive** the input schema (param names + JSON-Schema types),
   the output shape, and the compute behavior. Record these as **explicit
   assumptions** (they go in the PR body — there is no confirmation step).
3. **Scaffold** via `scripts/scaffold-tool.sh <slug> <type>` → the boilerplate
   files with names substituted.
4. **Implement** the tool-specific parts (Claude authors):
   - `core/src/lib.rs` — the pure logic (+ unit tests: ≥1 happy-path, ≥1 error).
   - `src/lib.rs` — the `#[wafer_block(skill(description, parameters))]` schema +
     `Args` + delegation to `core`.
   - `web/src/lib.rs` — the wasm-bindgen export (pure path) or `build_argv`
     (ffmpeg). **Use `f64` not `i64`** for numeric params (the BigInt gotcha).
   - `page/meta.toml` + `page/content.md` (for tiers with a page).
   - `tests/*.json` fixtures (UTF-8 JSON encoded as byte arrays — generated with
     `python3 -c "import json;print(list(json.dumps({...}).encode()))"`).
5. **Build**: `wafer build blocks/<slug>` → `cargo test` (core) →
   `wasm-pack build blocks/<slug>/web --target web --release --out-dir pkg` →
   `cargo run --manifest-path tools/generator/Cargo.toml -- .` → `solobase build`.
   Iterate on compile/test failures (bounded, ~3 attempts) before escalating.
6. **Test (type-aware)** — see the matrix below.
7. **Ship**: branch `feat/tool-<slug>` → commit → push → `gh pr create` with a body
   that states the **derived assumptions, the tool type, what was tested, and any
   limitation** (e.g. "gpu: page not built — no headless WebGPU").
8. **Code review**: run `/code-review` on the diff (or dispatch a review agent) and
   **post the findings as a PR comment / summary**. Do not auto-merge.

### Test strategy per type

- **Unit (always):** `cd blocks/<slug>/core && cargo test` — the pure logic.
- **Block fixtures (always):** `wafer test blocks/<slug>` against `tests/*.json`.
- **Playwright (pure-compute + ffmpeg):** `solobase build` → serve `pkg/` on a
  fresh port → drive `gizza.ai/tools/<slug>/` (or localhost): fill `#in-<name>` /
  upload a fixture file → assert `#tool-output` shows the expected value / an
  `<img>` renders. Reuse the session's Playwright pattern (navigate → wait for SW
  → assert). **Skipped for network-without-page and gpu.**
- **CLI (pure-compute + ffmpeg + network):** install/rebuild `gizza`, run
  `gizza tool <slug> <args>` and assert the result. **ffmpeg** needs system
  `ffmpeg` (the test gates on its presence). **gpu** is expected to return
  `unsupported_in_cli` (exit 3) — the skill asserts *that*, not a real result.

### Failure handling & honesty (non-negotiable)

- If a build or test fails, the skill debugs and retries (bounded). If it still
  fails, it **stops and reports the failure with the error** — it does NOT open a
  "done" PR for a broken tool.
- It **never claims a step passed that it didn't run** (e.g. gpu Playwright). The
  PR body states exactly what was and wasn't verified, and why.
- If the description is too ambiguous to author correct logic, the skill makes its
  best documented guess and flags it prominently in the PR for human review (the
  autonomy choice trades a confirmation step for explicit PR-time disclosure).

### Code review step

Use the existing `/code-review` skill against the PR's diff (the branch vs `main`),
at a sensible effort, and post the findings as a PR comment (or a summary in the
skill's final report). The review is advisory — the human merges.

---

## Testing the skill itself

- **Phase 0** is proven by the `image-resize` page + its Playwright test.
- **Phase 1** is proven by running the skill end-to-end for one tool of each
  testable tier and confirming a green PR:
  - pure-compute: e.g. a `unit-convert` or `word-count` tool → page + CLI green.
  - ffmpeg: e.g. an `image-grayscale` tool → page (upload→output) + CLI green.
  - gpu: confirm it builds the block, CLI returns `unsupported_in_cli`, and the PR
    honestly notes "no page / no headless verification."
- The skill's own scaffold script gets a smoke test (scaffold a throwaway slug →
  the files exist + compile), and `scripts/scaffold-tool.sh` is idempotent/safe
  (refuses to overwrite an existing `blocks/<slug>/`).

## Non-goals (YAGNI)

- No registry/marketplace, no per-tool routing changes (auto-discovery already
  covers new tools).
- No multi-threaded ffmpeg / COOP-COEP (the single-threaded core is enough and
  header-free).
- No auto-merge — the skill stops at "PR opened + reviewed."
- gpu standalone pages are not built by default (unverifiable headless).
- The skill does not invent host capabilities — a tool needing a service that
  doesn't exist yet is out of scope (it reports that, doesn't fabricate one).

## Open implementation details (resolve in the plan)

- Scaffold as a bash script vs a small Rust `tools/scaffold` bin — pick whichever
  is cleaner to keep the template substitution robust (prefer the Rust bin if the
  substitution gets fiddly; bash is fine for simple name swaps).
- Exact `window.GIZZA_TOOL` shape additions for `runtime`/file-inputs/media-output,
  and whether `tool.js` forks into `tool-media.js` or branches internally (prefer
  one file with a branch unless it grows unwieldy).
- How the Playwright media assertion fetches a fixture image (bundle a 1×1 PNG in
  the repo for tests).
- Whether `_template/` blocks are excluded from `build.rs`/the generator scan (they
  must not be built as real tools — gate on a missing `page/` or a `_`-prefix skip).

## Change-set summary

**Phase 0:** extend `meta.rs`/`template.rs`/generator + `site/tool.js` for
file-input + media-output + `ffmpeg` runtime; refactor `image-resize` to a
`core/`+`web/`+`page/` shape; add an `image-resize` Playwright test. **Phase 1:**
`.claude/skills/new-tool/SKILL.md`, `scripts/scaffold-tool.sh`, `blocks/_template/`,
and the skill's end-to-end proofs.
