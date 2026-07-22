# ini-env-diff — competitor analysis (2026-07-22)

Tool: diff two `.env` or `.ini` config files key-by-key, flagging keys **added**, **removed**,
and values **changed** between the two files. Pure/local (no I/O). Paraphrased scan below — no
competitor copy or branding reused.

## Competitors scanned

1. **EnvDiff** (envdiff.com) — browser-only `.env` comparator aimed at deployment safety.
   Features: parity checking (missing vars between dev/prod), secret detection (flags API
   keys/passwords), option to ignore comments and blank lines, case-insensitive compare mode,
   custom redaction regex to mask sensitive values, upload-or-paste input. Does not document how
   it labels added/removed/changed or its output shape.
2. **Env Variable Manager** (quicktoolsfor.me/tools/env-manager) — table view + two-file diff.
   Four categories made explicit: **Added** (key only in the 2nd/new file), **Removed** (key only
   in the 1st/old file), **Modified** (in both, values differ), **Unchanged** (identical). Parsing:
   strips wrapping quotes, strips inline comments, ignores blank lines. Output: color-coded
   side-by-side. No case/mask/sort options documented.
3. **FileDiffs ENV Compare** / **CodeSmith .env Compare** — upload two `.env` files, side-by-side
   highlight of every added/removed/changed key+value; CodeSmith also offers "sync" to copy
   variables across environments (out of scope — this is a diff, not an editor).

## Table-stakes params / behaviour

- Two text inputs: **left** (old/first) and **right** (new/second).
- Four diff categories: added (in right only), removed (in left only), changed (both, value
  differs), unchanged (identical). Direction: added/removed are relative to left→right.
- Robust parse: `KEY=VALUE` and `key = value`, `#` and `;` comments, blank lines, single/double
  quote stripping, inline comments, leading `export `, and `[section]` headers (`.ini`) →
  dotted `section.key`.
- Options: **format** auto|env|ini (auto detects `[section]` headers), **ignore_case** (compare
  keys case-insensitively), **mask_secrets** (redact values of sensitive-looking keys in output),
  **output** report|json.
- A summary line with the four counts; report groups keys under Added/Removed/Changed and lists
  Unchanged names only.

## In-model vs out-of-model

- In model: everything above — pure string parsing + set/map diff, deterministic, no I/O.
- Out of model (not built): side-by-side visual UI, "sync"/auto-fix that rewrites a file, CORS
  validation, custom user-supplied redaction regex (a fixed sensitive-key heuristic covers the
  masking table-stake). These are UI/editor/heuristic-config features, not diff logic.

## Sources
- https://envdiff.com/
- https://quicktoolsfor.me/tools/env-manager/
- https://filediffs.com/env-compare
- https://www.codesmith.in/tools/env-compare
