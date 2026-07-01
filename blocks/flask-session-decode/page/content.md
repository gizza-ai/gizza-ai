## What this tool does

Paste a Flask session cookie and this decoder turns it back into readable JSON —
**without needing the application's `SECRET_KEY`**. The session contents in a Flask
cookie are only *signed*, not *encrypted*, so anyone holding the cookie can read
what's inside. This tool makes that inspection one paste away, entirely in your
browser.

## How a Flask session cookie is built

By default Flask stores the session client-side in a cookie named `session`, set by
`SecureCookieSessionInterface`, which uses the
[itsdangerous](https://itsdangerous.palletsprojects.com/) `URLSafeTimedSerializer`.
The cookie value has three URL-safe-base64 segments joined by dots:

```
payload . timestamp . signature
```

- **payload** — the session dictionary serialized to JSON, then base64url-encoded.
  If itsdangerous compressed it (it does so whenever zlib makes the value shorter),
  the whole payload segment is prefixed with a literal `.` and the rest is base64url
  of a zlib stream. This decoder inflates that transparently.
- **timestamp** — a big-endian integer counting seconds since itsdangerous's own
  epoch (2011-01-01). The tool converts it back to a normal Unix time and an
  ISO-8601 UTC string.
- **signature** — an HMAC over `payload.timestamp`, keyed by the app's `SECRET_KEY`.
  It is reported here verbatim but **not verified**: validating it would require the
  secret key, which this tool never asks for. The `signature_verified` field is
  therefore always `false`.

## Input formats accepted

You can paste either the bare cookie value or a whole fragment copied from your
browser's dev-tools or a `Set-Cookie:` header — for example
`session="eyJ...".signature; Path=/; HttpOnly; SameSite=Lax`. A leading
`session=`, surrounding quotes, and trailing `; Path=/; HttpOnly` attributes are
stripped automatically.

## Privacy

Everything runs locally in your browser via WebAssembly. The cookie you paste is
never uploaded to any server.

## Security note

Because Flask sessions are signed but not encrypted, never store secrets (passwords,
tokens, private data) in a session cookie — treat anything you can decode here as
readable to the user. To make sessions tamper-proof *and* unreadable, use a
server-side session store instead of the default client-side cookie.

## FAQ

<details>
<summary>How can this decode the cookie without the SECRET_KEY?</summary>

Because Flask's default session cookie is **signed, not encrypted**. The payload
segment is just the session dictionary as base64url-encoded JSON — the
`SECRET_KEY` is only used for the HMAC signature that prevents *tampering*, not
for hiding the contents. That's also why `signature_verified` is always `false`
in the output: verifying the HMAC would require the key, which this tool never
asks for.

</details>

<details>
<summary>Can I paste the whole Set-Cookie header instead of the bare value?</summary>

Yes. A leading `session=`, surrounding quotes, and trailing attributes like
`; Path=/; HttpOnly; SameSite=Lax` are stripped automatically, so a fragment
copied straight from dev-tools or a `Set-Cookie:` header works as-is.

</details>

<details>
<summary>My cookie starts with a dot — is it corrupted?</summary>

No — that leading `.` is itsdangerous's **zlib-compression marker**. Whenever
compressing makes the value shorter, the payload is deflated before encoding.
The decoder detects the marker, inflates the stream transparently, and sets
`compressed: true` in the output.

</details>

<details>
<summary>What does the timestamp in the output represent?</summary>

It's when the cookie was signed — stored by itsdangerous as seconds since its own
epoch of **2011-01-01**, not the Unix epoch. The tool converts it for you and
reports both a normal Unix timestamp and an ISO-8601 UTC string, which is useful
for judging whether a captured session is stale.

</details>
