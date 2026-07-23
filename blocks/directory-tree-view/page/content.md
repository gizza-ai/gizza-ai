## What this tool does

Paste a **file listing** — one line per entry, each carrying a path and a byte
size — and get back a clean indented **directory tree with sizes rolled up** into
per-folder totals, plus a file/sub-directory count on each folder. It's the
`tree -s -h --du` view of a listing you already have. Everything runs in your
browser: nothing is uploaded, it works offline, and there's no sign-up.

Your browser can't scan a folder on disk, so instead of pointing at a folder you
paste a listing you generate once on the command line (or export from a
spreadsheet). Three common shapes are understood automatically:

| Where it comes from | Example line | Format |
| --- | --- | --- |
| `du -ab` | `1024⇥src/main.rs` | size, a tab, then the path |
| `find . -printf '%s\t%p\n'` | `1024⇥src/main.rs` | size, a tab, then the path |
| CSV / spreadsheet export | `src/main.rs,1024` | path, a comma, then the size |

Leave **Line format** on *auto* to detect each line, or force *size-first* /
*path-first* if your paths contain commas or spaces. Sizes may include unit
suffixes (`4K`, `1.5M`, `2MiB`, `500KB`).

## How sizes roll up

Every file's bytes are added into the cumulative total of each folder above it —
the same `--du` accounting `tree` and `du` use. If your listing also includes the
directory lines themselves (as `du -a` does), they're recognised as directories
and recomputed from their contents instead of being double-counted.

## Options

- **Size units** — *human* (1024-based `K`/`M`/`G`, the default), *si*
  (1000-based `k`/`M`/`G`), or *bytes* (raw, with thousands separators).
- **Sort entries** — by *name* (A–Z, default), *size-desc* (largest folder or
  file first), or *input* (keep the order paths first appeared).
- **Directories first** — list folders before files within each folder (on by
  default), like `tree --dirsfirst`.
- **Root label** — the text on the very top line (default `.`).
- **Plain-ASCII connectors** — swap `├──`, `└──`, `│` for `|--`, `` `-- ``, `|`.
- **Trailing slash on directories** — append `/` to folder names (on by default).
- **Show per-directory counts** — annotate each folder with its `(files, dirs)`
  counts; the totals line at the bottom always shows the grand total.
- **Max depth** — cap how many levels are printed (`0` = unlimited). Hidden
  deeper entries still count toward their ancestors' rolled-up sizes.

## Example

A `du -ab`-style listing:

```
1024	src/main.rs
3072	src/lib.rs
512	README.md
4096	docs/guide.md
```

becomes:

```
.  8.5K  (4 files, 2 dirs)
├── docs/  4.0K  (1 files, 0 dirs)
│   └── guide.md  4.0K
├── src/  4.0K  (2 files, 0 dirs)
│   ├── lib.rs  3.0K
│   └── main.rs  1.0K
└── README.md  512B

8.5K total · 2 directories · 4 files
```

## FAQ

<details>
<summary>Why do I paste a listing instead of choosing a folder?</summary>

A web page can't read files off your disk, so there's nothing to point at a
folder. Instead you generate a listing once — `du -ab > list.txt`,
`find . -printf '%s\t%p\n'`, or a CSV export — and paste it here. The tree,
sizes, and counts are all computed locally in your browser.

</details>

<details>
<summary>How do I generate a listing my system understands?</summary>

Any of these work, since **auto** detects the shape: `du -ab .` (bytes, then a
tab, then the path), `find . -type f -printf '%s\t%p\n'`, or a `path,size` CSV
from a spreadsheet. Sizes may carry suffixes like `4K` or `1.5M`.

</details>

<details>
<summary>What does "sizes roll up" mean?</summary>

Each folder's size is the **sum of everything inside it**, the same way
`tree --du` and `du` report directory usage. A file counts toward every folder
on its path up to the root, so the top line shows the grand total.

</details>

<details>
<summary>My listing includes directory lines too — will sizes be doubled?</summary>

No. `du -a` prints a line for each directory as well as each file. When a listed
path turns out to have children, it's treated as a directory and its size is
recomputed from its contents, so the directory line doesn't double-count.

</details>

<details>
<summary>What's the difference between human, SI, and bytes units?</summary>

**human** uses 1024-based units (`1.0K` = 1024 bytes), like `tree -h`. **si**
uses 1000-based units (`1.0k` = 1000 bytes), like `tree --si`. **bytes** prints
the exact byte count with thousands separators, like `tree -s`.

</details>

<details>
<summary>Is it free and private?</summary>

Yes — your listing never leaves your device, and the page keeps working offline
once it has loaded. There's no upload, account, or tracking.

</details>
