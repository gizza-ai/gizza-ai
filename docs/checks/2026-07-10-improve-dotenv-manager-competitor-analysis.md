# dotenv-manager — competitor analysis (2026-07-10)

Scan run before implementing. All findings paraphrased; no competitor copy,
branding, or trademarks reproduced. Goal: identify the table-stakes params,
defaults, worked examples, and UX controls a `.env` parse/validate/merge/mask
tool must ship, and tag each in-model (pure-Rust, browser-local) vs out-of-model.

## Competitors skimmed

- **A browser-local .env linter/validator** — runs entirely client-side, no
  upload. Checks: duplicate keys, keys without values, naming-convention
  (UPPER_SNAKE_CASE) violations, unclosed quotes, formatting. Can auto-generate
  a `.env.example` from the input. Line-by-line feedback.
- **A general "env validator/formatter" web tool** — normalizes `KEY=VALUE`
  assignments, preserves comments/blank lines, optionally sorts keys, optionally
  aligns assignments, trims trailing whitespace, ensures a final newline, and
  warns on duplicate keys (flagging which value wins at runtime).
- **A Rust CLI linter for .env files** — ~13 checks incl. duplicated keys,
  incorrect delimiters, keys without values, unordered keys, trailing
  whitespace, lowercase keys, quote characters, leading characters, and a
  compare mode that diffs two `.env` files to surface keys missing between them.
- **A secret-masking .env editor** — a table view of a `.env` that masks values
  meeting a length/sensitivity threshold to reduce accidental exposure while
  editing.
- **A secrets-manager built on dotenv** — encrypts/syncs `.env` across
  environments (a hosted product, well beyond a local parse tool).

## Table-stakes → decision

| Capability | In-model? | Decision |
|---|---|---|
| Parse `KEY=VALUE`, comments (`#`), blank lines | in-model | core parser |
| `export KEY=` prefix handling | in-model | stripped in parser |
| Quoted values (single/double) + inline comments | in-model | core parser (double-quote `\n`/`\t` unescape) |
| Duplicate-key detection (last-wins note) | in-model | report + `normalized` last-wins |
| Missing **required** keys check | in-model | `required_keys` param |
| Merge / overlay two files (later overrides) | in-model | `merge` param, overlay wins |
| Secret masking of sensitive values | in-model | `mask_secrets` (default on), key-name heuristic |
| Naming-convention lint (UPPER_SNAKE_CASE) | in-model | warnings section |
| Keys without values lint | in-model | warnings section |
| Generate `.env.example` (blanked values) | in-model | `output=example` |
| Sort keys | in-model | `sort_keys` param |
| Normalized/cleaned output | in-model | `output=normalized` |
| JSON export of parsed pairs | in-model | `output=json` |
| Align `=` columns / cosmetic pretty-print | in-model but low value | **not built** (adds no correctness; noise) |
| Encrypt / sync / hosted vault | out-of-model | listed, not built (needs a server + accounts) |
| Diff *two saved files by path* on disk | out-of-model (browser-local, no FS) | approximated by `merge` overlay + missing/required |
| Auto-detect high-entropy secrets by content scan | out-of-model-ish | **not built** — key-name heuristic is deterministic and testable; entropy scan is fuzzy |

## UX controls (declarative, generator-current)

- `env`, `merge` → `multiline` textareas (paste whole files, newlines preserved).
- `required_keys` → text field with placeholder.
- `mask_secrets`, `sort_keys` → boolean checkboxes (mask defaults ON).
- `output` → `Param::enumv` → `<select>` with `[input.labels]` friendly names.
- `[[example]]` preset chips: "Validate + mask", "Merge overlay", "Make .env.example".

## Out-of-model (listed, not built)

- Encrypted secret sync / hosted vault across environments.
- Reading/diffing files from the local filesystem by path (the page is a single
  paste box; the `merge` overlay param covers two-file comparison in-model).
- Content-entropy secret detection (kept deterministic via a key-name heuristic).
