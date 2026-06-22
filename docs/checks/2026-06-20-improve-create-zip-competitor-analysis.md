# create-zip — competitor analysis (2026-06-20)

Twenty-fourth `/create-next-tool` backlog pick. Pure-Rust (`zip` crate, deflate
only) tool; `Input::None` + a `files` source_list (like merge-pdf/images-to-pdf).
Surfaces: chat + CLI (no page — array input + binary output). Survey paraphrased.

## Competitors surveyed (general landscape)
| tool type | does well (paraphrased) | dimension |
| --------- | ----------------------- | --------- |
| online "files to zip" tools (ezyzip, archive.online-convert, etc.) | drag many files, deflate compress, download one .zip, in-browser | capabilities |
| zip utilities | choose compression level; rename entries; folders | capabilities |

## Gap diff vs our tool
Our tool: bundle a list of files (URL/ref, any bytes) into one deflate ZIP; entry
names come from each file's resolved name; duplicate names are auto-uniqued
(`name (2).ext`); blank names fall back to `file`. Verified the output is a valid
ZIP (Python `zipfile.testzip()` passes). Covers the core multi-file → one .zip.

**In-model gaps considered, deferred (fit the model; minor):**
- **Compression level** (store/fast/best) — a `level` param over the deflate
  options.
- **Custom per-entry names / folder paths** — a parallel `names` list or
  `name|url` pairs to control the archive layout.
- **Store mode** for already-compressed inputs (images/video) to skip wasted
  deflate.

**Out-of-model:** drag-drop multi-file UI (the page takes one upload; chat/CLI use
the source_list), password-protected zip (the zip `aes` feature isn't enabled —
it pulls non-wasm deps), unzip/extract (a separate inbound-direction tool).

## Tested
unit (4: files roundtrip + zip magic, duplicate names made unique
[dup.txt/dup (2).txt/dup (3).txt], empty error, blank-name fallback) +
drift-guard · `wafer build` validates the block (zip+flate2 → wasm32-wasip1;
pure-Rust so also works in the chat SW) · CLI bundles two real public files into
a valid ZIP (Python zipfile confirms entries + integrity) + empty error path. No
page surface.

> Original work only — no competitor copy, branding, or trademarks copied.
