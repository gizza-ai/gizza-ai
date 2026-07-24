# sbom-diff — competitor analysis (2026-07-24)

Tool: compare two dependency lockfiles or SBOMs and report **added / removed /
version-bumped** dependencies with counts. Pure, browser-local, deterministic —
no network, no resolver, no vulnerability database.

## Landscape surveyed (paraphrased, no copied copy/branding)

| Competitor (class) | What it does | Relevant table-stakes it sets |
| --- | --- | --- |
| CycloneDX CLI `diff` | Compares two CycloneDX BOMs; lists components added/removed and, with `--component-versions`, version changes | added/removed/changed sections; version-change detection; JSON + text |
| GitHub Dependency Review | On a PR, diffs base vs head manifests/lockfiles; renders added/removed/updated dependencies | grouped added/removed/updated; per-ecosystem grouping; markdown table render |
| Renovate / Dependabot PR body | Summarize the "old → new" version bump per dependency in an update PR | old→new version arrow; upgrade vs downgrade direction |
| `npm-lockfile-diff` / `lockfile-diff` (npm) | Diff two `package-lock.json`/`yarn.lock` and print added/removed/bumped packages | lockfile-native diff; dev-vs-prod filter; text output |
| `sbomdiff` / `bom-diff` utilities | Diff two SBOMs (CycloneDX/SPDX) and emit a change report | multi-format SBOM input; counts summary; machine-readable output |

## Table-stakes (all shipped in-model)

- **Added** dependencies (present only in the new file).
- **Removed** dependencies (present only in the old file).
- **Changed / version-bumped** dependencies (present in both, version differs),
  reported as `old → new` with an **upgrade / downgrade** direction.
- **Counts summary** (added / removed / changed / unchanged).
- **Multiple input formats**: npm `package-lock.json`, `Cargo.lock`, pip
  `requirements.txt`, plus existing SBOMs (CycloneDX JSON, SPDX JSON, SPDX
  tag-value) — auto-detected or forced per side.
- **Machine-readable output** (JSON) alongside a human report; markdown for PR/CI.
- **Dev-dependency filter** for npm.
- **Deterministic** ordering (sorted, de-duplicated).

## In-model decisions (built here)

- Reuse `sbom-generator` parsing for the three lockfile formats (call its
  CycloneDX serializer, read the components back) so npm/cargo/pip stay
  byte-identical to the sibling tool; add first-class parsers for CycloneDX
  JSON, SPDX JSON and SPDX tag-value **as inputs** (sbom-generator only emits
  these, never reads them).
- Diff key = `(ecosystem, name)`; versions collected as a set so a package that
  legitimately appears at multiple versions (common in `Cargo.lock`) diffs
  cleanly. Direction (upgraded/downgraded) is computed with a dependency-free
  dotted-numeric version comparison when both sides are single-version.
- Three output formats: `text` (grouped report), `markdown` (PR/CI table), `json`.
- `old_format` / `new_format` default to `auto`; `include_dev` (npm) defaults on.

## Out of model (deliberately NOT built — needs network/DB or a resolver)

- Vulnerability / CVE deltas (Snyk, Dependabot alerts, GitHub dependency review
  security tab) — needs an advisory database and network.
- License-change review / policy gates — needs registry license enrichment.
- Resolving a manifest (`package.json`, `Cargo.toml`) with ranges — needs a
  package-manager resolver + network; this tool consumes already-resolved
  lockfiles/SBOMs only.
- Transitive-path / dependency-graph reachability of a change.

## UX controls exposed on the page

- Two multiline text areas (old / new) with realistic lockfile placeholders.
- `old_format` / `new_format` selects (auto + 6 explicit formats), labelled.
- `include_dev` checkbox (default on), `output` select (text / markdown / json).
- Example chips: **npm bump**, **Cargo add/remove**, **CycloneDX → JSON**.

## Worked example

Old `package-lock.json` has `chalk@4.1.2` + `left-pad@1.3.0`; new has
`chalk@5.0.0` + `lodash@4.17.21`. Result: **added** `lodash@4.17.21`, **removed**
`left-pad@1.3.0`, **changed** `chalk 4.1.2 → 5.0.0 (upgraded)`; summary
`added 1, removed 1, changed 1`.
