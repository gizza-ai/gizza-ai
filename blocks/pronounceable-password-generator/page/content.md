## Make a password you can actually say

Create a **pronounceable** password in your browser. The letters strictly
alternate consonant and vowel — starting with a consonant — so the core is
always speakable (like `bofuka` or `tequni`) instead of a random symbol soup.
A few **digits** and **symbols** are then appended to satisfy site rules and
raise the entropy. Everything runs locally; the generated secret never leaves
your device or touches a server.

### How it works

- **Letters** — pick how many pronounceable letters to generate (4–64,
  default 12). Consonants are drawn from an 18-letter set (`q`, `x`, and `y`
  are dropped to avoid awkward clusters) and vowels from `aeiou`.
- **Capitalize first letter** — upper-cases the first character (on by default)
  so the result meets "needs a capital" rules.
- **Digits** — a run of random digits appended to the end (0–12, default 2).
  A two-digit suffix is a common way to add entropy without hurting readability.
- **Symbols** — random symbols from `!@#$%&*?-_+=` appended after the digits
  (0–12, default 1).

For example, `Bofuka92!` is 6 consonant/vowel letters plus two digits and a
symbol. The output shows the result plus its estimated **entropy in bits**
(higher is stronger). Re-run for a fresh value.

### Tips

- Aim for 12+ letters plus a digit or two for everyday accounts, and 16+ for
  important ones.
- Use a unique password per site and store them in a password manager.

## FAQ

<details>
<summary>Why is it easier to remember than a fully random password?</summary>

Because the letters follow English phonetic rhythm — consonant, vowel,
consonant, vowel — the core reads like a made-up word you can pronounce, such as
`bofuka`. Your brain chunks it into syllables instead of memorizing unrelated
characters, so it is easier to type from memory and even say aloud when reading
it to someone.

</details>

<details>
<summary>Is the randomness actually cryptographically secure?</summary>

Yes — every letter, digit, and symbol is drawn from the platform's CSPRNG (the
browser's `crypto.getRandomValues` when running as WebAssembly on this page), and
indices are sampled uniformly with rejection sampling, so there is no modulo bias
skewing some characters to appear more often.

</details>

<details>
<summary>How is the entropy-in-bits number calculated?</summary>

It sums the actual random choices: each consonant slot is `log2(18)` ≈ 4.17
bits, each vowel slot `log2(5)` ≈ 2.32 bits, each digit `log2(10)` ≈ 3.32 bits,
and each symbol `log2(12)` ≈ 3.58 bits. Capitalizing the first letter is
deterministic and adds nothing. A 12-letter password with 2 digits and 1 symbol
scores about **49 bits**.

</details>

<details>
<summary>What are the limits and character sets?</summary>

Length can be 4–64 letters (default 12); digits and symbols can each be 0–12
(defaults 2 and 1). Consonants come from `bcdfghjklmnprstvwz` (18 letters, no
`q`/`x`/`y`), vowels from `aeiou`, digits from `0123456789`, and symbols from
`!@#$%&*?-_+=`. Setting digits and symbols to 0 gives a pure pronounceable
letter string.

</details>
