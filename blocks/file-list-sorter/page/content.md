## About this tool

**File List Sorter** takes a pasted list of file names or paths and puts it in
the order a *file manager* would use, not the order a plain text sort gives.
A plain sort compares text character by character, so `img10.png` lands before
`img2.png` and `README.md` jumps ahead of every lowercase name. This tool
understands that a path has structure: a folder, a file name, an extension, a
depth, and sometimes a size column.

### Worked example

Paste this list:

```
img10.png
img2.png
img1.png
```

With **Sort by = Natural order**, the result is:

```
img1.png
img2.png
img10.png
```

The digit runs are compared as numbers, so `2` sorts before `10` instead of
after it. Switch **Sort by** to *Alphabetical* and you get the machine order
(`img1.png`, `img10.png`, `img2.png`) — useful when you need to match what
`sort` or a Git tree does.

### The options

- **Sort by** — *Natural* is human numbering over the whole path. *Alphabetical*
  is plain codepoint order. *File name only* ignores the folders above each
  file, so `zzz/apple.txt` sorts before `aaa/banana.txt`. *Extension* groups by
  file type (entries with no extension come first), then naturally by path.
  *Folder depth* lists the shallowest paths first. *Size* reads a size column
  off each line.
- **Order** — ascending or descending. Folders-first is *not* reversed: folders
  stay on top in both directions, the way Explorer and Finder behave.
- **Ignore case** — on by default, so `README.md` sits next to `readme.txt`.
  Turn it off for the case-sensitive order of `ls`, `sort` or a Git tree.
- **Folders first** — on by default. An entry counts as a folder when it ends in
  a slash (`src/`) *or* when another pasted entry lives underneath it (`src` is
  a folder if `src/main.rs` is also in the list).
- **Keep each folder's contents together** — sorts by parent folder first, then
  applies the chosen key inside each folder. Handy for sorting a deep `find`
  dump by file name or size while keeping each folder's files side by side.
- **Remove duplicate paths** — keeps the first spelling of each path.
  `./src/app.js`, `src/app.js` and `SRC/APP.JS` count as one entry when *Ignore
  case* is on.
- **Ignore surrounding whitespace** — on by default, because indented `tree` or
  `ls` output otherwise sorts by its indentation.
- **Output shape** — a plain list ready to paste back into a script, a numbered
  list, a table with type/extension/depth/size columns, or JSON.

### Handy for

- Ordering exported screenshots, scans, episodes or `page-1 … page-10` files so
  they concatenate in the right sequence.
- Tidying `ls -1`, `find .`, `git ls-files` or `du -h` output before pasting it
  into a script, ticket or README.
- Grouping a messy download folder by file type, or finding the deepest paths in
  a repository.
- Turning a `du -h` dump into a biggest-files-first list.

Everything runs **locally in your browser** via WebAssembly — your file list is
never uploaded, and nothing is read from your disk. Up to **20,000 paths** per
run; split a longer list and sort it in batches.

## FAQ

<details>
<summary>What exactly is "natural" sorting?</summary>

Natural sorting compares runs of digits as numbers instead of as text, so
`img2.png` comes before `img10.png` and `v1.9` before `v1.10`. The letters
around the digits are still compared as text. Leading zeros are handled too:
`file01` and `file1` compare as the same number, with the shorter spelling
first. Choose *Alphabetical* if you specifically want the raw codepoint order.

</details>

<details>
<summary>How does it know which entries are folders?</summary>

Two ways, since a pasted list carries no filesystem information. An entry that
ends in a slash (`src/`, `assets/img/`) is treated as a folder. So is any entry
that another pasted entry sits underneath — if the list contains both `src` and
`src/main.rs`, then `src` is a folder. Everything else is treated as a file. If
your listing has no trailing slashes and no parent entries, turn **Folders
first** off, since there is nothing for it to lift.

</details>

<details>
<summary>How do I sort by size?</summary>

Include a size on each line and pick **Sort by = Size**. A size is recognised
when it carries a unit — `4.0K  src/app.js`, `src/app.js  1.2MB`, `512B`,
`3GiB` — either before or after the path, which is exactly what `du -h` and
`ls -lh` produce. K/M/G/T are 1024-based. A bare byte count is only read when a
TAB separates it from the path, so a name like `2024 report.txt` keeps its year
instead of being mistaken for a size. Entries with no size always sort last, and
sorting by size with no size column anywhere is reported as an error rather than
silently ignored.

</details>

<details>
<summary>Does it handle Windows paths and mixed separators?</summary>

Yes. Both `/` and `\` count as folder separators, so `docs\report.docx` and
`docs/report.docx` are understood the same way for depth, folder and extension
purposes. A leading `./` is ignored. Your entries are printed back in exactly
the spelling you pasted — the normalisation is only used for comparing.

</details>

<details>
<summary>What counts as the extension?</summary>

The text after the last dot in the file name. So `archive.tar.gz` has the
extension `gz`, `README` has none, and dotfiles like `.gitignore` have none
either (a leading dot starts the name, it does not start an extension). Folders
never carry an extension, even if their name contains a dot.

</details>

<details>
<summary>Is there a limit, and does my list leave my device?</summary>

A run is capped at 20,000 paths; past that you get a clear error asking you to
split the list. Blank lines are skipped and do not count. The sorting itself
runs in WebAssembly inside your browser, so the list never leaves your device
and no file contents are read — only the names you paste.

</details>
