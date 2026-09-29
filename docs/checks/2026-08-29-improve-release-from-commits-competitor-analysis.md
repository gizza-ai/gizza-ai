# release-from-commits — competitor analysis (2026-08-29)

Scan run BEFORE implementing, per `/create-next-tool` step 4. One web search
("conventional commits next semver version calculator release notes generator tool")
plus direct reads of the three most relevant real tools. Everything below is a
**paraphrase** of publicly documented behaviour — no competitor copy, branding or
trademarks were reused, and no out-of-model feature was built.

## Competitors reviewed

### 1. semantic-release / release-notes-generator (conventional-changelog)
The de-facto notes generator in the JS release pipeline. Documented options:
`preset` (default `angular`), `config`, `parserOpts`, `writerOpts`, `presetConfig`,
`host`, `linkCompare` (default true), `linkReferences` (default true), `commit`
(link keyword, `commit`/`commits` on Bitbucket), `issue` (`issues`/`issue`).
Commit types map to changelog sections through the preset; the angular preset only
surfaces a handful of types (features, fixes, performance, reverts) and hides the
rest, and sorts entries by scope then subject. Breaking changes are pulled out of
commit bodies by keyword (`BREAKING CHANGE`, `BREAKING CHANGES`, `BREAKING`) and
rendered as their own highlighted section.

### 2. ietf-tools/semver-action
A GitHub Action that computes only the next version from conventional commits.
Documented inputs: `token`, `branch` (default `main`), `majorList` (empty),
`minorList` (default `feat, feature`), `patchList` (default
`fix, bugfix, perf, refactor, test, tests`), `patchAll` (default false),
`scopeList`, `prefix`, `additionalCommits`, `fromTag`, `maxTagsToFetch` (10),
`fallbackTag`, `skipInvalidTags`, `noNewCommitBehavior` (`error`),
`noVersionBumpBehavior` (`error`), `tagFilter`. A `BREAKING CHANGE` note or the
`!` marker forces a major bump regardless of the type lists. Prereleases are not
addressed in its docs.

### 3. git-cliff
Changelog generator with configurable commit groups, regex commit parsers and a
version-bump calculator. Documented bump options: `features_always_bump_minor`
(default true), `breaking_always_bump_major` (default true), `initial_tag`
(`0.1.0`), `custom_major_increment_regex`, `custom_minor_increment_regex`,
`bump_type`. When the two `always_bump` flags are false and the major version is
0, a feature bumps the patch and a breaking change bumps the minor — the
conservative pre-1.0 convention.

(Also noted from the search, not read in depth: `conventional_commits_next_version`,
`python-semantic-release`, `git-mkver`, `git-semver`. They restate the same
table stakes: type→bump lists, `!`/footer breaking detection, changelog grouping.)

## Table stakes → decision

| Table stake (seen at ≥1 competitor) | Fit | Where it landed |
| --- | --- | --- |
| Type→bump lists (`minorList`/`patchList`) | in-model | `minor_types` (default `feat,feature`), `patch_types` (default `fix,perf,revert`) |
| `patchAll` — every commit bumps a patch | in-model | `patch_types = *` wildcard (no extra field) |
| `!` marker forces major | in-model | parsed from the header |
| `BREAKING CHANGE:` / `BREAKING-CHANGE:` footer forces major | in-model | parsed from the body/footer, description captured |
| Pre-1.0 conservative bumping (`features_always_bump_minor` etc.) | in-model | `zero_version_policy` = `standard` \| `cautious` |
| Prerelease/pre-major handling | in-model | `prerelease_policy` = `finalize` \| `increment` \| `ignore` + `prerelease_identifier` |
| Grouped sections per commit type | in-model | fixed, ordered group map (Features, Bug Fixes, Performance, …, Other Changes) |
| Hiding noisy types from the notes | in-model | `hidden_types` (default `chore,style,ci,build,test`) |
| Breaking changes as their own section | in-model | always rendered first, even for hidden types |
| Scope shown per entry, entries sorted by scope | in-model | `**scope:** subject`, entries sorted by scope then subject |
| `linkReferences` — link `#123` issue refs | in-model | enabled when `repo_url` is set |
| Commit-hash links | in-model | short hash parsed off `git log --oneline`, linked when `repo_url` is set |
| `linkCompare` — compare link between tags | in-model | trailing compare link when `repo_url` is set |
| Dated release heading | in-model | `release_date` (native date picker on the page) |
| Version-only output for CI piping | in-model | `output_format = version` |
| Machine-readable result | in-model | `output_format = json` |
| Tag prefixes, incl. monorepo (`prefix`) | in-model | inferred from and preserved on the input version (`v1.2.3`, `web-v1.2.3`) |
| `noVersionBumpBehavior` — say when nothing releases | in-model | explicit "no release required" notes + unchanged version |
| Reading the log from git itself (`branch`, `fromTag`, `maxTagsToFetch`, `tagFilter`) | **out-of-model** | listed, not built — this repo's tools are browser-local/no-server and have no git access; the log is pasted |
| Tagging/publishing/committing the changelog | **out-of-model** | listed, not built — no repo write access |
| Custom preset packages / `writerOpts` templates (`parserOpts`, `config`, `presetConfig`) | **out-of-model** | listed, not built — they load arbitrary JS modules |
| `scopeList` monorepo commit filtering | **considered, rejected** | one more list field for a narrow case; the same result is achieved by pasting `git log -- <path>` |
| `majorList` (types that always mean major) | **considered, rejected** | the spec's own major signals (`!`, `BREAKING CHANGE:`) are supported; a third type-list is schema bloat for a non-standard convention |
| `custom_*_increment_regex` | **considered, rejected** | free-form regex in a version calculator is a foot-gun; the type lists cover the same intent readably |

## Naming note

The build brief suggested `premajor_policy`. The shipped param is
`prerelease_policy` because it governs every pre-release version
(`1.0.0-rc.1`, `1.3.0-beta.2`, …), not only pre-major ones; `premajor` is one
npm release type among `premajor`/`preminor`/`prepatch`/`prerelease`. Same
capability, accurate name.

## UX controls adopted

- Preset chips (`[[example]]`) for the four realistic shapes: a feature+fix log, a
  breaking-change log, a pre-1.0 log, and a release-candidate finalisation.
- `[input.labels]` friendly labels on all three enum selects.
- `kind = "date"` native picker for the release date.
- `kind = "tag-list"` pills for the three type-list fields.
- `multiline = true` textarea for the pasted log so newlines survive.
- Real placeholders everywhere, and a worked input→output example in the page copy.
