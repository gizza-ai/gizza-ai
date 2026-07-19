## Fix broken JSON in one paste

Copied JSON out of a log file, a JavaScript source, a Python `print`, or an LLM answer and
your parser rejects it? Paste it above and this tool rebuilds it as valid JSON. It is a
tolerant parser, not a find-and-replace: the input is re-parsed token by token and
re-serialized, so the output is always syntactically valid JSON. Everything runs locally in
your browser — nothing is uploaded.

### What it fixes

- **Trailing commas** — `[1, 2, 3,]` → `[1, 2, 3]`
- **Single quotes, backticks, and smart quotes** — `{'a': 'b'}` → `{"a": "b"}`
- **Unquoted keys and values** — `{name: John Smith}` → `{"name": "John Smith"}`
- **Missing commas** — `{"a": 1 "b": 2}` → `{"a": 1, "b": 2}`
- **Comments** — `//` line and `/* block */` comments are removed
- **Markdown fences** — the JSON inside ` ```json … ``` ` is extracted (prose around it is ignored)
- **Truncated JSON** — unclosed strings, arrays, and objects are closed (`{"a": [1, 2` → `{"a": [1, 2]}`)
- **Python & JS literals** — `True`/`False`/`None` → `true`/`false`/`null`; `undefined`, `NaN`, and `Infinity` become `null`
- **Raw newlines and tabs inside strings** — escaped to `\n` / `\t`
- **Mismatched brackets** — `[1, 2}` → `[1, 2]`

### Worked example

Input:

```
{'name': 'John', age: 30, // legacy record
 tags: ['admin' 'ops'],}
```

Repaired output (2-space indent):

```json
{
  "name": "John",
  "age": 30,
  "tags": [
    "admin",
    "ops"
  ]
}
```

### Limits and edge cases

- **Syntax only, not semantics** — the repair makes the text parse; it cannot know what a
  half-missing document was supposed to contain. Review the output before trusting it for
  anything critical.
- **Nesting is capped at 200 levels**; deeper input returns an error instead of a repair.
- **Duplicate keys keep the last value**, and object key order is otherwise preserved.
- **Unescaped double quotes inside a double-quoted string** end the string early — escape
  those by hand (`"he said \"hi\""`).
- `NaN` and `Infinity` have no JSON form and become `null`; numbers larger than 64-bit
  integers are converted to floating point.
- Several top-level values (e.g. newline-delimited JSON) are wrapped into one array.
- Text after the first complete value is ignored unless it looks like another JSON value.

## FAQ

<details>
<summary>Why is my JSON invalid in the first place?</summary>

The most common causes are hand-edited config files (trailing commas, comments), data pasted
from JavaScript or Python source (single quotes, unquoted keys, `True`/`None` literals),
word-processor pastes (curly “smart” quotes), and LLM answers that were cut off mid-response
or wrapped in a markdown code fence. All of those are exactly what this tool repairs.

</details>

<details>
<summary>Can it fix truncated JSON from an LLM response?</summary>

Yes. If the response was cut off mid-string, mid-array, or mid-object, the repair closes every
unclosed string, array, and object at the end of the input, and a dangling `"key":` becomes
`"key": null`. It also strips the ` ```json ` fence and any prose around it first. What it
cannot do is invent the data that was never generated — the repaired document contains only
what actually arrived.

</details>

<details>
<summary>Does repairing change my data?</summary>

Values that already parse are kept exactly (key order included). Repairs only normalize
syntax: quoting style, separators, comments, and literal spellings. Three genuine conversions
happen because JSON has no equivalent: `NaN`/`Infinity`/`undefined` become `null`, duplicate
keys collapse to the last value, and oversized integers become floats. Everything runs
locally; the text never leaves your machine.

</details>

<details>
<summary>How is this different from a JSON validator or beautifier?</summary>

A validator points at the first syntax error and stops; a beautifier re-indents JSON that is
already valid. This tool accepts input those reject, repairs every issue it finds in one pass,
and then formats the result with the indentation you chose (2/4 spaces, tabs, or minified).

</details>
