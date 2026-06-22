# outline-to-json — competitor analysis (2026-06-21)

Tool: convert an indented text outline into nested JSON, entirely in-browser
(pure-Rust core shared across chat / CLI / page). Each non-blank line is a node;
leading whitespace (spaces or tabs) sets nesting.

## Surfaces verified

- **Chat block** — `wafer build` validates + instantiates `target/block.wasm`
  (323.7 KiB); drift-guard schema test (`schema_json_matches_authored_chat_schema`)
  green.
- **CLI** — `gizza tool outline-to-json outline=… format=children|nested pretty=…`:
  children, nested, and the empty-input error path all confirmed (exit 1 on error).
- **Page** — `/tools/outline-to-json/`, 3 Playwright specs pass (children compact,
  nested, pretty-by-default).
- **Unit tests** — 13 core tests (nesting, dedent, tabs, blank lines, non-uniform
  indentation, duplicate siblings, error paths).

## Competitors surveyed

| # | Tool | What it does | Notes |
| --- | --- | --- | --- |
| 1 | jsonjson.com — Text to JSON | Closest direct competitor: parses indented / markdown-list text into nested JSON, with an "auto-detect" parse strategy. | Server-backed web app; auto-detect is the main differentiator. |
| 2 | JSON Editor Online (jsoneditoronline.org) | Tree-mode JSON editor; paste + auto-format, query, compare. | Not an outline→JSON converter — operates on JSON that is already JSON. |
| 3 | FreeFormatter JSON Formatter | Formats/validates JSON; 6 indentation presets (2/3/4-space, tab, compact, JS-escaped); collapsible tree view. | Formatter, not an outline parser. |
| 4 | json-indent.com / JSON Indent | In-browser JSON indent with 2-space / 4-space / tab options. | Formatter only. |
| 5 | Online TSV/TSV-to-JSON tools | Convert tab-separated *columns* to JSON records. | Flat/tabular, not hierarchical outline nesting. |

Most "indented text to JSON" search hits are JSON **formatters/validators** (a
different category — they pretty-print existing JSON). The only true peer that
parses an indented *outline* into a hierarchy is jsonjson.com.

## Gap analysis (fit-to-model)

Capabilities present in this tool and matching or exceeding peers:

- **Two output shapes** — `children` (ordered array of `{text, children}`,
  preserves duplicate siblings) and `nested` (object keyed by text). jsonjson
  offers array/object output; we match it.
- **Tabs + spaces, mixed** — configurable `tab_size` (1–16 columns) so tab- and
  space-indented lines nest consistently. Formatter competitors only offer
  indentation *output* presets; we use it for *parsing*.
- **Non-uniform indentation** — relative-depth parsing means inconsistent step
  sizes (2-space vs 4-space, ragged indents) still nest correctly.
- **Pretty / compact toggle** — matches every formatter peer.
- **Blank-line tolerance + clear root error** — robust input handling.
- **Privacy / offline** — runs 100% client-side (pure-Rust wasm), no upload,
  works offline; the web-app peers are server-backed.

Closed gaps in this build:
- Added the **nested-object output shape** in addition to the array shape so users
  who want a compact `{a:{b:{}}}` tree (the jsonjson "object" mode) are covered.
- Added **`tab_size`** so mixed tab/space outlines parse correctly — a gap vs
  competitors that quietly mis-handle tabs.

## Deliberately out of scope (not built)

- **Auto-detect parse strategy** (jsonjson) — guessing CSV/markdown/key-value
  vs outline is a separate heuristic tool; this tool is specifically the
  indentation-outline case. Kept focused rather than ambiguous.
- **Per-node values / typed leaves** (e.g. `key: value` → `{key: value}`) — would
  change the data model from a pure label tree into a key-value parser; out of
  scope for an "outline" tool and better served by a dedicated text-to-JSON tool.
- **Interactive collapsible tree viewer** — a rendering/UI feature, not a
  conversion capability; gizza pages output the JSON text for copy/paste.

No competitor copy, branding, or trademarks were reused.
