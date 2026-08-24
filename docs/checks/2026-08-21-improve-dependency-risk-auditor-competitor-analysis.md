# dependency-risk-auditor — competitor analysis (2026-08-21)

Scan run BEFORE implementing, per `create-next-tool` step 4. One WebSearch
("audit package.json lockfile risky dependencies wildcard versions git dependencies install
scripts tool"), then the top 3 reachable real tools were skimmed. All notes below are
**paraphrased** — no competitor copy, branding or trademarks are reproduced or shipped.

## Competitors reviewed

### 1. package.json Dependency Auditor (elysiatools.com) — closest direct competitor (browser tool)
- **Inputs:** package.json textarea (required), lockfile textarea (optional), strictness
  `<select>` (strict / standard / lenient, default standard), lockfile-type `<select>`
  (auto-detect / npm / yarn), a tree-depth number, and two display checkboxes
  (dependency table, dependency tree).
- **Findings:** duplicate packages declared in both `dependencies` and `devDependencies`;
  wildcard / pre-release version specs; unsorted keys; missing metadata such as `engines`;
  runtime-vs-dev misclassification; conflicting resolved versions when a lockfile is supplied.
  It also classifies each spec (caret / tilde / exact / wildcard / workspace / alias / git-url /
  comparator).
- **Output:** an HTML report with a project **grade**, a findings list, a version-policy table,
  and optional dependency table / resolved tree.
- **UX:** a run button, two **example preset buttons** that prefill the form, display toggles,
  and the strictness dropdown.

### 2. sdc-check (github.com/mbalabash/sdc-check) — CLI/CI supply-chain checker
- **Rules (10):** lockfile tampering, package released very recently, **install scripts**,
  obfuscated code, OS batch/shell scripts inside a package, dangerous shell commands
  (`curl`, `wget`, permission changes), release after a long dormancy, unmaintained package
  (no update for ~12 months), too many publishers (default limit 7), missing repository/homepage.
- **Config:** thresholds in `package.json` — maintainer limit, "how new is too new" day buffer
  (default 5), inactivity months (default 10), and which rules should hard-fail the run.
- **Severity:** flat; a rule either fails the run or does not.

### 3. package-lock-audit (github.com/Mermade/package-lock-audit) — lockfile-only CLI
- **Checks:** every entry has an `integrity` field; `resolved` uses https not http; `resolved`
  points at the npm registry host; the resolved URL's version matches the entry's version; the
  resolved URL's name matches the package name; packages that shadow a Node.js built-in module;
  optional GPL-only license flag (`--nogpl true`).
- **Options:** `--verbose 1`, `--nogpl true`, one or more lockfile paths.
- **Output:** console report; non-zero exit code on findings (CI-oriented).

## Table stakes → where each landed

| Table-stake capability | Source | Decision |
| --- | --- | --- |
| Wildcard / `*` / `latest` version specs | 1 | **In model** → rule `wildcard-version` (high) |
| Dist-tag specs (`next`, `beta`) | 1 | **In model** → rule `dist-tag-version` (high) |
| Pre-release specs (`-rc.1`, `-beta.2`) | 1 | **In model** → rule `prerelease-version` (medium) |
| git / GitHub-shorthand dependencies | 1, 3 | **In model** → rule `git-dependency` (high) |
| Remote tarball URL dependencies | 1 | **In model** → rule `url-dependency` (high) |
| Plain-http (non-TLS) specs / resolved URLs | 3 | **In model** → `http-dependency`, `insecure-resolved-url` (high) |
| Local `file:` / `link:` / `portal:` deps | 1 | **In model** → rule `file-dependency` (medium) |
| npm alias specs (`npm:other@1`) | 1 | **In model** → rule `alias-dependency` (medium) |
| Install / lifecycle scripts in the manifest | 2 | **In model** → `install-script` (high), `lifecycle-script` (medium) |
| Packages that run an install script (lockfile flag) | 2 | **In model** → `has-install-script` (high) via npm `hasInstallScript` / pnpm `requiresBuild` |
| Same package in `dependencies` and `devDependencies` | 1 | **In model** → rule `duplicate-dependency` (medium) |
| Missing `integrity` hash | 3 | **In model** → rule `missing-integrity` (high) |
| Weak (sha1) integrity hash | 3 (extension) | **In model** → rule `weak-integrity` (medium) |
| Resolved host is not the public npm registry | 3 | **In model** → rule `third-party-registry` (medium) |
| Resolved URL version disagrees with the entry version | 3 | **In model** → rule `resolved-version-mismatch` (medium) |
| Packages shadowing Node built-in module names | 3 | **In model** → rule `builtin-shadow` (low; browser shims are legitimate) |
| `overrides` / `resolutions` forcing transitive versions | 1 | **In model** → rule `forced-override` (low) |
| Missing `engines` metadata | 1 | **In model** → rule `missing-engines` (low) |
| Loose caret/tilde range prefixes | 1 | **In model** → rule `range-prefix` (low, strict-only by severity filter) |
| Legacy `lockfileVersion: 1` | 3 (context) | **In model** → rule `legacy-lockfile-version` (low) |
| Declared dependency absent from the lockfile | 1 | **In model** → rule `unlocked-dependency` (medium), needs both files |
| Exact pin disagrees with the locked version | 1 | **In model** → rule `pin-mismatch` (medium), needs both files |
| Strictness levels (lenient / standard / strict) | 1 | **In model** → `strictness` param, severity gate |
| Lockfile-type selector with auto-detect | 1 | **In model** → `manifest_format` param (auto / package-json / package-lock / yarn-lock / pnpm-lock) |
| Runtime-vs-dev scope control | 1 | **In model** → `include_dev` boolean |
| Project grade / score | 1 | **In model** → weighted score + A–F grade in every output format |
| CI-style non-zero exit / verdict threshold | 2, 3 | **In model** → `fail_on` param drives a PASS/FAIL verdict |
| Per-rule suppression | 2 | **In model** → `ignore` param (comma-separated rule IDs) |
| Preset example buttons | 1 | **In model** → three `[[example]]` chips on the page |
| Machine-readable report | 2, 3 | **In model** → `output` = text / markdown / json |

## Out of model (listed, deliberately NOT built)

Every one of these needs a registry/network lookup or the package tarball itself; this block is
pure, offline, deterministic compute over pasted file contents.

- **Known-vulnerability (CVE/advisory) matching** — requires the GitHub Advisory Database.
- **Package age / "released too recently"**, **release after long dormancy**,
  **unmaintained (no publish in N months)**, **maintainer/publisher count**,
  **missing repository or homepage** (sdc-check) — all need registry metadata.
- **Obfuscated code, OS scripts and dangerous shell commands inside packages** (sdc-check) —
  needs the downloaded tarball contents.
- **Lockfile tampering detection against the registry** — needs the registry's integrity hashes.
  What ships instead is local internal consistency (missing/weak integrity, resolved URL vs
  entry version/name, declared vs locked versions).
- **License checks (`--nogpl`)** — already covered by the existing `blocks/license-checker`
  block; not duplicated here.
- **Resolved dependency tree / tree-depth rendering** (competitor 1) — a visualization, not a
  risk finding; `blocks/sbom-generator` already turns lockfiles into a dependency inventory.
- **Unsorted-keys / key-ordering hygiene** (competitor 1) — style, not risk; deliberately
  omitted so `strict` stays about supply-chain risk rather than formatting.

## Duplicate check

- `blocks/sbom-generator` converts an npm/Cargo/pip lockfile into a CycloneDX/SPDX SBOM
  (inventory, not risk analysis).
- `blocks/sbom-diff` compares two SBOMs.
- `blocks/license-checker` applies SPDX allow/deny policy to an SBOM or dependency list.
- None of them inspect version specs, install scripts, git/URL deps or integrity hashes,
  so this tool is not a semantic duplicate.
