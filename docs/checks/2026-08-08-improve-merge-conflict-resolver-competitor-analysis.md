# merge-conflict-resolver — competitor analysis (2026-08-08)

Scan run **before** implementation, per the create-next-tool recipe. All notes are
**paraphrased observations of behaviour**; no competitor copy, branding, or trademark is
reproduced here or in the shipped tool.

## Competitors reviewed

| # | Tool | Reachable | Shape |
|---|------|-----------|-------|
| 1 | wtool.dev — git conflict resolver | yes | Paste-and-parse, per-conflict cards, live merged preview |
| 2 | dev-toolbox.tech — git conflict resolver | yes | Paste-and-parse, per-conflict side-by-side cards |
| 3 | gitgroomer.com — conflict resolver assistant | yes | One conflict block at a time, two buttons + manual edit |
| 4 | gitgroomer.com — conflict marker sweeper | yes | Detection only: reports marker line numbers |
| 5 | github.com/BaseMax/git-conflict-resolver-web | yes | Minimal client-side "keep local / keep remote" resolver |
| — | toolgrid.io — git conflict resolver | **no** (HTTP 402) | Replaced by #4 above; behaviour summary from search index only, not used as a source of record |

## Table stakes observed

| Capability | Seen on | Defaults observed | In model here? | Where it lands |
|---|---|---|---|---|
| Paste a whole file containing `<<<<<<< / ======= / >>>>>>>` | 1,2,3,4,5 | single textarea, no size cap advertised | yes | `text` param, 1 MB cap |
| Accept **ours / current / HEAD** | 1,2,3,5 | most common first button | yes | `strategy = "ours"` (default) |
| Accept **theirs / incoming** | 1,2,3,5 | — | yes | `strategy = "theirs"` |
| Accept **both, ours first** | 1,2 | — | yes | `strategy = "both"` |
| Accept **both, theirs first** | 1 | — | yes | `strategy = "both-theirs-first"` |
| Bulk "apply to every conflict" | 1,2 | bulk buttons / keyboard shortcuts | yes | `strategy` is the bulk default |
| **Per-conflict** choice (a button per conflict card) | 1,2 | interactive, per card | yes (as data, not as cards) | `choices = "2=theirs, 4-5=both"` |
| Leave some conflicts unresolved, markers intact | 2 | unresolved cards keep markers | yes | `strategy`/`choices` value `keep` |
| Conflict counter / progress ("3 of 5 resolved") | 1,2,3 | shown above the cards | yes | `output = "list"` header + `json` counts |
| Numbered inventory with line numbers | 4 | report of marker line numbers | yes | `output = "list"` |
| Side-by-side comparison of the two sides | 1,2,3 | two panes per conflict | yes (text columns) | `output = "sides"` |
| Multiple conflicts in one paste | 1,2,4,5 | auto-detected | yes | all modes |
| Sample / example loader | 1,3 | one-click sample | yes | three `[[example]]` preset chips |
| Copy result to clipboard | 1,2,3 | button (+ shortcut on 1) | yes (platform) | generator's built-in Copy/Reset/Download |
| Browser-local, no upload, no account | 1,2,3,4,5 | stated as a selling point | yes | wasm, runs entirely client-side |
| **diff3 / zdiff3 base section** (`|||||||`) | **none** — #2 explicitly says it is unsupported | — | yes | parsed; `strategy = "base"` returns the common ancestor |
| Machine-readable output | none | — | yes | `output = "json"` |
| Guard: fail when markers survive / none found | 4 (detect only) | — | yes | `strict` boolean |

## Decisions

### In model — built in this pass

1. `text` — the pasted file content, 1 MB cap, multiline field.
2. `strategy` — `ours` (default) · `theirs` · `both` · `both-theirs-first` · `base` · `keep`.
   Covers every bulk button the competitors ship, plus the diff3 base side none of them expose.
3. `choices` — per-conflict overrides using the numbers from `output=list`
   (`2=theirs`, `3-5=both`, `4-=keep`, `all=theirs`). This is the data equivalent of the
   per-conflict buttons on tools 1–3, and it is what makes the tool usable from the CLI and
   from chat, where clicking a card is not possible.
4. `output` — `resolved` (default) · `list` (numbered inventory with line spans, branch labels
   and per-side line counts, plus the resolved/kept counter) · `sides` (aligned two-column
   ours-vs-theirs text view) · `json`.
5. `strict` — off by default; when on, errors if the paste holds no conflict markers at all,
   or if any conflict is still marked up in the result. Covers the sweeper use case (#4) as a
   hard gate rather than a report.

Extras that fall out of the parser and that no reviewed competitor handles:

- **diff3 / zdiff3 conflict style** — the `|||||||` common-ancestor section is parsed, reported,
  and selectable. Tool #2 documents this as unsupported; #1, #3 and #5 silently mis-parse it
  (the base text ends up glued to the "ours" side).
- **CRLF preservation** — the result is re-emitted with the line ending the input used.
- **Located errors** — nested `<<<<<<<`, unterminated blocks and stray `>>>>>>>`/`|||||||`
  markers are reported with the offending line number and what was expected, instead of
  producing silently wrong output.
- **`=======` outside a conflict stays text** — a Markdown setext underline is not treated as a
  conflict separator (a naive line-prefix scanner corrupts README files).

### Out of model — listed, not built

- **Interactive per-conflict cards with click-to-choose buttons and a live preview.** The
  generated tool page renders one input form and one output pane; per-conflict widgets would
  need bespoke page JS. The `choices` param carries the same capability declaratively.
- **Free-hand manual edit of one side inside the tool** (tools 2 and 3). A one-shot pure
  function cannot host an editor; `strategy=keep` plus the Copy button covers the workflow —
  keep the block, edit it in your own editor.
- **True 3-way merge editor** (GitKraken-class, also disclaimed by tool 1). Needs the three
  file versions and a merge algorithm, not a conflicted file; out of scope for a paste tool.
- **Keyboard shortcuts** (tool 1). A page-chrome concern owned by the shared generator, not a
  per-tool feature.
- **Writing the result back to a repository / running `git add`.** No filesystem or repo
  access in the browser sandbox; the tool returns text only.

### Considered, rejected

- A `separator` param to insert a delimiter line between the two sides in `both` mode. No
  reviewed competitor offers it, and it invites output that no longer compiles. Rejected as
  schema bloat.
- Tag-list UI for `choices`. The values contain `=` and ranges; a plain text field with a
  worked placeholder reads better than pills (same reasoning as other comma-bearing fields).
