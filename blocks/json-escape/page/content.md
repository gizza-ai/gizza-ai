## About this tool

**JSON Escape / Unescape** makes a string safe to drop inside a JSON document —
or reverses the process.

- **Escape** turns raw text into a JSON-safe string body: double quotes become
  `\"`, backslashes `\\`, newlines `\n`, tabs `\t`, and other control characters
  `\uXXXX`. Tick **Wrap in quotes** to also add the surrounding `"…"`.
- **Unescape** turns an escaped JSON string back into raw text. It accepts input
  with or without the surrounding quotes.

The escaping is done with a spec-correct JSON codec, so it matches exactly what a
real JSON parser expects. Everything runs **locally in your browser** via
WebAssembly — nothing is uploaded.

### Handy for

- Pasting a multi-line snippet into a JSON config or API payload.
- Reading an escaped value out of a log or JSON blob.
- Embedding text in code that builds JSON by hand.

## FAQ

<details>
<summary>Will emoji and accented characters be turned into \uXXXX?</summary>

No. The JSON spec only requires escaping double quotes, backslashes, and
control characters — so `é` or `😀` pass through as-is, which every JSON
parser accepts. Only control characters (below U+0020) become `\uXXXX`
sequences.

</details>

<details>
<summary>When should I tick "Wrap in quotes"?</summary>

Tick it when you want a complete, standalone JSON string token (`"like this"`)
you can paste directly as a value. Leave it off when you're splicing the text
between quotes that already exist in your document. The option only applies in
escape mode.

</details>

<details>
<summary>Do I have to remove the surrounding quotes before unescaping?</summary>

No — unescape handles both forms. `"a\nb"` and `a\nb` decode to the same
two-line result, so you can paste a value straight out of a JSON blob without
trimming it first.

</details>

<details>
<summary>Why does unescape report "invalid JSON string escaping"?</summary>

The input contains a sequence a real JSON parser rejects — for example `\q`
(not a defined escape) or a truncated `\u12` unicode escape. Decoding is done
with a spec-correct JSON codec, so anything it refuses would also fail in your
application.

</details>
