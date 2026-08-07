## About this tool

A **field mask** is the little language Google's Partial Response `fields=` parameter uses:
`kind,items(title,stats/length)`. It says which parts of a JSON document you care about, and
the document comes back with everything else pruned away.

That last part is what makes a mask different from JSONPath, jq or JSONata. Those return a
flat list of the nodes that matched. A mask **preserves the shape** of the document: the same
object, at the same nesting, minus the branches you didn't ask for. If you are trimming an
API response down to the handful of fields a client actually reads, the shape is exactly what
you want to keep.

### Mask syntax

| Form | Meaning |
| --- | --- |
| `a` | keep `a` and its entire subtree |
| `a,b,c` | keep several siblings |
| `a/b/c` | follow a path — keep `c` inside `b` inside `a` |
| `a(b,c)` | sub-selection — from `a`, keep only `b` and `c` |
| `*` | wildcard — match every key of an object |
| `\,` `\/` `\(` `\)` `\*` `\\` | escape a literal special character inside a key name |

These nest and combine freely. Arrays are transparent: a selection applied to an array is
applied to every element, and the array's length and order survive untouched.

### Worked example

Mask `kind,items(title,stats/length)` over

```json
{
  "kind": "list",
  "etag": "W/\"9a1\"",
  "items": [
    { "title": "First", "author": "Ada", "stats": { "length": 120, "views": 3 } },
    { "title": "Second", "author": "Grace", "stats": { "length": 80, "views": 9 } }
  ]
}
```

gives

```json
{
  "kind": "list",
  "items": [
    { "title": "First", "stats": { "length": 120 } },
    { "title": "Second", "stats": { "length": 80 } }
  ]
}
```

`etag`, `author` and `views` are gone; `kind` and both array elements stay where they were.

### Beyond the standard grammar

The four selects add the things a one-shot tool needs that a library doesn't:

- **Remove instead of keep.** Switch the mode and the mask becomes a delete list — everything
  it names is dropped and the rest is returned. `users(token,password)` in remove mode is a
  one-line PII/secret stripper.
- **Missing fields.** By default a field the document doesn't have is silently skipped, which
  is what every implementation of this grammar does. You can instead emit it as `null` so a
  list of records all come out the same shape, or fail loudly with the offending path — the
  fastest way to catch a typo'd mask that would otherwise return a nearly-empty result.
- **Emptied objects.** When an element matches nothing, masking leaves a `{}` husk behind.
  Keep them (the compatible default) or drop them recursively.
- **Compact output** for pasting into another tool, or pretty 2-space indent for reading.

Everything runs in WebAssembly inside your browser tab. The document you paste is never
uploaded, and key order is preserved exactly as you wrote it.

## FAQ

<details>
<summary>How is a field mask different from JSONPath or jq?</summary>

JSONPath and jq are query languages: they evaluate an expression and hand back the nodes that
matched, usually as a flat list. A field mask is a *pruning* language: the result is the
original document with the unselected branches removed, at the same paths and in the same
order. Use a mask when the consumer still expects the document's shape — trimming an API
response, for instance — and jq when you want to reshape or compute.

</details>

<details>
<summary>Why did `a` keep everything under `a` instead of just `a` itself?</summary>

That is the defined behaviour of the grammar: a bare name selects the field and its whole
subtree. To prune inside it you have to say so with a sub-selection — `a(x,y)` — or a path
like `a/x`. So `items` keeps every field of every element, while `items(title)` keeps only
each element's `title`.

</details>

<details>
<summary>Do arrays need an index in the mask?</summary>

No. Arrays are transparent to a mask. Write the selection as though the array were a single
object and it is applied to every element, preserving the array's order and length. There is
no way to select element 3 specifically — that is a query, not a mask, and jq or JSONPath is
the right tool for it.

</details>

<details>
<summary>My mask returned `{}` or almost nothing. What went wrong?</summary>

Almost always a name that doesn't exist in the document — a typo, or the wrong case. Absent
fields are omitted silently by default, so a completely wrong mask looks like a working mask
with nothing to show. Set the missing-fields option to "Fail with an error" and the tool
names the offending field and the path it was looked for at.

</details>

<details>
<summary>Can I use this to strip secrets or PII out of a payload?</summary>

Yes — that is what remove mode is for. Write a mask that names the fields you want gone,
switch the mode to remove, and you get the whole document back minus exactly those fields:
`token`, `users(email,phone)`, `*/password`. Because the tool runs locally in your browser,
the payload never leaves the machine while you do it.

</details>

<details>
<summary>Is there a size limit?</summary>

The document can be up to 5,000,000 bytes and the mask up to 4,000 bytes, nested at most 64
levels deep. Those ceilings exist to keep a pathological input from hanging the tab; ordinary
API responses are nowhere near them.

</details>
