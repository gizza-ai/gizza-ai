# strip-console-logs — competitor analysis (2026-08-12)

Scan done BEFORE implementing, per `/create-next-tool` step 4. All findings are paraphrased
descriptions of observable behaviour and documented options. No competitor copy, branding,
trademarks, or markup is reproduced here or in the tool.

## Tools reviewed

| # | Tool | Shape | How it was inspected |
|---|------|-------|----------------------|
| 1 | `consolelog.tools` "Console.log Remover" | Browser paste-in tool | Direct fetch returned **HTTP 403** to the fetcher; behaviour taken from the indexed search-result description of its own option list. Recorded as second-hand, not first-hand. |
| 2 | Babel plugin `transform-remove-console` | Build-step AST transform | Official docs fetched (options page). |
| 3 | Terser `compress.drop_console` / `pure_funcs` | Minifier compress pass | Docs + community write-ups fetched via search. |
| 4 | `remove-console-logs` (npm/GitHub CLI) | Directory-walking CLI | Repo README fetched. |

(4 rather than 3 because #1 could not be fetched first-hand; the three fetchable ones carry the
table-stakes list.)

## Table-stakes matrix

| Capability | Seen in | Verdict | Where it landed |
|---|---|---|---|
| Choose which `console.*` methods to strip | 1, 2 (`exclude`), 3 (`drop_console: ['log','info']`), 4 (`--target=all`) | **in-model** | `methods` param (comma list, tag-list control) + literal `all` |
| Keep specific methods even when stripping everything | 2 (`exclude`), 3 | **in-model** | `keep` param — the `exclude` equivalent |
| "Remove every `console.*`" one-shot switch | 1, 4 | **in-model** | `methods=all` |
| Replace removed statements with comments / preserve line structure | 1 ("preserve as comments", keeps line count) | **in-model** | `action = remove \| comment \| blank` |
| Dry-run / preview of what would change | 4 (`--no-save`) | **in-model** | `output = report` (line numbers + method tally, source untouched) |
| Per-file / per-method removal counts | 1 (stats), 4 (`--verbose` table) | **in-model** | the `report` output's tally |
| Do not touch `console.log` text inside strings, template literals, regexes or comments | 2, 3 (AST/token based) | **in-model** | char-scanner that tracks strings/templates/regex/comments |
| Multi-line and nested-argument calls | 2, 3 | **in-model** | balanced-paren span matching, string-aware |
| TypeScript sources as well as JavaScript | 1, 4 | **in-model** | same scanner; TS/JSX are supersets at the token level |
| `debugger;` statement removal | grunt-groundskeeper / ConsoleAway family | **in-model** | `remove_debugger` boolean (default off) |
| `window.console.log` / `globalThis.console.log` receivers | 3 (`pure_funcs: ['console.log']` is name-based) | **in-model** | optional `window.`/`globalThis.`/`self.`/`global.` prefix is matched and removed |
| Optional chaining (`console?.log(x)`, `console.error?.(x)`) | modern minifiers | **in-model** | matched by the scanner |
| Rewrite an expression-position call to `void 0` so surrounding code still runs | 2 (Babel does this) | **out-of-model** | needs real AST + scope analysis; we KEEP such calls untouched and list them in the report instead of risking a semantic change |
| Walk a directory / rewrite files in place | 4 | **out-of-model** | gizza tools are single-input; the CLI reads one source at a time |
| Strip calls through aliases (`const log = console.log; log(x)`) | — | **out-of-model** | requires binding resolution |
| Source-map-preserving output | 3 | **out-of-model** | no source-map pipeline in this block |
| Editor/IDE integration, project-wide command | VSCode extensions | **out-of-model** | not a web/CLI tool surface |
| Also strip other loggers (`alert`, custom `logger.debug`) | grunt-groundskeeper | **out-of-model** for v1 | deliberately scoped to `console`; noted on the page |

Every table-stake above is either implemented or listed as out-of-model — none dropped silently.

## UX patterns competitors ship, and our answer

- **Per-method checkboxes** (tool 1). Ours: a `tag-list` pill control on `methods` plus a `keep`
  pill list — same expressiveness, one control, and it deep-links as `?methods=log,warn`.
- **A single "remove all" toggle** (tools 1, 4). Ours: the literal value `all` in `methods`,
  reachable in one click from a preset chip.
- **Preset buttons.** Ours: `[[example]]` chips — "Strip debug logs", "Remove all but errors",
  "Comment out instead", "Preview report".
- **Copy-to-clipboard + reset.** Provided by the generator for every text-output page; not
  re-implemented.
- **Stats panel.** Ours: `output=report`, which also doubles as the dry-run.
- **Friendly `<select>` labels.** `[input.labels]` on `action` and `output`.

## Deliberate behavioural decisions

1. **Statement position is checked before removing.** A call is removed only where deleting it
   cannot change program structure: at a statement boundary (start of file, after `{`, `}`, `;`,
   or across a newline where ASI applies), or as the un-braced body of `if`/`for`/`while`/`else`/
   `do`, where it is replaced by an empty statement `;`. Calls used as values
   (`const a = console.log(x)`, `x && console.log(y)`, `() => console.log(z)`, chained
   `console.log(x).y`) are left alone and reported. Babel can safely replace those with `void 0`
   because it has an AST; a text-level tool cannot, so we prefer "unchanged" over "wrong".
2. **Restrictive method validation.** An unknown name (a typo like `warnn`) errors with the valid
   list rather than silently removing nothing — the failure mode competitors have.
3. **Input cap 500,000 characters**, stated on the page and in the error message.

## Sources

- [Console.log Remover — consolelog.tools](https://www.consolelog.tools/tools/console-log-remover) (403 to the fetcher; description via search index)
- [babel-plugin-transform-remove-console](https://babeljs.io/docs/babel-plugin-transform-remove-console)
- [Terser options](https://terser.org/docs/options/)
- [gsmart/remove-console-logs](https://github.com/gsmart/remove-console-logs)
