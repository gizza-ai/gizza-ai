## Run sed-style edits without a shell

Stream Editor applies a small sed/ed-style command script to pasted text entirely in the browser. It uses sed's familiar cycle model: each input line becomes the pattern space, matching commands run in order, and the line is printed automatically unless **Quiet mode** is enabled.

Common scripts work as expected:

- `s/foo/bar/g` — substitute every `foo` with `bar` on each line.
- `/^$/d` — delete blank lines.
- `2,5d` — delete lines 2 through 5.
- `/error/p` with **Quiet mode** — print only matching lines.
- `1i header`, `$a footer`, and `/old/c replacement` — insert, append, or change text.

Filesystem and shell sed commands are deliberately unavailable in the sandbox. Commands such as reading a file, writing a file, or executing a shell command return a clear error instead of reaching outside the pasted text.

### Worked example

Input:

```text
foo

keep foo
drop this
```

Script:

```sed
s/foo/bar/g
/drop/d
/^[[:space:]]*$/d
```

Output:

```text
bar
keep bar
```

The first command replaces `foo`, the second deletes any line containing `drop`, and the third removes blank lines.

### Limits and edge cases

- Output is capped by **Max output lines** (default 100,000) to stop accidental explosions.
- A separate step cap stops runaway branch loops.
- **Basic** regex mode follows sed/BRE-style escaping. Choose **Extended** for `sed -E` style groups and alternation.
- **Whole buffer** mode loads the whole input into one pattern space for multi-line edits; most line-by-line scripts should leave it off.
- CRLF output is available for Windows-oriented text, but input line endings are normalized before editing.

## FAQ

<details>
<summary>Is this a full GNU sed clone?</summary>

No. It implements the common stream-editing core for pasted text: addresses, ranges, substitutions, delete, print, insert/append/change, transliteration, hold space, labels and branches. Commands that require a filesystem or shell are blocked because this tool runs in a sandboxed browser/runtime model.

</details>

<details>
<summary>When should I turn on Quiet mode?</summary>

Use Quiet mode for `sed -n` style extraction. With quiet off, every line that survives the script is printed automatically. With quiet on, output appears only when the script uses commands such as `p`, `P`, `=`, or `l`.

</details>

<details>
<summary>What is the difference between basic and extended regex?</summary>

Basic mode matches traditional sed regular expressions where grouping and alternation are escaped, such as `\(foo\|bar\)`. Extended mode matches `sed -E` style patterns like `(foo|bar)` and `ID-([0-9]+)`.

</details>

<details>
<summary>Can it edit files directly?</summary>

No. Paste text in and copy or download the result. Direct file reads/writes and shell execution are intentionally unavailable so the script cannot access anything outside the text you supplied.

</details>
