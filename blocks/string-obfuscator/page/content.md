## What this tool does

Mask or obfuscate a string right in your browser. Hide the middle of an API
key, token, email, or any other sensitive value before a screenshot — or
transform text with ROT13, leetspeak, or Unicode homoglyphs. Nothing is sent to
a server: it runs locally, works offline, and needs no sign-up.

## Modes

| Mode | What it does |
| --- | --- |
| **mask** (default) | Keeps the first *N* and last *M* characters visible and replaces the rest with a mask character. The classic way to redact a secret in a screenshot. |
| **rot** | Caesar / ROT-N rotation of the letters. The default shift is **13** (ROT13), which is its own inverse — apply it twice to get back the original. |
| **leetspeak** | Swaps letters for look-alike digits and symbols: `a→4`, `e→3`, `i→1`, `o→0`, `s→5`, `t→7`, `b→8`, `g→9`. |
| **homoglyph** | Swaps ASCII letters for identical-looking Unicode characters (mostly Cyrillic), so the text *looks* the same but is a different string. Useful for demoing homograph spoofing or defeating naive copy-paste. |

## Masking options

- **Mask character** — the symbol used for the hidden part (default `*`; try `•`
  or `#`). Only the first character you type is used.
- **Keep first N** / **Keep last N** — how many characters to leave visible at
  each end. Whitespace stays visible too, so the masked shape still reads
  naturally. If the kept ends overlap, the whole string is shown unchanged.

## Examples

| Input | Mode · Settings | Output |
| --- | --- | --- |
| `sk-1234567890abcdef` | mask · keep 5 / 4 | `sk-12**********cdef` |
| `password` | mask · keep 1 / 1 · char `•` | `p••••••d` |
| `Hello, World!` | rot · 13 | `Uryyb, Jbeyq!` |
| `elite hacker` | leetspeak | `31173 h4ck3r` |
| `paypal` | homoglyph | looks like `paypal`, different bytes |

## FAQ

<details>
<summary>Is it free and private?</summary>

Yes — your input never leaves your device, and it
keeps working offline once the page has loaded.

</details>

<details>
<summary>Is masking reversible?</summary>

No. `mask` permanently removes the hidden characters,
which is exactly what you want for redaction. `rot` is reversible (ROT13 twice
returns the original); `leetspeak` and `homoglyph` are not cleanly reversible.

</details>

<details>
<summary>What is a homoglyph?</summary>

A character that looks identical to another but has a
different Unicode codepoint — e.g. the Latin `a` (U+0061) versus the Cyrillic
`а` (U+0430). They render the same but are different bytes.

</details>

<details>
<summary>Can I use ROT13 to secure data?</summary>

No — ROT13 is a toy cipher with no key. It
hides text from a casual glance only; never use it for real secrets.

</details>
