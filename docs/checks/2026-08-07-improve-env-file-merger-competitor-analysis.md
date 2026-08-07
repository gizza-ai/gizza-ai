# env-file-merger — competitor analysis (2026-08-07)

Scan run BEFORE implementing, per `create-next-tool` step 4. All observations are
**paraphrased** from public documentation; no competitor copy, branding or trademarks are
reproduced or reused.

## Why this is not a duplicate of `blocks/dotenv-manager`

Checked first, because six `.env` rows have already been skiplisted as dotenv-manager subsets
(`env-json-converter`, `env-file-parser`, `env-template-generator`, `dotenv-normalizer`,
`env-file-sort`, plus `env-secret-encryptor` against `text-encrypt`).

`dotenv-manager` (`core/src/lib.rs`) takes exactly **two** documents — `env` plus one optional
`merge` overlay — and its only provenance signal is a per-entry `from_overlay: bool` rendered as a
single aggregate line, `Overridden by merge: N`. It cannot express a 3- or 4-file cascade, it never
says *which* file won a key, and it discards shadowed values entirely.

This row's two defining capabilities are exactly those gaps:

1. **N layered files** (the real-world cascade is four: `.env` → `.env.local` → `.env.<mode>` →
   `.env.<mode>.local`), not two.
2. **"which file set each value"** — per-key provenance plus the full override chain of the values
   that lost.

Neither is a render option on an existing output mode, so this is a distinct engine (ordered
multi-layer resolution with origin tracking), not the one-param-enhancement class the earlier
`.env` rows were skiplisted under. Adjacent blocks were also checked: `ini-env-diff` compares two
files pairwise (added/removed/changed), `dotenv-validator` lints one file, and
`env-schema-validate` validates one file against a declared schema — none resolve a cascade.

## Competitors reviewed

| # | Tool | What it is | Reached |
|---|------|-----------|---------|
| 1 | `dotenv-flow` (npm / kerimdzhanov) | The canonical Node cascade loader | README fetched |
| 2 | Vite env & mode | Framework-side cascade used by millions of projects | Docs fetched |
| 3 | `envcat` (pmac) | CLI that merges N env files to stdout | README fetched (thin) |
| 4 | `dotenv-extended` | Layered `.env.defaults` + `.env` + schema validation | npm page 403'd; behaviour taken from its documented option list surfaced in search |
| 5 | Browser/online mergers (FileProInfo ENV merger; `env-tools.vercel.app`) | Web UIs that merge uploaded `.env` files | Feature summaries only |

### 1. dotenv-flow

- Cascade, lowest → highest priority: `.env`, `.env.local`, `.env.<node_env>`,
  `.env.<node_env>.local`, then shell-defined variables. `.env.local` is deliberately skipped when
  the environment is `test`.
- Options with defaults: `node_env` (defaults to `process.env.NODE_ENV`), `default_node_env`
  (undefined), `path` (cwd), `pattern` (`.env[.node_env][.local]`), `encoding` (`utf8`),
  `purge_dotenv` (`false`), `silent` (`false`), `files` (undefined — an explicit ordered list that
  overrides the pattern).
- `listFiles()` returns the ordered list of files that actually exist — the closest thing any
  competitor has to provenance, but it is file-level, not key-level.
- No `${VAR}` expansion of its own; it defers to `dotenv-expand`.

### 2. Vite (env & mode)

- Same four-file order, same "mode-specific beats generic" rule, and pre-existing process
  variables outrank every file.
- Adds a **prefix filter**: only `VITE_`-prefixed keys are exposed to client code. Prefix filtering
  is therefore a genuine table stake for anyone resolving a cascade, not a nicety.

### 3. envcat

- Merges "any number of env files", tolerates missing files silently, and prints either an
  env-format document or a single line suitable for piping into a config-setting command.

### 4. dotenv-extended

- Two explicit layers by design — a defaults file plus the real file — with the real file winning,
  plus schema-driven validation flags for missing/extra keys.

### 5. Browser/online mergers

- The recurring pitch is privacy: merging happens locally so secrets are never uploaded. Feature
  set is thin — upload two or more files, get a merged file back; duplicate-key detection is
  advertised, key-level provenance is not.

## Table stakes → decision

| Capability | Seen in | Decision |
|---|---|---|
| Merge N files, last file wins | all five | **built** — 4 ordered layer inputs |
| Standard 4-file cascade naming | dotenv-flow, Vite | **built** — `layer_names` defaults to `.env,.env.local,.env.production,.env.production.local`; preset chips |
| Tolerate missing/empty layers | envcat, dotenv-flow | **built** — a blank layer is skipped, not an error |
| Per-key "which file set this" | nobody does it at key level (dotenv-flow is file-level) | **built** — this is the differentiator |
| Shadowed/overridden values | duplicate detection only | **built** — full override chain per key, plus a `conflicts` output |
| Prefix filter | Vite (`VITE_`), Next.js (`NEXT_PUBLIC_`) | **built** — `prefix_filter` |
| Env-format output | envcat, all online mergers | **built** — `output=env` |
| Single-line / shell-export output | envcat | **built** — `output=shell` (`export K='v'` lines) |
| JSON output | dotenv-extended ecosystem | **built** — `output=json` |
| `${VAR}` expansion | dotenv-expand, dotenvx, Vite | **built** — `expand_vars`, off by default (dotenv-flow's own default is no expansion), with `${VAR:-fallback}` and single-quote-is-literal semantics |
| Duplicate-key detection | online mergers, dotenv-manager | **built** — reported as warnings with line numbers |
| Secret masking | family invariant (`dotenv-manager`, `ini-env-diff`) | **built** — `mask_secrets`, default true, same marker list |
| Sort keys | dotenv-manager | **built** — `sort_keys` |
| Shell variables outrank files | dotenv-flow, Vite | **not a param** — a user pastes shell env as the last layer; documented on the page |

## Considered, not built (out of model)

- **Reading files from disk / auto-discovering `.env*` by pattern** (dotenv-flow `path`/`pattern`,
  envcat's file arguments). gizza blocks are browser-local and take pasted text; there is no
  filesystem surface.
- **Injecting the result into `process.env`** (`config()`, `purge_dotenv`, `unload`). No runtime to
  inject into.
- **Schema validation of the merged result** (dotenv-extended's `errorOnMissing`/`errorOnExtra`).
  Already covered by `blocks/env-schema-validate` and `blocks/dotenv-validator`; duplicating it
  here would rebuild those engines.
- **Encryption / secret-manager sync** (dotenvx). Covered for text by `blocks/text-encrypt`.
- **Uploading files** — the online mergers' upload step is the thing this tool's model avoids;
  paste stays local.
