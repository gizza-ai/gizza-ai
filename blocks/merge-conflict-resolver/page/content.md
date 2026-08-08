## About this tool

When a merge or a rebase stops, Git rewrites the conflicted file in place and leaves marker blocks behind: `<<<<<<<` opens the version already on your branch, `=======` separates it from the version being merged in, and `>>>>>>>` closes the block. This tool parses those blocks and rewrites each one with the side you choose, so you get back a clean file to paste over the conflicted one.

Paste the whole file, not just the conflict. Everything outside a marker block is preserved byte for byte, including indentation, the file's line endings (LF or CRLF) and whether it ended with a newline.

Alongside the two familiar sides, the parser also understands the three-way `diff3` and `zdiff3` conflict styles, where Git inserts a `|||||||` section holding the common ancestor — the text both branches started from. That section is reported and can be selected on its own, which is often the quickest way to see what each side actually changed.

Everything runs locally in your browser through WebAssembly. Nothing is uploaded, and the tool has no access to your repository — it reads text and returns text.

### Worked example

A file where two branches disagree on one line:

```text
const port = 1;
<<<<<<< HEAD
const host = "0.0.0.0";
=======
const host = "127.0.0.1";
>>>>>>> feature/local
start();
```

With **Keep** set to *Ours (current branch)*, the markers disappear and the current branch's line survives:

```text
const port = 1;
const host = "0.0.0.0";
start();
```

Switch **Keep** to *Theirs (incoming branch)* and the same paste returns `const host = "127.0.0.1";` instead. *Both, ours first* emits both lines in that order, which is what you want for conflicts in import lists, changelogs or test files where the two edits are additive rather than competing.

### Choosing per conflict

Real conflicts rarely all go the same way. Set **Output** to *Numbered conflict list* first to see what is in the file:

```text
2 conflicts · 2 resolved

[1] lines 2-6 · ours "HEAD" 1 line · theirs "topic" 1 line → ours
[2] lines 8-12 · ours "HEAD" 1 line · theirs "topic" 1 line → ours
```

Then use those numbers in **Per-conflict overrides**: `2=theirs` takes the incoming side for the second block only, `3-5=both` covers a range, `4-=keep` covers everything from the fourth block onwards, and `all=theirs` sets a new baseline that later entries can still override. Overrides are applied left to right, so a later entry wins over an earlier overlapping one.

*Side-by-side comparison* renders each conflict as two aligned text columns when you just want to read the difference, and *JSON report* returns the same inventory as data — line spans, branch labels, both sides' contents and the resolved text — for scripting.

### Limits and edge cases

- Input is capped at 1 MB. That is one conflicted file, not a whole repository dump; split larger pastes.
- `base` only works on input produced with the `diff3` or `zdiff3` conflict style. On an ordinary two-way conflict there is no `|||||||` section, and the tool reports which conflict lacked one rather than guessing.
- Marker lines are recognised as at least seven consecutive `<`, `|`, `=` or `>` characters followed by end-of-line or a space, which is what Git writes. A line such as `=========` used as a Markdown heading underline outside a conflict block is left alone as ordinary text.
- Malformed sequences are refused with the offending line number: an unterminated block, a nested `<<<<<<<`, a stray `>>>>>>>` or `|||||||`, or a block closed without a `=======`.
- Lines are compared and copied verbatim. The tool does no diffing inside a line, so two sides that differ only in whitespace are still two distinct sides.
- **Strict** makes a marker-free paste or a surviving `keep` block an error instead of output, which is useful when the tool is wired into a check.
- This is a text rewriter, not a merge algorithm. It cannot compute a merge from three file versions, run `git add`, or write anything back to disk.

## FAQ

<details>
<summary>Which side is "ours" and which is "theirs"?</summary>

`ours` is the first side, above `=======` — the version already on the branch you are on, usually labelled `HEAD`. `theirs` is the second side, below the separator, labelled with the branch or commit being merged in. During a **rebase** the labels are inverted relative to intuition: the commit being replayed is `theirs`, and `HEAD` is the upstream branch. The branch labels shown in the *Numbered conflict list* come straight from your paste, so check them there when in doubt.

</details>

<details>
<summary>Can I keep both sides?</summary>

Yes. *Both, ours first* emits the first side then the second with the markers removed; *Both, theirs first* reverses the order. No separator line is inserted between them, so the result stays syntactically valid for code. This is the usual answer for conflicts in import blocks, dependency lists and changelog entries, where both edits should survive.

</details>

<details>
<summary>What is the `|||||||` section in my conflict?</summary>

That is the common ancestor, added by the `diff3` and `zdiff3` conflict styles (`git config merge.conflictStyle diff3`). It shows the text both branches started from, which makes it obvious which side actually changed what. Set **Keep** to *Common ancestor (diff3)* to return it. Files produced with the default `merge` style have no such section, and selecting it then reports an error naming the conflict.

</details>

<details>
<summary>How do I resolve only some of the conflicts?</summary>

Set **Keep** to *Keep the markers* so nothing is resolved by default, then list the ones you have decided in **Per-conflict overrides**, for example `1=ours, 4-6=theirs`. The blocks you did not name keep their markers, so you can paste the result back, finish them in your editor, and run the file through again.

</details>

<details>
<summary>Will it corrupt a file that has no conflicts?</summary>

No. Text with no conflict markers is returned unchanged, and the *Numbered conflict list* view reports zero conflicts. Only complete, well-formed marker blocks are rewritten; anything else is either passed through as text or refused with the line number. Turn on **Strict** if you would rather a marker-free paste be treated as an error.

</details>

<details>
<summary>Does my code get uploaded anywhere?</summary>

No. The parser is compiled to WebAssembly and runs inside your browser tab, so the pasted text never leaves your machine. The same code backs the command-line version, which is equally offline.

</details>
