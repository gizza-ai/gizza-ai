## About this tool

A unified diff describes a change; this tool performs it. Paste the file as it is now and the
patch from `git diff`, `git show`, `git format-patch`, `diff -u`, or `svn diff`, and get the
patched text back — no repository, no working tree, no upload. It is the other half of a diff
viewer: instead of showing what a patch would do, it does it and tells you where every hunk
landed.

Worked example — this file:

```rust
fn main() {
    let x = 1;
    println!("{x}");
}
```

with this patch:

```diff
--- a/src/main.rs
+++ b/src/main.rs
@@ -1,4 +1,5 @@
 fn main() {
     let x = 1;
+    let y = 2;
     println!("{x}");
 }
```

produces:

```rust
fn main() {
    let x = 1;
    let y = 2;
    println!("{x}");
}
```

Tick **Reverse** and paste the patched file instead to get the original back — the same patch, run
backwards, the way `git apply -R` reverts a change.

### When the file has moved on

A patch rarely matches its file byte for byte forever. Three controls handle the usual drift:

- **Offset search** is automatic. If a hunk's context sits ten lines below where its `@@` header
  claims, it is found there and the report says `offset +10`.
- **Fuzz** (0–3, default 2, the same factor `patch -F` uses) lets a hunk drop that many *context*
  lines from each end when the exact context no longer matches. It never drops a `+` or `-` line
  and never strips a hunk down to nothing, so it cannot turn a genuine conflict into a blind
  insertion.
- **Ignore whitespace** compares context and deleted lines with whitespace runs collapsed — the fix
  for a file that was reindented from tabs to spaces since the patch was cut. Matching only: the
  output keeps your file's own indentation, byte for byte.

### Conflicts

Set **Output** to `Dry-run report` for a `git apply --check`-style pass that changes nothing:

```text
1 of 2 hunks applied · list.txt

 [1] @@ -1,2 +1,3 @@  applied at line 1
 [2] @@ -6,2 +7,2 @@  FAILED — expected `NOPE` at line 6, found `foxtrot`

1 hunk failed. Set output=rejects for those hunks as a patch, or on_conflict=skip to apply the rest.
```

**On conflict** defaults to `fail`, so you never get a half-applied file by accident — the error
names the hunk, the line it expected and the line it actually found. Switch it to `skip` to apply
every hunk that does match, then set **Output** to `Rejected hunks` to get the leftovers as a
standalone patch you can fix by hand. `JSON result` returns the same information as data: a status,
landing line, offset and fuzz per hunk, plus the patched text.

### Multi-file patches

This tool patches one pasted file. If the diff touches several paths, put one of them in **File in
a multi-file patch** — a full path, a bare filename, or any substring. With no filter, a multi-file
patch lists the paths it found instead of guessing which one your text is. To split a large patch
first, use a diff hunk selector; to produce a diff from two texts, use a text diff tool.

### Limits and edge cases

- 1 MB per input, for the file and for the patch.
- Hunks apply in order and cannot overlap; a hunk never matches inside a region an earlier hunk
  already consumed.
- Line endings are preserved: a CRLF file stays CRLF, and the file's final-newline state is kept
  unless a `\ No newline at end of file` marker changes it.
- A wrong line count in an `@@` header is recounted from the hunk body, so a mail-mangled patch
  still applies.
- Binary, rename-only, and mode-only entries have no text hunks and are reported as such rather
  than silently skipped. Combined merge diffs with `@@@` headers are rejected — diff against a
  single parent instead.
- Nothing is uploaded: the patch runs in your browser via WebAssembly.

## FAQ

<details>
<summary>Does this change any file on my computer or in my repository?</summary>

No. It reads the two texts you paste and returns a third one. Copy the result back into your editor
yourself — which is also why a bad patch here costs you nothing.

</details>

<details>
<summary>What is the difference between fuzz and offset?</summary>

Offset means the hunk matched perfectly, just at a different line number than its header claimed —
that is always allowed and always reported. Fuzz means the context did *not* match perfectly, so
the tool ignored up to that many context lines at each end of the hunk to place it. A hunk applied
with fuzz deserves a second look; a hunk applied with only an offset does not.

</details>

<details>
<summary>Why did my patch fail when `git apply` accepts it?</summary>

The most common causes are a source file that has drifted (raise fuzz or turn on ignore
whitespace), indentation changed between tabs and spaces (ignore whitespace), or a multi-file patch
where the pasted text is a different file than the hunks target (set the file filter). The failure
message names the line the hunk expected and the line found in its place, which usually identifies
which of the three it is.

</details>

<details>
<summary>Can I recover the original file from a patched one?</summary>

Yes — paste the patched file, paste the same diff, and tick **Reverse**. The `+` and `-` sides swap
roles, so the change is undone. This is the same operation as `git apply -R` or `patch -R`, and it
is how you revert a commit's diff without the commit.

</details>

<details>
<summary>What does the rejected-hunks output give me?</summary>

The hunks that did not apply, re-emitted as a valid unified diff with file headers and recounted
`@@` ranges — the equivalent of the `.rej` file `patch --reject` writes. Fix the context by hand,
then run it through this tool again.

</details>

<details>
<summary>Does it handle patches with no `---`/`+++` headers?</summary>

Yes. Bare `@@` hunks pasted out of a code review or a chat message are accepted; the file is simply
labelled as having no file header in the report. `diff --git`, `index`, and mail-style preambles are
also understood and ignored where they carry no hunks.

</details>
