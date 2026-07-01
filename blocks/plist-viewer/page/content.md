## About this tool

Paste an Apple property list and this tool renders it in a form that is easier to inspect, diff, and copy into scripts. It accepts XML plist text directly, and it also accepts Base64-encoded binary plist (`bplist00`) bytes for cases where the original file is not text.

Choose JSON when you want machine-readable output, or `tree` for a compact `plutil -p`-style view. Dictionary key order is preserved by default; turn on sorting when you want stable diffs.

Everything runs locally in your browser. The plist contents are not uploaded.

## FAQ

<details>
<summary>Can I view a binary .plist, or only XML?</summary>

Both. XML plist text is pasted directly, and a **binary** `bplist00` file is
supported by Base64-encoding its bytes and pasting that — the parser
auto-detects XML vs binary from the magic bytes. So a file that isn't text
still works once you `base64` it.

</details>

<details>
<summary>How are &lt;data&gt; blobs and &lt;date&gt; values shown?</summary>

`<data>` byte blobs are rendered as **Base64** by default; switch the data
encoding to **hex** if you'd rather see the raw bytes. `<date>` values are
emitted in ISO-8601 / XML date format, so the output stays readable and
diff-friendly in both JSON and tree modes.

</details>

<details>
<summary>Can I keep dictionary keys in their original order?</summary>

Yes — key order is **preserved by default**, matching the plist as written.
Turn on **Sort keys** only when you want alphabetical order for stable diffs.
JSON indentation is configurable from 0 to 8 spaces (default 2).

</details>

<details>
<summary>Is my plist uploaded anywhere?</summary>

No. Parsing and rendering happen entirely in your browser via WebAssembly, so
the plist contents — including anything sensitive in a preferences or
configuration file — never leave your device.

</details>
