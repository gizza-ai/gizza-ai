# diff-code — competitor analysis (2026-08-13)

Scan run BEFORE implementing `blocks/diff-code`. All competitor notes below are **paraphrased
observations of behaviour**; no competitor copy, branding, or trademarked wording is reproduced or
reused anywhere in this repo.

## Why a new block and not an extension of an existing one

`ls blocks/ | grep -i diff` surfaces 15 diff-family blocks. The four that could plausibly overlap
were read before deciding:

| Existing block | What it actually does | Overlap with `diff-code` |
| --- | --- | --- |
| `text-diff` | Two texts → **line-level** `unified` or `json`. No column layout, no intra-line refinement. `core` has an explicit test asserting `"side-by-side"` is an *unknown format* error. | Shares the two-text input, but has neither differentiating capability. |
| `diff-viewer` | Renders an **already-computed** unified diff/patch (git output) as inline / side-by-side / stats / json. Not a comparator. | Complementary: it is the downstream viewer; `diff-code` is the producer. |
| `html-diff` | Strips tags/attributes first, then diffs the **visible text** of two HTML snippets. HTML-specific. | Different domain. |
| `ast-diff` | Parses two **Rust** sources into an AST and diffs canonical forms. Rust-only, formatting-blind. | Different domain, language-locked. |

Conclusion: not a semantic duplicate. `diff-code` is the two-snippet comparator that produces
column layout + word/character-level intra-line highlighting, which nothing in the repo does.

## Competitors reviewed

Three real, reachable tools were skimmed (search: "online code diff tool side by side compare two
code snippets word level highlighting").

### C1 — W3docs "Code Diff"
- Split (side-by-side) view is the default; a unified/git-style view is the alternative.
- Ignore-whitespace toggle: collapses whitespace runs and trims line ends *for matching only* —
  the displayed text stays original.
- Ignore-case toggle, same matching-only semantics.
- Summary counters: added / removed / changed / unchanged.
- Changed lines are tokenised and only the differing tokens are marked; lines above roughly 400
  tokens fall back to whole-line marking (an explicit performance guard).
- Side-by-side rows carry a change-type indicator, a line number, and the content.
- Utility buttons: swap panes, load sample, clear, copy diff (copy emits `+`/`-` markers).
- No syntax-highlighting selector, no line-number toggle, no wrap control.
- Entirely client-side.

### C2 — Toolszu "Code Compare"
- Split and unified views.
- Word-level highlighting of the exact changed span inside a modified line.
- Syntax highlighting with language auto-detection.
- Ignore-whitespace toggle.
- File drag-and-drop upload for several text/code extensions.
- Export to PDF and HTML; shareable URL carrying compressed input.
- Addition/removal counters.
- Client-side only; no stated input cap.

### C3 — diffchecker.dev
- Split view default, unified view alternative.
- Case-sensitive toggle (on by default) and ignore-whitespace (collapse runs, trim line ends).
- Intra-line granularity is **switchable between character and word** — character is the default
  for partially changed lines.
- Unchanged regions collapse and expand on click (context-lines behaviour by another name).
- Per-change accept/reject merge controls plus accept-all/reject-all, with a merged result column.
- Copy merged text, download as a file, self-contained share URL, save to local storage.
- Per-panel character/line/size counters.
- States a soft limit of roughly 25,000 lines per side before it feels heavy.

## Table stakes → decision

Every table-stake below lands in the descriptor or in the out-of-model list. Nothing dropped
silently.

| # | Table stake | Seen in | Fit | Where it landed |
| --- | --- | --- | --- | --- |
| 1 | Side-by-side column layout | C1 C2 C3 | in-model | `view = "side-by-side"` (**default**) |
| 2 | Unified / git-style view | C1 C2 C3 | in-model | `view = "unified"` |
| 3 | Word-level intra-line highlighting | C1 C2 C3 | in-model | `granularity = "word"` (**default**), rendered with the `git diff --word-diff` convention `[-removed-]` / `{+added+}` |
| 4 | Character-level intra-line granularity | C3 | in-model | `granularity = "char"` |
| 5 | Turn intra-line refinement off | implied by C1's token fallback | in-model | `granularity = "none"` |
| 6 | Ignore whitespace (match-only, display original) | C1 C2 C3 | in-model | `ignore_whitespace` (default false) |
| 7 | Ignore case (match-only, display original) | C1 C3 | in-model | `ignore_case` (default false) |
| 8 | Added/removed/changed/unchanged counters | C1 C2 C3 | in-model | `view = "stats"`, plus a summary line on every text view |
| 9 | Collapse unchanged regions / context control | C1 C3 | in-model | `context` (0–100, default 3) — applies to `unified` **and** `side-by-side` |
| 10 | Line numbers on each side | C1 C3 | in-model | `line_numbers` boolean, **default true** |
| 11 | Column width control for the split layout | implicit in C1/C3 layout | in-model | `width` (20–200, default 60) — per-column content width |
| 12 | Structured/machine-readable output | family invariant (`text-diff`, `diff-viewer`) | in-model | `view = "json"` |
| 13 | Long-line refinement guard | C1 (~400 tokens) | in-model | refinement skipped above 400 tokens per side of a changed pair; documented on the page |
| 14 | Stated input cap | C3 (~25k lines, soft) | in-model | hard cap 1 MB per side, named in the error message |
| 15 | Word-diff single-stream output | `wdiff` / `git diff --word-diff` prior art surfaced while checking #3 | in-model | `view = "word-diff"` |
| 16 | Shareable URL carrying the inputs | C2 C3 | already covered | the generated page supports `?left=…&right=…&view=…` deep links platform-wide |
| 17 | Load-a-sample button | C1 | already covered | `[[example]]` preset chips |
| 18 | Clear / reset | C1 | already covered | the generator gives every field page a Reset button |
| 19 | Copy the result | C1 C2 C3 | already covered | the generator gives every text page a Copy button |

### Considered, rejected (in-model but declined)

- **Swap-panes toggle (C1).** One boolean would do it, but it is pure schema bloat here: the page
  user can swap two textareas themselves and a CLI/chat caller simply passes the arguments the
  other way round. An extra param that only permutes two other params is worse than the
  alternative it replaces.

### Out-of-model (feasibility spiked, then declined — not built)

- **Syntax highlighting / language auto-detect (C2).** Spiked: colouring needs a styled HTML/DOM
  renderer, and this page's output surface is plain text (`format = "text"`). Not expressible.
  The repo already has `code-language-detect` for the detection half and `code-screenshot` for a
  rendered, coloured view.
- **Interactive accept/reject merge with a result column (C3).** Requires stateful per-hunk UI and
  an editable third pane; the block model is one pure call producing one output value.
- **PDF / HTML export (C2).** The page renders text and offers copy/download of that text;
  `text-to-pdf` covers the PDF leg for anyone who wants it.
- **File drag-and-drop upload of two files (C2).** The page file input is a single media upload
  bound to the ffmpeg runtime; a pure two-text tool takes two fields. Same constraint that makes
  multi-input ffmpeg tools un-buildable here.
- **Save-to-local-storage history (C3).** Needs persistent browser state the block model has no
  access to; deep links already make a comparison re-openable.
