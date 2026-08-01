## What this tool does

Convert a **standard Base64** string into **URL-safe Base64url** and back, right
in your browser. Nothing is uploaded — it runs locally, works offline, and needs
no sign-up. This is a pure *alphabet* transform: it swaps the two characters that
differ between the alphabets and applies a padding rule. It does **not** decode
Base64 to plain text.

Standard Base64 uses `+` and `/`, which have special meaning in URLs, filenames,
and JWTs. Base64url replaces them with `-` and `_` and usually drops the `=`
padding, so the value can be dropped straight into a query string, path segment,
cookie, or filename.

| | Char 62 | Char 63 | Padding |
| --- | --- | --- | --- |
| **Standard Base64** | `+` | `/` | `=` (kept) |
| **URL-safe Base64url** | `-` | `_` | usually dropped |

## Direction

| Direction | What it does |
| --- | --- |
| **auto** (default) | If the input already contains `-` or `_` it is treated as Base64url and converted to standard; otherwise it is treated as standard Base64 and converted to URL-safe Base64url. |
| **to-url** | Force standard → URL-safe (`+` → `-`, `/` → `_`). |
| **to-standard** | Force URL-safe → standard (`-` → `+`, `_` → `/`). |

## Padding

| Padding | What it does |
| --- | --- |
| **auto** (default) | Pads standard output with `=`, leaves Base64url output unpadded — each alphabet's canonical form. |
| **keep** | Always pad the output to a length that is a multiple of 4. |
| **strip** | Remove all `=` padding from the output. |

Turn on **Validate input is real Base64** to reject a malformed string (wrong
length or stray characters) instead of silently transforming it.

## Examples

| Input | Settings | Output |
| --- | --- | --- |
| `c3ViamVjdHM/X2Q9MQ==` | to-url · auto padding | `c3ViamVjdHM_X2Q9MQ` |
| `c3ViamVjdHM_X2Q9MQ` | to-standard · auto padding | `c3ViamVjdHM/X2Q9MQ==` |
| `+/+/` | auto | `-_-_` |
| `c3ViamVjdHM/X2Q9MQ==` | to-url · keep padding | `c3ViamVjdHM_X2Q9MQ==` |
| `eyJhbGciOiJIUzI1NiJ9` | auto | `eyJhbGciOiJIUzI1NiJ9` |

In the last row the JWT-style token has no `+`, `/`, `-`, or `_`, so both
alphabets agree and only the padding could differ.

## Limits & edge cases

- **Whitespace is ignored.** Spaces, tabs, and newlines are stripped first, so
  line-wrapped MIME Base64 (76-column) and copy-pasted tokens convert cleanly.
- **Alphabet only.** Any character outside `A–Z a–z 0–9 + / - _ =` is rejected
  with a message naming it — this is a Base64 transcoder, not a text encoder.
- **Not a decoder.** It never turns Base64 into the underlying bytes/text; use a
  Base64 *decoder* for that. Here the output is still Base64, just in the other
  alphabet.
- **`=` only at the end.** Padding is only valid as a trailing run; a `=` in the
  middle is treated as malformed input.

## FAQ

<details>
<summary>What is the difference between Base64 and Base64url?</summary>

They encode the same bytes but use two different characters for the last two
symbols of the alphabet. Standard Base64 uses `+` and `/`; URL-safe Base64url
uses `-` and `_` so the value is safe inside URLs, filenames, and JWTs, and it
usually omits the `=` padding.

</details>

<details>
<summary>Does this decode my Base64 to text?</summary>

No. It only converts between the two Base64 alphabets and adjusts padding — the
output is still Base64, just URL-safe (or standard). To recover the original
bytes or text you need a Base64 decoder instead.

</details>

<details>
<summary>Why did the padding disappear?</summary>

With the default **auto** padding, URL-safe Base64url output is left unpadded
because that is its canonical form (and how JWTs are written). Choose **keep**
if the system you are targeting still expects trailing `=` characters.

</details>

<details>
<summary>How does auto-detect decide the direction?</summary>

It looks for the URL-safe markers `-` and `_`. If either is present the input is
assumed to be Base64url and converted to standard; otherwise it is assumed to be
standard Base64 and converted to URL-safe. Set the direction explicitly if your
input is ambiguous.

</details>

<details>
<summary>Can I paste a whole JWT?</summary>

Convert one segment at a time. A JWT is dot-separated Base64url parts
(`header.payload.signature`); the `.` separator is not part of the Base64
alphabet, so paste an individual segment rather than the entire token.

</details>
