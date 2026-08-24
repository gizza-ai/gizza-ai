# brotli-decompress — competitor analysis (2026-08-22)

Scan run **before** implementation, per `/create-next-tool` step 4. All notes are paraphrased
observations of publicly visible tool behaviour; no competitor copy, branding, or trademarks are
reproduced or reused anywhere in this block.

## Search

One WebSearch: *"brotli decompress online tool .br decode base64"*. Four reachable competitors were
skimmed (two candidates from the result list — `multiutil.com/brotli-to-text-decompress` and
`iotools.cloud/tool/brotli-compression` — returned empty content / HTTP 403 and were replaced, so
the scan still covers four real tools rather than fewer).

| # | Tool | URL |
|---|------|-----|
| 1 | JSON-to-Table "Brotli decompressor" | https://jsontotable.org/brotli-decompression |
| 2 | Devutils.lol "GZIP & Brotli decompressor" | https://devutils.lol/tools/gzip-brotli-decompressor |
| 3 | HiOFD "Brotli decompress online" | https://tool.hiofd.com/en/brotli-decompress-online/ |
| 4 | BeautifyCode "Brotli decompression" | https://beautifycode.net/brotli-decompression |

## What they all do

Every one of the four is a **paste-text → read-text** tool: you paste a Brotli payload (Base64 is
the universal interchange form) and it prints the decompressed content. That confirms the backlog
row's framing ("back to its original bytes **or text**", "so I can read the JSON inside") and is
the capability gap versus this repo's existing `file-compressor`, which decodes a `.br` **file**
(url/ref) into a **download** and cannot show you the payload inline.

## Table stakes → decision

Every table-stake below lands in the descriptor or in the explicit out-of-model list. Nothing is
dropped silently.

| # | Table stake | Seen at | Fit | Where it lands |
|---|---|---|---|---|
| 1 | Paste Base64-encoded Brotli data | 1, 2, 3, 4 | in-model | `data` param (multiline field), `encoding = base64` |
| 2 | Print the decompressed text | 1, 2, 3, 4 | in-model | `output = text` (default), page `format = "text"` |
| 3 | Auto-detect how the blob is encoded | 2 (magic bytes / extension) | in-model | `encoding = auto` (default) — tries hex then Base64 and keeps whichever actually Brotli-decodes |
| 4 | Hex input as well as Base64 | — (implied by dev workflows; 2 accepts arbitrary encodings) | in-model | `encoding = hex` (also reached by `auto`) |
| 5 | Show compressed vs decompressed sizes | 1 ("compressed vs. decompressed file sizes") | in-model | `stats` checkbox → compressed bytes, decompressed bytes, ratio, % smaller |
| 6 | Copy result to clipboard | 1, 3, 4 | in-model | generator gives every `format = "text"` page a Copy button — no per-tool work |
| 7 | Download the result | 1 (`.txt` download) | in-model | generator gives every `format = "text"` page a Download link |
| 8 | Clear / reset the form | 3 | in-model | generator ships Reset on every field tool |
| 9 | Sample / example data to try | 1 ("Sample" button), 3 ("View Examples") | in-model | four `[[example]]` preset chips (JSON payload, plain text, hex input, high-ratio stats) |
| 10 | Binary (non-text) payload handling | — (all four assume text) | in-model, **better than the field** | `output = hex` / `output = base64`; `output = text` on non-UTF-8 bytes returns an error that names the two fixes instead of printing mojibake |
| 11 | Upload a `.br` file | 1, 2 | **out-of-model here** | this repo renders file-upload page inputs only for the `ffmpeg`/`model` page runtimes (verified: all 128 `source = "file"` pages are ffmpeg or model); pure-Rust wasm pages are field-only. The file path is already shipped as `file-compressor` (`operation=decompress format=brotli`, url⊕ref → download) — this tool is deliberately the inline/readable half |
| 12 | Auto-detect gzip *vs* Brotli in one box | 2 | **out-of-model (already built)** | `archive-extractor` sniffs magic bytes across gzip/bzip2/xz/zstd/lz4/zip/tar, and `identify-archive-format` names a format from bytes. Adding a second auto-router would duplicate them. Mitigation kept here: when a blob decodes but is *not* Brotli, the error names the actual codec and the sibling tool to use |
| 13 | Multi-GB / streaming decompression | 2 ("no artificial size cap", File System Access API) | **out-of-model** | the payload lives in wasm linear memory, so a bomb guard is required. Capped at 8 MiB compressed input / 16 MiB decompressed output, matching `file-compressor`, `lz4-decompress`, and `lzma-decompress`. The cap is stated on the page, not just in code |
| 14 | Batch / multiple files at once | 1 ("one file at a time") | **out-of-model** | the competitor doesn't do it either; one blob per run |
| 15 | Local, nothing uploaded | 1, 2, 3 | in-model (already true) | pure Rust → wasm, runs in the browser tab; stated in the page copy |

## UX control patterns adopted

- **Preset chips** instead of a bespoke "Sample" button — competitors 1 and 3 both ship canned
  sample data, and `[[example]]` is this repo's declarative equivalent (one click prefills *and*
  runs). Four chips cover the JSON-payload case from the backlog row, a plain-text case, a
  hex-input case, and a high-ratio case with stats on.
- **`<select>` for both fixed-choice params** (`encoding`, `output`) via `Param::enumv`, with
  `[input.labels]` friendly labels — competitor 2 hides its algorithm choice entirely, which makes
  a wrong guess unexplainable; an explicit override that defaults to auto is strictly better.
- **Multiline textarea** for `data` so a pasted, line-wrapped Base64 blob keeps working (all
  ASCII whitespace is stripped before decoding).
- No slider/color/date control applies to this tool — there is no bounded numeric or temporal
  parameter to expose.

## Deliberate differences from the field

1. **Encoding auto-detection is verified, not guessed.** `auto` does not pick by shape alone (a
   short Base64 string can be all-hex-characters). It decodes each candidate and keeps the one that
   actually Brotli-decompresses, so the ambiguous case resolves correctly instead of failing.
2. **Wrong-codec errors are actionable.** If the blob decodes but is not Brotli, the magic bytes are
   checked *after* the Brotli attempt fails (never before — Brotli has no magic number, so an
   up-front sniff would false-reject valid input) and the message names the codec and the sibling
   tool: gzip → `gunzip`, zlib → `raw-inflate`, xz → `lzma-decompress`, lz4 → `lz4-decompress`,
   zstd → `file-compressor`, bzip2/zip/tar → `archive-extractor` / `unzip`.
3. **Stated limits.** Competitor 2 advertises no cap; that is not honest for an in-tab wasm tool, so
   the 8 MiB in / 16 MiB out bomb guard is documented on the page and in the FAQ.

## Not copied

No competitor wording, layout, sample payload, branding, or trademark is reproduced. The worked
examples, FAQ answers, and preset chips in `page/content.md` and `page/meta.toml` are original, and
every example payload was generated locally for this build.
