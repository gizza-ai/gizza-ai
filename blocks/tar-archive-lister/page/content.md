## About this tool

A tarball is just a sequence of 512-byte headers plus file payload blocks. This tool walks
those headers and prints the archive table of contents — paths, byte sizes, permission bits,
owners, groups, entry types, modification times and link targets — without extracting any
member data to disk.

Paste a base64 or hex encoded `.tar`, `.tar.gz` or `.tgz` archive, then choose whether you
want a human `tar -tvf` style table, one path per line, CSV, or structured JSON. Everything
runs locally in your browser; the archive bytes are not uploaded.

### Worked example

The built-in sample is a small gzip-compressed tar with directories, text files and a
symlink. With the default **table** output, the result looks like a traditional long listing:

```text
drwxr-xr-x alice/staff  0 2024-05-01 10:00:00 demo/
-rw-r--r-- alice/staff  7 2024-05-01 10:00:00 demo/README.md
drwxr-xr-x alice/staff  0 2024-05-01 10:00:00 demo/docs/
-rw-r--r-- alice/staff 12 2024-05-01 10:00:00 demo/docs/hello.txt
lrwxrwxrwx alice/staff  0 2024-05-01 10:00:00 demo/link.txt -> docs/hello.txt

5 of 5 member(s) listed (2 file(s), 2 director(y/ies), 1 other) — 19 byte(s) of content in a 4608 tar.gz stream
```

Switch **Output format** to `paths` when you only need names, `csv` for a spreadsheet, or
`json` for automation that needs offsets, numeric mode fields and archive totals.

### What it understands

- Plain `.tar` plus gzip-wrapped `.tar.gz` / `.tgz` streams.
- v7, ustar, GNU long-name / long-link headers and PAX extended headers.
- Regular files, directories, symlinks, hardlinks, device nodes, FIFOs and unknown types.
- Base64 (standard or URL-safe, with optional padding) and hex input.
- Path filters: `*.txt` and `src/*` are glob patterns; `README` is a substring match.

### Limits

Decoded and decompressed input is capped at **64 MiB**, with at most **200,000** members
parsed. The page default lists the first **500** matches; raise `limit` when you intentionally
want a longer listing. bzip2, xz and zstd tarballs are detected and rejected with a clear
message — decompress those first, then list the plain tar.

## FAQ

<details>
<summary>Does this unpack files from the archive?</summary>

No. The parser reads tar headers and skips over each payload block. It reports metadata such
as path, type, size and mode, but it never writes archive members to the browser filesystem or
to your disk.

</details>

<details>
<summary>Can it list `.tar.gz` and `.tgz` files?</summary>

Yes. gzip is detected from the magic bytes, so the same `input_format = base64` works for
plain `.tar` and gzip-wrapped `.tar.gz` / `.tgz` data. Other compressors such as bzip2, xz
and zstd are not decompressed here; decompress them first and paste the resulting tar.

</details>

<details>
<summary>What is the difference between `output = table`, `paths`, `csv` and `json`?</summary>

`table` is a human long listing similar to `tar -tvf`. `paths` emits only member names, one
per line. `csv` includes one row per member with fields such as mode, owner, mtime and offset.
`json` adds archive-level totals plus a structured object for every listed entry.

</details>

<details>
<summary>How do filters work?</summary>

A filter containing `*` or `?` is matched as a glob against the whole member path. For
example, `*.txt` matches text files anywhere in the archive and `src/*` matches members under
`src/`. A filter without wildcards is a plain substring search.

</details>

<details>
<summary>Why does the tool ask for base64 or hex instead of a file upload?</summary>

This repository's pure-WASM page surface uses text parameters shared with the CLI and chat
schema. Base64 and hex make the same deterministic engine work on the web page, in `gizza
tool`, and inside the block runtime without adding a separate file-upload API.

</details>
