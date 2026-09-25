# yaml-lint — competitor analysis (2026-08-29)

Scan run **before** implementation, per `create-next-tool` step 4. All findings are paraphrased
observations of publicly visible behaviour; no competitor copy, branding or trademark is reused.

## Duplicate check (why this tool ships at all)

`ls blocks/ | grep -iE 'yaml|lint'` surfaces 16 neighbours. The three that could plausibly overlap:

| Existing block | What it does | Overlap verdict |
| --- | --- | --- |
| `blocks/config-file-validator` | Syntax-validates JSON/YAML/TOML/INI/XML with one parser each; YAML path is a `serde_yml` parse + hint. `strict` adds portability warnings (BOM, CRLF, tab indentation) and duplicate-key detection **for JSON and INI only** (`json_duplicate_keys`, INI section/key scan). | Partial — it answers "does this parse?" across five formats. It has **no YAML duplicate-key detection** (serde_yml silently keeps the last value) and no YAML style-rule engine. |
| `blocks/yaml-formatter` | Re-indents / normalises / sorts YAML and emits it back. | No — it rewrites, it does not report problems. |
| `blocks/yaml-query`, `yaml-path-query`, `json-yaml-convert*`, `yaml-deep-merge`, `yaml-to-csv`, `csv-to-yaml` | Query / convert / merge. | No. |

Precedent for a per-language linter living beside a generic validator already exists in the repo:
`markdown-lint`, `sql-linter`, `shell-script-linter`, `prose-linter`. **Verdict: build it** — a
`yamllint`-style *rule engine* (duplicate keys, indentation consistency, truthy/octal traps, comment
and colon spacing, severity levels, presets) is a different tool from a five-format syntax gate.

## Competitors reviewed

1. **yamllint (rule-set reference implementation)** — <https://yamllint.readthedocs.io/en/stable/rules.html>
2. **CTRLOps YAML Validator** — <https://ctrlops.io/tools/yaml-validator>
3. **DevEssentials YAML Linter** — <https://devessentials.dev/yaml-validator>
4. **yamltools.dev Syntax Checker** — <https://yamltools.dev/en/syntax-checker>

### 1. yamllint (the de-facto rule vocabulary)

23 named rules, each individually configurable: `anchors`, `braces`, `brackets`, `colons`, `commas`,
`comments`, `comments-indentation`, `document-end`, `document-start`, `empty-lines`, `empty-values`,
`float-values`, `hyphens`, `indentation`, `key-duplicates`, `key-ordering`, `line-length`,
`new-line-at-end-of-file`, `new-lines`, `octal-values`, `quoted-strings`, `trailing-spaces`,
`truthy`. Notable defaults: `line-length.max = 80`, `empty-lines.max = 2`, `colons` = 0 spaces
before / 1 after, `hyphens.max-spaces-after = 1`, `comments` = require a starting space and ≥2
spaces from content, `truthy.allowed-values = ['true','false']`, `indentation.spaces = consistent`.
Rules are switchable per config, and problems carry `line:column  level  message  (rule-id)`.

### 2. CTRLOps

Real-parser syntax stage (broken indentation, tabs, unbalanced flow collections, bad anchors/aliases,
duplicate keys) **plus** a lighter style stage rendered as amber suggestions rather than hard errors
(trailing whitespace, CRLF line endings, over-long lines, missing space after a colon). Controls: a
2-space/4-space indent choice, file upload, fullscreen, a Format button, copy/download. Preset
"try this" buttons for Kubernetes, Docker Compose, GitHub Actions and Ansible documents. Output has
a valid/invalid verdict, line+column, the offending line highlighted, and a document count for
`---` streams. Also does schema-shaped required-field checks per detected document type.

### 3. DevEssentials

Minimal single-textarea linter over js-yaml: exact line **and** column for every syntax error plus a
context snippet. Its copy leans on a catalogue of the six most common raw parser messages
(`found character '\t' that cannot start any token`, `bad indentation of a sequence entry`,
`mapping values are not allowed here`, `duplicated mapping key`, `unexpected end of the stream`, …)
and a seven-entry FAQ explaining *why* each happens (indentation semantics, truthy literals,
multi-document files, syntax vs semantic errors).

### 4. yamltools.dev

Indentation (spaces vs tabs), duplicate keys, type-coercion traps (`yes`/`no` becoming booleans),
quoting of special characters, multi-line string problems, anchor/alias consistency. Reports line and
column with a fix suggestion. Ships two loadable sample documents — one valid, one deliberately
broken — an editor with highlighting, file upload, multi-document support, and a paid CI/CD API.

## Table stakes → decisions

| # | Table stake (who) | Fit | Where it lands |
| --- | --- | --- | --- |
| 1 | Real-parser syntax errors with exact line **and** column (all four) | in-model | `syntax` rule via `yaml-rust2`'s marked event parser (`ScanError` marker) |
| 2 | Duplicate mapping keys, reporting the first definition too (all four) | in-model | `key-duplicates` rule — event-level, per-mapping frame; merge keys `<<` only in `strict` |
| 3 | Tabs in indentation (all four) | in-model | `indentation` rule, error severity |
| 4 | Inconsistent / non-multiple indent width, misaligned sequence items (CTRLOps, DevEssentials, yamltools) | in-model | `indentation` rule, warning severity; `indent_spaces` param (CTRLOps' 2-vs-4 choice generalised to 1–8) |
| 5 | Configurable max line length (yamllint 80, CTRLOps "over-long lines") | in-model | `line-length` rule + `max_line_length` param, `0` disables |
| 6 | Trailing whitespace (yamllint, CTRLOps) | in-model | `trailing-spaces` rule |
| 7 | Missing newline at end of file (yamllint) | in-model | `new-line-at-end-of-file` rule |
| 8 | Too many consecutive blank lines (yamllint `empty-lines`) | in-model | `empty-lines` rule, max 2 |
| 9 | Comment formatting — starting space, ≥2 spaces from content (yamllint) | in-model | `comments` rule, quote-aware `#` scanner |
| 10 | Colon spacing incl. the classic missing space after `:` (yamllint, CTRLOps) | in-model | `colons` rule (URL/`12:30` false positives excluded by anchoring on the first colon of a plain key) |
| 11 | Hyphen spacing in sequences (yamllint) | in-model | `hyphens` rule |
| 12 | Truthy traps — `yes`/`no`/`on`/`off`/`True` (yamllint, yamltools) | in-model | `truthy` rule, event-level so only unquoted plain scalars fire |
| 13 | Octal-looking values (`0755`) (yamllint) | in-model | `octal-values` rule, event-level |
| 14 | Multi-document `---` streams + document count (CTRLOps, yamltools) | in-model | parser is stream-aware; count reported in both output formats |
| 15 | Severity split: hard errors vs amber suggestions (CTRLOps, yamllint levels) | in-model | every problem carries `error`/`warning`; `strict_warnings` promotes warnings to errors (yamllint `--strict`) |
| 16 | Rule presets / configurability (yamllint config, CTRLOps' lighter style stage) | in-model | `preset` = `relaxed` \| `default` \| `strict`, plus a `disable` list of rule ids |
| 17 | `---` document start required, alphabetical key ordering, empty values (yamllint `document-start`, `key-ordering`, `empty-values`) | in-model | shipped, `strict` preset only (off by default — all three are noisy on ordinary files) |
| 18 | Machine-readable output for CI (yamltools' API angle) | in-model | `report_format = json` → `{valid, documents, errors, warnings, problems[]}` |
| 19 | Preset example documents to load in one click (CTRLOps, yamltools) | in-model | `[[example]]` chips: Kubernetes manifest, Docker Compose, GitHub Actions workflow, broken-YAML demo |
| 20 | Everything runs client-side, nothing uploaded (all four) | in-model | wasm page + CLI; stated in the page copy |

## Deliberately NOT built (out of model / out of scope) — listed, not dropped

- **Schema / document-type validation** (CTRLOps' "Valid Kubernetes Manifest", required-field checks
  for Docker Compose / Ansible / GitHub Actions). Needs a bundled, versioned schema corpus; the repo
  already has `json-schema-validate`-family blocks for schema work. Out of scope for a syntax+style
  linter.
- **Auto-fix / reformat output** (CTRLOps' Format button). Already shipped as
  `blocks/yaml-formatter` (indent width, key sorting, block/flow style) — duplicating it here would
  be the redundant tool this analysis exists to avoid. The page copy points at that separation.
- **File upload + syntax-highlighted editor** (CTRLOps, yamltools). The generic tool page gives a
  paste-able textarea and deep-linkable `?yaml=` params; a code editor is a page-platform feature,
  not a block capability.
- **Saved validations / CI-CD API / accounts** (yamltools). Service features; the CLI (`gizza tool
  yaml-lint …`) with `report_format=json` is the CI story here.
- **yamllint rules left unimplemented on purpose:** `quoted-strings` (its default "quote everything"
  is wrong for most real files), `braces`/`brackets`/`commas` flow spacing (low value next to the
  syntax stage), `float-values`, `new-lines` (CRLF is already covered by `config-file-validator`'s
  portability warnings), `document-end`, `comments-indentation`, `anchors` (undeclared aliases are
  already a hard parse error here). Each is a config knob, not a capability gap.
- **Multi-line quoted-string interiors** are skipped by the line-based rules (block scalars are
  detected and skipped; a value split across lines inside quotes is not). Stated as a limit on the
  page rather than silently mis-flagged.

## UX patterns adopted

- `[[example]]` preset chips mirror the competitor "try Kubernetes / Docker Compose / GitHub Actions"
  buttons — the declarative equivalent in this generator.
- `kind = "slider"` for `indent_spaces` (1–8) and `max_line_length` (0–200), matching the 2-vs-4
  indent picker but generalised.
- `kind = "tag-list"` for `disable`, so rule ids are entered as removable pills.
- `[input.labels]` gives the `preset` and `report_format` selects plain-English labels.
- Output leads with a verdict line (`✓ valid` / `✗ N problems`) and a document count, then
  `line:col  level  message  [rule]` rows — the shape every reviewed tool converged on.
