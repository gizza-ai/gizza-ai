# stack-trace-analyzer — competitor analysis (2026-08-20)

Scan run **before** implementing, per `/create-next-tool` step 4. One web search
("online stack trace analyzer / parser / formatter"), then the top reachable real
tools were skimmed. **Everything below is paraphrased** — no competitor copy,
branding, or trademark text was reused, and no competitor markup or assets were
copied. Out-of-model items are listed, not built.

## Competitors reviewed

| # | Tool | Reached | What it is |
| --- | --- | --- | --- |
| 1 | DevToolbox — stack trace parser | yes | Multi-language parser: auto-detects the language, splits frames into function/file/line/column, collapses runtime + third-party frames. |
| 2 | devtoolsdaily — stack trace formatter | yes | Java-focused. Two views: a frames table and an exception tree that visualises the `Caused by` chain. File upload + example prefill. |
| 3 | StackTools — stack trace formatter | yes | Beautifier/highlighter with automatic language detection, per-frame click-to-copy, and two built-in example traces. |
| 4 | IO Tools — stack trace formatter | **no (HTTP 403)** | Not reachable; description from search listing only (colour-coded frame breakdown, separates app code from framework noise, Markdown output). Not counted as a skimmed profile. |

A 4th reachable profile was not pursued: the three above already converge on the
same feature set, and the search listing showed the remaining tools are the same
"paste → prettify" shape.

## Table stakes observed, and where each landed

| Table stake | Seen on | Decision | Where it landed |
| --- | --- | --- | --- |
| Auto-detect the language | 1, 2, 3 | **in-model** | `language = "auto"` (default) + 8 explicit choices |
| Multi-language support (JS/TS, Python, Java/Kotlin, Go, Ruby, C#/.NET, Rust, PHP) | 1 (7 langs), 3 ("and more") | **in-model** | 8 languages implemented in `core` |
| Split each frame into function / file / line / column | 1, 2, 3 | **in-model** | `Frame { function, file, line, column }`, all outputs |
| Pull out the error type + message | 1, 2 | **in-model** | Reported exception + type/message split |
| Exception chain / `Caused by` tree, root cause | 2 | **in-model** | Full chain across Java `Caused by`, Python `__cause__`/`__context__`, C# `--->`, PHP `Next`; explicit **root cause** line |
| Hide runtime / standard-library frames | 1 | **in-model** | `hide_framework` checkbox |
| Hide third-party (`node_modules`) frames | 1 | **in-model** | Same checkbox — the framework classifier already covers `node_modules`, `site-packages`, `/gems/`, `/vendor/`, `/pkg/mod/`, `/rustc/`, `java.*`/`System.*` etc. |
| Separate *your* code from framework noise | 1, 4 (listing) | **in-model** | `user`/`framework` kind on every frame + `*` marker + **first user frame** summary line |
| Reverse frame order | 1 | **in-model** | `reverse` checkbox |
| Frames **table** view | 2 | **in-model** | `output = "table"` (Markdown table) |
| Structured/machine-readable output | 1 (JSON-ish frame list) | **in-model** | `output = "json"` |
| Example traces to prefill | 2, 3 | **in-model** | Four `[[example]]` preset chips (Java, Python, JavaScript, Go) |
| Runs fully client-side, nothing uploaded | 1, 2, 3 | **already true** | wasm, no network — stated on the page |
| Copy the result | 1, 3 | **already true** | The generator gives every text tool Copy + Reset + Download |
| State the "minified names need source maps" limit | 1 | **in-model (copy)** | Stated in the Limits section and an FAQ |

## Beyond table stakes (ours, not copied)

- **Cross-language normalisation.** Every language is normalised to the same
  order — innermost (throw site) frame first, chain listed reported-exception
  first, root cause last — so a Python traceback and a Java trace read the same
  way. Competitors surface each language in its native order.
- **`user_packages`** — an explicit in-app prefix allow-list (the APM idea) so a
  trace with an unusual package layout still classifies correctly.
- **First user frame** promoted to a summary line: for most debugging sessions
  that single line is the answer.
- **`limit`** per-exception frame cap for very deep recursion traces.

## Considered, NOT built (out of model)

- **Source-map / ProGuard / R8 / dSYM de-obfuscation.** Needs a second file input
  (the `.map`/`mapping.txt`) — the tool page has one field-based input and this is
  a distinct tool, not a parameter. Called out on the page as a limit.
- **Click-a-frame-to-copy and per-frame clipboard shortcuts** (competitor 1, 3).
  The shared page runtime renders one text result with a Copy/Download control; a
  per-frame click target would mean per-tool JS in the shared runtime, which the
  workspace rule forbids. The `table`/`json` outputs cover the "grab one field"
  need.
- **Syntax-colour highlighting of frames.** Same reason — the generic result pane
  is plain text. Rejected in favour of the `*` user-code marker, which survives
  copy-paste into an issue or a chat message.
- **File upload of a `.log`/`.txt`.** The pure-tool page input is a textarea;
  paste covers it. (Media tools are the ones with file inputs.)
- **Deep links into a repository/IDE at `file:line`.** Needs project + host
  configuration this repo intentionally has no notion of.
- **Server-side symbolication / crash-report ingestion.** Backend feature, out of
  the browser-local, no-account model.
