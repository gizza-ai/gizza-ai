# unified-diff-reverse — competitor analysis (2026-08-20)

Scan run **before** implementing, per `.claude/skills/create-next-tool/SKILL.md` step 4.
All notes are paraphrased observations of what each tool exposes — no competitor copy,
branding, or trademarks are reproduced or reused.

Backlog row: `unified-diff-reverse` — "Reverse-applies a unified diff to undo a change or
invert hunks for a revert patch." (`tools-to-build.csv:1467`, size S, type_hint `pure`).

## Search + tools skimmed

One search for "reverse a unified diff patch online tool invert patch revert". The three
real, reachable tools skimmed:

1. **dev-toolbox.tech — diff patch applier.** Two panels (original text + unified diff),
   an apply/generate mode switch, a *reverse (unapply)* toggle described as swapping the
   meaning of the `+` and `-` lines, per-hunk accept/reject checkboxes, fuzzy matching
   when context drifts, and seven worked presets (simple add, remove, modify a function,
   multi-hunk refactor, config update, reverse apply, context mismatch, generate). Nine
   FAQ entries covering the unified format, git compatibility, reverse apply, hunk
   failure, selective hunks, and privacy. Nothing documented about git extended headers,
   `index` lines, rename/new-file/deleted-file entries, or binary patches.
2. **io-tools.com — diff/patch preview.** Original text + unified diff, one "apply
   preview" action, copy-output, and a load-sample button. Validates hunk headers,
   context/deleted/added lines and the declared line counts, and refuses rather than
   emitting a wrong result. No reverse mode, no multi-file/extended-header handling
   documented.
3. **scrapfly.io — git patch/diff viewer.** Paste a git diff/patch; unified and
   side-by-side views, an ignore-whitespace toggle, file-tree navigation, per-file
   +/- counters, shareable links, HTML download, all client-side. Read-only: it renders a
   patch, it does not transform or invert one.

Reference behaviour also cross-checked against the documented semantics of `git apply -R`,
`git diff -R` and `patch -R` (the CLI equivalents these tools imitate).

## Table-stakes, and where each one landed

| Capability | Competitors | Decision |
| --- | --- | --- |
| Paste a unified/`git diff` patch, get the inverted patch back | closest is a viewer + an *apply*-side reverse toggle; none emits a reverse **patch** | **in model** — this is the tool: `diff` → inverted patch text |
| `@@` header inversion (old/new ranges swap) | implicit in the apply-side reverse toggle | **in model** — mandatory, plus git's implicit `,1` shorthand is preserved |
| `+`/`-`/context line inversion, `\ No newline at end of file` markers | not documented anywhere | **in model** — markers are re-attached to the side they now belong to |
| Multi-file patches, file-tree/per-file counters | viewer only | **in model** — every file section is inverted; `file` picks one out |
| Per-file +/- statistics | viewer shows counters | **in model** — `output = "summary"` and `"json"` report post-invert and pre-invert counts |
| Ignore-whitespace | viewer toggle (a *rendering* option there) | **out of model here** — inversion is byte-exact by construction; whitespace tolerance belongs to *applying*, where `apply-patch` already has `ignore_whitespace` |
| Fuzzy/offset matching, per-hunk accept/reject | apply-side features | **out of model here, already shipped elsewhere** — `apply-patch` (fuzz, offset search, conflicts) and `diff-hunk-selector` (numbered hunk selection) cover these; the page cross-links them instead of duplicating |
| Reverse-*apply* to a pasted file | dev-toolbox's toggle | **already shipped** — `apply-patch reverse=true`. Documented in the FAQ so the user is routed there rather than given a second half-copy |
| Worked presets / examples | seven presets on one tool, load-sample on another | **in model** — four `[[example]]` chips (revert a commit, rename, new file, keep index lines) |
| Privacy / local execution | all three advertise client-side only | **already true** — the page runs the same WASM core locally; stated in the copy |

## Beyond the table stakes (git extended headers)

None of the three documents what happens to git's extended header lines, yet a real
`git format-patch`/`git diff` output is full of them and a "reversed" patch that keeps them
untouched will not apply. Handled here, matching `git diff -R`:

- `index <old>..<new> [mode]` → hashes swap (`index_lines = "swap"`, the default; `keep`
  and `drop` are also offered — `drop` is the safe choice when the hashes are stale).
- `new file mode X` ↔ `deleted file mode X`.
- `old mode X` / `new mode Y` → the two modes swap.
- `rename from A` / `rename to B` → swap; same for `copy from` / `copy to`.
- `--- a/x` / `+++ b/y` → the two path lines swap (with their timestamp columns), and the
  `diff --git a/x b/y` line's two sides swap with it. `swap_paths = false` keeps the header
  paths untouched for the rare case where the consumer keys off the original path.
- `GIT binary patch` sections cannot be inverted from the forward delta alone — `on_binary`
  chooses `fail` (default, with a message naming the file), `skip` (drop that file section)
  or `keep` (pass it through unchanged and warn).

## Out of model / not built

- Reverse-*applying* to a source file (that is `apply-patch reverse=true`).
- Fuzz / offset matching / conflict reports (`apply-patch`).
- Hunk selection, splitting, numbering (`diff-hunk-selector`).
- Side-by-side rendering, syntax highlighting, file-tree navigation (`diff-viewer`,
  `diff-highlight`).
- Inverting a binary delta (needs the reverse delta, which a forward-only patch omits).
- Combined/merge diffs with `@@@` headers — rejected with a message, as elsewhere in the
  diff family.

## Verification notes

Every advertised value form is exercised end-to-end (not just in argv/unit shape): all
three `output` choices, all three `index_lines` choices, all three `on_binary` choices,
both non-default booleans, the `file` selector on a two-file patch, the 1 MB cap at and one
over the boundary, and a `?diff=…&output=…` deep link on the page.
