## What this tool does

Encode any text or bytes to **Base32** (RFC 4648 and its common variants), or
decode a Base32 string back to the original data — instantly, right in your
browser. Nothing is sent to a server: it runs locally, works offline, and needs
no sign-up. Pick a **Mode**, a **Variant**, and a **Data format**, then optionally
toggle **Lowercase** or **Padding**.

## Modes

| Mode | What it does |
| --- | --- |
| **encode** (default) | Turns your input into a Base32 string — e.g. `foobar` becomes `MZXW6YTBOI======`. |
| **decode** | Reverses it — turns a Base32 string back into the original text or bytes. Decoding is case-insensitive and accepts padded or unpadded input. |

## Variants — the alphabet

| Variant | Alphabet | Notes |
| --- | --- | --- |
| **standard** (default) | RFC 4648 §6 — `A–Z` and `2–7` | The everyday Base32, used by TOTP secrets, S/KEY, and many file formats. |
| **hex** | RFC 4648 §7 base32hex — `0–9` and `A–V` | Sorts in the same order as the raw bytes; used in DNSSEC NSEC3. |
| **crockford** | `0–9` and `A–Z` minus the ambiguous `I L O U` | Human-friendly; case-insensitive; never padded. |
| **zbase32** | A permuted, human-oriented alphabet | Designed to be easier to read and type aloud; never padded. |

## Data format — text or raw bytes

| Format | When encoding | When decoding |
| --- | --- | --- |
| **text** (default) | Reads the input as UTF-8 text | Renders the bytes as UTF-8 text (errors if they aren't valid UTF-8) |
| **hex** | Reads the input as a hex byte string (`48 65 6c` or `0x48656c`) | Renders the decoded bytes as hex — use this for binary data |

Switch to **hex** whenever your data is binary and not readable text.

## Options

- **Lowercase** — emit a lowercase alphabet when encoding the `standard` or `hex`
  variants. (Crockford and z-base-32 have a fixed case; decoding is always
  case-insensitive.)
- **Padding** — add `=` padding when encoding `standard`/`hex`, per RFC 4648.
  Turn it off for the compact unpadded form. Crockford and z-base-32 are never
  padded, and decoding accepts either form.

## Examples

| Input | Settings | Output |
| --- | --- | --- |
| `foobar` | encode · standard | `MZXW6YTBOI======` |
| `foobar` | encode · standard · no padding | `MZXW6YTBOI` |
| `foobar` | encode · standard · lowercase | `mzxw6ytboi` |
| `foo` | encode · hex format · data `0x666f6f` | `MZXW6===` |
| `MZXW6YTBOI======` | decode · standard | `foobar` |
| `foobar` | encode · hex (base32hex) | `CPNMUOJ1E8======` |

## FAQ

**Is it free and private?** Yes — your input never leaves your device, and it
keeps working offline once the page has loaded.

**Which variant do I want?** Use **standard** unless you have a reason not to —
it's the RFC 4648 Base32 that most tools mean by "Base32". Choose **hex** for
DNSSEC/NSEC3 and order-preserving keys, **crockford** for human-typed IDs, and
**zbase32** when the string has to be read aloud.

**My decoded output looks garbled.** The bytes probably aren't UTF-8 text.
Switch the **Data format** to **hex** to see the raw bytes.

**Does it handle TOTP / 2FA secrets?** Yes — those are standard RFC 4648 Base32.
Decode them with the **standard** variant.

**Why does padding sometimes do nothing?** Padding only applies to the
`standard` and `hex` variants. Crockford and z-base-32 are defined without
padding, so the toggle is ignored for them.
