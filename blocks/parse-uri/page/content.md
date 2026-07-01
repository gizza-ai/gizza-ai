## About this tool

The URI / URL parser splits any URI or URL into the components defined by
[RFC 3986](https://www.rfc-editor.org/rfc/rfc3986): the **scheme**, the
**authority** (userinfo, host and port), the **path**, the **query string**, and
the **fragment**. Everything is computed locally in your browser — the URI you
paste is never sent to a server.

### What it extracts

- **Scheme** — lowercased (`https`, `mailto`, `ftp`, `file`, …).
- **Userinfo** — split into the percent-decoded **username** and **password**
  (the part before the `@`).
- **Host** — registered names are lowercased; IPv6 literals are kept in
  brackets (`[2001:db8::1]`).
- **Port** — parsed as a number when present.
- **Origin** — `scheme://authority`, when both are present (the value used for
  same-origin checks).
- **Path** — shown raw and, when it contains escapes, percent-decoded; also
  split into individual **segments** and, when the path points at a file, the
  **filename** and its **extension**.
- **Query** — the raw query string plus a breakdown into **key/value pairs**.
  Pairs are percent-decoded, a `+` becomes a space, `;` is accepted as a
  separator alongside `&`, duplicate keys are preserved in order, and a bare key
  (`?flag`) is reported with no value.
- **Fragment** — the part after `#`, percent-decoded.

### Notes

- **Relative references** (no scheme, e.g. `/search?q=rust`) are parsed too —
  the result simply has no scheme, host or port.
- Schemes without an `//` authority — like `mailto:alice@example.com` — keep the
  remainder in the path, with no host.
- Surrounding whitespace is trimmed before parsing.

This is the same parser exposed to the gizza chat assistant and the `gizza`
command-line tool, so you get identical results across all three surfaces.

## FAQ

<details>
<summary>Can I parse a relative URL that has no scheme or host?</summary>

Yes. A relative reference like `/search?q=rust&page=2` parses fine — the
result just has no scheme, host, or port, while the path, query pairs, and
fragment are extracted as usual. The only input that's rejected is an empty
one.

</details>

<details>
<summary>How are query parameters decoded?</summary>

Each pair is percent-decoded, a `+` becomes a space, and `;` is accepted as a
separator alongside `&`. Duplicate keys are kept in their original order (not
merged), and a bare key such as `?flag` is reported with no value — so the
breakdown mirrors exactly what a server would receive.

</details>

<details>
<summary>What about mailto: links and IPv6 hosts?</summary>

Schemes without a `//` authority, like `mailto:alice@example.com`, keep the
remainder in the path with no host. IPv6 literals stay in their brackets
(`https://[2001:db8::1]:8080/` reports host `[2001:db8::1]` and port `8080`).

</details>

<details>
<summary>Is the URL I paste sent anywhere?</summary>

No — parsing happens in WebAssembly inside your browser tab. That matters
here, since URLs often embed tokens, session IDs, or credentials in userinfo
and query strings.

</details>
