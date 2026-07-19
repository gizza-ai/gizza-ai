## About this tool

Paste a list of filenames and preview the exact rename mapping before you run a shell script or desktop batch rename. The output is plain `old -> new` lines, with a collision warning when multiple inputs would produce the same target name.

The tool supports four deterministic rules:

- Find and replace on the filename stem.
- Regex replacement with capture groups such as `$1`.
- Sequential numbering with `{n}`, `{name}`, and `{ext}` tokens.
- Case conversion to lower, upper, title, snake, kebab, camel, or pascal case.

Example:

```text
IMG_001.JPG
IMG_002.JPG
```

With find `IMG` and replacement `photo`, the mapping becomes:

```text
IMG_001.JPG -> photo_001.JPG
IMG_002.JPG -> photo_002.JPG
```

Limits and edge cases:

- This is a safe preview engine: it never renames local files, uploads a ZIP, or writes an archive.
- Extension preservation is on by default, so transforms apply to the stem and keep `.jpg`, `.pdf`, and similar suffixes.
- `archive.tar.gz` preserves only the final `.gz` extension; `.gitignore` is treated as a dotfile with no extension.
- Padding is capped at 20 digits, and blank filename lines are ignored.

## FAQ

<details>
<summary>Does this actually rename my files?</summary>

No. It only computes a mapping so you can review the result safely. Use the mapping in your file manager or automation script after checking for mistakes and collisions.

</details>

<details>
<summary>How do regex replacements work?</summary>

Choose regex mode, put the regular expression in the find field, and use capture references such as `$1` in the replacement field. Invalid regular expressions produce an error instead of a partial mapping.

</details>

<details>
<summary>How do I make photo_001.jpg, photo_002.jpg, and so on?</summary>

Choose sequential numbering, set the pattern to `photo_{n}`, start at `1`, set padding to `3`, and leave extension preservation enabled.

</details>

<details>
<summary>What happens if two files map to the same new name?</summary>

The output still shows every mapping, then adds a collision warning so you can adjust the rule before applying it elsewhere.

</details>
