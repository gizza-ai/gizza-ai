# ini-parser — competitor analysis (2026-07-17)

Backlog row: **ini-parser** / "Parses INI/conf files (sections, keys, comments) into
structured JSON and reports duplicate keys." / type: pure.

## Competitors scanned

1. **Ez Parser — INI Parser** (ezparser.com/ini-parser) — sections, key=value pairs,
   comments (`;`/`#`), quoted values, multiline values, boolean & numeric **type
   detection**, duplicate keys → **last value takes precedence**. Client-side. Controls:
   Clear / Load Sample / Parse / Copy JSON. No output-format toggles, no dot-notation.
2. **jc `ini` parser** (kellyjonbrazil.github.io/jc) — comments `#` and `;` (own line);
   quotes stripped by default (`-r`/`raw` to keep); duplicate keys → last value wins;
   **section-less/global keys at root**; delimiters `=` **and** `:`; "missing values"
   supported. Output = nested dict, sections nested under their name; a section named
   the same as a top-level key overwrites it.
3. **CodeShack INI→JSON** (codeshack.io) — parses sections, key/value, comments, "different
   data types"; pretty-print **and** minified output. Browser-side.
4. **Site24x7 INI→JSON** (site24x7.com/tools) — plain INI→JSON transform, no options.
5. **Python `configparser`** (docs.python.org) — the canonical semantics reference:
   `comment_prefixes` (default `#`,`;`) vs `inline_comment_prefixes` (default OFF);
   duplicate key/section handling is a **flag** (error vs last-wins); `=`/`:` delimiters;
   `[DEFAULT]`/global handling; keys lowercased by default (an option).

## Table-stakes → decision

| Capability | Fit | Decision |
| --- | --- | --- |
| `[section]` headers → nested JSON | in-model | `output=json` nests each section; duplicate sections merge |
| `key = value` **and** `key: value` delimiters | in-model | split on first `=` or `:` |
| Comments `;` and `#` (own line) | in-model | `comments` enum: both / semicolon / hash |
| **Inline** comments (`val ; note`) | in-model | `inline_comments` boolean, default **off** (matches configparser) |
| Section-less / global keys at root | in-model | globals go to root (json) / `report.globals` |
| Quoted values (strip surrounding quotes) | in-model | always strip matched surrounding `"`/`'`, document it |
| Type detection (bool / int / float) | in-model | `detect_types` boolean, default **off** (lossless by default) |
| **Duplicate-key policy** (row headline) | in-model | `duplicate_keys` enum: last / first / array / error |
| **Report duplicate keys** (row headline) | in-model | `output=report` lists every duplicate + stats |
| Flat dotted output (`section.key`) | in-model | `output=flat` |
| Multiline / value-continuation | edge | out of scope v1 — documented as a limit (rare, ambiguous across dialects) |
| PHP `ini` constants / `${}` interpolation | out-of-model | not built (dialect-specific; documented) |
| Case-lowercasing keys | out-of-model (skipped) | keys kept verbatim (lossless); documented |

Every table-stake lands in the descriptor except the two documented limits/out-of-model
rows above. No competitor copy, branding, or trademarks are reproduced.

## Differentiators we ship

- **Duplicate-key policy is first-class** (`last`/`first`/`array`/`error`) — most
  competitors silently last-wins; ours makes the choice explicit and can error.
- **`report` output** surfaces a `duplicates` list (section, key, count, values) plus a
  `stats` block (sections / keys / comments / duplicates) — directly answers the row's
  "reports duplicate keys". No scanned competitor reports duplicates at all.
- **Three output shapes** (nested json / flat dotted / report) vs competitors' single
  nested-JSON dump.
- 100% in-browser / offline, no upload, no sign-up (matches the privacy table-stake).
