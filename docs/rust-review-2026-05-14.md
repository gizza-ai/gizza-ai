# Rust Best-Practices Review — `gizza-ai`

**Date:** 2026-05-14
**Scope:** `src/` (the wasm-bindgen host crate) + the 12 sibling skill crates under `blocks/`. ~5,754 LOC of Rust.
**Reference:** Apollo GraphQL's [Rust Best Practices Handbook](https://github.com/apollographql/rust-best-practices) — chapter numbers below refer to that handbook.

The block crates each compile as their own independent workspace to `wasm32-wasip1`; the host crate compiles to `wasm32-unknown-unknown`. Findings keyed to Apollo's chapters.

## Clippy signal

`cargo clippy --lib --tests --offline` against the host crate is clean except for one warning — fixing it is a 1-line change.

**Ch.1 / clippy:unnecessary_map_or** — `src/blocks/agent.rs:427`:

```rust
.map_or(true, |arr| arr.is_empty())
// → .is_none_or(|arr| arr.is_empty())
```

I did not run clippy on the per-block sub-workspaces — each has its own `Cargo.toml` with `wasm32-wasip1` targets and they don't share a top-level workspace. If you want pre-merge guards, `wasm32-wasip1` clippy is worth adding to a `justfile` recipe; the same rules apply.

## High-impact findings

### 1. Massive cross-block duplication (Ch.1, Ch.3)

These helpers are copy-pasted verbatim across the block crates:

| Helper | Replicated in |
|---|---|
| `fn dispatch_ffmpeg_runtime` | `ffmpeg`, `image-crop`, `image-convert`, `image-resize`, `video-trim`, `video-transcode`, `video-frame-extract` (7×) |
| `fn derive_filename` | `image-fetch`, `image-crop`, `image-convert`, `image-resize`, `video-trim`, `video-transcode`, `video-frame-extract` (7×) |
| `fn percent_decode` + `fn hex_val` | 5–7× across the same blocks |
| `fn mime_to_ext` | 6× across image/video blocks |
| `struct FfmpegReq` / `FfmpegResp` | duplicated in every ffmpeg-calling block |

Apollo Ch.1 explicitly cautions against premature abstraction — but seven verbatim copies is past that line. Each block is its own workspace because they're independently compiled to wasm32-wasip1, but that doesn't preclude a shared `gizza-ai-blocks-common` crate as a path dep. Concrete proposal:

```
blocks/_common/
  ├── Cargo.toml      # path-deps wafer-sdk, serde, etc.
  └── src/lib.rs      # dispatch_ffmpeg_runtime, derive_filename,
                      # mime_to_ext, MAX_BYTES constants, FfmpegReq/Resp
```

This also fixes the next problem.

### 2. `#[allow(dead_code)]` on helpers used by the wasm32-gated impl (Ch.1)

`blocks/image-resize/src/lib.rs:96, 106, 118, 137, 156, 173` mark `mime_to_ext`, `derive_filename`, `percent_decode`, `hex_val`, `summary`, `dispatch_ffmpeg_runtime` as `dead_code`. They're not dead — they're called from inside the `#[cfg(target_arch = "wasm32")]` `impl` block. Same pattern in `image-convert`, `image-crop`, `video-*`.

Apollo Ch.1 §1.6 calls out comments-that-rot; the same applies to `#[allow]` attributes that paper over a structural issue. Two fixes, in order of preference:

- Move these into a shared crate (per finding #1). When that crate is consumed by the wasm32-gated impl only, mark the path-dep optional + feature-gate, or rely on dead-code elimination at link time and drop the `#[allow]`.
- If kept inline, gate the helpers themselves with `#[cfg(target_arch = "wasm32")]` so they only exist when consumed. Then unit tests for them (which exist in `#[cfg(test)] mod tests`) need re-gating to test on the wasm32 build — annoying, which is itself a hint to extract the shared crate.

### 3. Verbose `match { Ok=>.., Err(e)=>return ..error.. }` chains (Ch.1, Ch.4)

The skill blocks (especially the image/video ones) hit 17–21 manual match-and-return blocks per file:

| Block | match→return count |
|---|---|
| `image-resize` | 21 |
| `image-convert` | 19 |
| `video-trim` | 18 |
| `video-transcode` | 18 |
| `image-crop` | 18 |
| `video-frame-extract` | 17 |

Apollo Ch.4 §4.5 is explicit: "Prefer using `?` over verbose alternatives like `match` chains." The way to get `?` here is to add a small `SkillError` enum per block (or one shared in `_common`):

```rust
#[derive(Debug, thiserror::Error)]
enum SkillError {
    #[error("invalid args: {0}")]
    InvalidArgs(String),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Wafer(#[from] WaferError),
    #[error("ffmpeg failed (exit {exit}): {snippet}")]
    Ffmpeg { exit: i32, snippet: String },
    // ...
}

impl From<SkillError> for GuestResult { /* map to GuestResult::error w/ correct ErrorCode */ }

fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body)?;
    let net = wafer_sdk::clients::network::do_request(...)?;
    // ...
}

impl ImageResize {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        match run(body) { Ok(v) => GuestResult::respond(v), Err(e) => e.into() }
    }
}
```

This would cut `image-resize/src/lib.rs` from ~500 LOC to roughly half. The `thiserror` crate is already in the host `Cargo.toml`; it just needs adding to the block crates.

### 4. Error type is `String` (Ch.4)

`src/ffmpeg.rs:31` (`FfmpegError`) uses `thiserror` correctly — good. But:

- `decode_uploads` (`src/blocks/agent.rs:810`) returns `Result<_, String>` for caller-facing validation errors.
- `openai_json_to_chat_message` (`src/blocks/agent.rs:894`) — same.
- `Args::source()` in every image/video block — same.
- `collect_chat_text` (`src/blocks/agent.rs:745`) — `Result<String, String>`, the latter being the error message.
- `ParamExtraction::Error(String)` (`src/blocks/agent.rs:392`).

Apollo Ch.4 §4.7 calls out `Box<dyn Error>` as the anti-pattern; `Result<_, String>` is worse: it conflates "error message" with "error type" and erases all matching ability. None of these need `anyhow` either — they each have 2–5 distinct failure modes that map cleanly to a `thiserror` enum. The `ToolOutcome` and `ParamExtraction` types already do enum modeling for happy-path variants; do the same for failures.

### 5. `unsafe impl Send / Sync` on `BrowserFfmpegService` (Ch.9)

`src/ffmpeg.rs:58-61`:

```rust
unsafe impl Send for BrowserFfmpegService {}
unsafe impl Sync for BrowserFfmpegService {}
```

The justification comment is good ("wasm32 is single-threaded; service holds no JS state of its own"). Apollo Ch.9 requires `// SAFETY:` blocks for `unsafe`. The comment is there, just promote it to the conventional form so future-grep finds it:

```rust
// SAFETY: wasm32 is single-threaded; this is a unit struct holding no JS handles.
// Mirrors BrowserNetworkService in solobase-browser. Required because the
// FfmpegService trait bound is MaybeSend + MaybeSync.
unsafe impl Send for BrowserFfmpegService {}
unsafe impl Sync for BrowserFfmpegService {}
```

### 6. Module doc comments are doing too much (Ch.1, Ch.8)

`src/blocks/agent.rs:1-37` is a 37-line `//!` block describing routes, SSE event shape, and slash-command flow. Apollo Ch.8 says rustdoc for public APIs; runtime behavior + protocol specs belong in `docs/` or an ADR.

Same pattern: `src/lib.rs:1-9`, `src/blocks/ui.rs:1-5`, plus mid-file 5-10 line `//` blocks at every step inside `handle_chat` (`src/blocks/agent.rs:323`, `:585`, `:858`).

Concrete: keep the route table + SSE schema (it's the protocol contract — useful), but pull the multi-paragraph rationale ("Stub for PR 5…", "Legacy plain-text behavior…") out to `docs/architecture/agent-block.md` and reference it (`See: docs/architecture/agent-block.md`). PR references (`PR 5`, `PR 6`) will rot fast.

## Medium-priority

### 7. `let-else` opportunities (Ch.1 §1.3)

Several places still use the `match { Ok=>.., Err=>return }` form where `let Ok(x) = ... else { return ... }` would flatten one level of indentation:

- `src/blocks/agent.rs:200-218` (parse body)
- `src/blocks/agent.rs:227-230` (decode_uploads)
- Most of the skill blocks at the top of `handle()`.

You already use `let-else` correctly elsewhere (`agent.rs:263, 399, 405, 411, 783, 789`) — just apply it consistently.

### 8. `.clone()` audit (Ch.1 §1.1)

8 `.clone()` calls in `src/blocks/agent.rs`. Most are unavoidable (sending Strings into JSON values, building owned `Attachment`s for dispatch). One worth checking:

- `src/blocks/agent.rs:591` clones `Attachment` to put into the outgoing map. `Attachment` contains a `Vec<u8>` — that's the upload bytes. If `staged_uploads` is consumed exactly once per request (it is), `decode_uploads` could return owned items and `run_skill_dispatch` could take `Vec<(String, Attachment, String)>` by value rather than borrowing then cloning.

Low magnitude though — uploads are capped at 10 MiB and there's exactly one per dispatch.

### 9. `parse_skill_response` / `is_prompt_shaped` could return `Option<...>` from a single chain (Ch.1 §1.3)

`src/blocks/agent.rs:782-800` and `:398-415` have multiple `let Some(..) else { return ... }` blocks. They're readable as-is — leave them alone. But `is_prompt_shaped` could use `?` on Option since it returns `bool` from a unit `()` — minor.

### 10. Tests use `.expect("ok")` everywhere (Ch.5)

Lines like `match a.source().expect("ok") { Source::Url(u) => ..., _ => panic!("expected Url") }` (`image-resize:478`, `image-convert:401`, all the image/video blocks). Apollo Ch.5 says one assertion per test, and Ch.4 explicitly allows `expect` in tests — so this is fine. But the `_ => panic!` pattern is what `assert_matches!` is for:

```rust
assert!(matches!(a.source(), Ok(Source::Url(ref u)) if u == "u"));
```

Cosmetic; do this only when touching the tests anyway.

### 11. JSON-shaped Args have no schema validation (Ch.7 — type state)

Every block has `#[derive(Deserialize)] struct Args { url: Option<String>, ref: Option<String>, ... }` and then a runtime `Args::source()` that rejects illegal combinations. The JSON schema in the `#[wafer_block(... parameters = r#"..."#)]` attribute is the source of truth (with `"oneOf": [{"required":["url"]},{"required":["ref"]}]`) — but it's only enforced by the LLM extraction path, not by the deserializer.

Apollo Ch.7 calls out the type-state pattern: model "exactly one of url|ref" as an enum at the deserialization layer (`#[serde(untagged)]` on an enum), so `Args::source()` becomes a no-op. This is a real improvement, but only worth it once you have the shared block crate from #1.

## Low / nit

- **Imports ordering** (Ch.1 §1.7): generally fine, follows `std → external → crate`. Add the rustfmt config Apollo recommends to enforce:

  ```toml
  # rustfmt.toml
  group_imports = "StdExternalCrate"
  imports_granularity = "Crate"
  ```

- **`src/blocks/agent.rs:163`**: `serde_json::to_vec(&entries).unwrap_or_else(|_| b"[]".to_vec())` — this can never fail (we just built `entries` from `serde_json::Value`). Use `.expect("serde_json::Value always serializes")` (Apollo Ch.4: `expect` is fine "when failure is impossible").

- **`src/blocks/agent.rs:609`**: same — `serde_json::to_vec(&params).unwrap_or_else(|_| b"{}".to_vec())`. The `params` is a `serde_json::Value`; serialization is infallible.

- **`src/config.rs:151`**: `bytes.iter().map(|b| format!("{b:02x}")).collect()` allocates 32 small Strings then joins. Use the `hex` crate or write to a stack `String` with `write!`. Negligible (runs once at boot).

## Summary

The code is in good shape overall — well-structured, decent test coverage (especially `src/blocks/agent.rs` tests), clippy-clean. Biggest concrete wins, in order:

1. **Extract `blocks/_common/`** to kill ~500 lines of duplicated helpers (Ch.1, Ch.3).
2. **Replace `Result<_, String>` and match-chains with `thiserror` + `?`** in the skill blocks — would shrink the image/video crates by ~30–40% (Ch.4 §4.3, §4.5).
3. **Fix the one clippy warning** at `src/blocks/agent.rs:427` (`is_none_or`).
4. **Drop `#[allow(dead_code)]` attrs** once #1 lands (they're a smell, not a fix).

(1) and (2) are the same refactor and probably one PR; (3) is a 1-line follow-up.

---

## Resolution status — 2026-05-15

Worked through this doc in eight PRs over a follow-up session. Two merged; the rest are open. Below: disposition of each finding above, plus items this doc didn't catch.

### Per-finding disposition

| # | Finding | Status | Where |
|---|---|---|---|
| Clippy | `is_none_or` warning at `agent.rs:427` | ✅ Resolved, but inverted — see "Trade-offs" below | PR `chore/nice-to-haves` |
| 1 | Cross-block duplication | ✅ Done | `blocks/_common/` materialised as `block-utils/` — PR #52 (merged) extracted `fetch_from_url` / `load_from_attachment` / `AssetKind` / 6 image+video blocks now share it. PR `refactor/small-cleanups` added `default_filename_for_mime`, `validate_quality_1_100`, `replace_extension`. |
| 2 | `#[allow(dead_code)]` on helpers | ✅ Largely fixed | Once helpers moved into `block-utils`, the per-block crates only need a crate-level `#![cfg_attr(not(target_arch = "wasm32"), allow(dead_code, unused_imports))]` to silence host-side unused-import warnings for the macro-emitted impls. That single crate attr is documented inline as the wafer_block-macro side effect. |
| 3 | `match { Ok/Err => return }` chains | ✅ Done | PR #52 introduced `block_utils::SkillError` + `SkillResultExt::invalid_args(block)`. Every image/video block's `run()` now uses `?`. `image-resize` shrank from ~500 LOC to ~245. |
| 4 | `Result<_, String>` callers | 🟡 Partial | `decode_uploads`, `openai_json_to_chat_message`, and the LLM-side error helpers now use a proper `AgentError` enum (PR #53 agent.rs split moved them into `agent/uploads.rs` / `agent/messages.rs` / `agent/slash.rs`). `pick_source`, `collect_chat_text`, and `ParamExtraction::Error` still return `String` — deferred (cost > benefit for these short-lived strings). |
| 5 | `unsafe impl Send + Sync` on `BrowserFfmpegService` | ✅ Done — but the opposite direction this doc suggested | PR `chore/nice-to-haves` deleted both impls. They were dead code: `wafer_block::compat::MaybeSend`/`MaybeSync` blanket-implement for any `?Sized` type on wasm32, so a unit struct satisfies them auto-magically. The "promote to SAFETY comment" advice in this doc was right that the comment was good, but missed that the impl itself was unnecessary. |
| 6 | Module doc comments doing too much | ❌ Deferred | The `agent.rs:1-37` route-table doc moved verbatim into `agent.rs` after the agent split (PR #53) — still 37 lines at top of file. Pulling protocol prose to `docs/architecture/` is in scope but didn't make this round. |
| 7 | `let-else` opportunities | ❌ Deferred | Stylistic only. |
| 8 | `.clone()` audit | ❌ Deferred | Stylistic; the doc itself agreed "Low magnitude". |
| 9 | `parse_skill_response` / `is_prompt_shaped` chains | ❌ Skipped by design | Doc said "leave them alone". Confirmed during the agent split — they read fine. |
| 10 | Tests use `.expect("ok")` + `_ => panic!()` | ❌ Deferred | Cosmetic. |
| 11 | JSON Args type-state | ❌ Deferred | Doc said "only worth it once you have the shared block crate from #1". The shared crate now exists; this remains a real follow-up but didn't make this round. |
| Nit | rustfmt `group_imports` / `imports_granularity` | ❌ Not done | Worth adding to a rustfmt.toml in a small follow-up. |
| Nit | `.unwrap_or_else(\|_\| b"[]")` → `.expect(...)` | ❌ Reverted, see "Trade-offs" | PR `refactor/small-cleanups` deliberately went the OTHER direction. |
| Nit | hex encoding via `hex` crate | ✅ Done | PR `refactor/hex-encoder` (stacked on `refactor/bootstrap-robustness`) — added `hex = "0.4"` dep and a `random_secret_hex(key)` helper that pairs `getrandom` + `hex::encode`. |

### Newly surfaced findings (not in this doc)

The follow-up session caught a few items this audit didn't:

1. **Hardcoded MVP auth secrets in `src/lib.rs:137-142`** — a `.block_config("suppers-ai/auth", { JWT_SECRET, ADMIN_EMAIL, ADMIN_PASSWORD, INTERNAL_SECRET })` override that silently shipped the same dev secret to every browser, defeating the auto-generate path that `config.rs::seed_auto_generated()` already runs. Worse: `ADMIN_EMAIL: "admin@gizza.local"` (lib.rs) disagreed with `INSERT OR IGNORE 'admin@solobase.local'` (config.rs), making the DB seed dead code. **Resolution:** PR `refactor/bootstrap-robustness` removes the override; per-browser random secrets come from a new `seed_random_secret()` pass in `config.rs`.

2. **Silent SQL bootstrap failures** — `config.rs::exec_or_warn` logged a warning and continued on every schema-create failure; `db_query_raw` errors fell back to an empty string → empty vars → runtime started with zero config. A real OPFS quota or schema error looked indistinguishable from a healthy boot. **Resolution:** same PR — bootstrap fns now return `Result<_, JsValue>`, `initialize()` propagates with `?`, the Service Worker surfaces the error.

3. **Duplicate model-ID constant** — `agent.rs:DEFAULT_MODEL_ID` and `ui.rs:MVP_MODEL_ID` both hardcoded `"Qwen2.5-1.5B-Instruct-q4f32_1-MLC"`. **Resolution:** moved to `src/blocks/mod.rs::DEFAULT_MODEL_ID` in PR `refactor/small-cleanups`.

4. **`.expect()` on `serde_json::to_vec` on wasm32 hot paths** — `agent.rs::handle_commands` and `agent::dispatch::run_skill_dispatch` both panic-trap on wasm32 if serialization fails (it can't, but `.expect()` is a hard abort with no diagnostic on wasm32). **Resolution:** same PR — both use recoverable `.unwrap_or_else(|_| b"…".to_vec())` fallbacks. This is the conscious inversion of nit #1 above.

5. **`agent.rs` was 1414 lines** — bundled six concerns (slash parsing, LLM extraction, skill dispatch, plain chat, upload decode, SSE encoding). **Resolution:** PR #53 (merged) split into `agent/{slash,dispatch,chat,uploads,messages,sse}.rs` + slimmed-down entry `agent.rs` (404 lines).

6. **Six blocks shipped with zero `#[cfg(test)]` coverage** — `calculator`, `clock`, `ffmpeg`, `imagine`, `web-fetch`, `image-fetch`. **Resolution:** PR `tests/pure-compute-blocks` adds 20 host-runnable unit tests across them following the existing pattern (gate the macro impl wasm32-only, extract pure helpers, test them on host).

7. **Envelope vs flat response shapes were inconsistent** — `image-fetch`/`imagine` emitted `Envelope { _for_llm, _for_ui }`; `web-fetch`/`ffmpeg` emitted flat per-block `ToolResp`. The rule (Envelope iff renderable artifact, flat otherwise) was implicit. **Resolution:** PR `docs/response-shape-convention` formalises the rule in `block-utils/src/lib.rs` with a module-level comment. Every existing block already followed it; the audit gap was just "never written down".

### Trade-offs and disagreements with the original doc

**Clippy `unnecessary_map_or` vs MSRV (clippy bullet + nit):** the doc said `.map_or(true, ...)` should become `.is_none_or(...)` (Rust 1.82+, Oct 2024). The codebase doesn't declare `rust-version` in `Cargo.toml`, so implicit MSRV is "whatever was stable at first build." We reverted to `.map_or(true, ...)` in PR `chore/nice-to-haves` to keep the implicit MSRV permissive. The clippy warning will return until either (a) we add `rust-version = "1.82"` to Cargo.toml, or (b) we add a targeted `#[allow(clippy::unnecessary_map_or)]`. Worth picking one in a follow-up.

**`.unwrap_or_else` vs `.expect` for infallible serialization (nit):** the doc said `.expect("serde_json::Value always serializes")` is fine because Apollo Ch.4 allows `.expect()` when failure is impossible. We went the other way in `refactor/small-cleanups`: on wasm32, `.expect()` is a hard panic-trap with no diagnostic, so `.unwrap_or_else(|_| b"…".to_vec())` produces an empty body that the caller handles instead. Defensible either way; we prioritised wasm32-runtime resilience over expressiveness about invariants.

### What's deferred

For a future audit pass:

- `Result<_, String>` in `pick_source`, `collect_chat_text`, `ParamExtraction::Error` (item #4, partial)
- Module-doc-too-verbose (item #6) — move protocol prose to `docs/architecture/agent-block.md`
- JSON Args type-state via `#[serde(untagged)]` (item #11) — now that `block-utils` exists, this is a clean fit
- rustfmt.toml with `group_imports` + `imports_granularity` (nit)
- Decide on explicit `rust-version` and resolve the clippy `is_none_or` tension

### Eight PRs

| Branch | Status | Net LOC | Findings closed |
|---|---|---|---|
| `refactor/fetch-dedup` (#52) | ✅ merged | −269 | #1, #2 partial, #3 |
| `refactor/split-agent-rs` (#53) | ✅ merged | +25 (1414→404 largest file) | newly-surfaced #5 |
| `refactor/small-cleanups` | 🟡 open | +60 | #5 dedup, #7 helper, #9 helpers, newly-surfaced #3, #4 |
| `refactor/bootstrap-robustness` | 🟡 open | +71 | newly-surfaced #1, #2 |
| `tests/pure-compute-blocks` | 🟡 open | +331 | newly-surfaced #6 |
| `docs/response-shape-convention` | 🟡 open | +42 (doc-only) | newly-surfaced #7 |
| `chore/nice-to-haves` | 🟡 open | −1 net | #5 unsafe-impl + #13/`is_none_or` revert |
| `refactor/hex-encoder` (stacks on bootstrap) | 🟡 open | −9 net | nit hex |

Plus one upstream PR on `wafer-run/wafer-run` (`fix/dedup-build-increment-field-where`) — duplicate `build_increment_field_where` defined twice; blocks every downstream gizza-ai CI run until merged.
