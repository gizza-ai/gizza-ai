## About this tool

A reverse patch is the text you would apply to undo a change. Paste the output of `git diff`, `git show`, `git format-patch`, `diff -u`, or a bare unified hunk, and this tool emits the inverse patch: old and new hunk ranges swap, `+` lines become `-`, `-` lines become `+`, and git metadata is flipped so the result behaves like a revert.

Worked example — given this patch:

```diff
diff --git a/src/main.rs b/src/main.rs
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,3 +1,4 @@
 fn main() {
+    println!("hello");
 }
```

the default output is:

```diff
diff --git a/src/main.rs b/src/main.rs
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,4 +1,3 @@
 fn main() {
-    println!("hello");
 }
```

Save that output and apply it with your usual patch tool to remove the line. Nothing is applied here; the tool only rewrites the patch text.

### Git headers it understands

Real diffs are more than `@@` hunks. The reverse operation also handles the metadata that makes a patch apply cleanly:

- `index old..new` blob hashes swap by default. Choose **Drop index lines** when the hashes are stale or the target repository may not have the old blobs.
- `new file mode` becomes `deleted file mode`, and `old mode` / `new mode` pairs swap.
- `rename from` / `rename to` and `copy from` / `copy to` swap.
- `---` / `+++` paths and the two sides of `diff --git` swap when **Swap file paths** is on, which is the default.
- `\ No newline at end of file` markers stay attached to the line they describe after the line changes side.

### Output modes

- **Reverse patch** returns the inverted patch text.
- **Summary report** lists the files, hunk counts, swapped plus/minus totals, and each reversed hunk header next to the original one.
- **JSON report + patch** returns the same report as structured data with the reverse patch under `patch`.

Use **File filter** for a multi-file patch: full path, suffix on a directory boundary, or bare filename all match. Leave it blank to reverse every file section.

### Limits and edge cases

- Input is capped at 1 MB.
- Binary patches cannot be inverted from the forward delta alone. The default is to fail and name the binary file; **Skip** drops those sections, and **Keep** passes them through unchanged with a warning.
- Combined merge diffs with `@@@` headers are rejected. Diff against a single parent first.
- This tool does not apply patches, search your working tree, resolve conflicts, or do fuzzy matching. Use an apply-patch tool for reverse-apply and conflict handling.
- CRLF line endings, final-newline state, mail-style preambles, and bare `@@` hunks are preserved where possible.

## FAQ

<details>
<summary>Is this the same as `git apply -R`?</summary>

No. `git apply -R` applies a forward patch backwards to files in a working tree. This tool emits a new reverse patch as text. You can inspect, save, edit, or apply that patch yourself.

</details>

<details>
<summary>Why do index lines have swap, keep, and drop modes?</summary>

`git diff -R` swaps `index old..new` to `index new..old`, which is the default here. Keeping is useful for tools that treat those lines as annotations only. Dropping is safest when the patch is detached from the repository that produced the blob hashes.

</details>

<details>
<summary>Can it revert a binary patch?</summary>

Not from the forward patch alone. A `GIT binary patch` contains a delta in one direction, not enough information to synthesize the opposite delta. The tool fails by default so you do not mistake an incomplete reverse patch for a full revert.

</details>

<details>
<summary>What if I only want one file from a multi-file diff?</summary>

Put a path in **File filter**. It matches an exact path, a suffix such as `src/main.rs`, or a basename such as `main.rs`. The output contains only matching file sections.

</details>

<details>
<summary>Will this fix a patch that no longer applies?</summary>

No. Inversion is byte-exact; it does not search a working tree or tolerate drifted context. If you need fuzzy matching, conflict reports, or reverse-applying to pasted source text, use an apply-patch tool after generating or selecting the right patch.

</details>
