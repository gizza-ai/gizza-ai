# diff-viewer — competitor analysis (2026-07-31)

Tool: render a pasted unified diff as a clean inline or side-by-side view with add/remove stats and optional structured output.
Type: pure text parser/renderer. All competitor notes are paraphrased.

## Competitor scan (top real tools)

### 1. GitHub pull-request diff view
- **Function:** presents git patches as split or unified file views with additions/deletions, file status, comments, and review navigation.
- **Features:** per-file stats, inline vs split layout, syntax-colored additions/deletions, hidden whitespace changes, file tree/navigation, binary-file notices.
- **Input/output:** repository diff to interactive review UI.
- **UX:** split/unified toggle, whitespace toggle, per-file sections, colored line rows.

### 2. Diffchecker unified diff / text compare view
- **Function:** shows text and patch differences in visual side-by-side or inline layouts.
- **Features:** inserted/deleted highlighting, line alignment, export/share workflows, whitespace handling, examples for pasted text.
- **Input/output:** pasted diff or two texts to web view.
- **UX:** large text areas, side-by-side/inline display choices, highlighted changes.

### 3. CodeMirror / Monaco diff editors
- **Function:** embed diff rendering in developer tools, docs sites, and code-review products.
- **Features:** unified or split views, line numbers, syntax coloring hooks, collapsible unchanged context, and programmatic access to change metadata.
- **Input/output:** text/diff model to visual editor widget.
- **UX:** side-by-side panels, line gutters, change markers, scroll syncing.

## Table-stakes distilled

| Capability | In/out of model | Decision |
| --- | --- | --- |
| Parse common unified diff headers (`diff --git`, `---`, `+++`, `@@`) | in-model | built |
| Multi-file patches with per-file totals | in-model | built |
| Add/remove/context line classification with old/new line numbers | in-model | built |
| Inline unified view | in-model | built (`view=inline`, default) |
| Side-by-side text view | in-model | built (`view=side-by-side`) |
| `git diff --stat`-style summary | in-model | built (`view=stats`) |
| Structured machine-readable output | in-model | built (`view=json`) |
| Whitespace-only change folding | in-model | built (`ignore_whitespace=true`) |
| New/deleted/renamed/binary file notices | in-model | built at parser/banner level |
| Syntax-coloring by programming language | out-of-model | listed only; this repo's page runtime renders generic text output |
| Inline review comments / collaboration | out-of-model | listed only; requires accounts/server state |
| Generating a diff from two separate texts | out-of-model for this tool | deliberately not built; this is a viewer for already-computed unified diffs |
| Collapsible unchanged context and scroll sync | out-of-model | listed; page runtime is lightweight/static |

## Design decisions

- Keep the model centered on **unified diff input**, not comparison of two source texts; this avoids duplicating existing comparator tools.
- Expose a small `view` enum: `inline`, `side-by-side`, `stats`, `json`.
- Keep the web output as plain text so CLI, page, and chat surfaces agree exactly.
- Make `ignore_whitespace` a boolean checkbox to match common review UI controls.
- Parse leniently: ignore metadata lines and accept plain `diff -u` headers without a `diff --git` line.
- Return clear errors for prose or empty input that is not recognizable as a unified diff.

## Verification plan

Unit tests cover parsing, inline/stat/side-by-side/json rendering, new/deleted/renamed/binary files, whitespace folding, hunk section headings, no-newline markers, and bad input. Page tests cover exact inline output, side-by-side output, and a deep-link with `ignore_whitespace=true`.
