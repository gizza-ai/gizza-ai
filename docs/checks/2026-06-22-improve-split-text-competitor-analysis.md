# split-text — competitor analysis (2026-06-22)

Tool: split text on a chosen delimiter into one item per line.
Surfaces verified: chat block (`wafer build` OK), CLI (`gizza tool split-text`), standalone page (Playwright, 6/6).

## Top competitors surveyed

1. **onlinetexttools.com — "Split a Text"** — split by character, by length, by
   regex, or by a custom symbol; options to trim each piece and skip empty pieces;
   choice of join separator for the output.
2. **browserling tools — "Text Splitter"** — same engine family as onlinetexttools;
   split by delimiter / length / regex, output one per line.
3. **textfixer.com / convert.town "comma separated to lines"** — single-purpose:
   turn a comma- or delimiter-separated list into one item per line; basic trim.
4. **textcompare.org / text-utils "split text"** — split by delimiter/newline/space,
   remove empty lines, sort/dedupe after split.
5. **Various "string to array / list" dev utilities** — split on delimiter, trim
   whitespace, drop empties, optionally quote each item.

## Feature diff vs. gizza split-text

| Capability | Competitors | gizza split-text | Status |
| --- | --- | --- | --- |
| Split on a literal delimiter (incl. multi-char) | yes | yes (`delimiter`, default `,`) | covered |
| Escapes for tab/newline in a single-line field | partial | yes (`\n \t \r \\`) | covered (parity+) |
| Split on whitespace (collapse runs) | yes | yes (`mode=whitespace`) | covered |
| Split into characters | yes | yes (`mode=chars`, Unicode-correct) | covered |
| Trim each item | yes | yes (`trim`) | covered |
| Remove empty items | yes | yes (`remove_empty`) | covered |
| One-item-per-line output | yes | yes (always; newline-joined) | covered |
| Deterministic, private, in-browser | partial | yes (pure Rust/wasm, no server) | covered (advantage) |

## Gaps considered and decisions

- **Split by regex.** Competitors offer a regex delimiter. Deliberately NOT added:
  it needs a regex engine dependency and a much larger error/UX surface, and the
  three modes here (literal + whitespace + chars) cover the overwhelming majority
  of real splitting needs. Documented as out-of-scope rather than half-built.
- **Split by fixed length (every N characters).** A distinct operation (chunking,
  not delimiting) that belongs in its own tool; left out to keep this tool focused.
- **Custom output join separator.** This tool's contract is specifically
  "one item per line"; a configurable joiner is the inverse of the existing
  `join-lines` tool, so it stays out to avoid overlap.
- **Sort / dedupe after split.** Already provided by the existing `sort-lines`,
  `remove-duplicate-lines`, and `find-unique-lines` tools — composable, not
  duplicated here.

## Copy / UX / visual

- Page copy, FAQ, and an examples table were written from scratch (no competitor
  copy/branding/trademarks copied), oriented to the common jobs: comma-list→lines,
  tab/spreadsheet splitting, character listing, blank-line cleanup.
- The page delimiter field starts empty; the web wrapper falls back to the comma
  default in literal mode so a blank field still produces a sensible split (matches
  the chat/CLI schema default of `,`). Verified by Playwright.

## Conclusion

split-text reaches feature parity with mainstream delimiter-splitting tools for
the in-model (pure, no-regex) capabilities, with a Unicode-correct chars mode and
escape handling as small advantages. Regex / fixed-length splitting are recorded
as intentional out-of-scope items.
