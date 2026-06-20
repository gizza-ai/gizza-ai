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

**Are my credentials uploaded?** No — the header is built in your browser tab
with WebAssembly; nothing is sent.
