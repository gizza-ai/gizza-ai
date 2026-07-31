# html-diff — competitor analysis (2026-07-31)

Tool: diff the visible text of two HTML snippets while ignoring tag and attribute noise.
Type: pure text/HTML parser.

## Competitor scan

### 1. Online HTML diff/checker tools
- Compare pasted HTML and highlight differences.
- Table-stakes: two text areas, ignore whitespace/case options, inline highlights, examples.

### 2. Text compare tools with HTML stripping options
- Compare rendered or extracted text rather than source markup.
- Table-stakes: visible text extraction, line/word modes, unified or inline output.

### 3. Developer diff libraries (diff-match-patch, jsdiff, htmldiff)
- Provide word/line operations and structured output for apps.
- Table-stakes: programmatic JSON/change operations, unchanged/added/removed counts.

## Table-stakes distilled

| Capability | In/out of model | Decision |
| --- | --- | --- |
| Two HTML text inputs | in-model | built |
| Strip tags/attributes before comparison | in-model | built via visible-text extraction |
| Line and word granularity | in-model | built (`granularity=line|word`) |
| Unified/inline text output | in-model | built (`format=unified`) |
| Structured JSON output | in-model | built (`format=json`) |
| Ignore case | in-model | built |
| Ignore whitespace | in-model | built |
| Context-line control | in-model | built for line unified output |
| DOM visual rendering / screenshot diff | out-of-model | not built; requires browser layout engine/images |
| CSS cascade/script execution | out-of-model | not built; snippets are text-extracted only |

## Design decisions

- Compare visible text, not raw source, to avoid false positives from class/style/wrapper changes.
- Use `nanohtml2text` to match existing visible-text behavior in the repo.
- Keep output plain text for CLI/page parity and JSON for automation.
- Word mode uses compact `[-old-]` / `{+new+}` markers because unified hunks are line-oriented.

## Verification plan

Unit tests cover tag/attribute ignoring, line unified output, word inline markers, JSON changed pairs, ignore-case, ignore-whitespace, empty input, one-side-empty input, bad enum values, and wrapper behavior. Page tests cover exact line output, word output, and deep-link JSON/ignore options.
