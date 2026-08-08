# mv-script-generator — competitor analysis (2026-08-08)

Scan run **before** implementing, per `/create-next-tool` step 4. All notes are paraphrased
observations of publicly documented behaviour; no competitor copy, branding, or trademarked
wording is reused in the tool, its page, or its docs.

## Scope

Our tool takes an **already-decided before→after filename mapping** and emits a reviewable
shell script that performs the renames, plus the matching undo script. It never touches files.
That is deliberately the second half of the bulk-rename problem — the first half (deriving new
names from rules: find/replace, regex, numbering, case) is already covered by
`blocks/bulk-file-renamer`, whose output (an old → new mapping) is the natural input here.
Confirmed non-duplicate: `bulk-file-renamer`'s descriptor states it "computes a safe old -> new
rename mapping only"; it emits no script and no undo.

## Competitors reviewed

| # | Tool | What it is | Relevant to us |
|---|------|-----------|----------------|
| 1 | PowerRename (Microsoft PowerToys) | Explorer shell extension for bulk rename | Preview-before-apply, undo after apply, case-sensitivity, enumeration, apply-to name/extension |
| 2 | CSV File Renamer (filerenameonline.com) | Browser tool: upload a two-column CSV mapping + the files, download a renamed ZIP | Mapping-driven rename, CSV `old,new` per line, no header required, extension preservation, unmatched pass-through |
| 3 | renamex (CLI, GitHub) | Cross-platform bulk rename CLI | `--dry-run` preview, on-disk history log enabling `undo` of the last operation |
| 4 | mvi (PyPI) | Opens the directory listing in `$EDITOR`; you edit destination lines | Free-form mapping as the input model; prints the scheduled operations and asks for confirmation before applying |
| 5 | PowerShell `Rename-Item` / `Move-Item` idiom (Microsoft docs) + CSV `Import-Csv` recipes | The de-facto Windows scripting answer | `-WhatIf` report-only mode, `-LiteralPath` for odd characters, `-Force` to overwrite |

## Table stakes → decision

Every item below lands in the descriptor or in the explicit out-of-model list. Nothing dropped
silently.

### In-model (built)

| Capability | Seen in | How we ship it |
|---|---|---|
| Two-column `old,new` mapping, no header row required | 2 | `mapping` param, one pair per line; a recognised header row (`old,new`, `from,to`, `source,destination`, …) is auto-skipped |
| Multiple mapping notations | 2, 4 | `format` = `auto` \| `csv` \| `tsv` \| `arrow` \| `pipe`; `auto` picks the delimiter present on every line. CSV mode honours `"quoted,fields"` so filenames may contain commas |
| Bash target | 4, 5 | `shell = bash` → `set -euo pipefail` + a `mv_safe` helper + one call per pair |
| PowerShell target | 1, 5 | `shell = powershell` → `$ErrorActionPreference = 'Stop'` + a `Move-Safe` helper that uses `Rename-Item -LiteralPath -NewName` for same-directory renames and `Move-Item` when the destination changes directory |
| Dry-run / report-only preview | 1, 3, 4, 5 | `dry_run` — bash prints each `mv` instead of running it; PowerShell passes `-WhatIf` to the cmdlet (the documented report-only switch) |
| Undo the operation | 1, 3 | `undo_script` (default on) emits a second, separately-ordered script that reverses every emitted rename — including the temp files used to break cycles |
| Refuse to clobber an existing destination | — (gap in 2, 3) | Default `overwrite = false` makes the helper abort if the destination already exists; `overwrite = true` switches to `mv -f` / `-Force` |
| Odd characters in filenames handled safely | 5 (`-LiteralPath`) | Every path is single-quoted with dialect-correct escaping (`'\''` for POSIX, `''` for PowerShell) and `mv -- ` / `-LiteralPath` stop leading-dash paths being read as options |
| Collision detection before anything runs | 1 (preview pane) | Duplicate sources and duplicate destinations are hard errors naming both line numbers; `old == new` rows are skipped and reported |
| Create missing destination directories | 5 (`New-Item -Force` recipes) | `mkdir_parents` (default on) |
| Run from a chosen directory | 2, 5 | `base_dir` emits a guarded `cd -- '…'` / `Set-Location -LiteralPath '…'` header |
| Explanatory comments / operation summary | 1, 4 | `comments` (default on) — counts of renames, skips, reorderings, and cycle temps |

### Beyond table stakes (our differentiators)

- **Clobber-safe ordering.** A chained mapping (`a→b`, `b→c`) is topologically sorted so `b→c`
  runs first. None of the five reviewed tools documents this; a naive generated script silently
  destroys `b`.
- **Cycle breaking.** A true swap (`a→b`, `b→a`) cannot be ordered, so one side is staged through
  a `.mvtmp<N>` temp name and completed at the end — and the undo script unwinds the same way.
- **Two scripts from one run.** Reviewed tools either preview *or* keep a private on-disk undo log
  (renamex writes to `/usr/share/renamex`). Ours hands you the undo as plain text you can save,
  review, and commit.

### Out-of-model (listed, not built)

- Actually moving files / downloading a renamed ZIP (2) — gizza tools are pure compute; the
  script is the deliverable, execution stays with the user.
- Deriving names from rules: regex, enumeration counters, case conversion, prefix/suffix (1, 3) —
  already `blocks/bulk-file-renamer`; feed its mapping in here.
- Filename variables sourced from file metadata — creation date, EXIF/XMP camera fields (1) —
  requires reading the files themselves, which a pure text tool cannot do.
- Random-string / UUID name variables (1) — non-deterministic; gizza tool output must be
  reproducible for the same input.
- Recursive directory traversal, include/exclude files vs folders vs subfolders (1) — needs a
  real filesystem; the caller supplies the paths instead.
- A persistent rename history across sessions (3) — needs state; the emitted undo script is the
  stateless equivalent.
- Interactive `$EDITOR` round-trip (4) — a UI model, not a computation.

## UX patterns adopted

- Preset chips (`[[example]]`) for the three shapes users arrive with: a plain CSV mapping, an
  arrow mapping into subdirectories, and a swap that needs a temp file. Competitors 1 and 3 both
  lead with worked examples; chips are our declarative equivalent.
- Friendly `<select>` labels via `[input.labels]` for `format` and `shell`.
- Multiline textarea for the mapping (pasted lists are the whole input model).
- Page copy states the caps and the "review before running" rule explicitly, matching the
  preview-first posture of every reviewed tool.

## Verification note

The advertised-values matrix run for this tool executes both `shell` values, all five `format`
values, both non-default checkbox states that change the emitted script, the exact 1,000-pair cap
and one over it, plus the generated bash script being executed for real against a scratch
directory (including the swap-cycle case) to prove the emitted script does what it claims.
