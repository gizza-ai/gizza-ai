## About this tool

Python's `print()` and `repr()` produce a dict/list *literal*, not JSON — so pasting
it straight into a JSON parser fails. The syntaxes look alike but differ in ways a
strict JSON reader rejects: Python writes `True` / `False` / `None` where JSON writes
`true` / `false` / `null`, prefers `'single quotes'`, allows trailing commas, tuples
`(1, 2)` and sets `{1, 2}`, `#` comments, and integer literals like `0xff` or
`1_000`. This tool parses the Python literal and re-emits clean, valid JSON.

Everything runs locally in your browser via WebAssembly — nothing you paste leaves
your machine.

### Worked example

Input (a Python dict as printed by `print(user)`):

```python
{'name': 'Ann', 'active': True, 'scores': (9, 8, 10), 'city': None}
```

Output (indent = 2 spaces):

```json
{
  "name": "Ann",
  "active": true,
  "scores": [
    9,
    8,
    10
  ],
  "city": null
}
```

`True` became `true`, `None` became `null`, the single quotes became double quotes,
and the tuple `(9, 8, 10)` became a JSON array.

### What it converts

- `True` / `False` / `None` → `true` / `false` / `null`
- single-quoted, triple-quoted (`'''…'''`), raw (`r'…'`), bytes (`b'…'`), and
  `f'…'` / `u'…'` string prefixes → double-quoted JSON strings
- tuples `(1, 2)` and sets `{1, 2}` → JSON arrays (a set's uniqueness/order is not
  preserved beyond input order)
- trailing commas and `#` line comments → removed
- `0xff`, `0o17`, `0b101`, and digit separators like `1_000_000` → decimal numbers
- non-string dict keys (`1`, `True`, `None`) → stringified JSON keys (`"1"`,
  `"true"`, `"null"`), matching `json.dumps`

### Options

- **Indentation** — 2 or 4 spaces, tabs, or minified (single line).
- **Sort keys alphabetically** — like `json.dumps(obj, sort_keys=True)`.
- **Escape non-ASCII** — emit `\uXXXX` escapes (Python's `json.dumps` default);
  off keeps readable UTF-8.

### Limits

- Input is capped at 2 MB and nesting at 200 levels.
- This converts *data literals* only. Python expressions and constructor calls —
  `datetime(2020, 1, 1)`, `Decimal('1.5')`, `set()`, custom class instances, `...`
  (Ellipsis) — are not literals and will error. Reduce them to plain
  dict/list/str/number/bool/None values first.
- Complex numbers (`3j`) have no JSON equivalent and are rejected.

## FAQ

<details>
<summary>Why can't I just paste Python dict output into a JSON parser?</summary>

Because it isn't JSON. Python uses `True`/`False`/`None`, single quotes, tuples,
sets, trailing commas and `#` comments — all of which a strict JSON parser rejects.
This tool translates those Python-only conveniences into their JSON equivalents so
the result parses everywhere.

</details>

<details>
<summary>What happens to tuples and sets?</summary>

Both become JSON arrays, since JSON has no tuple or set type. `(1, 2, 3)` and
`{1, 2, 3}` each convert to `[1, 2, 3]`. A single parenthesized value without a
trailing comma is not a tuple in Python — `(5)` is just `5` — so it converts to the
bare value; write `(5,)` for a one-element array.

</details>

<details>
<summary>My dict has integer or boolean keys — is that valid?</summary>

JSON object keys must be strings, so non-string keys are stringified the same way
Python's `json.dumps` does: `{1: 'a', True: 'b', None: 'c'}` becomes
`{"1": "a", "true": "b", "null": "c"}`. Keys that are themselves tuples or lists
can't be represented as JSON keys and will error.

</details>

<details>
<summary>Does it handle non-decimal numbers like `0xff` or `1_000`?</summary>

Yes. Hex (`0xff`), octal (`0o17`), binary (`0b101`) integer literals and underscore
digit separators (`1_000_000`) are all parsed to their decimal value in the JSON
output (`255`, `15`, `5`, `1000000`). Complex numbers such as `3j` have no JSON
equivalent and are rejected with an error.

</details>

<details>
<summary>Is my data uploaded anywhere?</summary>

No. The conversion runs entirely in your browser through WebAssembly. Nothing you
paste is sent to a server, logged, or stored.

</details>
