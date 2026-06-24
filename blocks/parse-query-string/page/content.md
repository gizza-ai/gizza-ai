## What this tool does

Paste a URL **query string** — the part after the `?` — and instantly see every
parameter broken out. You get two views: an ordered list of every `key = value`
pair (duplicates kept, in source order), and a **structured JSON** object that
groups repeated keys into arrays and expands bracket notation into nested
arrays and objects. Everything runs locally in your browser — nothing is sent
to a server, it works offline, and there is no sign-up.

You can paste a bare query string, one with a leading `?`, or the `?...` part of
a full URL — the leading `?` is stripped automatically.

## What it handles

| Input | Structured result |
| --- | --- |
| `a=1&b=2` | `{ "a": "1", "b": "2" }` |
| `color=red&color=green&color=blue` | `{ "color": ["red", "green", "blue"] }` |
| `tag[]=a&tag[]=b` | `{ "tag": ["a", "b"] }` |
| `x[0]=a&x[2]=c` | `{ "x": ["a", null, "c"] }` |
| `user[name]=Ann&user[age]=30` | `{ "user": { "name": "Ann", "age": "30" } }` |
| `items[][id]=1&items[][id]=2` | `{ "items": [ { "id": "1" }, { "id": "2" } ] }` |
| `q=S%C3%A3o+Paulo` | `{ "q": "São Paulo" }` |
| `flag&k=v` | `{ "flag": "", "k": "v" }` |

- **Repeated keys** collapse into an array, preserving order.
- **Bracket notation** — empty `[]` appends to an array, a number `[0]` is an
  array index (gaps become `null`), and a name `[key]` is an object key. They
  nest arbitrarily deep (`a[b][c]=v`), including arrays of objects.
- **Percent-encoding** (`%20`, `%C3%A3`, …) is decoded in both keys and values.
- Both `&` and `;` separate pairs; empty segments are skipped.

## The "decode + as a space" option

By default a `+` is decoded to a space — the
`application/x-www-form-urlencoded` convention used by HTML form submissions and
most query strings. Turn the option **off** to keep `+` literal, following strict
RFC 3986 percent-encoding (where a space must be written `%20` and `+` is just a
plus sign).

## FAQ

**Is it free and private?** Yes — your input never leaves your device, and the
page keeps working offline once loaded.

**Does it parse a whole URL?** Paste only the query part (after `?`). It will
strip a leading `?` for you. For splitting a full URL into scheme/host/path/etc.,
use the URI parser tool.

**Why does a bare key like `?flag` show no value?** A key with no `=` has no
value — it's reported with `value` absent in the pairs list and as an empty
string in the structured object.

**What happens to duplicate keys?** They are all kept. In the ordered pairs list
each appears in order; in the structured object they collapse into an array.
