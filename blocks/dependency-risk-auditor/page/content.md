## About this tool

**Dependency Risk Auditor** reads an npm `package.json` or lockfile that you paste
in and reports the supply-chain risks that are visible in the file itself. Every
check is local, deterministic text analysis — there are no registry lookups, so
nothing about your project leaves the browser.

It accepts four file shapes, auto-detected:

- `package.json` — version specs, lifecycle scripts, overrides and metadata.
- `package-lock.json` — `lockfileVersion` 1, 2 and 3.
- `yarn.lock` — both classic v1 and Berry (v2+) layouts.
- `pnpm-lock.yaml` — v5 through v9 `packages:` entries.

Paste a `package.json` in the first box and its lockfile in the second to also get
the cross-checks between them.

### What gets flagged

**High** — `wildcard-version` (`*`, `x`, `latest`, empty), `dist-tag-version`
(`next`, `beta`), `git-dependency`, `url-dependency`, `http-dependency`,
`install-script` (`preinstall`/`install`/`postinstall`), `missing-integrity`,
`insecure-resolved-url`, `git-resolved`, `has-install-script`.

**Medium** — `prerelease-version`, `file-dependency`, `alias-dependency`,
`duplicate-dependency`, `lifecycle-script`, `weak-integrity` (SHA-1),
`third-party-registry`, `resolved-version-mismatch`, `unlocked-dependency`,
`pin-mismatch`.

**Low / info** — `range-prefix` (caret and tilde ranges), `missing-engines`,
`builtin-shadow`, `forced-override`, `legacy-lockfile-version`,
`no-lockfile-supplied`. These only appear at **strict** strictness.

Each report carries a 0–100 risk score and an A–F grade: every high finding costs
20 points, medium 8, low 3 and info 1.

### Worked example

Paste this into **package.json or lockfile**:

```json
{
  "name": "demo-app",
  "version": "1.0.0",
  "scripts": { "postinstall": "node scripts/setup.js" },
  "dependencies": {
    "axios": "*",
    "chalk": "^4.1.2",
    "internal-tool": "git+ssh://git@github.com/acme/internal-tool.git#main"
  },
  "devDependencies": { "chalk": "^5.3.0" }
}
```

With the default **Standard** strictness the report opens with:

```text
DEPENDENCY RISK AUDIT — FAIL
Input: package.json
Entries scanned: 4 | Strictness: standard
Risk score: 32/100 (grade F)
Findings: 3 high, 1 medium, 0 low, 0 info
```

then lists the wildcard `axios` spec, the git-sourced `internal-tool`, and the
`postinstall` script as high findings, plus `chalk` being declared in both
`dependencies` and `devDependencies` as medium. Switching **Strictness** to
`strict` additionally reports the two caret ranges, the missing `engines` field,
and a note that no lockfile was supplied.

### Limits and edge cases

- Only what is in the file can be checked. Known-vulnerability (CVE) matching,
  package age, maintainer counts, download stats and scanning of package contents
  all need registry or tarball access and are out of scope.
- `lockfileVersion` 1 predates npm 7 and has no `hasInstallScript` flag, so
  install-script packages cannot be detected from it.
- Caret and tilde ranges are normal practice, not defects. They are reported at
  **low** severity so they stay out of the default report.
- Node built-in shadowing is a hint, not a verdict: `path`, `buffer` and `process`
  are all real, legitimate npm shim packages.
- `third-party-registry` fires for any host that is not `registry.npmjs.org` or
  `registry.yarnpkg.com` — expected noise if you run a private mirror. Suppress it
  with the **Ignore rule IDs** field.
- Each input field accepts up to 2,097,152 bytes, and a report is capped at 1,000
  findings.

## FAQ

<details>
<summary>Does this replace <code>npm audit</code>?</summary>

No — the two look at different things. `npm audit` matches your installed versions
against a vulnerability advisory database, which needs a network call. This tool
looks at structural risk in the files themselves: how loosely versions are
specified, whether code runs at install time, whether packages come from the
registry with verified hashes, and whether your manifest and lockfile still agree.
Run both.

</details>

<details>
<summary>Why is a wildcard version treated as high severity?</summary>

A spec of `*` or `latest` means the resolver takes whatever exists at install
time. If a maintainer account is compromised and a malicious release is published,
a wildcard spec pulls it on the very next install with no review and no upgrade
commit to notice. A pinned range plus a committed lockfile turns that into a
deliberate, reviewable change.

</details>

<details>
<summary>What extra checks do I get by pasting the lockfile too?</summary>

Two cross-checks that need both files. `unlocked-dependency` flags a package
declared in `package.json` that has no lockfile entry — the state where `npm ci`
refuses to install. `pin-mismatch` flags an exact pin such as `"chalk": "4.1.1"`
whose lockfile entry holds a different version, which usually means one of the two
files was hand-edited. The lockfile's own integrity, registry and install-script
rules run either way.

</details>

<details>
<summary>How do strictness, ignore and the fail threshold interact?</summary>

**Strictness** sets the severity floor for what gets reported: `lenient` keeps
high only, `standard` adds medium, `strict` adds low and info. **Ignore rule IDs**
then drops specific rules by name from whatever survived. **Fail the audit on**
reads only the findings that remain, so a rule you suppressed can never fail the
run. Set it to `Never` for a report-only pass.

</details>

<details>
<summary>Is my package.json uploaded anywhere?</summary>

No. The audit is compiled to WebAssembly and runs inside your browser tab. The
file contents never leave the page, which is why the tool deliberately does not do
anything that would require contacting a registry.

</details>
