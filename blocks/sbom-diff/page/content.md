## About this tool

SBOM Diff compares two resolved dependency files — an "old" (before) and a "new" (after) — and reports exactly which packages were **added**, **removed**, or **version-bumped** between them. Paste an npm `package-lock.json`, a Rust `Cargo.lock`, a Python `requirements.txt`, or an existing CycloneDX / SPDX software bill of materials on each side, and the tool renders the change set as a human-readable report, a markdown table for a pull request, or machine-readable JSON.

It does not run a package manager or contact a registry. A lockfile or SBOM already records the resolved dependency set, so this tool parses both sides locally in WebAssembly and compares the inventories. That makes it useful for reviewing a dependency update in code review, gating a CI job on unexpected additions, or auditing what a release actually changed. Because the comparison is a pure set diff, identical input always produces byte-identical output.

### Worked example

Given this old `package-lock.json`:

```json
{"name":"my-app","version":"1.0.0","lockfileVersion":3,"packages":{"":{"name":"my-app","version":"1.0.0"},"node_modules/chalk":{"version":"4.1.2"},"node_modules/left-pad":{"version":"1.3.0"}}}
```

and this new one (chalk bumped, left-pad dropped, lodash added):

```json
{"name":"my-app","version":"1.1.0","lockfileVersion":3,"packages":{"":{"name":"my-app","version":"1.1.0"},"node_modules/chalk":{"version":"5.0.0"},"node_modules/lodash":{"version":"4.17.21"}}}
```

with **output** `text`, the result is:

```text
Dependency diff
  added:     1
  removed:   1
  changed:   1
  unchanged: 0

Added (1)
  + npm lodash@4.17.21

Removed (1)
  - npm left-pad@1.3.0

Changed (1)
  ~ npm chalk 4.1.2 -> 5.0.0  (upgraded)
```

Switch **output** to `markdown` for a pull-request table, or to `json` for a `{ "summary": …, "added": …, "removed": …, "changed": … }` change report. Each changed package is labelled `upgraded`, `downgraded`, or `changed`.

### Limits & notes

- Supported inputs (per side, auto-detected or forced): npm `package-lock.json` v1/v2/v3, Rust `Cargo.lock`, pip `requirements.txt`, CycloneDX JSON, SPDX JSON, and SPDX tag-value SBOMs. The two sides may use different formats.
- Both inputs must be **resolved** — a lockfile or SBOM, not a manifest like `package.json` or `Cargo.toml`. Unresolved version ranges are not expanded.
- The root project (the SBOM's described component) is excluded so its own version bump does not appear as a change. Each side is capped at 200,000 components.
- Version direction (`upgraded` / `downgraded`) is computed with a dependency-free dotted-version comparison; a package resolved to more than one version becomes a multi-version `changed` entry rather than a direction.
- The tool compares the dependency inventory only. It does not diff hashes, licenses, or vulnerability data, and does not fetch anything from a registry.

## FAQ

<details>
<summary>What is the difference between the "old" and "new" side?</summary>

The **old** field is the before state and the **new** field is the after state. A package present only in *new* is reported as **added**, one present only in *old* is **removed**, and one whose version differs between the two is **changed**. Swapping the two inputs flips added and removed and inverts each upgrade/downgrade label.

</details>

<details>
<summary>Can I diff two different file formats?</summary>

Yes. Each side is detected or forced independently, so you can compare an npm `package-lock.json` against a CycloneDX SBOM, or last release's SPDX document against this release's `Cargo.lock`. The tool normalizes both to a package-URL–keyed inventory before diffing, so cross-format comparisons line up by ecosystem and package name.

</details>

<details>
<summary>How are version bumps classified?</summary>

When a package resolves to a single version on each side, the tool compares them with a numeric-aware dotted-version compare and labels the change `upgraded` or `downgraded`. If either side has multiple versions, or the versions are not orderable, the change is labelled `changed` without a direction.

</details>

<details>
<summary>Does it include dev dependencies?</summary>

For npm `package-lock.json` input, the **Include npm dev / optional dependencies** toggle controls whether packages marked `dev` or `optional` are counted on both sides. Cargo, pip, and SBOM inputs do not carry that distinction in this parser, so the toggle only affects npm lockfiles.

</details>

<details>
<summary>Is my data uploaded anywhere?</summary>

No. Both files are parsed and diffed in your browser through WebAssembly. The lockfile and SBOM text stay on your device unless you copy the generated diff somewhere else.

</details>
