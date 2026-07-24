# directory-tree-view — competitor analysis (2026-07-23)

Tool: `directory-tree-view` — *Prints a clean indented tree of a chosen folder with per-entry
sizes and counts, like a size-annotated tree.*

gizza model: browser-local wasm, no server, no filesystem/folder access. The browser cannot walk
a "chosen folder", so the input is a **pasted listing** where each line carries a path and a byte
size (the output of `du -ab`, `find . -printf '%s\t%p\n'`, or a `path,size` CSV export). The tool's
distinct value over the existing `file-tree-generator` (which renders a *plain* ASCII tree from
bare paths, no sizes/counts) is the **size aggregation + count engine**: roll each file's bytes up
into cumulative per-directory totals and count files/sub-directories per directory — a
`tree --du`-style, size-annotated tree.

## Competitors surveyed (paraphrased — no copied copy/branding)

| # | Tool | What it does better | Dimension |
|---|------|---------------------|-----------|
| 1 | GNU `tree` (`-s`, `-h`, `--si`, `--du`, `--dirsfirst`, `-L`) | The reference size-annotated tree: per-file byte or human-readable sizes, cumulative directory sizes rolled up (`--du`), depth cap, directories-first ordering, and a final report line counting directories and files. | capabilities |
| 2 | `du` (+ `du -h`, `-a`, `--si`) | Human-readable size units (K/M/G, 1024-based) vs SI powers-of-1000; per-entry and cumulative usage. Establishes the size-unit vocabulary users expect. | capabilities |
| 3 | Browser tree generators (nathanfriend "tree", DirViz) | Live paste-to-tree, Unicode vs ASCII connectors, trailing-slash on folders, root label, one-click presets, runs fully in-browser with no upload. | UX / copy |
| 4 | TreeSize (desktop) | Rich per-folder size + file/folder counts, sort by size, directories-first, gradient size bars. Confirms *counts* and *sort-by-size* are table-stakes for a size tree. | capabilities / UX |

(Only ~4 genuinely comparable, reachable references exist; TreeSize is desktop-only and the
folder-scanning browser tools are out-of-model here — noted, not copied.)

## Table-stakes → fit decisions

| Table-stake (competitor) | Decision | Where it lands |
|--------------------------|----------|----------------|
| Per-file size, human-readable **and** raw bytes (`-s`/`-h`) | in-model | `units` enum `human`/`si`/`bytes` |
| SI (1000) vs binary (1024) units (`--si`) | in-model | `units = si` |
| Cumulative directory sizes rolled up (`--du`) | in-model | core aggregation (always on) |
| Directories first (`--dirsfirst`) | in-model | `dirsfirst` boolean, default true |
| Sort entries by name / by size | in-model | `sort` enum `name`/`size-desc`/`input` |
| Depth limit (`-L`) | in-model | `depth` integer (0 = unlimited) |
| Per-directory file/sub-dir **counts** + final report line | in-model | `show_counts` boolean + footer summary |
| Unicode vs ASCII connectors | in-model | `ascii` boolean |
| Root label | in-model | `root` string, default `.` |
| Accept `du` / `find` / CSV listing formats | in-model | `format` enum `auto`/`size-first`/`path-first` |
| Live paste-to-tree, in-browser, no upload | in-model | page (pure tool, runs client-side) |
| Preset examples | in-model | `[[example]]` chips |
| **Scan a real chosen folder from disk** | **out-of-model** | needs recursive folder filesystem access no gizza surface provides (same blocker skiplisted for `folder-size-treemap`/`largest-files-finder`); user pastes a `du`/`find`/CSV listing instead — stated on the page. |
| Interactive treemap / gradient size bars | out-of-model | a static text tree is the gizza surface; noted, not built. |
| Export to PDF/PNG image | out-of-model | text output only (copy/download as text). |

No competitor copy, branding, logos, or trademarks are reproduced — features/UX patterns only.
