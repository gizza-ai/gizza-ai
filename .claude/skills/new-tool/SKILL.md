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
   - `src/lib.rs` — edit `descriptor()` to the tool's real params (the SINGLE SOURCE
     for the chat schema AND the CLI; `parameters = schema_json()` is already wired, so
     do NOT hand-write an inline JSON schema) + the `skill(description=...)` text. The
     scaffold already delegates: `run_skill` (pure/no-page) or `resolve_source` →
     `dispatch_ffmpeg` → `build_media_envelope` (ffmpeg). Param API:
     `Param::string|integer|number|enumv|boolean|string_map(...)` +
     `.required()/.default(v)/.min(n)/.max(n)/.describe(s)` — give EVERY param a
     `.describe()` that tells an LLM/CLI user what to pass (values, units, default,
     an example); `Input::None` for
     pure/param-only, `Input::Image|Video|Audio|Document|File` for a `url`⊕`ref` media input,
     `source_list` for an array of sources. Exemplars: `blocks/url-encode` (pure),
     `blocks/image-resize` (ffmpeg page), `blocks/web-fetch` (no-page, flat output).
   - `web/src/lib.rs` — the export (pure: `run`; ffmpeg: `build_argv` returning the
     shared `gizza_ai_block_utils::ArgvPlan { argv, out_name }` — already wired by the scaffold).
   - `page/meta.toml` + `page/content.md` — real title/desc/tags/inputs (field ORDER must
     match the `build_argv` param order for ffmpeg); + `tests/*.json` wafer fixtures. Write
     the FAQ in `content.md` as `<details>`/`<summary>` accordions (the scaffold seeds one),
     NOT plain `## FAQ` markdown — keep a BLANK LINE inside each `<details>` so the answer's
     markdown renders and wraps in `<p>` (see `blocks/age-calculator`). Plain-markdown FAQ is
     a hard-fail in the hygiene gate.
   - `manifest.json` — the scaffold generates it (build.rs needs it; `wafer build` does
     NOT). **`tool.parameters` is LOAD-BEARING for the page**: the page form
     (`tools/generator/src/control.rs`) reads the MANIFEST (not the live descriptor) to pick
     each field's control — a param renders as a `<select>` only if its manifest property
     carries the `enum`, as a checkbox/number for `boolean`/`integer`, else a text box.
     **Do NOT hand-write `tool.*`** — it is generated: after `cargo install --path cli`
     (step 7), `python3 scripts/sync-tool-manifest.py <slug>` regenerates
     `tool.parameters`/`tool.description` from the live descriptor and propagates the
     `#[wafer_block(summary = "…")]` macro summary into `manifest.json` + `wafer.toml`.
     **Summaries:** write ONE clean one-line summary in the macro (no vestigial `"… skill"`
     suffix) — the sync script owns the other two copies. Fill every scaffold `TODO` (title,
     h1, hero, descriptions); the gate fails on any leftover `TODO`.
7. **Build (in this order):** `wafer build` (from `blocks/<slug>/`); `cargo test --workspace`
   (from `blocks/<slug>/`); `wasm-pack build blocks/<slug>/web --target web --release --out-dir pkg`;
   `cargo install --path cli --force`; `python3 scripts/sync-tool-manifest.py <slug>` (regenerates
   manifest `tool.*` + summaries from the live descriptor — required BEFORE the generator, which
   reads the manifest); `cargo run --manifest-path tools/generator/Cargo.toml -- .`;
   `solobase build` (may be SKIPPED per the create-next-tool throughput rule — `wafer build`
   already validated this block's wasm and CI runs the full build on deploy; say so in the PR
   if skipped); and `python3 scripts/check-tool-hygiene.py <slug>` (the hard gate CI
   enforces — per-slug mode is strict: drifted manifest, plain-markdown FAQ, scaffold TODOs,
   summary drift, missing placeholders, <3 FAQs, bad meta description length). Fix
   compile/test failures (≤3 attempts) before escalating. (No SKILL.md to regenerate —
   the root SKILL.md is static and points agents at `gizza list`/`gizza describe`.)
8. **Test (type-aware)** — see reference.md:
   - unit (always) + wafer fixtures (always);
   - **Playwright** the page (pure + ffmpeg): add a spec in `tests/` driving `/tools/<slug>/`.
     The spec MUST assert real output correctness — exact text for pure tools; for media,
     decode the result and assert dimensions/pixels (reference.md §media correctness) — plus
     one `?param=` deep-link case;
   - **CLI** (pure/ffmpeg/network): `gizza tool <slug> …` incl. one exact-output case;
     gpu: assert `unsupported_in_cli` + exit 3.
8b. **User-QA rubric (before ship)** — load `/tools/<slug>/` once more and check as a USER:
   every text/number field has a meaningful placeholder; the page copy shows ≥1 worked example
   (input AND output); the FAQ answers ≥3 real questions; known limits (max sizes, formats,
   depth caps) are stated on the page, not only in code; error messages say what was expected;
   every descriptor param has a `.describe()` an LLM could act on. Fix gaps before committing.
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
