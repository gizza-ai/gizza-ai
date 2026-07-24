# tabs-to-spaces — competitor analysis (2026-07-23)

Function: convert the whitespace in text or code between tabs and spaces, respecting tab
stops, with a choice of direction, tab width, and whether to touch only the leading
indentation. All notes are paraphrased from public marketing/help pages — no copy, branding,
or trademarks reproduced.

## Competitors skimmed

1. **browserling.com — Tabs to Spaces / Spaces to Tabs** — two separate single-purpose tools.
   The tabs→spaces page has one field, "How many spaces per tab?", then paste-and-convert; an
   undo is offered. The reverse direction lives on a distinct linked page. Minimal by design
   ("press button, get spaces"): no tab-stop awareness, no leading-only scope, no presets.
2. **onlinetexttools.com — Convert Tabs to Spaces** — a single "Number of spaces" parameter
   (fixed count per tab, 1:1 by default). Ships three preset examples (a quote, a "Format JS
   Code" 2-space reindent, and a zero-spaces "strip tabs" case), import file / download / copy /
   Pastebin export, and URL-query-string input for programmatic use. Reverse is a separate
   companion tool; conversion applies to all tabs, not leading-only.
3. **i2text.com — Convert Tabs to Spaces** — "choose how many spaces each tab should be
   converted into" (a single "Spaces" field, fixed count), file upload, editable output, 24+
   interface languages. No reverse direction, no leading-only option, no tab-stop handling.

## Table-stakes parameters (with the in/out-of-model decision)

| Capability | Competitors | Decision | Where it lands |
| --- | --- | --- | --- |
| Convert tabs → spaces | all three | **in-model** | `direction = "expand"` (default) |
| Reverse: spaces → tabs | browserling, onlinetexttools (separate pages) | **in-model** | `direction = "unexpand"` — one tool, both ways |
| Configurable tab width / spaces-per-tab | all three | **in-model** | `tab_width` (1–16, default 4), slider |
| Multiline paste, line breaks preserved | all | **in-model** | `text` textarea; each line processed independently |
| Preset examples / one-click demos | onlinetexttools | **in-model** | `[[example]]` chips (4-space, 2-space, spaces→tabs, indent-only) |
| URL query-string input (deep link) | onlinetexttools | **in-model** | page supports `?text=&direction=&tab_width=&scope=` |
| True tab-stop-aware expansion (tab advances to next stop, not a fixed count) | none (all use a fixed 1:1/N:1 count) | **in-model — differentiator** | core respects tab stops like `expand(1)` |
| Leading-indentation-only scope | none | **in-model — differentiator** | `scope = "leading"` vs `all` |
| Strip tabs entirely (0 spaces per tab) | onlinetexttools (spaces=0) | **out-of-model** | `tab_width` clamps to ≥1; a 0-width "delete tabs" mode is a find-replace, not this tool's job |
| File import / download to file | onlinetexttools, i2text | **partly** | page offers a Download link for `format="text"` output; no server-side file upload needed (paste covers it) |
| Pastebin / clipboard export | onlinetexttools | **out-of-model** | third-party publishing is out of scope for a private in-browser tool |
| Undo | browserling | **out-of-model** | re-run is instant and the input is preserved; a dedicated undo stack is UI chrome, not a param |
| 24+ UI languages | i2text | **out-of-model** | localization is a site-repo concern, not a tool parameter |

## UX patterns to match

- Tab width as a **slider** (bounded 1–16) with the default (4) shown as a placeholder.
- Direction and scope as small labelled `<select>` presets via friendly `[input.labels]`.
- One-click **preset chips** (`[[example]]`) for the common conversions, matching
  onlinetexttools' "presets" idea without copying its copy.
- Multiline **textarea** so pasted code keeps its line breaks.

## Differentiators (in-model, shipped)

Where the competitors do a naive fixed-count replace, this tool:

- **Respects tab stops** — a tab advances to the next multiple of the tab width, so a tab after
  `ab` fills only 2 spaces at width 4 (matching a real editor / Unix `expand`).
- **Round-trips** — `unexpand` collapses runs of spaces back into tabs, never turning a lone
  space into a tab, so it is safe on prose.
- **Leading-only scope** — re-indent code without disturbing aligned comments or tab-separated
  data later in the line.

All three live in a single tool with one form instead of the competitors' split
tabs-page / spaces-page pair.
