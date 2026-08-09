# license-checker — competitor analysis (2026-08-09)

Scan run **before** implementation, per `create-next-tool` step 4. Everything below is a
**paraphrase** of publicly documented behaviour — no competitor copy, branding, or trademark text
is reproduced, and no competitor asset is used. Out-of-model items are listed, not built.

## Scope of our tool

Row picked from the backlog: *"Checks the SPDX license IDs in an SBOM or dependency list against
user-supplied allow/deny rules."* Type: **pure** (paste text → report text; no network, no
resolver, no filesystem walk).

## Competitors reviewed

| # | Tool | Ecosystem | What it does |
|---|------|-----------|--------------|
| 1 | cargo-deny (`licenses` check) | Rust | Evaluates each crate's SPDX **expression** against a TOML allow list, per-crate exceptions, and a confidence threshold for text-detected licenses. |
| 2 | npm `license-checker` | Node | Walks `node_modules`, prints a license inventory, and can fail the build via an allow list or a fail-on list. |
| 3 | `licensee` (jslicense) | Node | Checks package metadata against an allow list of SPDX IDs **or** a minimum license *rating* tier; supports per-package overrides and ignore rules. |
| 4 | `license-compliance` | Node | Simpler allow-list gate over installed packages; non-zero exit when a package is non-compliant. |
| 5 | LicenseFinder | Multi-language | Scans many package managers, keeps a persisted approve/deny decision set, reports what is unapproved. |

Reachability note: npmjs.com returned HTTP 403 to the fetcher, so the npm tools were read from
their upstream repository READMEs instead of the registry pages. All five are real, actively
referenced tools; no substitution was needed.

## Table-stakes extracted, and where each landed

| Capability (seen in ≥1 competitor) | Fit | Where it landed |
|---|---|---|
| Allow list of SPDX IDs; anything else fails | in-model | `allow` param |
| Deny list / fail-on list of SPDX IDs | in-model | `deny` param |
| **SPDX expression** evaluation — `OR` picks any acceptable branch, `AND` requires all, `WITH` exception clauses, parentheses | in-model | core expression evaluator; `WITH`/full-expression strings are also directly matchable in `allow`/`deny` |
| `-or-later` / `+` and deprecated-ID handling (`GPL-3.0` ⇒ `GPL-3.0-only`), GNU version pedantry | in-model | ID normalisation table; `-or-later` matches an `-or-later` or plain-family allow entry, never a narrower `-only` |
| License **category / rating tiers** instead of naming every ID (cargo-deny copyleft posture, licensee's rating tiers) | in-model | `category:` tokens usable inside `allow`/`deny` (`category:permissive`, `category:weak-copyleft`, `category:strong-copyleft`, `category:network-copyleft`, `category:public-domain`, `category:proprietary`, `category:unknown`) |
| Per-package **exceptions** / overrides that bypass the rules | in-model | `exceptions` param (`name` or `name@version`) |
| Distinct policy for packages with **no license metadata** | in-model | `unknown` param (`allow`/`warn`/`deny`) |
| Distinct policy for a license that matches no rule when an allow list exists | in-model | `unlisted` param (`allow`/`warn`/`deny`, default `deny` — matches the allow-list-is-exhaustive posture) |
| Validity check of the identifier itself against the SPDX list | in-model | `validate_ids` param; unrecognised IDs are flagged in the report |
| Machine-readable output (JSON) + spreadsheet output (CSV) + human report | in-model | `output` = `text` / `markdown` / `json` / `csv` |
| Summary counts / roll-up by license | in-model | every output carries a summary block plus a per-license roll-up |
| Show the full inventory, not only violations | in-model | `include_allowed` boolean (default off, so the report leads with violations) |
| Pass/fail verdict suitable for a CI gate | in-model | explicit `PASS`/`FAIL` verdict line + `"verdict"` field in JSON |
| Multiple input shapes (SBOM and plain dependency lists) | in-model | `input_format` = `auto` / `cyclonedx-json` / `spdx-json` / `spdx-tag` / `npm-json` / `list` |

## Considered, **not** built (out of model)

- **Scanning an installed dependency tree** (`node_modules`, a Cargo workspace, a virtualenv).
  Requires filesystem/package-manager access; gizza blocks are browser-local pure compute over
  pasted text. Our input is the SBOM or dependency list a scanner already produced (e.g. from the
  sibling `sbom-generator` block).
- **Detecting a license from raw LICENSE-file text** (cargo-deny's confidence threshold, SPDX
  Online Tools' license-text matching). Needs a full license-text corpus plus fuzzy matching; far
  beyond a text-in/text-out block, and the corpus alone would dwarf the wasm artifact.
- **Fetching license metadata from a registry** for packages whose SBOM entry has none. Network
  I/O is out of model for a pure block.
- **Persisted approval state across runs** (LicenseFinder's decision file). Blocks are stateless;
  the equivalent here is passing `exceptions` explicitly, which keeps the run reproducible.
- **Process exit codes.** The block returns a report; the CLI prints it. The `PASS`/`FAIL` verdict
  line and the JSON `verdict` field are the greppable equivalent for a CI step.
- **Author/scope-based ignore rules** (licensee). SBOM entries do not reliably carry author
  metadata, and package-name exceptions already cover the same intent.

## Rejected on judgment (in-model but declined)

- **A separate `allow_categories` / `deny_categories` parameter pair.** Folding category tokens
  into `allow`/`deny` keeps one precedence order to reason about (deny wins, then allow, then the
  `unlisted` policy) and avoids two more fields on the form for the same idea.
- **`tag-list` pill control for `allow`/`deny`.** Rejected: SPDX expressions users legitimately
  paste — `(MIT OR Apache-2.0)`, `Apache-2.0 WITH LLVM-exception` — read badly as pills, and the
  common workflow is bulk-pasting a policy list. Multiline text fields with worked placeholders
  are the better control here.

## UX patterns adopted

- Preset chips (`[[example]]`) for the postures competitors ship as documented recipes:
  a permissive-only policy, a no-copyleft policy, and a CSV inventory run.
- Defaults that produce a useful result on a bare paste: with no rules at all the tool still
  reports the inventory, the per-license roll-up, and flags unknown/invalid IDs.
- Errors name what was expected (`unknown output '…' (expected text, markdown, json, or csv)`).
