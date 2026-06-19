# Shared Tool Abstraction — Plan 3: query-param deep-linking + auto-run + docs

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Every tool page can be opened pre-filled and auto-run via URL query parameters named by the tool's inputs — e.g. `/tools/calculator/?expr=2%2B2*3` shows the result immediately; media tools accept `?url=` to fetch and run. The accepted params + an example deep-link are documented on the page and in the markdown twin so LLMs can drive tools by URL.

**Architecture:** Pure additions to the existing page runtime — the generator already emits `window.GIZZA_TOOL` (input names + elementIds) and `site/tool.js` drives the page from it, so query-params need only (1) a small pure `queryPrefill` helper, (2) wiring it into `tool.js`'s two paths (pure-text auto-run; ffmpeg field-prefill + `?url=` fetch), and (3) the two doc generators. **No descriptor dependency.**

**Tech Stack:** Vanilla ES modules (`site/*.js`), Rust generator (maud + plain string for markdown), Playwright (headless, per the build-notes caveat).

**Spec:** `docs/superpowers/specs/2026-06-19-gizza-shared-tool-abstraction-design.md` §6.

## Re-sequencing note (from the code map)

The spec's Plan 3 also bundled "generator derives the page form from `descriptor.json` (replacing `meta.toml [[input]]`) + slim `meta.toml`." The code map shows the generator already builds the page + `window.GIZZA_TOOL` from `meta.toml`'s `[[input]]`, and the query-param feature works on that existing config. The descriptor-derived page form requires every page tool to have a `core::descriptor()` (i.e. the retrofit), so it is **moved into Plan 4** (where each tool gains its descriptor and emits `descriptor.json`; the generator then sources `[[input]]` from the descriptor and `meta.toml` slims to chrome). Plan 3 ships the user-facing deep-link feature now; the query-param names are the input names either way, so nothing here changes when Plan 4 lands.

## Global Constraints

- **Repo:** `gizza-ai`. Query-param names == input `name` from `window.GIZZA_TOOL` (== `meta.toml [[input]].name`). `source="clock"` inputs are never query-params; `source="file"` inputs are driven by `?url=`.
- **No magic:** the same input `name` is the page field id suffix, the chat-schema property, and the URL query param (consistent with the descriptor design).
- **Auto-run:** pure-text tools auto-run once prefilled (reusing the existing initial `compute()`); media tools auto-run only when `?url=` resolves to a file (scalar-only query just prefills and waits).
- **`?url=` is best-effort:** cross-origin fetch may be blocked; on failure show a clear, non-fatal message and fall back to manual upload. Never a broken state.
- **CI:** gizza CI runs `cargo test` (generator suite included) + a Node test step (`js/*.test.js`). Playwright `tests/fixtures.ts` throws on import in this env (pre-existing) — verify pages with a standalone `@playwright/test` headless spec serving the built `pkg/tools/<slug>/` (see `docs/checks/2026-06-18-gizza-new-tool-build-notes.md` "Playwright").

---

### Task 1: Pure `queryPrefill` helper + Node unit test + generator copies it

**Files:**
- Create: `site/query-prefill.js`
- Create: `js/query-prefill.test.js`
- Modify: `tools/generator/src/main.rs` (copy `query-prefill.js` into each `pkg/tools/<slug>/`)
- Test: `js/query-prefill.test.js`

**Interfaces:**
- Produces: `export function queryPrefill(inputs, searchString) -> { fields: [{elementId, value}], url: string|null }`. `inputs` is `cfg.inputs` (each `{name, source, elementId}`). Consumed by `tool.js` (Tasks 2-3). Pure (no DOM/window) → Node-testable.

- [ ] **Step 1: Write the failing test** — `js/query-prefill.test.js`:

```js
import { test } from "node:test";
import assert from "node:assert/strict";
import { queryPrefill } from "../site/query-prefill.js";

const inputs = [
  { name: "expr", source: "field", elementId: "in-expr" },
  { name: "mode", source: "field", elementId: "in-mode" },
  { name: "unix", source: "clock", elementId: "in-unix" },
  { name: "image", source: "file", elementId: "in-image" },
];

test("prefills only field inputs present in the query", () => {
  const r = queryPrefill(inputs, "?expr=2%2B2&mode=decode&unix=123&nope=x");
  assert.deepEqual(r.fields, [
    { elementId: "in-expr", value: "2+2" },
    { elementId: "in-mode", value: "decode" },
  ]);
  assert.equal(r.url, null); // no ?url= given; clock ignored; unknown ignored
});

test("captures ?url= for media tools", () => {
  const r = queryPrefill(inputs, "?url=https%3A%2F%2Fx.test%2Fcat.jpg&fit=cover");
  assert.equal(r.url, "https://x.test/cat.jpg");
  assert.deepEqual(r.fields, []); // 'fit' isn't a declared input here
});

test("empty search yields nothing", () => {
  const r = queryPrefill(inputs, "");
  assert.deepEqual(r.fields, []);
  assert.equal(r.url, null);
});
```

- [ ] **Step 2: Run to verify it fails**

Run: `node --test js/query-prefill.test.js 2>&1 | tail -15`
Expected: FAIL — cannot find module `../site/query-prefill.js`.

- [ ] **Step 3: Implement `site/query-prefill.js`**

```js
// Pure URL-query → field-value mapping for tool pages. No DOM/window access so
// it is Node-unit-testable; tool.js applies the result to the page.
export function queryPrefill(inputs, searchString) {
  const params = new URLSearchParams(searchString || "");
  const fields = [];
  for (const inp of inputs || []) {
    if (inp.source === "field") {
      const v = params.get(inp.name);
      if (v != null) fields.push({ elementId: inp.elementId, value: v });
    }
  }
  return { fields, url: params.get("url") };
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `node --test js/query-prefill.test.js 2>&1 | tail -10`
Expected: `# pass 3`.

- [ ] **Step 5: Make the generator copy it to each tool page**

In `tools/generator/src/main.rs`, in the per-tool copy block (alongside `copy_file(&root.join("site/tool.js"), &out.join("tool.js"))?;`), add:

```rust
        copy_file(&root.join("site/query-prefill.js"), &out.join("query-prefill.js"))?;
```

- [ ] **Step 6: Commit**

```bash
git -C /home/joris/Programs/suppers-ai/workspace/gizza-ai add site/query-prefill.js js/query-prefill.test.js tools/generator/src/main.rs
git -C /home/joris/Programs/suppers-ai/workspace/gizza-ai commit -m "feat(tools): queryPrefill helper + generator copies it to each page

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 2: Wire prefill + auto-run into `tool.js` (pure-text path)

**Files:**
- Modify: `site/tool.js`
- Test: `tests/tool-page-query-params.spec.ts` (committed for the repo) + a standalone headless verification

**Interfaces:**
- Consumes: `queryPrefill` (Task 1). On load, prefills `field` inputs from `location.search` before the existing initial `compute()` (which then auto-runs and renders).

- [ ] **Step 1: Add the import** at the top of `site/tool.js`:

```js
import { queryPrefill } from "./query-prefill.js";
```

- [ ] **Step 2: Prefill before the initial compute()**

In the non-ffmpeg path, immediately BEFORE the wiring/initial-compute block (the code at lines ~117-131 that wires `input` listeners and calls `compute()`), insert:

```js
  // Deep-link: prefill fields from the URL query, then the initial compute()
  // below auto-runs with those values. Param names == input names.
  for (const f of queryPrefill(cfg.inputs, location.search).fields) {
    const el = document.getElementById(f.elementId);
    if (el) el.value = f.value;
  }
```

(The existing `for (const inp of cfg.inputs) { if field … addEventListener("input", compute) }` and the trailing `compute()` are unchanged — prefilling before them makes the initial `compute()` render the deep-linked result.)

- [ ] **Step 3: Commit a Playwright spec following the repo pattern**

Create `tests/tool-page-query-params.spec.ts` mirroring an existing `tests/tool-page-*.spec.ts` (same import of `tests/fixtures.ts`, same python `webServer` serving the built `pkg/`). Assert: navigating `/tools/calculator/?expr=2%2B2*3` (use calculator's real input name — confirm via `blocks/calculator/page/meta.toml`) results in `#tool-output` containing the computed value; and `/tools/url-encode/?text=a%20b&mode=encode` yields the encoded output.

- [ ] **Step 4: Verify yourself (standalone headless — the repo `fixtures.ts` is broken)**

Per the build-notes: build the pages, then drive them with a standalone `@playwright/test` headless spec + a tiny temp config serving `pkg/`. Concretely:
```
# build calculator + url-encode pages
bash -c 'cd /home/joris/Programs/suppers-ai/workspace/gizza-ai && wasm-pack build blocks/calculator/web --target web --release --out-dir pkg'
bash -c 'cd /home/joris/Programs/suppers-ai/workspace/gizza-ai && wasm-pack build blocks/url-encode/web --target web --release --out-dir pkg'
# (build every page block's web/pkg, then:) cargo run --manifest-path tools/generator/Cargo.toml -- .
```
Then a temp spec navigates `http://localhost:PORT/tools/calculator/?expr=2%2B2*3` and asserts `#tool-output` text is `8`. Remove temp files after. Record the actual observed output in the commit/PR.

- [ ] **Step 5: Commit**

```bash
git -C /home/joris/Programs/suppers-ai/workspace/gizza-ai add site/tool.js tests/tool-page-query-params.spec.ts
git -C /home/joris/Programs/suppers-ai/workspace/gizza-ai commit -m "feat(tools): query-param prefill + auto-run on pure-text tool pages

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 3: ffmpeg path — field prefill + `?url=` fetch + CORS fallback

**Files:**
- Modify: `site/tool.js` (the `cfg.runtime === "ffmpeg"` branch, ~lines 46-96)
- Test: extend the standalone headless verification (Task 2) for a media tool

**Interfaces:**
- Consumes: `queryPrefill`. Prefills `field` inputs; if `?url=` present, fetches it into the file input and triggers `run()`. On fetch/CORS failure, calls `showError(...)` with the documented message and leaves the manual file input usable.

- [ ] **Step 1: Add a fetch-into-file-input helper + prefill + auto-run**

In the ffmpeg branch of `site/tool.js`, after `fieldInputs`/`fileInput` are resolved and `run()` is defined (and before/replacing the final event wiring), insert:

```js
  // Deep-link: prefill scalar fields from the query.
  const { fields: qpFields, url: qpUrl } = queryPrefill(cfg.inputs, location.search);
  for (const f of qpFields) {
    const el = document.getElementById(f.elementId);
    if (el) el.value = f.value;
  }

  // ?url= → fetch the remote media into the file input, then auto-run.
  async function loadUrlIntoFile(url) {
    try {
      const resp = await fetch(url);
      if (!resp.ok) throw new Error("HTTP " + resp.status);
      const blob = await resp.blob();
      const name = (url.split("/").pop() || "input").split("?")[0] || "input";
      const dt = new DataTransfer();
      dt.items.add(new File([blob], name, { type: blob.type }));
      fileInput.files = dt.files;
      return true;
    } catch (e) {
      showError(
        "Couldn't fetch " + url + " — the host may block cross-origin access. " +
        "Download it and choose the file instead."
      );
      return false;
    }
  }
  if (qpUrl && fileInput) {
    loadUrlIntoFile(qpUrl).then((ok) => { if (ok) run(); });
  }
```

(The existing `fileInput.addEventListener("change", run)` and field `input` listeners remain — manual use still works, and a successful `?url=` fetch triggers `run()` directly.)

- [ ] **Step 2: Verify (standalone headless)**

Build a media page (e.g. `image-resize`) and drive `…/tools/image-resize/?width=64&url=<same-origin-fixture-image>` — assert `#tool-output-media` becomes visible with a `data:` src. For the CORS path, point `?url=` at a host that blocks CORS and assert `#tool-output` shows the fallback message and `#tool-output-media` stays hidden. Use a fixture image served by the same temp `webServer` for the success case to avoid external flakiness. Record observed results.

- [ ] **Step 3: Commit**

```bash
git -C /home/joris/Programs/suppers-ai/workspace/gizza-ai add site/tool.js
git -C /home/joris/Programs/suppers-ai/workspace/gizza-ai commit -m "feat(tools): ?url= deep-link fetch + field prefill on ffmpeg tool pages

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

### Task 4: Document the query params — markdown twin + page section

**Files:**
- Modify: `tools/generator/src/markdown.rs` (`tool_markdown`)
- Modify: `tools/generator/src/template.rs` (`render_page`)
- Test: `tools/generator/` test suite (add tests, or extend existing)

**Interfaces:**
- Produces: a "Query parameters" section in `index.md` and a "Use via URL" section on the page, both listing the field params (+ `url` for media) and a concrete example deep-link, derived from `meta.inputs`.

- [ ] **Step 1: Write the failing generator test**

In the generator's test module (find it: `grep -rn "#\[cfg(test)\]" tools/generator/src/`), add a test that builds a `ToolMeta` with two `field` inputs and asserts `tool_markdown(&meta, "")` contains `"## Query parameters"`, each param name, and an example URL containing `/tools/<slug>/?`. (Mirror how existing generator tests construct a `ToolMeta` — reuse their helper/fixture.)

- [ ] **Step 2: Run to verify it fails**

Run: `cargo test --manifest-path tools/generator/Cargo.toml query 2>&1 | tail -12`
Expected: FAIL — the section/string is absent.

- [ ] **Step 3: Add an `example_deeplink` helper + the markdown section**

In `tools/generator/src/markdown.rs`, add a helper and a section. Insert the section between the "## Output" block and the `"---\n\n"` separator in `tool_markdown`:

```rust
fn example_deeplink(meta: &ToolMeta) -> String {
    let mut pairs: Vec<String> = Vec::new();
    for i in &meta.inputs {
        if i.source == "file" {
            pairs.push("url=https://example.com/input".to_string());
        } else if i.source == "field" {
            let sample = if i.placeholder.is_empty() { "value" } else { &i.placeholder };
            pairs.push(format!("{}={}", i.name, urlencode(sample)));
        }
    }
    format!("{}/tools/{}/?{}", SITE, meta.slug, pairs.join("&"))
}

/// Minimal percent-encoding for example URLs (space, &, =, +, non-ASCII).
fn urlencode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    out
}
```

Then in `tool_markdown`, before the `"---\n\n"` line:

```rust
    let qp: Vec<&Input> = meta.inputs.iter().filter(|i| i.source == "field").collect();
    let has_file = meta.inputs.iter().any(|i| i.source == "file");
    if !qp.is_empty() || has_file {
        s.push_str("## Query parameters\n\n");
        s.push_str("Open the tool pre-filled and auto-run via URL:\n\n");
        for i in &qp {
            let label = if i.label.is_empty() { i.name.as_str() } else { i.label.as_str() };
            s.push_str(&format!("- `{}` — {}\n", i.name, label));
        }
        if has_file {
            s.push_str("- `url` — fetch the input file from a public URL (CORS-permitting)\n");
        }
        s.push_str(&format!("\nExample: `{}`\n\n", example_deeplink(meta)));
    }
```

(Confirm `SITE` is in scope in `markdown.rs` — the map shows it is, used by the "## Run it" web URL.)

- [ ] **Step 4: Add the page "Use via URL" section**

In `tools/generator/src/template.rs` `render_page`, after the inputs/output block and before the prose/content section, add a maud block rendering the same param list + example (reuse `example_deeplink` — make it `pub(crate)` in `markdown.rs` and `use` it, or duplicate the tiny logic in `template.rs`; prefer the shared `pub(crate)` import to avoid drift). Render a `<details>`/section with a copyable `<code>` example. Keep it visually minor (it's a power-user/AI affordance).

- [ ] **Step 5: Run to verify pass**

Run: `cargo test --manifest-path tools/generator/Cargo.toml 2>&1 | tail -12`
Expected: all generator tests pass, including the new query-params test.

- [ ] **Step 6: Commit**

```bash
git -C /home/joris/Programs/suppers-ai/workspace/gizza-ai add tools/generator/src/markdown.rs tools/generator/src/template.rs
git -C /home/joris/Programs/suppers-ai/workspace/gizza-ai commit -m "feat(tools): document query-param deep-links in index.md + on the page

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
```

---

## Final verification

- [ ] `node --test js/query-prefill.test.js` → pass.
- [ ] `cargo test --manifest-path tools/generator/Cargo.toml` → pass.
- [ ] Standalone headless: `/tools/calculator/?expr=2%2B2*3` auto-renders the result; a media tool `?url=<fixture>` fetches+runs; `?url=<cors-blocked>` shows the fallback message. Record observed output.
- [ ] Built `pkg/tools/<slug>/` contains `query-prefill.js` (generator copy) and `index.md` has the "Query parameters" section.

**Done when:** deep-linking works on a pure tool and a media tool, the docs are generated, and the generator + JS tests are green. **Handoff:** Plan 4 (retrofit) gives each tool a `core::descriptor()`, wires Plan 1's expr `parameters` + Plan 2's helpers, emits `descriptor.json`, and switches the generator to source `[[input]]` from the descriptor (slimming `meta.toml`) — query-params keep working unchanged because the names are identical.
