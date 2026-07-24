## What this tool does

Paste a raw HTTP request **`Cookie:` header** — the semicolon-separated
`name=value` list your browser sends and that shows up in DevTools under
**Network → Headers → Request Headers** — and get clean **JSON** back. Pick the
shape you need: a `{ "name": "value" }` object, or an ordered array of
`{ "name", "value" }` pairs (the shape Puppeteer, Playwright, and Selenium use).
Values are URL-decoded by default. Everything runs locally in your browser —
nothing is uploaded, it works offline, and there is no sign-up.

You can paste a bare cookie string, or the whole header line including the
`Cookie:` name — the leading `Cookie:` (or `Set-Cookie:`) label and surrounding
whitespace are stripped automatically.

## Worked example

Input:

```
sessionid=abc123; theme=dark; redirect=%2Faccount%2Fsettings
```

Output (object shape, values decoded):

```json
{
  "sessionid": "abc123",
  "theme": "dark",
  "redirect": "/account/settings"
}
```

Switch **Output shape** to the array form and the same input becomes:

```json
[
  { "name": "sessionid", "value": "abc123" },
  { "name": "theme", "value": "dark" },
  { "name": "redirect", "value": "/account/settings" }
]
```

## What it handles

- **Separators** — cookies split on `;`; newlines work too, so a pasted
  multi-line list parses.
- **Percent-decoding** — `%2F` → `/`, `%20` → a space, and so on, in both names
  and values. Turn it off to keep values exactly as sent. A `+` is kept literal:
  cookies are **not** form-urlencoded, so `+` is a plus sign, not a space.
- **Quoted values** — a value wrapped in double quotes (`sid="a b"`) is
  unwrapped to `a b`, following RFC 6265.
- **Duplicate names** — in object shape a repeated cookie name collapses into an
  array of its values (no data is lost); in array shape every cookie is kept as
  its own entry, in order.
- **Source order** — the object keeps cookies in the order you pasted them, not
  alphabetical.

## Limits and edge cases

- This parses the request **`Cookie:` header** — a flat name/value list.
  `Set-Cookie` **response** attributes (`Expires`, `Max-Age`, `Path`, `Domain`,
  `Secure`, `HttpOnly`, `SameSite`) are **not** extracted; if you paste a
  `Set-Cookie` line, those attributes are treated as ordinary `name=value`
  entries.
- A segment with no `=` (a bare token like `flag`) is kept as a name with an
  empty-string value.
- An invalid `%` escape (e.g. `100%off`) is left literal rather than raising an
  error — parsing is lenient.
- Empty or whitespace-only input returns an empty result (`{}` or `[]`), not an
  error.

## FAQ

<details>
<summary>Is it free and private?</summary>

Yes — the parsing happens entirely in your browser with WebAssembly. Your cookie
string never leaves your device, and the page keeps working offline once loaded.

</details>

<details>
<summary>Does it parse <code>Set-Cookie</code> response headers with attributes?</summary>

No. This tool reads the request `Cookie:` header, which is a flat `name=value`
list with no attributes. If you paste a `Set-Cookie` line, its attributes
(`Path`, `Domain`, `Secure`, `HttpOnly`, `SameSite`, …) are treated as ordinary
cookies rather than parsed as metadata.

</details>

<details>
<summary>Why is a <code>+</code> in a value kept as a plus, not a space?</summary>

Cookie values use percent-encoding but are **not**
`application/x-www-form-urlencoded`, so a `+` is a literal plus sign. Only `%NN`
escapes are decoded (when URL-decoding is on). A real space is written `%20`.

</details>

<details>
<summary>What happens to duplicate cookie names?</summary>

Nothing is dropped. In the object shape a repeated name collapses into an array
of its values, in source order. In the array shape each cookie stays a separate
`{ name, value }` entry, so you see every duplicate exactly as sent.

</details>

<details>
<summary>Which output shape should I use for browser automation?</summary>

Use the **array** shape — `[{ "name", "value" }]` matches the cookie objects
Puppeteer, Playwright, and Selenium expect (you'll typically add `domain` and
`path` yourself). Use the **object** shape when you just want a quick
`{ name: value }` lookup.

</details>
