# convert-quotes — competitor analysis (2026-08-29)

Scan run while finishing the tool, per the create-next-tool / improve-tool procedure. All competitor behaviour below is paraphrased from search results and quick page-shape inspection; no competitor copy, branding, markup or trademarks were reproduced, and out-of-model items are listed rather than built.

## Scope

Search used: "online single quote to double quote converter escape quotes code smart quotes converter". The result set splits into two adjacent markets: JSON/code quote fixers that normalize single/backtick/smart quotes to JSON-valid double quotes, and prose smart-quote converters that move between straight and typographic punctuation.

| # | Tool (function) | Reachable | Shape |
|---|-----------------|-----------|-------|
| 1 | JSON quote fixer | yes | Browser text box, normalizes non-JSON quotes to double quotes |
| 2 | Citation-style smart quote converter | yes | Prose-focused straight ↔ curly quote conversion |
| 3 | TextGround-style smart quote converter | yes | Straight ↔ curly quotes, single/double/apostrophes |
| 4 | Text toolbox smart quote converter | yes | Typographic quote conversion plus punctuation cleanup |
| 5 | MonoCalc smart quotes converter | yes | Prose typography conversion with apostrophe/dash/ellipsis handling |

## What each one actually signals

**1 — JSON quote fixer.** The closest code-oriented competitor. It converts single quotes, smart quotes and sometimes backticks into JSON-compliant double quotes. Table-stakes: paste input, one-click output, apostrophe awareness, and keeping the result syntactically usable. Its scope is intentionally JSON-shaped rather than a general bidirectional delimiter converter.

**2 — citation/essay quote converter.** Prose-oriented. It converts straight quotes to curly quotes and back, with apostrophes handled as text punctuation rather than delimiters. Table-stakes: smart/curly support and preserving contractions.

**3 — TextGround-style smart quote converter.** Similar straight ↔ curly converter with controls for single quotes, double quotes and apostrophes. It confirms users expect separate treatment of double quotes, single quotes and apostrophes.

**4 — text-toolbox smart quote converter.** Combines quote conversion with related punctuation cleanup such as dashes and ellipses. Useful for publishing copy, but that broader cleanup overlaps existing gizza tools and is not the core of this backlog item.

**5 — MonoCalc smart quote converter.** Prose/publishing tool that educates straight quotes into directional curly quotes and straightens them back. It reinforces the need for curly input support but not code-safe escaping.

## Table-stakes checklist → shipped decisions

Every item is tagged in-model (browser-local, pure Rust/wasm, no account, no server) or out-of-model, and every in-model item appears in the descriptor or page controls.

| Table-stake | Verdict | Where it landed |
|---|---|---|
| Paste text/code input | in-model | multiline `input` field |
| Single quotes → double quotes | in-model | `direction = "single-to-double"` (default) |
| Double quotes → single quotes | in-model | `direction = "double-to-single"` |
| Curly/smart quote input | in-model | `smart-to-double`, `smart-to-single`, `auto-to-double`, `auto-to-single` |
| Mixed quote normalization | in-model | `auto-to-*` reads straight and curly single/double delimiters together |
| Swap single and double delimiters in one pass | in-model | `direction = "swap"` |
| Preserve apostrophes/contractions | in-model | `preserve_apostrophes` default true |
| Respect backslash escapes | in-model | parser skips escaped delimiters and rewrites escaped quote bodies |
| Code-safe escaping for the new delimiter | in-model | `escape_style = backslash` default |
| SQL/CSV doubled-quote output | in-model | `escape_style = doubled` |
| Prose/no-escape output | in-model | `escape_style = bare` |
| Unbalanced quote handling | in-model | `on_unbalanced = keep|error` |
| Counts/report for auditing | in-model | `include_report` JSON report |
| Straight → curly education | deliberately not shipped | existing `smart-quotes-converter` already handles typographic education |
| Dash/ellipsis/space typography cleanup | deliberately not shipped | existing `smart-quotes-clean` covers typographic punctuation cleanup |
| Full language parsing for JS/Python/Rust comments, regexes, raw/triple strings | out-of-model | would require per-language parsers; this tool is a deterministic delimiter converter |
| JSON repair of arbitrary invalid JSON | out-of-model for this tool | separate JSON repair/beautify tools cover JSON-specific recovery |

## UX control patterns adopted

- Preset chips cover Python single → double, JS double → single, curly → straight, SQL-style doubling, and mixed-style normalization.
- The main input placeholder shows code, an escaped apostrophe and curly quotes so users see the non-find-and-replace cases immediately.
- Dropdown labels describe outcomes (`'x' → "x"`) instead of internal enum names.
- Boolean controls expose apostrophe preservation and JSON report counts without crowding the common path.
- The page states the 1,000,000-byte cap and explains that this is not a full language parser.

## Considered, deferred, rejected

- **Straight → curly typographic education**: rejected as a duplicate of the existing smart-quotes converter. This tool reads curly quotes but emits straight delimiters for code/data workflows.
- **Backtick/template literal conversion**: deferred. Backticks in JavaScript carry interpolation and multiline semantics; a safe converter needs language awareness rather than a delimiter swap.
- **Language-specific AST rewriting**: out of model for this generic pure-text block. It would be better as separate formatter/parser-backed tools.
- **Blind global replacement mode**: rejected because it is the failure mode this tool exists to avoid; preserving escapes and apostrophes is the differentiator.
