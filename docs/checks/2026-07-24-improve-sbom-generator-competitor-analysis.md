# sbom-generator — competitor analysis (2026-07-24)

Function: read a resolved dependency **lockfile** (npm `package-lock.json`, Rust `Cargo.lock`,
Python `requirements.txt`) and emit a Software Bill of Materials (SBOM) in **CycloneDX** or **SPDX**
format. A lockfile is already the fully-resolved dependency graph, so no package-manager resolver /
network is needed — this fits gizza's pure-wasm, browser-local model exactly.

Scan of the top real tools (features/params paraphrased only — no copy/branding reused):

## Competitors surveyed

1. **Syft (Anchore)** — the reference multi-ecosystem SBOM CLI. Catalogs from lockfiles AND
   installed dirs/images; outputs `cyclonedx-json`, `cyclonedx-xml`, `spdx-json`, `spdx-tag-value`,
   `syft-json`, table. Auto-detects the ecosystem from the file. Emits package `purl`s
   (`pkg:npm/...`, `pkg:cargo/...`, `pkg:pypi/...`), a primary/root component, and dependency
   relationships. Table-stakes: format choice, ecosystem auto-detect, purls, pretty output.

2. **cyclonedx-npm / @cyclonedx/cyclonedx-npm** — Node-specific. Reads `package-lock.json`
   (v1 `dependencies` and v2/v3 `packages`), produces CycloneDX 1.4–1.6 JSON/XML. Flags:
   include/omit `dev` dependencies, set the root component name/version, `--spec-version`,
   `--output-format json|xml`. Table-stakes: dev-dependency toggle, root component metadata,
   spec version.

3. **cyclonedx-py (CycloneDX Python)** — reads `requirements.txt` / `poetry.lock` / `Pipfile.lock`.
   Handles `name==version` pins, extras `name[extra]`, environment markers (`; python_version<"3.8"`),
   comments, and `-r`/`-e` directives. Emits `pkg:pypi/<normalized-name>@<version>`. Table-stakes:
   requirements.txt edge-case parsing (extras, markers, comments), PEP 503 name normalization.

4. **sbom4rust / sbom4python (APH10)** — language-specific generators, both CycloneDX + SPDX.
   sbom4rust walks `Cargo.lock` `[[package]]` entries (name, version, source). Confirms Cargo.lock
   TOML parsing + cargo purls is a table-stake, and that both output standards matter per ecosystem.

5. **SBOM viewer / cyclonedx.org & spdx.dev web tools** — browser tools that ingest an SBOM,
   detect its format, and view/convert CycloneDX <-> SPDX. Confirms the two dominant standards
   (CycloneDX 1.x, SPDX 2.3) and that JSON is the default serialization, with SPDX also shipping a
   tag-value text form.

## Table-stakes → decision (each lands in the descriptor or is listed out-of-model)

| Capability | Decision |
|---|---|
| Read npm `package-lock.json` (v1 + v2/v3) | in-model — `input_format` auto/npm |
| Read `Cargo.lock` (TOML `[[package]]`) | in-model — auto/cargo, `toml` crate |
| Read `requirements.txt` (pins, extras, markers, comments) | in-model — auto/pip |
| Auto-detect ecosystem from content | in-model — `input_format = auto` (default) |
| CycloneDX JSON output | in-model — `output = cyclonedx-json` (default) |
| SPDX JSON output (2.3) | in-model — `output = spdx-json` |
| SPDX tag-value text output | in-model — `output = spdx-tag` |
| Package URLs (purl) per component | in-model — always emitted |
| Primary/root component metadata (name + version) | in-model — `component_name` / `component_version`, auto-derived from the lockfile when present |
| Include / omit dev dependencies (npm) | in-model — `include_dev` boolean (default true) |
| Pretty-print JSON | in-model — `pretty` boolean (default true) |
| Deterministic output (stable component order, no random UUID/clock by default) | in-model — components sorted, optional `timestamp` param (omitted when empty) |
| CycloneDX/SPDX **XML** serialization | considered, rejected — JSON + SPDX tag-value cover the dominant real-world use; XML adds a large surface for marginal value. Listed, not built. |
| SBOM **<-> conversion** / validation / vuln scan / license enrichment | out-of-model — needs a components graph merge / vuln DB / network; separate tools. Listed, not built. |
| poetry.lock / Pipfile.lock / yarn.lock / pnpm / go.sum | out-of-model for v1 — the three named ecosystems ship now; others are additive follow-ups. |
| Full dependency **relationships** graph (who-depends-on-whom) | considered, rejected for v1 — lockfiles express this inconsistently across ecosystems; v1 emits the flat component inventory + a DESCRIBES/DEPENDS_ON root relationship (SPDX) which is the load-bearing part. |

No competitor copy, wording, or branding is reused; all page/CLI copy is original.
