## About this tool

**HMAC Verify** recomputes an HMAC over a message with a secret key, then compares the recomputed tag with the tag you were given using a timing-safe comparison. Use it to debug webhook signatures, API authentication headers, and message-authentication-code checks without uploading data.

### Worked example

RFC 4231 test case 2 uses:

- message: `what do ya want for nothing?`
- key: `Jefe`
- algorithm: `sha256`
- expected: `5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843`

The result starts with:

```text
MATCH — the tag is a valid HMAC-SHA256 of the message under this key.
status:    MATCH
```

Change one hex digit in the expected tag and the status becomes `MISMATCH` while the report still shows the computed tag for comparison.

### Encodings

`message_encoding` and `key_encoding` decide how the message and key are converted to bytes before HMAC: `text` uses UTF-8 bytes, `hex` decodes hexadecimal, and `base64` decodes standard base64. `expected_encoding=auto` tries hex first, then base64, so most pasted signatures work without changing a setting.

### Limits and edge cases

- This verifies one tag at a time. For composite headers such as `t=...,v1=...`, paste only the tag portion into the expected field.
- The verifier strips common prefixes like `sha256=` and `0x` before decoding the expected tag.
- Match checking is timing-safe for equal-length tags. The report also warns when the supplied tag length does not match the selected algorithm.
- MD5 and SHA-1 are included for legacy systems, but prefer SHA-256 or stronger for new protocols.

## FAQ

<details>
<summary>Is this the same as hashing the message?</summary>

No. HMAC combines the message with a secret key before hashing. A plain hash proves only that bytes are unchanged; an HMAC proves the tag was made by someone who knows the key.

</details>

<details>
<summary>What should I paste from a webhook signature header?</summary>

Paste the actual tag value. For `sha256=abcdef...`, you can paste the whole string because the tool strips the `sha256=` prefix. For comma-separated headers such as `t=timestamp,v1=tag`, paste just the `v1` tag (or the provider-specific HMAC value) and make sure the message field exactly matches the signed payload.

</details>

<details>
<summary>Why does the same payload still mismatch?</summary>

HMAC is byte-exact. A different newline, whitespace change, JSON reformat, character encoding, or key encoding changes the tag. Verify that the message is the exact raw payload bytes and that `message_encoding` / `key_encoding` match the data you pasted.

</details>

<details>
<summary>Why include MD5 and SHA-1?</summary>

Some legacy APIs still use HMAC-MD5 or HMAC-SHA1. They are provided for interoperability and debugging old integrations; use HMAC-SHA256, SHA-384, SHA-512, or SHA-3 for new designs.

</details>
