# diff-hunk-selector — competitor scan (2026-08-08)

Scan run **before** implementation, per `create-next-tool` step 4. All notes are paraphrased
observations of behaviour; no competitor copy, branding, or trademarks are reproduced.

## Tools reviewed

| Tool | Shape | What it does |
| --- | --- | --- |
| `git add -p` / `git add --patch` (git) | interactive terminal | Walks a working-tree diff hunk by hunk and asks stage / skip / split / edit per hunk. The canonical mental model for "pick a subset of a diff". |
| `filterdiff` (patchutils, with `lsdiff` / `grepdiff` / `splitdiff`) | CLI filter | Reads a patch on stdin and writes a smaller patch: `--hunks`/`-#` takes a 1-based comma list with `first-last` spans, open-ended spans, and an inversion modifier; `-i`/`-x` include/exclude files by pattern; `--lines` keeps hunks touching a line range; `--annotate` labels hunks; `--clean` drops non-diff noise; `--strip` removes path components; `--format` converts unified↔context. `lsdiff` prints the numbered inventory that feeds `--hunks`. |
| `splitpatch` | CLI splitter | Splits one patch into several files — by file by default, one file per hunk with `--hunk(s)`; output files are named after the patched file plus a sequence number so each piece applies on its own. |
| `hunk.nvim` | Neovim UI | Diffs two directories and lets you toggle whole files, whole hunks, or individual lines, then writes the resulting partial diff/tree. Selection granularity below hunk level is its differentiator. |
| Emacs `diff-mode` (`diff-split-hunk`) | editor mode | Splits one oversized hunk into two at a context line, so a "too big" hunk can be narrowed before applying. |

## Table stakes → decision

| Capability | Seen in | Decision |
| --- | --- | --- |
| Numbered inventory of every hunk (file, header, +/- counts) before you pick | `lsdiff`, `git add -p`, hunk.nvim | **In model** — `output=list`, the default, so pasting a diff immediately answers "what is in this patch". |
| Select hunks by 1-based number with ranges (`1,3-5`) | `filterdiff --hunks` | **In model** — `hunks` param, plus open-ended spans (`4-`, `-2`) and `all`. |
| Invert the selection (drop the listed hunks, keep the rest) | `filterdiff` `x` modifier | **In model** — `invert` boolean. |
| Include/exclude files by pattern | `filterdiff -i/-x` | **In model** — `files` param takes comma-separated globs; a `!` prefix excludes. |
| Keep only hunks touching a line range | `filterdiff --lines` | **In model** — `lines` param, same span syntax, matched against original-file line numbers. |
| Emit a valid, applicable smaller patch (file headers preserved) | filterdiff, splitpatch, hunk.nvim | **In model** — `output=patch` re-emits `diff --git`/`---`/`+++` preambles for every file that keeps a hunk. |
| Renumber the new-side hunk starts after dropping hunks | filterdiff (offsets), splitpatch (per-hunk files) | **In model** — `renumber` boolean, default on; the new-side start of each kept hunk is shifted by the net line delta of the dropped hunks before it in the same file. |
| One standalone patch per hunk | `splitpatch --hunks` | **In model** — `output=split` emits each selected hunk as its own complete patch under a labelled separator. |
| Machine-readable inventory | none directly (JSON output is our own affordance; `lsdiff` is text-only) | **In model** — `output=json`. |
| Preset/one-click starting points | `git add -p` prompt shortcuts, hunk.nvim keymaps | **In model** — three `[[example]]` chips on the page (list, export a subset, split per hunk). |
| Per-line selection inside a hunk | hunk.nvim, `git add -p` `e` | **Out of model** — needs an interactive two-pane editor; this is a one-shot pure transform with no UI state. Listed, not built. |
| Splitting one big hunk into two | `git add -p` `s`, Emacs `diff-split-hunk` | **Out of model for v1** — the split point is an interactive choice, and a wrong split makes an inapplicable patch. Documented on the page as a limit. |
| Reading the diff from a repo / working tree | `git add -p`, hunk.nvim | **Out of model** — no filesystem or git access from the sandbox; the diff is pasted or piped in. |
| Writing N separate output files | `splitpatch` | **Out of model** — the surfaces return one text payload; `output=split` returns the same content with labelled separators to copy or `csplit`. |
| Context↔unified format conversion, `--strip` path rewriting | `filterdiff --format`, `--strip` | **Out of model for v1** — different concern (format munging, not selection). Listed here so it is not silently dropped. |
| Applying the patch | `git apply`, `patch` | **Out of model** — no filesystem. The output is meant to be piped into `git apply`. |

## UX controls / defaults to match

- Default output is the **inventory**, not a no-op copy of the input — mirrors how `lsdiff` precedes
  `filterdiff` in real use, and gives the page a useful first render from a single paste.
- Hunk numbers are **global and 1-based across the whole patch** (filterdiff's convention), shown in
  the list as `[n]` so the number can be pasted straight into the selection field.
- The selection field accepts the same span grammar as the established CLI (`1,3-5`, `4-`, `-2`,
  `all`), so muscle memory transfers.
- Counts per hunk (`+a −d`) and per file totals, like a `--stat` summary, so a picker can judge size
  before selecting.
- Preset chips for the three real jobs: inventory, export a subset, one patch per hunk.
- Stated cap on input size, stated behaviour for binary files, renames, and `\ No newline at end of
  file` — the edge cases that make hand-rolled hunk splitters emit broken patches.
