## About this tool

Glob Filter lets you paste a list of paths and preview which ones match include and exclude patterns. It is useful for designing `.gitignore`, Docker ignore, CI include/exclude, packaging, and code-search file filters before applying them to a real repository.

Use **gitignore-style** syntax when you want familiar `.gitignore` behavior: patterns without `/` match at any depth, `/` anchors to the root, directory patterns cover everything below them, blank lines and `#` comments are ignored, and `!` can re-include a later exception. Use **whole-path glob** syntax when each pattern should match the entire path and `**/` is required for any-depth matches.

## Worked example

Paths:

```text
src/main.rs
src/lib.rs
tests/app.test.ts
target/debug/app
README.md
```

Include patterns:

```text
*.rs
```

Exclude patterns:

```text
target/
```

With **Pattern syntax = gitignore-style** and **Output = Matched**, the result is:

```text
src/main.rs
src/lib.rs
```

## Syntax notes

- `*` matches characters within one path segment.
- `**` spans directories; in whole-path glob mode, use `**/*.rs` for Rust files at any depth.
- `?` matches one non-slash character.
- `[abc]`, `[a-z]`, and `[!0-9]` character classes are supported.
- `{png,jpg,gif}` brace alternatives are supported.
- `!pattern` negates a later pattern line in the same include or exclude list; the last matching line wins.

## Limits & edge cases

- This tool filters a pasted path list; it does not walk your filesystem or upload folders.
- Matching uses `/` as the path separator. Convert Windows backslashes first if needed.
- In gitignore-style mode, a pattern like `*.rs` matches at any depth. In whole-path glob mode, `*.rs` only matches top-level paths.
- Exclude patterns run after include patterns; a path is kept when it is included and not excluded.
- Case-sensitive matching is on by default. Turn it off for case-insensitive filesystems or mixed-case path lists.

## FAQ

<details>
<summary>What is the difference between glob and gitignore-style syntax?</summary>

Whole-path glob treats every pattern as matching the complete path, so `*.rs` matches `main.rs` but not `src/main.rs`; use `**/*.rs` for any depth. Gitignore-style syntax follows `.gitignore` conventions, so `*.rs` matches Rust files anywhere in the path list.

</details>

<details>
<summary>How do include and exclude patterns combine?</summary>

A path is kept if it matches the include list (or the include list is empty) and it does not match the exclude list. Exclude patterns therefore remove paths after the include step. Within each pattern list, later lines override earlier ones when you use `!` negation.

</details>

<details>
<summary>Can I test a real folder directly?</summary>

No. The tool is text-in/text-out and browser-local: paste paths from commands such as `git ls-files`, `find`, or a build log. That keeps the page portable and avoids filesystem permissions or uploads.

</details>

<details>
<summary>Does it support comments and negation like .gitignore?</summary>

Yes in gitignore-style mode. Blank lines and `#` comments are ignored, and a leading `!` re-includes paths in the same include or exclude list. Escape a literal leading `#` or `!` with a backslash.

</details>
