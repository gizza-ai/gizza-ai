## What this tool does

Encrypt and decrypt text with four classic pre-computer ciphers — **Caesar**,
**Vigenère**, **Atbash**, and **rail-fence** — entirely in your browser. Nothing
is sent to a server: it runs locally, works offline, and needs no sign-up. Pick a
**Cipher**, an **Operation**, fill in the **Key** if the cipher needs one, and
paste your **Text**.

> These are educational / puzzle ciphers (great for CTFs, escape rooms, and
> cryptograms). They are **not** secure encryption — do not use them to protect
> real secrets.

## The ciphers

| Cipher | Key | What it does |
| --- | --- | --- |
| **Caesar** | shift number (default 3) | Shifts every letter forward by a fixed amount around the alphabet. Shift 3 turns `Hello` into `Khoor`. Negative and large shifts wrap (mod 26). |
| **Vigenère** | a keyword (e.g. `LEMON`) | Shifts each letter by the next letter of a repeating keyword — a polyalphabetic Caesar. `ATTACKATDAWN` with key `LEMON` → `LXFOPVEFRNHR`. |
| **Atbash** | none | Mirrors the alphabet: A↔Z, B↔Y, … It is its own inverse, so encrypt and decrypt are the same. `Hello` → `Svool`. |
| **Rail-fence** | number of rails (2–64, default 3) | Writes the text in a zig-zag across N rails, then reads it off rail by rail — a transposition cipher (spaces and punctuation move too). |

In Caesar, Vigenère, and Atbash, **case is preserved** and digits, spaces, and
punctuation pass through unchanged. Rail-fence transposes the whole string,
including spaces.

## Operations

| Operation | What it does |
| --- | --- |
| **encrypt** (default) | Apply the cipher with your key. |
| **decrypt** | Reverse it with the same key. |
| **brute-force** | *Caesar only.* Lists all 26 possible shifts as `shift NN: <text>` lines so you can spot the one that reads as plain English. |

## Examples

| Text | Cipher · Operation · Key | Result |
| --- | --- | --- |
| `Hello, World!` | Caesar · encrypt · `3` | `Khoor, Zruog!` |
| `Khoor` | Caesar · brute-force | …`shift  3: Hello`… |
| `ATTACKATDAWN` | Vigenère · encrypt · `LEMON` | `LXFOPVEFRNHR` |
| `Hello` | Atbash · encrypt | `Svool` |
| `WEAREDISCOVEREDFLEEATONCE` | Rail-fence · encrypt · `3` | `WECRLTEERDSOEEFEAOCAIVDEN` |

## FAQ

<details>
<summary>Is it free and private?</summary>

Yes — your text never leaves your device, and it keeps
working offline once the page has loaded.

</details>

<details>
<summary>Is ROT13 supported?</summary>

Yes — ROT13 is just a Caesar cipher with shift `13`. (And
because the alphabet is 26 letters, encrypting and decrypting ROT13 are the same.)

</details>

<details>
<summary>The Vigenère key has spaces or numbers — does that matter?</summary>

No. Only the
letters of the key are used; spaces, digits, and punctuation in the key are
ignored. The key just needs at least one letter.

</details>

<details>
<summary>I have a Caesar message but don't know the shift.</summary>

Choose **brute-force** — it
prints all 26 shifts at once so you can read off the one that makes sense.

</details>

<details>
<summary>Are these secure?</summary>

No. Classical ciphers are trivially broken with modern tools
(and the brute-force here demonstrates exactly that for Caesar). Use them for
puzzles and learning, not real security.

</details>
