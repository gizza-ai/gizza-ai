## About this tool

`release-from-commits` turns a pasted Conventional Commits log into two things release managers usually need together: the next semantic version and grouped release notes. It understands `feat`, `fix`, `perf`, `revert`, `type(scope): subject`, `type!:` breaking markers, and `BREAKING CHANGE:` footers.

Use it when you already have the relevant log range, for example:

```text
feat(api): add token refresh
fix(ui): keep menu open
perf(cache)!: replace storage layer
```

With `current_version = v1.4.2`, that log produces a major release heading for `v2.0.0`, calls out the breaking cache change, and groups the feature and fix under readable sections. Set `output_format = version` when a CI step only needs the computed tag, or `json` when another tool will consume the result.

## Limits and edge cases

- The tool does not read git repositories. Paste the commit range you want to release, such as `git log --oneline v1.4.2..HEAD`.
- Logs are capped at 1 MiB and 5,000 parsed commits so browser runs stay responsive.
- `repo_url` only adds links; it never contacts the repository.
- Hidden types are omitted from markdown notes, but breaking changes from hidden types are still shown.
- `0.x` releases can use `zero_version_policy = cautious` if your project treats breaking changes as minor and features as patch before 1.0.

## FAQ

<details>
<summary>Does this tool run git or inspect my repository?</summary>

No. It is a pure text tool. Paste the exact commit log you want analysed; the tool does not clone, fetch, tag, commit, or publish anything.

</details>

<details>
<summary>How are breaking changes detected?</summary>

A commit is breaking when its header includes a bang marker, such as `feat(api)!: change auth`, or when its body/footer contains `BREAKING CHANGE:`, `BREAKING CHANGES:`, or `BREAKING-CHANGE:`. Breaking entries are listed before the grouped notes.

</details>

<details>
<summary>What happens when no commit triggers a release?</summary>

The version stays unchanged and markdown output starts with `No release required`. Use `patch_types = *` if your workflow wants any non-breaking commit to create at least a patch release.

</details>

<details>
<summary>Can I use it for pre-release tags?</summary>

Yes. `prerelease_policy = finalize` removes a matching prerelease suffix when the bump is ready, `increment` advances or starts an `rc.0` style prerelease, and `ignore` drops prerelease metadata before applying a stable bump.

</details>
