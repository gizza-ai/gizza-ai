# regex-literal-escape — competitor analysis (2026-08-09)

Scan run during the build, per `/improve-tool` Phase 2–3. Findings are paraphrased; no competitor copy, branding, or assets were reused.

## Competitors reviewed

| # | Tool/reference | Shape |
|---|---|---|
| 1 | Beautify Code regex escape tool | Single input → escaped regex text, aimed at generic special-character escaping |
| 2 | regex-escape.com online escaper | Small paste box for escaping literal text for regex use |
| 3 | MDN `RegExp.escape()` reference | Documents strict ECMAScript escaping semantics and edge cases |
| — | Also surveyed: general regex tester/docs pages and language helpers (`preg_quote`, `re.escape`, `QuoteMeta`) | Confirmed that escaping rules differ across engines |

## Table stakes observed

| Capability | Seen in | Defaults observed | Verdict |
|---|---|---|---|
| Paste arbitrary literal text and return escaped regex fragment | 1, 2 | Generic regex escaping | **in-model** → required `text` field |
| Preserve literal punctuation such as `.`, `*`, `+`, `(`, `)` | 1, 2 | Backslash escaping | **in-model** → core escape tables |
| JavaScript-specific escaping | 3 | `RegExp.escape()` safe output | **in-model** → `flavor=javascript` and `javascript-strict` |
| PCRE/PHP delimiter escaping | PHP docs/tools | delimiter supplied separately | **in-model** → `delimiter` param |
| Python/Go/.NET/Java/Ruby/Rust helper parity | language docs | exact helper behavior | **in-model** → flavor enum covers each helper |
| Source-code string literal output | developer use cases | double backslashes | **in-model** → `string_literal` checkbox |
| Whitespace-safe output for extended/free-spacing mode | PCRE/Ruby `/x` docs | explicit escaping needed | **in-model** → `escape_whitespace` checkbox |
| Examples/presets for common languages | 1, docs | common punctuation sample | **in-model** → example chips in `meta.toml` |
| Regex validation/testing | regex testers | run a pattern against sample text | **out-of-model for this tool** — covered by regex testers; this block focuses on literal escaping |

## Out-of-model or intentionally not built

- Full regex syntax validation and match testing — a different tool shape; this one emits a literal fragment.
- Engine runtime execution against samples — not needed to escape a literal and would require bundling multiple regex engines.
- Automatic detection of target regex flavor — impossible from a text literal alone; the user must choose the runtime.

## Gaps closed in this build

The descriptor exposes nine flavors, delimiter escaping, whitespace escaping, and source string-literal output. Page examples cover PHP/PCRE, JavaScript, Go/RE2, Java, Python URL matching, and extended mode. Core tests pin exact outputs for the supported language helpers and error cases.

## Notes

- Java `Pattern.quote()` deliberately uses `\Q...\E`, including embedded `\E` splicing.
- `javascript-strict` follows `RegExp.escape()`-style `\xNN` behavior for first alphanumeric characters and unsafe punctuation.
- No competitor wording or UI assets were copied.
