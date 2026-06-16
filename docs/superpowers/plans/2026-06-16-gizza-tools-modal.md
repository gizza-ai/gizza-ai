# Tools Modal Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the home page's inline bottom "Tools" list with a hammer button (beside the brain/model picker) that opens a searchable modal, backed by a build-time static `/tools/_index.json`.

**Architecture:** The generator (`tools/generator`) emits `pkg/tools/_index.json` (`[{slug,title,description}]`) alongside the per-tool pages — already covered by the `/tools/` Service Worker bypass. A new `site/tools-modal.js` fetches it once on first open and filters client-side; a new `site/tools-modal.css` styles the hammer icon + modal. The dead `build.rs` `TOOLS` scanner and the inline list in `ui.rs` are removed, leaving the generator as the single source of the tool list.

**Tech Stack:** Rust (the standalone `gizza-tool-pages` generator crate; the gizza-ai `ui` block + `build.rs`), maud templating, vanilla ESM JS (`node:test`), CSS, `solobase build` (wasm-pack + asset bundling driven by `solobase.toml`).

**Branch:** `feat/tools-modal` (already created; the design spec is committed there). All task commits land on this branch; Task 7 opens the PR.

**Reference spec:** `docs/superpowers/specs/2026-06-16-gizza-tools-modal-design.md`

---

### Task 1: Generator emits `pkg/tools/_index.json`

The generator already parses every `blocks/<tool>/page/meta.toml` into a `ToolMeta { slug, title, description, … }` and writes per-tool pages + `pkg/sitemap.xml`. Add a JSON index.

**Files:**
- Create: `tools/generator/src/index.rs`
- Modify: `tools/generator/src/main.rs` (declare the module + emit the file)

- [ ] **Step 1: Write the failing test**

Create `tools/generator/src/index.rs`:

```rust
//! Build-time JSON index of all tool pages, consumed by the in-app tools modal.

use crate::meta::ToolMeta;
use serde::Serialize;

#[derive(Serialize)]
struct IndexEntry<'a> {
    slug: &'a str,
    title: &'a str,
    description: &'a str,
}

/// Serialize `[{slug,title,description}]` for every tool, in the given order.
pub fn tools_index_json(metas: &[ToolMeta]) -> String {
    let entries: Vec<IndexEntry> = metas
        .iter()
        .map(|m| IndexEntry {
            slug: &m.slug,
            title: &m.title,
            description: &m.description,
        })
        .collect();
    serde_json::to_string(&entries).expect("serialize tools index")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn calc() -> ToolMeta {
        ToolMeta::from_toml(
            r#"
slug          = "calculator"
title         = "Free Online Calculator — gizza.ai"
description   = "Evaluate expressions instantly."
h1            = "Free Online Calculator"
hero_subtitle = "Type a math expression."
wasm          = "gizza_ai_calculator_web"
export        = "evaluate"
live          = false
output_label  = "Result"
format        = "number"

[[input]]
name        = "expr"
label       = "Expression"
placeholder = "2 + 2 * 3"
source      = "field"
"#,
        )
        .unwrap()
    }

    #[test]
    fn index_has_slug_title_description() {
        let json = tools_index_json(&[calc()]);
        let v: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(v.is_array());
        assert_eq!(v[0]["slug"], "calculator");
        assert_eq!(v[0]["title"], "Free Online Calculator — gizza.ai");
        assert_eq!(v[0]["description"], "Evaluate expressions instantly.");
    }

    #[test]
    fn empty_metas_is_empty_array() {
        assert_eq!(tools_index_json(&[]), "[]");
    }
}
```

- [ ] **Step 2: Wire the module and run the test to verify it fails to compile (module not declared yet)**

In `tools/generator/src/main.rs`, add the module declaration after `mod template;` (line 10, alongside `mod meta; mod seo; mod template;`):

```rust
mod index;
```

Run: `cargo test --manifest-path tools/generator/Cargo.toml index`
Expected: at first (before adding `mod index;`) FAIL to compile / unknown module; after adding the `mod index;` line, the two `index::tests` pass. Confirm both pass.

- [ ] **Step 3: Emit the file in `main.rs`**

In `tools/generator/src/main.rs`, after the loop that writes per-tool pages and just before/after the sitemap write (where `metas` and `pkg`/`pkg_tools` are in scope), add:

```rust
    // Static index for the in-app tools modal (fetched client-side; lives under
    // /tools/ so it is covered by the runtime SW's /tools/ bypass).
    let metas_only: Vec<ToolMeta> = metas.iter().map(|(_, m)| m.clone()).collect();
    fs::write(
        pkg_tools.join("_index.json"),
        index::tools_index_json(&metas_only),
    )
    .map_err(|e| format!("write tools/_index.json: {e}"))?;
```

(`ToolMeta` derives `Clone`; `pkg_tools` is `root/pkg/tools` and already exists by this point because the per-tool pages were written into it. If `metas` is empty the generator already returns early, so the index is only written when there is at least one tool.)

- [ ] **Step 4: Run the generator tests**

Run: `cargo test --manifest-path tools/generator/Cargo.toml`
Expected: PASS (the existing 6 + the 2 new index tests).

- [ ] **Step 5: Commit**

```bash
git add tools/generator/src/index.rs tools/generator/src/main.rs
git commit -m "feat(gizza-tool-pages): emit pkg/tools/_index.json for the tools modal"
```

---

### Task 2: `tools-modal.js` — fetch + filter + render

**Files:**
- Create: `site/tools-modal.js`
- Create: `js/tools-modal.test.js`

- [ ] **Step 1: Write the failing test**

Create `js/tools-modal.test.js`:

```js
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { filterTools } from '../site/tools-modal.js';

const LIST = [
  { slug: 'calculator', title: 'Free Online Calculator', description: 'Evaluate math expressions' },
  { slug: 'clock', title: 'Current UTC Time', description: 'Live timestamp' },
];

test('empty query returns the full list', () => {
  assert.deepEqual(filterTools(LIST, ''), LIST);
  assert.deepEqual(filterTools(LIST, '   '), LIST);
});

test('matches on title, case-insensitive', () => {
  const r = filterTools(LIST, 'CALC');
  assert.equal(r.length, 1);
  assert.equal(r[0].slug, 'calculator');
});

test('matches on description', () => {
  const r = filterTools(LIST, 'timestamp');
  assert.equal(r.length, 1);
  assert.equal(r[0].slug, 'clock');
});

test('no match returns empty array', () => {
  assert.deepEqual(filterTools(LIST, 'zzznope'), []);
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `node --test js/tools-modal.test.js`
Expected: FAIL — `Cannot find module '../site/tools-modal.js'` (file doesn't exist yet).

- [ ] **Step 3: Write `site/tools-modal.js`**

Create `site/tools-modal.js`:

```js
// Tools modal: a searchable directory of every standalone tool page.
// Triggered by the composer hammer button (#open-tools). The list is a static
// JSON index (/tools/_index.json) emitted at build time and served under the
// /tools/ Service-Worker bypass; we fetch it once on first open and filter
// client-side, so the page itself ships none of it.

const INDEX_URL = '/tools/_index.json';

/** Pure: substring match (case-insensitive) over title + description. */
export function filterTools(list, query) {
  const q = query.trim().toLowerCase();
  if (!q) return list;
  return list.filter(
    (t) => (t.title + ' ' + t.description).toLowerCase().includes(q),
  );
}

function rowHtml(t) {
  const esc = (s) =>
    String(s).replace(/&/g, '&amp;').replace(/</g, '&lt;').replace(/>/g, '&gt;');
  return (
    `<li><a href="/tools/${encodeURIComponent(t.slug)}/">` +
    `<span class="tool-title">${esc(t.title)}</span>` +
    `<span class="tool-desc">${esc(t.description)}</span>` +
    `</a></li>`
  );
}

function initToolsModal() {
  const dialog = document.getElementById('tools-modal');
  const openBtn = document.getElementById('open-tools');
  if (!dialog || !openBtn) return;

  const search = dialog.querySelector('#tools-search');
  const results = dialog.querySelector('#tools-results');
  const empty = dialog.querySelector('#tools-empty');
  const errorBox = dialog.querySelector('#tools-error');
  const closeBtn = dialog.querySelector('#tools-close');
  const retryBtn = dialog.querySelector('#tools-retry');

  let all = null; // cached index once loaded

  function render() {
    if (!all) return;
    const matches = filterTools(all, search.value);
    results.innerHTML = matches.map(rowHtml).join('');
    empty.hidden = matches.length > 0;
  }

  async function load() {
    errorBox.hidden = true;
    if (all) {
      render();
      return;
    }
    results.innerHTML = '';
    try {
      const res = await fetch(INDEX_URL, { cache: 'no-cache' });
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      all = await res.json();
      render();
    } catch (e) {
      console.error('[gizza] tools index load failed:', e);
      errorBox.hidden = false;
    }
  }

  openBtn.addEventListener('click', () => {
    dialog.showModal();
    search.focus();
    load();
  });
  search.addEventListener('input', render);
  closeBtn.addEventListener('click', () => dialog.close());
  retryBtn.addEventListener('click', () => {
    all = null;
    load();
  });
}

// Only wire DOM in a browser — keeps filterTools importable under node:test.
if (typeof document !== 'undefined') {
  if (document.readyState === 'loading') {
    document.addEventListener('DOMContentLoaded', initToolsModal);
  } else {
    initToolsModal();
  }
}
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `node --test js/tools-modal.test.js`
Expected: PASS (4 tests). The `typeof document` guard means importing the module under node does not touch the DOM.

- [ ] **Step 5: Commit**

```bash
git add site/tools-modal.js js/tools-modal.test.js
git commit -m "feat(gizza): tools-modal.js — fetch /tools/_index.json + client-side filter"
```

---

### Task 3: `tools-modal.css` — hammer icon + modal styling

The composer is a 5-column grid (`auto auto auto 1fr auto`); column 2 is free (between the brain at col 1 and attach at col 3). Place the hammer there. Base button sizing/`::before` mask scaffolding is inherited from the existing `#composer button` / `#composer button::before` rules in `gizza.css`, so this file only sets the column, the hammer mask, and the modal.

**Files:**
- Create: `site/tools-modal.css`

- [ ] **Step 1: Write `site/tools-modal.css`**

```css
/* Hammer button in the composer (between brain @col1 and attach @col3). */
#composer #open-tools {
  grid-column: 2;
  background: transparent;
  color: var(--color-muted);
}
#composer #open-tools::before {
  -webkit-mask-image: url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='black' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><path d='m15 12-8.373 8.373a1 1 0 1 1-3-3L12 9'/><path d='m18 15 4-4'/><path d='m21.5 11.5-1.914-1.914A2 2 0 0 1 19 8.172V7l-2.26-2.26a6 6 0 0 0-4.202-1.756L9 2.96l.92.82A6.18 6.18 0 0 1 12 8.4V10l2 2h1.172a2 2 0 0 1 1.414.586L18.5 14.5'/></svg>");
          mask-image: url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='none' stroke='black' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'><path d='m15 12-8.373 8.373a1 1 0 1 1-3-3L12 9'/><path d='m18 15 4-4'/><path d='m21.5 11.5-1.914-1.914A2 2 0 0 1 19 8.172V7l-2.26-2.26a6 6 0 0 0-4.202-1.756L9 2.96l.92.82A6.18 6.18 0 0 1 12 8.4V10l2 2h1.172a2 2 0 0 1 1.414.586L18.5 14.5'/></svg>");
}
#composer #open-tools:hover {
  background: var(--color-canvas-soft);
  color: var(--color-ink);
}

/* Tools modal — reuses the base dialog/::backdrop styling from gizza.css. */
#tools-modal {
  width: min(560px, 92vw);
  max-height: 80vh;
  padding: 0;
  overflow: hidden;
  display: flex;
  flex-direction: column;
}
.tools-modal-head {
  display: flex;
  gap: var(--space-xs);
  align-items: center;
  padding: var(--space-base);
  border-bottom: 1px solid var(--color-hairline-strong);
}
#tools-search {
  flex: 1;
  font: inherit;
  padding: var(--space-sm) var(--space-base);
  border: 1px solid var(--color-hairline-strong);
  border-radius: 10px;
  background: var(--color-canvas-soft);
  color: var(--color-ink);
}
#tools-close {
  border: none;
  background: transparent;
  color: var(--color-muted);
  font-size: 1.1rem;
  cursor: pointer;
  padding: var(--space-xs) var(--space-sm);
  border-radius: 8px;
}
#tools-close:hover { background: var(--color-canvas-soft); color: var(--color-ink); }

.tools-results {
  list-style: none;
  margin: 0;
  padding: var(--space-xs);
  overflow-y: auto;
}
.tools-results li a {
  display: block;
  padding: var(--space-sm) var(--space-base);
  border-radius: 10px;
  text-decoration: none;
  color: inherit;
}
.tools-results li a:hover { background: var(--color-canvas-soft); }
.tools-results .tool-title { display: block; font-weight: 600; color: var(--color-ink); }
.tools-results .tool-desc { display: block; font-size: 0.875rem; color: var(--color-muted); }

.tools-empty, .tools-error {
  padding: var(--space-lg) var(--space-base);
  text-align: center;
  color: var(--color-muted);
}
.tools-error button {
  margin-left: var(--space-xs);
  font: inherit;
  cursor: pointer;
  text-decoration: underline;
  background: none;
  border: none;
  color: var(--color-primary, inherit);
}
```

- [ ] **Step 2: Commit** (CSS is verified visually at integration; no unit test)

```bash
git add site/tools-modal.css
git commit -m "feat(gizza): tools-modal.css — composer hammer icon + modal styling"
```

---

### Task 4: `ui.rs` — hammer button, dialog, head tags; remove the bottom list

**Files:**
- Modify: `src/blocks/ui.rs`

- [ ] **Step 1: Replace the `renders_tools_interlink` test with the modal test**

In `src/blocks/ui.rs`, replace the whole `renders_tools_interlink` test fn (the one asserting `class="gizza-tools"` / `/tools/calculator/`) with this (it obtains the rendered HTML the same way the old test did — `render_chat().into_string()`):

```rust
    #[test]
    fn renders_tools_button_and_modal() {
        let s = render_chat().into_string();
        assert!(s.contains(r#"id="open-tools""#), "hammer button present");
        assert!(s.contains(r#"id="tools-modal""#), "tools modal present");
        assert!(s.contains(r#"id="tools-search""#), "search input present");
        assert!(!s.contains("class=\"gizza-tools\""), "old inline list removed");
    }
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test --lib renders_tools_button_and_modal`
Expected: FAIL — the markup (`open-tools`, `tools-modal`) does not exist yet.

- [ ] **Step 3: Add the head tags**

In `src/blocks/ui.rs` `head { … }`, right after `link rel="stylesheet" href="/model-picker.css";`, add:

```rust
                link rel="stylesheet" href="/tools-modal.css";
```

And where `script type="module" src="/gizza-app.js" {}` is emitted (the body module scripts), add right after it:

```rust
                script type="module" src="/tools-modal.js" {}
```

- [ ] **Step 4: Add the hammer button in the composer**

In the `form#composer`, immediately after the `button id="open-brain-picker" …` line, add:

```rust
                        button id="open-tools" type="button" aria-label="Tools" title="Browse tools" {}
```

- [ ] **Step 5: Remove the inline tools section and the TOOLS include**

Delete the block:

```rust
                @if !TOOLS.is_empty() {
                    section class="gizza-tools" aria-label="Standalone tools" {
                        h2 { "Tools" }
                        ul {
                            @for (slug, title) in TOOLS {
                                li {
                                    a href=(format!("/tools/{slug}/")) { (title) }
                                }
                            }
                        }
                    }
                }
```

and delete the line:

```rust
include!(concat!(env!("OUT_DIR"), "/tools.rs"));
```

(and the `// Generated by build.rs: pub const TOOLS …` comment above it).

- [ ] **Step 6: Add the tools modal dialog**

Next to the other dialogs (e.g. right after the `dialog id="info-dialog" { … }` block), add:

```rust
                // Searchable tools directory — opened by the composer hammer (#open-tools).
                dialog id="tools-modal" {
                    div class="tools-modal-head" {
                        input id="tools-search" type="search" placeholder="Search tools…" autocomplete="off" aria-label="Search tools";
                        button id="tools-close" type="button" aria-label="Close" { "✕" }
                    }
                    ul id="tools-results" class="tools-results" {}
                    p id="tools-empty" class="tools-empty" hidden { "No tools match your search." }
                    p id="tools-error" class="tools-error" hidden {
                        "Couldn’t load tools. "
                        button id="tools-retry" type="button" { "Retry" }
                    }
                }
```

- [ ] **Step 7: Run the test to verify it passes**

Run: `cargo test --lib renders_tools_button_and_modal`
Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add src/blocks/ui.rs
git commit -m "feat(gizza): composer hammer button + searchable tools modal; drop inline list"
```

---

### Task 5: `build.rs` — remove the now-dead tools scanner

After Task 4, nothing includes `tools.rs` or references `TOOLS`. Remove the generation (keep `SKILLS`).

**Files:**
- Modify: `build.rs`

- [ ] **Step 1: Remove the tools-scanning + tools.rs generation**

In `build.rs`:
- Delete the `let mut tools: Vec<(String, String)> = Vec::new();` declaration and the `// (slug, title) …` comment above it.
- Delete the `page/meta.toml` scan block inside the loop:

```rust
            let meta_path = entry.path().join("page/meta.toml");
            if meta_path.is_file() {
                println!("cargo:rerun-if-changed={}", meta_path.display());
                let meta_text = fs::read_to_string(&meta_path).expect("read page/meta.toml");
                if let (Some(slug), Some(h1)) = (
                    toml_str_value(&meta_text, "slug"),
                    toml_str_value(&meta_text, "h1"),
                ) {
                    tools.push((slug, h1));
                }
            }
```
- Delete the `tools.sort_by(…)` line and its `// Sort tools by slug …` comment.
- Delete the entire `tools_out` block that builds and writes `tools.rs` (from `let mut tools_out = String::new();` through `fs::write(&tools_dest, tools_out).expect("write tools.rs");`).
- If `toml_str_value` is now unused, delete that helper fn too (the compiler will warn `function is never used` — remove it to keep the build warning-free).

Keep everything related to `SKILLS` / `entries` / `skills.rs` untouched.

- [ ] **Step 2: Run the full lib + integration tests**

Run: `cargo test`
Expected: PASS (no `tools.rs` needed; `renders_tools_button_and_modal` green; no unused-code warnings from `build.rs`).

- [ ] **Step 3: Commit**

```bash
git add build.rs
git commit -m "refactor(gizza): drop dead build.rs TOOLS scanner (generator is sole source)"
```

---

### Task 6: `solobase.toml` — ship the new assets

`solobase build` only copies overlay assets it's told about, and the runtime SW only serves bypassed paths. Wire `tools-modal.js/css` exactly like `model-picker.js/css`.

**Files:**
- Modify: `solobase.toml`

- [ ] **Step 1: Add the overlay entries**

In `solobase.toml`, after the `model-picker.css` overlay block, add:

```toml
[[assets.overlay]]
from = "site/tools-modal.js"
to = "tools-modal.js"

[[assets.overlay]]
from = "site/tools-modal.css"
to = "tools-modal.css"
```

- [ ] **Step 2: Add the bypass prefixes**

In `solobase.toml`, append `"/tools-modal.js"` and `"/tools-modal.css"` to the `[assets].extra_bypass_prefix` array (the index `/tools/_index.json` needs no entry — `/tools/` already covers it):

```toml
extra_bypass_prefix = [ …existing…, "/tools/", "/tools-modal.js", "/tools-modal.css"]
```

- [ ] **Step 3: Commit**

```bash
git add solobase.toml
git commit -m "build(gizza): overlay + SW-bypass tools-modal.js/css"
```

---

### Task 7: Integration verify + open PR

The controller's DOM behaviour and the styling aren't unit-tested — verify them against a real build. (Controller assigns `dialog.showModal()`, fetch, render; the generator emits the index.)

- [ ] **Step 1: Build the site and the tool pages**

Run:
```bash
solobase build
cargo run --manifest-path tools/generator/Cargo.toml -- .
```
Expected: both exit 0.

- [ ] **Step 2: Verify the build outputs**

Run:
```bash
test -f pkg/tools/_index.json && head -c 200 pkg/tools/_index.json; echo
grep -o "startsWith('/tools/')" pkg/sw.js | head -1
test -f pkg/tools-modal.js && test -f pkg/tools-modal.css && echo "assets shipped"
```
Expected: `_index.json` is a JSON array with calculator/clock entries; `pkg/sw.js` still bypasses `/tools/` (so the index + tool pages stay static); `tools-modal.js` + `tools-modal.css` are in `pkg/`.

- [ ] **Step 3: Run the full test suites**

Run:
```bash
cargo test
cargo test --manifest-path tools/generator/Cargo.toml
npm test
```
Expected: all green (the new index + filterTools tests included; `npm test` also covers `js/sw-bypass.test.js`).

- [ ] **Step 4: Visual smoke (manual or via the `run`/Playwright skill)**

Serve `pkg/` and confirm in a browser: the hammer appears right of the brain icon; clicking it opens the modal; typing filters by title/description; clicking a result navigates to `/tools/<slug>/`; the old bottom "Tools" list is gone. Note: a plain static server (e.g. `python3 -m http.server --directory pkg`) is enough — the modal fetches `/tools/_index.json` directly (no SW needed for the smoke test).

- [ ] **Step 5: Push and open the PR**

```bash
git push -u origin feat/tools-modal
gh pr create -R gizza-ai/gizza-ai --base main --head feat/tools-modal \
  --title "Searchable tools modal (hammer button) replacing the inline list" \
  --body "Implements docs/superpowers/specs/2026-06-16-gizza-tools-modal-design.md.

The home page's inline 'Tools' list doesn't scale to thousands. Replace it with a hammer button beside the brain/model picker that opens a searchable modal.

- generator emits pkg/tools/_index.json ([{slug,title,description}]) — under the existing /tools/ SW bypass
- site/tools-modal.js fetches it once on open, filters client-side (filterTools unit-tested); site/tools-modal.css = hammer icon + modal
- ui.rs: hammer #open-tools + <dialog id=tools-modal>; inline gizza-tools list + TOOLS include removed
- build.rs: dead TOOLS scanner removed (generator is the single source)
- solobase.toml: overlay + bypass for tools-modal.js/css

Test plan:
- [x] cargo test (gizza-ai + generator), npm test green
- [x] solobase build + generator -> pkg/tools/_index.json present; pkg/sw.js keeps /tools/ bypass
- [ ] CI green
- [ ] post-deploy: hammer opens modal, search works, results link to /tools/<slug>/

🤖 Generated with [Claude Code](https://claude.com/claude-code)"
```

- [ ] **Step 6: Wait for CI, report**

Run: `gh pr checks <PR#> -R gizza-ai/gizza-ai --watch`
Expected: the `test` workflow passes. Report PR URL + status; do not merge without user sign-off.

---

## Verification summary (after deploy)
- `curl -s https://gizza.ai/tools/_index.json | head -c 200` → JSON array of tools.
- App: hammer beside the brain button → opens modal → search filters title+description → result navigates to `/tools/<slug>/`; bottom inline list gone.
