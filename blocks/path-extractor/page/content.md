## About this tool

Use Path Extractor when you have noisy text — a compiler log, a Python traceback, `git status`, CI output, or a pasted chat message — and you only want the file paths. It recognizes common POSIX paths, Windows drive paths, UNC shares, quoted paths with spaces, and source locators such as `src/main.rs:42:9` or `src\main.c(12,4)`.

Worked example: paste this log:

```text
error[E0308]: mismatched types
  --> src/main.rs:42:9
warning: unused import in src/main.rs
   Compiling foo (/home/dev/projects/foo)
```

With the defaults, the output is:

```text
src/main.rs
/home/dev/projects/foo
```

Turn on “Keep :line and :column suffixes” when you want `src/main.rs:42:9` in the list. Change “Return” to `Filename only` for `main.rs`, or to `Directory only` for `src`. Use the extension filter to keep just files like `rs, toml, md`, and switch the output format to CSV or JSON when you need occurrence counts or line/column metadata.

Limits and edge cases: the scanner is shape-based and never checks whether a file exists on disk. URLs, dates, numeric ratios, and ordinary prose are intentionally ignored. Bare filenames such as `main.rs` are matched only when “Require / or \ in each match” is turned off, because that mode is higher-recall and more ambiguous. Input is capped at about 1 MB and 20,000 path occurrences.

## FAQ

<details>
<summary>Does this check whether the files actually exist?</summary>

No. The tool runs locally in a browser or sandboxed WebAssembly block and never stats the filesystem. It extracts strings that look like paths, then filters, deduplicates, sorts, and formats them.

</details>

<details>
<summary>Why are line numbers stripped by default?</summary>

Most users want a reusable file list, so `src/main.rs:42:9` becomes `src/main.rs` by default. Enable “Keep :line and :column suffixes” when you are pasting output into an editor, quickfix list, or another tool that understands locators.

</details>

<details>
<summary>How do I extract only filenames or directories?</summary>

Set “Return” to `Filename only` to turn `src/app/main.rs` into `main.rs`, or to `Directory only` to return `src/app`. Deduplication happens after this projection, so repeated filenames or directories are counted together.

</details>

<details>
<summary>Why did it miss a bare filename like main.rs?</summary>

The default requires a slash or backslash because bare words with dots are easy to confuse with versions, decimals, prose, or domain-like text. Turn off “Require / or \ in each match” to opt into bare extension-bearing filenames such as `main.rs` and `Cargo.toml`.

</details>
