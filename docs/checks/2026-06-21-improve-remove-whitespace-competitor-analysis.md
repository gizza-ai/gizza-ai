# remove-whitespace — competitor analysis (2026-06-21)

Tool: `blocks/remove-whitespace` — trim, collapse, or strip whitespace from text.
Pure-Rust, runs on all three surfaces (chat/LLM, CLI, in-browser page). Nothing is
uploaded.

## Surfaces verified
- **Chat block:** `wafer build` OK (298.5 KiB, validated/instantiates). Drift-guard
  schema test passes (`schema_json_matches_authored_chat_schema`).
- **CLI:** `gizza tool remove-whitespace text=… mode=trim|collapse|strip
  [collapse_blank_lines=true]` — all modes exercised; bad mode exits non-zero with a
  clear message.
- **Page:** `/tools/remove-whitespace/` — 3 Playwright tests pass (trim default,
  collapse, strip). Query-param prefill is generated.
- **Unit tests:** 9 core tests pass (trim/collapse/strip, CRLF normalization,
  Unicode spaces, blank-line collapse, leading/trailing blank-line trim, empty input).

## Top competitors surveyed
1. **StripHTML — Whitespace Cleanup** (striphtml.com) — checkbox matrix: trim,
   collapse, convert tabs, etc.
2. **TextFixer — Remove Spaces** (textfixer.com) — collapses multiple spaces/tabs to one.
3. **PicoToolkit — Remove Spaces** (picotoolkit.com) — three explicit modes: Strip
   (trim), Remove extra (collapse), Remove all (delete ASCII spaces/tabs).
4. **Browserling — Remove All Whitespace** (browserling.com) — one-shot "delete all
   whitespace/tabs/newlines" → single dense string.
5. **CodeShack / MiniWebtool / CodeBeautify** — "collapse multiple spaces into one"
   plus a normalize/trim toggle.

## Capability diff (us vs. them)

| Capability                                   | Competitors        | gizza remove-whitespace |
|----------------------------------------------|--------------------|-------------------------|
| Trim leading/trailing whitespace per line    | most               | yes (`mode=trim`)       |
| Collapse runs of spaces/tabs → single space  | most               | yes (`mode=collapse`)   |
| Remove ALL whitespace → dense string         | Browserling, Pico  | yes (`mode=strip`)      |
| Collapse multiple blank lines                 | some (StripHTML)   | yes (`collapse_blank_lines`) |
| Drop leading/trailing blank lines             | a few              | yes (built into trim/collapse) |
| CRLF/CR normalization to `\n`                 | implicit           | yes (explicit)          |
| Unicode whitespace (NBSP U+00A0, U+3000…)     | rarely             | **yes** — `char::is_whitespace`, covers Unicode spaces |
| Runs locally / nothing uploaded               | varies (server)    | **yes** — wasm, in-browser; also CLI + chat |

## Gaps closed this pass
- Unified the three competing one-trick tools (trim / collapse / strip) into a single
  `mode` parameter so all three workflows ship in one tool.
- Added `collapse_blank_lines` to match StripHTML's blank-line squashing.
- Web wrapper defaults an empty `mode` field to `trim` (matches the descriptor default)
  so the page works out of the box without selecting a mode.
- Explicit Unicode-space handling — a differentiator over ASCII-only competitors
  (NBSP and ideographic space are removed/collapsed correctly).

## Out-of-model / intentionally not built
- "Convert tabs ↔ spaces" / tab-width re-indentation: a distinct transform (closer to a
  code-formatter), not whitespace removal; out of scope for this tool.
- Per-checkbox UI matrix: gizza's page model uses typed params; a single `mode` enum +
  one boolean is a cleaner fit than a checkbox grid and keeps the CLI/chat schema simple.

## Honesty notes
No competitor copy, branding, or trademarks were used. All three surfaces were built and
verified headlessly in this run.
