## About this tool

Google Authenticator's export QR uses an `otpauth-migration://offline?data=...`
link that bundles multiple accounts in a compact protobuf payload. This tool
decodes that payload locally and rebuilds standard single-account `otpauth://`
provisioning URIs that other password managers and authenticator apps can
import.

Paste either the full migration link or only its `data` value. The default output
is one `otpauth://` URI per line; switch to JSON when you need issuer, account
name, base32 secret, algorithm, digits, counter, and rebuilt URI as structured
fields.

Worked example:

```text
otpauth://totp/Example:alice%40example.com?secret=JBSWY3DP&issuer=Example&algorithm=SHA1&digits=6&period=30
otpauth://hotp/ACME:bob?secret=K5XXE3DE&issuer=ACME&algorithm=SHA256&digits=8&counter=5
```

Limits and edge cases: this is a text decoder, not a QR-image scanner. Use your
camera or a QR decoder first if you only have a screenshot. The migration payload
does not carry custom TOTP periods, so TOTP entries are emitted with the standard
`period=30`. Unsupported protobuf enum values and truncated payloads return an
explicit error instead of guessing.

## FAQ

<details>
<summary>Can I paste the whole otpauth-migration link?</summary>

Yes. Paste the full `otpauth-migration://offline?data=...` value, or paste only
the `data` payload. The decoder accepts standard base64, base64url, missing
padding, and percent-escaped payloads.

</details>

<details>
<summary>Does this reveal my 2FA secrets to a server?</summary>

The block is pure Rust and the page runs it in browser WebAssembly. The decoded
secrets appear in the output because that is the purpose of the export, so treat
the result like credentials and do not paste it into logs or tickets.

</details>

<details>
<summary>Why do TOTP entries always include period=30?</summary>

The Google Authenticator migration protobuf stores the secret, issuer, account,
algorithm, digits, type, and HOTP counter, but it does not store a custom TOTP
period. Authenticator apps use 30 seconds by default, so the rebuilt TOTP URIs
state `period=30` explicitly.

</details>

<details>
<summary>Can it scan a QR image directly?</summary>

No. This tool decodes the text payload after a QR has already been read. If you
only have a PNG or screenshot, run it through a QR decoder first and paste the
resulting `otpauth-migration://` text here.

</details>
