## About this tool

A raw HTTP cookie header is easy to copy out of devtools and hard to read. The two
directions look similar but mean different things: a **request** `Cookie:` header is a
flat `name=value; name=value` list with no attributes, while each **response**
`Set-Cookie:` line carries exactly **one** cookie plus its attributes — `Domain`,
`Path`, `Expires`, `Max-Age`, `Secure`, `HttpOnly`, `SameSite`, `Priority` and
`Partitioned`.

Paste either one. **Header direction** defaults to *Auto-detect*: a line is read as
`Set-Cookie` when it carries a `Set-Cookie:` header name or any known attribute after
the first `;`, and as `Cookie` otherwise. A pasted `Cookie:` / `Set-Cookie:` prefix is
stripped for you, so you can copy a whole header line straight from the network panel.

### Worked example

The built-in **Set-Cookie with attributes** example parses one response header with the
**Table** output:

```text
1 cookie (Set-Cookie header)

Name  Value   Size  Domain       Path  Expires               Max-Age  Secure  HttpOnly  SameSite  Priority  Partitioned
----  ------  ----  -----------  ----  --------------------  -------  ------  --------  --------  --------  -----------
sid   abc123  10    example.com  /     2015-10-21T07:28:00Z  -        yes     yes       Lax       -         no
```

`Expires` is shown normalized to ISO-8601 UTC — the input read
`Expires=Wed, 21 Oct 2015 07:28:00 GMT`. A `-` means the attribute was not present.

The **Cookie header** example switches direction and format. Request cookies have no
attributes, so the JSON is just the pairs, their decoded values and their byte sizes:

```json
{
  "cookies": [
    { "name": "sessionid", "size": 16, "value": "abc123", "warnings": [] },
    { "name": "theme", "size": 10, "value": "dark", "warnings": [] },
    { "name": "redirect", "size": 19, "value": "/account", "warnings": [] }
  ],
  "count": 3,
  "mode": "cookie"
}
```

Note `redirect`: the wire value was `%2Faccount` and **URL-decode names and values** is
on by default, so it reads back as `/account`. `size` still counts the raw 19 bytes that
were actually sent.

### Misconfiguration warnings

**Flag misconfigurations** is on by default and reports structural problems per cookie.
The **Several Set-Cookie lines** example, rendered as a Markdown table:

```text
| Name | Value | Size | Domain | Path | Expires | Max-Age | Secure | HttpOnly | SameSite | Priority | Partitioned |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| __Host-id | 1 | 11 | - | / | - | - | yes | yes | Strict | - | no |
| tracker | xyz | 11 | .example.com | - | - | 0 | no | no | None | - | no |

**Warnings**

- `tracker` — SameSite=None without Secure — browsers reject this cookie
- `tracker` — no Secure — the cookie may be sent over plain HTTP
- `tracker` — no HttpOnly — the cookie is readable from JavaScript
- `tracker` — Max-Age 0 — this deletes the cookie immediately
- `tracker` — leading dot in Domain is ignored by modern browsers (RFC 6265)
```

The full warning set covers: `SameSite=None` without `Secure`; a missing or unknown
`SameSite`; missing `Secure` or `HttpOnly`; an unnamed cookie; a duplicate name; a
cookie over the 4096-byte limit; both `Expires` and `Max-Age` set; a non-integer
`Max-Age`; a `Max-Age` of `0` or less; an unparseable `Expires`; a leading dot in
`Domain`; `Partitioned` without `Secure`; an unknown `Priority`; and the
`__Secure-` / `__Host-` name-prefix rules. Turn the checkbox off for a clean
machine-readable result.

### What it handles

- **Both directions**, auto-detected, or forced with **Header direction** when a
  request header happens to contain a pair named like an attribute.
- **Dates in every shape RFC 6265 §5.1.1 allows** —
  `Wed, 21 Oct 2015 07:28:00 GMT`, `Wed, 21-Oct-2015 07:28:00 GMT` and the asctime
  `Wed Oct 21 07:28:00 2015` all normalize to `2015-10-21T07:28:00Z`. Two-digit years
  follow the spec: `70`–`99` → `19xx`, `00`–`69` → `20xx`.
- **Quoted values** — an RFC 6265 `"double-quoted"` value is unwrapped.
- **Unknown attributes** are never dropped; they are kept under `attributes.other`.
- **Derived fields** in JSON — `session` (no `Expires` and no `Max-Age`, so the cookie
  dies with the browser session) and `host_only` (no `Domain`, so it is sent to the
  origin host only).
- **Four output formats** — JSON, an aligned plain-text table, CSV with a header row,
  and a Markdown pipe table for pasting into a doc or an issue.

### Limits and edge cases

- **Percent-decoding is lenient and not form-decoding.** A `+` stays a literal `+` —
  cookies are not `application/x-www-form-urlencoded`. A malformed escape like a stray
  `%` is left as-is rather than erroring, and non-UTF-8 bytes degrade to `U+FFFD`.
  Attribute values are never decoded.
- **`size` is always the raw byte count** of `name=value` as written, before decoding
  and excluding attributes — that is what the common 4096-byte per-cookie budget
  measures.
- **Nothing is validated against a clock.** The tool never reads the current time, so
  it will not tell you whether a cookie is already expired and shows no countdown; the
  output is fully deterministic. `Max-Age` of `0` or less is still flagged, because
  that is a delete regardless of "now".
- **No decryption or signature checking.** A signed or encrypted session value is
  reported verbatim; this tool splits headers, it does not open them.
- **A segment with no `=`** becomes a name with an empty value, and a cookie whose
  `name=value` pair has no name at all is reported as `(unnamed)` and flagged rather
  than silently dropped.
- **Blank lines are skipped**, and in `set-cookie` mode each remaining line is one
  cookie — a single cookie split across wrapped lines will not parse as one.

### Privacy

Everything runs **in your browser** via WebAssembly — nothing is uploaded, logged or
stored. Session cookies from a live account are safe to paste. The same engine is
available from the CLI and in chat.

## FAQ

<details>
<summary>What is the difference between a Cookie header and a Set-Cookie header?</summary>

`Set-Cookie` is a **response** header sent by the server: one cookie per line, plus the
attributes that tell the browser how to store it. `Cookie` is the **request** header the
browser sends back: just a flat `name=value; name=value` list, with the attributes
stripped out. That is why the request-header view has no Domain/Path/Secure columns —
those values are never transmitted back.

</details>

<details>
<summary>Why does my Cookie header get parsed as Set-Cookie (or the other way round)?</summary>

Auto-detect looks for a `Set-Cookie:` header name or a known attribute name after the
first `;`. A request header carrying a cookie literally named `path` or `secure` trips
that heuristic. Set **Header direction** to `Cookie (request header)` to force every
`;`-separated segment to be read as a name/value pair, or to
`Set-Cookie (response header)` to force one-cookie-per-line with attributes.

</details>

<details>
<summary>Which wins, Expires or Max-Age?</summary>

`Max-Age` wins wherever it is supported, and setting both is flagged as a warning
because it is usually accidental. `Max-Age` is a relative lifetime in seconds, so it
sidesteps client-clock skew; `Expires` is an absolute date, normalized here to
ISO-8601 UTC so two cookies written in different date shapes can be compared directly.
A cookie with neither is a **session** cookie.

</details>

<details>
<summary>What do the __Host- and __Secure- name prefixes require?</summary>

They are enforced by the browser, not just conventions. `__Secure-` requires the
`Secure` attribute. `__Host-` requires `Secure`, **no** `Domain` attribute at all, and
`Path=/`. A cookie whose name uses a prefix it does not satisfy is rejected outright,
so the tool flags it as a warning rather than letting it look valid.

</details>

<details>
<summary>Why is my cookie value still URL-encoded — or decoded when I did not want it?</summary>

**URL-decode names and values** is on by default and turns `%2F` into `/`. Turn it off
to see exactly what was sent on the wire. Two things never change either way: a `+` is
kept literal (cookie values are not form-encoded, so `+` means `+`), and attribute
values such as `Domain` or `Path` are never decoded.

</details>

<details>
<summary>Can it tell me whether a cookie has already expired?</summary>

No — deliberately. The tool never reads the clock, which keeps the same input producing
the same output everywhere. It normalizes `Expires` to an ISO-8601 UTC timestamp and
reports `Max-Age` as written, so you can compare against whatever "now" you care about.
The one time-related case it does flag is `Max-Age` of `0` or less, which deletes the
cookie immediately no matter when it is sent.

</details>
