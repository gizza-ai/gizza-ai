## About this tool

Desktop disk-space analyzers answer one question first: **which file types are eating the
drive?** This tool gives you that same ranked breakdown from a listing you already have — a
`du` dump pasted from a server, a `find` run, an `ls -lR` capture, or a two-column CSV export.
Every line's size is added to its file extension (or to a broad category such as *video* or
*documents*), and the result comes back sorted biggest-first with the size, the share of the
total, a bar and the file count for each type.

It runs entirely in your browser on the text you paste. No folder is opened, nothing is
uploaded, and no agent has to be installed on the machine that holds the files — which is why
it works for a listing you captured over SSH from a box you can't install software on.

### Worked example

Paste this `du -ah` output:

```
4.0K	./src/app.js
2.0M	./assets/hero.png
1.0M	./assets/logo.png
18M	./media/clip.mp4
8.0K	./README.md
```

With the defaults (group by extension, sort by size, binary units, 32-character bars) you get:

```
Disk usage by extension — 5 file(s), 21.0 MiB total

.mp4   18.0 MiB   85.7%  ████████████████████████████████  1 file(s)
.png    3.0 MiB   14.3%  █████▍                            2 file(s)
.md     8.0 KiB    0.0%  ▏                                 1 file(s)
.js     4.0 KiB    0.0%  ▏                                 1 file(s)
```

Switch **Group by** to *Category* and the same listing collapses to `video 18.0 MiB 85.7%`,
`images 3.0 MiB 14.3%`, `documents 8.0 KiB`, `code 4.0 KiB` — the fastest way to see whether a
disk is full of media, build artifacts or source.

### Listings it reads

| Command | Example line |
| --- | --- |
| `du -ah` / `du -a` | `4.0K	./src/app.js` |
| `find . -type f -printf '%s\t%p\n'` | `4096	./src/app.js` |
| `ls -l`, `ls -lRh` | `-rw-r--r--  1 me  staff  1024 Jan  3 10:11 notes.md` |
| CSV / TSV export | `4096,src/app.js` |

Size suffixes are 1024-based, the way `du -h` and `ls -lh` print them, so `4.0K` is 4096 bytes
and `1.5MiB` is 1572864. A bare number is bytes. Thousands separators (`1,234,567`) are
accepted. Lines with no readable size — `total 48` headers, tree art, prose — are skipped and
counted in a note under the chart, so you can tell whether the tool understood your paste.

### Limits and edge cases

- **Up to 20000 sized files per run.** Past that the tool stops with an error rather than
  truncating silently. For a whole disk, run `du -a` one folder deeper and paste per folder,
  or feed it `find`'s output for the subtree you care about.
- **Folder rows are skipped by default.** `du -a` prints a line per folder whose size already
  includes everything beneath it; counting those would double-count the same bytes. A line
  counts as a folder when the path ends in a slash, when `ls -l` marked it `d`, or when
  another pasted path sits inside it. Turn the checkbox off if your listing is files-only and
  you want every line counted verbatim.
- **`.tar.gz`, `.tar.bz2`, `.tar.xz`, `.tar.zst` stay whole** instead of being counted as
  `.gz`/`.xz`. Any other double extension is counted by its last part.
- **Names with no usable extension** — `README`, `Makefile`, `.gitignore`, `report v1.2 final`
  — are grouped under `(no extension)` rather than guessed at.
- **Sizes are apparent file sizes**, whatever your listing reported. Block-level allocation,
  sparse files, compression and hard links are not modelled: two hard links to the same file
  appear twice if the listing lists them twice.
- **Percentages are of the pasted listing**, not of the disk. Paste a partial listing and the
  shares are partial too.
- Beyond the top rows (15 by default) the remaining types are folded into one `(other N)` row
  that still counts toward the total, so the percentages always add up.

## FAQ

<details>
<summary>What exactly do I paste in?</summary>

Any listing where each line carries a size and a path. The usual sources are
`du -ah`, `find . -type f -printf '%s\t%p\n'`, `ls -lR` (or `ls -lRh`), and CSV/TSV exports
with a size column and a path column in either order. Suffixed sizes (`4.0K`, `18M`,
`1.5MiB`) are read as 1024-based, matching `du -h`; bare numbers are bytes.

</details>

<details>
<summary>Why is my total smaller than what `du -sh` reports?</summary>

Two common reasons. First, folder rows are skipped by default so their contents are not
counted twice — the total is the sum of the *files* in your listing. Second, `du` reports
space **allocated on disk** (rounded up to block size, and it counts a hard-linked file once),
while a `find -printf '%s'` listing reports **apparent** file size. Both are legitimate
numbers; this tool simply adds up whatever your listing said.

</details>

<details>
<summary>Can it group by something broader than a file extension?</summary>

Yes — set **Group by** to *Category* and extensions are rolled into images, video, audio,
documents, archives, code, data, executables, fonts and other (plus `(no extension)`). It's the
quicker read when you want to know whether a drive is full of media or of build output.
Unrecognised extensions land in *other*, and you can always switch back to the per-extension
view to see exactly which ones.

</details>

<details>
<summary>How do I export the numbers?</summary>

Set the output shape to **CSV** for `name,bytes,size,percent,files` rows that paste straight
into a spreadsheet, or **JSON** for a structured object with `total_bytes`, `total_files`,
`skipped_folders`, `ignored_lines` and a `groups` array. Both keep the exact byte counts
alongside the human-readable sizes. The **Download** button under the result saves whatever is
currently shown.

</details>

<details>
<summary>Can I get a picture rather than text?</summary>

Choose the **Colored SVG bar chart** output shape. It returns SVG source with one colored bar
per file type (categories get a fixed colour each), plus size and share labels and a hover
tooltip per bar. Copy it into a README, a ticket, a slide or a report — it's a plain text
`<svg>` element, so it stays sharp at any size and needs no image hosting.

</details>

<details>
<summary>Are my file names or sizes uploaded anywhere?</summary>

No. The analysis runs as WebAssembly inside your browser tab on the text in the box; the page
makes no request with your data and nothing is stored. The same computation is available
offline through the command-line tool if you'd rather keep the listing on the machine that
produced it.

</details>
