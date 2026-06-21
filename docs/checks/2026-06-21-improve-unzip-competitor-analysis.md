# unzip — competitor analysis & improvements (2026-06-21)

**Tool:** `gizza-ai/unzip` — extract the files from a .zip archive and return them
individually. Pure-Rust (`zip`, deflate). File input → JSON output, so chat + CLI,
no page (file→JSON, the F3 no-page file-input pattern, like `extract-tar` /
`detect-file-type`).

## What competitors do

- **Online "unzip / extract zip" sites** — upload a zip, browse/download files.
  **Weakness: the archive (often containing private files) is uploaded** to a
  server; free tiers cap size.
- **`unzip` / Archive Utility / Explorer** — local and universal, but a desktop/
  terminal action, not callable from chat or a script-over-URL.
- **Language libs** (`zipfile` in Python, `unzip` shell) — fine locally, but you
  write code or run a terminal.

## How this tool competes / improves

1. **Runs locally — nothing uploaded.** Pure-Rust (`zip`) compiled to wasm: chat
   Service Worker and headless CLI. The archive never leaves the device.
2. **Returns files individually, with content inline.** Unlike `extract-tar`
   (which repacks into one ZIP), unzip yields a JSON array — each entry has its
   **path, size, and content**: `text` when the file is UTF-8, base64 `data` when
   binary — so an LLM or script can read each file directly without a second step.
3. **Bounded output.** Inlined content is capped by a total byte budget; files
   past it are still **listed** (name + size) with `content_omitted`, so a huge
   archive can't blow up the response while you still see the manifest.
4. **Directories skipped, paths preserved** (`dir/sub/file.ext`), so structure is
   visible.
5. **Chainable + agent-friendly.** Takes the zip by `url` or `ref`; identical from
   chat and CLI.

## Honest scope

- **Deflate/stored zips** (the common case); legacy/zstd/bzip2 or **encrypted**
  zips aren't handled (kept out to stay pure-Rust + wasm-safe).
- **Content as text or base64 in JSON** — for very large archives this is bulkier
  than a binary download; the byte budget keeps it bounded (use `extract-tar`'s
  repack model when you want a single downloadable archive).

## Tests

3 core unit tests over **zips built in-test**: extracts a text file (inline
`text`) and a binary file (base64 `data`) with the right `count`; a file larger
than the content budget is **listed without content** (`content_omitted`, size
still reported); and a non-zip input errors. Plus the block drift-guard schema
test. **CLI verified** end-to-end on a real public zip (a GitHub `codeload`
archive → its files with inline content). `wafer build` instantiates the chat
block (633 KiB).
