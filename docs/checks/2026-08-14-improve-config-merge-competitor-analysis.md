# config-merge — competitor analysis (2026-08-14)

Scan run **before** the UX/descriptor was finalized, per `/improve-tool` Phase 2–3.
Everything below is **paraphrased** from public documentation and landing copy — no
competitor wording, branding, or trademarks were copied into the tool, its page, or its
tests.

## Search

One web search: *"online tool merge layered config files JSON YAML TOML env override
precedence variable substitution"*. The result set splits into two clusters:

1. **Developer CLI/library tools** that do the real job (cross-format layering + `${VAR}`
   substitution) but require an install/Docker/Node runtime.
2. **Single-format browser mergers** (YAML-only, TOML-only) that are easy to reach but
   shallow — mostly one format in, one format out, often shallow-merge only.

The gap between those two clusters is exactly where this tool sits: the CLI feature set,
running locally in a browser page with no install.

## Competitor 1 — Docker-packaged cross-format config merger (`config-merge`, boxboat)

Closest functional match; the row's own name comes from this shape of tool.

- **Inputs:** `.json`, `.js`, `.json5`, `.toml`, `.yaml`/`.yml`, plus `.env`/`.sh` files that
  are *sourced into the environment* rather than merged as data. Also a "patch" file class
  (`.patch.json`, `.patch.yaml`, …) applied after the main sources.
- **Precedence:** sources are applied left → right; later sources overwrite earlier ones.
  Undefined source values are skipped when the destination already has a value.
- **Objects:** merged recursively (deep) by default.
- **Arrays:** configurable — `merge` (default, index-wise), `overwrite`, `concat`.
- **Output format:** `-f json|json5|toml|yaml`, default `yaml`.
- **Variable substitution:** on by default (`--no-envsubst` disables). Shell-style forms:
  `${var}`, `${var-DEFAULT}`, `${var:-DEFAULT}`, `${var=DEFAULT}`, `${var:=DEFAULT}`,
  `${var+ALT}`, `${var:+ALT}`.
- **Limits:** none documented.
- **Delivery:** runs in a Docker container — needs Docker and a filesystem of real files.

**Table stakes taken:** multi-format input, output-format choice, left-to-right precedence,
deep object merge, an array policy knob, and `${VAR}` / `${VAR:-default}` substitution that
is **on by default with an off switch**.

## Competitor 2 — Node config-merging library (Telefonica `node-merge-config`)

- Merges JSON and YAML files, whole directories, environment variables, and CLI arguments
  into one config object.
- Environment variables can be loaded into the config with an optional **allow-list**, and
  env keys are transformed (their convention: camelCase).
- Library-only: no UI, no page, must be wired into a Node app.

**Idea taken:** treating "the environment" as a real layer with its own precedence rather
than as a substitution-only side channel — and the notion that env keys carry structure that
can be projected into nested config. Our version does this with the widely used `__`
(double-underscore) path separator instead of a case transform, because `__` is what
Docker Compose / ASP.NET-style config providers use and it round-trips losslessly.

**Not taken (out of model):** directory scanning and CLI-argv merging — a browser page has no
filesystem and no argv.

## Competitor 3 — Browser TOML overlay/merge tool (tomlkit.org)

- Landing copy: combine a base TOML with an overlay, deep-merge tables, choose a conflict
  policy and an array policy.
- Part of a suite: a separate `.env → TOML` builder tool exists alongside it.
- Documentation on the landing page is thin: the concrete policy choices, limits, and
  examples are not stated up front.

**Table stakes confirmed:** "conflict policy" and "array policy" as *named, visible*
controls, and a base-vs-overlay mental model.
**Gap we close:** their base and overlay must both be TOML, and env is a *separate* tool.
Ours takes any mix of formats in the same merge, so a `.env` layer can override a TOML base
in one pass.

## Competitor 4 (secondary) — Browser YAML mergers (merge-json-files.com, and similar)

- Multiple `.yaml`/`.yml` files via drag-and-drop; processing is in-browser (a privacy angle
  they advertise explicitly).
- Strategies are about *packaging*, not semantics: wrap the files in an array under a root
  key, wrap them in an object keyed by filename, or emit a multi-document stream separated by
  `---`.
- Conflict rule is **shallow**: same top-level key → last file wins, whole value replaced.
  The docs admit this and point users at `yq` for recursive merging.
- Six FAQ entries covering strategy choice, indentation, Kubernetes fit, comment handling,
  environment configs, and privacy.
- Stated limit: no semantic deep merge for Kubernetes / Docker Compose files that share
  nested keys.

**Table stakes taken:** in-browser/no-upload privacy statement, a stated indentation control,
a real FAQ that names the limits, and env-config layering as the headline use case.
**Gap we close:** deep recursive merge is the *default* here, not an unavailable feature.

## Synthesis — what shipped

| Table stake | Where it landed |
| --- | --- |
| Mixed-format inputs in one merge | `input_format = auto` per-layer detection (json/yaml/toml/env), with an explicit override |
| Left-to-right precedence | 4 ordered layers, `layer1` lowest → `layer4` highest |
| Output-format choice | `output = json\|yaml\|toml\|env\|report` |
| Deep vs shallow object merge | `object_merge = deep\|shallow` (deep default) |
| Array policy | `array_merge = replace\|append\|unique` (replace default) |
| `${VAR}` substitution, on by default | `substitute` (default true) + `vars` for values that must not appear in the output |
| Shell default forms | `${VAR:-default}` and `${VAR-default}` both supported |
| Deleting an inherited key | `null_deletes` (default true) |
| Stable output for diffs | `sort_keys` |
| Indentation | `indent` (1–8) |
| Which file set this value | `output = report` — per-key provenance + the full override chain |
| Stated limits | 256 KiB total input / depth 64 / TOML has no null — all on the page |

## Considered, not built

- **Out of model — file/directory input.** Competitor 1 and 2 read real files and whole
  directories, and derive layer names from filenames. A browser block gets pasted text, so
  layer names are supplied via `layer_names` instead.
- **Out of model — sourcing `.sh` scripts.** Competitor 1 *executes* `.env`/`.sh` files in a
  shell to collect variables. Running shell is not something a sandboxed wasm block does, and
  should not be. Static `KEY=VALUE` parsing covers the same files without the execution.
- **Out of model — reading the real process environment.** There is no ambient environment in
  the browser; `vars` is the explicit, auditable substitute.
- **Considered, rejected — JSON5 input.** Competitor 1 accepts JSON5/`.js`. `blocks/json5-convert`
  already owns JSON5, and adding a fifth auto-detected syntax makes detection materially more
  ambiguous for a format that is rare as a *config layer*. Convert first, then merge.
- **Considered, rejected — the full shell parameter-expansion grammar.** `${var=D}`,
  `${var:=D}`, `${var+A}`, `${var:+A}` mutate or invert the variable set; in a pure
  data-merge context they add four near-identical syntaxes for a case that barely occurs in
  config files. The two forms that actually appear in `.env`/compose files —
  `${VAR:-default}` and `${VAR-default}` — are supported, and anything unresolved is left
  literal and reported instead of silently blanked.
- **Considered, rejected — packaging strategies (array/object/document wrappers).**
  Competitor 4's three "strategies" solve *not having a deep merge*. With a real deep merge
  they are noise; `blocks/yaml-deep-merge` already emits multi-document YAML if that is what
  someone wants.
- **Considered, rejected — comment preservation.** Advertised by some YAML mergers. Across
  four formats there is no shared comment model, and a merged value's comment is ambiguous by
  construction. The page states plainly that comments are dropped.

## Overlap check against shipped blocks

Not a duplicate of any existing block: `blocks/json-merge` (JSON only), `blocks/yaml-deep-merge`
(YAML only, Helm semantics), `blocks/env-file-merger` (`.env` only, flat KEY=VALUE),
`blocks/json-yaml-convert` and `blocks/data-format-converter` (convert one document, no merge).
The distinguishing capability here is **merging layers that are in different formats** plus
**variable substitution across the merged result** — no shipped block does either.
