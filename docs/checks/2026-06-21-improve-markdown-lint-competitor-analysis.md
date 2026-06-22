# markdown-lint — competitor analysis (2026-06-21)

New tool built from the backlog: **markdown-lint** — "Finds and auto-fixes Markdown
style and consistency issues." Pure-Rust (`core` shared by chat block + web page),
runs on every backend (chat / CLI / browser page). Two modes: `check` (report) and
`fix` (corrected Markdown).

## Competitors surveyed

1. **DavidAnson/markdownlint** (`dlaa.me/markdownlint/` demo + VS Code extension) —
   the de-facto reference. 60+ rules (MD001–MD059), `--fix` for the subset that is
   safely auto-fixable, rich config (`.markdownlint.json`, enable/disable by
   rule/tag, per-file HTML-comment overrides, `extends`).
2. **markdownlint (Ruby, mdl)** — original Ruby linter, similar MD-rule set, config
   via style files.
3. **Markdown Utils — Markdown Linter** (markdownutils.com) — in-browser; checks alt
   text, skipped heading levels, empty links, line length (>120), duplicate headings;
   inline line highlights; pairs with a separate formatter for fixes.
4. **Aback Tools — Markdown Validator** — in-browser lint + fix assistant; heading
   structure, links, image alt text, code blocks, list formatting, table alignment,
   emphasis markers; line-aware diagnostics.
5. **MarkdownMe / ToolsBox linters** — in-browser, privacy-first (no upload),
   markdownlint-style rule reporting.

## Capability diff (us vs. the field)

| Rule / capability | markdownlint (ref) | competitors (web) | gizza markdown-lint |
|---|---|---|---|
| MD001 heading increment (no skipped levels) | yes | yes (Utils) | **yes (added)** |
| MD004 consistent UL markers | yes | partial | **yes + auto-fix** |
| MD009 trailing whitespace | yes (fix) | yes | **yes + auto-fix** (keeps 2-space hard break) |
| MD010 hard tabs | yes (fix) | — | **yes + auto-fix** (→ spaces) |
| MD012 multiple blank lines | yes (fix) | — | **yes + auto-fix** (collapse + trim EOF) |
| MD018 no space after `#` | yes (fix) | — | **yes + auto-fix** |
| MD019 multiple spaces after `#` | yes (fix) | — | **yes + auto-fix** |
| MD022 blank line around heading | yes (fix) | — | **yes + auto-fix (before)** |
| MD025 multiple top-level H1 | yes | yes (dup headings, Utils) | **yes (flag)** |
| MD026 trailing punctuation in heading | yes (fix) | — | **yes + auto-fix** |
| MD040 fenced code missing language | yes | yes (Aback) | **yes (flag, added)** |
| MD047 single trailing newline | yes (fix) | — | **yes + auto-fix** |
| Fenced-code awareness (prose rules skip code) | yes | partial | **yes** |
| Privacy / runs locally | n/a (lib) | yes | **yes (wasm, no upload)** |
| Three surfaces (chat LLM + CLI + page) | no | no (web only) | **yes** |

## Gaps closed this run

- **MD001 (heading-increment)** — added; competitors (Markdown Utils) advertise
  "skipped heading levels" as a headline check, so this was the clearest gap.
- **MD040 (fenced code language)** — added as a check-only flag; advertised by
  markdownlint and Aback Tools.

## Out-of-model / deliberately not built

- **Rich configuration** (per-rule enable/disable, config files, severity levels):
  gizza tools take flat scalar params, not a config file — out of the page/chat
  model. The fixed rule set with a `check`/`fix` switch is the right shape here.
- **Link/image-target validation** (MD042 empty links, missing alt text MD045):
  feasible in-model but lower-value for a style/consistency linter; deferred to keep
  the tool focused. Candidate for a future pass.
- **MD013 line-length**: intentionally omitted — it is noisy and almost universally
  disabled in real markdownlint configs; flagging it by default would bury the
  high-signal findings.
- **Table-alignment / emphasis-marker normalization** (Aback): low-value, high
  false-positive risk for a line-based linter; deferred.

No competitor copy, branding, or trademarks were used. The rule IDs (MDxxx) are the
shared community vocabulary from the open markdownlint rule set, used descriptively.

## Surfaces verified

- **Core unit tests**: 30 passing (every rule + every auto-fix + idempotence).
- **Chat schema drift guard**: passing (`schema_json_matches_authored_chat_schema`).
- **CLI**: `gizza tool markdown-lint` — both `check` and `fix` modes.
- **Page**: Playwright `tool-page-markdown-lint.spec.ts` (check report + fix output).
