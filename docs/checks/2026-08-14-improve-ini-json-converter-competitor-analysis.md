# ini-json-converter — competitor analysis (2026-08-14)

Scan run BEFORE finalizing the UX, per `/create-next-tool` step 3. One web search
("online INI to JSON converter tool JSON to INI"), then the top reachable results were
skimmed. **Everything below is paraphrased from observed behaviour — no competitor copy,
branding or trademarks are reproduced or reused.**

## Competitors skimmed

Two of the top-ranked results were unreachable and were replaced with the next real tools
in the result list:

- codeshack.io INI-to-JSON — **HTTP 403 to the fetcher**, replaced.
- wtools.io INI-to-JSON — **expired TLS certificate**, replaced.

| # | Tool | Direction | Options exposed | Input/output affordances | Copy / help |
|---|------|-----------|-----------------|--------------------------|-------------|
| 1 | ConvertSimple (convertsimple.com) INI→JSON | One page per direction; a separate JSON→INI page exists | JSON indent width (number of spaces) + a "use tabs" toggle | Paste, file upload, download, copy-to-clipboard | 4-step how-to, a note that a conversion error is shown in the output box, an in-browser-processing claim, INI/JSON format reference tables, a large related-tools index |
| 2 | Site24x7 INI→JSON | One direction only | None — just Convert and Clear buttons | Paste in, read result out | One line on what an INI file is; no FAQ, no stated limits. The page was rate-limiting during the visit |
| 3 | jsontotable.org INI→JSON | One page per direction; a separate JSON→INI page exists | None user-facing; advertises validation + syntax highlighting | Paste, file upload (.ini/.conf/.cfg/.txt), load-sample button, copy, download .json, live re-convert as you type | FAQ covering file-size capacity, structure preservation, comment handling, API-readiness, and a free/no-limits claim; cross-links to INI formatter/validator/YAML converters |

Reference point outside the web-tool set: the widely used `crudini` command-line utility, whose
`--get` / `--set` / `--del` verbs are the established model for editing a single INI key in place
without reformatting the file. Our get/set/delete modes follow that verb shape (and the same
"create the section if it is missing" behaviour) because it is what users of this class of tool
already expect.

## Table stakes observed → decision

| Table stake | Where it landed |
|---|---|
| Paste INI, get JSON | `mode=ini_to_json` (and the default `auto`) |
| JSON back to INI | `mode=json_to_ini` — **in one tool**, not a second page (2/3 competitors split it across two URLs) |
| Live re-convert as you type | Platform: the page runtime recomputes on every `input`/`change` event |
| Pretty-print with a chosen indent width, incl. tabs | **Gap closed this run**: added `indent` (`Param::enumv` `2` / `4` / `tab`) alongside the existing `pretty` boolean. `serde_json::to_string_pretty` hardcodes 2 spaces, so `render_json` now drives `PrettyFormatter::with_indent` directly. Unit-tested for all three values plus the compact path and a rejected value |
| Copy to clipboard / download the result | Platform: `format = "text"` pages already ship a copy control and a Download link |
| Load-sample / example button | Six `[[example]]` preset chips (both directions, typed values at 4-space indent, get, set, delete) |
| Clear/reset | Platform: the shared widget chrome provides reset |
| Validation with a readable error | Core errors are line-numbered and say what was expected (`line 3: expected 'key = value', '[section]', or a comment: …`); a JSON paste into an edit mode gets a specific "run json_to_ini first" message |
| Explain what an INI file is / why convert | `page/content.md` intro + Modes section |
| FAQ | Six `<details>` accordions: comment loss on a JSON round trip, type detection, repeated keys, inline comments, delimiter choice, and how INI differs from `.properties`/TOML |
| Stated limits | "Limits and edge cases" section: 100k-line cap, 64-level nesting cap, array/scalar top-level rejection, CRLF preservation, auto-quoting rules |
| File upload (.ini/.conf/.cfg) | **Considered, not built.** The generator's `source = "file"` input is wired to the ffmpeg/media runtime, not to pure text tools; adding drag-and-drop text upload belongs in the shared generator as a declarative control (platform-over-per-tool-hack rule), not in this block. Paste + the CLI cover the case today |
| Syntax highlighting of the output | **Considered, rejected.** The shared text-output pane is plain `<pre>`; a per-tool highlighter would be a slug-specific hack in the shared runtime |

## Out-of-model (not built, by design)

- Server-side/API conversion endpoints and "API-ready" claims — gizza tools are browser-local
  wasm with no backend.
- Accounts, saved conversion history, paid tiers.
- Bulk/multi-file conversion — the page has a single input; the CLI covers scripted batches.

## Where this tool is ahead of the scanned set

- **Both directions plus in-place key editing in one tool.** No scanned competitor offers
  get/set/delete at all; they are pure converters.
- **Comment/layout preservation on edits.** Every scanned tool re-serialises, so a round trip
  through them loses comments and blank-line grouping. Our edit modes rewrite one line.
- **Repeated keys are preserved as arrays** (and written back out as repeated lines) rather than
  being last-write-wins.
- **Type detection is opt-in and explained**, with quoted values documented as the escape hatch.
- **Delimiter control** (`key = value` / `key=value` / `key: value`) for written lines, which none
  of the scanned tools expose.
- **Three surfaces** from one descriptor: chat block, `gizza tool ini-json-converter …` CLI, and
  the standalone page with deep-linkable query params.

## Neighbouring block check (not a duplicate)

`blocks/ini-parser` was grepped before building. It is INI→JSON **only** (`output` json/flat/report,
duplicate-key policy, comment-prefix choice) with no reverse direction and no editing. This tool's
reason to exist is JSON→INI plus the crudini-style get/set/delete edit modes; the overlap is the
one shared direction. Not skiplisted.
