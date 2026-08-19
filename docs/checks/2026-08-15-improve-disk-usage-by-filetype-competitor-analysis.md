# disk-usage-by-filetype — competitor analysis (2026-08-15)

Scan run **before** implementing, per the create-next-tool recipe. One web search
("disk usage by file type analyzer tool aggregate folder space by extension") plus direct
reads of the tools below. Everything here is **paraphrased**; no competitor copy, wording,
branding or trademark is reused anywhere in the tool, its page or its schema.

## Competitors reviewed

| # | Tool | What was read | Shape |
| --- | --- | --- | --- |
| 1 | DiskSavvy (Flexense) | its "file categories and file filters" documentation page | Windows GUI scanner |
| 2 | ncdu (Yorhel) | the manual page | terminal disk-usage browser |
| 3 | WinDirStat | its encyclopedia entry / feature description | Windows GUI scanner (3-pane) |
| — | TreeSize Free (JAM Software) | product page — thin on the file-type view, used only as corroboration | Windows GUI scanner |

WizTree's site was fetched first but its page carried no detail on a file-type view, so it was
replaced by ncdu (a substantive, documented CLI competitor) to keep three real sources.

### 1. DiskSavvy — categorized usage
Groups scan results by file extension **by default**, and can also group by file type, size,
owner and various timestamps. Each group row shows the **number of files**, the **space used**
and the **percentage** of the total relative to the other groups. Ships predefined category
filters plus user-defined rules, and renders pie charts of the selected grouping.

### 2. ncdu — terminal view
Columns are user-selectable: **apparent size vs. on-disk usage**, **item counts**, a
proportional **graph bar**, and a **percentage** of the current directory. Sorting by size,
name, item count or mtime, each ascending or descending. Units switch between **base-10 SI
(kB/MB)** and **base-2 binary (KiB/MiB)**. Exports to JSON.

### 3. WinDirStat — extension list
One of three panes is a **usage-sorted list of file extensions**, each extension carrying its
own colour, cross-linked with the coloured treemap. The tree pane shows a per-node share of
disk use.

### 4. TreeSize Free (corroboration only)
Advertises bar charts and file types grouped by extension into buckets such as videos,
documents and system files; report export to spreadsheet/CSV/PDF in the paid editions.

## Table stakes → decision

| # | Table stake | Seen in | Decision |
| --- | --- | --- | --- |
| 1 | Aggregate bytes per file extension | DiskSavvy, WinDirStat, TreeSize | **In model** — default `group_by=extension` |
| 2 | Roll extensions into broad categories (video/images/docs…) | DiskSavvy, TreeSize | **In model** — `group_by=category`, 10 buckets + `(no extension)` |
| 3 | Per-row size **and** file count **and** % of total | DiskSavvy, ncdu | **In model** — every format carries all three |
| 4 | Usage-sorted, biggest first | WinDirStat, DiskSavvy | **In model** — `sort_by=size`, `order=desc` defaults |
| 5 | Sort by count / name, ascending or descending | ncdu | **In model** — `sort_by`, `order` |
| 6 | Proportional bar per row | ncdu, TreeSize | **In model** — `format=chart`, eighth-block bars, `chart_width` 8–120 |
| 7 | Colour per file type | WinDirStat | **In model, partially** — `format=svg` gives one colour per bar (fixed colour per category); the monospace `chart` output is uncoloured because the page renders plain text |
| 8 | Binary (KiB) vs SI (kB) units | ncdu | **In model** — `units=binary\|si\|bytes` |
| 9 | Export the numbers | ncdu (JSON), TreeSize (CSV/Excel) | **In model** — `format=csv` and `format=json`, byte counts kept exact |
| 10 | Focus/filter on the interesting groups | DiskSavvy filters | **In model, adapted** — `top_n` folds the tail into one `(other N)` row that still counts toward the total (a scan-side extension filter has no meaning for pasted text) |
| 11 | Don't double-count folder totals | implicit in every scanner (they walk the tree once) | **In model** — `skip_folders` detects `du -a` folder rows, trailing slashes and `ls -l` `d` rows |
| 12 | Case-insensitive extension merge (.JPG/.jpg) | GUI scanners on Windows (case-insensitive FS) | **In model** — `ignore_case` |
| 13 | Read the listings people actually have | — (our input differs from theirs) | **In model** — `du -ah`, `find -printf '%s %p'`, `ls -l`/`ls -lRh`, `size,path` CSV/TSV, 1024-based suffixes, thousands separators |
| 14 | Treemap / cushion visualisation | WinDirStat, WizTree | **Out of model** — a treemap needs an interactive canvas and a full directory tree; this tool's output is text/SVG rows |
| 15 | Pie chart of the grouping | DiskSavvy | **Out of model (deliberate)** — the backlog row asks for a sorted bar chart, and a pie of the same numbers already exists as a separate toolkit block |
| 16 | Scan a folder / drive directly, live | all four | **Out of model** — a browser wasm block and a sandboxed CLI have no filesystem walk; the tool consumes a pasted listing instead, which is what makes it work for a remote box over SSH |
| 17 | On-disk (allocated) size, sparse/hard-link awareness | ncdu, TreeSize | **Out of model** — only the sizes present in the pasted listing are available; stated explicitly in the page limits |
| 18 | Trend/history snapshots, owner/age grouping, duplicate finder | DiskSavvy, FolderSizes | **Out of model** — needs repeated scans and file metadata a listing doesn't carry |

Every table stake above is either implemented or listed as out-of-model; none was dropped
silently.

## UX patterns adopted

- **Preset chips** (`[[example]]`) for the four realistic entry points: a `du -ah` paste, the
  category view, a `find` paste rendered as a table, and a count-sorted CSV export — matching
  the "predefined filter/preset" affordance the GUI scanners ship.
- **Slider** for the bar width (bounded 8–120), friendly `<select>` labels for every enum
  (`[input.labels]`) so `binary`/`si` read as "KiB, MiB, GiB (1024, like du)" / "kB, MB, GB
  (1000)".
- **Multiline textarea** with a real `du` paste as its placeholder.
- A **TOTAL row** in the table format (what a report export is expected to end with) and a
  footer note counting skipped folder rows and unreadable lines, so a mis-pasted listing is
  visible rather than silently wrong.

## Notes

- Nothing was copied from any competitor: all copy, labels, category definitions, colours and
  output layouts were written for this tool.
- Out-of-model items are listed, not built.
