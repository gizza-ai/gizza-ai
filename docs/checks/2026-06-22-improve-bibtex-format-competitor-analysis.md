# bibtex-format — competitor analysis (2026-06-22)

Tool: `blocks/bibtex-format` — validate, sort, and pretty-print BibTeX bibliography entries.
Pure-Rust, in-browser (page + wasm) and chat/CLI. No network, no server.

## Competitors surveyed (top 5)

1. **bibtex-tidy** (flamingtempura.github.io/bibtex-tidy, the de-facto standard) — cleans
   spacing, sorts entries + fields, removes duplicate entries, strips/whitelists fields,
   enforces brace vs quote style, escapes characters, wraps long values, runs in-browser.
2. **BeeHive BibTeX Formatter** (beehive.tools) — re-formats with consistent indentation,
   brace placement, standardized capitalization of entry types + field names; local-only.
3. **Bibby BibTeX Validator** (trybibby.com) — syntax + completeness report: required
   fields per entry type, entry-type validity, **duplicate keys**, malformed values.
4. **bibclean** (classic CLI, Ubuntu manpage) — prettyprint + syntax check of BibTeX,
   normalizes braces/quotes, checks balanced delimiters.
5. **bib2x / bibtex.org** — converters (BibTeX → HTML/other); formatting is a side effect.

## Capability diff (theirs vs ours at first build)

| Capability | Competitors | Ours (initial) | Action |
|---|---|---|---|
| Pretty-print, 1 field/line, indent | tidy, BeeHive, bibclean | yes | — |
| Lowercase entry type + field names | BeeHive, tidy | yes | — |
| Sort entries (by key / type+key) | tidy | yes | — |
| Sort fields within entry | tidy, kitchin script | yes | — |
| Align `=` signs | tidy (curly column) | yes | — |
| Syntax validation w/ error location | bibclean, Bibby | yes (parse errors) | — |
| Preserve `{}`/`""`/`#`-macro values | bibclean | yes | — |
| @string / @preamble / @comment handling | tidy, bibclean | yes | — |
| **Duplicate cite-key detection** | Bibby, tidy | **MISSING** | **CLOSED** (added `check_duplicates`) |
| Remove duplicate *entries* | tidy | no | out-of-scope (formatter, not dedup; needs merge policy) |
| Strip/whitelist fields | tidy | no | out-of-scope (a "remove fields X,Y" editor, distinct tool) |
| Char escaping / value wrapping | tidy, bibclean | no | out-of-scope (LaTeX-escaping is its own concern; risks corrupting valid values) |
| Convert to HTML/other formats | bib2x | no | out-of-scope (this is a formatter, not a converter) |

## Gap closed this pass

**Duplicate cite-key detection** — the single highest-value validation gap shared by the two
validators (Bibby) and the cleaner (bibtex-tidy warns on it). Added a `check_duplicates`
parameter (default **true**): when on, the formatter errors if two `@type` entries share a
cite key (case-insensitive), naming the offending key — BibTeX itself silently keeps only the
first such entry, so a duplicate key is a real bug a bibliography author wants flagged. Set
`check_duplicates=false` to format anyway. This fits the pure-compute model perfectly (no new
deps, deterministic).

## Deliberately NOT built (out of model / out of scope, per skill rules)

- **Duplicate-entry merging** and **field whitelisting/stripping** are editing operations with
  policy choices (which copy wins? which fields to keep?), distinct from a deterministic
  formatter — they'd be separate tools, not copy of a competitor's feature set here.
- **LaTeX character escaping / value re-wrapping** risks corrupting already-correct values and
  needs a full LaTeX-aware model; out of scope for a faithful round-trip formatter.
- **Format conversion** (BibTeX→HTML/RIS) is a converter, a different tool.

No competitor copy, branding, or trademarks were used; the implementation is original.

## Verification (this pass)

- `cargo test --workspace` — core + drift-guard schema test pass (27 tests).
- `wafer build` — chat block.wasm builds + instantiates (validated OK).
- `wasm-pack build …/web` — page wasm built.
- generator — `tools/bibtex-format/` rendered.
- CLI — `gizza tool bibtex-format` verified: pretty-print, sort+align, duplicate-key error.
- Playwright — `tool-page-bibtex-format.spec.ts`, 5/5 pass (pretty-print, sort, align,
  lowercase-off, query-param deep-link).

Sources: [bibtex-tidy](https://flamingtempura.github.io/bibtex-tidy/),
[BeeHive BibTeX Formatter](https://www.beehive.tools/en/tools/formatters/bibtex-formatter),
[Bibby BibTeX Validator](https://trybibby.com/bibtex-validator),
[bibclean manpage](https://manpages.ubuntu.com/manpages/bionic/man1/bibclean.1.html),
[bibtex.org / bib2x](https://www.bibtex.org/).
