## What this tool does

Translate plain text into International Morse code, or decode Morse code back into
text — instantly and right in your browser. Nothing is sent to a server: it runs
locally, works offline, and needs no sign-up. Pick a **Direction**, paste your
input, and optionally tweak the **separators**.

## Direction

| Direction | What it does |
| --- | --- |
| **encode** (default) | Turns text into Morse — `SOS` becomes `... --- ...`. |
| **decode** | Turns Morse back into text — `... --- ...` becomes `SOS`. |

Encoding is case-insensitive (`Hello` and `HELLO` give the same Morse). Any
character that isn't in the Morse alphabet is replaced by the code for `?`, so the
output is still decodable.

## Separators

Morse code on its own is just dots and dashes — the spacing is what tells letters
and words apart. You can set both:

- **Letter separator** — placed between the symbols for each letter. The default
  is a single space (`.... ..` = `HI`).
- **Word separator** — placed between words. The default is ` / `
  (space-slash-space), the convention used in most printed Morse.

Leave a field blank to use its default. When decoding, the tool also accepts an
underscore (`_`) anywhere a dash (`-`) would appear.

## Supported characters

- **Letters:** A–Z (case-insensitive).
- **Digits:** 0–9.
- **Punctuation:** `. , ? ' ! / ( ) & : ; = + - _ " $ @`

## Examples

| Input | Direction | Output |
| --- | --- | --- |
| `SOS` | encode | `... --- ...` |
| `Hello World` | encode | `.... . .-.. .-.. --- / .-- --- .-. .-.. -..` |
| `... --- ...` | decode | `SOS` |
| `.... .. / -.-- --- ..-` | decode | `HI YOU` |
| `2026` | encode | `..--- ----- ..--- -....` |

## FAQ

<details>
<summary>Is it free and private?</summary>
<p>Yes — your input never leaves your device, and the translator keeps working
offline once the page has loaded. There is no sign-up and nothing is logged.</p>
</details>

<details>
<summary>Which Morse standard does it use?</summary>
<p>The International Morse code (ITU-R M.1677-1) alphabet — the same letters,
digits, and punctuation used worldwide for amateur radio and signalling, plus a
few everyday extras like <code>@</code> and <code>$</code>.</p>
</details>

<details>
<summary>What happens to characters that have no Morse code?</summary>
<p>When encoding, any character outside the supported set is replaced by the code
for <code>?</code> (<code>..--..</code>) so the message still translates cleanly.</p>
</details>

<details>
<summary>How do I decode Morse with unusual spacing?</summary>
<p>Set the <strong>Letter separator</strong> and <strong>Word separator</strong> to
match the spacing in your input. For example, if words are split with a pipe
(<code>|</code>) and letters by a single space, set the word separator to
<code>|</code> and leave the letter separator blank.</p>
</details>

<details>
<summary>Can I convert numbers and punctuation?</summary>
<p>Yes — digits 0–9 and common punctuation (<code>. , ? ! / : ; = + - " $ @</code>
and more) all have Morse codes and round-trip both ways.</p>
</details>
