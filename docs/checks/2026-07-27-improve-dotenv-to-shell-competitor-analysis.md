# dotenv-to-shell — competitor analysis (2026-07-27)

Tool function: convert a `.env` file into `export`-prefixed shell statements (and
back), handling quoting and special characters safely.

## Competitors surveyed

1. **shdotenv** (`ko1nksm/shdotenv`, GitHub) — POSIX-shell dotenv loader/exporter.
   Output formats: `sh`, `csh`, `fish`, `json`, `jsonl`, `yaml`. Input dialects:
   posix/ruby/node/python/php/go/rust/docker. `export`/`eval` workflow
   (`eval "$(shdotenv)"`), `--grep PATTERN` filtering, `--overload`,
   `--ignore-environment`. Safe parsing (awk, never executes the file). Quoting:
   unquoted allows `#-./:@_`, single-quoted = literal (no embedded `'`),
   double-quoted supports `${VAR}` expansion and escapes `$` `` ` `` `"` `\`.
2. **php-xdg/dotenv-spec** (`exporting.md`) — the formal spec for exporting a
   POSIX-compatible dotenv file; defines exportable-name rules and the
   override-vs-keep algorithm. Confirms the target is *100% POSIX-shell-compatible*
   output.
3. **`set -a` / `source` idiom + `env`/`printenv`** (widely documented Bash/POSIX
   gists) — the ad-hoc baseline: `set -a; . ./.env; set +a`. Fragile with spaces,
   quotes, `#`, and newlines — exactly the special-character handling a dedicated
   converter must get right. `printenv`/`env` show the reverse (already-exported →
   listing).

(A general web search for hosted GUI converters returned no dedicated, reachable
"env → export" web tool of note; the space is dominated by the CLI tools above, so
the browser-local page is a genuine niche.)

## Table-stakes → decisions

| Capability | In model? | Decision |
|---|---|---|
| `.env` → `export KEY=value` (POSIX/bash) | yes | **built** — default `direction=to-shell`, `shell=posix`/`bash` |
| Reverse: shell exports → `.env` | yes | **built** — `direction=to-env` (the "and back") |
| Safe value quoting (spaces, `#`, `$`, `` ` ``, `\`, quotes, `=`) | yes | **built** — POSIX single-quote escaping (`'\''`); literal `$`/backtick preserved |
| Minimal vs. always-quote | yes | **built** — `quote=auto` (bareword when safe) / `single` (always) |
| `fish` dialect (`set -gx KEY value`) | yes | **built** — `shell=fish` with fish single-quote escaping (`\\`, `\'`) |
| Preserve comments / blank lines | yes | **built** — full-line `#` comments + blanks pass through (both syntaxes use `#`) |
| Strip inline `# comment` from unquoted `.env` values | yes | **built** |
| Flag / skip keys that aren't valid shell identifiers | yes | **built** — emits a `# skipped …` note, never emits invalid syntax |
| `csh`/`tcsh` (`setenv`) output | yes (but unsafe) | **considered, rejected** — csh single-quotes can't hold newlines and `!` still triggers history expansion, so byte-safe output can't be guaranteed; excluded rather than ship subtly-wrong output |
| Double-quoted output (`export KEY="value"`) | yes | **considered, rejected** — double quotes re-introduce `$`/backtick expansion, defeating the literal-safety goal single-quoting provides |
| Variable expansion / `${VAR}` interpolation | yes | **considered, rejected** — expansion is a footgun for a "safe literal export" tool; values are emitted verbatim |
| `--grep`/filter by name; `--overload`; multi-file merge | partial | **out of scope** — merge/validate/mask already covered by `blocks/dotenv-manager`; this tool stays focused on the shell⇄env transform |
| JSON/YAML output | yes | **out of scope** — `blocks/dotenv-manager` (`output=json`) and `blocks/json-yaml-convert` already cover structured export |

## Distinct from existing blocks

`blocks/dotenv-manager` normalizes/validates/masks and can emit `KEY=value`, JSON,
or `.env.example`, but its `normalized` output uses **dotenv double-quote escaping**
(`\n`, `\"`) — **not** shell-safe, and it has no `export` prefix, no shell dialects,
and no reverse (shell → `.env`). This tool owns the shell-safe `export`/`set -gx`
transform in both directions. Confirmed not a semantic duplicate.

Rule followed: no competitor copy, branding, or trademarks reproduced — features and
UX patterns analyzed for ideas only; all copy here and on the page is original.
