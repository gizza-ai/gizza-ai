# Design — gizza shared tool abstraction (single-source descriptor)

**Date:** 2026-06-19
**Status:** Design approved (brainstorming); pending writing-plans.
**Repos:** `wafer-run` (producer, one small change) → `gizza-ai` (consumer).
**Related:** [[gizza-new-tool-skill]] (`/new-tool`), the `gizza-search-tools` backlog, the build-notes (`docs/checks/2026-06-18-gizza-new-tool-build-notes.md`).

## Problem

gizza now has ~25 tools and a 1000-tool backlog. Each tool is an independent
crate (so the worktree-per-tool parallel build model works), with shared code
already factored into two path/git-dep crates: `block-utils` (I/O: fetch,
attachment, `Envelope`/`ForUi`, `AssetKind`, ffmpeg dispatch, filename/mime
helpers, `SkillError`) and `chrome` (header/footer UI).

What is **still duplicated** across every tool:

1. **The skill wrapper boilerplate** — an `Args` struct, a `handle()` that does
   parse → call `core` → wrap `{result}`/`{error}`, a re-defined `respond_error`,
   and the `web/src/lib.rs` `wasm-bindgen` export. Copy-pasted 25×.
2. **The param declaration, declared twice** — the chat JSON schema (the
   `parameters` literal in `src/lib.rs`'s `#[wafer_block(skill(...))]`, which also
   drives the CLI via `cli/args.rs::map_args(schema, …)`) **and** the page form
   fields (`page/meta.toml` `[[input]]` blocks). Nothing enforces these agree, and
   `build_argv`'s positional order is also coupled to the page field order. This is
   the primary correctness/consistency risk.

A subtlety that shapes the whole design: the chat and page surfaces do **not**
share the same param *set*. For a media tool the *input* differs by surface —
chat takes `{url|ref}`, the page takes a file upload — while the *logical* params
(width/height/fit, mode/target, …) are identical but written out twice. So the
single source of truth cannot be "the JSON schema"; it must be one level up.

## Goals

- **Reduce code:** a tool is essentially its `core` logic + one declaration; the
  wrapper, the chat schema, the page fields, and the web export are derived/shared.
- **Reduce error surface:** the chat↔page param drift becomes structurally
  impossible; the `url⊕ref` oneOf and the media fetch→ffmpeg→envelope orchestration
  (recurring per-tool bug sources) are written once.
- **Consistency:** every tool returns the same `{result}`/`{error}` shape and
  validates inputs the same way; global fixes land in one place.
- **Retrofit all ~25 existing tools** (not just new ones).
- **Deep-linkable, AI-discoverable tools:** every tool page accepts URL **query
  parameters** named by the descriptor, pre-fills the form, and auto-runs when the
  inputs are satisfied (e.g. `/tools/calculator/?expression=2%2B2*3` opens already
  showing the result). The accepted params + an example deep-link are documented on
  the page **and** in the markdown twin so an LLM can drive the tool by URL.

## Non-goals / out of scope

- A full `#[gizza_skill]` proc-macro that also generates the impl (Approach C). The
  descriptor introduced here is exactly what such a macro would consume later; this
  spec stops short of it deliberately (YAGNI / risk).
- New browser loaders (transformers.js / onnx / pyodide) for the model-backed
  backlog tools — separate effort.
- The `/improve-tool` competitor-research skill — separate spec.
- `chrome` (header/footer) — already shared, untouched here.

## Design

### 1. The descriptor (single source, lives in `core`)

Each tool's `core` crate exposes one declaration. The `ToolDescriptor`, `Param`,
and `Input` types live in `block-utils` (shared dep):

```rust
pub enum Input { None, Image, Video, Document, File }  // the binary/remote input
                                                       // that varies by surface;
                                                       // plain text is just a String Param.

pub struct Param {
    name: String,             // = chat-schema property AND URL query-param name
    kind: ParamKind,          // String | U32 | F64 | Enum(Vec<String>) | Bool
    required: bool,
    default: Option<Value>,
    description: String,       // LLM-facing → chat schema
    label: Option<String>,    // UI-facing → page form
    placeholder: Option<String>,
    multiline: bool,          // page hint: render a String param as a textarea
}

pub struct ToolDescriptor { input: Input, params: Vec<Param> }
```

```rust
// blocks/image-resize/core/src/lib.rs
pub fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::Image)
        .param(Param::u32("width").label("Width (px)").placeholder("640")
                    .describe("Target width in pixels."))
        .param(Param::u32("height").label("Height (px)").placeholder("(optional)")
                    .describe("Target height in pixels; omit to scale by width."))
        .param(Param::enumv("fit", ["contain","cover","stretch"]).default("contain")
                    .label("Fit (contain|cover|stretch)")
                    .describe("How to fit the image into the box."))
}

// blocks/calculator/core/src/lib.rs — pure text, the deep-link example
pub fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("expression").required()
                    .label("Expression").placeholder("2 + 2 * 3")
                    .describe("The arithmetic expression to evaluate."))
}
// → chat: { "expression": {...}, required:["expression"] }
// → page field "expression"; → /tools/calculator/?expression=2%2B2*3 auto-runs.
```

### 2. Deriving each surface

| Surface | Derivation |
|---|---|
| **Chat schema** (`block.info()` JSON `parameters`) | `Input::Image/Video/Document/File` → a `url`⊕`ref` `oneOf` (exactly one); `Input::None` → no input property. **+** each `Param` (including plain `String`/text params) → a JSON property with description/enum/default, listed in `required` when `required`. Produced by `descriptor().to_schema_json()`. |
| **CLI** | reads the chat schema from `block.info()` via `cli/args.rs::map_args` → already single-sourced, no change. |
| **Page form** (`[[input]]`) | `Input::Image/Video/…` → a `source="file"` upload with the right `accept`; **+** each `Param` → a `source="field"` input (text, or textarea when `multiline`) using its label/placeholder. Produced by `descriptor().to_page_inputs()`, consumed by the generator. |
| **`build_argv`** | receives a `name → value` map keyed by `Param.name` (no positional-order coupling to the page). |
| **Page query params + auto-run** | the shared page runtime hydrates each field from the URL query param of the same name (`?width=…&fit=…`); the media/file/document input is hydrated from `?url=` (fetched in-browser, fed to the same ffmpeg path). Auto-runs once required inputs are satisfied. Generic — no per-tool JS (see §6). |
| **Docs** (page + markdown twin) | the generator renders a "Use via URL" section on the page (param table + copyable example deep-link) and the same in `/tools/<slug>/index.md` (the LLM-facing surface). Produced from `descriptor()`. |

`page/meta.toml` is slimmed to **page chrome only**: `slug`, `title`, `tags`,
`h1`, `hero_subtitle`, `wasm`, `export`, `runtime`, `output_label`, `format`,
SEO. It no longer contains `[[input]]` blocks.

### 3. Chat-schema injection (wafer-run change — "2a")

The `wafer_block` macro currently requires `skill(parameters = …)` to be a string
**literal** (`parse_skill` accepts only a `syn::LitStr` and validates the JSON at
macro-expansion time). Change: **allow `parameters` to be an expression** that
evaluates to `&str`/`String` (a `const`/`fn` path), keeping the string-literal form
for backward compatibility. When an expression is given, compile-time JSON
validation is replaced by a runtime/test check.

Then a gizza skill writes:

```rust
#[wafer_block(
    name = "gizza-ai/image-resize", version = "0.1.0", interface = "handler@v1",
    summary = "Resize an image",
    skill(description = "...", parameters = gizza_ai_image_resize_core::schema_json())
)]
```

`schema_json()` is `descriptor().to_schema_json()` (cached). The hand-written
chat-schema literal is **deleted**.

### 4. The helper layer (`block-utils`)

- `run_skill<A: DeserializeOwned, T: Serialize>(body, |A| -> Result<T, SkillError>) -> GuestResult`
  — deserialize → call → shape `{result}`/`{error}`. Replaces every `handle` +
  `respond_error`.
- `respond_ok(value)` / `respond_err(SkillError)` — the one canonical result/error
  shape.
- `run_media_skill(body, descriptor, |fields, in_name| -> Argv)` — the whole media
  path once: `pick_source` → `fetch_from_url`/`load_from_attachment` →
  `dispatch_ffmpeg_runtime` → `Envelope`.
- `gizza_web_export!` (`macro_rules!`) — generates the `web/src/lib.rs`
  `wasm-bindgen` export from `core`'s `convert`/`build_argv`.

**Every** tool declares a `core::descriptor()` — it is what derives the chat
schema (and thus the CLI) for all shapes. Only tools that have a page additionally
use `to_page_inputs()`. Resulting tool shapes:
- **pure text** (`Input::None` + a `String` param, e.g. `expression`) → `core` (`convert`) + `descriptor()` + `run_skill(body, |a| core::convert(a))`; page derives from `to_page_inputs()`.
- **media / ffmpeg page** (`Input::Image|Video`) → `core` (`build_argv`) + `descriptor()` + `run_media_skill(body, core::descriptor(), core::build_argv)`; page derives from `to_page_inputs()`.
- **no-page chat+CLI / network** (`Input::Document|File|None`) → `descriptor()` (drives chat schema only) + `run_skill` (+ `block-utils` input helpers). No `meta.toml`, no `to_page_inputs()` call.

The typed `Args` struct stays (ergonomic deserialize); its field names overlap the
descriptor's, but that is local and low-risk (covered by the per-tool tests).

### 5. Generator reads the descriptor

A pre-generator build step serializes `core::descriptor()` to a gitignored
`blocks/<tool>/descriptor.json` (one per page tool). The generator reads
`descriptor.json` (form fields) + `meta.toml` (page chrome) and renders the page.
The generator does **not** link the 25 tool crates — it reads JSON. This mirrors
the existing build-notes step "build each page block's `web/pkg` before the
generator"; add "emit `descriptor.json`" alongside it.

### 6. Query parameters & deep-linking (page + docs)

The query-param contract **is** the descriptor's param set — same names as the
chat schema, so it costs nothing extra to keep consistent.

**Runtime (shared, generic — no per-tool JS).** The existing shared page script
(`site/tool.js`) gains a hydrate-and-run step: on load it parses
`location.search` and, for each descriptor field, sets the matching input by name
(`?expression=…`, `?text=…&mode=…`, `?width=…&height=…&fit=…`). For media/file/
document tools the input is hydrated from **`?url=`**: the page fetches the remote
bytes in-browser and feeds them into the *same* path a file upload uses (browser
ffmpeg via the existing engine). After hydration, if every required input is
present the page **auto-runs** and renders the result; otherwise it just pre-fills
and waits (e.g. a media page with only `?width=` set waits for a file). Param names
match `Param.name`, values are standard percent-encoded. This is driven entirely by
the generic descriptor field list, so it applies to all page tools at once.

**`?url=` failure handling.** A cross-origin fetch can be blocked by the remote
host's CORS policy. On failure the page shows a clear, non-fatal message — e.g.
"Couldn't fetch `<url>` (the host may not allow cross-origin access). Download it
and drop it in instead." — and falls back to the manual file input. No silent
failure, no broken auto-run state.

**Docs (the AI-facing requirement).** From `descriptor()` the generator emits, on
both the page and the markdown twin `/tools/<slug>/index.md`:
- a **parameter table** — name, type, required, default, description (and, for
  media/file tools, that `url` fetches a remote input);
- a **copyable example deep-link**, e.g.
  `https://gizza.ai/tools/calculator/?expression=2%2B2*3` or
  `https://gizza.ai/tools/image-resize/?url=https://example.com/cat.jpg&width=512`.

Because `index.md` is what `llms.txt` already points LLMs to, this is how an agent
learns to drive a tool by URL. `llms.txt` page-tool entries gain a short
"supports query-param deep-linking — see index.md" note.

## Sequencing (producer → consumer)

1. **wafer-run** — `wafer_block` accepts an expression `parameters` (PR → merge →
   fast-forward the local `/workspace/wafer-run` tree, respecting the shared-tree
   hazard: do it when no gizza build is in flight).
2. **gizza `block-utils`** — `ToolDescriptor`/`Param`/`Input` (+ serde),
   `to_schema_json()`, `to_page_inputs()`, `to_descriptor_json()`, `run_skill`,
   `respond_*`, `run_media_skill`, `gizza_web_export!`. Land as one infra PR
   **before** fanning out retrofits (lesson from the `AssetKind` reconciliation:
   shared infra goes first, not ad hoc per tool).
3. **gizza generator + page runtime** — read `descriptor.json` + emit step; slim
   `meta.toml`; render the "Use via URL" param table + example deep-link on the page
   and in `index.md`; add the generic query-param hydrate-and-run (incl. `?url=`
   fetch + CORS fallback) to the shared `site/tool.js`. Prove on 2–3 exemplars (one
   pure, one media).
4. **Retrofit all 25** — grouped by shape (pure-text · ffmpeg-page · no-page
   chat+CLI · network), ~4–6 tools per PR, worktree-per-batch + adversarial
   review (the build-program pattern; see [[workflow-durability-protocol]]).
5. **`/new-tool` + `scaffold-tool.sh` + build-notes** — emit the descriptor-based
   shape so future tools are born consistent.

## Testing / drift guards

- `block-utils` unit tests: descriptor → schema (string/u32/f64/enum/default/
  required; the media `url⊕ref` oneOf) and descriptor → page fields.
- **Migration safety:** during retrofit, diff each tool's *derived* schema against
  its *current authored* schema — param names/descriptions/enums/defaults must
  match so LLM tool-calling quality is preserved. A tool ships only when the diff
  is intentional/empty.
- CI check that `descriptor.json` regenerates clean (cannot go stale).
- Existing per-tool behavior tests + generator page tests stay green.
- `wafer_block` macro: a test that the expression form produces info() with valid
  JSON parameters (replacing the lost compile-time validation).
- **Query-param deep-linking** (Playwright, per the build-notes headless pattern):
  a pure tool loaded with `?<param>=…` pre-fills and auto-renders the result; a
  media tool with `?url=…` fetches + runs; the `?url=` CORS-failure path shows the
  fallback message (not a broken state); a media page with only scalar params set
  pre-fills and waits for the file.
- Generator/markdown tests: `index.md` and the page contain the param table + a
  correctly percent-encoded example deep-link derived from the descriptor.

## Risks & mitigations

- **Schema fidelity on retrofit.** Hand-tuned descriptions/enums/defaults matter
  for LLM tool-calling. Mitigation: the per-tool derived-vs-authored schema diff
  gate above.
- **`url⊕ref` oneOf shape.** The derived oneOf must match what the chat agent and
  CLI expect today. Mitigation: encode the exact current shape in
  `to_schema_json()` and assert against a real tool's current schema.
- **wafer-run shared-tree hazard.** The producer change + local fast-forward must
  not race a concurrent gizza build. Mitigation: do the ff when idle; gizza's
  `.cargo` patch points at the local tree so no push is needed for local builds,
  but CI clones wafer-run main → the wafer-run PR must merge before gizza retrofit
  PRs' CI can pass (producer-merge-first; see [[gizza-ci-pin-vs-wafer-main-drift]]).
- **`?url=` CORS for media deep-links.** Arbitrary remote media may block
  cross-origin fetch. Mitigation: graceful fallback message + manual upload (§6);
  deep-linking still works for same-origin / CORS-permissive hosts and for all
  text tools. Not a correctness risk, a coverage limit — documented, not silent.
- **Scope.** ~9 PRs (1 wafer-run + 1 infra + 1 generator/runtime + ~5 retrofit + 1
  skill-update); the query-param surface rides the generator/runtime + retrofit
  work, not a separate phase. Phased and reviewable; matches "retrofit everything".

## Open questions (for writing-plans)

- Exact `ParamKind` set — do any current tools need types beyond
  String/U32/F64/Enum/Bool (e.g., arrays for multi-input tools like `merge-pdf`)?
  Likely add `Array(Box<ParamKind>)`; confirm against the no-page tools.
- Whether `run_media_skill` fully covers every current ffmpeg tool's quirks
  (e.g., `video-trim` stream-copy vs re-encode) or needs a small per-tool hook.
