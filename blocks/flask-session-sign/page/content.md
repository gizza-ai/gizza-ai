## About this tool

Flask's default session cookie is signed, not encrypted: the browser can hold the session data, and Flask verifies an HMAC signature before trusting it. This tool builds a compatible value from a JSON object payload and your Flask `SECRET_KEY` using the same pieces Flask wires into `itsdangerous.URLSafeTimedSerializer` by default: salt `cookie-session`, `hmac` key derivation, SHA-1, compact sorted JSON, and zlib compression only when it makes the payload smaller.

Use it when you need a reproducible cookie for a local test app, a CTF lab, or a migration check. Set `timestamp` to a fixed Unix time when you want byte-for-byte repeatable output; leave it at `0` to sign with the current clock in the running surface. The result includes the full cookie value, a ready-to-paste `Set-Cookie` header, each signed segment, the serialized payload, the derived signing key in hex, and cookie-size warnings.

### Worked example

Payload:

```json
{"user":1,"admin":true}
```

Secret: `dev-key-123`, timestamp: `1700000000`, Flask defaults. The output JSON contains a `cookie` field shaped like:

```text
eyJhZG1pbiI6dHJ1ZSwidXNlciI6MX0.ZVPxAA.<signature>
```

Copy the `cookie` value into the `session` cookie, or use the `set_cookie_header` field in a local response. If your app uses a different salt, digest, key derivation, cookie name, or byte-encoded secret, set the matching advanced field before signing.

### Limits and edge cases

- The payload must be a JSON object string. Use JSON spelling (`true`, `false`, `null`) rather than Python literals (`True`, `False`, `None`).
- Flask tagged types such as bytes, datetimes, tuples, UUIDs, and Markup are not expressible in plain JSON input. Reserved one-key tag dictionaries are escaped so ordinary JSON objects with those keys do not become Flask tagged values by accident.
- Flask's default JSON behavior escapes non-ASCII characters (`ensure_ascii = true`), and this tool follows that default for byte-for-byte compatibility.
- Cookies over about 4096 bytes are likely to be dropped by browsers; the output includes a warning when `name=value` exceeds that limit.
- This tool signs a cookie. It does not brute-force secrets, fetch cookies from URLs, or verify an existing cookie against a wordlist.

## FAQ

<details>
<summary>Is a Flask session cookie encrypted?</summary>

No. Flask's default client-side session is signed for integrity, not encrypted for secrecy. Anyone with the cookie can base64-decode the payload, but only someone with the correct `SECRET_KEY` can create a signature Flask accepts.

</details>

<details>
<summary>Which settings match Flask defaults?</summary>

Use salt `cookie-session`, digest `sha1`, key derivation `hmac`, compression `auto`, cookie name `session`, and a UTF-8 secret. Those defaults match `SecureCookieSessionInterface` for ordinary Flask apps.

</details>

<details>
<summary>Why should I set a timestamp?</summary>

The timestamp is part of the signed value. If you leave `timestamp` at `0`, the current clock is used and the cookie changes every run. Set a Unix timestamp when you need a deterministic value for tests or documentation.

</details>

<details>
<summary>What is legacy epoch mode?</summary>

Older itsdangerous releases encoded timestamps as seconds since 2011-01-01. Current itsdangerous signs the full Unix timestamp. Enable legacy epoch mode only when you need compatibility with an old application or old challenge material.

</details>

<details>
<summary>Can this recover or crack a secret key?</summary>

No. The tool signs with a secret you already know. Secret recovery is a long-running brute-force workflow over candidate keys, which is outside this single-shot browser-safe tool model.

</details>
