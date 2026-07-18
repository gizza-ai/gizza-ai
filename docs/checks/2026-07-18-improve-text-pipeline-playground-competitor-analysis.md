# text-pipeline-playground — competitor analysis (2026-07-18)

Scan of interactive text-pipeline and command-style text wrangling tools. All notes are paraphrased; no competitor copy, branding, or trademarks copied.

## Competitors reviewed

| # | Tool | URL | Shape |
| - | ---- | --- | ----- |
| 1 | Ultimate Plumber | https://github.com/akavel/up | Terminal text pipeline workbench with live previews for shell pipelines. |
| 2 | CyberChef | https://gchq.github.io/CyberChef/ | Browser recipe builder with chained operations, previews, and many text/data transforms. |
| 3 | Online grep / regex filter tools | e.g. browser-local regex line filters | Paste text, enter a pattern, keep matching lines, often with case-insensitive options. |
| 4 | Online find/replace tools | browser text replacers | Regex/literal replacement, sometimes multiline, with a live output pane. |
| 5 | Sort/unique line utilities | browser line tools | Sort, dedupe, trim, reverse, head/tail-style list cleanup. |

## Table-stakes shipped

- **Chained operations with live preview** — one recipe transforms the previous step's output, matching pipeline mental models. ✅
- **Filter lines** — `grep PATTERN` and `reject PATTERN`, with literal mode by default and optional regex. ✅
- **Regex replace** — `replace /old/new/` with capture replacements and alternate delimiters. ✅
- **Mapping operations** — prefix/suffix, upper/lower, trim. ✅
- **List operations** — sort, reverse sort, unique, head, tail, reverse. ✅
- **Split/join** — break each line into tokens or rejoin lines with a separator. ✅
- **Case-insensitive matching** — shared option for grep/reject/replace. ✅
- **Error handling controls** — stop with an explanatory line-numbered error, or skip malformed operations while experimenting. ✅
- **Preset examples** — chips for common recipes (log errors, email extraction, CSV bullets, regex filter). ✅

## In-model design decisions

- Use a **safe DSL**, not arbitrary Python or shell execution. The backlog phrase mentions Python transforms, but executing user Python would require Pyodide/model/runtime support outside gizza's pure Rust/WebAssembly model and would be unsafe for a general text page.
- Make literal grep/reject the default so punctuation-heavy text works without escaping; Regex mode is opt-in.
- Keep the DSL compact and line-oriented so it is easy to paste, save, and share in query parameters.
- Include a line cap to prevent accidental huge outputs.

## Out-of-model / not built

- **Arbitrary Python, shell, jq, awk, or JavaScript execution** — needs a code runtime/sandbox beyond this repo's model and has security risk.
- **File-system pipelines and subprocess previews** — Ultimate Plumber-style terminal process orchestration is outside browser-local pure WASM.
- **Structured CSV/JSON semantic parsing** — this tool is plain text; dedicated tools handle quoting/schemas better.
