# vscode-snippets-generator — competitor analysis (2026-09-24)

Research pass run **before** implementation, so the first shipped version already carries the
table stakes. All findings are **paraphrased** — no competitor copy, branding, or trademarks were
reused. Screenshots were not captured (every surface is a plain two-pane form; markdown captured
everything that mattered).

## Reference: the VS Code snippet format itself

Source: the official VS Code user-defined-snippets documentation.

| Aspect | What the format requires |
| ------ | ------------------------ |
| Entry shape | A snippets file is one JSON object; each key is the snippet *name* and its value is an object. |
| `prefix` | One trigger word (string) **or** several (array of strings). IntelliSense substring-matches it. |
| `body` | A string, or an array of strings that VS Code joins with newlines. Array-of-lines is the idiomatic form. |
| `description` | Optional text shown in the IntelliSense detail pane. |
| `scope` | Comma-separated language identifiers. Only meaningful in a global `.code-snippets` file; a language-named file (`rust.json`) is already scoped by its filename. |
| `isFileTemplate` | Optional boolean; opts the snippet into the "fill file with snippet" command. |
| Tabstops | `$1`, `$2`, … with `$0` as the final cursor position. |
| Placeholders | `${1:label}`, nestable. |
| Choices | `${1\|a,b,c\|}` — a pick-list; inside a choice, `\` additionally escapes `,` and `\|`. |
| Variables | `$TM_FILENAME`, `$TM_SELECTED_TEXT`, `$CLIPBOARD`, `$CURRENT_YEAR`, `$RANDOM`, `$UUID`, … plus `${VAR:fallback}`. |
| Transforms | `${TM_FILENAME/(.*)\..+$/$1/}` on variables, and the same regex/format shape on placeholders. |
| Escaping | Inside the snippet *string*, `$`, `}` and `\` are escaped with a backslash. On top of that the whole thing lives in JSON, so `"`, `\` and control characters need JSON escaping too — a **double** escaping layer that hand-written snippets routinely get wrong. |

## Competitor profiles (top 5)

### 1. snipgen.sftwr.dev — "VSCode Snippet Generator"
- **features:** two-pane live form → JSON; title, prefix (accepts several, comma-separated), description, language scope picked from a long dropdown, and the code body.
- **params/options:** a "cursor index" auto-increment helper that bumps the tabstop number as you insert; buttons that insert `$1`, `${1:placeholder}` and `$0` into the body at the caret.
- **output:** a complete JSON snippet object rendered in the right-hand pane.
- **ux:** copy-to-clipboard, clear/reset, and a short "Command Palette → Configure User Snippets" hand-off note.
- **limits/pricing:** free, no account.

### 2. lewishowles/tool-vs-code-snippet-generator (archived June 2026)
- **features:** deliberately minimal — snippet title, prefix, code body, nothing else.
- **conversion behaviour (the interesting part):** splits the pasted block on newlines into `body` array elements, turns leading tabs into escaped `\t` sequences, and escapes quotes; emits `prefix` as an array even for a single trigger.
- **output:** the snippet entry as JSON, ready to paste.
- **ux:** paste → read → copy. No scope field, no description field.

### 3. snippetgen.com — "SnippetGen"
- **features:** a VS Code *extension* rather than a web page. Generates snippets from existing files, and (paid tiers) from whole folders in one pass.
- **params/options:** configurable snippet templates that can carry placeholders, tabstops and choice elements.
- **output:** exports as plain JSON, as a packaged VS Code extension, or as a shareable bundle.
- **ux:** command-palette integration, a CLI, and a watch mode that re-generates as the source changes.
- **free_vs_paid:** core tier free; batch folder processing, team sharing and API access are paid.

### 4. julienverneaut.com — "Visual Studio Code snippet generator"
- **features:** paste code, configure, receive a snippet definition that matches the official spec.
- **params/options:** a scope/language setting to limit where the snippet fires; prefix as the trigger.
- **output:** a fragment intended to be pasted **inside** a JSON object in a `.code-snippets` file under `.vscode/` — i.e. an *entry*, not a whole file.
- **ux:** minimal form, emphasis on "you don't have to know the snippet grammar".

### 5. willwull.github.io/vscode-snippet-generator
- **features:** a small single-page generator in the same family (form on the left, JSON on the right). The page is fully client-rendered and served no text to a plain fetch, so only its shape could be confirmed, not its option set. Recorded as *present but not deeply profiled* rather than silently dropped.

(A sixth hit, `snippetgenerator.online`, now redirects to an expired-domain parking page and was replaced by the VS Code documentation as the format reference above.)

## Gap list → decisions

| # | Gap (≥1 competitor ships it) | Dimension | Verdict |
| - | ---------------------------- | --------- | ------- |
| 1 | Name / prefix / description / scope / body as separate fields | capabilities | **in-model — built** (`name`, `prefix`, `description`, `scope`, `template`) |
| 2 | Multiple comma-separated triggers | capabilities | **in-model — built**: one trigger emits a JSON string, several emit an array (what VS Code accepts for both) |
| 3 | `body` as an array of lines | capabilities | **in-model — built**, always; it is the idiomatic form and survives round-tripping |
| 4 | Tab/indent normalisation (competitor 2's `\t` conversion) | capabilities | **in-model — built** as an explicit `indent` choice: `keep` / `tabs` / `spaces`, with `tab_size` |
| 5 | Correct JSON escaping of quotes, backslashes, tabs, control chars | capabilities | **in-model — built**; this is the whole point of the tool |
| 6 | Snippet-syntax escaping of stray `$` **without breaking** `$1` / `${1:x}` / `${1\|a,b\|}` / `$TM_FILENAME` | capabilities | **in-model — built** as `dollars = auto` (default): a real parser for tabstops, placeholders, choices, variables and transforms; only `$` that is *not* a valid construct gets `\$`. `literal` escapes every `$` (and `\`), `raw` passes the text through untouched. No competitor profiled does this correctly. |
| 7 | Whole-snippets-object vs paste-inside-a-file entry (competitors split on this — 1 and 2 do the object, 4 does the entry) | capabilities | **in-model — built** as `output = snippets-file \| entry` |
| 8 | `isFileTemplate` | capabilities | **in-model — built** (boolean; omitted when false, as VS Code omits it) |
| 9 | Final-tabstop helper buttons (`$0`) | capabilities/UX | **in-model — built** as `final_tabstop`, a boolean that appends `$0` only when the template doesn't already contain one — the durable version of a caret-insert button, which doesn't translate to a stateless CLI/chat surface |
| 10 | JSON indentation | capabilities | **in-model — built** (`json_indent`, 0–8; 0 emits compact one-line JSON) |
| 11 | Long language dropdown for `scope` | UX | **in-model — built** as a free-text field with a `languages` autocomplete vocabulary in the shared generator, so it also accepts language ids the list doesn't know |
| 12 | One-click presets / examples | UX | **in-model — built** as `[[example]]` chips (React component, HTML boilerplate, choice-driven snippet) |
| 13 | Copy to clipboard, reset | UX | **already platform** — every generated page ships Copy + Reset + a Download link for `format = "text"` |
| 14 | Caret-aware "insert `$1` here" buttons | UX | **considered, rejected**: needs bespoke per-tool JS bound to a textarea selection, and the same capability has to exist on the CLI and chat surfaces where there is no caret. `final_tabstop` + documented syntax covers the actual need. |
| 15 | Generate snippets from a whole file / folder in one pass | capabilities | **out-of-model** — needs filesystem access we don't have in the browser sandbox; one snippet per run. |
| 16 | Export as a packaged VS Code extension | capabilities | **out-of-model** — needs a `package.json` + directory tree, i.e. a multi-file archive build and a publisher identity. |
| 17 | Watch mode / continuous regeneration | capabilities | **out-of-model** — needs a resident process watching a workspace. |
| 18 | Team sharing, accounts, API keys, paid tiers | n/a | **out-of-model** — gizza is browser-local, no account, no server. |
| 19 | Live preview of what the snippet *expands to* | UX | **considered, rejected** — faithfully simulating VS Code's expansion (variables, transforms, choice UI) is a second tool's worth of work and would mislead where it diverged. The page instead documents the syntax it preserves. |

## Copy / SEO angles observed (paraphrased)

Competitors rank on: "vscode snippet generator", "create vs code snippets", "code to snippet json",
"user snippets file", and how-to phrasing around the Command Palette → Configure User Snippets flow.
They all explain *where* the output goes (`~/.config/Code/User/snippets/<lang>.json`,
`.vscode/*.code-snippets`) — our page does the same in original wording, and additionally documents
the escaping rules, which none of them spell out.

> Original work only — no competitor copy, branding, or trademarks were reproduced.
