# python-syntax-check — competitor scan + design decisions (2026-09-25)

Scan run BEFORE implementing, per `.claude/skills/create-next-tool/SKILL.md` step 4.
All notes are paraphrased observations of publicly visible behaviour; no competitor copy,
branding, or trademark text is reproduced or reused anywhere in this block.

## Search

One WebSearch: *"online Python syntax checker check python code for syntax errors tool"*.
Top real tools reviewed (fetched and skimmed):

1. **ExtendsClass — Python tester** (`extendsclass.com/python-tester.html`)
2. **TechBeamers — Python Code Checker** (`techbeamers.com/python-code-checker/`)
3. **Softwium — Python validator** (`softwium.com/python-checker/`)
4. **InfoHeap — Python lint online** (`infoheap.com/python-lint-online/`)

A fifth candidate (`pynerds.com/python-syntax-checker/`) was unreachable from this
environment (DNS timeout) and was replaced by InfoHeap rather than running with fewer, as the
skill requires. Its search-result snippet is still informative and is quoted as a *claim* only:
it advertises detecting `SyntaxError`, `IndentationError` and `TabError` with line **and column**
hints — that matches the three error classes CPython itself raises at compile time, so it is
treated as a table stake below.

## Observed feature surface

| Capability | Seen in | Our decision |
|---|---|---|
| Paste code into a big editor/textarea | all 4 | **In model** — `code`, `multiline = true` textarea |
| Upload / drag-drop a `.py` file | ExtendsClass, Softwium | **Out of model** for the page form (our text-field pages take pasted text; file-input pages are a different descriptor shape). Noted, not built. |
| Explicit "Check syntax" button | all 4 | Platform: the generic page runs on input change; equivalent UX, nothing to build |
| Report the **line number** of the error | all 4 | **In model** — every diagnostic carries `line` |
| Report the **column** of the error | pynerds (claimed) | **In model** — `column` is derived from the parser byte offset (1-based, character-counted) |
| Distinguish `SyntaxError` / `IndentationError` / `TabError` | pynerds (claimed) | **In model** — the parser's messages are classified into these three CPython classes |
| Show the offending source line with a caret marker | CPython's own `py_compile` output; none of the 4 do it in-page | **In model** — `show_context`, on by default. This is our clearest differentiator: an exact `line`, `column`, source echo and `^` caret, like a real traceback. |
| Highlight/auto-jump to the error line in the editor | ExtendsClass, Softwium | **Out of model** — needs an editor widget with gutter decorations; the generic page renders a text output area. The caret context line above is the in-model substitute. |
| Input statistics (total lines, non-empty lines, characters) | TechBeamers ("8 lines (5 non-empty) 135 chars") | **In model** — `stats`, on by default |
| Preset example snippets (hello world / function / class / deliberate errors) | TechBeamers | **In model** — shipped as `[[example]]` chips on the page |
| Save / download the checked file, share a report link | TechBeamers | Platform: `format = "text"` pages already get a Download link, and every page is deep-linkable via `?param=` — nothing tool-specific to build |
| PEP 8 / style linting, naming conventions | TechBeamers | **Out of model** — a style linter is a different tool (this one is a *compile* check, mirroring `python -m py_compile`); listed, not built |
| Runtime errors / `NameError` / actually executing the code | TechBeamers (via Pyodide) | **Out of model** — executing user Python needs a full interpreter runtime; the same reason `notebook-runner` is skiplisted. Explicitly stated as a limit in the page copy. |
| Choice of Python version (2 vs 3, or 3.x minor) | none offered a selector; all state "Python 3" | **Out of model** as a *selector* (one grammar is compiled in). But the common failure it hides IS in model: Python-2-only constructs produce cryptic messages, so `python2_hints` (on by default) recognises `print "x"`, `except E, e:`, backticks, `<>`, `exec "..."`, `raise E, "msg"` and explains the Python 3 equivalent. |
| JSON output for CI / scripting | none of the 4 | **In model** — `format = json`, matching our own `js-linter` convention |
| Checking an expression or a REPL-style block rather than a whole module | none of the 4 | **In model** — `mode` = `module` / `expression` / `interactive`, mirroring CPython's `compile()` modes |
| A filename label in the report | py_compile prints one | **In model** — `filename`, default `<input>`, so CI output looks like a real compiler line |

## Engine decision (spike, not a guess)

`rustpython-parser = "0.4.0"` was spiked before committing to it (5-minute rule):

- native `cargo test --release` — parses valid modules, and produces CPython-shaped messages for
  the bad cases (`invalid syntax. Got unexpected token ':'`, `unexpected EOF while parsing`,
  `unexpected indent`, `EOL while scanning string literal`, `inconsistent use of tabs and spaces
  in indentation`) with a **byte offset** we convert to line/column ourselves;
- `cargo build --release --target wasm32-wasip1` — builds (chat/CLI block target);
- `cargo build --release --target wasm32-unknown-unknown` — builds (browser page target).

Instantiation under the wafer/wasmi runtime is the real gate and is verified by running
`gizza tool python-syntax-check …` after `scripts/build-block-wasm.sh`.

Rejected alternatives: `ruff_python_parser` is published only as an explicitly-internal Ruff
component crate (plus third-party vendored forks), so it is not a stable dependency; a
hand-rolled Python grammar would be strictly worse than a real parser at the one job this tool
has.

## Resulting descriptor

`code` (required) · `mode` (module|expression|interactive) · `format` (text|json) ·
`show_context` (bool, default on) · `python2_hints` (bool, default on) · `stats` (bool, default on) ·
`filename` (string, default `<input>`).

Every table stake above is either in that list or in the out-of-model column — none dropped
silently.
