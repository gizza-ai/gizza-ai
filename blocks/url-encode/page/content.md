## What this tool does

Percent-encode (URL-encode) or decode text and URLs instantly, right in your
browser. Nothing is sent to a server — it runs locally, works offline, and needs
no sign-up. Pick a **Mode** and a **Target**, and optionally turn on **Per line**
or raise **Repeat**.

## Modes

| Mode | What it does |
| --- | --- |
| **encode** (default) | Turns unsafe characters into `%XX` escapes — a space becomes `%20`, `ã` becomes `%C3%A3`. |
| **decode** | Reverses it — turns `%XX` escapes back into the original characters. |

## Target — the encoding style

| Target | What it escapes | Use it for | JS equivalent |
| --- | --- | --- | --- |
| **component** (default) | Everything except letters, digits, and `- _ . ~` | A single query value or path segment | `encodeURIComponent` |
| **uri** | Only unsafe bytes; keeps URL delimiters (`: / ? # @ & = …`) | Cleaning a whole URL without breaking it | `encodeURI` |
| **form** | Like component, but a space becomes `+` | HTML form bodies and `+`-style query strings (`application/x-www-form-urlencoded`) | — |

When decoding with **form**, a `+` turns back into a space.

## Batch and repeat

- **Per line** — convert each line of the input independently, keeping the line
  breaks between them. Handy for a list of values or URLs: paste one per line and
  convert them all at once, without the newlines themselves getting encoded.
- **Repeat (1–16)** — apply the operation that many times. Decode with
  `repeat = 2` to un-nest a double-encoded string (a value that was encoded twice,
  e.g. `a b` → `a%20b` → `a%2520b`); or double-encode by repeating the encode.

## Examples

| Input | Settings | Output |
| --- | --- | --- |
| `São Paulo` | encode · component | `S%C3%A3o%20Paulo` |
| `name=John Doe&city=x` | encode · component | `name%3DJohn%20Doe%26city%3Dx` |
| `https://ex.com/a b?x=1&y=2` | encode · uri | `https://ex.com/a%20b?x=1&y=2` |
| `a b+c` | encode · form | `a+b%2Bc` |
| `a%2520b` | decode · repeat 2 | `a b` |
| `S%C3%A3o%20Paulo` | decode | `São Paulo` |

## FAQ

**Is it free and private?** Yes — your input never leaves your device, and it
keeps working offline once the page has loaded.

**When should I use `form` instead of `component`?** Use **form** when the value
goes into an HTML form submission or a query string that encodes spaces as `+`.
Use **component** for modern percent-encoded URLs, where a space is `%20`.

**How does this map to JavaScript?** `component` matches `encodeURIComponent`,
`uri` matches `encodeURI`, and `form` additionally turns spaces into `+`.

**My string looks like it's encoded twice — how do I fix it?** Decode with the
**Repeat** field set to `2` (or higher) to peel off each layer of encoding.
