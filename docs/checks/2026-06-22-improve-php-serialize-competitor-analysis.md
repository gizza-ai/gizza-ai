# php-serialize — competitor analysis (2026-06-22)

Tool: **JSON → PHP `serialize()`**. Converts a JSON value into the string format
produced by PHP's `serialize()` so a PHP app's `unserialize()` can read it.
Surfaces: chat block (LLM API), CLI (`gizza tool php-serialize`), standalone page
(`/tools/php-serialize/`). Pure-compute, runs entirely client-side.

## Top competitors surveyed

| # | Tool | Notes |
| - | ---- | ----- |
| 1 | WTOOLS — Convert JSON to PHP Serialized string (`wtools.io/convert-json-to-serialize-string`) | Paste JSON → serialized string, single button. (Site TLS cert expired at fetch time.) |
| 2 | CodeBeautify — JSON Serialize Online (`codebeautify.org/json-serialize-online`) | Paste / URL-load / file-upload JSON, copy + download output. (403 to bots.) |
| 3 | OneClick Pro — PHP Serializer (`oneclick.pro/php-serializer/`) | JSON ⇄ serialized, both directions. |
| 4 | Web Lite Solutions — Serialized PHP ⇄ JSON (`solutions.weblite.ca/php2json/`) | Two-way converter. |
| 5 | Dev Lateral — Serialize PHP Online (`devlateral.com/tools/serialize-php-online`) | Tabbed input: plain **Text** / **PHP Array** / **JSON**; flags PHP Object Injection risk of unserialize on untrusted data. |

## Feature diff (in-model = fits a text-in/text-out browser tool)

| Capability | Competitors | gizza php-serialize | Verdict |
| ---------- | ----------- | ------------------- | ------- |
| JSON → serialized string | all | ✅ | parity |
| Nested arrays/objects (any depth) | most | ✅ (recursive) | parity |
| **Byte-accurate string length** (`s:<bytes>` for multi-byte UTF-8) | inconsistent — some count characters, which breaks `unserialize()` | ✅ `é`→`s:2`, `€`→`s:3` | **ahead** (correctness) |
| **Preserve object key order** | inconsistent | ✅ (`preserve_order`) | **ahead** |
| int vs float tagging (`i:` vs `d:`), big-int → double fallback | varies | ✅ matches PHP serialize_precision=-1 | parity / ahead |
| `null`/`bool` mapping (`N;`, `b:1;`/`b:0;`) | all | ✅ | parity |
| Clear error on invalid JSON | varies | ✅ (`invalid JSON: …`) | parity |
| Copy / download output | many | page template responsibility (not per-tool) | n/a |
| Runs locally / private / offline | a few | ✅ (WASM, client-side) | parity |

## Out-of-model (intentionally NOT built)

- **Reverse direction (PHP serialized → JSON / `unserialize`)** — a distinct
  parser; belongs in a separate `php-unserialize` tool, not this one's contract.
- **Plain-text / PHP-array-literal input tabs** (Dev Lateral) — a different,
  PHP-specific parser. JSON is the canonical structured-interop input and covers
  the documented use case (writing data a PHP app reads). Adding a second input
  grammar would change the tool's single-purpose contract.
- **URL-load / file-upload input** (CodeBeautify) — generic page-shell input
  affordances, not part of this tool's compute contract.

## Conclusion

The tool reaches **parity on every in-model capability** and is **ahead on the two
correctness details competitors most often get wrong** — byte-length string sizing
for multi-byte UTF-8 and preserved object key order, both of which are required for
the output to round-trip through PHP's `unserialize()`. No in-model capability,
copy, UX, or visual gap requires a code change. Remaining competitor features are
either a separate tool (reverse direction, alternate input grammars) or page-shell
affordances (copy/download/URL/file) rather than this tool's logic.

## Verification (all surfaces)

- `cargo test --workspace` — 7 core + 1 drift-guard schema test pass.
- `wafer build` — chat `block.wasm` validates/instantiates (308.9 KiB).
- CLI — `gizza tool php-serialize json='{"name":"Al","age":30}'` →
  `a:2:{s:4:"name";s:2:"Al";s:3:"age";i:30;}`; `'"café"'` → `s:5:"café";`;
  invalid JSON exits 1 with `invalid JSON: …`.
- Page — Playwright `tool-page-php-serialize.spec.ts` passes (object → expected
  serialized string).
