## Compare Base64 by payload, not by spelling

Base64 strings are wrappers around bytes. Padding, line wrapping, data-URI prefixes and the
standard (`+` and `/`) versus URL-safe (`-` and `_`) alphabets can make two encodings look
unrelated even when they decode to the same payload. This tool decodes both sides first, then
compares the underlying bytes.

Use it when a token, attachment, signed blob or encoded API field changed and you need to know
whether the payload changed too. The report names the first differing offset, total differing
bytes, size delta, SHA-256 for each side and compact byte ranges with both hex and printable text.
For text payloads, switch to **Unified text diff**; for binary payloads, use the side-by-side
hexdump.

Everything runs locally in your browser through WebAssembly. The Base64 strings are never
uploaded anywhere.

## Worked example

Compare `SGVsbG8gd29ybGQh` (`Hello world!`) with `SGVsbG8gV29ybGQh` (`Hello World!`) and choose
**Readable summary**:

```text
Payloads differ: both 12 bytes. First difference at offset 0x0006 (6).
1 byte differs across 1 range.
@ 0x0006 (1 byte) changed: 77 |w| -> 57 |W|
```

The encoded strings differ in several characters, but the decoded payload differs by exactly one
byte: lowercase `w` (`0x77`) became uppercase `W` (`0x57`).

## Useful output modes

* **Full JSON report** — best for automation: decoded sizes, detected payload type, SHA-256 hashes,
  alphabet/padding notes and machine-readable diff ranges.
* **Readable summary** — a short verdict plus one line per changed/added/removed byte range.
* **Side-by-side hex dump** — classic hex + ASCII rows with changed rows marked by `*`.
* **Unified text diff** — decodes both payloads as UTF-8 and shows line-level `+` / `-` changes.

## Limits and edge cases

* Each input is capped at **4 MiB of Base64 text** (roughly 3 MiB decoded).
* Lenient mode repairs missing padding, ignores whitespace/line wrapping and strips a leading
  `data:...;base64,` prefix. Turn on **Strict RFC 4648** to reject those instead.
* `alphabet=auto` detects the alphabet per side and rejects a single side that mixes standard and
  URL-safe characters.
* `align=shift` trims the common prefix and suffix so one inserted byte is reported as an
  insertion; `align=offset` compares byte `i` with byte `i`, which is clearer for fixed-layout data.
* Very large text diffs fall back to a positional comparison rather than building an expensive
  line-matching table.

## FAQ

<details>
<summary>Why not just compare the Base64 strings directly?</summary>

String comparison reports encoding noise: padding, wrapped lines, URL-safe alphabets and data-URI
prefixes all change the text without changing the bytes. This tool normalises by decoding first, so
the result answers the payload question.

</details>

<details>
<summary>Can it compare Base64url tokens?</summary>

Yes. Leave **Alphabet** on automatic detection for most cases, or choose **Base64url** when you want
to force RFC 4648 URL-safe decoding. Automatic mode detects each side independently but rejects a
single input that mixes the two alphabets.

</details>

<details>
<summary>What does shift-aware alignment do?</summary>

Offset alignment compares byte 10 with byte 10, byte 11 with byte 11, and so on. If one byte was
inserted near the front, every later byte appears different. Shift-aware alignment trims the common
prefix and suffix first, so that case is reported as one insertion.

</details>

<details>
<summary>When should I use the text output?</summary>

Use **Unified text diff** only when the decoded bytes are UTF-8 text, such as a Base64-encoded JSON
or config file. Binary payloads are rejected with a message pointing you to the hexdump output.

</details>
