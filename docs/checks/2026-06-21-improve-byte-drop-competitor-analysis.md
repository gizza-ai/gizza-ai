# byte-drop — competitor analysis (2026-06-21)

**Tool:** `byte-drop` — remove a contiguous byte range (start offset + length)
from text, hex, or Base64 input and return the remainder, rendered as text, hex,
or Base64. Pure compute, runs locally (chat block + CLI + page).

## Surfaces verified

- **Chat / LLM API:** `wafer build` validates `target/block.wasm` (310 KiB, OK);
  drift-guard schema test (`schema_json_matches_authored_chat_schema`) passes —
  the authored chat schema matches the descriptor.
- **CLI:** `gizza tool byte-drop input="Hello, World" start=5 length=4` → `"Hellorld"`;
  hex (`00112233` start 1 len 2 → `0033`); base64 in / text out (`SGVsbG8=` → `ello`);
  negative start (`abcdef` start -2 len 2 → `abcd`). All pass.
- **Page:** 5 Playwright tests pass (text drop, negative start, hex→hex,
  base64→text, query-param deep-link prefill).

## Top competitors

| # | Competitor | What it is | Relevance |
| --- | --- | --- | --- |
| 1 | **CyberChef — "Drop bytes"** (gchq.github.io/CyberChef) | The direct analog: a recipe operation taking `start`, `length`, and `apply to each line`. | Closest match — same core operation. |
| 2 | **HexEd.it** (hexed.it) | Full browser hex editor; select a byte range and delete it interactively. | Overlapping (range-delete) but file/UI-centric, not a one-shot transform. |
| 3 | **byte.tools / Inventive HQ hex editor** | In-browser hex editors with select + cut/delete and offset jump. | Same — interactive editors, no headless/CLI/LLM API. |
| 4 | **Webacus HEX/EDITOR** (app.webacus.dev) | Byte-level data editor and inspector. | Interactive editor. |
| 5 | **onlinebase64tools — Split Base64** | Base64-focused chunk/split utilities. | Adjacent (Base64 byte ops) but no arbitrary range-drop. |

## Gap analysis (fit-to-model)

**Where byte-drop already matches or leads:**

- **Multi-format I/O (text / hex / Base64) on both input and output.** CyberChef's
  Drop bytes operates only on the current byte stream — you must add separate
  From/To operations to change representation. byte-drop folds the decode/encode
  into one step, and supports Base64 directly (the base64-tools competitors don't
  do arbitrary range removal at all).
- **Negative start offsets** (count from the end, `-1` = last byte). CyberChef's
  Drop bytes takes a non-negative start only; removing from the tail requires
  knowing the absolute length. This is a genuine UX lead.
- **Clamping instead of erroring** on out-of-range start/length — a length past the
  end removes only what exists rather than failing.
- **Headless + LLM-callable.** None of the interactive editors (HexEd.it, byte.tools,
  Webacus) expose a CLI or an LLM-tool API; byte-drop ships all three surfaces.
- **Privacy parity.** Like the better competitors, byte-drop runs entirely locally —
  nothing is uploaded.

**Closed in this build (already present at ship):** multi-format I/O, negative
offsets, clamping, clear UTF-8-remainder error with a "switch to hex/base64" hint,
query-param deep-linking, full SEO page copy with examples + FAQ.

## Out-of-model / deliberate non-goals (not built)

- **"Apply to each line"** (CyberChef's 3rd arg). byte-drop operates on a single
  byte buffer; "lines" are a text concept that conflicts with hex/Base64 byte input,
  so a per-line variant would muddy the model. Documented as out of scope — users who
  need per-record edits can run the tool per line. No in-model gap left open.
- **Interactive visual hex grid / in-place file editing** (HexEd.it-style). byte-drop
  is a deterministic one-shot transform (the gizza page/CLI/chat model), not a
  stateful editor — a different product shape, intentionally not pursued.
- **Inserting/replacing bytes.** byte-drop is scoped to removal; insert/replace are
  separate operations and a candidate for a sibling tool rather than feature-creep here.

## Conclusion

byte-drop meets or beats the only direct one-shot competitor (CyberChef Drop bytes)
on format flexibility and negative offsets, and uniquely offers CLI + LLM-API
surfaces. The single competitor feature not carried over ("apply to each line") is a
deliberate, documented non-goal that conflicts with the byte-buffer model. No
in-model capability, copy, UX, or visual gaps remain open.
