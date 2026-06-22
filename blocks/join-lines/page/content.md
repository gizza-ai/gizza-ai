## About this tool

**Join Lines** merges a list of lines into a single line — paste your lines and
choose what goes between them.

- **Separator** — the text placed between lines. Type a comma, a space, a pipe
  `|`, or anything you like. Use `\t` for a tab and `\n` for a newline, and `\\`
  for a literal backslash. Leave it blank to concatenate the lines with nothing
  in between.
- **Trim each line** — strip leading and trailing whitespace from every line
  before joining.
- **Remove blank lines** — drop empty or whitespace-only lines instead of
  joining them.
- **Prefix each line** / **Suffix each line** — wrap every line, for example with
  quotes to build a comma-separated list or parentheses for a SQL `IN (…)` clause.

Everything runs **locally in your browser** via WebAssembly — your text is never
uploaded.

### Handy for

- Turning a column of values into a comma-separated list for a spreadsheet, CSV
  or config file.
- Building a SQL `IN ('a', 'b', 'c')` clause from a pasted list of IDs.
- Collapsing a multi-line snippet into one line for a single-line field or log.
- Joining words or tokens with a custom delimiter such as a tab or pipe.
