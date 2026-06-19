# Shared Tool Abstraction — Plan 4: retrofit all ~25 tools (recipe + batched rollout)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans. This plan is a **recipe applied per tool**, not 25 hand-written task specs — each tool follows the matching shape recipe below, exemplar-first then batched.

**Goal:** Convert every tool to the single-source abstraction: a `core::descriptor()` drives the chat schema (via Plan 1's `parameters = core::schema_json()`), and the per-tool wrapper/orchestration collapses into Plan 2's helpers (`run_skill`, `respond_ok`, `build_media_envelope`, `resolve_source`, `dispatch_ffmpeg`, `ArgvPlan`). Less code, one schema source, uniform errors — across all tools.

**Architecture:** Per tool, edit `core` (add `descriptor()` + `schema_json()`), `src/lib.rs` (delete the inline JSON schema + hand-written `handle`/`respond_error`; delegate to helpers), and `web/src/lib.rs` (use `ArgvPlan` for ffmpeg). A **drift-guard test** per tool asserts the derived schema matches the current authored schema so LLM tool-calling quality is preserved.

**Tech Stack:** Rust (block crates + block-utils), the Plan 1 macro feature, Plan 2 helpers.

**Spec:** `docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md` §1/§4 (retrofit) + the migration-safety gate.

## Prerequisites (DONE)

- Plan 1 on **origin** wafer-run main (`48926f4`), Plan 2 + Plan 3 on **origin** gizza main (`b3bb642`). So retrofit PRs' gizza CI (clones wafer-run main, builds gizza) has the macro feature + helpers. ✅ pushed 2026-06-19.

## Scope decision (read before executing)

This plan does the **chat-side** retrofit (the bulk of the code reduction + the single-source chat schema) and **keeps `meta.toml [[input]]` as the page-form source**, guarded by a per-tool test that the page inputs stay consistent with `core::descriptor()`. 

**Deferred to Plan 4b (optional):** fully eliminating `meta.toml [[input]]` by having the generator render the page form from an emitted `descriptor.json`. That needs a descriptor.json-emission mechanism (per-tool `build.rs` writing the file, or a small workspace binary) + a generator rewrite. It removes the *remaining* page-side duplication but is mechanically heavier and not required for the chat-schema single-sourcing or the code reduction. **Decide 4b after 4 lands** — the drift-guard test below keeps page↔descriptor honest in the meantime, so there is no correctness gap.

## Global Constraints

- **Repo:** `gizza-ai`. One PR per batch (~4–6 tools), worktree-per-batch (the build-program pattern; see the build-notes + [[workflow-durability-protocol]]).
- **Migration safety (hard gate):** for each tool, the schema produced by `core::schema_json()` must be **semantically equal** to that tool's *current* authored `parameters` JSON (same property names, types, enums, defaults, `required`, and the `url`⊕`ref` `oneOf`). A per-tool test asserts this. Descriptions may be reworded only intentionally.
- **Error unification:** replace any `{ "error": … }`-as-200 path (e.g. url-encode's `respond_error`) with `Err(SkillError) → GuestResult::error(e.into())`.
- **Additive-only to shared crates:** do not change `block-utils` public items here (Plan 2 is frozen); only consume them.
- **Per-tool tests stay green:** existing `core`/block unit tests + the tool's behavior must be unchanged.

## The drift-guard / migration test (every tool)

In each tool's `core` (native-testable), add:

```rust
#[test]
fn schema_json_matches_authored_chat_schema() {
    // Paste the tool's CURRENT authored `parameters` JSON here verbatim.
    let authored: serde_json::Value = serde_json::from_str(r#"{ …current schema… }"#).unwrap();
    let derived: serde_json::Value = serde_json::from_str(&descriptor().to_schema_json()).unwrap();
    assert_eq!(derived, authored, "derived schema must match the authored one (no LLM-facing drift)");
}
```

Author `descriptor()` until this passes — that *is* the proof the migration is lossless. (Object key order is irrelevant: `serde_json::Value` compares maps structurally.)

---

## Recipe P — pure-text tool (exemplar: `url-encode`)

**Before** (`blocks/url-encode/src/lib.rs`): an `Args` struct, a big inline `parameters` JSON literal in `#[wafer_block(skill(...))]`, a `handle` that matches Ok/Err, and a re-defined `respond_error` returning `{error}` as a 200.

**After:**

1. **`core/src/lib.rs`** — add:

```rust
use gizza_ai_block_utils::{ToolDescriptor, Input, Param};

pub fn descriptor() -> ToolDescriptor {
    ToolDescriptor::new(Input::None)
        .param(Param::string("text").required()
            .describe("The text or URL to encode or decode."))
        .param(Param::enumv("mode", ["encode", "decode"]).default("encode")
            .describe("Direction: 'encode' (default) percent-encodes, 'decode' reverses it."))
        .param(Param::enumv("target", ["component", "uri"]).default("component")
            .describe("Encode mode only: 'component' (default) escapes reserved chars for a single value; 'uri' preserves URL delimiters. Ignored when decoding."))
}

pub fn schema_json() -> String { descriptor().to_schema_json() }
```
…then the drift-guard test (paste url-encode's current authored schema).

2. **`src/lib.rs`** — replace the body with:

```rust
#[wafer_block(
    name = "gizza-ai/url-encode", version = "0.1.0", interface = "handler@v1",
    summary = "URL Encode skill",
    skill(description = "…unchanged…", parameters = gizza_ai_url_encode_core::schema_json())
)]
impl UrlEncode {
    fn handle(_msg: Message, body: Vec<u8>) -> GuestResult {
        // `run_skill` wraps the returned value in `{ "result": … }` — exactly
        // url-encode's current success shape — so return the encoded String.
        match gizza_ai_block_utils::run_skill(&body, "url-encode", |a: Args| {
            gizza_ai_url_encode_core::convert(&a.text, &a.mode, &a.target)
                .map_err(gizza_ai_block_utils::SkillError::InvalidArgs)
        }) {
            Ok(v) => GuestResult::respond(v),
            Err(e) => GuestResult::error(e.into()),
        }
    }
}
```
Delete the inline schema literal, the old `respond_error`, and the manual parse. Keep `Args` (typed deserialize). The exact current result shape is preserved by `run_skill`'s `{ "result": … }` wrapping — confirm against each tool's current output (some return `{result}`, media tools return an `Envelope` via Recipe M).

3. **`web/src/lib.rs`** — unchanged for pure tools (it already forwards to `core`).

4. **Verify:** `cargo test` in core (drift-guard + existing) + `wafer build` in the block + a CLI smoke (`gizza tool url-encode 'text=a b'`).

**Other pure tools:** `phone-format`, `word-count`, `calculator`, `clock` (clock = `Input::None`, no params).

---

## Recipe M — media / ffmpeg-page tool (exemplar: `image-resize`)

**Before:** inline schema with `url`/`ref`/`width`/`height`/`fit` + `oneOf`; a `run()` that does `pick_source → fetch/load → build argv → FfmpegReq → dispatch → exit/size checks → Envelope` by hand; `web/src/lib.rs` with a local `struct Plan`.

**After:**

1. **`core/src/lib.rs`** — add `descriptor()` (`Input::Image`, params `width`/`height` `Param::integer().min(1.0)`, `fit` enum default `contain`) + `schema_json()` + drift-guard test (paste the current schema, incl. the `oneOf`).

2. **`src/lib.rs`** `run()` collapses to:

```rust
fn run(body: Vec<u8>) -> Result<Vec<u8>, SkillError> {
    let args: Args = serde_json::from_slice(&body).invalid_args("image-resize")?;
    // tool-specific validation stays (width/height required, fit=cover needs both)…
    let (bytes, mime, in_name) = resolve_source(args.source.into_inner(), AssetKind::Image, MAX_INPUT_BYTES)?;
    let ext = mime_to_ext(&mime).ok_or_else(|| SkillError::InvalidArgs(format!("unsupported input mime: {mime}")))?;
    let (argv, out_name) = core::plan_resize(&format!("in.{ext}"), &format!("out.{ext}"), args.width, args.height, fit);
    let output = dispatch_ffmpeg(argv, format!("in.{ext}"), bytes, out_name)?;
    let filename = filename_with_suffix(&in_name, &dim_suffix(args.width, args.height), ext);
    build_media_envelope(&output, &mime, filename, summary(&in_name, args.width, args.height, output.len(), &mime), MAX_OUTPUT_BYTES)
}
```
i.e. `resolve_source` + `dispatch_ffmpeg` + `build_media_envelope` (Plan 2) replace ~50 lines of hand-orchestration; `parameters = core::schema_json()`.

3. **`web/src/lib.rs`** — return `gizza_ai_block_utils::ArgvPlan { argv, out_name }` instead of a local `struct Plan`.

4. **Verify:** core tests + `wafer build` + page build (`wasm-pack` + generator) + a CLI smoke against a public image URL.

**Other media tools:** `image-compress`, `image-convert`, `image-crop`, `image-grayscale`, `image-fetch`, `video-compress`, `video-transcode`, `video-trim`, `video-frame-extract`.

---

## Recipe N — no-page chat+CLI / network tool (exemplar: `web-fetch`)

**Before:** `Args` + a typed `ToolResp` + inline schema + hand-written `handle`.

**After:** add `core::descriptor()` (`Input::None` for param-only tools like web-fetch/http-request; `Input::Document`/`File` for byte-input tools like pdf-extract-text/xlsx-to-csv/merge-pdf, which emit the `url`⊕`ref` `oneOf`) + `schema_json()` + drift-guard; `src/lib.rs` uses `run_skill(&body, "<slug>", |a: Args| -> Result<ToolResp, SkillError>)` and returns the typed result (wrapped `{result}` by `respond_ok`). Byte-input tools use `resolve_source(..)` with the appropriate `AssetKind`. No page, no `web/` change beyond what already exists.

**Other no-page tools:** `http-request`, `pdf-extract-text`, `xlsx-to-csv`, `merge-pdf`, `vectorize`, `code-screenshot`, `css-select-extract`, `imagine`, `ffmpeg` (ffprobe). (Note: `merge-pdf` takes an **array** of inputs → add a `ParamKind::Array` variant to `block-utils` first, as an additive Plan 2 follow-up, before retrofitting it.)

---

## Batching & sequencing

1. **Exemplar PR (one per shape):** retrofit `url-encode` (P), `image-resize` (M), `web-fetch` (N) in one PR. Proves all three recipes + the drift-guard. Review carefully.
2. **Batch PRs (~4–6 tools each), worktree-per-batch:**
   - Batch P: `phone-format`, `word-count`, `calculator`, `clock`.
   - Batch M1: `image-compress`, `image-convert`, `image-crop`, `image-grayscale`, `image-fetch`.
   - Batch M2: `video-compress`, `video-transcode`, `video-trim`, `video-frame-extract`.
   - Batch N1: `http-request`, `css-select-extract`, `imagine`, `ffmpeg`.
   - Batch N2 (byte-input): `pdf-extract-text`, `xlsx-to-csv`, `vectorize`, `code-screenshot`, then `merge-pdf` (after `ParamKind::Array`).
3. Each batch PR: per tool — recipe edits, drift-guard green, `wafer build`, `cargo test`, fmt; one `gizza tool` CLI smoke per tool. Open PR (review-only), CI green, merge.

This is a candidate for **parallel agents** (one per tool, worktree-isolated) given the uniform recipe — opt into that scale explicitly if desired; otherwise execute batches sequentially.

## Testing / done

- Per tool: drift-guard (`schema_json_matches_authored_chat_schema`) green; existing core/block tests green; `wafer build` ok; one CLI smoke recorded.
- Whole-repo: `cargo test` (with block.wasm artifacts built), generator suite, JS suite — green.
- **Done when** all ~25 tools are on the abstraction, every drift-guard passes (proving no chat-schema drift), and the code-reduction is realized (no inline schema literals, no per-tool `respond_error`, no hand-rolled media orchestration).

## Handoff

After Plan 4: **Plan 5** updates `/new-tool` + `scaffold-tool.sh` + the build-notes to emit the descriptor-based shape so future tools are born consistent. **Plan 4b** (optional) eliminates `meta.toml [[input]]` via generator-from-descriptor.json if the remaining page-side duplication is worth the mechanism.
