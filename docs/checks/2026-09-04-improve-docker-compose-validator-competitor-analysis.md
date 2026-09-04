# docker-compose-validator — competitor analysis (2026-09-04)

Scan run BEFORE implementation, per `/improve-tool` Phase 2–3. Everything below is a
**paraphrase** of observed behaviour; no competitor copy, branding or trademark text was
reused anywhere in this tool.

## Sources skimmed

| # | Source | Reachable | Notes |
|---|--------|-----------|-------|
| 1 | `vahac.com` — browser docker-compose validator | yes | Richest rule set of the reachable web tools; severity tiers + a docker-run→compose converter. |
| 2 | `dev-toolbox.tech` — docker-compose validator | yes | Zero-config validator; auto-validate, sample loader, summary counts. |
| 3 | `zavoloklom/docker-compose-linter` (DCLint) rule reference | yes | The de-facto CLI rule vocabulary (style / security / best-practice categories). Used as the capability yardstick. |
| — | `devtoolbox.dedyn.io` | no (DNS failure) | Replaced by source 3. |
| — | `toolkitgen.com` | no (HTTP 403) | Replaced by source 3. |
| — | `abacktools.com` | listing only | No per-tool detail published; not counted. |

Search overview additionally confirmed the generic YAML validators (codebeautify, jsonformatter)
that people currently paste compose files into — they only check YAML syntax, with no
compose-aware semantics. That is the gap this tool targets.

## Table-stakes observed (with defaults), and where each landed

| Capability | Seen at | Default there | Decision |
|---|---|---|---|
| YAML syntax error with line/column | 1, 2, 3 | always on | **in-model** → rule `syntax` |
| Root must be a mapping with a `services` key | 2 | always on | **in-model** → `top-level-type`, `services-missing` |
| Service defines neither `image` nor `build` | 1, 2 | error | **in-model** → `image-or-build` |
| Service defines *both* `build` and `image` | 3 | error | **in-model**, but shipped as a **warning** — Compose genuinely supports build-then-tag, so an error would be wrong |
| Port mapping syntax + 1–65535 range | 1, 2 | error | **in-model** → `port-syntax` (short + long syntax, ranges, `/proto`, host IP, `[::1]` brackets) |
| Duplicate published host ports | 3 | error | **in-model** → `duplicate-host-port` |
| Duplicate `container_name` | 3 | error | **in-model** → `duplicate-container-name` |
| `depends_on` → undefined service | 1, 2 | error | **in-model** → `undefined-depends-on` |
| Circular `depends_on` | 2 | error | **in-model** → `circular-depends-on` (DFS, reports the cycle path) |
| Named volume used but not declared top-level | 1 | error | **in-model** → `undefined-volume` |
| Network referenced but not declared | 2 | error | **in-model** → `undefined-network` |
| Obsolete top-level `version:` | 1, 3 | warning | **in-model** → `version-field` |
| `:latest` / untagged image | 1, 3 | warning / error | **in-model** → `image-tag` (warning; digest pins count as pinned) |
| Missing `restart:` policy | 1 | warning | **in-model** → `missing-restart` (hint, strict preset) |
| Host port published on all interfaces | 1, 3 | warning / error | **in-model** → `unbound-port-interface` (hint, strict preset — binding `0.0.0.0` is often deliberate) |
| `privileged: true`, `network_mode: host` | 1 | warning | **in-model** → `privileged`, `host-network` |
| Secret-looking literal in `environment:` | 1 | warning | **in-model** → `env-secrets` (skips `${VAR}` interpolation) |
| Deprecated `links:` | 1 | warning | **in-model** → `deprecated-links` |
| Hints: missing healthcheck / resource limits / logging | 1 | **off by default** | **in-model** → `missing-healthcheck`, `resource-limits`, `logging-options` (hints, strict preset — matches "off by default") |
| Project `name:` field present | 3 | warning | **in-model** → `project-name` (hint, strict preset) |
| Quote port mappings | 3 | warning | **in-model** → `quote-ports` (plain-style scalars only; scalar style comes from the marked event parser) |
| Severity tiers + filter by level | 1, 2 | 3 tiers | **in-model** → `min_severity` param (`hint`/`warning`/`error`) |
| Sample/example loader | 1, 2 | button | **in-model** → four `[[example]]` preset chips |
| Copy / download result | 1 | buttons | **already platform** — the generator gives Copy + Download to every text tool |
| Auto-validate on typing | 2 | toggle | **already platform** — the page recomputes on input; no toggle needed |
| Per-issue remediation text | 1, 2 | always | **in-model** → every problem message states what was expected and what to do |
| Machine-readable output for CI | 3 (CLI) | JSON/codeclimate | **in-model** → `report_format=json` |
| Rule opt-out | 3 (config file) | config | **in-model** → `disable` (rule ids, tag-list control) |
| Warnings-as-errors CI mode | 3 | flag | **in-model** → `strict_warnings` |

## Considered and rejected (in-model but declined)

- **Auto-fix / reformat output.** DCLint auto-fixes style rules and one web tool ships a
  "format" button. Rewriting the user's file is a *formatter's* job, and this repo already has
  `yaml-formatter` for that. A validator that silently reorders keys would also destroy the
  line numbers its own findings point at.
- **Alphabetical-ordering and key-order style rules** (DCLint's whole style category). Pure
  formatting preference with no correctness signal; they would dominate the report with noise.
- **`docker run` → Compose conversion** (source 1 bundles it). That is a different tool's job;
  `blocks/docker-compose-generator` already covers spec→compose generation.
- **Missing `container_name` as a finding** (source 1 lists it as a hint). Setting
  `container_name` actively prevents `--scale` and is discouraged in multi-replica setups, so
  flagging its absence would give bad advice.

## Out of model (needs a backend / account — not built)

- Pulling the referenced images to verify tags actually exist on a registry.
- Resolving `env_file:` / `include:` / `extends: file:` targets from disk.
- Running `docker compose config` for a real engine-side verdict.
- Persisted project config files, shareable result links, team rule presets.

## Positioning

The generic YAML validators people currently use for compose files stop at "is this valid
YAML". Everything in the *error* tier here (undefined volumes/networks/services, port syntax,
dependency cycles, duplicate host ports) parses as perfectly valid YAML and fails only when
`docker compose up` runs. That's the wedge, and it drives the page copy and tags.
