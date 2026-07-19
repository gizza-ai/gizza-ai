# dotenv-validator — competitor analysis (2026-07-18)

Scan of the top online/CLI `.env` linters, to place `dotenv-validator` and decide
which table-stakes fit gizza's browser-local, pure-Rust/wasm, no-account model.
All descriptions are **paraphrased** — no competitor copy, branding, or
trademarks are reproduced.

## Competitor profiles

### 1. dotenv-linter (open-source Rust CLI) — the reference implementation
- **Features:** ~14 checks — duplicated key, key without value, incorrect
  delimiter, whitespace around `=`, quote-character / value-without-quotes,
  `${VAR}` substitution-syntax check, invalid leading char, lowercase key,
  unordered keys, trailing whitespace, extra/ending blank lines, and a
  schema-violation compare.
- **Params/options:** `check` / `fix` / `compare` commands; `--skip <CHECK>`
  (disable named rules), `--exclude`, `--recursive`, `--no-color`, `--quiet`;
  inline `# dotenv-linter:off/on` skips; `.dotenv-linter.yml` config. Default:
  all checks on, non-recursive.
- **Input/output:** local files/directories → `file:line RuleName: message`
  lines; `fix` rewrites files; `compare` diffs two files.
- **UX:** CLI only; pre-commit / CI / MegaLinter integration; no GUI.
- **Limits/pricing:** free, open-source. Note: its "missing key" is *file vs
  file*, not the *in-file `${VAR}` reference* check we do.

### 2. devFlokers Env Validator (online, client-side)
- **Features:** duplicate keys, invalid line format, empty values, required-vars
  present (via a pasted template), improperly-quoted values, malformed key
  names, inconsistent formatting, hardcoded-secret heuristic.
- **Params/options:** template field (paste `.env.example`), sort-keys toggle,
  remove-comments toggle, format-output toggle.
- **Input/output:** paste text (+ template); severity-colored errors/warnings
  with counts (lines, pairs, issues); a formatter that doubles as a fixer.
- **UX:** "Load Example" preset, two-pane input+template, formatting toggles.
- **Limits/pricing:** free, no signup, fully client-side.

### 3. EnvLint (online, client-side)
- **Features:** missing values, duplicate keys, invalid line format,
  exposed-secret heuristic (SECRET/TOKEN/KEY/PASSWORD names with real values),
  suspicious-placeholder detection (`changeme`, `xxx`, `your-key-here`).
- **Params/options:** minimal — lint / clear / upload; ⌘↵ to run.
- **Input/output:** paste, file upload, load-example; issues section + a parsed
  keys table (key/value/status) + an auto-generated masked `.env.example`.
- **UX:** load-example preset, keyboard shortcut, table grouping, template out.
- **Limits/pricing:** free, entirely client-side. (AquilaX's `.env` linter is a
  near-twin: paste + load-example, mask-values toggle, metrics, live preview.)

## Gap list & classification

### In-model, SHIPPED in dotenv-validator
- Duplicate-key detection (warning; reports the earlier line).
- Unquoted-value-with-spaces check (toggleable).
- `${VAR}`/`$VAR` interpolation syntax check — unclosed `${`, empty `${}`.
- **In-file undefined-reference check** — `${VAR}` pointing at a var never
  defined in the file, with an `allow_undefined` whitelist for shell/CI vars.
  This is our differentiator: the CLI reference tool only compares file-vs-file.
- Invalid/lowercase key names, whitespace around `=`, empty values, unterminated
  quotes, CRLF line endings, trailing whitespace.
- Report **and** structured JSON output (`ok`, `keys`, counts, `issues[]`) for
  CI — matches the counts/severity summary every online tool shows.
- Line numbers + error/warning severities + rule names on every issue.
- Load-Example presets: three `[[example]]` chips (messy file, undefined+external
  vars, JSON for CI) — the declarative equivalent of a "Load Example" button.
- Privacy messaging (runs in-browser, nothing uploaded) — matches our model.

### In-model, considered but NOT built (kept the linter focused)
- **Auto-fix / normalize / sort / masked `.env.example` generation / merge
  overlay.** These transform-and-rewrite features already live in the sibling
  **`dotenv-manager`** tool; duplicating them here would blur the two. This tool
  deliberately *diagnoses only, never rewrites*.
- **Secret / placeholder heuristics** (SECRET/TOKEN names with real values,
  `changeme`/`xxx`). Regex-only and in-model, but it's a security-hygiene axis
  distinct from structural linting and is already covered by `dotenv-manager`'s
  `mask_secrets`; left out to avoid overlap and false-positive noise.
- **Template / `.env.example` "required keys missing" compare.** Also owned by
  `dotenv-manager` (`required_keys`); our missing-variable axis is the in-file
  `${VAR}` reference check, which is the sharper, unique angle.
- **Per-check enable/disable for every rule** (dotenv-linter's `--skip`). We
  expose the two highest-value toggles (`check_interpolation`,
  `require_quotes_for_spaces`); a full per-rule matrix would bloat the schema for
  little gain in a paste-and-check tool.
- **Ordering / unordered-key check.** Low value for most `.env` files and often
  fights intentional grouping; declined.

### Out-of-model (needs server/infra — not built)
- Recursive directory / whole-repo filesystem scanning (a browser can't walk a
  repo; only multi-file upload approximates it).
- CI / pre-commit / MegaLinter integration and installable-binary distribution
  (the JSON output covers the *data* side of CI use).
- Live secret-validity verification (needs network calls).
- Cloud history / saved projects / accounts.

## Takeaway
`dotenv-validator` already covers the four headline checks (duplicate keys,
unquoted spaces, bad interpolation, missing-variable references) plus the common
syntax nits, with a unique in-file `${VAR}` reference check, an `allow_undefined`
whitelist, and CI-ready JSON. The remaining online-competitor table-stakes
(fix/normalize, secret-masking, `.env.example` generation, template compare) are
intentionally left to the sibling `dotenv-manager` so the two tools stay
distinct rather than redundant.
