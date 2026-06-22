# data-uri-decode — competitor analysis (2026-06-22)

Tool: decodes a `data:` URI (RFC 2397) into its MIME type, media-type
parameters (charset), encoding (base64 / percent-encoded), decoded byte size,
and payload (text, or binary file-type + hex preview). Pure-Rust, runs on all
surfaces (chat block, CLI, browser page).

## Surfaces verified

- **Chat block** (`gizza-ai/data-uri-decode`): `wafer build` validates +
  instantiates the wasm (322 KiB). Drift-guard schema test passes.
- **CLI**: `gizza tool data-uri-decode uri=…` — verified for base64 text,
  percent-encoded non-ASCII text, binary PNG (detected `image/png` + hex), and
  a non-data-uri error (exit 1).
- **Page** (`/tools/data-uri-decode/`): 3 Playwright specs pass (base64 text,
  percent-encoded UTF-8 `世界`, binary PNG with detected type + hex preview).

## Competitors surveyed (top 5)

1. **base64decode.org / base64encode.org** — general Base64 + data-URI; auto
   charset handling, in-browser.
2. **ToolsKit — Data URL Parser** (toolskit.cc) — extracts MIME type, detects
   base64, previews decoded content.
3. **liminfo.com — Data URL Encoder & Decoder** — decode mode extracts/displays
   the original content from a pasted data: URI.
4. **monocalc.com — Data URI Generator** — file → data URI, MIME detection,
   in-browser.
5. **peasydev.com — Data URI Converter** + decodingbase64.com — paste a data:
   URL, decode to original; magic-byte file-type detection (PNG/JPEG/GIF/WebP/
   PDF/ZIP).

## Feature diff (gap → status)

| Capability | Competitors | gizza data-uri-decode |
|---|---|---|
| Parse `data:[<mediatype>][;base64],<data>` | yes | **yes** |
| Default `text/plain;charset=US-ASCII` on bare `data:,` | partial | **yes** (RFC 2397) |
| Extract MIME type | yes | **yes** |
| Extract media-type params (charset, etc.) | partial | **yes** (full param list) |
| Detect base64 vs percent-encoded | yes | **yes** (`encoding` field) |
| Decoded byte size | some | **yes** |
| Show decoded text (incl. non-ASCII UTF-8) | yes | **yes** |
| Magic-byte file-type detection for binaries | some | **yes** (reuses detect-file-type) |
| Hex preview of binary payload | rare | **yes** (first 64 bytes) |
| Tolerate whitespace/newlines in base64 | some | **yes** |
| Case-insensitive `DATA:` / `;BASE64` tokens | rare | **yes** |
| Preserve commas inside the data | yes | **yes** (split on first comma) |

## In-model gaps closed this run

All in-model parsing/decoding capabilities that competitors expose are covered:
MIME + params + charset, encoding detection, size, text/binary classification,
file-type detection, and hex preview — plus RFC-2397 edge cases (default media
type, embedded commas, whitespace-wrapped base64, case-insensitive tokens) that
several competitors miss. The decoded-text preview is capped at 100k chars and
the hex preview at 64 bytes, with a `truncated` flag, to keep responses bounded.

## Out-of-model (NOT built — page-render features, not pure compute)

- **Rendered image/PDF preview** of an image/PDF data URI. The page output is a
  text render mode; an inline `<img>`/media preview would require a media render
  surface the pure-text page doesn't have. The decoded bytes, detected type, and
  hex are reported instead.
- **Download button** for the decoded binary file. Same reason — the text page
  has no media/download envelope for a decoded-from-input blob.

These are presentation features of the standalone competitor pages, not decode
capability; the core decode result (type, encoding, size, bytes) is complete and
identical across chat, CLI, and page.

No competitor copy, branding, or trademarks were used.
