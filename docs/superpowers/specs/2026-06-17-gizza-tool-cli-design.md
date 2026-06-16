# Design: `gizza` tool CLI — a headless front-end onto the skill runtime

**Date:** 2026-06-17
**Repo:** gizza-ai (no wafer-run or solobase change required)
**Status:** approved design, pending implementation plan

## Motivation

Every gizza tool (`calculator`, `clock`, `image-resize`, `web-fetch`, …) is already a
self-contained **skill**: a `#[wafer_block]` that declares a JSON-schema `parameters`
and a `handle(args_json) -> result_json` (or a `{_for_llm, _for_ui}` envelope). The chat
app dispatches LLM tool-calls to these blocks through one path
(`src/blocks/agent/dispatch.rs::run_skill_dispatch` → `ctx.call_block_buffered`).

We want those same tools usable **from a terminal and by an LLM agent via `SKILL.md`**,
e.g.:

```
gizza tool calculator "2*2"        # → 4
gizza tool clock "%H:%M"           # → 14:07
gizza tool image-resize url=https://… width=640 --out cat.png
```

The hard requirement (from the requester): **a single source of truth, no second
implementation of any tool, and no splitting the tools into "CLI-able" vs "not".**
Tools that cannot run in a headless process return a clean "not available here" error
through the *same* path rather than being special-cased out.

## The core insight — the CLI is a third host environment, not a second system

The tools' logic, agent contract, and embedding already exist as build artifacts:

| Concern | Single source of truth (existing artifact) |
|---|---|
| Tool **logic** | `blocks/<tool>/target/block.wasm` — the exact wasm the chat loads |
| Agent **contract** (description + JSON schema) | `blocks/<tool>/manifest.json` → `{ "role": "skill", "tool": { "description", "parameters" } }` — the same schema the chat sends to the LLM |
| **Embedding** | `build.rs` already emits `SKILLS: &[(&str, &[u8])]` from `blocks/*/target/block.wasm` + `manifest.json` |

The only environment-specific parts are the **injected host services**. This is an
existing pattern: the `FfmpegService` trait (`src/ffmpeg.rs:46`) already has a browser
impl (`BrowserFfmpegService`, wasm32) and a test impl (`FakeFfmpegService`); the
`FfmpegBlock` (`src/blocks/ffmpeg.rs:19-27`) holds an `Arc<dyn FfmpegService>` and the
image/video skill blocks call it identically regardless of which service backs it. The
CLI is simply a **third sibling environment** — native — that injects native services.

Feasibility was verified against wafer-run source:

- **Native wasmi hosting works.** `WasmiBlock::load_from_bytes()` is gated on the `wasmi`
  cargo feature, *not* on `target_arch="wasm32"`; native `#[tokio::test]`s load and call
  wasm blocks today (`wafer-run/crates/wafer-run/tests/wasmi_block_test.rs`,
  `service_client_e2e.rs`).
- **Async host imports resolve on native tokio** — the wasmi resumable-call + `Context`
  dispatch has no JS/browser dependency (`service_client_e2e.rs` proves a wasm guest's
  async host call resolving on native tokio).
- **Native network is already shipped** — `wafer_block_network::HttpNetworkService`
  (SSRF-guarded, no target gating) is what solobase-native registers via
  `make_fetch_network_service()`.

The only missing piece is a **new native binary entry point**. Gizza is the first
wasmi-skill host; today only `initialize()` (wasm32, `src/lib.rs`) loads these blocks.

## Goals

- A native `gizza` binary with a `tool` subcommand: `gizza tool <name> [args…]`,
  `gizza tool list`, `gizza tool describe <name>`.
- Tool calls dispatch through the **same** runtime + `call_block_buffered` + envelope
  contract the chat uses; the skill-block wasm is the only source of tool logic.
- Every tool is reachable through one path. Tools with no native host backing (e.g.
  `imagine`/WebGPU) return a clean, typed "not available in the CLI" error from a stub
  service — no separate code branch, exit code non-zero.
- A machine-readable contract (`--json`) and a generated `SKILL.md` so an LLM agent or a
  third-party program can drive it without reading source.
- Offline by default for pure-compute tools (`calculator`, `clock`): no network, no
  external binary.

## Non-goals (YAGNI)

- **No native re-implementation of any tool.** The CLI never links a tool's `core/`
  crate directly; it runs the wasm. (Pure tools *could* be linked directly, but that
  would create the second code path the requester explicitly vetoed.)
- **No long-lived process / server / REPL.** One invocation = one tool call, then exit.
  No cross-invocation attachment `ref` store (see "ref inputs" below).
- **No new tool.** This is a front-end; tools are added the existing way (`blocks/<tool>/`).
- **No `imagine` backend.** It is stubbed; wiring a native T2I model is out of scope.
- **No changes to the chat app, the web pages, or how blocks are built.**

## Architecture — one runtime, one dispatch, same artifacts

```
gizza tool <name> [args…]
  1. resolve <name> → "gizza-ai/<name>"
  2. boot native Wafer (wasmi) ONCE per invocation:
       - register each embedded skill block as a WasmiBlock (same SKILLS table)
       - register native host services: HttpNetworkService, NativeFfmpegService,
         and stub services for unsupported capabilities (imagine)
  3. map args → JSON body          (schema-driven, from manifest.json tool.parameters)
  4. ctx.call_block_buffered("gizza-ai/<name>", msg, body) → response bytes
  5. parse_skill_response(bytes) → { for_llm, for_ui }      (same envelope contract)
  6. render:
       default     → print for_llm (human text)
       --json      → print the full { _for_llm, _for_ui } envelope
       _for_ui.data_url present → decode base64, write to --out <file> (or cwd)
  7. exit 0 on success; non-zero on tool error / unsupported / bad args
```

The tools cannot tell they are in a CLI: the message they receive
(`msg.kind`, `META_REQ_ACTION="create"`, `META_REQ_RESOURCE="/b/<name>"`) mirrors the
chat dispatch (`src/blocks/agent/dispatch.rs:98-102`).

## Where it lives + the one bit of restructuring

### New crate, not a `src/bin/`

The CLI is a **new crate in the gizza-ai workspace** (`cli/`), *not* a `src/bin/` inside
the app crate. The app crate (`gizza-ai`) is wasm-first (`crate-type = ["cdylib",
"rlib"]`) and depends on browser-only `solobase-browser`; it cannot compile to a native
binary. The `cli/` crate depends instead on:

- `wafer-run` (native, `features = ["wasm"]` → wasmi + reqwest + tokio) and `wafer-block`
- `wafer-block-network` (`HttpNetworkService`)
- the relocated host-agnostic runtime pieces (below)
- its own `build.rs` reusing the **exact** `SKILLS` embedding logic from the app's
  `build.rs` (factored into a shared build helper so the two never diverge)

### Relocate the host-agnostic bridge into a native-compilable crate (root-cause, not a shim)

`FfmpegBlock`, the `FfmpegService` trait, and `ExecArgs`/`ExecResult` currently live
inside the browser-first app crate (`src/ffmpeg.rs`, `src/blocks/ffmpeg.rs`). The
`FfmpegBlock` struct and the trait are already native-compilable (only
`BrowserFfmpegService` is wasm32-gated). They move into a **shared native-compilable
crate** (extend `block-utils`, or a new `gizza-ffmpeg-runtime` crate) so that:

- the **app** keeps registering `FfmpegBlock::new(Arc::new(BrowserFfmpegService))`,
- the **CLI** registers `FfmpegBlock::new(Arc::new(NativeFfmpegService))`,

against the **identical** block + trait. This is the existing browser/test/native
service pattern made reusable — not duplicated code. The same treatment applies to any
other native bridge block the tools depend on (e.g. an `imagine`-runtime bridge), which
the CLI backs with a stub service.

## Host services — and "stub, don't split" in practice

| Capability | Browser today | CLI (new) | Tools unblocked |
|---|---|---|---|
| network fetch | `BrowserNetworkService` | `wafer_block_network::HttpNetworkService` (native, SSRF-guarded) | `web-fetch`, `image-fetch`, `image-*` (url input) |
| ffmpeg | `BrowserFfmpegService` (JS) | `NativeFfmpegService` → shells out to system `ffmpeg` | `image-*`, `video-*`, `ffmpeg` |
| pure compute | (none) | (none needed) | `calculator`, `clock` |
| T2I / WebGPU | browser GPU | **stub service** → typed "not available in the CLI" error | `imagine` |

`NativeFfmpegService` (v1) shells out to the system `ffmpeg` binary: write `ExecArgs.inputs`
to a temp dir, run `ffmpeg <args>` with the temp paths, read back `ExecArgs.output`,
capture exit code + stderr into `ExecResult`. If `ffmpeg` is not on `PATH`, return a
clear error naming the missing dependency (non-zero exit). The image/video skill blocks
are unchanged — they still call `gizza-ai/ffmpeg-runtime`.

The stub services return a stable, typed error (e.g.
`{"error":"unsupported_in_cli","message":"imagine requires a browser GPU; use the web app at gizza.ai"}`)
so agents and scripts can branch on it. Every tool dispatches through the same path.

## The arg + output contract

### Input — schema-driven, three layers, single source = `manifest.json tool.parameters`

1. **Positional** — when the schema's `required` properties are scalar (string/number),
   positional args fill them in `required` order. Drives the headline UX:
   `gizza tool calculator "2*2"` → `{"expr":"2*2"}`,
   `gizza tool clock "%H:%M"` → `{"format":"%H:%M"}`.
2. **`key=value`** — for multi-field / optional params:
   `gizza tool image-resize url=https://… width=640 fit=cover`. Values are coerced to the
   schema's declared JSON type (integer/number/boolean/string).
3. **`--json '{…}'`** — full escape hatch supplying the entire body. Mutually exclusive
   with positional/`key=value` (giving both is a usage error, exit 2), so there is no
   merge-precedence ambiguity.

Args are validated against the schema *before* dispatch where cheap (required-present,
type-coercible); deeper validation stays in the block (its existing `InvalidArgs`).

### Output

- **default** → `for_llm` as plain text on stdout.
- **`--json`** → the full `{ "_for_llm", "_for_ui"? }` envelope on stdout (one JSON object,
  newline-terminated) — the integration contract for agents/programs.
- **binary results** → when `_for_ui.data_url` is a `data:` URL, decode the base64 and
  write the bytes to `--out <file>` (default: a name derived from `_for_ui.filename` in
  the cwd). stdout then prints the `for_llm` summary (or, with `--json`, the envelope with
  `data_url` replaced by the written path to avoid multi-MB stdout).
- **exit codes** — `0` success; `1` tool error (`tool_failed`/envelope error); `2` bad CLI
  usage (unknown tool, bad args); `3` unsupported-in-CLI (stub). Distinct codes let
  callers branch.

### `ref` inputs

Skill blocks accept `{ref: "upload_N"}` to reference a prior in-chat upload
(`dispatch.rs:73-93`). The CLI has no chat history, so **`ref` is not supported in v1**;
instead the CLI accepts a local file via `file=<path>` / stdin for tools that take a
`SourceFields` (url-or-ref) input, materializing it as the URL/bytes source the block
already understands. (Detail to finalize in the plan: map `file=` to the block's existing
`Source` enum without a new block code path.)

## Discovery + `SKILL.md` + library reuse — all from the same manifests

- **`gizza tool list`** → table of `name`, `summary` from each `manifest.json`.
- **`gizza tool describe <name>`** → the tool's `description` + `parameters` schema (human
  or `--json`).
- **`SKILL.md` is generated**, not hand-written: a small generator (a `gizza tool
  gen-skill` subcommand, or a `build.rs`/`xtask` step) renders `SKILL.md` from
  `gizza tool list --json`. A CI check asserts the committed `SKILL.md` matches
  regeneration, so it can never drift from the tools. The `SKILL.md` documents the
  `gizza tool …` contract and lists each tool with its schema, so an agent can self-serve.
- **Programmatic integration**, two ways off the same core:
  - shell out to the binary and parse `--json`;
  - depend on the `cli` crate's library half — `run_tool(name: &str, args: serde_json::Value)
    -> Result<Envelope, ToolError>` — with the binary as a thin `main` over it. (The crate
    is `[lib] + [[bin]]`.)

## Error handling

- Unknown tool → exit 2, message lists `gizza tool list`.
- Bad args (missing required, uncoercible type) → exit 2, echoes the expected schema.
- Tool returns an error envelope / `tool_failed` → exit 1, prints the message; `--json`
  prints the structured error.
- Unsupported-in-CLI (stub) → exit 3, typed `unsupported_in_cli`.
- Missing system dependency (`ffmpeg` absent) → exit 1, names the dependency.
- All error text goes to **stderr**; only tool output goes to **stdout**, so piping
  `gizza tool … --json | jq` is clean.

## Testing

- **Pure tools** — exact-output asserts: `calculator "2*2"` → `4`, `clock` formats, error
  cases (`calculator "1/0"` → non-zero + message). No runtime mocks needed beyond booting
  the wasm.
- **Arg mapping** — unit tests for positional/`key=value`/`--json` → JSON body against
  representative schemas (single-required-scalar, multi-field, type coercion, the
  both-given ambiguity error).
- **ffmpeg path** — an integration test gated on `ffmpeg` being present
  (`#[cfg_attr(not(has_ffmpeg), ignore)]` or a build-probe), resizing a tiny fixture image
  end-to-end through `gizza-ai/image-resize` + `NativeFfmpegService`.
- **Stub path** — `gizza tool imagine …` asserts the typed `unsupported_in_cli` error +
  exit 3.
- **Discovery / drift** — a snapshot test of `gizza tool list --json` and a CI check that
  the committed `SKILL.md` equals a fresh regeneration.

## Open implementation details (resolved in the plan, not blocking the design)

- Exact home of the relocated `FfmpegBlock`/`FfmpegService` (extend `block-utils` vs. new
  `gizza-ffmpeg-runtime` crate) — pick whichever keeps `block-utils` native-clean.
- How `file=<path>` maps onto each block's existing `Source`/`SourceFields` without adding
  a block code path.
- Whether `SKILL.md` generation is a CLI subcommand or an `xtask` — both read the same
  `tool list --json`.
- Whether the `cli` crate vendors a second copy of the `SKILLS` `build.rs` or both crates
  call a shared `build-support` helper (prefer the shared helper).

## Change-set summary (all under `gizza-ai/`)

1. **Relocate** `FfmpegBlock` + `FfmpegService` + `ExecArgs`/`ExecResult` into a
   native-compilable shared crate; app + CLI both register the same block with their own
   service.
2. **New `cli/` crate** (`[lib] + [[bin]] gizza`): native `Wafer`(wasmi) boot, host-service
   registration, `tool` subcommand (`run`/`list`/`describe`), schema-driven arg mapping,
   envelope rendering, exit-code policy.
3. **`NativeFfmpegService`** (shell-out) + **stub services** for unsupported capabilities.
4. **Shared `SKILLS` embedding** (factor the app `build.rs` logic into a helper both
   crates use).
5. **Generated `SKILL.md`** + CI drift check.
6. **Tests** per the Testing section.
