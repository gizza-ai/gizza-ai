## What this tool does

Validate a **Base64** or **Base64url** string and get a precise report instead of a bare pass/fail. The checker points to invalid characters by 1-based position and line/column, explains padding mistakes, detects mixed alphabets, checks optional MIME/PEM line lengths, and reports the decoded byte count for valid input.

It understands the two RFC 4648 alphabets:

| Variant | Characters 62/63 | Typical padding |
| --- | --- | --- |
| Standard Base64 | `+` and `/` | `=` required or commonly present |
| Base64url | `-` and `_` | often omitted, as in JWT segments |

The validator also accepts practical pasted forms: wrapping single/double quotes are stripped, `data:<mime>;base64,` prefixes are detected, and whitespace can be ignored for MIME or PEM-style wrapped data.

## Options

| Option | What it checks |
| --- | --- |
| **Alphabet** | Auto-detect either alphabet, or strictly require standard Base64 or URL-safe Base64url. Mixed `+`/`/` with `-`/`_` is reported. |
| **Padding rule** | Accept optional padding, require strict padding to a multiple of 4, or forbid `=` for unpadded URL-safe strings. |
| **Ignore whitespace** | Leave on for pasted wrapped data; turn off to flag spaces, tabs, and newlines as invalid characters. |
| **Max line length** | Use `76` for MIME, `64` for PEM, or `0` to skip line-length checks. |
| **Output format** | Choose a readable text report or JSON for scripts and tests. |

## Examples

| Input | Settings | Result |
| --- | --- | --- |
| `SGVsbG8sIHdvcmxkIQ==` | auto · optional padding | Valid, 13 decoded bytes, text preview `Hello, world!`. |
| `SGVsbG8_d29ybGQ` | url-safe · forbidden padding | Valid Base64url with no trailing `=`. |
| `SGVsbG8s!Q==` | auto | Invalid character `!` with its position and a suggested repair when possible. |
| `QUJD\nRA==` | standard · required · max line length 76 | Valid wrapped data; JSON output can be consumed by another tool. |

## Limits & edge cases

- Maximum input is 4 MiB before whitespace stripping.
- A valid length can leave 0, 2, or 3 data characters after dividing by 4; a leftover group of 1 is always invalid.
- At most two `=` padding characters are allowed, and only at the end.
- Some strings are alphabet-ambiguous because they contain only `A-Z`, `a-z`, and digits; the report marks those as usable with either alphabet.
- Non-canonical trailing bits are reported as warnings: the bytes decode, but strict decoders may reject the final character.
- This is a validator, not a general decoder. It previews obvious UTF-8 text and a few binary signatures, but it does not save decoded bytes.

## FAQ

<details>
<summary>Why does the report say the alphabet is “either”?</summary>

Some Base64 strings contain only letters and digits. Those symbols are shared by standard Base64 and Base64url, so there is no `+`, `/`, `-`, or `_` marker to distinguish the alphabet. The data can still be valid; set the alphabet option if your target system requires one.

</details>

<details>
<summary>Should JWT segments use padding?</summary>

JWT headers, payloads, and signatures are Base64url and are normally unpadded. Choose **URL-safe Base64url** plus **Padding forbidden** when validating a single JWT segment. Validate one segment at a time, not the full dot-separated token.

</details>

<details>
<summary>Why is whitespace ignored by default?</summary>

Many real Base64 values are wrapped for MIME, PEM, email, or terminal output. Ignoring whitespace makes those pasted values validate cleanly. Turn the checkbox off when you need a byte-for-byte strict input check.

</details>

<details>
<summary>What does non-canonical trailing bits mean?</summary>

Base64's final character can contain unused bits. RFC 4648 expects those unused bits to be zero. If they are not zero, the string may decode to the same bytes in permissive decoders, but strict decoders can reject it; the report shows the canonical final character.

</details>
