## Basic Auth header generator

Turn a username and password into an HTTP **Basic** Authorization header. The
value is `Basic ` followed by the base64 of `username:password` (RFC 7617). It's
computed locally in your browser — your credentials are never sent anywhere.

### Output

- Default: just the value, e.g. `Basic YWxhZGRpbjpvcGVuc2VzYW1l` — drop it into an
  `Authorization` header.
- Tick **full header** to get the whole line: `Authorization: Basic …` — handy
  for pasting into `curl -H`, a REST client, or docs.

### Notes

- The username can't contain a colon (`:`) — that's the separator between
  username and password. The password may be empty.
- Base64 is encoding, **not** encryption — anyone can decode a Basic header, so
  only use it over HTTPS.

### FAQ

<details>
<summary>Are my credentials uploaded?</summary>

No — the header is built in your browser tab
with WebAssembly; nothing is sent.

</details>

<details>
<summary>Why can't my username contain a colon?</summary>

RFC 7617 encodes the credentials as `username:password`, so the first colon marks where the username ends. A colon in the username would shift everything after it into the password. Colons in the *password* are fine — the tool rejects only usernames containing `:`.

</details>

<details>
<summary>Can the password be empty?</summary>

Yes. Leaving the password blank produces `base64("username:")`, which some APIs use for token-style auth (token as username, empty password). Only the username is required.

</details>

<details>
<summary>What does the "full header" option change?</summary>

Off (the default) you get just the value, `Basic YWxhZGRpbjpvcGVuc2VzYW1l`, ready to assign to an `Authorization` header in code. On, you get the complete line `Authorization: Basic …`, which you can paste directly after `curl -H` or into an HTTP client.

</details>

<details>
<summary>Do special characters like é or ü work?</summary>

Yes — the credentials are encoded as UTF-8 before base64, so accented and non-Latin characters round-trip correctly. Just be aware some older servers expect Latin-1 and may reject UTF-8 credentials.

</details>
