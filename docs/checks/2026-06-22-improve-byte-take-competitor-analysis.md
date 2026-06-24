# byte-take — competitor analysis & verification (2026-06-22)

## Tool
**byte-take** — extract a contiguous byte range (start offset + length) from
text, hex, or Base64 input and return that slice in any of those formats. Pure
compute; runs on all backends (chat block, CLI, browser page). Complement of the
existing `byte-drop` tool (take keeps the range, drop removes it).

## Surface verification (Phase 1)
- **Chat / LLM API:** `wafer build` validated `target/block.wasm` (309 KiB, OK);
  drift-guard unit test `schema_json_matches_authored_chat_schema` passes — the
  descriptor-derived schema matches the authored chat schema.
- **CLI:** `gizza tool byte-take` verified for text→text (`", Wo"`), hex→hex
  (`"1122"`), base64→text (`"ell"`), and negative start (`"ef"`).
- **Page (query-params / fields):** Playwright `tool-page-byte-take.spec.ts` —
  2 tests pass (text extract + hex in/out), driving the rendered `/tools/byte-take/`
  page including the `<select>` format controls.
- **Unit tests:** 14 core tests pass (happy paths across formats, negative start,
  clamping, zero length, invalid UTF-8 slice error, negative length error,
  invalid format error).

## Competitor landscape (top references)
Byte-range slicing is a niche of broader byte/hex editors and "extract substring"
utilities. Representative competitors / reference points:

1. **CyberChef** (GCHQ) — "Take bytes" operation: start, length, "apply to each
   line" toggle. The canonical reference for this exact operation.
2. **Online hex editors** (hexed.it, hexyl-style viewers) — let you select a byte
   range visually but are heavier, editor-focused tools, not a single-purpose
   "give offset+length, get the slice" utility.
3. **`dd` / `tail -c` / `head -c` / `cut -b`** (Unix) — the CLI equivalents of
   byte slicing; powerful but offset/length semantics differ per tool.
4. **Python/JS REPL snippets** (`bytes[start:start+length]`) — what developers
   reach for absent a dedicated tool.
5. **String "substring"/"extract" web tools** — character-based, not byte-based,
   so they get multibyte/encoding cases wrong.

## Gap diff & ranking (fit-to-model)
Capabilities byte-take already matches or exceeds vs. the references:
- **Multi-format I/O** (text / hex / base64 on both input and output) — exceeds
  most single-encoding competitors; matches CyberChef's flexibility for this op.
- **Negative start** (count from the end, `-1` = last byte) — Pythonic slicing
  ergonomics that CyberChef's "Take bytes" lacks.
- **Safe clamping** — out-of-range start/length never errors; clamps to the
  buffer (matches CyberChef's forgiving behavior).
- **Byte-accurate** — operates on raw bytes, not characters, with explicit
  guidance to switch to hex/base64 when a slice splits a multibyte char (a
  correctness edge most "substring" tools get wrong).
- **Privacy / offline** — runs entirely client-side, no upload, no sign-up.

In-model gaps considered and addressed via copy/UX:
- Clear FAQ distinguishing **byte vs. character** offsets and the **invalid-UTF-8
  slice** case, with the remedy (switch output to hex/base64).
- Explicit **byte-take vs. byte-drop** FAQ entry so users pick the right tool.

## Out-of-model / not built (would need new infrastructure; intentionally skipped)
- **"Apply to each line"** (CyberChef-style per-line slicing) — would need a
  mode param; deferred as scope creep for a single-range tool.
- **Visual byte-grid selection** — needs a hex-editor UI component the page
  framework doesn't provide.
- **Binary file upload** — byte-take is a text/hex/base64 field tool; arbitrary
  binary file input needs an `AssetKind` file-input page surface (unbuilt).

## Notes
No competitor copy, branding, or trademarks were copied. Operation name "take
bytes" is a generic byte-slicing term.
