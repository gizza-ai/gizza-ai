# join-lines — competitor analysis (2026-06-22)

Tool: **Join Lines** — merge multiple lines of text into one line with a chosen
separator. Pure-Rust, runs on all surfaces (chat skill / CLI / in-browser page).

## Surfaces verified

- **Chat block** — `wafer build` OK, block instantiates (301.6 KiB).
- **CLI** — `gizza tool join-lines text=$'a\nb\nc'` → `{"joined":3,"result":"a, b, c","total":3}`;
  pipe + trim + remove_blank and prefix/suffix (SQL IN clause) all verified.
- **Page** — 3 Playwright tests pass (comma-space join; trim + remove-blank with
  a pipe; prefix/suffix SQL `IN ('1', '2', '3')`).
- **Drift guard** — `schema_json_matches_authored_chat_schema` passes; manifest.json
  synced to the descriptor schema.

## Top competitors surveyed

1. CoderDesign Text Joiner — separator (comma/space/newline/pipe/custom), CSV/array/SQL use-cases.
2. Web ToolBox Text Line Joiner — preset + custom delimiters; CSV, arrays, SQL IN, CLI args.
3. TextToolbox Merge Lines — custom separator (comma/space/pipe/semicolon/any); SQL IN, spreadsheet, API params.
4. HexToString Join Text — join with customizable separator, remove line breaks.
5. Browserling Join Text Lines — merge strings with a chosen delimiter; in-browser/private.

## Feature diff (competitor → our coverage)

| Capability | Competitors | join-lines | Notes |
|---|---|---|---|
| Arbitrary separator string | yes | yes | free-text `separator` param (default `, `) |
| Tab / newline separator | preset buttons | yes | `\t` `\n` `\r` `\\` escapes decoded in core |
| Empty separator (concatenate) | yes | yes | blank separator joins with nothing |
| Trim each line | yes | yes | `trim` |
| Remove blank lines | yes | yes | `remove_blank` |
| Prefix / suffix per line (quotes, brackets) | yes | yes | `prefix`/`suffix` → SQL IN, CSV, arrays |
| Local / private processing | yes | yes | wasm in-browser, nothing uploaded |
| Line counts in result | partial | yes | `total` + `joined` in JSON output |

## Gaps considered and decisions

- **Preset separator dropdown** (comma/space/tab/pipe buttons): competitors offer
  clickable presets, but our single free-text field with `\t`/`\n` escapes covers
  every preset value and is more flexible; the placeholder documents the escapes.
  A `<select>` would cap flexibility (can't type a custom multi-char delimiter),
  so we keep the free-text field. No gap shipped.
- **Wrap-in-quotes one-click**: equivalent to `prefix`/`suffix` = `"` / `'` /
  `(` `)`; already supported, no dedicated toggle needed.
- Nothing out-of-model: this is pure text manipulation, fully in-model.

## Result

All in-model competitor capabilities are covered. No copy/branding/trademarks
were taken from any competitor; the page copy and tags are original.
