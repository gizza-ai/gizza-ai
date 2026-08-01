# task-format-converter — competitor analysis (2026-07-31)

Tool: convert task lists between todo.txt, Markdown checklist, JSON, and CSV while preserving priorities, tags, projects, contexts, and dates.
Type: pure text/parser converter.

## Competitor scan

### 1. todo.txt web editors and converters
- Table-stakes: parse `(A)` priorities, leading completion markers, `+project`, `@context`, and `due:` metadata.
- UX patterns: format selectors, paste area, examples for todo.txt syntax.

### 2. Markdown checklist / GitHub task-list utilities
- Table-stakes: support `- [ ]` and `- [x]` items, preserve checked state, keep normal prose text intact.
- UX patterns: source/target dropdowns, copyable plain-text output, worked examples.

### 3. CSV/JSON task import tools
- Table-stakes: fixed or discoverable columns, boolean completion fields, array/object JSON, stable CSV header for spreadsheets.
- UX patterns: pretty JSON toggle, explicit import/export formats, sample rows.

## Table-stakes distilled

| Capability | In/out of model | Decision |
| --- | --- | --- |
| Convert todo.txt, Markdown checklist, JSON, and CSV | in-model | built |
| Auto-detect source format | in-model | built (`auto`) |
| Preserve completion state | in-model | built |
| Preserve priority | in-model | built for single uppercase todo.txt priority |
| Preserve `+project` and `@context` tags | in-model | built |
| Preserve created, due, completed dates | in-model | built; Markdown stores non-native dates as metadata tags |
| Preserve extra `key:value` metadata | in-model | built |
| Pretty JSON toggle | in-model | built |
| Sync with hosted task services | out-of-model | not built; needs network/API credentials |
| Natural-language task extraction | out-of-model | not built; separate ML/NLP class |

## Design decisions

- Use a shared task model so every format pair goes through the same parser/writer path.
- Keep CSV column order fixed for predictable spreadsheet and script output.
- Let users choose `from=auto` for convenience while keeping explicit source formats for ambiguous text.
- Preserve rather than normalize user metadata: unsupported target-native fields are emitted as portable `key:value` tags.

## Verification plan

Unit tests cover todo.txt metadata, completed todo.txt dates, Markdown checkboxes, JSON to todo.txt, Markdown to CSV round trip, date preservation, auto-detection, empty input, invalid JSON, unknown formats, CSV header errors, and pretty JSON. Page tests cover exact todo.txt to Markdown output, Markdown to JSON output, and deep-linked CSV to todo.txt output.
