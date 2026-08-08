# json5-convert competitor analysis (2026-08-08)

Tool: `json5-convert` — convert JSON5/JSONC (comments, trailing commas, unquoted keys)
to strict JSON and back.

## Sources scanned

Search query: `JSON5 to JSON converter online tool remove comments trailing commas unquoted keys`.

Reachable competitors reviewed (fetched and read, 2026-08-08):

1. **GoTool — JSON5 → JSON** (`gotool.io/en/tools/json5-to-json/`). Input/output panes with
   character counts, a Convert button, a Share button and a "Load example" link. Advertises
   handling of every JSON5 extension: `//` and `/* */` comments, trailing commas, unquoted
   identifier keys, single-quoted strings, hex numbers, multiline strings, `+Infinity`,
   `-Infinity`, `NaN`. FAQ covers the JSON5 feature list, the fact that comments are stripped,
   and typical use cases (tsconfig, Babel configs). No formatting options exposed.
2. **HCODX — JSON5 to JSON Converter** (`hcodx.com/tools/json5-to-json`). Auto-convert on input
   change, manual Convert button, Copy result, Download as `data.json`, and a link to a separate
   JSON → JSON5 tool for the reverse direction. One bundled sample demonstrating comment removal,
   single→double quotes, trailing-comma removal, key quoting and number normalization. Documents
   that `Infinity`/`NaN` cannot be represented in strict JSON and trigger an error, and that
   comments are silently removed. Browser-side, UTF-8 safe. Six FAQ entries.
3. **json5.net — JSON5 & JSON Formatter/Validator/Converter**. A suite page: Format, Minify,
   Beautify, Validate JSON, Validate JSON5, plus many conversions (JSON5 → JSON, XML ↔ JSON,
   CSV → JSON, YAML ↔ JSON, JSON → TypeScript interface, JSON → Python dict), JSON diff, JSON
   merge, a REST API formatter and a JWT decoder. No FAQ section.
4. **Hexmos JSON5 Converter** (from search description). Converts JSON5 → JSON, validates syntax
   and lets the user "specify indentations". Free, no registration.
5. **OmniConvert JSON5 → JSON** (from search description). Same core conversion but
   **server-side** with a metered free tier ("2 free conversions to start").

## Table-stakes capabilities

| Capability / UX pattern | In current gizza model? | Decision |
| --- | --- | --- |
| Paste JSON5/JSONC into a textarea, get JSON out | Yes | Required multiline `text` param; page renders a `<textarea>`. |
| Strip `//` and `/* */` comments | Yes | Parser skips both; documented as always-dropped. |
| Remove trailing commas | Yes | Parser accepts, writer omits (or re-adds on request). |
| Quote unquoted identifier keys | Yes | Strict-JSON writer always double-quotes keys. |
| Single-quoted → double-quoted strings | Yes | Strict-JSON writer always uses `"`. |
| Hexadecimal / `.5` / `5.` / `+1` / `007` numbers normalized | Yes | Normalized as text (no f64 round trip → big ints keep precision). |
| `NaN` / `Infinity` / `-Infinity` handling | Yes, and wider | Competitors either hard-error (HCODX) or say nothing; we expose `nonfinite = null \| string \| error`, default `null` (matches `JSON.stringify`). |
| Multiline strings (backslash line continuation) | Yes | `\`+newline continuation, plus `\x`, `\v`, `\0`, `\uXXXX` incl. surrogate pairs. |
| Indentation choice | Yes | `indent = 2 \| 4 \| tab \| minify` — covers Hexmos's "specify indentations" and json5.net's Format/Minify/Beautify. |
| Reverse direction (JSON → JSON5) | Yes, in ONE tool | HCODX needs a second tool for this; we ship `direction = to-json5` with `quote_style`, `unquote_keys`, `trailing_commas`, plus `direction = auto`. |
| Auto-convert as you type | Yes | The page recomputes on every input event; no Convert button needed. |
| Copy / Download the result | Yes | Generic page affordances for `format = "text"` (Copy + Download link). |
| Load-an-example button | Yes | Five `[[example]]` preset chips (JSONC config, minify, JSON→JSON5, hex/NaN, sort keys). |
| Validation with a useful error | Yes | Parse errors report line and column; unterminated strings/comments name the opening position. |
| Runs locally, nothing uploaded | Yes | WebAssembly in the tab (OmniConvert is server-side + metered; we are neither). |
| Sort keys alphabetically | Yes — beyond table stakes | `sort_keys`, recursive; asked for by config-diffing workflows, absent from all five competitors. |
| Share-a-link button | Effectively yes | The page already supports `?param=` deep links, which carry the input and every option; a dedicated Share button is site chrome, not block scope. |
| Syntax highlighting / dual-pane editor with line numbers | No | Out of model: this repo renders generic pages with a plain textarea + output pane; a CodeMirror-class editor is a site-level concern. |
| JSON Schema validation | No | Out of model here — a distinct tool, not a dialect conversion. |
| XML/CSV/YAML/TypeScript/Python conversions, JSON diff, JSON merge, JWT decode (json5.net suite) | No | Out of scope for this block; gizza covers these as separate tools. |
| Preserve comments through the round trip | No | Impossible by construction: strict JSON has no comment syntax, so comments cannot survive the intermediate representation. Documented as a limit rather than silently dropped. |
| File upload / drag-and-drop of a `.json5` file | No | Out of model for a pure text block on this page runtime (file inputs are for media/asset tools); paste and the 1 MB cap cover the realistic config-file sizes. |

## Defaults chosen

- `direction = to-json` — the overwhelmingly common ask ("make this parseable"), and what every
  competitor does by default.
- `indent = 2` — the JSON config convention (npm, tsconfig, VS Code all write 2).
- `nonfinite = null` — matches `JSON.stringify`, so pasted JS values behave the way users expect;
  `error` is available for people who want the strictness HCODX hard-codes.
- `quote_style = single` and `unquote_keys = true` — the JSON5 house style, so the reverse
  direction produces idiomatic JSON5 rather than JSON with different brackets.
- `trailing_commas = false` — additive-only, off by default; on when you want one-line diffs.
- `sort_keys = false` — preserving the author's key order is the safer default for a config file.

## Examples and controls

Five `[[example]]` chips prefill realistic inputs (a commented dev-server config, a strip-and-minify
pass, a JSON→JSON5 round trip with trailing commas, loose numbers with `NaN`/`Infinity`, and a
sort-keys diff cleanup). Enum params use `[input.labels]` so the dropdowns read as
"JSON5 / JSONC → strict JSON", "Minify — one line" and "Convert to null" rather than raw values.
The input field is `multiline = true` so pasted config files keep their newlines.

No competitor copy, branding, wording or trademark was reproduced; the table above records
behaviour observed on public pages and the decisions taken for this block.
