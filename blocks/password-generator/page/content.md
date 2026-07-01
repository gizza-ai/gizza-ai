## Generate a strong password in your browser

Create a random **password** or word **passphrase** with a cryptographic RNG.
Everything runs locally in your browser — the generated secret never leaves your
device or touches a server.

### Modes

- **password** — a random string of *length* characters. Lowercase is always
  included; toggle **uppercase**, **digits**, and **symbols** to widen the
  alphabet (and the entropy).
- **passphrase** — *words* random words joined by your **separator** (e.g.
  `able-fire-coat-desk`). Easier to type and remember than a symbol soup.

The output shows the result plus its estimated **entropy in bits** (higher is
stronger). Re-run for a fresh value.

### Tips

- Aim for 16+ characters (passwords) or 4+ words (passphrases).
- Use a unique password per site and store them in a password manager.

## FAQ

<details>
<summary>Is the randomness actually cryptographically secure?</summary>

Yes — characters and words are drawn from the platform's CSPRNG (the browser's
`crypto.getRandomValues` when running as WebAssembly on this page), and indices
are sampled uniformly, so there's no modulo bias skewing some characters to
appear more often. This is the same class of RNG password managers use.

</details>

<details>
<summary>How is the entropy-in-bits number calculated?</summary>

It's `length × log2(alphabet size)`. With all classes on, the alphabet is 85
characters (26 lower + 26 upper + 10 digits + 23 symbols) ≈ 6.4 bits per
character, so a 16-character password scores ≈ 102 bits. Turning off classes
shrinks the alphabet and the score. For passphrases it's
`words × log2(120)` ≈ 6.9 bits per word.

</details>

<details>
<summary>Why does a 4-word passphrase show far less entropy than a 16-character password?</summary>

Because the built-in word list has 120 words, each word contributes only ~6.9
bits — 4 words ≈ 28 bits, versus ~102 bits for a full-alphabet 16-character
password. Passphrases trade raw entropy for memorability; add more words (the
tool allows up to 20) to close the gap.

</details>

<details>
<summary>What limits and character sets does the generator use?</summary>

Password length can be 1–512 characters (default 16); passphrases take 1–20
words (default 4) joined by a separator you choose (default `-`). Lowercase
letters are always included; uppercase, digits, and the symbol set
`!@#$%^&*()-_=+[]{};:,.?` are individually toggleable and all on by default.

</details>
