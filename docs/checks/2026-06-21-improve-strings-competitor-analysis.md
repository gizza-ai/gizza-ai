# strings — competitor analysis & improvements (2026-06-21)

**Tool:** `gizza-ai/strings` — extract printable string sequences from a binary
file, like the Unix `strings` command. Pure-Rust, dependency-free. File input →
text/JSON output, so chat + CLI, no page (file→text — the F3 no-page file-input
pattern, like `detect-file-type` / `file-hash`).

## What competitors do

- **Unix `strings` (binutils)** — the reference: fast, local, with `-n` (min
  length) and `-e` (encoding). But it's a terminal tool on a machine that has
  binutils; not available in a browser/chat or to a non-CLI user.
- **Online "strings extractor" / hex-viewer sites** — paste/upload a file, see its
  strings. **Weakness: you upload the binary** (which may be malware or sensitive)
  to a server.
- **CyberChef "Extract strings"** — excellent and local-in-browser, but a separate
  app you load and configure.

## How this tool competes / improves

1. **Runs locally — nothing uploaded.** Pure-Rust compiled to wasm: runs in the
   chat Service Worker and headless in the CLI. The file never leaves the device —
   important when triaging unknown/suspect binaries.
2. **`strings`-compatible knobs.** `min_len` mirrors `strings -n` (default 4);
   `encoding` mirrors `-e`: `ascii` (default), `utf16` (both LE and BE), or `all`.
3. **Structured output.** Returns a JSON array of the found strings plus a
   `count`, so an LLM or a script can filter/grep them directly (find URLs, keys,
   error messages, embedded paths) rather than parsing console text.
4. **Bounded + honest.** Caps the result at 100,000 strings and reports
   `truncated` so huge inputs don't blow up the response silently.
5. **Chainable + agent-friendly.** Takes the file by `url` or `ref`; identical from
   chat and CLI.

## Honest scope

- **Basic-latin UTF-16** (printable low byte + zero high byte) — it finds
  ASCII-range text stored as UTF-16, not arbitrary multi-byte Unicode scripts.
- **Tab + printable ASCII** (0x20–0x7e, plus tab) define "printable", matching
  `strings`' default; it does not apply locale-specific `isprint`.
- **No page** — file input + text output don't fit the page's text/field model
  (consistent with the other file-input tools).

## Tests

6 core unit tests: ASCII extraction skips non-printable bytes and keeps runs;
`min_len` filters short runs (4 keeps `abcd`/`xyzzy`, 6 keeps neither); UTF-16
**LE and BE** "Hi" are both decoded; `all` returns ASCII *and* UTF-16 hits; tab
counts as printable; empty input yields nothing. Plus the block drift-guard schema
test. **CLI verified** end-to-end on a real binary (extracts known strings).
`wafer build` instantiates the chat block (489 KiB).
