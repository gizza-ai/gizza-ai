# Competitor analysis — wrap-lines-in-quotes (2026-07-29)

Tool goal: wrap each (non-empty) line of pasted text in chosen quotes or brackets,
with an optional trailing separator — the classic "turn a column of values into a
SQL `IN (…)` list / JSON array / CSV row" helper.

## Competitors scanned

1. **onlinetexttools.com — Add Quotes to Lines** — reachable.
2. **easytexttool.com — Quote Wrapper** — reachable.
3. **text-tools.io — Add Quotes to Lines** — reachable.
4. **adedx.com — Wrap Each Line in Double Quotes** — reachable (thin docs).
5. freetoolscorner.com — Add Quotes to Text — **403 Forbidden / unreachable**, replaced by
   adedx.com + the search snippet for its advertised SQL/JSON/CSV batch angle.

## Table-stakes params observed (paraphrased, never copied)

| Capability | onlinetexttools | easytexttool | text-tools.io | In gizza model? |
|---|---|---|---|---|
| Custom left/right quote chars | ✅ (default `"`) | — | — | ✅ `wrap=custom` + `open`/`close` |
| Preset quote/bracket styles | — | ✅ 6 (double/single/backtick/`[]`/`()`/`{}`) | ✅ (double/single/curly/`«»`) | ✅ `wrap` enum (9 presets + custom) |
| Trailing separator between lines (`,`) | ❌ (keeps line breaks only) | ❌ | ❌ | ✅ `separator` (the SQL/JSON/CSV angle competitors advertise but under-deliver) |
| No trailing separator on last line | ❌ | ❌ | ❌ | ✅ `last_line_separator` (default off → valid JSON/SQL) |
| Wrap empty lines toggle | ✅ (default: skip) | — | — | ✅ `skip_empty` (default true) |
| Trim each line before wrapping | ❌ | ❌ | ❌ | ✅ `trim` |
| Escape inner quote chars | ❌ (docs warn it breaks output) | ❌ | ❌ (docs warn `""x""`) | ✅ `escape` — the real differentiator |
| Runs locally / private | ✅ | ✅ | ✅ | ✅ (wasm, no upload) |

## Worked examples seen

- text-tools.io: `Apple / Banana / Cherry` → `"Apple"` / `"Banana"` / `"Cherry"`.
- onlinetexttools.com: guillemets `«…»` around each line; multi-quote + empty-line toggles.

## Decisions

**In-model, built:**
- `wrap` enum presets: `double "`, `single '`, `backtick`, `paren ()`, `square []`,
  `curly {}`, `angle <>`, `guillemet «»`, plus `custom` (uses `open`/`close`).
- `open`/`close` custom delimiters (parity with onlinetexttools' left/right quote).
- `separator` trailing string — the genuine gap: every competitor advertises SQL/JSON/CSV
  output but only preserves line breaks. We append the separator and, via
  `last_line_separator=false` (default), omit it on the final element so the result is a
  valid `IN (…)`/JSON array body.
- `skip_empty` (default true — matches the description "each non-empty line"): empty /
  whitespace-only lines pass through unchanged.
- `trim` — strip surrounding whitespace per line before wrapping.
- `escape` — backslash-escape the delimiter (and `\`) inside each line. This is the
  concrete pain three competitors explicitly document as *broken* (`""x""`); we fix it,
  producing valid quoted string literals.

**Out-of-model (not built):** none material. Competitors offer nothing beyond the above;
CSV/SQL/JSON "format presets" reduce to `separator` + wrap choice, which the example chips
cover declaratively.

No competitor copy, branding, or trademarks were reproduced.
