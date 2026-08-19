## About this tool

**License Checker** evaluates SPDX license metadata from an SBOM or dependency
inventory against a policy you paste into the form. It is meant for quick CI
policy design, PR review, and one-off audits where you already have the package
list and want a deterministic PASS/FAIL report.

It accepts common inventory shapes:

- CycloneDX JSON SBOMs (`components[].licenses`).
- SPDX JSON or SPDX tag-value documents.
- npm-style JSON maps such as `{ "pkg@1.0.0": { "licenses": "MIT" } }`.
- Plain lists: `name@version: MIT`, `name,version,MIT`, or `name MIT`.

Rules can be exact SPDX identifiers (`MIT`, `Apache-2.0 WITH LLVM-exception`) or
license-family categories such as `category:permissive`,
`category:strong-copyleft`, and `category:network-copyleft`. SPDX expressions are
evaluated rather than string-matched: `MIT OR Apache-2.0` passes if either branch
is allowed, while `MIT AND GPL-3.0-only` requires both branches to be acceptable.

### Worked example

Paste this dependency list:

```text
chalk@4.1.2: MIT
copyleft-lib@2.0.0: GPL-3.0-only
dual@1.0.0: MIT OR Apache-2.0
mystery@0.1.0: NOASSERTION
```

Set **Allowed licenses/categories** to:

```text
MIT, Apache-2.0, category:public-domain
```

The report fails `copyleft-lib`, accepts the `MIT OR Apache-2.0` expression, and
warns about the missing license on `mystery` unless you change **Missing license
policy** to `allow` or `deny`.

### Limits and edge cases

- This is not legal advice; the category map is a practical compliance grouping.
- It does not crawl `node_modules`, Cargo workspaces, or package registries. Use
  an SBOM generator first, then paste the SBOM here.
- It validates common SPDX identifiers and exceptions, but custom `LicenseRef-*`
  values are treated as valid SPDX custom identifiers.
- Package exceptions are explicit and reproducible: pass `name` or
  `name@version`; no decision state is saved between runs.

## FAQ

<details>
<summary>Can this scan my repository and discover dependency licenses?</summary>

No. This tool checks license metadata you already have. Generate a CycloneDX or
SPDX SBOM with your build tooling, or paste a dependency list, then use this
checker to apply allow/deny rules locally in the browser.

</details>

<details>
<summary>How do OR and AND SPDX expressions affect the verdict?</summary>

`OR` means the package offers alternatives, so the expression is accepted when at
least one branch is allowed and not denied. `AND` means multiple obligations
apply, so every branch must be acceptable. A deny rule always wins over an allow
rule.

</details>

<details>
<summary>What is the difference between unlisted and unknown?</summary>

**Unlisted** means a package has a license, but it does not match any allow rule
when an allow list is configured. **Unknown** means the package has no usable
license metadata at all, such as `NOASSERTION` or an empty field. Each has its
own allow/warn/deny policy.

</details>

<details>
<summary>Why use category rules instead of listing every SPDX ID?</summary>

Categories let you express a posture, such as allowing permissive licenses while
denying strong and network copyleft families. You can still mix exact SPDX IDs
with category tokens when a specific license needs special treatment.

</details>
