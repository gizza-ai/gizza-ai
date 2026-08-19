## About this tool

**JSON5 / JSONC ⇄ JSON Converter** takes the relaxed JSON people actually write in config files — `tsconfig.json`, VS Code `settings.json`, Babel and ESLint configs, `.jsonc` fixtures — and turns it into strict [RFC 8259](https://www.rfc-editor.org/rfc/rfc8259) JSON that any parser will accept. It also runs the other way, rewriting plain JSON as JSON5 with unquoted keys and single quotes.

The parser reads the full JSON5 grammar, which is a superset of both JSONC and strict JSON:

- **Comments** — `// line` and `/* block */`, anywhere whitespace is allowed.
- **Trailing commas** — after the last element of an array or the last member of an object.
- **Unquoted keys** — bare identifiers such as `port:` instead of `"port":`.
- **Single-quoted strings** — `'a'` as well as `"a"`.
- **Line continuations** — a backslash at end of line joins the next line into the same string.
- **Extra escapes** — `\x41`, `\v`, `\0`, and `\a`-style escapes that stand for the character itself.
- **Loose numbers** — hexadecimal (`0xff`), leading-dot (`.5`), trailing-dot (`5.`), leading-plus (`+1`) and leading-zero (`007`) literals.
- **Non-finite literals** — `NaN`, `Infinity`, `-Infinity`.

Everything JSON5-only is normalized on the way out: numbers become plain decimal literals, keys and strings get double quotes, comments and trailing commas disappear.

### Worked example

JSONC input:

```json5
{
  // the dev server port
  port: 8080,
  /* only these hosts are allowed */
  hosts: ['a', 'b',],
}
```

Strict JSON output (**Direction** = JSON5 / JSONC → strict JSON, **Indentation** = 2 spaces):

```json
{
  "port": 8080,
  "hosts": [
    "a",
    "b"
  ]
}
```

Going the other way, `{"name": "ada", "tags": ["x", "y"]}` with **Direction** = JSON → JSON5 and **JSON5: add trailing commas** ticked becomes:

```json5
{
  name: 'ada',
  tags: [
    'x',
    'y',
  ],
}
```

And loose numeric syntax is normalized — `{ mask: 0xff, ratio: .5, missing: NaN, cap: Infinity }` with **NaN / Infinity in strict JSON** = *Convert to a string* gives:

```json
{
  "mask": 255,
  "ratio": 0.5,
  "missing": "NaN",
  "cap": "Infinity"
}
```

### Options

- **Direction** — *JSON5 / JSONC → strict JSON* (the default), *JSON → JSON5*, or *Auto*, which emits strict JSON when the input used any JSON5-only syntax and JSON5 when the input was already strict.
- **Indentation** — 2 spaces, 4 spaces, a tab, or *Minify* for a single compact line.
- **Sort keys alphabetically** — sorts every object's keys by Unicode code point, at every nesting level, so two configs diff cleanly.
- **NaN / Infinity in strict JSON** — strict JSON cannot spell them, so pick `null` (what `JSON.stringify` does), a string such as `"Infinity"`, or refuse the conversion.
- **JSON5 quote style** — single quotes (JSON5 house style) or double quotes, for JSON5 output.
- **JSON5: leave identifier keys unquoted** — on by default; keys that are plain ASCII identifiers stay bare, anything else stays quoted.
- **JSON5: add trailing commas** — appends a comma after the last element of every non-empty array and object, so adding a line later touches one line in the diff.

### Limits and edge cases

- **Comments carry no data and are always dropped.** JSON has no comment syntax, and this converter emits data only — it does not try to re-attach comments when writing JSON5. If your comments matter, keep the JSON5 file as the source of truth and generate the JSON.
- **Key order is preserved.** Objects keep the order they were written in, so a round trip does not reshuffle your config. Repeated keys collapse last-wins at the first occurrence's position — exactly what a JavaScript object literal does.
- **Number precision is preserved.** Numbers are normalized as *text*, not through a 64-bit float, so `12345678901234567890123` survives intact instead of being rounded.
- **Input caps:** 1 MB of text and 200 levels of nesting. Larger inputs are refused rather than risking an out-of-memory failure in the sandbox.
- Parse errors name the problem and the **line and column** where it was found, e.g. `unterminated string starting at line 2, column 8`.
- Only one top-level value is accepted. Newline-delimited JSON (one object per line) is a different format and is rejected as trailing content.

## FAQ

<details>
<summary>What is the difference between JSON5 and JSONC?</summary>

JSONC ("JSON with Comments") is the dialect Microsoft uses for `tsconfig.json` and VS Code's `settings.json`: strict JSON plus `//` and `/* */` comments and trailing commas. JSON5 is a larger superset that adds unquoted keys, single-quoted strings, line continuations, hexadecimal and leading-dot numbers, and `NaN`/`Infinity`. This converter reads the full JSON5 grammar, so every valid JSONC file is accepted too.

</details>

<details>
<summary>Why did my comments disappear?</summary>

Because strict JSON has no way to express them. The conversion keeps data — objects, arrays, strings, numbers, booleans and nulls — and a comment is not data. That applies in the JSON → JSON5 direction as well: the converter has no comments to restore. Keep the commented file as your source and treat the strict JSON as build output.

</details>

<details>
<summary>What happens to `NaN` and `Infinity`?</summary>

JSON5 allows them, strict JSON does not, so you choose with the **NaN / Infinity in strict JSON** option: `null` (the default, matching `JSON.stringify`), a string like `"NaN"` / `"Infinity"` / `"-Infinity"` if the downstream code re-parses them, or *Refuse the conversion* if a non-finite value means the input is wrong. Writing JSON5 keeps the literals as-is regardless of this setting.

</details>

<details>
<summary>Does converting a big config change my numbers?</summary>

No. Numbers are carried through as text and only re-spelled where JSON5 syntax requires it — `0xff` becomes `255`, `.5` becomes `0.5`, `5.` becomes `5`, `+1` becomes `1` and `007` becomes `7`. Long integers and long decimals are not routed through a floating-point value, so a 23-digit ID stays exactly what you typed.

</details>

<details>
<summary>What does the Auto direction do?</summary>

It looks at what the input actually used. If the text contains any JSON5-only construct — a comment, a trailing comma, a bare key, a single-quoted string, a hex number, `NaN` — it converts to strict JSON. If the input was already strict JSON, it converts to JSON5 instead. It is the one-click "give me the other dialect" mode.

</details>

<details>
<summary>Is my JSON uploaded anywhere?</summary>

No. The converter is a WebAssembly module that runs inside your browser tab. Your config text is parsed and rewritten locally, and you can copy or download the result without it leaving your device — which matters, because config files often hold host names, ports and internal paths.

</details>
