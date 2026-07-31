## What this tool does

**JSON Array Sorter** reorders the elements of a JSON array of objects by the value of one
or more fields — the classic *ORDER BY* operation, but for a raw JSON array you can paste
straight from an API response, a log export, or a database dump. Give it one field or a
whole list, and it returns the same objects in a new order, ready to copy back out.

Each sort key can dig into **nested paths** with dot-notation (`address.city`, or the array
index `tags.0`), carry its own **direction** with a `+`/`-` prefix, and fall back to a global
ascending/descending default. You control where rows with a **missing or null** field land,
whether strings compare **case-insensitively**, and how the result is **indented** — or
minified onto a single line. It sorts array *elements* by a field; it never touches the keys
*inside* each object, so the shape of every record is preserved exactly.

## Worked example

**Input**

```json
[
  { "dept": "Eng", "salary": 120 },
  { "dept": "Ops", "salary": 90 },
  { "dept": "Eng", "salary": 150 }
]
```

With **keys = `dept,-salary`** (department ascending, then salary descending within each
department) and **indent = 2**, the output is:

```json
[
  {
    "dept": "Eng",
    "salary": 150
  },
  {
    "dept": "Eng",
    "salary": 120
  },
  {
    "dept": "Ops",
    "salary": 90
  }
]
```

The two `Eng` rows group together and sort highest-paid first; `Ops` follows. Set **indent
to `0`** to get the same order minified onto one line:
`[{"dept":"Eng","salary":150},{"dept":"Eng","salary":120},{"dept":"Ops","salary":90}]`.

## How to use it

1. Paste a **JSON array of objects** into the input.
2. Enter one or more **sort keys**, comma-separated — e.g. `age`, or `dept,-salary,name`.
   Use dots for nested fields and `-`/`+` to flip a single key's direction.
3. Pick the **default direction**, where **missing/null** values go, whether strings are
   **case-insensitive**, and the **indent** (0 to minify).
4. Read the sorted array. Everything runs locally in your browser — nothing is uploaded.

## FAQ

<!-- FAQ MUST be <details>/<summary> accordions. Keep the blank line inside each. -->

<details>
<summary>How is this different from sorting the keys of a JSON object?</summary>

This tool sorts the **elements** of an array — it changes the *order of the objects*, like
SQL's `ORDER BY`. Alphabetizing the **keys inside** each object (so `"zip"` comes after
`"city"`) is a different job; use a JSON key-sort tool for that. Here, the keys inside every
record stay exactly where they were.

</details>

<details>
<summary>Can I sort by more than one field, each in its own direction?</summary>

Yes. List the keys comma-separated and prefix any key with `-` for descending or `+` for
ascending — e.g. `dept,-salary,name` sorts by department ascending, then salary descending,
then name ascending. Keys without a prefix use the **default direction** you pick. Ties on
the first key are broken by the next, and the sort is **stable**, so equal rows keep their
original relative order.

</details>

<details>
<summary>What happens to rows where the sort field is missing or null?</summary>

A field that is absent or explicitly `null` is treated as **missing** and grouped together
at the **end** by default — switch **Missing / null values** to *first* to move them to the
top instead. This placement is **absolute**: missing rows stay on their chosen side even when
the key sorts descending, so they never mix into the middle of your data.

</details>

<details>
<summary>How are numbers, strings, and mixed types compared?</summary>

Numbers compare **numerically**, so `9` sorts before `10` (not lexically). Strings compare by
character; enable **Case-insensitive strings** to fold case so `Banana` sits next to `apple`
rather than ahead of all lowercase letters. If a field holds mixed types across rows, values
fall back to a stable type order (boolean, number, string, array, object) so the sort still
finishes deterministically.

</details>

<details>
<summary>Can I reach into nested objects or array elements?</summary>

Yes — use **dot-notation** in the key. `user.name` sorts by the `name` inside each row's
`user` object, and `tags.0` sorts by the first element of each row's `tags` array. If any
segment of the path doesn't exist for a row, that row counts as **missing** and follows the
missing/null placement rule.

</details>

## Limits & edge cases

- The input must be a **top-level JSON array**; a bare object or scalar is rejected with a
  clear error. Each element is expected to be an object you can sort by field.
- **Indent** is clamped to `0`–`8` spaces; `0` minifies to a single line, and any larger
  value is capped at 8.
- Comparison is **type-aware** but not locale-aware: strings sort by Unicode codepoint (with
  optional case-folding), and there is no numeric tolerance — `10` and `10.0` are equal as
  numbers but `"10"` (a string) is not.
- At least one **sort key** is required; an empty or all-blank `keys` value is an error.
- Everything runs in-browser via WebAssembly — the JSON you paste is never uploaded.
