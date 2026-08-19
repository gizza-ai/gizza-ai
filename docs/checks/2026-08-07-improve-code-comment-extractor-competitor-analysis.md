# code-comment-extractor — competitor analysis (2026-08-07)

Scan run BEFORE implementing, per `/create-next-tool` step 4. All notes are **paraphrased**
observations of behaviour and parameters; no competitor copy, branding, or trademarks are
reproduced. Sources are named only to identify what was examined.

Search: "extract comments from source code tool online comment extractor strip comments
multi-language".

## Competitors examined

### 1. `multilang-extract-comments` (npm library)

- Extracts comments from more than just JavaScript: its documented examples cover JS, Python, C
  and Handlebars, plus PowerShell, and the language set is extensible.
- Language selection is driven by a `filename` option (the extension picks the pattern set), with
  an escape hatch: a caller-supplied `pattern` object that declares the language name, its matching
  extensions, its line-comment openers, and its block-comment start/middle/end strings.
- Per-comment output is a record, not a bare string: start line, end line, the line where the
  following code begins, the comment content, the first line of that following code, and an `info`
  block carrying the comment *type* (single-line vs multi-line) and an api-doc flag.
- The overall return value is keyed by line number, so position is a first-class part of the result.

### 2. `CommentLister` (JVM CLI, comment inventory over a repo)

- Language coverage is the headline: C/C++, Java, ECMAScript, C#, Python, PHP and Ruby, with build
  files handled too. Each language has a real lexer behind it rather than a regex.
- Runs over a git repository and can be pointed at a specific revision, i.e. its input unit is a
  tree of files, not a pasted snippet.
- JSON output per comment: the comment text, the line number, and the character offset within that
  line. Around that it reports repository metadata, per-file comment counts, per-file-type
  statistics, and how long the scan took — so *aggregate comment statistics* are treated as a
  deliverable in their own right, not just the comment list.

### 3. dCode — HTML/JavaScript/CSS comments extractor (hosted web tool)

- Single large paste area for "source of a web page" plus one Extract button — the minimal-input UX
  a hosted tool converges on.
- Deliberately mixed-syntax: in one pass it recognises HTML `<!-- -->`, JS `//` and `/* */`, and CSS
  `/* */`, and labels which syntax each result came from.
- Output affordances beyond the list: copy-to-clipboard and export as `.csv` or `.txt`.
- Carries an on-page FAQ explaining the comment syntaxes it recognises.

### 4. `comment-scanner` (PyPI module) — cross-check on scope

- Positions itself as multi-language and, importantly, distinguishes **single-line, in-line, and
  multi-line** comments as separate categories. Confirms that comment *kind* is an expected axis of
  the output, matching multilang-extract-comments' `info.type`.

## Table stakes → in-model / out-of-model

| # | Table stake observed | Verdict | Where it lands |
|---|---|---|---|
| 1 | Many languages, not just one | **in-model** | `language` enum: auto + 16 named languages |
| 2 | Auto-detect the language instead of forcing a choice | **in-model** | `language = "auto"` (default) |
| 3 | Line comments AND block comments in one pass | **in-model** | per-language profile carries both |
| 4 | Comment *kind* reported / filterable (single vs multi vs doc) | **in-model** | `kind` enum filter: all / line / block / doc |
| 5 | Doc-comment (api-doc) flag as a distinct category | **in-model** | `/** */`, `///`, `//!`, `##`, `=begin`, Python docstrings classified as `doc` |
| 6 | Line number (and column) per comment | **in-model** | JSON/markdown outputs carry line+column; `line_numbers` adds them to the plain list |
| 7 | Structured machine-readable output | **in-model** | `output = "json"` (array of line/column/kind/text) |
| 8 | Strings must not be mistaken for comments | **in-model** | shared string/char-literal tokenizer skips `"// not a comment"` |
| 9 | Strip the comment markers vs keep them verbatim | **in-model** | `strip_markers` boolean (default on) |
| 10 | Aggregate comment statistics (counts, density) | **in-model** | `output = "stats"` — total/line/block/doc counts, comment lines, code lines, density % |
| 11 | The inverse operation: source with comments removed | **in-model** | `output = "stripped"` — this row's "strips out" half |
| 12 | Tabular export | **in-model** | `output = "markdown"` (table); JSON covers the CSV-ish need |
| 13 | Copy / download the result | **in-model, free** | generator gives every `format = "text"` page Copy + Download |
| 14 | Preset examples to click | **in-model** | three `[[example]]` chips on the page |
| 15 | Minimum-length filter to drop noise like `// x` | **in-model** | `min_length` integer |
| 16 | Scan a whole git repo / directory tree at a revision | **out-of-model** | gizza tools take one pasted input; no VCS or filesystem access |
| 17 | Per-file and per-file-type roll-up statistics | **out-of-model** | follows from 16 — single input means a single-file stat block |
| 18 | The code line that follows each comment (`codeStart` / `code`) | **out-of-model** | needs code-vs-comment association heuristics beyond a lexer; the `line` field already lets a user jump there |
| 19 | Caller-supplied custom comment-syntax pattern object | **out-of-model** | a nested object param is a poor fit for a chat/CLI/URL param surface; the `auto` + 16-language table covers the common syntaxes instead |
| 20 | A real per-language parser (ANTLR-style grammars) | **out-of-model** | parser-generator grammars are C libraries that do not instantiate under `wasm32-wasip1` (see `references/wasm-crates.md`); a deterministic tokenizer is the shipped approach and its limits are stated on the page |

## Defaults chosen (and why)

- `language = "auto"` — hosted competitors ask for zero configuration; detection picks a profile
  from the comment/keyword shapes present and the user can always override.
- `output = "comments"` — the row's primary verb is "lists all comments"; the plain list is what a
  first-time user wants to see.
- `kind = "all"` — filtering is opt-in.
- `strip_markers = true` — every competitor shows comment *content*, not the delimiters.
- `docstrings = true` — Python has no block-comment syntax, so a docstring is its doc comment; a
  Python user expecting comments and getting nothing back would be a surprise.
- `line_numbers = false`, `min_length = 0` — quiet defaults; both are one click away.

## UX fit decisions

- Big multiline paste box for `code` (`multiline = true`) with a mixed-syntax placeholder, matching
  the single-textarea pattern hosted tools use.
- `[input.labels]` gives every enum a friendly `<select>` label ("Auto-detect", "C#", "Shell /
  Bash", "Source with comments removed") while the wire values stay canonical.
- Three `[[example]]` chips stand in for the preset buttons competitors ship: a JavaScript extract,
  a Python docstring/`#` mix, and a strip-comments run.
- Copy + Download come free from `format = "text"`; no custom JS.

## Limits stated on the page

Tokenizer, not a parser: unbalanced `/*` runs to end of input; Perl/Ruby POD-style blocks beyond
`=begin`/`=end` are not special-cased; a `#` inside a shell here-doc body can read as a comment.
Output is capped at 50,000 comments.
