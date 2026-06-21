# hex-view — competitor analysis & differentiation

**Tool:** `gizza-ai/hex-view` — render any file as a classic hex dump (offset
column, hex bytes, ASCII gutter).
**Date:** 2026-06-21

## What's out there

| Competitor | Form | Notes / gaps |
|---|---|---|
| `xxd` / `hexdump -C` | CLI | The reference, but a terminal tool; not available in chat or to non-devs. |
| Online hex viewers | Web | Exist, but **upload your file to a server**; many cap size or render slowly in JS. |
| Hex editors (HxD, etc.) | App | Heavyweight desktop installs for a quick look. |
| `od -A x -t x1z` | CLI | Works but cryptic flags; output format differs from the familiar xxd layout. |

## How gizza's tool is better / different

1. **Familiar `xxd` layout.** 8-digit hex offset, bytes grouped 8+8 with a gap,
   and an ASCII gutter (`|...PNG....IHDR|`) — exactly what people expect.
2. **Local — file never uploaded.** Runs in WASM (chat SW + CLI). Safe for
   inspecting private or binary files.
3. **Bounded output.** `max_bytes` (default 4096, hard cap 256 KiB) keeps the
   dump readable for huge files, and the response reports total vs shown bytes
   and whether it was truncated.
4. **Adjustable width.** `bytes_per_line` 1-64 for narrow or wide views.
5. **Any file via url or ref.** Dump a fetched URL or a `ref` from a prior tool
   call — great for peeking at what another tool produced.

## Verification

Core unit tests pin the exact layout (offset, hex grouping, gutter,
last-row padding, non-printable→`.`, custom width, empty input). **End-to-end
CLI** on the kernel.org Tux PNG (`max_bytes=48`) rendered the canonical PNG
header — `89 50 4e 47 0d 0a 1a 0a … 49 48 44 52` with gutter `|.PNG........IHDR|`
— and correctly reported total 7666 / shown 48 / truncated.

## Surfaces & honest scope

- **Chat + CLI only — no web page** (file input + text output, the no-page
  file-input pattern, like `detect-file-type` / `pdf-extract-text`).
- Read-only viewer; it does not edit bytes (a hex *editor* would need a very
  different I/O shape).

## Possible future enhancements

- Offset start parameter (begin the dump at byte N).
- Toggle the ASCII gutter, or a uppercase-hex option.
- Highlight a byte range.
