# markdown-list-marker-normalizer — competitor analysis (2026-08-21)

Scan run before finishing the tool. Notes are paraphrased from public documentation and
tool pages; no competitor copy, branding, wording, or assets were reused.

## Competitors reviewed

| # | Competitor | Shape | Relevant table stakes |
|---|---|---|---|
| 1 | markdownlint / markdownlint-cli2 (David Anson) | Linter + autofix, config-driven | The de-facto rule vocabulary for this problem: MD004 `ul-style` (`consistent`/`asterisk`/`dash`/`plus`/`sublist`, default `consistent`), MD005 inconsistent indentation (no options), MD007 `ul-indent` (`indent` default 2, plus `start_indent`/`start_indented`), MD029 `ol-prefix` (`one`/`one_or_ordered`/`ordered`/`zero`, default `one_or_ordered`), MD030 `list-marker-space` (`ul_single`/`ul_multi`/`ol_single`/`ol_multi`, all default 1), MD032 lists surrounded by blank lines. |
| 2 | remark / remark-stringify (unified) | Programmatic AST serializer | Serialization knobs: `bullet` (`*`/`+`/`-`, default `*`), `bulletOther` (fallback marker, must differ from `bullet`), `bulletOrdered` (`.`/`)`, default `.`), `listItemIndent` (`one`/`tab`/`mixed`, default `one`), `incrementListMarker` (boolean, default true), `rule` for thematic breaks. Round-trips through an AST, so anything it cannot represent is rewritten. |
| 3 | Prettier (markdown parser) | Opinionated zero-config formatter | Emits one bullet style, renumbers ordered lists, and alternates the marker between adjacent sibling lists so they stay separate blocks. Exposes only `printWidth`, `proseWrap`, `tabWidth`, `endOfLine` — no list-specific options at all; the list behaviour is fixed. |
| 4 | markdownutils.com Markdown Formatter | Online paste-box formatter | Zero-config "clean everything" pass: heading spacing, list indentation standardised at 2 spaces, blank lines between blocks, trailing whitespace, inconsistent bullets. Browser-local with no upload. UX: before/after diff view, copy button, hand-off to a PDF/editor step. FAQ covers what gets fixed, whether text is modified, whether changes are shown, and how it compares to running Prettier. |
| 5 | freemarkdowntools.com Markdown List Style Unifier | Online single-purpose list tool | Closest direct match in scope: normalize every bullet marker to `-`, `*`, or `+` (user's choice) and set ordered numbering to auto-increment or all-`1.`. Sits in a family alongside a markdown indent/outdent tool and a task-list manager. |

## Table stakes shipped

- `marker` uses exactly the MD004 vocabulary — `dash`, `asterisk`, `plus`, `consistent`,
  `sublist` — so a file normalized here satisfies whatever `ul-style` a repo has configured.
  `sublist` alternates `-` → `*` → `+` by depth and repeats.
- `indent` is MD007's `indent`, 1–8 spaces per nesting level, default 2 (the markdownlint,
  Prettier, and CommonMark default). Ragged 1/3/5-space nesting is snapped onto the ladder
  instead of only being reported.
- Tabs are expanded at 4 columns (CommonMark) and written back as spaces, so a tab-indented
  file lands on the same ladder as a space-indented one — competitor #4's implicit behaviour,
  made explicit.
- `ordered` covers MD029's actionable styles: `ordered` (sequential from each list's own first
  number), `one`, `zero`, plus `keep` for leaving numbering untouched.
- `marker_space` is MD030's single-line case, 1–4 spaces, default 1.
- `normalize_indent` off is the markers-and-numbering-only pass — the thing competitor #5 does
  and competitor #4 cannot be told to skip.
- Non-destructive by construction, and stated on the page rather than implied: item text,
  headings, tables, blockquotes, thematic breaks (`---`, `***`), line-start emphasis, task-list
  checkboxes, ordered `.`/`)` delimiters, CRLF line endings, and every line inside a fenced code
  block are preserved byte-for-byte. Wrapped continuation lines move with their item.
- Browser-local, no upload, no account — matching #4/#5 and stated in the page copy.
- Deep-linkable `?param=` presets and one-click example chips for the common styles
  (Prettier-style dashes, 4-space asterisks, sublist alternation, renumbering, lazy `1.`,
  markers-only).

## Considered, not built

- **MD029's `one_or_ordered` style.** It is a lint *acceptance* policy ("either is fine"), not
  an output style — there is no single rewrite it maps to. `keep` already covers "leave a file
  that is already acceptable alone".
- **MD007 `start_indent` / `start_indented`.** Indenting the entire top level is a niche
  house-style used mostly to nest lists inside other content; adding two more params to serve it
  would bloat the schema for every other user, and `normalize_indent = false` already protects a
  file that is deliberately indented.
- **Separate `ul_single`/`ul_multi`/`ol_single`/`ol_multi` marker spacing (MD030).** All four
  default to 1 in markdownlint and multi-line variants only matter for loose lists; one
  `marker_space` knob covers the real-world case without four near-identical fields.
- **remark's `bulletOrdered` (rewriting `1.` ↔ `1)`).** Deliberately rejected: the `.`/`)`
  delimiter is preserved instead. Some renderers treat a change of delimiter as the start of a
  *new* list, so silently flipping it can split one list into two.
- **Prettier's alternate-marker-between-adjacent-lists rule.** It only fires on two sibling
  lists separated by a comment or blank line and depends on block-level AST context this
  line-based engine does not build; getting it half-right would merge or split lists.
- **Everything-else formatting (headings, trailing whitespace, blank lines, EOF newline, table
  alignment)** as in competitors #3 and #4. Out of scope on purpose — this tool touches list
  scaffolding only, and the repo already ships a general markdown linter/fixer for the rest.
- **Before/after diff view** (competitor #4). Out of model for the generic tool page, which
  renders a single text output; a dedicated diff tool already exists in the toolkit.
- **File upload / whole-repo or multi-file runs, config-file (`.markdownlint.json`) import,
  editor and CI integrations, accounts.** All need a backend, a filesystem, or an editor host —
  outside the browser-local, no-account model.

## Verification snapshot

Built and verified on 2026-08-21: `cargo test --workspace` in the block (24 core + 3 descriptor
tests incl. the regenerated drift-guard), canonical `scripts/build-block-wasm.sh`, `wasm-pack`
web build, `cargo install --path cli`, `scripts/sync-tool-manifest.py`, targeted generator page render
for this block (the full-repo generator rendered this page before the orchestrator's 600s command cap),
`gizza tool` exact-output checks including the generated page CLI example verbatim, the
Playwright page spec (`tests/tool-page-markdown-list-marker-normalizer.spec.ts`, covering every
`marker` and `ordered` value, a non-default checkbox state, both sliders, the `?param=`
deep-link, and the exact 500000-character cap boundary), and
`scripts/check-tool-hygiene.py markdown-list-marker-normalizer`.
