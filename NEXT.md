# Next Steps

Short handoff note — what's still open to pick up next session. `FUTURE.md`
is the long-term catalogue; this file is the near-term queue plus the
operational gotchas worth keeping at hand. Historical merge logs were
dropped 2026-06-03 (they live in git history); only open work and living
reference remain.

## Pending work

### Blocked on a producer-side (solobase) change

- [ ] **Conversation persistence** — write user/assistant turns to
  `suppers-ai/messages`; load prior entries on page init. **Blocked**: the
  `suppers-ai/messages` list endpoint requires authentication and gizza-ai
  runs anonymous. Either an anonymous list path or a server-side helper that
  synthesizes auth needs to land in solobase first.
- [ ] **`gizza-ai/search-messages`** — calls `suppers-ai/messages` via
  `ctx.call_block` to search past conversation. Blocked on conversation
  persistence above (no chat history is persisted today).

### Operator items (your action only)

- [ ] Configure GitHub Pages for the repo (Settings → Pages → source:
  `gh-pages`).
- [ ] DNS: CNAME `gizza.ai` → `gizza-ai.github.io` (and `www.` if desired).

### CI / quality

- [ ] **Pre-merge clippy on the wasm32-wasip1 sub-workspaces.** The host
  crate is clippy-clean, but each block crate has its own `Cargo.toml` and
  is never linted by CI. Add a `justfile` recipe that loops `cargo +nightly
  clippy --target wasm32-wasip1 --all-targets` over `block-utils/` and
  `blocks/*/`, and wire it into the GH Actions `test` workflow.
- [ ] Make the Playwright e2e suite reliable in CI (model caching across
  runs via persistent context, OR an LLM-free dispatch path). Low priority —
  `cargo test --test dispatch_skills` covers the bulk of regressions cheaply.

### Smaller paper-cuts

- [ ] `security-headers` block reads CSP from flow-step config, not from
  `block_configs` (filed in Plan B commit `d83d657`). Clean fix is upstream:
  have the block also consult `block_configs`, or expose
  `SolobaseBuilder::csp(...)`.
- [ ] Error taxonomy for `AssetLoadError` could split `LoaderNotConfigured`
  from `UnknownLoader`, and `Bridge(String)` from `Unknown(String)` (both
  noted in Plan A reviews). Low urgency.
- [ ] Trim remaining module-doc rationale: `src/lib.rs:1-9` and
  `src/blocks/ui.rs:1-5` still describe high-level architecture in `//!`
  comments — either keep (they're short) or move to `docs/architecture/`.

_let-else / `.clone` audit was intentionally skipped: let-else doesn't
cleanly bind `Err` variants, and the `Attachment` clone path is gated to
10 MiB once-per-dispatch (rated "low magnitude"). Revisit only if either
becomes a hotspot._

## Operational gotchas (living reference)

- `solobase` on `$PATH` is the wrong binary (Go-based). Use the full path
  `/home/joris/Programs/suppers-ai/workspace/solobase/target/release/solobase`.
- `~/.cargo/bin/wafer` can be stale relative to `wafer-run/main`. Symptom:
  `wafer build` fails with "function type mismatch for import
  wafer::__wafer_host_call_block". Fix: `cd wafer-run && cargo install --path
  crates/wafer-cli --force`.
- Always `--no-track` when creating a new branch: `git worktree add
  --no-track -b NEW ../path origin/main`. Plain `git checkout -b NEW
  origin/main` silently sets upstream to `origin/main` and a later
  `git push` pushes to main.
- The Playwright e2e (`tests/e2e_smoke.spec.ts`) is gated on headed Chromium
  (WebGPU) and a 1.2 GB WebLLM download; it's a manual smoke, not CI. The
  deterministic CI gate is `cargo test --test dispatch_skills` (see
  `tests/README.md`).
- **Debugging Service Worker tests in Playwright**: `page.on('console')`
  only captures the page's console, NOT the SW's. To see SW logs, hook
  `context.on('serviceworker', sw => sw.on('console', ...))`.
- **Rebuilding sql.js FTS5**: Docker Desktop must be running. Run `bash
  scripts/build-sql-js-fts5.sh` (in solobase) — ~5 min in the
  `emscripten/emsdk:3.1.74` container. The script passes the FULL upstream
  `SQLITE_COMPILATION_FLAGS` plus `-DSQLITE_ENABLE_FTS5`; do NOT shorten that
  list (see solobase #47 commit `1373975` — dropping
  `-DSQLITE_OMIT_LOAD_EXTENSION` leaves dlopen stubs as null function
  pointers in wasm and traps with "null function" on first
  `new SQL.Database()` in a Service Worker).
- **Skill blocks must use `#[wafer_block(skill(...))]` to appear in
  tool-calling.** Without the `skill(...)` attribute, `BlockInfo` carries
  `role: None` and `tool: None`, so `build_tools()` silently returns an empty
  list — the block is invisible to the LLM.

## References

- Original design: `docs/superpowers/specs/2026-04-18-gizza-ai-design.md` (workspace sibling)
- Plan A (upstream enablers, merged): `docs/superpowers/plans/2026-04-18-gizza-ai-plan-a-upstream.md`
- Plan B (MVP, merged): `docs/superpowers/plans/2026-04-18-gizza-ai-plan-b-mvp.md`
- Plan C campaign (NEXT.md execution): `docs/superpowers/plans/2026-04-30-gizza-ai-next-md-implementation.md`
- Deferred features catalogue: `FUTURE.md`
