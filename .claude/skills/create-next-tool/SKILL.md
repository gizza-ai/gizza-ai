---
name: create-next-tool
description: "Use when the user wants to build the next un-built gizza tool from the tools-to-build.csv backlog. Picks the next tool whose blocks/<slug>/ folder doesn't exist yet, builds it with the /new-tool procedure, fully enhances + verifies it with the /improve-tool procedure (competitor research + CLI/page checks), then commits and pushes on the current branch. No new branch, no PR. One tool per run."
---

# create-next-tool — build the next backlog tool end to end

Autonomous: build ONE tool per run from `tools-to-build.csv`, on the CURRENT branch, with NO
new branch and NO PR — just commits + a push. It orchestrates the two sibling skills: follow
`/new-tool`'s build steps, then the FULL `/improve-tool` procedure, but own the git yourself.
NEVER claim a step passed that you didn't run.

Read the two sibling skills for the actual recipes:
- `.claude/skills/new-tool/{SKILL.md,reference.md}` — the build procedure.
- `.claude/skills/improve-tool/{SKILL.md,reference.md}` — the verify+improve procedure.

Plus this skill's `references/` files (see the index at the bottom) — they encode every dead end
already hit; read the relevant one BEFORE implementing.

Follow these steps in order:

0. **Toolchain.** This is the public toolkit repo (blocks + `gizza` CLI + the generic tool-page
   generator; no app, no branding, no deploy — those live in a private site repo that consumes this
   one at a pin). This skill needs `cargo`, `wasm-pack`, `gizza`, Playwright, and `ffmpeg`; the
   `wafer` CLI is OPTIONAL (a convenience wrapper around `cargo build --target wasm32-wasip1
   --release`, only buildable from a sibling `wafer-run` checkout — not required here, and CI never
   uses it). If any required tool is missing, bootstrap once with `scripts/bootstrap-toolchain.sh`
   (details + gotchas in `docs/TOOLCHAIN-SETUP.md`); the very first run also needs a baseline pass
   building every existing block's `target/block.wasm` (`cargo build --target wasm32-wasip1
   --release` per block, copied in) + `web/pkg/` (`wasm-pack build blocks/<slug>/web --target web
   --release --out-dir pkg` per block) — else the generator skips blocks missing `web/pkg/` and
   `gizza list` is incomplete.

1. **Pick the next tool.** From the gizza-ai repo root:
   ```bash
   scripts/pick-next-tool.py
   ```
   It prints `<slug>\t<name>\t<description>\t<type_hint>` for the next buildable tool, or a
   sentinel (`BACKLOG_COMPLETE` / `NO_BUILDABLE_REMAINING`) — report it and stop on a sentinel.
   The picker improves on a plain first-un-built scan (it logs every skip to stderr):
   - **built** = `blocks/<slug>/` committed in `git HEAD` (a half-built failure never counts, so a
     crashed run is retried, not skipped forever);
   - **curated skips** from `docs/tool-skiplist.txt` — confirmed duplicates of an existing tool
     (the exact-slug scan can't catch semantic near-dups like `pdf-to-text` ≈ `pdf-extract-text`);
   - **out-of-model** rows (need an ML model / pyodide — whisper, transformers-js, etc.) are
     **deferred** by default; gizza is pure-Rust + ffmpeg. Pass `--include-model` only if you
     intend to build one as a gpu chat-only block.
   `name` + `description` are your build inputs; `type_hint` (pure|ffmpeg|network|model) is a
   starting guess — still classify properly in step 2. `scripts/pick-next-tool.py --stats` shows
   the backlog breakdown.
   **If during the build you discover the tool is a semantic near-dup of an existing one, STOP, add
   a `<slug>  # duplicate of blocks/<other>` line to `docs/tool-skiplist.txt`, commit that, and
   re-run the picker** rather than shipping a redundant tool.

2. **Build** — follow `/new-tool` **steps 3–8** (classify type → `scripts/scaffold-tool.sh <slug> <type>`
   → implement `core`/`descriptor`/`web`/`page` → build → type-aware tests) using the `name` +
   `description` from step 1. **SKIP** `/new-tool` step 2 (branch) and steps 9–10 (push/PR/code-review)
   — git is owned by step 4 here. The manifest `tool.*` section is GENERATED, not hand-written:
   after `cargo install --path cli`, run `python3 scripts/sync-tool-manifest.py <slug>` (see
   "Manifest sync + hygiene gate" below).

   **Throughput note (verified):** the per-tool validation is `cd blocks/<slug> && cargo build
   --target wasm32-wasip1 --release` (then copy the produced wasm to `target/block.wasm` — exactly
   what CI's "Build changed skill wasms" step runs) + `cargo test --workspace` + `wasm-pack build
   blocks/<slug>/web --target web --release --out-dir pkg` + `cargo run --manifest-path
   tools/generator/Cargo.toml -- .` (renders the page, GENERIC — no site config here) + `gizza tool`
   (CLI) + Playwright. These are all minutes, and there is no whole-app build to amortize in this
   repo — that bottleneck belonged to the site repo (which consumes this one at a pin and builds its
   own branded app separately); each tool's page is fully standalone under `pkg/tools/<slug>/`. The
   generator step still needs every OTHER block's `web/pkg/` to already exist (built once in the
   toolchain baseline) so it doesn't skip them — only the new tool's `web/pkg/` is added per run.

3. **Improve** — follow the FULL `/improve-tool` **Phases 1–5** on `<slug>`: verify the 2
   locally-verifiable surfaces (CLI, page query-params — the live chat UI lives in the private site
   repo and can't be exercised here; see `/improve-tool` Phase 1) + fix any breakage → research the top-5 competitors → diff +
   rank gaps (fit-to-model) → close every in-model capability/copy/UX/visual gap → regenerate the
   drift-guard → re-run the full test matrix. Write the competitor-analysis snapshot to
   `docs/checks/<YYYY-MM-DD>-improve-<slug>-competitor-analysis.md`. **SKIP** `/improve-tool`'s "Gather
   + branch" step and **Phase 6** (PR). Its rules carry over: **NEVER copy competitor
   copy/branding/trademarks**; list out-of-model features, don't build them.

4. **Commit + push** on the CURRENT branch (no PR). Two commits keep history clear:
   ```bash
   git add blocks/<slug> tests/
   git commit -m "feat(<slug>): new tool"
   git add blocks/<slug> tests/ docs/checks/
   git commit -m "feat(<slug>): competitor improvements + analysis"
   git push
   ```
   Publication is a separate, out-of-scope step: this push doesn't reach gizza.ai by itself — the
   private site repo consumes this repo at a pin, so the new tool goes live only after that repo's
   pin is bumped past this commit (a PR there, not here).

**Honesty + cleanup gate:** if the build (step 2) or verification (step 3 Phase 1) fails
unrecoverably (≤3 fix attempts per the sibling skills), **STOP, run `git clean -fd blocks/<slug>` to
remove the partial scaffold, and report the failure with the error.** NEVER commit a broken tool — a
committed broken tool's `blocks/<slug>/` would make the next run skip it forever. If a surface can't
be headlessly verified (gpu has no page; chat-ffmpeg can't run in a Service Worker — page + CLI only),
state it explicitly rather than claiming a pass. **One tool per run**; re-invoke (or `/loop`) for the
next.

**Known limitation (mitigated):** the picker matches built tools by exact slug, so a semantic
near-dup (e.g. `pdf-to-text` vs the built `pdf-extract-text`) isn't auto-detected. `docs/tool-skiplist.txt`
holds the confirmed dups found so far; when you spot a NEW one mid-build, add it there (step 1) rather
than shipping a redundant tool. Always `ls blocks/ | grep -i <topic>` before building, and grep the
named block's `core/src/lib.rs` to confirm a shaky skiplist reason. Token-overlap auto-detection was
tried and rejected — it false-flags distinct tools (`age-calculator` is not a dup of `calculator`),
so dups stay hand-curated.

## Manifest sync + hygiene gate

The page form reads `manifest.json` `tool.parameters` (NOT the live descriptor) to pick each
field's control — a stub/stale manifest renders every field as a plain text box, enums included.
The manifest is therefore **generated, never hand-edited**: after `cargo install --path cli`, run

```bash
python3 scripts/sync-tool-manifest.py <slug>
```

which regenerates `tool.parameters` + `tool.description` from the installed CLI's live descriptor
and syncs the one-line `summary` from the `#[wafer_block(summary = "…")]` macro into
`manifest.json` + `wafer.toml` (one clean summary, no `"… skill"` suffix). Then

```bash
python3 scripts/check-tool-hygiene.py <slug>
```

must exit 0 before committing — per-slug mode is STRICT: enum→manifest sync, FAQ as `<details>`
accordions (blank line inside each), no scaffold TODOs, summary consistency, page copy stays
GENERIC (no `gizza.ai`/`gizza-ai.pages.dev` string anywhere under `page/` — check 8; this repo
renders unbranded, the private site repo injects branding at ITS build time, not here), plus
placeholders on text/number fields, ≥3 FAQ entries, and meta description length. CI runs the same
gate repo-wide.

## References (read the relevant one BEFORE implementing)

- `references/wasm-crates.md` — proven wasm-safe crates with full recipes (crypto, PGP,
  ECDSA/Ed25519, EPUB/ZIP, mail parsing, GIF encoding, misc). Check here before adding ANY
  dependency — an engine crate must INSTANTIATE, not just compile.
- `references/page-patterns.md` — how params render on the page (select/checkbox/textarea),
  boolean checkbox defaults, drift-guard `N.0` gotcha, clocks across surfaces, recurring tool
  shapes, page output formats (incl. audio), what's un-buildable (audio-input, multi-input ffmpeg).
- `references/ops.md` — disk cleanup, the never-delete-web/pkg rule, cwd resets, SSRF-guarded CLI
  fetch + public test URLs, hardware/concurrency reality, usage limits.

When you resolve a NOVEL build-level finding (a new wasm-safe crate, a page pattern, an ops
gotcha), append it to the matching `references/` file — not to this SKILL.md, which stays the
procedure. Dispatcher/operational findings go to `create-tool-loop/SKILL.md`.

## Sub-agent dispatch mode (long `/loop` runs)

The long-running loop — dispatcher algorithm, pacing, failure/limit back-off, task-leak cleanup,
and the ONLY copy of the **BUILDER PROMPT** — lives in the sibling
`.claude/skills/create-tool-loop/SKILL.md`. Do NOT duplicate the builder prompt here: a second
copy drifted once (it kept telling builders to run heavy builds in background + poll after the
loop had banned that for leaking orphan tasks by the hundreds) and was removed on 2026-07-02.
