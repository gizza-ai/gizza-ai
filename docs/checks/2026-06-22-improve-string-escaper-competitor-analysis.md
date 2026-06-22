# string-escaper — competitor analysis (2026-06-22)

Tool: `blocks/string-escaper` — escape (or unescape) a string for a chosen
target syntax: JSON, JavaScript, HTML, URL, shell, SQL, or regex. Pure-Rust,
runs on all surfaces (chat block, CLI, standalone page).

## Surfaces verified

- **Chat block** — `wafer build` validates `target/block.wasm` (321 KiB, instantiates OK).
- **CLI** — `gizza tool string-escaper text=… target=…` verified across all 7 targets,
  both modes, the `quotes` flag, and both error paths (unknown target → exit 1;
  unescape on a one-way target → exit 1).
- **Page** — 4 Playwright tests pass (JSON escape, HTML escape, SQL wrap-in-quotes,
  URL unescape) at `/tools/string-escaper/`.
- Drift-guard schema test passes (`schema_json_matches_authored_chat_schema`).

## Competitors surveyed

| Competitor | Targets offered | Escape + unescape | Notes |
|---|---|---|---|
| FreeFormatter.com | JSON, JavaScript, CSV, SQL, XML | escape + unescape per tool | one separate page per target |
| CodeBeautify.org | HTML, XML, Java, C#, JavaScript, JSON, CSV, SQL | both | one page per target |
| JSONViewerTool.com | JSON only | both | quotes/backslashes/tabs/newlines |
| CodeShack.io | JSON only | both | client-side, auto-format |
| FreeOnlineFormatter.com | XML, HTML, JSON, CSV, Java, SQL, JavaScript | escape (mostly) | one page per target |
| wtools.io | JSON | both | |

Sources:
- [FreeFormatter JSON Escape](https://www.freeformatter.com/json-escape.html)
- [FreeFormatter JavaScript Escape](https://www.freeformatter.com/javascript-escape.html)
- [CodeBeautify JSON Escape/Unescape](https://codebeautify.org/json-escape-unescape)
- [JSONViewerTool JSON Escape](https://jsonviewertool.com/json-escape)
- [CodeShack JSON Escape](https://codeshack.io/json-escape/)
- [FreeOnlineFormatter JSON Escape](https://freeonlineformatter.com/json-escape)
- [wtools.io JSON Escape/Unescape](https://wtools.io/json-escape-unescape)

## Gap analysis (fit-to-model)

The common competitor model is **one separate tool/page per target**. gizza's
`string-escaper` unifies the 7 most-requested targets behind a single `target`
selector — a UX improvement over the competitor pattern, and it covers the
full set offered by the broadest competitors (CodeBeautify, FreeOnlineFormatter)
**except**:

- **CSV escaping** — gizza does not include a `csv` target. Out of scope here:
  gizza already ships a dedicated `csv-json-convert` block, and CSV quoting is
  a row/field operation rather than a single-string escape. Left as a distinct
  tool, not folded in.
- **Java / C# string-literal escaping** (CodeBeautify) — niche; Java/C# string
  escaping is byte-for-byte the same as the JavaScript target for the common
  cases (`\n \t \" \\ \uXXXX`). A user can use the `javascript` target. Not
  worth a separate enum value that would only diverge on edge cases.

### Capabilities gizza has that competitors mostly lack

- **Shell** and **regex** targets — most JSON/HTML escapers omit these; gizza
  covers POSIX single-quote shell wrapping and regex-metacharacter escaping.
- **Unified single tool** for 7 targets vs. a page-per-target.
- **JS line-separator safety** — escapes U+2028 / U+2029 (which silently break
  JS string literals), which simple replace-based escapers miss.
- **Spec-correct JSON** via `serde_json` (control chars → `\uXXXX`).
- **Round-trip unescape** for json/javascript/html/url, with clear errors when
  unescape is requested for a one-way target (shell/sql/regex).
- **Local / private** — runs entirely in the browser (WASM) or CLI; nothing
  uploaded. Matches the best competitors' privacy claim.

## Decisions

- No new in-model gaps to close — the unified 7-target design already meets or
  exceeds the surveyed competitors for single-string escaping.
- CSV and Java/C# targets intentionally NOT added (out of scope / covered by
  the JS target / a separate existing tool). No copy or branding copied from any
  competitor.
