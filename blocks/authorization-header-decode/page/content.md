## About this tool

An HTTP `Authorization` header is two things glued together: an **auth-scheme**
and the **credentials** that follow it (RFC 7235). This tool takes them apart and
tells you what each half means, whichever scheme you paste.

Paste a whole header line (`Authorization: Basic QWxhZGRpbjpvcGVuIHNlc2FtZQ==`), just the value
(`Basic …`), or even a naked base64 credential with no scheme at all — it is
accepted and reported as such. Line breaks from a wrapped log line are treated as
spaces. Everything runs in your browser via WebAssembly; the header is never
uploaded.

### What it decodes

- **Basic** — base64-decoded and split on the **first** colon into username and
  password (RFC 7617). A colon inside the password is kept where it belongs.
- **Bearer** (also `DPoP`, `Token`, `ApiKey`) — the token's *structure*: JWT, JWE
  or opaque, segment count and per-segment lengths, character set, and the
  decoded **JOSE header** (`alg`, `typ`, `kid`). Payload claims are deliberately
  not decoded — see the FAQ.
- **Digest** — every `name=value` auth-param parsed into a map, quoted values
  unescaped: `realm`, `nonce`, `qop`, `nc`, `cnonce`, `opaque`, `algorithm`,
  `uri`, `response`.
- **AWS4-HMAC-SHA256** — the `Credential=` scope split into access key ID, date,
  region, service and termination string, plus `SignedHeaders` and `Signature`.
- **Negotiate / NTLM** — the base64 blob is decoded and identified from its
  signature, including the NTLMSSP message type (1 negotiate, 2 challenge,
  3 authenticate).
- **Anything else** — `Hawk`, `Signature`, `HOBA`, `Mutual`, `SCRAM-SHA-256`,
  `vapid`, `OAuth` and unknown custom schemes are parsed with the generic RFC 7235
  grammar, so a custom scheme still yields a structured result.

### Options

- **Output format** — `json` (default) returns every field plus the warning list;
  `text` returns aligned `key: value` lines; `table` returns an ASCII
  field/value table for pasting into a ticket or a chat message.
- **Mask secrets** — replaces the password, bearer token, `response`, `signature`,
  `mac` and the raw credential string with asterisks while keeping the reported
  lengths, so you can share a parse without sharing a secret.
- **Strict** — turns every warning (non-canonical scheme spelling, missing scheme,
  missing colon, unpadded or URL-safe base64, non-UTF-8 credentials, unknown
  scheme) into an error instead. Useful when checking that a client emits a
  clean header.

### Limits and edge cases

- Input is capped at **8192 characters** — far above any real header, including
  SPNEGO blobs.
- The **first** colon separates username from password; a password containing
  colons round-trips correctly and is flagged so you know why.
- Decoded credentials that are not valid UTF-8 are shown with `U+FFFD` for the
  invalid bytes and marked `valid_utf8: false` rather than rejected.
- Both standard (`+ /`) and URL-safe (`- _`) base64 alphabets decode, with or
  without `=` padding; unusual alphabets and missing padding raise a warning.
- Nothing is verified. A `Digest` response, a JWT signature or a SigV4 signature
  is reported, never checked — verification needs the shared secret or key.
- The scheme is matched case-insensitively; the spelling you pasted is reported as
  `scheme` and the registered spelling as `scheme_canonical`.

## FAQ

<details>
<summary>Are my credentials uploaded anywhere?</summary>

No. The decoding runs inside your browser tab as WebAssembly — the header never
leaves your device, and there is no server-side step. That said, base64 is
*encoding*, not encryption: if a header has been pasted somewhere it shouldn't,
rotate the credential rather than trusting that it stayed hidden.

</details>

<details>
<summary>Why doesn't it show the JWT payload claims?</summary>

By design. A Bearer token here is described **structurally** — JWT vs opaque,
segment lengths, character set, and the JOSE header with `alg`/`typ`/`kid`. Claim
inspection and expiry validation are a different job with different options
(`exp`/`nbf` handling, clock leeway), so reach for a dedicated JWT decoder for
those. Everything this tool reports is about the *header*, not the token's
contents.

</details>

<details>
<summary>What happens if my password contains a colon?</summary>

It stays in the password. RFC 7617 splits the decoded `username:password` string
on the **first** colon only, so `user:pa:ss` decodes to username `user` and
password `pa:ss`. The result includes a warning pointing this out, because it's a
common source of confusion. A colon in the *username* is impossible to express —
there would be no way to tell where the username ended.

</details>

<details>
<summary>Can I paste the whole header line, or just the value?</summary>

Either. `Authorization: Basic QWxhZGRpbjpvcGVuIHNlc2FtZQ==`, `Basic
QWxhZGRpbjpvcGVuIHNlc2FtZQ==` and the bare `QWxhZGRpbjpvcGVuIHNlc2FtZQ==` all
work. `Proxy-Authorization: Basic QWxhZGRpbjpvcGVuIHNlc2FtZQ==` is accepted too. If the name is something else
entirely, it's still parsed with the same grammar and you get a warning. A value
with no scheme at all is guessed (JWT-shaped → Bearer, base64 of `user:pass` →
Basic) and flagged, since a real header must start with a scheme.

</details>

<details>
<summary>What does "mask secrets" actually hide?</summary>

The Basic password, the bearer token, the raw credential string, and the
secret-bearing auth-params (`response`, `signature`, `mac`, `cnonce`, `sig`).
Non-secret context — the scheme, the username, `realm`, `nonce`, `qop`, the AWS
credential scope, and every reported *length* — is kept, so a masked result is
still diagnostic enough to paste into a bug report.

</details>

<details>
<summary>Does it verify the signature or check whether the token is valid?</summary>

No. This is a parser, not a verifier. It will tell you that a Digest header
carries a `response`, that a JWT has three segments and an `HS256` header, or
that a SigV4 header signs `host;x-amz-date` — but confirming those values are
correct requires the shared secret, the private key, or the full canonical
request, none of which belong in a browser tool.

</details>

<details>
<summary>What is "strict" mode for?</summary>

It turns every warning into a hard error, so it doubles as a lint for
header-emitting code. Run a client's header through with strict on: if it comes
back clean, the scheme is spelled canonically, the base64 is padded and uses the
standard alphabet, the credentials are valid UTF-8 with a proper colon, and the
scheme is one that's actually registered. Anything less and you get the reason.

</details>
