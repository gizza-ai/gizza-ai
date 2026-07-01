## About this tool

**JSON Merge** deep-merges two or more JSON documents into one. Paste your JSON
values one after another (separated by whitespace or a newline) — they're merged
**left to right**.

The merge rules:

- **Objects** are merged **recursively** — nested objects combine key by key.
- **Conflicts** (a key set to different scalar/types) are resolved
  **last-wins**: the later document's value replaces the earlier one.
- **Arrays** are **replaced** by the later value by default; tick **Concatenate
  arrays** to append them instead.

Set **Indent** for the output (or 0 to minify). Everything runs **locally in your
browser** via WebAssembly — your data is never uploaded.

### Handy for

- Layering a base config with environment-specific overrides.
- Combining partial JSON responses or fixtures.
- Applying a "patch" object on top of a default.

## FAQ

<details>
<summary>Can I delete a key by setting it to null in a later document?</summary>

No — this is a deep merge, not JSON Merge Patch (RFC 7386). A `null` in a
later document is treated as an ordinary value, so the key stays in the output
with the value `null` rather than being removed.

</details>

<details>
<summary>How do I separate the documents I paste in?</summary>

Just put whitespace or a newline between them — the input is read as a stream
of JSON values, merged left to right. Two, three, or more documents all work;
pasting a single document simply reformats it at your chosen indent.

</details>

<details>
<summary>Are arrays merged element by element?</summary>

No. When two arrays collide, the later one **replaces** the earlier by
default, or is **appended** to it if you tick *Concatenate arrays*. There is
no index-wise merging of array elements — only objects merge recursively.

</details>

<details>
<summary>Can I get minified output?</summary>

Yes — set **Indent** to `0` to emit the merged document on a single line.
Values 1–8 pretty-print with that many spaces per level (the default is 2).

</details>
