# stream-editor competitor analysis — 2026-08-13

## Scope

Tool: `stream-editor` — apply safe ed/sed-style command scripts to pasted text in the browser and CLI.

Model fit: pure text/Rust/WASM. Direct filesystem editing, shell execution, and GNU sed byte-for-byte completeness are out of model for the sandbox.

## Competitor scan

| Competitor | Table-stakes capabilities observed | UX/control patterns | Fit decision |
| --- | --- | --- | --- |
| LangStop Sed Command Builder / online sed tool | Find/replace, regex-oriented substitution, delete lines, insert text, examples/tutorial framing, command builder style guidance. | Text input, command/script field, generated or runnable command examples, preset-like common operations. | In model: substitution, delete, insert/append/change, examples, CLI example. Out of model: file mutation. |
| GNU sed live editor (`sed.js.org`) | Runs sed commands against sample text, exposes command-line/options idea, behaves like a live REPL for GNU sed. | Large text/script areas, live output, option toggles, quick iteration. | In model: live browser-local output, quiet mode (`-n`), basic vs extended regex. Out of model: complete GNU sed compatibility and external files. |
| regex101 | Deep regex testing/debugging, flavor selection, sample text, match highlighting and generated explanations/code. | Flavor selector, sample text, live results, detailed error feedback. | In model: regex flavor selector and clear regex errors. Out of model: match highlighting/debugger/explanations because this tool is a stream editor, not a regex tutor. |
| Quickref / sed cheat sheets | Worked sed examples for substitution, deleting lines, inserting/appending, ranges, printing matches. | Copyable examples, concise explanation of command idioms. | In model: page examples and FAQ documenting supported commands and limits. |

## Decisions implemented

- Input text and command script are both multiline fields.
- Boolean controls cover quiet mode (`sed -n`), global ignore-case behavior, and whole-buffer/multiline editing.
- Enum controls cover `regex_flavor` (`basic`, `extended`) and `line_ending` (`lf`, `crlf`) so page and CLI expose fixed accepted values.
- Output safety cap is exposed as `max_output_lines` with min/max metadata and a default of 100000.
- Preset examples cover replace/delete, quiet extraction, and extended regex groups.
- Core supports common stream-editing commands: addresses, ranges, substitution flags, delete, print, insert, append, change, transliteration, hold space, labels/branches, quit, line numbers, and list output.
- File and shell commands are rejected with explicit sandbox errors rather than omitted silently.

## Out-of-model / intentionally not built

- In-place file editing (`sed -i`), reading files, writing files, and shell execution.
- Full GNU sed compatibility, including every extension and byte-for-byte behavior.
- Regex debugger visualizations, match highlighting, generated regex explanations, and code generation.
- Multi-file batch processing.

## Verification focus

- Exact output for replacement/delete and quiet-mode extraction.
- Extended regex capture-group replacement.
- Explicit sandbox error for file/shell style commands.
- Deep-link query parameter coverage for booleans, enums, and max-output cap default.
