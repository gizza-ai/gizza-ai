## About this tool

A large patch is easier to review when you can separate the unrelated pieces. Paste a unified diff from `git diff`, `git show`, `git format-patch`, or `diff -u`; this tool numbers every hunk globally, shows the file and `@@` header for each one, then exports only the hunks you select.

Worked example — given this diff:

```diff
diff --git a/src/main.rs b/src/main.rs
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,2 +1,3 @@
 fn main() {
+    println!("hello");
 }
@@ -10,2 +11,2 @@
-old_call();
+new_call();
```

The default list output is:

```text
1 file · 2 hunks · +2 −1

src/main.rs · 2 hunks · +2 −1
 [1] @@ -1,2 +1,3 @@  +1 −0
 [2] @@ -10,2 +11,2 @@  +1 −1
```

Set **Output** to `patch` and **Hunks** to `2` to export just the second change with the file headers preserved. Use `1,3-5`, `4-`, or `-2` for ranges, and tick **Invert hunk selection** to drop the listed hunks instead of keeping them.

### Selection filters

- **Hunks** selects by the global 1-based numbers shown in the inventory.
- **File globs** narrows by path. `src/*.rs` includes matching Rust files; `!*.lock` excludes lock files; an exclude wins over an include.
- **Original-file line spans** keeps hunks touching old-side line numbers, such as `40-120` or `200-`.
- **Renumber kept hunk headers** is on by default. If you drop an earlier hunk that added or removed lines, later kept hunks have their new-side start adjusted so the smaller patch still applies to the original file.

### Output modes

- `list` prints a hunk inventory with file totals and plus/minus counts.
- `patch` writes one smaller unified diff containing the selected hunks.
- `split` writes labelled sections, one complete patch per selected hunk.
- `json` returns the inventory and current selection as machine-readable data.

### Limits and edge cases

- Input is capped at 1 MB.
- Binary-only, rename-only, and mode-only file entries can be listed, but they have no textual `@@` hunks to export.
- Combined merge diffs with `@@@` headers are rejected; make a diff against one parent first.
- The tool does not run `git apply` and cannot read your working tree. It only transforms the pasted patch text.
- It preserves `\ No newline at end of file` marker lines inside selected hunks.

## FAQ

<details>
<summary>Does this apply the patch to my repository?</summary>

No. It only returns text. Copy the `patch` output to a file or pipe it into your own `git apply` command after reviewing it.

</details>

<details>
<summary>Why does the default output list hunks instead of returning a patch?</summary>

The first step in hunk picking is usually discovery: you need the numbers before you can select them. The list output is the inventory you paste into the hunk selector, similar to running a listing step before filtering a diff.

</details>

<details>
<summary>Can I select individual lines inside one hunk?</summary>

No. This version selects whole hunks. Per-line editing requires an interactive editor that can split or rewrite the hunk safely; this one-shot tool deliberately avoids creating hand-edited, possibly inapplicable patches.

</details>

<details>
<summary>What does renumbering do?</summary>

If an earlier dropped hunk added two lines, later kept hunks in the same file need their new-side `@@` start shifted back by two. With renumbering on, the emitted smaller patch is adjusted for that net delta. Turn it off only when you plan to apply the dropped hunks separately first.

</details>

<details>
<summary>How do file globs work?</summary>

Use comma-separated patterns. `src/*.rs` includes paths matching that pattern, a pattern without a slash also matches a basename, and `!` excludes. For example, `*, !*.lock` keeps every selected hunk except lock-file hunks.

</details>
