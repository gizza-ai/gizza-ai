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
