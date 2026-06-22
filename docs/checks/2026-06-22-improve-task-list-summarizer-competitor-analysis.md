# task-list-summarizer — competitor analysis (2026-06-22)

New tool built end-to-end this run, then reviewed against the online
Markdown-checklist / task-list landscape. `task-list-summarizer` parses a pasted
Markdown task list (GFM checklists) and returns done/pending counts with a
completion percentage, or a filtered list of just the done or pending tasks, or a
JSON object. Pure-Rust wasm, browser-local, no account, no server. Surfaces:
**chat + CLI + page** (text in → text out).

All competitor notes below are **paraphrased**; no copy, branding, or trademarks
reproduced.

## Competitors surveyed (6)

| Tool | Segment | What it does | Counts / % | Notes |
|---|---|---|---|---|
| GitHub / GitLab task lists | platform-native | Render `- [ ]`/`- [x]` as interactive checkboxes in issues/PRs | shows "3 of 5" + a progress bar | the de-facto GFM source; tied to a repo, not a paste-in tool |
| Obsidian "Checklist Progress" plugin | editor plugin | Inserts/updates a `n/total` or % marker inside a note's checklist | yes (fraction or %) | requires Obsidian; edits in place rather than reporting |
| Folge.me checklist creator | browser app | Build/manage interactive checklists | completion % + item counters | builder/manager, not a Markdown parser; per-list state |
| ShiftFlow free checklist | browser app | Check off tasks, export a PDF report | progress tracking | template-driven builder, PDF export focus |
| TheToolApp checklist generator | browser tool | Create an interactive checklist, mark done, export text | live progress bar | generator/editor, not a paste-Markdown summarizer |
| MarkdownLivePreview list generator | browser tool | Generate bullet/numbered/task-list Markdown | none | authoring helper; produces lists, doesn't summarize them |

## Cross-cutting findings

- Two segments, and **neither is a direct peer**: (a) **platform-native**
  renderers (GitHub/GitLab) compute a progress bar but only inside a repo/issue;
  (b) **checklist builders/editors** (Folge, ShiftFlow, TheToolApp,
  MarkdownLivePreview) let you *author* an interactive list, holding their own
  state, rather than ingesting arbitrary Markdown you already have.
- The closest analogue is the Obsidian plugin, which computes a fraction/percent —
  but it lives inside one editor and mutates the note in place.
- **No surveyed tool is a stateless "paste your Markdown checklist → get counts +
  percentage + filtered views" utility.** That paste-and-summarize niche
  (e.g. summarizing a README/issue body's checklist outside GitHub, in a script,
  or in chat) is exactly what this tool fills.

## Gap analysis vs. our tool

### Covered at launch (in-model, built this run)
- **Done/pending/total counts + completion percentage** — the headline metric
  every progress tool exposes; `summary` mode matches GitHub's "n done / n
  pending / %".
- **Filtered views** — `done` and `pending` modes list just the matching task
  texts (no competitor offers a one-click "show me only what's left" extract from
  pasted Markdown).
- **Machine-readable JSON** — `json` mode (`total/done/pending/percent` +
  `done_items`/`pending_items`) is ahead of the builder tools, which only render
  a visual bar; suits scripts/dashboards/CI.
- **Broad GFM parsing** — recognizes `-`, `*`, `+` bullets and ordered `1.` / `2)`
  markers, upper/lowercase `[x]`/`[X]`, and nested/indented items; ignores
  headings, prose, and bullets without a checkbox, so a whole document can be
  pasted in. At parity with GitHub's parser for the common cases.
- **Three surfaces** — chat (LLM tool), CLI, and a standalone page with
  query-param deep-linking; the browser-app peers are page-only.

### Out-of-model (NOT built — recorded, not implemented)
- **Interactive checkboxes / in-place editing** (GitHub, Obsidian, TheToolApp):
  this tool is a stateless summarizer, not an editor — out of the page's
  recompute-on-input model.
- **Drag-to-reorder / persistent multi-list management** (Folge): stateful app
  features, out of scope for a pure-compute tool.
- **PDF / image export of the checklist** (ShiftFlow, Canva): a rendering/design
  concern; this tool reports text, not a styled artifact.
- **Per-section / per-heading progress breakdown**: plausible future in-model
  enhancement (group counts under the nearest preceding `#` heading), deferred to
  keep the launch schema focused; noted for a later pass.

## Verification (this run)

- `cargo test --workspace` — 12 core tests + 1 drift-guard schema test pass.
- `wafer build` — chat block validates (301 KiB), `wafer test` 5/5 fixtures pass.
- `wasm-pack build …/web` — page wasm built.
- CLI — `gizza tool task-list-summarizer` verified for summary / done / json /
  invalid-mode (exit 1) paths.
- Page — Playwright: summary, pending, json, and query-param deep-link, 4/4 pass.

## Sources

- https://blog.markdowntools.com/posts/markdown-task-lists-and-checkboxes-complete-guide
- https://github.blog/developer-skills/github/video-how-to-create-checklists-in-markdown-for-easier-task-tracking/
- https://www.obsidianstats.com/plugins/checklist-progress
- https://folge.me/tools/checklist-creator
- https://www.shiftflow.app/free-tools/free-checklist
- https://thetoolapp.com/generators/checklist/
- https://markdownlivepreview.dev/tools/list-generator
