# indent-block-text — competitor scan + design decisions (2026-08-17)

Backlog row: `indent-block-text` / "Adds a configurable number of spaces or a fixed prefix to the start of every line." / `pure`.

## Duplicate check

Existing text tools cover trimming whitespace, converting tabs/spaces, adding line numbers, removing empty lines, and wrapping/unwrapping text. None ship the exact operation of adding a configurable prefix to every selected line, with outdent/dedent as reverse operations. Not a duplicate.

## Competitors reviewed

### Browserling add prefix to lines
- Core behavior: prepend a fixed string to every line of pasted text.
- UX pattern: input textarea, prefix field, output textarea.

### TextFixer add text to beginning/end of lines
- Core behavior: add text before each line, commonly for bullets, quotes, or code comments.
- UX pattern: simple prefix/suffix controls and worked examples.

### Python `textwrap.indent` / `textwrap.dedent`
- Standard library reference for adding a prefix per line and removing common indentation.
- UX pattern: predicate/scope and blank-line handling matter because blank-line prefixes can create trailing whitespace.

## Table stakes → decision

| Capability | Verdict | How it lands here |
| --- | --- | --- |
| Add spaces to every line | in-model — built | `mode=indent`, `style=spaces`, `count` |
| Add tabs to every line | in-model — built | `style=tabs` |
| Add a fixed prefix | in-model — built | `style=custom`, `prefix`, `count` repeats |
| Outdent fixed indentation | in-model — built | `mode=outdent` |
| Dedent common whitespace | in-model — built | `mode=dedent` |
| Skip blank lines | in-model — built | `skip_blank_lines` checkbox |
| Hanging indent / first-line / paragraph starts | in-model — built | `lines` enum |
| Preserve trailing newline | in-model — built | core split/rejoin preserves final newline |
| Rich editor selection integration | out-of-model | Browser page uses textarea, not a code editor |

## UX patterns adopted

- Multiline textarea for the text block.
- Selects for mode, style, and line scope.
- Numeric count field with max boundary.
- Prefix field with examples such as `> `, `# `, and `// `.
- Preset chips for four spaces, Markdown quote, hanging indent, and dedent.

## Not copied

No competitor copy, branding, examples, or assets were reused. Terms such as prefix, indent, outdent, and dedent are standard domain vocabulary.
